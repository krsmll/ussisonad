use crate::ussisonad::lex::LexError;
use crate::ussisonad::lex::Token;
use crate::ussisonad::lex::{LexResult, Spanned};
use crate::ussisonad::parse::ast::{
    BinOp, BuiltinCommand, Command, CustomCommand, Expr, PipelineNode, SortDirection,
};
use crate::ussisonad::parse::error::ParserError;
use std::collections::{HashMap, HashSet};
use std::iter::Peekable;
use std::mem;

pub type ParserResult = Result<PipelineNode, ParserError>;
pub type ParsingResult = Result<Expr, ParserError>;

pub struct Parser<T: Iterator<Item = LexResult>> {
    tokens: Peekable<T>,
    lex_errors: Vec<LexError>,
}

impl<T: Iterator<Item = LexResult>> Parser<T> {
    pub fn parse(tokens: T) -> ParserResult {
        let mut parser = Self {
            tokens: tokens.peekable(),
            lex_errors: Vec::new(),
        };

        if let Err(e) = parser.expect(Token::Semicolon) {
            return parser.reduce_errors(e);
        }

        let node = match parser.parse_pipeline() {
            Ok(v) => v,
            Err(e) => parser.reduce_errors(e)?,
        };

        match parser.next() {
            Some(span) => parser.reduce_errors(ParserError::UnexpectedTrailingToken(span)),

            None => {
                if parser.lex_errors.is_empty() {
                    Ok(node)
                } else {
                    Err(ParserError::LexerStage(parser.lex_errors))
                }
            }
        }
    }

    fn reduce_errors(&mut self, err: ParserError) -> ParserResult {
        self.drain();
        let lex_errors = mem::take(&mut self.lex_errors);
        match lex_errors[..] {
            [] => Err(err),
            _ => Err(ParserError::LexerStage(lex_errors)),
        }
    }

    fn drain(&mut self) {
        // next() checks for errors produced by the lexer and pushes them to self.lex_errors
        while self.next().is_some() {}
    }

    fn parse_pipeline(&mut self) -> Result<PipelineNode, ParserError> {
        let left = self.parse_command()?;
        let mut node = PipelineNode::Command(left);

        while let Some(tok) = self.peek() {
            match tok {
                Token::GtGt => {
                    self.next();
                    let rhs = self.parse_command()?;
                    node = PipelineNode::Pipe {
                        lhs: Box::new(node),
                        rhs: Box::new(PipelineNode::Command(rhs)),
                    };
                }
                Token::AddAdd => {
                    self.next();
                    let rhs = self.parse_command()?;
                    node = PipelineNode::Concat {
                        lhs: Box::new(node),
                        rhs: Box::new(PipelineNode::Command(rhs)),
                    };
                }
                _ => break,
            }
        }

        Ok(node)
    }

    fn parse_command(&mut self) -> Result<Command, ParserError> {
        match self.peek().ok_or(ParserError::UnexpectedEOF)? {
            Token::Filter => self.parse_filter(),
            Token::Sort => self.parse_sort(),
            Token::Take => self.parse_take(),
            Token::Map => self.parse_map(),
            Token::Unique => self.parse_unique(),
            Token::Count => {
                self.next();
                Ok(Command::Builtin(BuiltinCommand::Count))
            }
            Token::Str(_) => self.parse_custom_command(),
            _ => Err(ParserError::UnexpectedToken(self.next().unwrap())),
        }
    }

    fn parse_unique(&mut self) -> Result<Command, ParserError> {
        self.next();
        let field = match self.peek() {
            Some(Token::Dot) => Some(self.parse_expr(0)?),
            _ => None,
        };
        Ok(Command::Builtin(BuiltinCommand::Unique(field)))
    }

