use crate::ussisonad::lex::error::{LexError, LexErrorType, SrcSpan};
use crate::ussisonad::lex::lexer::make_tokenizer;
use crate::ussisonad::lex::token::Token;

macro_rules! assert_tokens {
   ($src:expr, $($expected:expr),* $(,)?) => {
        let tokens = tokenize_sequence($src);
        let expected = vec![$($expected),*];
        assert_eq!(
            tokens.len(),
            expected.len(),
            "Token count mismatch for input: {}",
            $src
        );
        for (i, (token, exp)) in tokens.iter().zip(expected.iter()).enumerate() {
            assert_eq!(token, exp, "Token mismatch at index {}: expected {:?}, got {:?}", i, exp, token);
        }
       ()
    };
}

macro_rules! assert_error {
    ($src:expr, $($expected:expr),* $(,)?) => {
        let errors = tokenize_error_sequence($src);
        let expected = vec![$($expected),*];
        assert_eq!(
            errors.len(),
            expected.len(),
            "Error count mismatch for input: '{}'",
            $src
        );
        for (i, (error, exp)) in errors.iter().zip(expected.iter()).enumerate() {
            assert_eq!(error, exp, "Error mismatch at index {}: expected {:?}, got {:?}", i, exp, error);
        }
    };
}

#[allow(dead_code)]
fn tokenize_sequence(source: &str) -> Vec<Token> {
    make_tokenizer(source)
        .map(|x| x.unwrap())
        .map(|x| x.0)
        .collect()
}

#[allow(dead_code)]
fn tokenize_error_sequence(source: &str) -> Vec<LexError> {
    make_tokenizer(source)
        .filter(|x| x.is_err())
        .map(|x| x.unwrap_err())
        .collect()
}

#[test]
fn flag() {
    assert_tokens!(";top", Token::Semicolon, Token::Ident("top".to_string()));
}

#[test]
fn value_identifier() {
    assert_tokens!(
        ";top Slay",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::Ident("Slay".to_string())
    );
}

#[test]
fn value_string() {
    assert_tokens!(
        ";top \"Tiger Claw\"",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::Str("Tiger Claw".to_string())
    );
}

#[test]
fn value_array_with_commas() {
    assert_tokens!(
        ";top (Slay, \"Tiger Claw\")",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::LeftParen,
        Token::Ident("Slay".to_string()),
        Token::Comma,
        Token::Str("Tiger Claw".to_string()),
        Token::RightParen,
    );
}

#[test]
fn value_array_no_commas() {
    assert_tokens!(
        ";top (Slay Lotragon blourgh \"Tiger Claw\")",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::LeftParen,
        Token::Ident("Slay".to_string()),
        Token::Ident("Lotragon".to_string()),
        Token::Ident("blourgh".to_string()),
        Token::Str("Tiger Claw".to_string()),
        Token::RightParen,
    );
}

#[test]
fn value_integer() {
    assert_tokens!(
        ";square 67",
        Token::Semicolon,
        Token::Ident("square".to_string()),
        Token::Int("67".to_string()),
    );
}

#[test]
fn value_expr() {
    assert_tokens!(
        ";square 67 + 7.27",
        Token::Semicolon,
        Token::Ident("square".to_string()),
        Token::Int("67".to_string()),
        Token::Add,
        Token::Float("7.27".to_string()),
    );
}

#[test]
fn value_with_underscore() {
    assert_tokens!(
        ";top CreeperBro_2015",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::Ident("CreeperBro_2015".to_string()),
    );
}

#[test]
fn option_all_types() {
    assert_tokens!(
        ";top --limit 5 -mode standard -fc",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::SubSub,
        Token::Ident("limit".to_string()),
        Token::Int("5".to_string()),
        Token::Sub,
        Token::Ident("mode".to_string()),
        Token::Ident("standard".to_string()),
        Token::Sub,
        Token::Ident("fc".to_string()),
    );
}

#[test]
fn options_with_value() {
    assert_tokens!(
        ";top Slay --limit 5 --mode standard",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::Ident("Slay".to_string()),
        Token::SubSub,
        Token::Ident("limit".to_string()),
        Token::Int("5".to_string()),
        Token::SubSub,
        Token::Ident("mode".to_string()),
        Token::Ident("standard".to_string())
    );
}

