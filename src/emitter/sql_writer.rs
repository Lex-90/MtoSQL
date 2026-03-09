/// Pretty-prints SQL AST to string.
use crate::dialect::Dialect;

/// A complete SQL query with optional CTEs.
#[derive(Debug, Clone)]
pub struct SqlQuery {
    /// Common Table Expressions.
    pub ctes: Vec<Cte>,
    /// The final SELECT statement.
    pub final_select: SelectStmt,
}

/// A Common Table Expression.
#[derive(Debug, Clone)]
pub struct Cte {
    /// CTE name.
    pub name: String,
    /// The SELECT statement.
    pub select: SelectStmt,
}

/// A SELECT statement.
#[derive(Debug, Clone)]
pub struct SelectStmt {
    /// Columns to select.
    pub columns: Vec<SelectCol>,
    /// FROM clause.
    pub from: Option<TableRef>,
    /// JOIN clauses.
    pub joins: Vec<JoinClause>,
    /// WHERE clause.
    pub where_clause: Option<SqlExpr>,
    /// GROUP BY columns.
    pub group_by: Vec<SqlExpr>,
    /// HAVING clause.
    pub having: Option<SqlExpr>,
    /// ORDER BY items.
    pub order_by: Vec<OrderByItem>,
    /// LIMIT value.
    pub limit: Option<u64>,
    /// UNION ALL members (additional SELECT statements to combine).
    pub union_all: Vec<SelectStmt>,
}

impl SelectStmt {
    /// Create a new empty SELECT statement.
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            from: None,
            joins: Vec::new(),
            where_clause: None,
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
            union_all: Vec::new(),
        }
    }
}

/// A table reference in a FROM clause.
#[derive(Debug, Clone)]
pub struct TableRef {
    /// Schema/database name.
    pub schema: Option<String>,
    /// Table name.
    pub name: String,
    /// Optional alias.
    pub alias: Option<String>,
}

/// A JOIN clause.
#[derive(Debug, Clone)]
pub struct JoinClause {
    /// Join type.
    pub join_type: JoinType,
    /// The table being joined.
    pub table: TableRef,
    /// Join condition.
    pub on: SqlExpr,
}

/// SQL JOIN types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    /// INNER JOIN.
    Inner,
    /// LEFT JOIN.
    Left,
    /// RIGHT JOIN.
    Right,
    /// FULL OUTER JOIN.
    FullOuter,
    /// LEFT JOIN (for anti-join pattern).
    LeftAnti,
    /// RIGHT JOIN (for anti-join pattern).
    RightAnti,
}

/// A column in a SELECT clause.
#[derive(Debug, Clone)]
pub enum SelectCol {
    /// `*`
    Wildcard,
    /// `table.*`
    QualifiedWildcard(String),
    /// An expression with optional alias.
    Expr {
        /// The expression.
        expr: SqlExpr,
        /// Optional alias.
        alias: Option<String>,
    },
    /// `* EXCEPT (col1, col2, ...)` — used by BigQuery and DuckDB.
    Except {
        /// Columns to exclude.
        columns: Vec<String>,
    },
}

