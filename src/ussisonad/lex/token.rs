use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    EOF,

    /// Alphanumeric identifier that is either used in a function lookup or as a string value.
    ///
    /// For example, for command: ';top Slay'
    /// 'Slay' is not a known identifier and therefore, is treated as a string.
    /// 'top' is known to our interpreter, so it is treated as a function call with 'Slay' being its argument.
    Ident(String),

    Str(String),   // Alphanumeric sequence of characters between quotes
    Int(String),   // Integer number
    Float(String), // Floating point number

    // Keywords
    Filter, // 'filter'
    Sort,   // 'sort'
    Count,  // 'count'
    Take,   // 'take'
    Map,    // 'map'
    It,     // 'it'

    // Logical operators
    Or,    // 'or'
    And,   // 'and'
    In,    // 'in'
    Not,   // '!' OR 'not'
    True,  // 'true'
    False, // 'false'

    // Comparison operators
    Eq, // '='
    Ne, // '!='
    Gt, // '>'
    Lt, // '<'
    Ge, // '>='
    Le, // '<='

    // Math operators
    Add, // '+'
    Sub, // '-'
    Mul, // '*'
    Div, // '/'
    Mod, // '%'

    // Vectors
    LeftParen,  // '('
    RightParen, // ')'

    // Independent scope
    LeftBrace,  // '{'
    RightBrace, // '}'

    GtGt,      // '>>' pipes result of a previous step into another
    Dot,       // '.' field access
    Comma,     // ',' optional separator for vector elements
    Semicolon, // ';' command flag
    AddAdd,    // '++' used for appending/prepending to vector or concatenating vectors
    SubSub,    // '--' option flag
}

impl Token {
    pub fn keyword_from_str(s: &str) -> Option<Token> {
        match s {
            "filter" => Some(Token::Filter),
            "sort" => Some(Token::Sort),
            "count" => Some(Token::Count),
            "take" => Some(Token::Take),
            "map" => Some(Token::Map),
            "it" => Some(Token::It),
            "or" => Some(Token::Or),
            "and" => Some(Token::And),
            "in" => Some(Token::In),
            "not" => Some(Token::Not),
            "true" => Some(Token::True),
            "false" => Some(Token::False),
            _ => None,
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Token::Ident(s) | Token::Str(s) | Token::Int(s) | Token::Float(s) => s.as_str(),
            Token::Filter => "filter",
            Token::Sort => "sort",
            Token::Count => "count",
            Token::Take => "take",
            Token::Map => "map",
            Token::It => "it",
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
