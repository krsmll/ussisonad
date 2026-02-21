use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Word(String),
    String(String),  // string or "string"
    Integer(String), // 67
    Float(String),   // 3.14

    // Keywords
    Filter, // filter
    Sort,   // sort
    Take,   // take
    Map,    // map
    Or,     // or
    And,    // and
    In,     // in
    Not,    // not
    True,   // true
    False,  // false

    Eq, // =
    Ne, // !=
    Gt, // >
    Lt, // <
    Ge, // >=
    Le, // <=

    LeftParen,  // (
    RightParen, // )
    LeftBrace,  // {
    RightBrace, // }
    Plus,       // +
    Minus,      // -
    Asterisk,   // *
    Slash,      // /
    Percent,    // %
    Dot,        // .
    Comma,      // ,
    Semicolon,  // ;
    MinusMinus, // --
    GtGt,       // >>

    EOF,
}

impl Token {
    pub fn keyword_from_str(s: &str) -> Option<Token> {
        match s {
            "filter" => Some(Token::Filter),
            "sort" => Some(Token::Sort),
            "take" => Some(Token::Take),
            "map" => Some(Token::Map),
            "or" => Some(Token::Or),
            "and" => Some(Token::And),
            "in" => Some(Token::In),
            "not" => Some(Token::Not),
            "true" => Some(Token::True),
            "false" => Some(Token::False),
            _ => None,
        }
    }

    // pub fn is_keyword(&self) -> bool {
    //     matches!(
    //         self,
    //         Token::Filter
    //             | Token::Sort
    //             | Token::Take
    //             | Token::Map
    //             | Token::Or
    //             | Token::And
    //             | Token::In
    //             | Token::Not
    //             | Token::True
    //             | Token::False
    //     )
    // }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Token::Word(s) | Token::String(s) | Token::Integer(s) | Token::Float(s) => s.as_str(),
            Token::Filter => "filter",
            Token::Sort => "sort",
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
            Token::Not => "not",
            Token::True => "true",
            Token::False => "false",
            Token::LeftParen => "(",
            Token::RightParen => ")",
            Token::LeftBrace => "{",
            Token::RightBrace => "}",
            Token::Plus => "+",
            Token::Minus => "-",
            Token::Asterisk => "*",
            Token::Slash => "/",
            Token::Percent => "%",
            Token::Dot => ".",
            Token::Comma => ",",
            Token::Semicolon => ";",
            Token::MinusMinus => "--",
            Token::GtGt => ">>",
            Token::EOF => "EOF",
        };
        write!(f, "{}", s)
    }
}
