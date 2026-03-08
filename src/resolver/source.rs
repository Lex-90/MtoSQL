/// Infers table/schema names from M data-source expressions.
use crate::parser::ast::*;

/// Resolved data source information.
#[derive(Debug, Clone)]
pub struct ResolvedSource {
    /// Database/schema name.
    pub schema: Option<String>,
    /// Table name.
    pub table: Option<String>,
    /// Raw source expression for unrecognised patterns.
    pub raw: Option<String>,
}

/// Resolve a data source from an M expression.
pub fn resolve_source(expr: &MExpr) -> ResolvedSource {
    match expr {
        // Source{[Name="TableName"]}[Data]
        MExpr::FieldAccess { expr, field } if field == "Data" => {
            if let MExpr::IndexAccess {
                expr: source_expr,
                key,
            } = expr.as_ref()
            {
                if let Some((schema, table)) = extract_index_record(key) {
                    let parent_schema = resolve_parent_schema(source_expr);
                    return ResolvedSource {
                        schema: schema.or(parent_schema),
                        table: Some(table),
                        raw: None,
                    };
                }
            }
            ResolvedSource {
                schema: None,
                table: None,
                raw: Some(format_expr(expr)),
            }
        }
        // Sql.Database("server", "db")
        MExpr::FunctionCall { name, args } if name == "Sql.Database" => {
            let schema = args.get(1).and_then(|a| {
                if let MExpr::Literal(MLiteral::Text(s)) = a {
                    Some(s.clone())
                } else {
                    None
                }
            });
            ResolvedSource {
                schema,
                table: None,
                raw: None,
            }
        }
        // Excel.Workbook(...)
        MExpr::FunctionCall { name, .. } if name == "Excel.Workbook" => ResolvedSource {
            schema: None,
            table: None,
            raw: Some(format!("Excel.Workbook(...)")),
        },
        _ => ResolvedSource {
            schema: None,
            table: None,
            raw: Some(format_expr(expr)),
        },
    }
}

/// Extract schema and table from an index record like `[Name="Orders"]` or `[Schema="s", Item="t"]`.
fn extract_index_record(key: &MExpr) -> Option<(Option<String>, String)> {
    if let MExpr::Record(fields) = key {
        let mut name = None;
        let mut schema = None;
        let mut item = None;

        for (k, v) in fields {
            if let MExpr::Literal(MLiteral::Text(s)) = v {
                match k.as_str() {
                    "Name" => name = Some(s.clone()),
                    "Schema" => schema = Some(s.clone()),
                    "Item" => item = Some(s.clone()),
                    _ => {}
                }
            }
        }

        if let Some(table) = item {
            return Some((schema, table));
        }
        if let Some(table) = name {
            return Some((None, table));
        }
    }
    None
}

/// Try to resolve the schema from a parent data source expression.
fn resolve_parent_schema(expr: &MExpr) -> Option<String> {
    match expr {
        MExpr::FunctionCall { name, args } if name == "Sql.Database" => args.get(1).and_then(|a| {
            if let MExpr::Literal(MLiteral::Text(s)) = a {
                Some(s.clone())
            } else {
                None
            }
        }),
        _ => None,
    }
}

/// Format an M expression to a short string (for diagnostics).
pub fn format_expr(expr: &MExpr) -> String {
    match expr {
        MExpr::Identifier(name) => name.clone(),
        MExpr::Literal(MLiteral::Text(s)) => format!("\"{}\"", s),
        MExpr::Literal(MLiteral::Integer(n)) => n.to_string(),
        MExpr::Literal(MLiteral::Number(n)) => n.to_string(),
        MExpr::Literal(MLiteral::Bool(b)) => b.to_string(),
        MExpr::Literal(MLiteral::Null) => "null".to_string(),
        MExpr::FunctionCall { name, args } => {
            let args_str: Vec<String> = args.iter().map(|a| format_expr(a)).collect();
            format!("{}({})", name, args_str.join(", "))
        }
        MExpr::FieldAccess { expr, field } => format!("{}[{}]", format_expr(expr), field),
        MExpr::IndexAccess { expr, key } => {
            format!("{}{{{}}}", format_expr(expr), format_expr(key))
        }
        MExpr::RowField(name) => format!("[{}]", name),
        MExpr::BinaryOp { op, left, right } => {
            let op_str = match op {
                BinOp::Eq => "=",
                BinOp::Neq => "<>",
                BinOp::Lt => "<",
                BinOp::Lte => "<=",
                BinOp::Gt => ">",
                BinOp::Gte => ">=",
                BinOp::And => "and",
                BinOp::Or => "or",
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Concat => "&",
            };
            format!("{} {} {}", format_expr(left), op_str, format_expr(right))
        }
        MExpr::UnaryOp { op, expr } => {
            let op_str = match op {
                UnOp::Not => "not ",
                UnOp::Neg => "-",
            };
            format!("{}{}", op_str, format_expr(expr))
        }
        MExpr::List(items) => {
            let items_str: Vec<String> = items.iter().map(|i| format_expr(i)).collect();
            format!("{{{}}}", items_str.join(", "))
        }
        MExpr::Record(fields) => {
            let fields_str: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{}={}", k, format_expr(v)))
                .collect();
            format!("[{}]", fields_str.join(", "))
        }
        MExpr::EachExpr(e) => format!("each {}", format_expr(e)),
        MExpr::Let { .. } => "let ... in ...".to_string(),
        MExpr::TypeAnnotation { expr, ty } => {
            format!(
                "{} as {}",
                format_expr(expr),
                crate::parser::grammar::type_to_string(ty)
            )
        }
    }
}
