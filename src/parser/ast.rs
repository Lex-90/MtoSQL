//! AST node types for M expressions.

/// Top-level M document — either a single expression or a section with bindings.
#[derive(Debug, Clone, PartialEq)]
pub enum MDocument {
    /// A single `let … in` expression (or any standalone expression).
    Expression(MExpr),
    /// A `section` with named bindings.
    Section {
        /// Section name.
        name: String,
        /// List of (is_shared, binding_name, expression).
        bindings: Vec<(bool, String, MExpr)>,
    },
}

/// An M expression node.
#[derive(Debug, Clone, PartialEq)]
pub enum MExpr {
    /// `let bindings in body`
    Let {
        /// Named bindings.
        bindings: Vec<(String, MExpr)>,
        /// The body expression.
        body: Box<MExpr>,
    },
    /// A function call like `Table.SelectRows(args...)`.
    FunctionCall {
        /// Function name (e.g., "Table.SelectRows").
        name: String,
        /// Function arguments.
        args: Vec<MExpr>,
    },
    /// Field access: `expr[field]`.
    FieldAccess {
        /// The expression being accessed.
        expr: Box<MExpr>,
        /// The field name.
        field: String,
    },
    /// Index access: `expr{key}`.
    IndexAccess {
        /// The expression being indexed.
        expr: Box<MExpr>,
        /// The key expression.
        key: Box<MExpr>,
    },
    /// `each <expr>` — introduces a row context.
    EachExpr(Box<MExpr>),
    /// `[ColumnName]` inside an `each` context.
    RowField(String),
    /// Binary operation.
    BinaryOp {
        /// The operator.
        op: BinOp,
        /// Left operand.
        left: Box<MExpr>,
        /// Right operand.
        right: Box<MExpr>,
    },
    /// Unary operation.
    UnaryOp {
        /// The operator.
        op: UnOp,
        /// The operand.
        expr: Box<MExpr>,
    },
    /// A literal value.
    Literal(MLiteral),
    /// A named identifier.
    Identifier(String),
    /// A list `{e1, e2, ...}`.
    List(Vec<MExpr>),
    /// A record `[k1=v1, k2=v2, ...]`.
    Record(Vec<(String, MExpr)>),
    /// A type annotation `expr as type`.
    TypeAnnotation {
        /// The annotated expression.
        expr: Box<MExpr>,
        /// The type.
        ty: MType,
    },
}

/// M literal values.
#[derive(Debug, Clone, PartialEq)]
pub enum MLiteral {
    /// Text string.
    Text(String),
    /// Floating-point number.
    Number(f64),
    /// Integer.
    Integer(i64),
    /// Boolean.
    Bool(bool),
    /// Null.
    Null,
}

/// M types for type annotations and casts.
#[derive(Debug, Clone, PartialEq)]
pub enum MType {
    /// `type text`
    Text,
    /// `type number`
    Number,
    /// `type integer` or `Int64.Type`
    Integer,
    /// `type logical`
    Logical,
    /// `type date`
    Date,
    /// `type datetime`
    DateTime,
    /// `type datetimezone`
    DateTimeZone,
    /// `type duration`
    Duration,
    /// `type binary`
    Binary,
    /// `type table`
    Table,
    /// `type any`
    Any,
    /// An unknown type string.
    Unknown(String),
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
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
    /// `and`
    And,
    /// `or`
    Or,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `&` (text concatenation)
    Concat,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `not`
    Not,
    /// `-` (negation)
    Neg,
}
