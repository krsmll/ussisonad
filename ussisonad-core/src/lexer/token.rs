use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Eof,

    // Literals
    Ident(String),
    Str(String),
    Int(String),
    Float(String),
    Bool(bool),

    // Keywords
    Filter,
    Sort,
    Count,
    Take,
    Unique,
    It,

    // Logical operators
    Or,
    And,
    In,
    Contains,
    Not,

    // Comparison operators
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,

    // Math operators
    Add,
    Sub,
    Mul,
    Div,
    DivDiv,
    Mod,

    // Vectors
    LeftParen,
    RightParen,

    Dot,
    Comma,
    Semicolon,
    SubSub,

    // Pipeline operators
    GtGt,
    AddAdd,
}

impl Token {
    #[must_use]
    pub fn from_word(s: &str) -> Option<Token> {
        match s {
            "filter" | "where" => Some(Token::Filter),
            "sort" | "order" => Some(Token::Sort),
            "self" | "it" => Some(Token::It),
            "count" => Some(Token::Count),
            "take" => Some(Token::Take),
            "unique" => Some(Token::Unique),
            "with" => Some(Token::AddAdd),
            "or" => Some(Token::Or),
            "and" => Some(Token::And),
            "not" => Some(Token::Not),
            "atleast" | "ge" => Some(Token::Ge),
            "atmost" | "le" => Some(Token::Le),
            "above" | "gt" => Some(Token::Gt),
            "below" | "lt" => Some(Token::Lt),
            "contains" => Some(Token::Contains),
            "is" | "eq" => Some(Token::Eq),
            "in" => Some(Token::In),
            "true" => Some(Token::Bool(true)),
            "false" => Some(Token::Bool(false)),
            _ => None,
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Str(s) => write!(f, r#""{s}""#),
            Token::Ident(s) | Token::Int(s) | Token::Float(s) => write!(f, "{s}"),
            Token::Bool(b) => write!(f, "{}", b.clone()),
            Token::Contains => write!(f, "contains"),
            Token::Filter => write!(f, "filter"),
            Token::Sort => write!(f, "sort"),
            Token::Count => write!(f, "count"),
            Token::Take => write!(f, "take"),
            Token::Unique => write!(f, "unique"),
            Token::It => write!(f, "it"),
            Token::Eq => write!(f, "="),
            Token::Ne => write!(f, "!="),
            Token::Gt => write!(f, ">"),
            Token::Lt => write!(f, "<"),
            Token::Ge => write!(f, ">="),
            Token::Le => write!(f, "<="),
            Token::Or => write!(f, "or"),
            Token::And => write!(f, "and"),
            Token::In => write!(f, "in"),
            Token::Not => write!(f, "not"),
            Token::LeftParen => write!(f, "("),
            Token::RightParen => write!(f, ")"),
            Token::Add => write!(f, "+"),
            Token::Sub => write!(f, "-"),
            Token::Mul => write!(f, "*"),
            Token::Div => write!(f, "/"),
            Token::DivDiv => write!(f, "//"),
            Token::Mod => write!(f, "%"),
            Token::Dot => write!(f, "."),
            Token::Comma => write!(f, ","),
            Token::Semicolon => write!(f, ";"),
            Token::AddAdd => write!(f, "++"),
            Token::SubSub => write!(f, "--"),
            Token::GtGt => write!(f, ">>"),
            Token::Eof => write!(f, "EOF"),
        }
    }
}
