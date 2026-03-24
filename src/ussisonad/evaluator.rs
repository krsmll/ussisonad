use crate::ParserError;
use crate::ussisonad::lex;
use crate::ussisonad::model::{CommandInput, EvalError, Registry, Value};
use crate::ussisonad::parse::ast;
use crate::ussisonad::parse::parser::Parser;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Evaluator {
    registry: Arc<Registry>,
}

impl Evaluator {
    #[must_use]
    pub fn new(registry: Arc<Registry>) -> Self {
        Self { registry }
    }

    pub async fn execute(&self, src_input: &str) -> Result<Value, EvalError> {
        let tokenizer = lex::make_tokenizer(src_input);
        let ast = Parser::parse(tokenizer).map_err(|err| match err {
            ParserError::LexerStage(lex_errs) => EvalError::LexerStage(lex_errs),
            parse_errs => EvalError::ParsingStage(parse_errs),
        })?;

        self.evaluate_ast(&ast).await
    }

    pub async fn evaluate_ast(&self, node: &ast::PipelineNode) -> Result<Value, EvalError> {
        self.eval_node(node, Value::None).await
    }

    async fn eval_node(&self, node: &ast::PipelineNode, input: Value) -> Result<Value, EvalError> {
        match node {
            ast::PipelineNode::Command(cmd) => self.eval_command(cmd, input).await,

            ast::PipelineNode::Pipe { lhs, rhs } => {
                let lhs = Box::pin(self.eval_node(lhs, input)).await?;
                Box::pin(self.eval_node(rhs, lhs)).await
            }

            ast::PipelineNode::Concat { lhs, rhs } => {
                let lhs = Box::pin(self.eval_node(lhs, input.clone())).await?;
                let rhs = Box::pin(self.eval_node(rhs, input)).await?;
                Ok(Self::concat_values(lhs, rhs))
            }
        }
    }

    async fn eval_command(&self, cmd: &ast::Command, input: Value) -> Result<Value, EvalError> {
        match cmd {
            ast::Command::Builtin(builtin) => self.eval_builtin(builtin, input),
            ast::Command::Custom(custom) => self.eval_custom(custom, input).await,
        }
    }

    fn eval_builtin(&self, cmd: &ast::BuiltinCommand, input: Value) -> Result<Value, EvalError> {
        match cmd {
            ast::BuiltinCommand::Filter(expr) => self.eval_filter(input, expr),
            ast::BuiltinCommand::Unique(field) => self.eval_unique(input, field.as_ref()),
            ast::BuiltinCommand::Map(expr) => self.eval_map(input, expr),

            ast::BuiltinCommand::Sort { field, direction } => {
                self.eval_sort(input, field, *direction)
            }

            ast::BuiltinCommand::Limit(n) => {
                let items = input.into_vector()?;
                Ok(Value::Vector(items.into_iter().take(*n as usize).collect()))
            }

            ast::BuiltinCommand::Count => {
                let items = input.into_vector()?;
                Ok(Value::Int(items.len() as i64))
            }
        }
    }

