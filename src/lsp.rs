//! LSP server implementation for diesel-guard.
//!
//! Exposes a `run()` entry point that starts a JSON-RPC Language Server over
//! stdin/stdout. Editors spawn the process and communicate via the LSP protocol.
//!
//! Only `.sql` files are checked. Diagnostics are published:
//! - On `didOpen` and `didChange` using the in-memory document content
//!   (via `check_sql`, which uses a default migration context).
//! - On `didSave` using the saved file on disk
//!   (via `check_file`, which picks up `metadata.toml` / `-- no-transaction`).
//! - On `didClose` diagnostics are cleared.

use std::ops::ControlFlow;

use async_lsp::ClientSocket;
use async_lsp::client_monitor::ClientProcessMonitorLayer;
use async_lsp::lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Initialized, PublishDiagnostics,
};
use async_lsp::lsp_types::request::Initialize;
use async_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, InitializeResult, Position, PublishDiagnosticsParams, Range,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::router::Router;
use async_lsp::server::LifecycleLayer;
use camino::Utf8PathBuf;
use diesel_guard::violation::Severity;
use diesel_guard::{SafetyChecker, ViolationList};
use tower::ServiceBuilder;

struct ServerState {
    client: ClientSocket,
    checker: SafetyChecker,
}

impl ServerState {
    fn new(client: ClientSocket) -> Self {
        // SafetyChecker::new() loads diesel-guard.toml from CWD.
        // Editors typically launch the LSP from the workspace root, so this
        // finds the config in most cases.
        Self {
            client,
            checker: SafetyChecker::new(),
        }
    }

    fn to_diagnostics(violations: ViolationList) -> Vec<Diagnostic> {
        violations
            .into_iter()
            .map(|(line, v)| {
                let line = u32::try_from(line).unwrap_or(u32::MAX).saturating_sub(1);
                Diagnostic {
                    range: Range {
                        start: Position { line, character: 0 },
                        end: Position {
                            line,
                            character: u32::MAX,
                        },
                    },
                    severity: Some(match v.severity {
                        Severity::Error => DiagnosticSeverity::ERROR,
                        Severity::Warning => DiagnosticSeverity::WARNING,
                    }),
                    source: Some("diesel-guard".into()),
                    message: format!("{}: {}\n\n{}", v.operation, v.problem, v.safe_alternative),
                    ..Default::default()
                }
            })
            .collect()
    }

    fn publish_sql(&mut self, uri: Url, sql: &str) {
        let diagnostics = match self.checker.check_sql(sql) {
            Ok(violations) => Self::to_diagnostics(violations),
            Err(_) => return,
        };
        self.client
            .notify::<PublishDiagnostics>(PublishDiagnosticsParams {
                uri,
                diagnostics,
                version: None,
            })
            .ok();
    }

    fn publish_file(&mut self, uri: Url) {
        let Ok(path) = uri.to_file_path() else { return };
        let Ok(utf8_path) = Utf8PathBuf::from_path_buf(path) else {
            return;
        };
        let diagnostics = match self.checker.check_file(&utf8_path) {
            Ok(violations) => Self::to_diagnostics(violations),
            Err(_) => return,
        };
        self.client
            .notify::<PublishDiagnostics>(PublishDiagnosticsParams {
                uri,
                diagnostics,
                version: None,
            })
            .ok();
    }

    fn clear(&mut self, uri: Url) {
        self.client
            .notify::<PublishDiagnostics>(PublishDiagnosticsParams {
                uri,
                diagnostics: vec![],
                version: None,
            })
            .ok();
    }
}

fn is_sql(uri: &Url) -> bool {
    uri.path().to_ascii_lowercase().ends_with(".sql")
}

pub async fn run() {
    let (server, _) = async_lsp::MainLoop::new_server(|client| {
        let mut router = Router::new(ServerState::new(client.clone()));

        router
            .request::<Initialize, _>(|_, _| async move {
                Ok(InitializeResult {
                    capabilities: ServerCapabilities {
                        text_document_sync: Some(TextDocumentSyncCapability::Kind(
                            TextDocumentSyncKind::FULL,
                        )),
                        ..ServerCapabilities::default()
                    },
                    server_info: None,
                })
            })
            .notification::<Initialized>(|_, _| ControlFlow::Continue(()))
            .notification::<DidOpenTextDocument>(|st, params| {
                let uri = params.text_document.uri;
                let text = params.text_document.text;
                if is_sql(&uri) {
                    st.publish_sql(uri, &text);
                }
                ControlFlow::Continue(())
            })
            .notification::<DidChangeTextDocument>(|st, params| {
                // FULL sync — take the last (and only) change as the full content.
                if let Some(change) = params.content_changes.into_iter().next_back() {
                    let uri = params.text_document.uri;
                    if is_sql(&uri) {
                        st.publish_sql(uri, &change.text);
                    }
                }
                ControlFlow::Continue(())
            })
            .notification::<DidSaveTextDocument>(|st, params| {
                let uri = params.text_document.uri;
                if is_sql(&uri) {
                    st.publish_file(uri);
                }
                ControlFlow::Continue(())
            })
            .notification::<DidCloseTextDocument>(|st, params| {
                st.clear(params.text_document.uri);
                ControlFlow::Continue(())
            });

        ServiceBuilder::new()
            .layer(LifecycleLayer::default())
            .layer(CatchUnwindLayer::default())
            .layer(ClientProcessMonitorLayer::new(client))
            .service(router)
    });

    #[cfg(unix)]
    let (stdin, stdout) = (
        async_lsp::stdio::PipeStdin::lock_tokio().unwrap(),
        async_lsp::stdio::PipeStdout::lock_tokio().unwrap(),
    );
    #[cfg(not(unix))]
    let (stdin, stdout) = (
        tokio_util::compat::TokioAsyncReadCompatExt::compat(tokio::io::stdin()),
        tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(tokio::io::stdout()),
    );

    server.run_buffered(stdin, stdout).await.unwrap();
}