/// A SQL expression.
#[derive(Debug, Clone)]
pub enum SqlExpr {
    /// A column reference.
    Column {
        /// Optional table qualifier.
        table: Option<String>,
        /// Column name.
        name: String,
    },
    /// A literal value.
    Literal(SqlLiteral),
    /// A binary operation.
    BinaryOp {
        /// The operator.
        op: SqlBinOp,
        /// Left operand.
        left: Box<SqlExpr>,
        /// Right operand.
        right: Box<SqlExpr>,
    },
    /// A unary operation.
    UnaryOp {
        /// The operator.
        op: SqlUnOp,
        /// The operand.
        expr: Box<SqlExpr>,
    },
    /// A function call.
    FunctionCall {
        /// Function name.
        name: String,
        /// Arguments.
        args: Vec<SqlExpr>,
        /// Whether DISTINCT is used.
        distinct: bool,
    },
    /// A CAST expression.
    Cast {
        /// The expression to cast.
        expr: Box<SqlExpr>,
        /// Target type.
        ty: String,
    },
    /// A LIKE expression.
    Like {
        /// The expression to match.
        expr: Box<SqlExpr>,
        /// The pattern.
        pattern: String,
        /// Whether this is NOT LIKE.
        negated: bool,
        /// Whether to use ILIKE.
        case_insensitive: bool,
    },
    /// An IN list expression.
    InList {
        /// The expression to check.
        expr: Box<SqlExpr>,
        /// The list of values.
        list: Vec<SqlExpr>,
        /// Whether this is NOT IN.
        negated: bool,
    },
    /// IS NULL / IS NOT NULL.
    IsNull {
        /// The expression.
        expr: Box<SqlExpr>,
        /// Whether this is IS NOT NULL.
        negated: bool,
    },
    /// A TRY_CAST / SAFE_CAST expression (error-safe cast).
    TryCast {
        /// The expression to cast.
        expr: Box<SqlExpr>,
        /// Target type.
        ty: String,
    },
    /// A CASE WHEN expression.
    CaseWhen {
        /// Condition.
        condition: Box<SqlExpr>,
        /// THEN expression.
        then_expr: Box<SqlExpr>,
        /// ELSE expression.
        else_expr: Box<SqlExpr>,
    },
    /// A COALESCE expression.
    Coalesce {
        /// Arguments.
        args: Vec<SqlExpr>,
    },
    /// Raw SQL string (escape hatch).
    Raw(String),
    /// Parenthesized expression.
    Parens(Box<SqlExpr>),
}

/// SQL literal values.
#[derive(Debug, Clone)]
pub enum SqlLiteral {
    /// String literal.
    String(String),
    /// Integer literal.
    Integer(i64),
    /// Float literal.
    Float(f64),
    /// Boolean literal.
    Boolean(bool),
    /// NULL.
    Null,
}

/// SQL binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlBinOp {
    /// `=`
    Eq,
    /// `<>`
    Neq,
    /// `<`
    Lt,
    /// `<=`
    Lte,
    /// `>`
    Gt,
    /// `>=`
    Gte,
    /// `AND`
    And,
    /// `OR`
    Or,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `||` (concatenation)
    Concat,
}

/// SQL unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlUnOp {
    /// `NOT`
    Not,
    /// `-`
    Neg,
}

/// ORDER BY item.
#[derive(Debug, Clone)]
pub struct OrderByItem {
    /// The expression.
    pub expr: SqlExpr,
    /// Ascending or descending.
    pub asc: bool,
}

/// Write a SQL query to a formatted string.
pub fn write_sql(query: &SqlQuery, dialect: &dyn Dialect) -> String {
    let mut out = String::new();

    if !query.ctes.is_empty() {
        out.push_str("WITH ");
        for (i, cte) in query.ctes.iter().enumerate() {
            if i > 0 {
                out.push_str(",\n");
            }
            out.push_str(&cte.name);
            out.push_str(" AS (\n");
            let select_str = write_select(&cte.select, dialect);
            for line in select_str.lines() {
                out.push_str("    ");
                out.push_str(line);
                out.push('\n');
            }
            out.push(')');
        }
        out.push('\n');
    }

    let final_str = write_select(&query.final_select, dialect);
    out.push_str(&final_str);
    out.push(';');

    out
}

