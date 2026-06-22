use super::{CustomCheck, ScriptInputs};
use crate::checks::MigrationContext;
use crate::config::Config;
use crate::violation::Violation;
use pg_query::protobuf::node::Node as NodeEnum;
use rhai::Dynamic;

impl CustomCheck {
    /// Build a violation for failures preparing script inputs.
    pub(super) fn internal_error(&self, err: &dyn std::fmt::Display) -> Vec<Violation> {
        vec![Violation::new(
            format!("SCRIPT ERROR: {}", self.name),
            format!("Error in custom check '{}': {err}", self.name),
            "This is likely a diesel-guard bug. Please report it.",
        )]
    }

    /// Serialize runtime inputs and bind them into a Rhai scope.
    pub(super) fn script_scope(
        node: &NodeEnum,
        config: &Config,
        ctx: &MigrationContext,
    ) -> std::result::Result<rhai::Scope<'static>, String> {
        script_inputs(node, config, ctx).map(script_scope_from_inputs)
    }
}

/// Serialize all inputs needed by a custom check script.
pub(super) fn script_inputs(
    node: &NodeEnum,
    config: &Config,
    ctx: &MigrationContext,
) -> std::result::Result<ScriptInputs, String> {
    let node = script_node_input(node)?;
    script_inputs_with_node(node, config, ctx)
}

/// Serialize config and context after the AST node has been prepared.
pub(super) fn script_inputs_with_node(
    node: Dynamic,
    config: &Config,
    ctx: &MigrationContext,
) -> std::result::Result<ScriptInputs, String> {
    let config = script_config_input(config)?;
    script_inputs_with_node_config(node, config, ctx)
}

/// Assemble script inputs after node and config values have been serialized.
pub(super) fn script_inputs_with_node_config(
    node: Dynamic,
    config: Dynamic,
    ctx: &MigrationContext,
) -> std::result::Result<ScriptInputs, String> {
    Ok(ScriptInputs {
        node,
        config,
        ctx: script_context_input(ctx)?,
    })
}

/// Serialize a PostgreSQL AST node for Rhai.
pub(super) fn script_node_input(node: &NodeEnum) -> std::result::Result<Dynamic, String> {
    rhai::serde::to_dynamic(node).map_err(|err| err.to_string())
}

/// Serialize runtime configuration for Rhai.
pub(super) fn script_config_input(config: &Config) -> std::result::Result<Dynamic, String> {
    rhai::serde::to_dynamic(config).map_err(|err| err.to_string())
}

/// Serialize migration context for Rhai.
pub(super) fn script_context_input(ctx: &MigrationContext) -> std::result::Result<Dynamic, String> {
    rhai::serde::to_dynamic(ctx).map_err(|err| err.to_string())
}

/// Bind prepared script inputs into a Rhai scope.
pub(super) fn script_scope_from_inputs(inputs: ScriptInputs) -> rhai::Scope<'static> {
    let mut scope = rhai::Scope::new();
    scope.push("node", inputs.node);
    scope.push("config", inputs.config);
    scope.push("ctx", inputs.ctx);
    scope
}
