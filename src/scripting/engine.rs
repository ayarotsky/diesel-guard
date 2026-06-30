use rhai::Engine;

/// Build a Rhai module exposing commonly needed pg_query protobuf enum constants.
///
/// Scripts access these as `pg::OBJECT_TABLE`, `pg::AT_ADD_COLUMN`, etc.
fn create_pg_constants_module() -> rhai::Module {
    use pg_query::protobuf::{AlterTableType, ConstrType, DropBehavior, ObjectType};

    let mut m = rhai::Module::new();

    // ObjectType — used by DropStmt.remove_type, RenameStmt.rename_type, etc.
    m.set_var("OBJECT_INDEX", ObjectType::ObjectIndex as i64);
    m.set_var("OBJECT_TABLE", ObjectType::ObjectTable as i64);
    m.set_var("OBJECT_COLUMN", ObjectType::ObjectColumn as i64);
    m.set_var("OBJECT_DATABASE", ObjectType::ObjectDatabase as i64);
    m.set_var("OBJECT_SCHEMA", ObjectType::ObjectSchema as i64);
    m.set_var("OBJECT_SEQUENCE", ObjectType::ObjectSequence as i64);
    m.set_var("OBJECT_VIEW", ObjectType::ObjectView as i64);
    m.set_var("OBJECT_FUNCTION", ObjectType::ObjectFunction as i64);
    m.set_var("OBJECT_EXTENSION", ObjectType::ObjectExtension as i64);
    m.set_var("OBJECT_TRIGGER", ObjectType::ObjectTrigger as i64);
    m.set_var("OBJECT_TYPE", ObjectType::ObjectType as i64);

    // AlterTableType — used by AlterTableCmd.subtype
    m.set_var("AT_ADD_COLUMN", AlterTableType::AtAddColumn as i64);
    m.set_var("AT_COLUMN_DEFAULT", AlterTableType::AtColumnDefault as i64);
    m.set_var("AT_DROP_NOT_NULL", AlterTableType::AtDropNotNull as i64);
    m.set_var("AT_SET_NOT_NULL", AlterTableType::AtSetNotNull as i64);
    m.set_var("AT_DROP_COLUMN", AlterTableType::AtDropColumn as i64);
    m.set_var(
        "AT_ALTER_COLUMN_TYPE",
        AlterTableType::AtAlterColumnType as i64,
    );
    m.set_var("AT_ADD_CONSTRAINT", AlterTableType::AtAddConstraint as i64);
    m.set_var(
        "AT_DROP_CONSTRAINT",
        AlterTableType::AtDropConstraint as i64,
    );
    m.set_var(
        "AT_VALIDATE_CONSTRAINT",
        AlterTableType::AtValidateConstraint as i64,
    );

    // ConstrType — used by Constraint.contype
    m.set_var("CONSTR_NOTNULL", ConstrType::ConstrNotnull as i64);
    m.set_var("CONSTR_DEFAULT", ConstrType::ConstrDefault as i64);
    m.set_var("CONSTR_IDENTITY", ConstrType::ConstrIdentity as i64);
    m.set_var("CONSTR_GENERATED", ConstrType::ConstrGenerated as i64);
    m.set_var("CONSTR_CHECK", ConstrType::ConstrCheck as i64);
    m.set_var("CONSTR_PRIMARY", ConstrType::ConstrPrimary as i64);
    m.set_var("CONSTR_UNIQUE", ConstrType::ConstrUnique as i64);
    m.set_var("CONSTR_EXCLUSION", ConstrType::ConstrExclusion as i64);
    m.set_var("CONSTR_FOREIGN", ConstrType::ConstrForeign as i64);

    // DropBehavior — used by DropStmt.behavior
    m.set_var("DROP_RESTRICT", DropBehavior::DropRestrict as i64);
    m.set_var("DROP_CASCADE", DropBehavior::DropCascade as i64);

    m
}

/// Create a sandboxed Rhai engine with safety limits.
pub(super) fn create_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(100_000);
    engine.set_max_string_size(10_000);
    engine.set_max_array_size(1_000);
    engine.set_max_map_size(1_000);
    engine.register_static_module("pg", create_pg_constants_module().into());
    engine
}
