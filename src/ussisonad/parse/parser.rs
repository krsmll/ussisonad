use crate::ussisonad::lex::LexError;
use crate::ussisonad::lex::Token;
use crate::ussisonad::lex::{LexResult, Spanned};
use crate::ussisonad::parse::ast::{BinOp, Expr};
use std::iter::Peekable;

pub struct Parser<T: Iterator<Item = LexResult>> {
    tokens: Peekable<T>,
    lex_errors: Vec<LexError>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParseError {
    UnexpectedToken(Spanned),
    UnsupportedOperator(Spanned),
    IntParseError(Spanned),
    FloatParseError(Spanned),
    EmptyVector((usize, usize)),
    UnexpectedEOF,
}

impl<T: Iterator<Item = LexResult>> Parser<T> {
    pub fn new(tokens: T) -> Self {
        Self {
            tokens: tokens.peekable(),
            lex_errors: Vec::new(),
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
    fn peek(&mut self) -> Option<&Spanned> {
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

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let (tok, start, end) = self.next().ok_or(ParseError::UnexpectedEOF)?;

        let mut lhs = match tok {
            Token::Int(s) => self.parse_int((Token::Int(s), start, end))?,
            Token::Float(s) => self.parse_float((Token::Float(s), start, end))?,
            Token::LeftParen => self.parse_vector(start)?,
            Token::Dot => Expr::It,
            Token::Ident(s) => Expr::Ident(s),
            Token::Str(s) => Expr::Str(s),
            Token::LeftBrace => todo!(),
            _ => return Err(ParseError::UnexpectedToken((tok, start, end))),
        };

        loop {
            let tok = match self.next() {
                Some(span) => span,
                None => break,
            };

            let op = match tok.0 {
                Token::Add => BinOp::Add,
                Token::Sub => BinOp::Sub,
                Token::Mul => BinOp::Mul,
                Token::Div => BinOp::Div,
                Token::Mod => BinOp::Mod,
                Token::In => BinOp::In,
                Token::Or => BinOp::Or,
                Token::And => BinOp::And,
                Token::Eq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                Token::Gt => BinOp::Gt,
                Token::Lt => BinOp::Lt,
                Token::Ge => BinOp::Ge,
                Token::Le => BinOp::Le,
                Token::GtGt => BinOp::Pipe,
                Token::Dot => BinOp::Get,
                Token::AddAdd => BinOp::Concat,
                Token::RightParen | Token::RightBrace => break,
                _ => return Err(ParseError::UnsupportedOperator(tok.clone())),
            };

            let (lbp, rbp) = op.bp();
            if lbp < min_bp {
                break;
            }

            let rhs = self.parse_expr(rbp)?;
            lhs = Expr::Binary {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            }
        }

        Ok(lhs)
    }

    fn parse_vector(&mut self, left_paren_pos: usize) -> Result<Expr, ParseError> {
        let mut exprs: Vec<Expr> = Vec::new();

        loop {
            if let Some((Token::RightParen, pos, _)) = self.peek() {
                let expr = match exprs[..] {
                    [] => return Err(ParseError::EmptyVector((left_paren_pos, *pos))),
                    [Expr::Binary { .. }] => Ok(exprs.pop().unwrap()),
                    _ => Ok(Expr::Vector(exprs, (left_paren_pos, *pos))),
                };

                self.next();
                return expr;
            }

            let (tok, start, end) = self.peek().ok_or(ParseError::UnexpectedEOF)?;
            match tok {
                Token::Str(_)
                | Token::Ident(_)
                | Token::Int(_)
                | Token::Float(_)
                | Token::LeftBrace => exprs.push(self.parse_expr(0)?),
                Token::Comma => continue,
                tok => return Err(ParseError::UnexpectedToken((tok.clone(), *start, *end))),
            }
        }
    }

    fn parse_int(&mut self, spanned: Spanned) -> Result<Expr, ParseError> {
        let (tok, start, end) = spanned;
        match tok {
            Token::Int(s) => s
                .parse::<i64>()
                .map(|i| Expr::Int(i))
                .map_err(|_| ParseError::IntParseError((Token::Int(s), start, end))),
            _ => Err(ParseError::IntParseError((tok, start, end))),
        }
    }

    fn parse_float(&mut self, spanned: Spanned) -> Result<Expr, ParseError> {
        let (tok, start, end) = spanned;
        match tok {
            Token::Float(s) => s
                .parse::<f64>()
                .map(|f| Expr::Float(f))
                .map_err(|_| ParseError::FloatParseError((Token::Float(s), start, end))),
            _ => Err(ParseError::FloatParseError((tok, start, end))),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ussisonad::lex::make_tokenizer;
    use crate::ussisonad::parse::ast::{BinOp, Expr};
    use crate::ussisonad::parse::parser::Parser;

    macro_rules! assert_ast {
        ($src:expr, $expected:expr) => {
            let toks = make_tokenizer($src);
            let mut parser = Parser::new(toks);
            let result = parser.parse_expr(0);

            assert_eq!(result, Ok($expected))
        };
    }

    fn create_binary(lhs: Expr, op: BinOp, rhs: Expr) -> Expr {
        Expr::Binary {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        }
    }

    #[test]
    fn binary_numbers() {
        assert_ast!(
            "6 + 7",
            create_binary(Expr::Int(6), BinOp::Add, Expr::Int(7))
        );
    }

    #[test]
    fn binary_numbers_with_two_ops() {
        assert_ast!(
            "6 + 7 * 727",
            create_binary(
                Expr::Int(6),
                BinOp::Add,
                create_binary(Expr::Int(7), BinOp::Mul, Expr::Int(727))
            )
        );
    }

    #[test]
    fn binary_numbers_with_parentheses() {
        assert_ast!(
            "(6 + 7 * 727) + 25",
            create_binary(
                create_binary(
                    Expr::Int(6),
                    BinOp::Add,
                    create_binary(Expr::Int(7), BinOp::Mul, Expr::Int(727)),
                ),
                BinOp::Add,
                Expr::Int(25)
            )
        );
    }

    #[test]
    fn binary_logic() {
        assert_ast!(
            "1 = 1",
            create_binary(Expr::Int(1), BinOp::Eq, Expr::Int(1))
        );
    }
}
