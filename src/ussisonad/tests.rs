use crate::ussisonad::error::{LexError, LexErrorType, SrcSpan};
use crate::ussisonad::lexer::make_tokenizer;
use crate::ussisonad::token::Token;

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
    };
}

macro_rules! assert_error {
    ($src:expr, $($expected:expr),* $(,)?) => {
        let errors = tokenize_error_sequence($src);
        let expected = vec![$($expected),*];
        assert_eq!(
            errors.len(),
            expected.len(),
            "Error count mismatch for input: {}",
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
fn simple_command_no_options_no_values() {
    assert_tokens!(";top", Token::Semicolon, Token::Word("top".to_string()));
}

#[test]
fn simple_command_with_value() {
    assert_tokens!(
        ";top Slay",
        Token::Semicolon,
        Token::Word("top".to_string()),
        Token::Word("Slay".to_string())
    );

    assert_tokens!(
        ";top \"Tiger Claw\"",
        Token::Semicolon,
        Token::Word("top".to_string()),
        Token::String("Tiger Claw".to_string())
    );

    assert_tokens!(
        ";top (Slay, \"Tiger Claw\")",
        Token::Semicolon,
        Token::Word("top".to_string()),
        Token::LeftParen,
        Token::Word("Slay".to_string()),
        Token::Comma,
        Token::String("Tiger Claw".to_string()),
        Token::RightParen,
    );

    assert_tokens!(
        ";top (Slay Lotragon blourgh \"Tiger Claw\")",
        Token::Semicolon,
        Token::Word("top".to_string()),
        Token::LeftParen,
        Token::Word("Slay".to_string()),
        Token::Word("Lotragon".to_string()),
        Token::Word("blourgh".to_string()),
        Token::String("Tiger Claw".to_string()),
        Token::RightParen,
    );

    assert_tokens!(
        ";top CreeperBro_2015",
        Token::Semicolon,
        Token::Word("top".to_string()),
        Token::Word("CreeperBro_2015".to_string()),
    );

    assert_tokens!(
        ";square 67",
        Token::Semicolon,
        Token::Word("square".to_string()),
        Token::Integer("67".to_string()),
    );

    assert_tokens!(
        ";square 67 + 7.27",
        Token::Semicolon,
        Token::Word("square".to_string()),
        Token::Integer("67".to_string()),
        Token::Plus,
        Token::Float("7.27".to_string()),
    );
}

#[test]
fn simple_command_with_options() {
    assert_tokens!(
        ";top --whatever",
        Token::Semicolon,
        Token::Word("top".to_string()),
        Token::MinusMinus,
        Token::Word("whatever".to_string())
    );

    assert_tokens!(
        ";top --some value -lol rofl -flag",
        Token::Semicolon,
        Token::Word("top".to_string()),
        Token::MinusMinus,
        Token::Word("some".to_string()),
        Token::Word("value".to_string()),
        Token::Minus,
        Token::Word("lol".to_string()),
        Token::Word("rofl".to_string()),
        Token::Minus,
        Token::Word("flag".to_string()),
    );
}



#[test]
fn simple_command_error_unclosed_string_value() {
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