    fn parse_custom_command(&mut self) -> Result<Command, ParserError> {
        let name = self.expect_str()?;
        let mut args = Vec::new();
        let mut flags = HashSet::new();
        let mut options = HashMap::new();

        loop {
            match self.peek() {
                Some(Token::GtGt | Token::AddAdd | Token::Eof) | None => break,
                Some(Token::SubSub) => {
                    self.next();
                    let key = self.expect_str()?;
                    match self.peek() {
                        Some(Token::Str(_) | Token::Int(_) | Token::Float(_) | Token::Bool(_)) => {
                            let val = self.parse_expr(0)?;
                            options.insert(key, val);
                        }
                        _ => {
                            flags.insert(key);
                        }
                    }
                }
                _ => {
                    args.push(self.parse_expr(0)?);
                }
            }
        }

        let command = CustomCommand {
            name,
            arg: match args.len() {
                0 => None,
                1 => Some(args.remove(0)),
                _ => Some(Expr::Vector(args)),
            },
            flags,
            options,
        };

        Ok(Command::Custom(command))
    }

    fn parse_filter(&mut self) -> Result<Command, ParserError> {
        self.next();
        let expr = self.parse_expr(0)?;
        Ok(Command::Builtin(BuiltinCommand::Filter(expr)))
    }

    fn parse_map(&mut self) -> Result<Command, ParserError> {
        self.next();
        let expr = self.parse_expr(0)?;
        Ok(Command::Builtin(BuiltinCommand::Map(expr)))
    }

    fn parse_sort(&mut self) -> Result<Command, ParserError> {
        self.next();

        let mut field = Expr::It;
        let mut direction = SortDirection::Desc;

        loop {
            match self.peek() {
                Some(Token::GtGt | Token::AddAdd | Token::Eof) | None => break,
                Some(Token::Sub | Token::SubSub) => {
                    self.next();
                    direction = match self.next() {
                        Some((Token::Str(s), _, _)) if s == "asc" => SortDirection::Asc,
                        Some((Token::Str(s), _, _)) if s == "desc" => SortDirection::Desc,
                        Some(actual) => return Err(ParserError::UnexpectedToken(actual)),
                        None => return Err(ParserError::UnexpectedEOF),
                    }
                }
                _ => field = self.parse_expr(0)?,
            }
        }

        Ok(Command::Builtin(BuiltinCommand::Sort { field, direction }))
    }

    fn parse_take(&mut self) -> Result<Command, ParserError> {
        self.next();
        let n = match self.next() {
            Some((Token::Int(s), start, end)) => Self::parse_uint(s, start, end)?,
            Some(t) => return Err(ParserError::UnexpectedToken(t)),
            None => return Err(ParserError::UnexpectedEOF),
        };

        Ok(Command::Builtin(BuiltinCommand::Limit(n)))
    }

