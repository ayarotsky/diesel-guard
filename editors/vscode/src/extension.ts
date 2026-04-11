import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient;

export function activate(ctx: vscode.ExtensionContext): void {
  const config = vscode.workspace.getConfiguration("diesel-guard");
  const binary = config.get<string>("binaryPath", "diesel-guard");

  const serverOptions: ServerOptions = {
    command: binary,
    args: ["lsp"],
    transport: TransportKind.stdio,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "sql" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.sql"),
    },
  };

  client = new LanguageClient(
    "diesel-guard",
    "diesel-guard",
    serverOptions,
    clientOptions
  );

  ctx.subscriptions.push(client.start());
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
