#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Ident(String),
    Str(String),
    Int(i64),
    Float(f64),
    It,
    Bool(bool),
    Vector(Vec<Expr>, (usize, usize)),
    Subcommand(Box<FunctionPipeline>),
    Unary {
        op: UnaryOp,
        rhs: Box<Expr>,
    },
    Binary {
        lhs: Box<Expr>,
        op: BinOp,
        rhs: Box<Expr>,
    },
}
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    // Dot,
}

impl UnaryOp {
    pub fn bp(&self) -> (u8, u8) {
        match self {
            // UnaryOp::Dot => (0, 91),
            UnaryOp::Not => (0, 80),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BinOp {
    Get,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    In,
    And,
    Or,
    Concat,
    Pipe,
}

impl BinOp {
    pub fn bp(&self) -> (u8, u8) {
        match self {
            BinOp::Get => (90, 91),
            BinOp::Mul | BinOp::Div | BinOp::Mod => (70, 71),
            BinOp::Add | BinOp::Sub => (60, 61),
            BinOp::In | BinOp::Eq | BinOp::Ne | BinOp::Gt | BinOp::Lt | BinOp::Ge | BinOp::Le => {
                (50, 51)
            }
            BinOp::And => (40, 41),
            BinOp::Or => (30, 31),
            BinOp::Concat => (20, 21),
            BinOp::Pipe => (10, 11),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionPipeline {
    pub head: Function,
    pub stages: Vec<Stage>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub args: Vec<Arg>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Positional(Expr),
    Flag { name: String, value: Option<Expr> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stage {
    Filter(Expr),
    Sort { key: Expr, opts: Vec<Arg> },
    Take(Expr),
    Map(Expr),
    Count,
}