#[test]
fn error_unclosed_string_value() {
    let s = ";top \"Tiger Claw";
    let last_quote_pos = s.rfind('\"').unwrap() + 1;
    assert_error!(
        s,
        LexError {
            error: LexErrorType::UnexpectedStringEnd,
            location: SrcSpan {
                start: last_quote_pos,
                end: last_quote_pos
            },
        }
    );
}

#[test]
fn dot_access() {
    assert_tokens!(
        ";top .some.value",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::Dot,
        Token::Ident("some".to_string()),
        Token::Dot,
        Token::Ident("value".to_string()),
    );
}

#[test]
fn error_unfinished_dot_access() {
    let s = ";top . --limit 5";
    let err_idx = s.rfind('.').unwrap();
    assert_error!(
        s,
        LexError {
            error: LexErrorType::UnfinishedDotAccess,
            location: SrcSpan {
                start: err_idx,
                end: err_idx
            }
        }
    );
}

#[test]
fn error_unfinished_dot_access_at_eof() {
    let s = ";top .";
    let err_idx = s.len() - 1;
    assert_error!(
        s,
        LexError {
            error: LexErrorType::UnfinishedDotAccess,
            location: SrcSpan {
                start: err_idx,
                end: err_idx
            }
        }
    );
}

#[test]
fn with_one_pipe_no_expr() {
    assert_tokens!(
        ";top >> count",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::GtGt,
        Token::Count,
    );
}

#[test]
fn one_pipe_with_expr() {
    assert_tokens!(
        ";top >> filter .bpm >= 250",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::GtGt,
        Token::Filter,
        Token::Dot,
        Token::Ident("bpm".to_string()),
        Token::Ge,
        Token::Int("250".to_string()),
    );
}

#[test]
fn multiple_pipes_with_expr() {
    assert_tokens!(
        ";top chocomint >> filter .bpm >= 250 >> sort .acc --ascending",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::Ident("chocomint".to_string()),
        Token::GtGt,
        Token::Filter,
        Token::Dot,
        Token::Ident("bpm".to_string()),
        Token::Ge,
        Token::Int("250".to_string()),
        Token::GtGt,
        Token::Sort,
        Token::Dot,
        Token::Ident("acc".to_string()),
        Token::SubSub,
        Token::Ident("ascending".to_string()),
    );
}

#[test]
fn subcommand() {
    assert_tokens!(
        ";tops (Slay, Lotragon) ++ { top mrekk --server akatsuki } >> sort .bpm",
        Token::Semicolon,
        Token::Ident("tops".to_string()),
        Token::LeftParen,
        Token::Ident("Slay".to_string()),
        Token::Comma,
        Token::Ident("Lotragon".to_string()),
        Token::RightParen,
        Token::AddAdd,
        Token::LeftBrace,
        Token::Ident("top".to_string()),
        Token::Ident("mrekk".to_string()),
        Token::SubSub,
        Token::Ident("server".to_string()),
        Token::Ident("akatsuki".to_string()),
        Token::RightBrace,
        Token::GtGt,
        Token::Sort,
        Token::Dot,
        Token::Ident("bpm".to_string()),
    );
}

#[test]
fn logic() {
    assert_tokens!(";top >> filter (.bpm >= 230 and HD in .mods) or .bpm >= 250",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::GtGt,
        Token::Filter,
        Token::LeftParen,
        Token::Dot,
        Token::Ident("bpm".to_string()),
        Token::Ge,
        Token::Int("230".to_string()),
        Token::And,
        Token::Ident("HD".to_string()),
        Token::In,
        Token::Dot,
        Token::Ident("mods".to_string()),
        Token::RightParen,
        Token::Or,
        Token::Dot,
        Token::Ident("bpm".to_string()),
        Token::Ge,
        Token::Int("250".to_string()),
    );
}

#[test]
fn logic_with_subcommand() {
    assert_tokens!(
        ";top >> filter .title in { top blourgh >> map .title } or .acc > 98.5",
        Token::Semicolon,
        Token::Ident("top".to_string()),
        Token::GtGt,
        Token::Filter,
        Token::Dot,
        Token::Ident("title".to_string()),
        Token::In,
        Token::LeftBrace,
        Token::Ident("top".to_string()),
        Token::Ident("blourgh".to_string()),
        Token::GtGt,
        Token::Map,
        Token::Dot,
        Token::Ident("title".to_string()),
        Token::RightBrace,
        Token::Or,
        Token::Dot,
        Token::Ident("acc".to_string()),
        Token::Gt,
        Token::Float("98.5".to_string()),
    );
}
