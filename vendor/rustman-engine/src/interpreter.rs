//! Tree-walking evaluator.

use std::collections::HashMap;

use crate::ast::{BinOp, Expr, Stmt, UnaryOp};
use crate::builtins;
use crate::value::Value;

/// Read-only inputs a script can observe.
#[derive(Debug, Clone, Default)]
pub struct HostInput {
    pub env_vars: Vec<(String, String)>,
    /// Request headers (pre-request scripts) or response headers (test scripts).
    pub headers: Vec<(String, String)>,
    pub cookies: Vec<(String, String)>,
    /// The outgoing request's body — available in both script slots (a test
    /// script inspecting what was actually sent is a normal debugging need;
    /// use `response.text()`/`response.json()` for what came back instead).
    pub body: String,
    /// The outgoing request's URL — available in both script slots.
    pub url: String,
    /// `None` for pre-request scripts (no response exists yet).
    pub response: Option<ResponseInput>,
}

#[derive(Debug, Clone)]
pub struct ResponseInput {
    pub status: u16,
    pub body: String,
}

/// A mutation a script asked for. The host applies these after the script
/// finishes running — the interpreter itself never touches host state
/// directly, so it stays a pure, easily-testable function of its input.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    SetEnv(String, String),
    SetHeader(String, String),
    /// Replaces the outgoing request body (pre-request scripts only —
    /// produced by `set_body(...)`).
    SetBody(String),
    Test { name: String, passed: bool },
    /// A `print(...)` call, for debugging a script without an assertion.
    Log(String),
}

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub message: String,
}

pub(crate) struct Interpreter {
    vars: HashMap<String, Value>,
    input: HostInput,
    effects: Vec<Effect>,
}

impl Interpreter {
    pub(crate) fn new(input: HostInput) -> Self {
        let mut vars = HashMap::new();
        if let Some(resp) = &input.response {
            vars.insert(
                "response".to_owned(),
                Value::Object(vec![
                    ("status".to_owned(), Value::Number(f64::from(resp.status))),
                    ("__raw_body".to_owned(), Value::String(resp.body.clone())),
                ]),
            );
        }
        Self { vars, input, effects: Vec::new() }
    }

    pub(crate) fn run(&mut self, program: &[Stmt]) -> Result<RunOutcome, RuntimeError> {
        for stmt in program {
            self.exec(stmt)?;
        }
        Ok(RunOutcome { effects: std::mem::take(&mut self.effects) })
    }

    fn exec(&mut self, stmt: &Stmt) -> Result<(), RuntimeError> {
        match stmt {
            Stmt::Let { name, value } => {
                let v = self.eval(value)?;
                self.vars.insert(name.clone(), v);
            }
            Stmt::If { cond, then_branch, else_branch } => {
                if self.eval(cond)?.is_truthy() {
                    for s in then_branch {
                        self.exec(s)?;
                    }
                } else {
                    for s in else_branch {
                        self.exec(s)?;
                    }
                }
            }
            Stmt::Expr(expr) => {
                self.eval(expr)?;
            }
        }
        Ok(())
    }

