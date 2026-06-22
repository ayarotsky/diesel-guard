use super::{CustomCheck, result::parse_script_result};
use crate::checks::{Check, MigrationContext};
use crate::config::Config;
use crate::violation::Violation;
use pg_query::protobuf::node::Node as NodeEnum;
use rhai::Dynamic;

impl CustomCheck {
    /// Evaluate the compiled Rhai script and parse its return value.
    fn evaluate_custom_check(&self, scope: &mut rhai::Scope<'_>) -> Vec<Violation> {
        match self.engine.eval_ast_with_scope::<Dynamic>(scope, &self.ast) {
            Ok(result) => parse_script_result(self.name, result),
            Err(err) => vec![self.runtime_error_violation(&err)],
        }
    }

    /// Build a violation for a runtime error raised by the Rhai script.
    fn runtime_error_violation(&self, err: &dyn std::fmt::Display) -> Violation {
        Violation::new(
            format!("SCRIPT ERROR: {}", self.name),
            format!("Runtime error in custom check '{}': {err}", self.name),
            "Fix the custom check script to eliminate the runtime error.",
        )
    }

    /// Evaluate this check after preparing its Rhai scope.
    pub(super) fn check_with_scope_result(
        &self,
        scope: std::result::Result<rhai::Scope<'static>, String>,
    ) -> Vec<Violation> {
        let mut scope = match scope {
            Ok(scope) => scope,
            Err(err) => return self.internal_error(&err),
        };
        self.evaluate_custom_check(&mut scope)
    }
}

impl Check for CustomCheck {
    /// Return the custom check name.
    fn name(&self) -> &'static str {
        self.name
    }

    /// Return the source path for this custom check script.
    fn script_path(&self) -> Option<&str> {
        Some(&self.path)
    }

    /// Return documentation produced by the script's optional `describe` function.
    fn describe(&self) -> Option<String> {
        // clone_functions_only() strips the script body so call_fn won't
        // try to evaluate statements that reference `node`.
        let fns_ast = self.ast.clone_functions_only();
        let mut scope = rhai::Scope::new();
        if let Ok(result) =
            self.engine
                .call_fn::<rhai::Dynamic>(&mut scope, &fns_ast, "describe", ())
        {
            return result.into_string().ok();
        }
        None
    }

    /// Run the custom check against one parsed PostgreSQL AST node.
    fn check(&self, node: &NodeEnum, config: &Config, ctx: &MigrationContext) -> Vec<Violation> {
        self.check_with_scope_result(Self::script_scope(node, config, ctx))
    }
}