/// Write a SELECT statement to string.
fn write_select(stmt: &SelectStmt, dialect: &dyn Dialect) -> String {
    let mut out = String::new();
    out.push_str("SELECT ");

    // Columns
    if stmt.columns.is_empty() {
        out.push('*');
    } else {
        let cols: Vec<String> = stmt
            .columns
            .iter()
            .map(|c| write_select_col(c, dialect))
            .collect();
        if cols.len() <= 3 {
            out.push_str(&cols.join(", "));
        } else {
            out.push('\n');
            for (i, col) in cols.iter().enumerate() {
                out.push_str("    ");
                out.push_str(col);
                if i < cols.len() - 1 {
                    out.push(',');
                }
                out.push('\n');
            }
        }
    }

    // FROM
    if let Some(ref from) = stmt.from {
        out.push_str("\nFROM ");
        out.push_str(&write_table_ref(from));
    }

    // JOINs
    for join in &stmt.joins {
        out.push('\n');
        let join_str = match join.join_type {
            JoinType::Inner => "INNER JOIN",
            JoinType::Left | JoinType::LeftAnti => "LEFT JOIN",
            JoinType::Right | JoinType::RightAnti => "RIGHT JOIN",
            JoinType::FullOuter => "FULL OUTER JOIN",
        };
        out.push_str(join_str);
        out.push(' ');
        out.push_str(&write_table_ref(&join.table));
        out.push_str(" ON ");
        out.push_str(&write_expr(&join.on, dialect));
    }

    // WHERE
    if let Some(ref where_clause) = stmt.where_clause {
        out.push_str("\nWHERE ");
        out.push_str(&write_expr(where_clause, dialect));
    }

    // GROUP BY
    if !stmt.group_by.is_empty() {
        out.push_str("\nGROUP BY ");
        let groups: Vec<String> = stmt
            .group_by
            .iter()
            .map(|e| write_expr(e, dialect))
            .collect();
        out.push_str(&groups.join(", "));
    }

    // HAVING
    if let Some(ref having) = stmt.having {
        out.push_str("\nHAVING ");
        out.push_str(&write_expr(having, dialect));
    }

    // ORDER BY
    if !stmt.order_by.is_empty() {
        out.push_str("\nORDER BY ");
        let orders: Vec<String> = stmt
            .order_by
            .iter()
            .map(|o| {
                let dir = if o.asc { "ASC" } else { "DESC" };
                format!("{} {}", write_expr(&o.expr, dialect), dir)
            })
            .collect();
        out.push_str(&orders.join(", "));
    }

    // LIMIT
    if let Some(limit) = stmt.limit {
        out.push_str(&format!("\nLIMIT {}", limit));
    }

    // UNION ALL
    for union_stmt in &stmt.union_all {
        out.push_str("\nUNION ALL\n");
        out.push_str(&write_select(union_stmt, dialect));
    }

    out
}

/// Write a column in a SELECT clause.
fn write_select_col(col: &SelectCol, dialect: &dyn Dialect) -> String {
    match col {
        SelectCol::Wildcard => "*".to_string(),
        SelectCol::QualifiedWildcard(table) => format!("{}.*", table),
        SelectCol::Expr { expr, alias } => {
            let expr_str = write_expr(expr, dialect);
            if let Some(alias) = alias {
                format!("{} AS {}", expr_str, alias)
            } else {
                expr_str
            }
        }
        SelectCol::Except { columns } => {
            format!("* EXCEPT ({})", columns.join(", "))
        }
    }
}

/// Write a table reference.
fn write_table_ref(table_ref: &TableRef) -> String {
    let mut s = String::new();
    if let Some(ref schema) = table_ref.schema {
        s.push_str(schema);
        s.push('.');
    }
    s.push_str(&table_ref.name);
    if let Some(ref alias) = table_ref.alias {
        s.push_str(" AS ");
        s.push_str(alias);
    }
    s
}