    fn eval(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::String(s) => Ok(Value::String(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Null => Ok(Value::Null),
            Expr::Ident(name) => Ok(self.vars.get(name).cloned().unwrap_or(Value::Null)),
            Expr::Field(inner, name) => {
                let v = self.eval(inner)?;
                Ok(v.get_field(name))
            }
            Expr::Unary(UnaryOp::Not, inner) => {
                let v = self.eval(inner)?;
                Ok(Value::Bool(!v.is_truthy()))
            }
            Expr::Binary(left, op, right) => self.eval_binary(left, *op, right),
            Expr::Call(callee, args) => self.eval_call(callee, args),
        }
    }

    fn eval_binary(&mut self, left: &Expr, op: BinOp, right: &Expr) -> Result<Value, RuntimeError> {
        // Short-circuit before evaluating the right-hand side.
        if op == BinOp::And {
            let l = self.eval(left)?;
            if !l.is_truthy() {
                return Ok(Value::Bool(false));
            }
            return Ok(Value::Bool(self.eval(right)?.is_truthy()));
        }
        if op == BinOp::Or {
            let l = self.eval(left)?;
            if l.is_truthy() {
                return Ok(Value::Bool(true));
            }
            return Ok(Value::Bool(self.eval(right)?.is_truthy()));
        }

        let l = self.eval(left)?;
        let r = self.eval(right)?;
        match op {
            BinOp::Add => match (&l, &r) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                _ => Ok(Value::String(format!("{l}{r}"))),
            },
            BinOp::Sub => match (&l, &r) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a - b)),
                _ => Err(RuntimeError {
                    message: format!("cannot subtract {} and {}", r.type_name(), l.type_name()),
                }),
            },
            BinOp::Eq => Ok(Value::Bool(values_equal(&l, &r))),
            BinOp::NotEq => Ok(Value::Bool(!values_equal(&l, &r))),
            BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
                let (a, b) = match (&l, &r) {
                    (Value::Number(a), Value::Number(b)) => (*a, *b),
                    _ => {
                        return Err(RuntimeError {
                            message: format!(
                                "cannot compare {} and {}",
                                l.type_name(),
                                r.type_name()
                            ),
                        });
                    }
                };
                let result = match op {
                    BinOp::Lt => a < b,
                    BinOp::Gt => a > b,
                    BinOp::LtEq => a <= b,
                    BinOp::GtEq => a >= b,
                    _ => unreachable!(),
                };
                Ok(Value::Bool(result))
            }
            BinOp::And | BinOp::Or => unreachable!("handled above"),
        }
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<Value, RuntimeError> {
        let arg_values: Result<Vec<Value>, RuntimeError> =
            args.iter().map(|a| self.eval(a)).collect();
        let arg_values = arg_values?;

        match callee {
            Expr::Ident(name) => self.call_builtin(name, arg_values),
            Expr::Field(receiver, method) => {
                let recv = self.eval(receiver)?;
                call_method(&recv, method, &arg_values)
            }
            _ => Err(RuntimeError { message: "expression is not callable".to_owned() }),
        }
    }

    fn call_builtin(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        match name {
            "env" => {
                let key = expect_string(&args, 0, "env")?;
                Ok(Value::String(
                    self.input
                        .env_vars
                        .iter()
                        .find(|(k, _)| k == key)
                        .map(|(_, v)| v.clone())
                        .unwrap_or_default(),
                ))
            }
            "set_env" => {
                let key = expect_string(&args, 0, "set_env")?.to_owned();
                let value = expect_stringlike(&args, 1, "set_env")?;
                self.effects.push(Effect::SetEnv(key, value));
                Ok(Value::Null)
            }
            "header" => {
                let key = expect_string(&args, 0, "header")?;
                Ok(Value::String(lookup_ci(&self.input.headers, key).unwrap_or_default()))
            }
            "headers" => Ok(Value::Object(
                self.input.headers.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect(),
            )),
            "set_header" => {
                let key = expect_string(&args, 0, "set_header")?.to_owned();
                let value = expect_stringlike(&args, 1, "set_header")?;
                self.effects.push(Effect::SetHeader(key, value));
                Ok(Value::Null)
            }
            "cookie" => {
                let key = expect_string(&args, 0, "cookie")?;
                Ok(Value::String(lookup_ci(&self.input.cookies, key).unwrap_or_default()))
            }
            "body" => Ok(Value::String(self.input.body.clone())),
            "set_body" => {
                let value = expect_stringlike(&args, 0, "set_body")?;
                self.effects.push(Effect::SetBody(value));
                Ok(Value::Null)
            }
            "url" => Ok(Value::String(self.input.url.clone())),
            "test" => {
                let name = expect_string(&args, 0, "test")?.to_owned();
                let passed = args.get(1).is_some_and(Value::is_truthy);
                self.effects.push(Effect::Test { name, passed });
                Ok(Value::Null)
            }
            "print" => {
                let text = expect_stringlike(&args, 0, "print")?;
                self.effects.push(Effect::Log(text));
                Ok(Value::Null)
            }
            "base64_decode" => {
                let s = expect_string(&args, 0, "base64_decode")?;
                Ok(builtins::base64_decode(s))
            }
            "base64_encode" => {
                let s = expect_string(&args, 0, "base64_encode")?;
                Ok(Value::String(builtins::base64_encode(s)))
            }
            "jwt_decode" => {
                let s = expect_string(&args, 0, "jwt_decode")?;
                Ok(builtins::jwt_decode(s))
            }
            "json_parse" => {
                let s = expect_string(&args, 0, "json_parse")?;
                Ok(builtins::json_parse(s))
            }
            "json_stringify" => {
                let v = args
                    .first()
                    .ok_or_else(|| RuntimeError { message: "json_stringify() expects 1 argument".to_owned() })?;
                Ok(Value::String(builtins::json_stringify(v)))
            }
            "aes_encrypt" => {
                let plaintext = expect_string(&args, 0, "aes_encrypt")?;
                let key = expect_string(&args, 1, "aes_encrypt")?;
                Ok(builtins::aes_encrypt(plaintext, key))
            }
            "aes_decrypt" => {
                let ciphertext = expect_string(&args, 0, "aes_decrypt")?;
                let key = expect_string(&args, 1, "aes_decrypt")?;
                Ok(builtins::aes_decrypt(ciphertext, key))
            }
            "contains" => {
                let haystack = expect_any(&args, 0, "contains")?;
                let needle = expect_any(&args, 1, "contains")?;
                Ok(Value::Bool(value_contains(haystack, needle)))
            }
            other => Err(RuntimeError { message: format!("unknown function '{other}'") }),
        }
    }
}

