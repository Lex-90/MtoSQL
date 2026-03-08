/// T-SQL dialect implementation.
use super::{Dialect, TopNSyntax};
use crate::parser::ast::MType;

/// T-SQL dialect.
pub struct TSql;

impl Dialect for TSql {
    fn name(&self) -> &'static str {
        "tsql"
    }

    fn identifier_quote(&self) -> char {
        '['
    }

    fn identifier_close_quote(&self) -> char {
        ']'
    }

    fn ilike_supported(&self) -> bool {
        false
    }

    fn top_n_syntax(&self) -> TopNSyntax {
        TopNSyntax::Top
    }

    fn boolean_literal(&self, val: bool) -> &'static str {
        if val {
            "1"
        } else {
            "0"
        }
    }

    fn current_timestamp(&self) -> &'static str {
        "GETDATE()"
    }

    fn map_type(&self, m_type: &MType) -> &'static str {
        match m_type {
            MType::Text => "NVARCHAR(MAX)",
            MType::Number => "FLOAT",
            MType::Integer => "BIGINT",
            MType::Logical => "BIT",
            MType::Date => "DATE",
            MType::DateTime => "DATETIME2",
            MType::DateTimeZone => "DATETIMEOFFSET",
            MType::Duration => "/* UNSUPPORTED_TYPE */",
            MType::Binary => "VARBINARY(MAX)",
            MType::Table => "/* UNSUPPORTED_TYPE */",
            MType::Any => "SQL_VARIANT",
            MType::Unknown(_) => "/* UNSUPPORTED_TYPE */",
        }
    }
}