    fn parse_expr(&mut self, min_bp: u8) -> ParsingResult {
        let (tok, start, end) = self.next().ok_or(ParserError::UnexpectedEOF)?;

        let mut lhs = match tok {
            Token::Str(s) => Expr::Str(s),
            Token::Bool(b) => Expr::Bool(b),
            Token::It => Expr::It,
            Token::Int(s) => Self::parse_int_expr((Token::Int(s), start, end))?,
            Token::Float(s) => Self::parse_float_expr((Token::Float(s), start, end))?,
            Token::Dot => self.parse_field_path()?,
            Token::LeftParen => self.parse_group(start)?,
            Token::Not => {
                let rhs = self.parse_expr(80)?;
                Expr::Not(Box::new(rhs))
            }

            tok => return Err(ParserError::UnexpectedToken((tok, start, end))),
        };

        while let Some(tok) = self.peek()
            && let Some(op) = Self::token_to_binop(tok)
        {
            let (left_bp, right_bp) = BinOp::bp(op);
            if left_bp < min_bp {
                break;
            }

            self.next();

            let rhs = self.parse_expr(right_bp)?;

            lhs = Expr::Binary {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn parse_field_path(&mut self) -> ParsingResult {
        let mut segments = Vec::new();
        segments.push(self.expect_str()?);

        while let Some(&Token::Dot) = self.peek() {
            self.next();
            segments.push(self.expect_str()?);
        }

        Ok(Expr::FieldPath(segments))
    }

    fn parse_group(&mut self, left_paren_pos: usize) -> ParsingResult {
        let mut exprs: Vec<Expr> = Vec::new();

        loop {
            let (tok, pos, _) = self.peek_span().ok_or(ParserError::UnexpectedEOF)?;
            match tok {
                Token::RightParen => {
                    let expr = match exprs[..] {
                        [] => return Err(ParserError::EmptyVector((left_paren_pos, *pos))),
                        [Expr::Binary { .. }] => exprs.pop().unwrap(),
                        _ => Expr::Vector(exprs),
                    };

                    self.next();
                    return Ok(expr);
                }
                Token::Comma => {
                    self.next();
                }
                _ => exprs.push(self.parse_expr(0)?),
            }
        }
    }

    fn parse_int_expr(spanned: Spanned) -> ParsingResult {
        let (tok, start, end) = spanned;
        match tok {
            Token::Int(s) => Ok(Expr::Int(Self::parse_int(s, start, end)?)),
            _ => Err(ParserError::InvalidInt((tok, start, end))),
        }
    }

    fn parse_int(s: String, start: usize, end: usize) -> Result<i64, ParserError> {
        s.parse::<i64>()
            .map_err(|_| ParserError::InvalidInt((Token::Int(s), start, end)))
    }

    fn parse_uint(s: String, start: usize, end: usize) -> Result<u64, ParserError> {
        s.parse::<u64>()
            .map_err(|_| ParserError::InvalidUnsignedInt((Token::Int(s), start, end)))
    }

    fn parse_float_expr(spanned: Spanned) -> ParsingResult {
        let (tok, start, end) = spanned;
        match tok {
            Token::Float(s) => s
                .parse::<f64>()
                .map(Expr::Float)
                .map_err(|_| ParserError::InvalidFloat((Token::Float(s), start, end))),
            _ => Err(ParserError::InvalidFloat((tok, start, end))),
        }
    }

    fn token_to_binop(token: &Token) -> Option<BinOp> {
        match token {
            Token::Add => Some(BinOp::Add),
            Token::Sub => Some(BinOp::Sub),
            Token::Mul => Some(BinOp::Mul),
            Token::Div => Some(BinOp::Div),
            Token::DivDiv => Some(BinOp::DivDiv),
            Token::Mod => Some(BinOp::Mod),
            Token::Eq => Some(BinOp::Eq),
            Token::Ne => Some(BinOp::Ne),
            Token::Gt => Some(BinOp::Gt),
            Token::Lt => Some(BinOp::Lt),
            Token::Ge => Some(BinOp::Ge),
            Token::Le => Some(BinOp::Le),
            Token::And => Some(BinOp::And),
            Token::Or => Some(BinOp::Or),
            Token::In => Some(BinOp::In),
            Token::Contains => Some(BinOp::Contains),
            _ => None,
        }
    }

    fn next(&mut self) -> Option<Spanned> {
        match self.tokens.next()? {
            Ok(tok) => Some(tok),
            Err(err) => {
                self.lex_errors.push(err);
                None
            }
        }
    }

    fn peek_span(&mut self) -> Option<&Spanned> {
        match self.tokens.peek() {
            Some(Ok(tok)) => Some(tok),
            _ => None,
        }
    }

    fn peek(&mut self) -> Option<&Token> {
        self.peek_span().map(|span| &span.0)
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParserError> {
        match self.next() {
            Some((tok, _, _)) if tok == expected => Ok(()),
            Some(actual) => Err(ParserError::UnexpectedTokenWithContext(expected, actual)),
            None => Err(ParserError::UnexpectedEOF),
        }
    }

    fn expect_str(&mut self) -> Result<String, ParserError> {
        match self.next() {
            Some((Token::Str(s), _, _)) => Ok(s),
            Some(actual) => Err(ParserError::ExpectedString(actual)),
            None => Err(ParserError::UnexpectedEOF),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ussisonad::lex::make_tokenizer;
    use crate::ussisonad::parse::ast::{
        BinOp, BuiltinCommand, Command, CustomCommand, Expr, PipelineNode, SortDirection,
    };
    use crate::ussisonad::parse::error::ParserError;
    use crate::ussisonad::parse::parser::Parser;
    use std::collections::{HashMap, HashSet};

    macro_rules! assert_ast {
        ($src:expr, $expected:expr) => {
            let toks = make_tokenizer($src);
            let result = Parser::parse(toks);
            assert_eq!(result, Ok($expected))
        };
    }

    fn cmd(c: Command) -> PipelineNode {
        PipelineNode::Command(c)
    }

    fn custom(name: &str) -> Command {
        Command::Custom(CustomCommand {
            name: name.to_string(),
            arg: None,
            flags: HashSet::new(),
            options: HashMap::new(),
        })
    }

    fn field(segments: &[&str]) -> Expr {
        Expr::FieldPath(segments.iter().map(|s| s.to_string()).collect())
    }

    fn binary(lhs: Expr, op: BinOp, rhs: Expr) -> Expr {
        Expr::Binary {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        }
    }

    #[test]
    fn test_count() {
        assert_ast!(";count", cmd(Command::Builtin(BuiltinCommand::Count)));
    }

    #[test]
    fn test_take() {
        assert_ast!(";take 10", cmd(Command::Builtin(BuiltinCommand::Limit(10))));
    }

    #[test]
    fn test_sort_default_desc() {
        assert_ast!(
            ";sort .bpm",
            cmd(Command::Builtin(BuiltinCommand::Sort {
                field: field(&["bpm"]),
                direction: SortDirection::Desc,
            }))
        );
    }

    #[test]
    fn test_sort_asc() {
        assert_ast!(
            ";sort .bpm --asc",
            cmd(Command::Builtin(BuiltinCommand::Sort {
                field: field(&["bpm"]),
                direction: SortDirection::Asc,
            }))
        );
    }

    #[test]
    fn test_sort_desc_explicit() {
        assert_ast!(
            ";sort .bpm --desc",
            cmd(Command::Builtin(BuiltinCommand::Sort {
                field: field(&["bpm"]),
                direction: SortDirection::Desc,
            }))
        );
    }

    #[test]
    fn test_order_keyword_alias() {
        assert_ast!(
            ";order .name --asc",
            cmd(Command::Builtin(BuiltinCommand::Sort {
                field: field(&["name"]),
                direction: SortDirection::Asc,
            }))
        );
    }

    #[test]
    fn test_filter_eq() {
        assert_ast!(
            ";filter .rank = 1",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["rank"]),
                BinOp::Eq,
                Expr::Int(1),
            ))))
        );
    }

    #[test]
    fn test_filter_ne() {
        assert_ast!(
            ";filter .rank != 1",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["rank"]),
                BinOp::Ne,
                Expr::Int(1),
            ))))
        );
    }

    #[test]
    fn test_filter_gt() {
        assert_ast!(
            ";filter .score > 900",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["score"]),
                BinOp::Gt,
                Expr::Int(900),
            ))))
        );
    }

    #[test]
    fn test_filter_lt() {
        assert_ast!(
            ";filter .score < 100",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["score"]),
                BinOp::Lt,
                Expr::Int(100),
            ))))
        );
    }

    #[test]
    fn test_filter_ge() {
        assert_ast!(
            ";filter .bpm >= 200",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["bpm"]),
                BinOp::Ge,
                Expr::Int(200),
            ))))
        );
    }

    #[test]
    fn test_filter_le() {
        assert_ast!(
            ";filter .bpm <= 300",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["bpm"]),
                BinOp::Le,
                Expr::Int(300),
            ))))
        );
    }

    #[test]
    fn test_filter_above_keyword() {
        assert_ast!(
            ";filter .bpm above 200",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["bpm"]),
                BinOp::Ge,
                Expr::Int(200),
            ))))
        );
    }

    #[test]
    fn test_filter_below_keyword() {
        assert_ast!(
            ";filter .bpm below 300",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["bpm"]),
                BinOp::Le,
                Expr::Int(300),
            ))))
        );
    }

    #[test]
    fn test_filter_is_keyword() {
        assert_ast!(
            ";filter .status is active",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["status"]),
                BinOp::Eq,
                Expr::Str("active".to_string()),
            ))))
        );
    }

    #[test]
    fn test_filter_where_keyword_alias() {
        assert_ast!(
            ";where .age > 18",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["age"]),
                BinOp::Gt,
                Expr::Int(18),
            ))))
        );
    }

    #[test]
    fn test_filter_float() {
        assert_ast!(
            ";filter .acc > 98.5",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["acc"]),
                BinOp::Gt,
                Expr::Float(98.5),
            ))))
        );
    }

    #[test]
    fn test_filter_bool_true() {
        assert_ast!(
            ";filter .fc = true",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["fc"]),
                BinOp::Eq,
                Expr::Bool(true),
            ))))
        );
    }

    #[test]
    fn test_filter_bool_false() {
        assert_ast!(
            ";filter .ranked = false",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["ranked"]),
                BinOp::Eq,
                Expr::Bool(false),
            ))))
        );
    }

    #[test]
    fn test_filter_string_literal() {
        assert_ast!(
            ";filter .mode = osu",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["mode"]),
                BinOp::Eq,
                Expr::Str("osu".to_string()),
            ))))
        );
    }

    #[test]
    fn test_filter_contains() {
        assert_ast!(
            ";filter .title contains loved",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["title"]),
                BinOp::Contains,
                Expr::Str("loved".to_string()),
            ))))
        );
    }

    #[test]
    fn test_filter_in_vector() {
        assert_ast!(
            ";filter .status in (active, banned)",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["status"]),
                BinOp::In,
                Expr::Vector(vec![
                    Expr::Str("active".to_string()),
                    Expr::Str("banned".to_string()),
                ]),
            ))))
        );
    }

    #[test]
    fn test_filter_in_field() {
        assert_ast!(
            ";filter HD in .mods",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                Expr::Str("HD".to_string()),
                BinOp::In,
                field(&["mods"]),
            ))))
        );
    }

    #[test]
    fn test_filter_and() {
        assert_ast!(
            ";filter .bpm > 200 and .acc > 95",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                binary(field(&["bpm"]), BinOp::Gt, Expr::Int(200)),
                BinOp::And,
                binary(field(&["acc"]), BinOp::Gt, Expr::Int(95)),
            ))))
        );
    }

    #[test]
    fn test_filter_or() {
        assert_ast!(
            ";filter .bpm > 250 or .acc > 99",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                binary(field(&["bpm"]), BinOp::Gt, Expr::Int(250)),
                BinOp::Or,
                binary(field(&["acc"]), BinOp::Gt, Expr::Int(99)),
            ))))
        );
    }

    #[test]
    fn test_filter_not() {
        assert_ast!(
            ";filter not .fc",
            cmd(Command::Builtin(BuiltinCommand::Filter(Expr::Not(
                Box::new(field(&["fc"])),
            ))))
        );
    }

    #[test]
    fn test_filter_not_binds_tighter_than_and() {
        assert_ast!(
            ";filter not .a and .b",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                Expr::Not(Box::new(field(&["a"]))),
                BinOp::And,
                field(&["b"]),
            ))))
        );
    }

    #[test]
    fn test_filter_nested_field_path() {
        assert_ast!(
            ";filter .user.age > 18",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["user", "age"]),
                BinOp::Gt,
                Expr::Int(18),
            ))))
        );
    }

    #[test]
    fn test_filter_grouped_binary_unwrapped() {
        assert_ast!(
            ";filter (.bpm > 200)",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                field(&["bpm"]),
                BinOp::Gt,
                Expr::Int(200),
            ))))
        );
    }

    #[test]
    fn test_filter_arithmetic_precedence() {
        assert_ast!(
            ";filter .score + 10 > 100",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                binary(field(&["score"]), BinOp::Add, Expr::Int(10)),
                BinOp::Gt,
                Expr::Int(100),
            ))))
        );
    }

    #[test]
    fn test_filter_mul_precedence_over_add() {
        assert_ast!(
            ";filter 2 + 3 * 4 > 0",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                binary(
                    Expr::Int(2),
                    BinOp::Add,
                    binary(Expr::Int(3), BinOp::Mul, Expr::Int(4)),
                ),
                BinOp::Gt,
                Expr::Int(0),
            ))))
        );
    }

    #[test]
    fn test_custom_no_args() {
        assert_ast!(";top", cmd(custom("top")));
    }

    #[test]
    fn test_custom_with_string_arg() {
        assert_ast!(
            ";top chocomint",
            cmd(Command::Custom(CustomCommand {
                name: "top".to_string(),
                arg: Some(Expr::Str("chocomint".to_string())),
                flags: HashSet::new(),
                options: HashMap::new(),
            }))
        );
    }

    #[test]
    fn test_custom_with_int_arg() {
        assert_ast!(
            ";recent 5",
            cmd(Command::Custom(CustomCommand {
                name: "recent".to_string(),
                arg: Some(Expr::Int(5)),
                flags: HashSet::new(),
                options: HashMap::new(),
            }))
        );
    }

    #[test]
    fn test_custom_with_multiple_args() {
        assert_ast!(
            ";search hello 42",
            cmd(Command::Custom(CustomCommand {
                name: "search".to_string(),
                arg: Some(Expr::Vector(vec![
                    Expr::Str("hello".to_string()),
                    Expr::Int(42)
                ])),
                flags: HashSet::new(),
                options: HashMap::new(),
            }))
        );
    }

    #[test]
    fn test_custom_with_flag() {
        assert_ast!(
            ";top --global",
            cmd(Command::Custom(CustomCommand {
                name: "top".to_string(),
                arg: None,
                flags: {
                    let mut s = HashSet::new();
                    s.insert("global".to_string());
                    s
                },
                options: HashMap::new(),
            }))
        );
    }

    #[test]
    fn test_custom_with_multiple_flags() {
        assert_ast!(
            ";top --global --recent",
            cmd(Command::Custom(CustomCommand {
                name: "top".to_string(),
                arg: None,
                flags: ["global", "recent"].iter().map(|s| s.to_string()).collect(),
                options: HashMap::new(),
            }))
        );
    }

    #[test]
    fn test_custom_with_int_option() {
        assert_ast!(
            ";top --limit 10",
            cmd(Command::Custom(CustomCommand {
                name: "top".to_string(),
                arg: None,
                flags: HashSet::new(),
                options: [("limit".to_string(), Expr::Int(10))].into(),
            }))
        );
    }

    #[test]
    fn test_custom_with_string_option() {
        assert_ast!(
            ";top --mode standard",
            cmd(Command::Custom(CustomCommand {
                name: "top".to_string(),
                arg: None,
                flags: HashSet::new(),
                options: [("mode".to_string(), Expr::Str("standard".to_string()))].into(),
            }))
        );
    }

    #[test]
    fn test_custom_mixed_args_flags_options() {
        assert_ast!(
            ";top chocomint --global --limit 5",
            cmd(Command::Custom(CustomCommand {
                name: "top".to_string(),
                arg: Some(Expr::Str("chocomint".to_string())),
                flags: {
                    let mut s = HashSet::new();
                    s.insert("global".to_string());
                    s
                },
                options: [("limit".to_string(), Expr::Int(5))].into(),
            }))
        );
    }

    #[test]
    fn test_pipe() {
        assert_ast!(
            ";top >> count",
            PipelineNode::Pipe {
                lhs: Box::new(cmd(custom("top"))),
                rhs: Box::new(cmd(Command::Builtin(BuiltinCommand::Count))),
            }
        );
    }

    #[test]
    fn test_concat_with_keyword() {
        assert_ast!(
            ";top with recent",
            PipelineNode::Concat {
                lhs: Box::new(cmd(custom("top"))),
                rhs: Box::new(cmd(custom("recent"))),
            }
        );
    }

    #[test]
    fn test_concat_plusplus() {
        assert_ast!(
            ";top ++ recent",
            PipelineNode::Concat {
                lhs: Box::new(cmd(custom("top"))),
                rhs: Box::new(cmd(custom("recent"))),
            }
        );
    }

    #[test]
    fn test_chained_pipe_is_left_associative() {
        assert_ast!(
            ";top >> filter .bpm > 200 >> take 5",
            PipelineNode::Pipe {
                lhs: Box::new(PipelineNode::Pipe {
                    lhs: Box::new(cmd(custom("top"))),
                    rhs: Box::new(cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                        field(&["bpm"]),
                        BinOp::Gt,
                        Expr::Int(200),
                    ))))),
                }),
                rhs: Box::new(cmd(Command::Builtin(BuiltinCommand::Limit(5)))),
            }
        );
    }

    #[test]
    fn test_pipe_into_sort() {
        assert_ast!(
            ";top >> sort .bpm --asc",
            PipelineNode::Pipe {
                lhs: Box::new(cmd(custom("top"))),
                rhs: Box::new(cmd(Command::Builtin(BuiltinCommand::Sort {
                    field: field(&["bpm"]),
                    direction: SortDirection::Asc,
                }))),
            }
        );
    }

    #[test]
    fn test_map_field() {
        assert_ast!(
            ";top >> map .title",
            PipelineNode::Pipe {
                lhs: Box::new(cmd(custom("top"))),
                rhs: Box::new(cmd(Command::Builtin(BuiltinCommand::Map(field(&[
                    "title"
                ]))))),
            }
        );
    }

    #[test]
    fn test_map_it() {
        assert_ast!(
            ";top >> map it",
            PipelineNode::Pipe {
                lhs: Box::new(cmd(custom("top"))),
                rhs: Box::new(cmd(Command::Builtin(BuiltinCommand::Map(Expr::It)))),
            }
        );
    }

    #[test]
    fn test_map_arithmetic() {
        assert_ast!(
            ";top >> map it * 2",
            PipelineNode::Pipe {
                lhs: Box::new(cmd(custom("top"))),
                rhs: Box::new(cmd(Command::Builtin(BuiltinCommand::Map(binary(
                    Expr::It,
                    BinOp::Mul,
                    Expr::Int(2),
                ))))),
            }
        );
    }

    #[test]
    fn test_filter_complex_predicate() {
        assert_ast!(
            ";top >> filter (.bpm >= 230 and HD in .mods) or .bpm >= 250",
            PipelineNode::Pipe {
                lhs: Box::new(cmd(custom("top"))),
                rhs: Box::new(cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                    binary(
                        binary(field(&["bpm"]), BinOp::Ge, Expr::Int(230)),
                        BinOp::And,
                        binary(Expr::Str("HD".to_string()), BinOp::In, field(&["mods"])),
                    ),
                    BinOp::Or,
                    binary(field(&["bpm"]), BinOp::Ge, Expr::Int(250)),
                ))))),
            }
        );
    }

    #[test]
    fn test_filter_sub() {
        assert_ast!(
            ";filter it - 1 > 0",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                binary(Expr::It, BinOp::Sub, Expr::Int(1)),
                BinOp::Gt,
                Expr::Int(0),
            ))))
        );
    }

    #[test]
    fn test_filter_div() {
        assert_ast!(
            ";filter it / 2 > 1",
            cmd(Command::Builtin(BuiltinCommand::Filter(binary(
                binary(Expr::It, BinOp::Div, Expr::Int(2)),
                BinOp::Gt,
                Expr::Int(1),
            ))))
        );
    }

    #[test]
    fn test_map_sub() {
        assert_ast!(
            ";top >> map it - 1",
            PipelineNode::Pipe {
                lhs: Box::new(cmd(custom("top"))),
                rhs: Box::new(cmd(Command::Builtin(BuiltinCommand::Map(binary(
                    Expr::It,
                    BinOp::Sub,
                    Expr::Int(1),
                ))))),
            }
        );
    }

    #[test]
    fn test_error_no_leading_semicolon() {
        let toks = make_tokenizer("count");
        let result = Parser::parse(toks);
        assert!(matches!(
            result,
            Err(ParserError::UnexpectedTokenWithContext(..))
        ));
    }

    #[test]
    fn test_error_premature_eof() {
        let toks = make_tokenizer(";count extra");
        let result = Parser::parse(toks);
        assert!(matches!(
            result,
            Err(ParserError::UnexpectedTrailingToken(..))
        ));
    }

    #[test]
    fn test_error_unexpected_eof_in_filter() {
        let toks = make_tokenizer(";filter");
        let result = Parser::parse(toks);
        assert!(matches!(result, Err(ParserError::UnexpectedEOF)));
    }

    #[test]
    fn test_error_empty_vector_in_filter() {
        let toks = make_tokenizer(";filter ()");
        let result = Parser::parse(toks);
        assert!(matches!(result, Err(ParserError::EmptyVector(..))));
    }

    #[test]
    fn test_error_expected_string_for_option_key() {
        let toks = make_tokenizer(";top --5");
        let result = Parser::parse(toks);
        assert!(matches!(result, Err(ParserError::ExpectedString(..))));
    }
}
