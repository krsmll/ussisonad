# 🐍 Ussisõnad

silly little dsl for my discord bot that will never go live

## Examples:

Simple greeting command:

```rust
struct GreetHandler;

#[async_trait]
impl CommandHandler for GreetHandler {
    async fn execute(&self, _context: Value, input: CommandInput) -> Result<Value, CommandError> {
        let target = match input.arg {
            Value::Str(s) => s,
            _ => "World".to_string(),
        };

        let greeting = format!("Hello, {}!", target);

        Ok(Value::Str(greeting))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = Registry::builder()
        .register(
            CommandDefinition::builder()
                .name("greet")
                .arg(ArgSchema::builder().name("name").accepts(ValueType::Str))
                .returns(ValueType::Str)
                .handler(GreetHandler),
        )
        .build()?;

    let evaluator = Evaluator::new(Arc::new(registry));

    let input = ";greet John";
    let result = evaluator.execute(input).await?;

    let Value::Str(greeting) = result else {
        panic!("Expected greeting to be a string");
    };

    println!("{greeting}");
    Ok(())
}
```

You can find more usage examples in [examples](examples).

## License

Licensed under either of

* [Apache License, Version 2.0](LICENSE-APACHE)
* [MIT license](LICENSE-MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

