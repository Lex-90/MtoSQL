/// Snowflake dialect implementation.
use super::{Dialect, TopNSyntax};
use crate::parser::ast::MType;

/// Snowflake dialect.
pub struct Snowflake;

impl Dialect for Snowflake {
    fn name(&self) -> &'static str {
        "snowflake"
    }

    fn identifier_quote(&self) -> char {
        '"'
    }

    fn ilike_supported(&self) -> bool {
        true
    }

    fn top_n_syntax(&self) -> TopNSyntax {
        TopNSyntax::Limit
    }

    fn current_timestamp(&self) -> &'static str {
        "CURRENT_TIMESTAMP()"
    }

    fn map_type(&self, m_type: &MType) -> &'static str {
        match m_type {
            MType::Text => "VARCHAR",
            MType::Number => "FLOAT",
            MType::Integer => "BIGINT",
            MType::Logical => "BOOLEAN",
            MType::Date => "DATE",
            MType::DateTime => "TIMESTAMP_NTZ",
            MType::DateTimeZone => "TIMESTAMP_TZ",
            MType::Duration => "/* UNSUPPORTED_TYPE */",
            MType::Binary => "BINARY",
            MType::Table => "/* UNSUPPORTED_TYPE */",
            MType::Any => "VARIANT",
            MType::Unknown(_) => "/* UNSUPPORTED_TYPE */",
        }
    }
}
