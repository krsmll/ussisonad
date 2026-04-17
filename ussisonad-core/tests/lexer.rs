#[cfg(test)]
mod tests {
    use ussisonad_core::*;

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
            ()
        };
    }

    macro_rules! assert_error {
        ($src:expr, $($expected:expr),* $(,)?) => {
            let got = tokenize_error_sequence($src);
            let expected = vec![$($expected),*];
            assert_eq!(
                got.len(),
                expected.len(),
                "Error count mismatch for input: '{}'\n  expected: {:?}\n  got: {:?}",
                $src,
                expected,
                got,
            );
            for (i, (error, exp)) in got.into_iter().zip(expected.iter()).enumerate() {
                assert_eq!(error, *exp, "Error mismatch at index {}: expected {:?}, got {:?}", i, exp, error);
            }
            ()
        };
    }

    fn tokenize_sequence(source: &str) -> Vec<Token> {
        Lexer::new_from_str(source)
            .map(|x| x.expect("expected all tokens to be ok"))
            .map(|x| x.0)
            .collect()
    }

    fn tokenize_error_sequence(source: &str) -> Vec<LexError> {
        Lexer::new_from_str(source)
            .filter(LexResult::is_err)
            .map(LexResult::unwrap_err)
            .collect()
    }

    #[test]
    fn test_flag() {
        assert_tokens!(";top", Token::Semicolon, Token::Ident("top".to_string()));
    }

    #[test]
    fn test_value_identifier() {
        assert_tokens!(
            ";top Slay",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Ident("Slay".to_string())
        );
    }

    #[test]
    fn test_value_string() {
        assert_tokens!(
            ";top \"Tiger Claw\"",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Str("Tiger Claw".to_string())
        );
    }

    #[test]
    fn test_value_array_with_commas() {
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
    fn test_value_array_no_commas() {
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
    fn test_value_integer() {
        assert_tokens!(
            ";square 67",
            Token::Semicolon,
            Token::Ident("square".to_string()),
            Token::Int("67".to_string()),
        );
    }

    #[test]
    fn test_value_negative_integer() {
        assert_tokens!(
            ";square -69",
            Token::Semicolon,
            Token::Ident("square".to_string()),
            Token::Int("-69".to_string()),
        );
    }

    #[test]
    fn test_value_expr() {
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
    fn test_value_with_underscore() {
        assert_tokens!(
            ";top CreeperBro_2015",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Ident("CreeperBro_2015".to_string()),
        );
    }

    #[test]
    fn test_option_all_types() {
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
    fn test_options_with_value() {
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
    fn test_error_unclosed_string_value() {
        let s = ";top \"Tiger Claw";
        let last_quote_pos = s.rfind('\"').unwrap() + 1;
        assert_error!(
            s,
            LexError::UnexpectedStringEnd(Loc::Slice(last_quote_pos, s.len())),
        );
    }

    #[test]
    fn test_dot_access() {
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
    fn test_error_unfinished_dot_access() {
        let s = ";top . --limit 5";
        let err_idx = s.rfind('.').unwrap();
        assert_error!(s, LexError::UnfinishedDotAccess(Loc::Point(err_idx)));
    }

    #[test]
    fn test_error_unfinished_dot_access_at_eof() {
        let s = ";top .";
        let err_idx = s.len() - 1;
        assert_error!(s, LexError::UnfinishedDotAccess(Loc::Point(err_idx)));
    }

    #[test]
    fn test_with_one_pipe_no_expr() {
        assert_tokens!(
            ";top >> count",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Count,
        );
    }

    #[test]
    fn test_one_pipe_with_expr() {
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
    fn test_multiple_pipes_with_expr() {
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
    fn test_subcommand() {
        assert_tokens!(
            ";tops (Slay, Lotragon) ++ top mrekk --server akatsuki >> sort .bpm",
            Token::Semicolon,
            Token::Ident("tops".to_string()),
            Token::LeftParen,
            Token::Ident("Slay".to_string()),
            Token::Comma,
            Token::Ident("Lotragon".to_string()),
            Token::RightParen,
            Token::AddAdd,
            Token::Ident("top".to_string()),
            Token::Ident("mrekk".to_string()),
            Token::SubSub,
            Token::Ident("server".to_string()),
            Token::Ident("akatsuki".to_string()),
            Token::GtGt,
            Token::Sort,
            Token::Dot,
            Token::Ident("bpm".to_string()),
        );
    }

    #[test]
    fn test_logic() {
        assert_tokens!(
            ";top >> filter (.bpm >= 230 and HD in .mods) or .bpm >= 250",
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
    fn test_keyword_it() {
        assert_tokens!(
            ";top >> filter it > 0",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::It,
            Token::Gt,
            Token::Int("0".to_string()),
        );
    }

    #[test]
    fn test_keyword_self_alias() {
        assert_tokens!(
            ";top >> filter self > 0",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::It,
            Token::Gt,
            Token::Int("0".to_string()),
        );
    }

    #[test]
    fn test_keyword_booleans() {
        assert_tokens!(
            ";top true false",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Bool(true),
            Token::Bool(false),
        );
    }

    #[test]
    fn test_keyword_not() {
        assert_tokens!(
            ";top >> filter not .fc",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Not,
            Token::Dot,
            Token::Ident("fc".to_string()),
        );
    }

    #[test]
    fn test_operator_bang_not() {
        assert_tokens!(
            ";top >> filter !.fc",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Not,
            Token::Dot,
            Token::Ident("fc".to_string()),
        );
    }

    #[test]
    fn test_keyword_contains() {
        assert_tokens!(
            ";top >> filter .title contains loved",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("title".to_string()),
            Token::Contains,
            Token::Ident("loved".to_string()),
        );
    }

    #[test]
    fn test_keyword_take() {
        assert_tokens!(
            ";top >> take 5",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Take,
            Token::Int("5".to_string()),
        );
    }

    #[test]
    fn test_keyword_with_alias() {
        assert_tokens!(
            ";top with recent",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::AddAdd,
            Token::Ident("recent".to_string()),
        );
    }

    #[test]
    fn test_keyword_is_alias() {
        assert_tokens!(
            ";top >> filter .status is active",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("status".to_string()),
            Token::Eq,
            Token::Ident("active".to_string()),
        );
    }

    #[test]
    fn test_keyword_above_alias() {
        assert_tokens!(
            ";top >> filter .bpm above 200",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("bpm".to_string()),
            Token::Gt,
            Token::Int("200".to_string()),
        );
    }

    #[test]
    fn test_keyword_atleast_alias() {
        assert_tokens!(
            ";top >> filter .bpm atleast 200",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("bpm".to_string()),
            Token::Ge,
            Token::Int("200".to_string()),
        );
    }

    #[test]
    fn test_keyword_below_alias() {
        assert_tokens!(
            ";top >> filter .bpm below 300",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("bpm".to_string()),
            Token::Lt,
            Token::Int("300".to_string()),
        );
    }

    #[test]
    fn test_keyword_atmost_alias() {
        assert_tokens!(
            ";top >> filter .bpm atmost 300",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("bpm".to_string()),
            Token::Le,
            Token::Int("300".to_string()),
        );
    }

    #[test]
    fn test_operator_mul_mod() {
        assert_tokens!(
            ";top 3 * 4 % 2",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Int("3".to_string()),
            Token::Mul,
            Token::Int("4".to_string()),
            Token::Mod,
            Token::Int("2".to_string()),
        );
    }

    #[test]
    fn test_operator_div() {
        assert_tokens!(
            ";top 10 / 2",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Int("10".to_string()),
            Token::Div,
            Token::Int("2".to_string()),
        );
    }

    #[test]
    fn test_operator_divdiv() {
        assert_tokens!(
            ";top 10 // 3",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Int("10".to_string()),
            Token::DivDiv,
            Token::Int("3".to_string()),
        );
    }

    #[test]
    fn test_operator_ne() {
        assert_tokens!(
            ";top >> filter .rank != 1",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("rank".to_string()),
            Token::Ne,
            Token::Int("1".to_string()),
        );
    }

    #[test]
    fn test_operator_lt_le() {
        assert_tokens!(
            ";top >> filter .score < 100",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("score".to_string()),
            Token::Lt,
            Token::Int("100".to_string()),
        );
        assert_tokens!(
            ";top >> filter .score <= 100",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::GtGt,
            Token::Filter,
            Token::Dot,
            Token::Ident("score".to_string()),
            Token::Le,
            Token::Int("100".to_string()),
        );
    }

    #[test]
    fn test_value_negative_float() {
        assert_tokens!(
            ";top -3.14",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Float("-3.14".to_string()),
        );
    }

    #[test]
    fn test_value_quoted_string_with_escape() {
        assert_tokens!(
            r#";top "say \"hi\"""#,
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Str(r#"say \"hi\""#.to_string()),
        );
    }

    #[test]
    fn test_error_bad_string_escape() {
        assert_error!(r#";top "\z""#, LexError::BadStringEscape(Loc::Slice(6, 7)));
    }

    #[test]
    fn test_error_unrecognized_token() {
        assert_error!(
            ";top @value",
            LexError::UnrecognizedToken(Loc::Point(5), '@')
        );
    }

    #[test]
    fn test_empty_string_literal() {
        assert_tokens!(
            r#";top """#,
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Str("".to_string()),
        );
    }

    #[test]
    fn test_string_escape_newline() {
        assert_tokens!(
            r#";top "\n""#,
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Str("\\n".to_string()),
        );
    }

    #[test]
    fn test_string_escape_tab() {
        assert_tokens!(
            r#";top "\t""#,
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Str("\\t".to_string()),
        );
    }

    #[test]
    fn test_string_escape_backslash() {
        assert_tokens!(
            r#";top "\\""#,
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Str("\\\\".to_string()),
        );
    }

    #[test]
    fn test_number_zero() {
        assert_tokens!(
            ";top 0",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Int("0".to_string()),
        );
    }

    #[test]
    fn test_number_with_underscore() {
        assert_tokens!(
            ";top 1_000",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Int("1000".to_string()),
        );
    }

    #[test]
    fn test_operator_ge_symbol() {
        assert_tokens!(
            ";top >= 5",
            Token::Semicolon,
            Token::Ident("top".to_string()),
            Token::Ge,
            Token::Int("5".to_string()),
        );
    }

    #[test]
    fn test_error_multiple_unrecognized_tokens() {
        assert_error!(
            ";top @ # value",
            LexError::UnrecognizedToken(Loc::Point(5), '@'),
            LexError::UnrecognizedToken(Loc::Point(7), '#'),
        );
    }
}