/// Write a SQL expression.
pub fn write_expr(expr: &SqlExpr, dialect: &dyn Dialect) -> String {
    match expr {
        SqlExpr::Column { table, name } => {
            if let Some(table) = table {
                format!("{}.{}", table, name)
            } else {
                name.clone()
            }
        }
        SqlExpr::Literal(lit) => match lit {
            SqlLiteral::String(s) => format!("'{}'", s.replace('\'', "''")),
            SqlLiteral::Integer(n) => n.to_string(),
            SqlLiteral::Float(n) => {
                let s = n.to_string();
                if s.contains('.') {
                    s
                } else {
                    format!("{}.0", s)
                }
            }
            SqlLiteral::Boolean(b) => dialect.boolean_literal(*b).to_string(),
            SqlLiteral::Null => "NULL".to_string(),
        },
        SqlExpr::BinaryOp { op, left, right } => {
            let op_str = match op {
                SqlBinOp::Eq => "=",
                SqlBinOp::Neq => "<>",
                SqlBinOp::Lt => "<",
                SqlBinOp::Lte => "<=",
                SqlBinOp::Gt => ">",
                SqlBinOp::Gte => ">=",
                SqlBinOp::And => "AND",
                SqlBinOp::Or => "OR",
                SqlBinOp::Add => "+",
                SqlBinOp::Sub => "-",
                SqlBinOp::Mul => "*",
                SqlBinOp::Div => "/",
                SqlBinOp::Concat => "||",
            };
            format!(
                "{} {} {}",
                write_expr(left, dialect),
                op_str,
                write_expr(right, dialect)
            )
        }
        SqlExpr::UnaryOp { op, expr } => {
            let op_str = match op {
                SqlUnOp::Not => "NOT ",
                SqlUnOp::Neg => "-",
            };
            format!("{}{}", op_str, write_expr(expr, dialect))
        }
        SqlExpr::FunctionCall {
            name,
            args,
            distinct,
        } => {
            let args_str: Vec<String> = args.iter().map(|a| write_expr(a, dialect)).collect();
            if *distinct {
                format!("{}(DISTINCT {})", name, args_str.join(", "))
            } else {
                format!("{}({})", name, args_str.join(", "))
            }
        }
        SqlExpr::Cast { expr, ty } => dialect.cast_syntax(&write_expr(expr, dialect), ty),
        SqlExpr::Like {
            expr,
            pattern,
            negated,
            case_insensitive,
        } => {
            let like_kw = if *case_insensitive && dialect.ilike_supported() {
                if *negated {
                    "NOT ILIKE"
                } else {
                    "ILIKE"
                }
            } else if *negated {
                "NOT LIKE"
            } else {
                "LIKE"
            };
            format!("{} {} '{}'", write_expr(expr, dialect), like_kw, pattern)
        }
        SqlExpr::InList {
            expr,
            list,
            negated,
        } => {
            let items: Vec<String> = list.iter().map(|i| write_expr(i, dialect)).collect();
            let in_kw = if *negated { "NOT IN" } else { "IN" };
            format!(
                "{} {} ({})",
                write_expr(expr, dialect),
                in_kw,
                items.join(", ")
            )
        }
        SqlExpr::IsNull { expr, negated } => {
            if *negated {
                format!("{} IS NOT NULL", write_expr(expr, dialect))
            } else {
                format!("{} IS NULL", write_expr(expr, dialect))
            }
        }
        SqlExpr::TryCast { expr, ty } => {
            if let Some(try_cast) = dialect.try_cast_syntax(&write_expr(expr, dialect), ty) {
                try_cast
            } else {
                // Fallback to regular CAST if dialect doesn't support TRY_CAST
                dialect.cast_syntax(&write_expr(expr, dialect), ty)
            }
        }
        SqlExpr::CaseWhen {
            condition,
            then_expr,
            else_expr,
        } => {
            format!(
                "CASE WHEN {} THEN {} ELSE {} END",
                write_expr(condition, dialect),
                write_expr(then_expr, dialect),
                write_expr(else_expr, dialect)
            )
        }
        SqlExpr::Coalesce { args } => {
            let args_str: Vec<String> = args.iter().map(|a| write_expr(a, dialect)).collect();
            format!("COALESCE({})", args_str.join(", "))
        }
        SqlExpr::Raw(s) => s.clone(),
        SqlExpr::Parens(inner) => {
            format!("({})", write_expr(inner, dialect))
        }
    }
}
