/// Infers table/schema names from M data-source expressions.
use crate::parser::ast::*;
use indexmap::IndexMap;
use std::collections::HashSet;

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

/// M keywords that should never be classified as parameters.
const M_KEYWORDS: &[&str] = &[
    "each", "in", "let", "if", "then", "else", "try", "otherwise",
    "true", "false", "null", "and", "or", "not", "as", "is", "error",
    "section", "shared", "type",
];

/// Built-in M function and type name prefixes that should not be classified as parameters.
const M_BUILTIN_PREFIXES: &[&str] = &[
    "Sql.", "Table.", "List.", "Text.", "Date.", "DateTime.", "DateTimeZone.",
    "Duration.", "Binary.", "Number.", "Int64.", "Int32.", "Int16.", "Int8.",
    "Value.", "Record.", "Type.", "Excel.", "Csv.", "Json.", "Web.", "OData.",
    "Odbc.", "Oracle.", "Teradata.", "Sybase.", "MySQL.", "PostgreSQL.",
    "DB2.", "Informix.", "Access.", "File.", "Folder.", "SharePoint.",
    "AzureStorage.", "ActiveDirectory.", "AnalysisServices.", "Cube.",
    "Facebook.", "GoogleAnalytics.", "Hdfs.", "HdInsight.", "Salesforce.",
    "JoinKind.", "Comparer.", "Order.", "MissingField.", "Precision.",
    "RoundingMode.", "GroupKind.", "ExtraValues.", "SortDirection.",
    "Culture.", "Occurrence.", "TextEncoding.", "LineStyle.", "QuoteStyle.",
    "CsvStyle.", "RelativePosition.", "TraceLevel.",
];

/// Built-in M identifiers that are standalone (not dot-prefixed).
const M_BUILTIN_IDENTIFIERS: &[&str] = &[
    "JoinKind.Inner", "JoinKind.Left", "JoinKind.Right", "JoinKind.Full",
    "JoinKind.LeftAnti", "JoinKind.RightAnti",
    "Comparer.OrdinalIgnoreCase", "Comparer.Ordinal", "Comparer.FromCulture",
    "Int64.Type", "Int32.Type", "Int16.Type", "Int8.Type",
    "Percentage.Type", "Currency.Type",
    "Order.Ascending", "Order.Descending",
    "MissingField.Error", "MissingField.Ignore", "MissingField.UseNull",
    "GroupKind.Global", "GroupKind.Local",
    "ExtraValues.Error", "ExtraValues.Ignore", "ExtraValues.List",
    "_",
];

/// Check if an identifier is a known M keyword or built-in.
fn is_m_builtin(name: &str) -> bool {
    if M_KEYWORDS.contains(&name) {
        return true;
    }
    if M_BUILTIN_IDENTIFIERS.contains(&name) {
        return true;
    }
    for prefix in M_BUILTIN_PREFIXES {
        if name.starts_with(prefix) {
            return true;
        }
    }
    // Type keywords and "type X" identifiers (e.g. "type integer", "type text")
    if name.starts_with("type ") {
        return true;
    }
    matches!(name,
        "type" | "text" | "number" | "integer" | "logical" | "date" | "datetime"
        | "datetimezone" | "duration" | "binary" | "table" | "any" | "record"
        | "list" | "function" | "action" | "none"
    )
}

/// Detected Power Query parameter information.
#[derive(Debug, Clone)]
pub struct DetectedParameter {
    /// Parameter name.
    pub name: String,
    /// File name where first detected.
    pub file: String,
    /// 1-indexed line of first use.
    pub line: usize,
}

/// Detect Power Query parameters in an M expression.
/// Returns a map of parameter name -> first use info, sorted lexicographically.
pub fn detect_parameters(
    expr: &MExpr,
    let_bindings: &HashSet<String>,
    input_file_stems: &HashSet<String>,
    file_name: &str,
) -> IndexMap<String, DetectedParameter> {
    let mut params: IndexMap<String, DetectedParameter> = IndexMap::new();
    collect_parameters(expr, let_bindings, input_file_stems, file_name, &mut params, 1);

    // Sort lexicographically by parameter name
    let mut sorted: Vec<(String, DetectedParameter)> = params.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    sorted.into_iter().collect()
}

