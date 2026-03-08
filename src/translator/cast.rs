/// M type → SQL type coercion rules.
use crate::parser::ast::MType;
use crate::parser::grammar::string_to_type;

/// Parse a type annotation from an M expression for use in CAST.
pub fn resolve_m_type(type_expr: &str) -> MType {
    // Handle common type identifiers
    let cleaned = type_expr
        .trim()
        .strip_prefix("type ")
        .unwrap_or(type_expr)
        .trim();
    string_to_type(cleaned)
}

/// Check if a type is unsupported for a given dialect.
pub fn is_unsupported(m_type: &MType, dialect_name: &str) -> bool {
    match m_type {
        MType::Duration => {
            matches!(dialect_name, "tsql" | "bigquery" | "snowflake")
        }
        MType::Table | MType::Unknown(_) => true,
        _ => false,
    }
}
