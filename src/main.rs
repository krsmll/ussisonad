use crate::ussisonad::lexer;

mod ussisonad;

fn main() {
    lexer::make_tokenizer(";add (1, 3, 7)")
        .enumerate()
        .for_each(|(i, result)| {
        let (tok, _, _) = result.unwrap();
        println!("{}: {:?}", i, tok);
    });
}