    fn eval_map(&self, input: Value, expr: &ast::Expr) -> Result<Value, EvalError> {
        let items = input.into_vector()?;
        let mapped = items
            .into_iter()
            .map(|item| self.eval_expr(expr, &item))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::Vector(mapped))
    }

    fn eval_unique(&self, input: Value, field: Option<&ast::Expr>) -> Result<Value, EvalError> {
        let items = input.into_vector()?;
        let mut seen: Vec<Value> = Vec::new();
        let mut result = Vec::new();
        for item in items {
            let key = match field {
                Some(expr) => self.eval_expr(expr, &item)?,
                None => item.clone(),
            };

            if !seen.contains(&key) {
                seen.push(key);
                result.push(item);
            }
        }
        Ok(Value::Vector(result))
    }

    fn eval_filter(&self, input: Value, expr: &ast::Expr) -> Result<Value, EvalError> {
        let items = input.into_vector()?;
        let filtered = items
            .into_iter()
            .filter_map(|item| match self.eval_expr(expr, &item) {
                Ok(Value::Bool(true)) => Some(Ok(item)),
                Ok(Value::Bool(false) | Value::None) => None,
                Ok(other) => Some(Err(EvalError::TypeMismatch {
                    expected: "boolean",
                    got: other.type_name(),
                })),
                Err(e) => Some(Err(e)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::Vector(filtered))
    }

    fn eval_sort(
        &self,
        input: Value,
        field: &ast::Expr,
        direction: ast::SortDirection,
    ) -> Result<Value, EvalError> {
        let items = input.into_vector()?;
        let mut keyed: Vec<(Value, Value)> = items
            .into_iter()
            .map(|item| Ok((self.eval_expr(field, &item)?, item)))
            .collect::<Result<_, _>>()?;

        let mut lazy_error = None;
        keyed.sort_by(|(a, _), (b, _)| {
            if lazy_error.is_some() {
                return std::cmp::Ordering::Equal;
            }

            let ord = Self::compare_values(a, b).unwrap_or_else(|e| {
                lazy_error = Some(e);
                std::cmp::Ordering::Equal
            });

            match direction {
                ast::SortDirection::Asc => ord,
                ast::SortDirection::Desc => ord.reverse(),
            }
        });

        if let Some(err) = lazy_error {
            return Err(err);
        }

        Ok(Value::Vector(
            keyed.into_iter().map(|(_, item)| item).collect(),
        ))
    }

    async fn eval_custom(
        &self,
        cmd: &ast::CustomCommand,
        input: Value,
    ) -> Result<Value, EvalError> {
        let def = self
            .registry
            .get_command(&cmd.name)
            .ok_or_else(|| EvalError::UnknownCommand(cmd.name.clone()))?;

        if !def.depends_on.is_empty() && !def.depends_on.iter().any(|t| t.matches(&input)) {
            return Err(EvalError::UnexpectedInputType {
                command: cmd.name.clone(),
                expected: def.depends_on.clone(),
                got: input.type_name(),
            });
        }

        let arg = match &cmd.arg {
            None => Value::None,
            Some(arg) => self.eval_expr(arg, &input)?,
        };

        if let Some(arg_schema) = &def.arg
            && arg_schema.required
            && !arg_schema.accepts.is_empty()
            && !arg_schema.accepts.iter().any(|t| t.matches(&arg))
        {
            return Err(EvalError::UnexpectedArgumentType {
                command: cmd.name.clone(),
                expected: arg_schema.accepts.clone(),
                got: arg.type_name(),
            });
        }

        let options = cmd
            .options
            .iter()
            .map(|(k, expr)| Ok((k.clone(), self.eval_expr(expr, &input)?)))
            .collect::<Result<HashMap<_, _>, EvalError>>()?;

        let command_input = CommandInput {
            arg,
            flags: cmd.flags.clone(),
            options,
        };

        let result = def
            .handler
            .execute(input, command_input)
            .await
            .map_err(EvalError::Handler)?;

        if !def.returns.matches(&result) {
            return Err(EvalError::UnexpectedReturnType {
                command: cmd.name.clone(),
                expected: def.returns.clone(),
                got: result.type_name(),
            });
        }

        Ok(result)
    }

    fn eval_expr(&self, expr: &ast::Expr, context: &Value) -> Result<Value, EvalError> {
        match expr {
            ast::Expr::It => Ok(context.clone()),
            ast::Expr::Bool(b) => Ok(Value::Bool(*b)),
            ast::Expr::Int(n) => Ok(Value::Int(*n)),
            ast::Expr::Float(f) => Ok(Value::Float(*f)),
            ast::Expr::Str(s) => Ok(Value::Str(s.clone())),

            ast::Expr::Vector(items) => {
                let vals = items
                    .iter()
                    .map(|e| self.eval_expr(e, context))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Vector(vals))
            }

            ast::Expr::FieldPath(segments) => {
                segments
                    .iter()
                    .try_fold(context.clone(), |val, seg| match val {
                        Value::Object(map) => map
                            .get(seg)
                            .cloned()
                            .ok_or_else(|| EvalError::UnknownField(seg.clone())),
                        _ => Err(EvalError::TypeMismatch {
                            expected: "object",
                            got: val.type_name(),
                        }),
                    })
            }

            ast::Expr::Not(inner) => match self.eval_expr(inner, context)? {
                Value::Bool(b) => Ok(Value::Bool(!b)),
                other => Err(EvalError::TypeMismatch {
                    expected: "boolean",
                    got: other.type_name(),
                }),
            },

            ast::Expr::Binary { lhs, op, rhs, .. } => {
                match op {
                    ast::BinOp::And => {
                        let l = self.eval_expr(lhs, context)?;
                        return match l {
                            Value::Bool(true) => self.eval_expr(rhs, context),
                            Value::Bool(false) => Ok(Value::Bool(false)),
                            _ => Err(EvalError::TypeMismatch {
                                expected: "boolean",
                                got: l.type_name(),
                            }),
                        };
                    }

                    ast::BinOp::Or => {
                        let l = self.eval_expr(lhs, context)?;
                        return match l {
                            Value::Bool(true) => Ok(Value::Bool(true)),
                            Value::Bool(false) => self.eval_expr(rhs, context),
                            _ => Err(EvalError::TypeMismatch {
                                expected: "boolean",
                                got: l.type_name(),
                            }),
                        };
                    }

                    _ => {}
                }

                let l = self.eval_expr(lhs, context)?;
                let r = self.eval_expr(rhs, context)?;

                Self::eval_binary(op, l, r)
            }
        }
    }

    fn eval_binary(op: &ast::BinOp, lhs: Value, rhs: Value) -> Result<Value, EvalError> {
        match op {
            // arithmetic
            ast::BinOp::Add => Self::numeric_op(lhs, rhs, |a, b| a + b, |a, b| a + b),
            ast::BinOp::Sub => Self::numeric_op(lhs, rhs, |a, b| a - b, |a, b| a - b),
            ast::BinOp::Mul => Self::numeric_op(lhs, rhs, |a, b| a * b, |a, b| a * b),
            ast::BinOp::Div => Self::numeric_op(lhs, rhs, |a, b| a / b, |a, b| a / b),
            ast::BinOp::Mod => Self::numeric_op(lhs, rhs, |a, b| a % b, |a, b| a % b),
            ast::BinOp::DivDiv => {
                Self::numeric_op(lhs, rhs, |a, b| a / b, |a, b| (a as i64 / b as i64) as f64)
            }

            // comparison
            ast::BinOp::Eq => Ok(Value::Bool(lhs == rhs)),
            ast::BinOp::Ne => Ok(Value::Bool(lhs != rhs)),
            ast::BinOp::Gt => Self::compare_values(&lhs, &rhs).map(|o| Value::Bool(o.is_gt())),
            ast::BinOp::Lt => Self::compare_values(&lhs, &rhs).map(|o| Value::Bool(o.is_lt())),
            ast::BinOp::Ge => Self::compare_values(&lhs, &rhs).map(|o| Value::Bool(o.is_ge())),
            ast::BinOp::Le => Self::compare_values(&lhs, &rhs).map(|o| Value::Bool(o.is_le())),

            // membership
            ast::BinOp::Contains => match &lhs {
                Value::Vector(items) => Ok(Value::Bool(items.contains(&rhs))),

                Value::Str(s) => match &rhs {
                    Value::Str(sub) => Ok(Value::Bool(s.contains(sub))),
                    _ => Err(EvalError::TypeMismatch {
                        expected: "string",
                        got: rhs.type_name(),
                    }),
                },

                _ => Err(EvalError::TypeMismatch {
                    expected: "list or string",
                    got: rhs.type_name(),
                }),
            },

            ast::BinOp::In => match &rhs {
                Value::Vector(items) => Ok(Value::Bool(items.contains(&lhs))),

                Value::Str(s) => match &lhs {
                    Value::Str(sub) => Ok(Value::Bool(s.contains(sub.as_str()))),
                    _ => Err(EvalError::TypeMismatch {
                        expected: "string",
                        got: lhs.type_name(),
                    }),
                },

                _ => Err(EvalError::TypeMismatch {
                    expected: "list or string",
                    got: rhs.type_name(),
                }),
            },

            ast::BinOp::And | ast::BinOp::Or => unreachable!(),
        }
    }

    fn numeric_op(
        lhs: Value,
        rhs: Value,
        int_op: impl Fn(i64, i64) -> i64,
        float_op: impl Fn(f64, f64) -> f64,
    ) -> Result<Value, EvalError> {
        match (lhs, rhs) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(int_op(a, b))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(float_op(a, b))),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(float_op(a as f64, b))),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(float_op(a, b as f64))),
            (l, r) => Err(EvalError::NumberTypeMismatch(vec![
                l.type_name(),
                r.type_name(),
            ])),
        }
    }

    fn compare_values(a: &Value, b: &Value) -> Result<std::cmp::Ordering, EvalError> {
        match (a, b) {
            (Value::Int(a), Value::Int(b)) => Ok(a.cmp(b)),

            (Value::Float(a), Value::Float(b)) => a
                .partial_cmp(b)
                .ok_or(EvalError::NotComparable("decimal", "decimal")),

            (Value::Int(a), Value::Float(b)) => (*a as f64)
                .partial_cmp(b)
                .ok_or(EvalError::NotComparable("integer", "decimal")),

            (Value::Float(a), Value::Int(b)) => a
                .partial_cmp(&(*b as f64))
                .ok_or(EvalError::NotComparable("decimal", "integer")),

            (Value::Str(a), Value::Str(b)) => Ok(a.cmp(b)),
            _ => Err(EvalError::NotComparable(a.type_name(), b.type_name())),
        }
    }

    fn concat_values(a: Value, b: Value) -> Value {
        match (a, b) {
            (Value::Vector(mut a), Value::Vector(b)) => {
                a.extend(b);
                Value::Vector(a)
            }

            (Value::Vector(mut a), b) => {
                a.push(b);
                Value::Vector(a)
            }

            (a, Value::Vector(mut b)) => {
                b.insert(0, a);
                Value::Vector(b)
            }

            (a, b) => Value::Vector(vec![a, b]),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ussisonad::evaluator::Evaluator;
    use crate::ussisonad::model::{
        ArgSchema, CommandDefinition, CommandError, CommandHandler, CommandInput, ConfigError,
        EvalError, FieldSchema, ObjectSchema, Registry, Value, ValueType,
    };
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio_test::assert_ok;

    macro_rules! assert_expr {
        ($src:expr, $expected:expr) => {
            let registry = assert_ok!(create_registry());
            let evaluator = Evaluator::new(Arc::new(registry));
            let result = evaluator.execute($src).await.unwrap();

            assert_eq!(result, $expected);
        };
    }

    macro_rules! assert_expr_err {
        ($src:expr, $pattern:pat) => {
            let registry = assert_ok!(create_registry());
            let evaluator = Evaluator::new(Arc::new(registry));
            let result = evaluator.execute($src).await;
            assert!(
                matches!(result, Err($pattern)),
                "expected error, got: {:?}",
                result
            );
        };
    }

    struct RangeHandler;

    #[async_trait]
    impl CommandHandler for RangeHandler {
        async fn execute(
            &self,
            _context: Value,
            input: CommandInput,
        ) -> Result<Value, CommandError> {
            match input.arg {
                Value::Int(n) if n < 0 => Err(CommandError::InvalidArgument(format!(
                    "n must be non-negative, got {n}"
                ))),

                Value::Int(n) => Ok(Value::Vector((0..=n).map(Value::Int).collect())),

                other => Err(CommandError::TypeMismatch {
                    expected: vec![ValueType::Int],
                    got: other.type_name(),
                }),
            }
        }
    }

    struct MultiplyEachHandler;

    #[async_trait]
    impl CommandHandler for MultiplyEachHandler {
        async fn execute(
            &self,
            context: Value,
            input: CommandInput,
        ) -> Result<Value, CommandError> {
            let factor = input.arg;
            let factor = match factor {
                Value::Int(n) => n,
                other => {
                    return Err(CommandError::TypeMismatch {
                        expected: vec![ValueType::Int],
                        got: other.type_name(),
                    });
                }
            };

            match context {
                Value::None => Ok(Value::None),
                Value::Int(n) => Ok(Value::Int(n * factor)),

                Value::Vector(items) => {
                    let result = items
                        .iter()
                        .map(|item| match item {
                            Value::Int(n) => Ok(Value::Int(n * factor)),
                            other => Err(CommandError::TypeMismatch {
                                expected: vec![ValueType::Int],
                                got: other.type_name(),
                            }),
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(Value::Vector(result))
                }

                other => Err(CommandError::TypeMismatch {
                    expected: vec![ValueType::Int, ValueType::Vector(Box::new(ValueType::Int))],
                    got: other.type_name(),
                }),
            }
        }
    }

    struct ItemsHandler;

    #[async_trait]
    impl CommandHandler for ItemsHandler {
        async fn execute(
            &self,
            _context: Value,
            _input: CommandInput,
        ) -> Result<Value, CommandError> {
            Ok(Value::Vector(vec![
                Value::Object(HashMap::from([
                    ("value".to_string(), Value::Int(1)),
                    ("tag".to_string(), Value::Str("a".to_string())),
                ])),
                Value::Object(HashMap::from([
                    ("value".to_string(), Value::Int(2)),
                    ("tag".to_string(), Value::Str("b".to_string())),
                ])),
                Value::Object(HashMap::from([
                    ("value".to_string(), Value::Int(3)),
                    ("tag".to_string(), Value::Str("a".to_string())),
                ])),
            ]))
        }
    }

    struct MultiplyHandler;

    #[async_trait]
    impl CommandHandler for MultiplyHandler {
        async fn execute(
            &self,
            context: Value,
            _input: CommandInput,
        ) -> Result<Value, CommandError> {
            match context {
                Value::Vector(items) => {
                    let result = items
                        .iter()
                        .map(|item| match item {
                            Value::Int(n) => *n,
                            _ => unreachable!(),
                        })
                        .reduce(|acc, n| acc * n)
                        .unwrap_or(0);
                    Ok(Value::Int(result))
                }

                other => Err(CommandError::TypeMismatch {
                    expected: vec![ValueType::Vector(Box::new(ValueType::Int))],
                    got: other.type_name(),
                }),
            }
        }
    }

    struct GreetHandler;

    impl GreetHandler {
        const DEFAULT_TARGET: &'static str = "buddy";

        fn target_name(input: &CommandInput) -> String {
            match &input.arg {
                Value::Str(s) => s.clone(),
                _ => Self::DEFAULT_TARGET.to_string(),
            }
        }

        fn validate_flags(input: &CommandInput) -> Result<(), CommandError> {
            let upper = input.has_flag("upper");
            let lower = input.has_flag("lower");

            if upper && lower {
                return Err(CommandError::FlagConflict(vec!["upper", "lower"]));
            }

            Ok(())
        }

        fn format_silly(target_name: &str, lower: bool) -> Value {
            if lower {
                Value::Str(format!(
                    "｡･ﾟﾟ*(>д<)*ﾟﾟ･｡ hallo... {}... ｡･ﾟﾟ*(>д<)*ﾟﾟ･｡",
                    target_name.to_lowercase()
                ))
            } else {
                Value::Str(format!(
                    "☆*:.｡.o(≧▽≦)o.｡.:*☆ HALLO {}!! ☆*:.｡.o(≧▽≦)o.｡.:*☆",
                    target_name
                ))
            }
        }

        fn format_standard(target_name: &str, upper: bool, lower: bool) -> Value {
            if upper {
                Value::Str(format!("HELLO, {}!", target_name.to_uppercase()))
            } else if lower {
                Value::Str(format!("hello, {}!", target_name.to_lowercase()))
            } else {
                Value::Str(format!("Hello, {}!", target_name))
            }
        }
    }

    #[async_trait]
    impl CommandHandler for GreetHandler {
        async fn execute(
            &self,
            _context: Value,
            input: CommandInput,
        ) -> Result<Value, CommandError> {
            Self::validate_flags(&input)?;
            let target_name = Self::target_name(&input);
            let silly = input.has_flag("silly");
            let uppercase = input.has_flag("upper");
            let lowercase = input.has_flag("lower");

            let value = if silly {
                Self::format_silly(&target_name, lowercase)
            } else {
                Self::format_standard(&target_name, uppercase, lowercase)
            };

            Ok(value)
        }
    }

    fn create_registry() -> Result<Registry, ConfigError> {
        let item_schema = ObjectSchema::builder()
            .name("item")
            .field(
                FieldSchema::builder()
                    .name("value")
                    .value_type(ValueType::Int),
            )
            .field(
                FieldSchema::builder()
                    .name("tag")
                    .value_type(ValueType::Str),
            )
            .build()?;

        Registry::builder()
            .register(
                CommandDefinition::builder()
                    .name("items")
                    .returns(ValueType::Vector(Box::new(ValueType::Object(Box::new(
                        item_schema,
                    )))))
                    .handler(ItemsHandler),
            )
            .register(
                CommandDefinition::builder()
                    .name("range")
                    .alias("r")
                    .arg(
                        ArgSchema::builder()
                            .name("n")
                            .accepts(ValueType::Int)
                            .required(),
                    )
                    .returns(ValueType::Vector(Box::new(ValueType::Int)))
                    .handler(RangeHandler),
            )
            .register(
                CommandDefinition::builder()
                    .name("multiply")
                    .alias("mul")
                    .depends_on(ValueType::Vector(Box::new(ValueType::Int)))
                    .returns(ValueType::Int)
                    .handler(MultiplyHandler),
            )
            .register(
                CommandDefinition::builder()
                    .name("multiply_each")
                    .alias("mul_e")
                    .arg(
                        ArgSchema::builder()
                            .name("factor")
                            .accepts(ValueType::Int)
                            .required(),
                    )
                    .depends_on(ValueType::Vector(Box::new(ValueType::Int)))
                    .returns(ValueType::Vector(Box::new(ValueType::Int)))
                    .handler(MultiplyEachHandler),
            )
            .register(
                CommandDefinition::builder()
                    .name("greet")
                    .arg(ArgSchema::builder().name("name").accepts(ValueType::Str))
                    .flag("upper")
                    .flag("lower")
                    .flag("silly")
                    .returns(ValueType::Str)
                    .handler(GreetHandler),
            )
            .build()
    }

    #[tokio::test]
    async fn test_range() {
        assert_expr!(
            ";range 5",
            Value::Vector(vec![
                Value::Int(0),
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
                Value::Int(4),
                Value::Int(5)
            ])
        );
    }

    #[tokio::test]
    async fn test_range_alias() {
        assert_expr!(
            ";r 5",
            Value::Vector(vec![
                Value::Int(0),
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
                Value::Int(4),
                Value::Int(5)
            ])
        );
    }

    #[tokio::test]
    async fn test_range_with_filter() {
        assert_expr!(
            ";range 5 >> filter it > 3",
            Value::Vector(vec![Value::Int(4), Value::Int(5)])
        );
    }

    #[tokio::test]
    async fn test_range_with_sort() {
        assert_expr!(
            ";range 2 >> sort it",
            Value::Vector(vec![Value::Int(2), Value::Int(1), Value::Int(0)])
        );

        assert_expr!(
            ";range 2 >> sort",
            Value::Vector(vec![Value::Int(2), Value::Int(1), Value::Int(0)])
        );

        assert_expr!(
            ";range 2 >> sort --asc",
            Value::Vector(vec![Value::Int(0), Value::Int(1), Value::Int(2)])
        );
    }

    #[tokio::test]
    async fn test_range_into_mul() {
        assert_expr!(";range 5 >> filter it > 0 >> mul", Value::Int(120));
    }

    #[tokio::test]
    async fn test_range_into_mul_each() {
        assert_expr!(
            ";range 5 >> multiply_each 5",
            Value::Vector(vec![
                Value::Int(0),
                Value::Int(5),
                Value::Int(10),
                Value::Int(15),
                Value::Int(20),
                Value::Int(25)
            ],)
        );
    }

    #[tokio::test]
    async fn test_count() {
        assert_expr!(";range 3 >> count", Value::Int(4));
    }

    #[tokio::test]
    async fn test_limit() {
        assert_expr!(
            ";range 10 >> take 3",
            Value::Vector(vec![Value::Int(0), Value::Int(1), Value::Int(2)])
        );
    }

    #[tokio::test]
    async fn test_concat() {
        assert_expr!(
            ";range 1 ++ range 1",
            Value::Vector(vec![
                Value::Int(0),
                Value::Int(1),
                Value::Int(0),
                Value::Int(1),
            ])
        );
    }

    #[tokio::test]
    async fn test_filter_eq() {
        assert_expr!(
            ";range 5 >> filter it = 3",
            Value::Vector(vec![Value::Int(3)])
        );
    }

    #[tokio::test]
    async fn test_filter_ne() {
        assert_expr!(
            ";range 3 >> filter it != 1",
            Value::Vector(vec![Value::Int(0), Value::Int(2), Value::Int(3)])
        );
    }

    #[tokio::test]
    async fn test_filter_ge() {
        assert_expr!(
            ";range 3 >> filter it >= 2",
            Value::Vector(vec![Value::Int(2), Value::Int(3)])
        );
    }

    #[tokio::test]
    async fn test_filter_le() {
        assert_expr!(
            ";range 3 >> filter it <= 1",
            Value::Vector(vec![Value::Int(0), Value::Int(1)])
        );
    }

    #[tokio::test]
    async fn test_filter_and() {
        assert_expr!(
            ";range 5 >> filter it > 1 and it < 4",
            Value::Vector(vec![Value::Int(2), Value::Int(3)])
        );
    }

    #[tokio::test]
    async fn test_filter_or() {
        assert_expr!(
            ";range 5 >> filter it < 2 or it > 3",
            Value::Vector(vec![
                Value::Int(0),
                Value::Int(1),
                Value::Int(4),
                Value::Int(5),
            ])
        );
    }

    #[tokio::test]
    async fn test_filter_not() {
        assert_expr!(
            ";range 3 >> filter not (it > 1)",
            Value::Vector(vec![Value::Int(0), Value::Int(1)])
        );
    }

    #[tokio::test]
    async fn test_filter_mul_arithmetic() {
        assert_expr!(
            ";range 5 >> filter it * 2 > 6",
            Value::Vector(vec![Value::Int(4), Value::Int(5)])
        );
    }

    #[tokio::test]
    async fn test_filter_mod_arithmetic() {
        assert_expr!(
            ";range 5 >> filter it % 2 = 0",
            Value::Vector(vec![Value::Int(0), Value::Int(2), Value::Int(4)])
        );
    }

    #[tokio::test]
    async fn test_filter_in_vector() {
        assert_expr!(
            ";range 5 >> filter it in (1, 3)",
            Value::Vector(vec![Value::Int(1), Value::Int(3)])
        );
    }

    #[tokio::test]
    async fn test_filter_by_field() {
        let item2 = Value::Object(HashMap::from([
            ("value".to_string(), Value::Int(2)),
            ("tag".to_string(), Value::Str("b".to_string())),
        ]));
        let item3 = Value::Object(HashMap::from([
            ("value".to_string(), Value::Int(3)),
            ("tag".to_string(), Value::Str("a".to_string())),
        ]));
        assert_expr!(
            ";items >> filter .value > 1",
            Value::Vector(vec![item2, item3])
        );
    }

    #[tokio::test]
    async fn test_sort_by_field_asc() {
        assert_expr!(
            ";items >> sort .value --asc",
            Value::Vector(vec![
                Value::Object(HashMap::from([
                    ("value".to_string(), Value::Int(1)),
                    ("tag".to_string(), Value::Str("a".to_string())),
                ])),
                Value::Object(HashMap::from([
                    ("value".to_string(), Value::Int(2)),
                    ("tag".to_string(), Value::Str("b".to_string())),
                ])),
                Value::Object(HashMap::from([
                    ("value".to_string(), Value::Int(3)),
                    ("tag".to_string(), Value::Str("a".to_string())),
                ]))
            ])
        );
    }

    #[tokio::test]
    async fn test_filter_contains_string() {
        assert_expr!(
            ";items >> filter .tag contains a",
            Value::Vector(vec![
                Value::Object(HashMap::from([
                    ("value".to_string(), Value::Int(1)),
                    ("tag".to_string(), Value::Str("a".to_string())),
                ])),
                Value::Object(HashMap::from([
                    ("value".to_string(), Value::Int(3)),
                    ("tag".to_string(), Value::Str("a".to_string())),
                ]))
            ])
        );
    }

    #[tokio::test]
    async fn test_map_it() {
        assert_expr!(
            ";range 3 >> map it",
            Value::Vector(vec![
                Value::Int(0),
                Value::Int(1),
                Value::Int(2),
                Value::Int(3)
            ])
        );
    }

    #[tokio::test]
    async fn test_map_arithmetic() {
        assert_expr!(
            ";range 3 >> map it * 2",
            Value::Vector(vec![
                Value::Int(0),
                Value::Int(2),
                Value::Int(4),
                Value::Int(6)
            ])
        );
    }

    #[tokio::test]
    async fn test_map_field() {
        assert_expr!(
            ";items >> map .value",
            Value::Vector(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
        );
    }

    #[tokio::test]
    async fn test_map_chained_with_filter() {
        assert_expr!(
            ";range 5 >> filter it > 2 >> map it * 3",
            Value::Vector(vec![Value::Int(9), Value::Int(12), Value::Int(15)])
        );
    }

    #[tokio::test]
    async fn test_unknown_command_error() {
        assert_expr_err!(";unknown_cmd", EvalError::UnknownCommand(_));
    }

    #[tokio::test]
    async fn test_wrong_input_type_error() {
        assert_expr_err!(";multiply", EvalError::UnexpectedInputType { .. });
    }

    #[tokio::test]
    async fn test_filter_non_bool_type_mismatch() {
        assert_expr_err!(
            ";range 3 >> filter it + 1",
            EvalError::TypeMismatch {
                expected: "boolean",
                ..
            }
        );
    }

    #[tokio::test]
    async fn test_unknown_field_error() {
        assert_expr_err!(
            ";items >> filter .nonexistent > 1",
            EvalError::UnknownField(_)
        );
    }

    #[tokio::test]
    async fn test_filter_sub_arithmetic() {
        assert_expr!(
            ";range 5 >> filter it - 1 > 3",
            Value::Vector(vec![Value::Int(5)])
        );
    }

    #[tokio::test]
    async fn test_filter_div_arithmetic() {
        assert_expr!(
            ";range 5 >> filter it / 2 > 1",
            Value::Vector(vec![Value::Int(4), Value::Int(5)])
        );
    }

    #[tokio::test]
    async fn test_map_divdiv_floor_division() {
        assert_expr!(
            ";range 5 >> map it // 2",
            Value::Vector(vec![
                Value::Int(0),
                Value::Int(0),
                Value::Int(1),
                Value::Int(1),
                Value::Int(2),
                Value::Int(2),
            ])
        );
    }

    #[tokio::test]
    async fn test_unique_no_field() {
        assert_expr!(
            ";range 2 ++ range 2 >> unique",
            Value::Vector(vec![Value::Int(0), Value::Int(1), Value::Int(2)])
        );
    }

    #[tokio::test]
    async fn test_unique_with_field() {
        assert_expr!(
            ";items >> unique .tag",
            Value::Vector(vec![
                Value::Object(HashMap::from([
                    ("value".to_string(), Value::Int(1)),
                    ("tag".to_string(), Value::Str("a".to_string())),
                ])),
                Value::Object(HashMap::from([
                    ("value".to_string(), Value::Int(2)),
                    ("tag".to_string(), Value::Str("b".to_string())),
                ])),
            ])
        );
    }

    #[tokio::test]
    async fn test_unique_empty_vector() {
        assert_expr!(
            ";range 0 >> filter it > 100 >> unique",
            Value::Vector(vec![])
        );
    }

    #[tokio::test]
    async fn test_limit_zero() {
        assert_expr!(";range 5 >> take 0", Value::Vector(vec![]));
    }

    #[tokio::test]
    async fn test_limit_exceeds_size() {
        assert_expr!(
            ";range 2 >> take 100",
            Value::Vector(vec![Value::Int(0), Value::Int(1), Value::Int(2)])
        );
    }

    #[tokio::test]
    async fn test_count_empty() {
        assert_expr!(";range 5 >> filter it > 100 >> count", Value::Int(0));
    }

    #[tokio::test]
    async fn test_not_comparable_error() {
        assert_expr_err!(";range 2 >> filter it > ok", EvalError::NotComparable(..));
    }

    #[tokio::test]
    async fn test_unexpected_argument_type_error() {
        assert_expr_err!(
            ";range 3 >> multiply_each hello",
            EvalError::UnexpectedArgumentType { .. }
        );
    }

    #[tokio::test]
    async fn test_flags() {
        assert_expr!(";greet --upper", Value::Str("HELLO, BUDDY!".to_string()));
    }

    #[tokio::test]
    async fn test_args_and_flags_together() {
        assert_expr!(
            ";greet kris --upper",
            Value::Str("HELLO, KRIS!".to_string())
        );
    }

    #[tokio::test]
    async fn test_multiple_flags() {
        assert_expr!(
            ";greet --lower --silly",
            Value::Str("｡･ﾟﾟ*(>д<)*ﾟﾟ･｡ hallo... buddy... ｡･ﾟﾟ*(>д<)*ﾟﾟ･｡".to_string())
        );
    }
}