fn call_method(receiver: &Value, method: &str, args: &[Value]) -> Result<Value, RuntimeError> {
    match method {
        "contains" => {
            let needle = expect_any(args, 0, ".contains")?;
            Ok(Value::Bool(value_contains(receiver, needle)))
        }
        "json" => {
            let raw = receiver.get_field("__raw_body");
            match raw.as_str() {
                Some(text) => Ok(builtins::json_parse(text)),
                None => Err(RuntimeError {
                    message: "'.json()' is only available on 'response'".to_owned(),
                }),
            }
        }
        "text" => {
            let raw = receiver.get_field("__raw_body");
            match raw.as_str() {
                Some(text) => Ok(Value::String(text.to_owned())),
                None => Err(RuntimeError {
                    message: "'.text()' is only available on 'response'".to_owned(),
                }),
            }
        }
        other => Err(RuntimeError { message: format!("unknown method '.{other}()'") }),
    }
}

/// Backs both `contains(haystack, needle)` and `haystack.contains(needle)`,
/// which are the same function spelled two ways — scripts read better one way
/// or the other depending on whether the haystack is a call result
/// (`response.text().contains("...")`) or a variable (`contains(body, "...")`).
///
/// What "contains" means depends on the haystack: a substring test on a
/// string, a membership test on an array, and a key test on an object. A
/// non-string needle is compared by its text form against a string haystack,
/// so `contains(body(), 404)` works without stringifying by hand.
///
/// Matching is case-sensitive. There is no `lower()` in this language, so a
/// case-insensitive match would be impossible to opt out of, while an exact
/// one can always be loosened by passing a shorter needle.
///
/// Anything else — `null`, a bool, a number — contains nothing and returns
/// `false` rather than erroring, matching how field access on a non-object is
/// a silent `null`: a script asking "is X in this missing field" wants a
/// usable `false`, not an aborted run.
fn value_contains(haystack: &Value, needle: &Value) -> bool {
    match haystack {
        Value::String(text) => text.contains(&needle.to_string()),
        Value::Array(items) => items.iter().any(|item| values_equal(item, needle)),
        Value::Object(fields) => match haystack.get_field("__raw_body") {
            // `response` is an object carrying the body in a private field, so
            // `response.contains("...")` searches the body. Its own field
            // names are an implementation detail no script can see, which
            // makes them the one thing nobody can mean here.
            Value::String(body) => body.contains(&needle.to_string()),
            _ => {
                let key = needle.to_string();
                fields.iter().any(|(name, _)| *name == key)
            }
        },
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        _ => false,
    }
}

fn expect_string<'a>(args: &'a [Value], index: usize, fn_name: &str) -> Result<&'a str, RuntimeError> {
    args.get(index)
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeError {
            message: format!("{fn_name}() expects a string argument at position {}", index + 1),
        })
}

/// For arguments where every type is meaningful and none needs converting —
/// `contains`, whose answer depends on whether it was handed a string, an
/// array or an object. Only presence is checked.
fn expect_any<'a>(args: &'a [Value], index: usize, fn_name: &str) -> Result<&'a Value, RuntimeError> {
    args.get(index).ok_or_else(|| RuntimeError {
        message: format!("{fn_name}() expects an argument at position {}", index + 1),
    })
}

/// Like [`expect_string`], but for value-shaped arguments (a header/env/body
/// value) rather than name-shaped ones (a header/env key) — any value is
/// accepted and auto-converted to a string, since forwarding "whatever this
/// JWT claim / JSON field turned out to be" as a header value is a normal
/// thing to want, and shouldn't require every script to remember to call
/// `json_stringify` on non-string values by hand.
fn expect_stringlike(args: &[Value], index: usize, fn_name: &str) -> Result<String, RuntimeError> {
    args.get(index)
        .map(|v| match v {
            Value::Object(_) | Value::Array(_) => crate::builtins::json_stringify(v),
            other => other.to_string(),
        })
        .ok_or_else(|| RuntimeError {
            message: format!("{fn_name}() expects an argument at position {}", index + 1),
        })
}

fn lookup_ci(pairs: &[(String, String)], key: &str) -> Option<String> {
    pairs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.clone())
}

pub(crate) fn new_interpreter(input: HostInput) -> Interpreter {
    Interpreter::new(input)
}

pub(crate) fn run_program(
    interpreter: &mut Interpreter,
    program: &[Stmt],
) -> Result<RunOutcome, RuntimeError> {
    interpreter.run(program)
}