/// Recursively collect parameter identifiers from an M expression.
fn collect_parameters(
    expr: &MExpr,
    let_bindings: &HashSet<String>,
    input_file_stems: &HashSet<String>,
    file_name: &str,
    params: &mut IndexMap<String, DetectedParameter>,
    line: usize,
) {
    match expr {
        MExpr::Identifier(name) => {
            if !let_bindings.contains(name)
                && !is_m_builtin(name)
                && !input_file_stems.contains(name)
            {
                params.entry(name.clone()).or_insert_with(|| DetectedParameter {
                    name: name.clone(),
                    file: file_name.to_string(),
                    line,
                });
            }
        }
        MExpr::FunctionCall { name, args } => {
            // Skip identifiers inside Table.Combine list arguments — those are table
            // references handled by the combine validator, not parameters.
            if name == "Table.Combine" {
                return;
            }
            for arg in args {
                collect_parameters(arg, let_bindings, input_file_stems, file_name, params, line);
            }
        }
        MExpr::BinaryOp { left, right, .. } => {
            collect_parameters(left, let_bindings, input_file_stems, file_name, params, line);
            collect_parameters(right, let_bindings, input_file_stems, file_name, params, line);
        }
        MExpr::UnaryOp { expr: inner, .. } => {
            collect_parameters(inner, let_bindings, input_file_stems, file_name, params, line);
        }
        MExpr::Let { bindings, body } => {
            // Add nested let bindings to scope
            let mut inner_bindings = let_bindings.clone();
            for (name, _) in bindings {
                inner_bindings.insert(name.clone());
            }
            for (_, e) in bindings {
                collect_parameters(e, &inner_bindings, input_file_stems, file_name, params, line);
            }
            collect_parameters(body, &inner_bindings, input_file_stems, file_name, params, line);
        }
        MExpr::EachExpr(inner) => {
            collect_parameters(inner, let_bindings, input_file_stems, file_name, params, line);
        }
        MExpr::FieldAccess { expr: inner, .. } => {
            collect_parameters(inner, let_bindings, input_file_stems, file_name, params, line);
        }
        MExpr::IndexAccess { expr: inner, key } => {
            collect_parameters(inner, let_bindings, input_file_stems, file_name, params, line);
            collect_parameters(key, let_bindings, input_file_stems, file_name, params, line);
        }
        MExpr::List(items) => {
            for item in items {
                collect_parameters(item, let_bindings, input_file_stems, file_name, params, line);
            }
        }
        MExpr::Record(fields) => {
            for (_, v) in fields {
                collect_parameters(v, let_bindings, input_file_stems, file_name, params, line);
            }
        }
        MExpr::TypeAnnotation { expr: inner, .. } => {
            collect_parameters(inner, let_bindings, input_file_stems, file_name, params, line);
        }
        _ => {}
    }
}

/// Generate the parameter header block for a SQL output file.
pub fn generate_param_header(params: &IndexMap<String, DetectedParameter>) -> String {
    if params.is_empty() {
        return String::new();
    }

    let mut header = String::new();
    header.push_str("-- =====================================================================\n");
    header.push_str("-- POWER QUERY PARAMETERS DETECTED\n");
    header.push_str("-- The following identifiers are Power Query parameters. Replace each\n");
    header.push_str("-- /* PARAM: <name> */ placeholder with the actual value before running\n");
    header.push_str("-- this query. Parameter values are not available at translation time.\n");
    header.push_str("--\n");
    header.push_str("-- Parameters found in this file:\n");

    for (name, info) in params {
        header.push_str(&format!("--   {}  (used at: {}:{})\n", name, info.file, info.line));
    }

    header.push_str("-- =====================================================================\n");
    header
}
