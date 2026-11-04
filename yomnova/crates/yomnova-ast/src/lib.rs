//! Abstract Syntax Tree (AST) definitions for YOMNOVA.

use yomnova_diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub module_name: Option<String>,
    pub items: Vec<TopLevelItem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TopLevelItem {
    Function(FunctionDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Const(ConstDecl),
    Statement(Stmt),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeAnnotation>,
    pub effects: Vec<String>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub is_mut: bool,
    pub name: String,
    pub ty: TypeAnnotation,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<StructField>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: String,
    pub ty: TypeAnnotation,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub payload: Option<TypeAnnotation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    pub name: String,
    pub ty: Option<TypeAnnotation>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        name: String,
        ty: Option<TypeAnnotation>,
        init: Expr,
        span: Span,
    },
    Mut {
        name: String,
        ty: Option<TypeAnnotation>,
        init: Expr,
        span: Span,
    },
    Assign {
        target: String,
        value: Expr,
        span: Span,
    },
    If {
        condition: Expr,
        then_branch: Block,
        else_branch: Option<Block>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Block,
        span: Span,
    },
    /// `for name in iterable { body }`: iterates the elements of an array.
    For {
        var: String,
        iterable: Expr,
        body: Block,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Flow {
        name: String,
        source: Expr,
        stages: Vec<PipeStage>,
        span: Span,
    },
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PipeStage {
    pub op_name: String,
    pub arg: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Integer(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    Unit,
    Identifier(String),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    Array(Vec<Expr>),
    Index {
        collection: Box<Expr>,
        index: Box<Expr>,
    },
    FieldAccess {
        object: Box<Expr>,
        field: String,
    },
    /// `Name { field: expr, … }`
    StructLiteral {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    Pipe {
        expr: Box<Expr>,
        stages: Vec<PipeStage>,
    },
    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeAnnotation {
    Int,
    Float,
    Bool,
    Text,
    Unit,
    IPv4,
    IPv6,
    Port,
    Endpoint,
    Packet,
    TCP,
    UDP,
    Array(Box<TypeAnnotation>),
    Custom(String),
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. } => *span,
            Stmt::Mut { span, .. } => *span,
            Stmt::Assign { span, .. } => *span,
            Stmt::If { span, .. } => *span,
            Stmt::While { span, .. } => *span,
            Stmt::For { span, .. } => *span,
            Stmt::Return { span, .. } => *span,
            Stmt::Flow { span, .. } => *span,
            Stmt::Expr(e) => e.span,
        }
    }
}
