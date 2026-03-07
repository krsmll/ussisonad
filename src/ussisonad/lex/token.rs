use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    EOF,

    // Literals
    Str(String),
    Int(String),
    Float(String),
    Bool(bool),

    // Keywords
    Filter,
    Sort,
    Count,
    Take,
    Map,

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
    Mod,

    // Vectors
    LeftParen, 
    RightParen,

    // Independent scope
    LeftBrace, 
    RightBrace,
    
    Dot,
    Comma,
    Semicolon,
    SubSub,
    
    // Pipeline operators
    GtGt,
    AddAdd,
}

impl Token {
    pub fn str_to_keyword(s: &str) -> Option<Token> {
        match s {
            "filter" | "where" => Some(Token::Filter),
            "sort" | "order" => Some(Token::Sort),
            "count" => Some(Token::Count),
            "take" => Some(Token::Take),
            "map" => Some(Token::Map),
            "or" => Some(Token::Or),
            "and" => Some(Token::And),
            "not" => Some(Token::Not),
            "above" => Some(Token::Ge),
            "below" => Some(Token::Le),
            "contains" => Some(Token::Contains),
            "with" => Some(Token::AddAdd),
            "is" => Some(Token::Eq),
            "in" => Some(Token::In),
            "true" => Some(Token::Bool(true)),
            "false" => Some(Token::Bool(false)),
            _ => None,
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Token::Str(s) | Token::Int(s) | Token::Float(s) => s.as_str(),
            Token::Bool(b) => {
                if *b {
                    "true"
                } else {
                    "false"
                }
            }
            Token::Filter => "filter",
            Token::Sort => "sort",
            Token::Count => "count",
            Token::Take => "take",
            Token::Map => "map",
            Token::Eq => "=",
            Token::Ne => "!=",
            Token::Gt => ">",
            Token::Lt => "<",
            Token::Ge => ">=",
            Token::Le => "<=",
            Token::Or => "or",
            Token::And => "and",
            Token::In => "in",
            Token::Contains => "contains",
            Token::Not => "not",
            Token::LeftParen => "(",
            Token::RightParen => ")",
            Token::LeftBrace => "{",
            Token::RightBrace => "}",
            Token::Add => "+",
            Token::Sub => "-",
            Token::Mul => "*",
            Token::Div => "/",
            Token::Mod => "%",
            Token::Dot => ".",
            Token::Comma => ",",
            Token::Semicolon => ";",
            Token::AddAdd => "++",
            Token::SubSub => "--",
            Token::GtGt => ">>",
            Token::EOF => "EOF",
        };
        write!(f, "{}", s)
    }
}
