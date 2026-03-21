use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    It,
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Vector(Vec<Expr>),

    FieldPath(Vec<String>),

    Binary {
        lhs: Box<Expr>,
        op: BinOp,
        rhs: Box<Expr>,
    },
    Not(Box<Expr>),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BinOp {
    // arithmetic
    Add,
    Sub,
    Mul,
    Div,
    DivDiv,
    Mod,

    // comparison
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,

    // logical
    And,
    Or,

    // membership
    In,
    Contains,
}

impl BinOp {
    pub fn bp(&self) -> (u8, u8) {
        match self {
            BinOp::Mul | BinOp::Div | BinOp::DivDiv | BinOp::Mod => (70, 71),
            BinOp::Add | BinOp::Sub => (60, 61),
            BinOp::In
            | BinOp::Contains
            | BinOp::Eq
            | BinOp::Ne
            | BinOp::Gt
            | BinOp::Lt
            | BinOp::Ge
            | BinOp::Le => (50, 51),
            BinOp::And => (40, 41),
            BinOp::Or => (30, 31),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineNode {
    Pipe {
        lhs: Box<PipelineNode>,
        rhs: Box<PipelineNode>,
    },
    Concat {
        lhs: Box<PipelineNode>,
        rhs: Box<PipelineNode>,
    },
    Command(Command),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Custom(CustomCommand),
    Builtin(BuiltinCommand),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomCommand {
    pub name: String,
    pub arg: Option<Expr>,
    pub flags: HashSet<String>,
    pub options: HashMap<String, Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BuiltinCommand {
    Filter(Expr),
    Sort {
        field: Expr,
        direction: SortDirection,
    },
    Count,
    Limit(u64),
    Map(Expr),
    Unique(Option<Expr>),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SortDirection {
    Asc,
    Desc,
}
