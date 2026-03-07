use crate::ussisonad::lex::LexError;
use crate::ussisonad::lex::Token;
use crate::ussisonad::lex::{LexResult, Spanned};
use crate::ussisonad::parse::ast::{
    BinOp, BuiltinCommand, Command, CustomCommand, Expr, PipelineNode, SortDirection,
};
use std::collections::{HashMap, HashSet};
use std::iter::Peekable;

pub struct Parser<T: Iterator<Item = LexResult>> {
    tokens: Peekable<T>,
    lex_errors: Vec<LexError>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ParserError {
    parsing_error: ParsingError,
    lex_errors: Vec<LexError>,
}

impl ParserError {
    fn new(parsing_error: ParsingError, lex_errors: Vec<LexError>) -> Self {
        Self {
            parsing_error,
            lex_errors,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParsingError {
    UnexpectedToken(Spanned),
    UnexpectedTokenWithContext(Token, Spanned),
    ExpectedString(Spanned),
    IntParseError(Spanned),
    FloatParseError(Spanned),
    EmptyVector((usize, usize)),
    PrematureEOF(Spanned),
    UnexpectedEOF,
}

impl<T: Iterator<Item = LexResult>> Parser<T> {
    pub fn new(tokens: T) -> Self {
        Self {
            tokens: tokens.peekable(),
            lex_errors: Vec::new(),
        }
    }

    pub fn parse(&mut self) -> Result<PipelineNode, ParserError> {
        self.expect(Token::Semicolon)
            .map_err(|err| ParserError::new(err, vec![]))?;

        let node = self
            .parse_pipeline()
            .map_err(|err| ParserError::new(err, self.lex_errors.clone()))?;
        match self.next() {
            Some(span) => Err(ParserError::new(ParsingError::PrematureEOF(span), vec![])),
            None => {
                if self.lex_errors.is_empty() {
                    Ok(node)
                } else {
                    Err(ParserError::new(ParsingError::UnexpectedEOF, vec![]))
                }
            }
        }
    }

    fn parse_pipeline(&mut self) -> Result<PipelineNode, ParsingError> {
        let left = self.parse_command()?;
        let mut node = PipelineNode::Command(left);

        loop {
            if let Some(tok) = self.peek() {
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
                            left: Box::new(node),
                            right: Box::new(PipelineNode::Command(rhs)),
                        };
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }

        Ok(node)
    }

    fn parse_command(&mut self) -> Result<Command, ParsingError> {
        match self.peek().ok_or(ParsingError::UnexpectedEOF)? {
            Token::Filter => self.parse_filter(),
            Token::Sort => self.parse_sort(),
            Token::Take => self.parse_take(),
            Token::Count => {
                self.next();
                Ok(Command::Builtin(BuiltinCommand::Count))
            }
            Token::Str(_) => self.parse_custom_command(),
            _ => Err(ParsingError::UnexpectedToken(self.next().unwrap())),
        }
    }

    fn parse_custom_command(&mut self) -> Result<Command, ParsingError> {
        let name = self.expect_str()?;
        let mut args = Vec::new();
        let mut flags = HashSet::new();
        let mut options = HashMap::new();

        loop {
            match self.peek() {
                Some(Token::GtGt) | Some(Token::AddAdd) | Some(Token::EOF) | None => break,
                Some(Token::SubSub) => {
                    self.next();
                    let key = self.expect_str()?;
                    match self.peek() {
                        Some(Token::Str(_))
                        | Some(Token::Int(_))
                        | Some(Token::Float(_))
                        | Some(Token::Bool(_)) => {
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
            args,
            flags,
            options,
        };

        Ok(Command::Custom(command))
    }

    fn parse_filter(&mut self) -> Result<Command, ParsingError> {
        self.next();
        let expr = self.parse_expr(0)?;
        Ok(Command::Builtin(BuiltinCommand::Filter(expr)))
    }

    fn parse_sort(&mut self) -> Result<Command, ParsingError> {
        self.next();
        let field = self.parse_expr(0)?;
        let direction = match self.peek() {
            Some(Token::Str(s)) if s == "asc" => {
                self.next();
                SortDirection::Asc
            }
            Some(Token::Str(s)) if s == "desc" => {
                self.next();
                SortDirection::Desc
            }
            _ => SortDirection::Desc,
        };
        Ok(Command::Builtin(BuiltinCommand::Sort { field, direction }))
    }

    fn parse_take(&mut self) -> Result<Command, ParsingError> {
        self.next();
        let n = match self.next() {
            Some((Token::Int(s), start, end)) => Self::parse_int(s, start, end)?,
            Some(t) => return Err(ParsingError::UnexpectedToken(t)),
            None => return Err(ParsingError::UnexpectedEOF),
        };
        Ok(Command::Builtin(BuiltinCommand::Limit(n as u64)))
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ParsingError> {
        let (tok, start, end) = self.next().ok_or(ParsingError::UnexpectedEOF)?;

        let mut lhs = match tok {
            Token::Str(s) => Expr::Str(s),
            Token::Bool(b) => Expr::Bool(b),
            Token::Int(s) => self.parse_int_expr((Token::Int(s), start, end))?,
            Token::Float(s) => self.parse_float_expr((Token::Float(s), start, end))?,
            Token::Dot => self.parse_field_path()?,
            Token::LeftParen => self.parse_group(start)?,
            Token::Not => {
                let rhs = self.parse_expr(80)?;
                Expr::Not(Box::new(rhs))
            }

            tok => return Err(ParsingError::UnexpectedToken((tok, start, end))),
        };

        loop {
            let tok = match self.peek() {
                Some(tok) => tok,
                None => break,
            };

            let op = match Self::token_to_binop(tok) {
                Some(op) => op,
                None => break,
            };

            let (left_bp, right_bp) = op.bp();
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

    fn parse_field_path(&mut self) -> Result<Expr, ParsingError> {
        let mut segments = Vec::new();
        segments.push(self.expect_str()?);

        while let Some(&Token::Dot) = self.peek() {
            self.next();
            segments.push(self.expect_str()?);
        }

        Ok(Expr::FieldPath(segments))
    }

    fn parse_group(&mut self, left_paren_pos: usize) -> Result<Expr, ParsingError> {
        let mut exprs: Vec<Expr> = Vec::new();

        loop {
            let (tok, pos, _) = self.peek_span().ok_or(ParsingError::UnexpectedEOF)?;
            match tok {
                Token::RightParen => {
                    let expr = match exprs[..] {
                        [] => return Err(ParsingError::EmptyVector((left_paren_pos, *pos))),
                        [Expr::Binary { .. }] => exprs.pop().unwrap(),
                        _ => Expr::Vector(exprs),
                    };

                    self.next();
                    return Ok(expr);
                }
                Token::Comma => {
                    self.next();
                    continue;
                }
                _ => exprs.push(self.parse_expr(0)?),
            }
        }
    }

    fn parse_int_expr(&mut self, spanned: Spanned) -> Result<Expr, ParsingError> {
        let (tok, start, end) = spanned;
        match tok {
            Token::Int(s) => Ok(Expr::Int(Self::parse_int(s, start, end)?)),
            _ => Err(ParsingError::IntParseError((tok, start, end))),
        }
    }

    fn parse_int(s: String, start: usize, end: usize) -> Result<i64, ParsingError> {
        s.parse::<i64>()
            .map_err(|_| ParsingError::IntParseError((Token::Int(s), start, end)))
    }

    fn parse_float_expr(&mut self, spanned: Spanned) -> Result<Expr, ParsingError> {
        let (tok, start, end) = spanned;
        match tok {
            Token::Float(s) => s
                .parse::<f64>()
                .map(|f| Expr::Float(f))
                .map_err(|_| ParsingError::FloatParseError((Token::Float(s), start, end))),
            _ => Err(ParsingError::FloatParseError((tok, start, end))),
        }
    }

    fn token_to_binop(token: &Token) -> Option<BinOp> {
        match token {
            Token::Add => Some(BinOp::Add),
            Token::Sub => Some(BinOp::Sub),
            Token::Mul => Some(BinOp::Mul),
            Token::Div => Some(BinOp::Div),
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
        if let Some(token) = self.tokens.next() {
            match token {
                Ok(token) => Some(token),
                Err(err) => {
                    self.lex_errors.push(err);
                    None
                }
            }
        } else {
            None
        }
    }

    fn peek_span(&mut self) -> Option<&Spanned> {
        if let Some(peeked) = self.tokens.peek() {
            match peeked {
                Ok(token) => Some(token),
                Err(err) => {
                    self.lex_errors.push(*err);
                    None
                }
            }
        } else {
            None
        }
    }

    fn peek(&mut self) -> Option<&Token> {
        self.peek_span().map(|span| &span.0)
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParsingError> {
        match self.next() {
            Some((tok, _, _)) if tok == expected => Ok(()),
            Some(actual) => Err(ParsingError::UnexpectedTokenWithContext(expected, actual)),
            None => Err(ParsingError::UnexpectedEOF),
        }
    }

    fn expect_str(&mut self) -> Result<String, ParsingError> {
        match self.next() {
            Some((Token::Str(s), _, _)) => Ok(s),
            Some(actual) => Err(ParsingError::ExpectedString(actual)),
            None => Err(ParsingError::UnexpectedEOF),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ussisonad::lex::make_tokenizer;
    use crate::ussisonad::parse::ast::{
        BinOp, BuiltinCommand, Command, CustomCommand, Expr, PipelineNode, SortDirection,
    };
    use crate::ussisonad::parse::parser::Parser;
    use std::collections::{HashMap, HashSet};

    macro_rules! assert_ast {
        ($src:expr, $expected:expr) => {
            let toks = make_tokenizer($src);
            let mut parser = Parser::new(toks);
            let result = parser.parse();
            assert_eq!(result, Ok($expected))
        };
    }

    fn cmd(c: Command) -> PipelineNode {
        PipelineNode::Command(c)
    }

    fn custom(name: &str) -> Command {
        Command::Custom(CustomCommand {
            name: name.to_string(),
            args: vec![],
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
            ";sort .bpm asc",
            cmd(Command::Builtin(BuiltinCommand::Sort {
                field: field(&["bpm"]),
                direction: SortDirection::Asc,
            }))
        );
    }

    #[test]
    fn test_sort_desc_explicit() {
        assert_ast!(
            ";sort .bpm desc",
            cmd(Command::Builtin(BuiltinCommand::Sort {
                field: field(&["bpm"]),
                direction: SortDirection::Desc,
            }))
        );
    }

    #[test]
    fn test_order_keyword_alias() {
        assert_ast!(
            ";order .name asc",
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
                args: vec![Expr::Str("chocomint".to_string())],
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
                args: vec![Expr::Int(5)],
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
                args: vec![Expr::Str("hello".to_string()), Expr::Int(42)],
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
                args: vec![],
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
                args: vec![],
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
                args: vec![],
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
                args: vec![],
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
                args: vec![Expr::Str("chocomint".to_string())],
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
                left: Box::new(cmd(custom("top"))),
                right: Box::new(cmd(custom("recent"))),
            }
        );
    }

    #[test]
    fn test_concat_plusplus() {
        assert_ast!(
            ";top ++ recent",
            PipelineNode::Concat {
                left: Box::new(cmd(custom("top"))),
                right: Box::new(cmd(custom("recent"))),
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
            ";top >> sort .bpm asc",
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
}
