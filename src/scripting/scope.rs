use crate::checks::MigrationContext;
use crate::config::Config;
use pg_query::protobuf::node::Node as NodeEnum;

/// Serialize runtime inputs and bind them into a Rhai scope.
pub(super) fn script_scope(
    node: &NodeEnum,
    config: &Config,
    ctx: &MigrationContext,
) -> std::result::Result<rhai::Scope<'static>, String> {
    let node = rhai::serde::to_dynamic(node).map_err(|err| err.to_string())?;
    let config = rhai::serde::to_dynamic(config).map_err(|err| err.to_string())?;
    let ctx = rhai::serde::to_dynamic(ctx).map_err(|err| err.to_string())?;

    let mut scope = rhai::Scope::new();
    scope.push("node", node);
    scope.push("config", config);
    scope.push("ctx", ctx);
    Ok(scope)
}
