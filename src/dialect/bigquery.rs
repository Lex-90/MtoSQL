/// BigQuery dialect implementation.
use super::{Dialect, TopNSyntax};
use crate::parser::ast::MType;

/// BigQuery dialect.
pub struct BigQuery;

impl Dialect for BigQuery {
    fn name(&self) -> &'static str {
        "bigquery"
    }

    fn identifier_quote(&self) -> char {
        '`'
    }

    fn ilike_supported(&self) -> bool {
        false
    }

    fn supports_select_except(&self) -> bool {
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
            MType::Text => "STRING",
            MType::Number => "FLOAT64",
            MType::Integer => "INT64",
            MType::Logical => "BOOL",
            MType::Date => "DATE",
            MType::DateTime => "DATETIME",
            MType::DateTimeZone => "TIMESTAMP",
            MType::Duration => "/* UNSUPPORTED_TYPE */",
            MType::Binary => "BYTES",
            MType::Table => "/* UNSUPPORTED_TYPE */",
            MType::Any => "STRING",
            MType::Unknown(_) => "/* UNSUPPORTED_TYPE */",
        }
    }
}
