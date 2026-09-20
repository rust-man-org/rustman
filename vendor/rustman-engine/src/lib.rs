//! `rustman-engine`: a tiny, deliberately simple scripting language for
//! API pre-request/test scripts.
//!
//! It exists because hand-writing Postman-style JavaScript is not the point
//! here — scripts in this language are meant to be *generated*, by an AI,
//! from a plain-English description of what the user wants. The spec lives in
//! `docs/scripting.html` in the main rustman repo (published at
//! <https://rust-man-org.github.io/rustman/scripting.html>); it has the
//! exact grammar, the built-in function reference, and a copy-pasteable
//! prompt to hand an AI as context.
//!
//! Deliberately small on purpose: `let`, `if`/`else`, function calls, field
//! access, and a handful of comparison/logic operators — no loops, no
//! user-defined functions, no classes. Interpreted directly from an AST
//! (no bytecode/JIT); scripts are a few lines, so this is plenty fast.

mod ast;
mod builtins;
mod interpreter;
mod lexer;
mod parser;
mod value;

pub use interpreter::{Effect, HostInput, ResponseInput, RunOutcome, RuntimeError};
pub use value::Value;

/// Something went wrong turning source text into a result: a lex error, a
/// parse error, or a runtime error, each with the best line number available.
#[derive(Debug, Clone)]
pub struct ScriptError {
    pub message: String,
    /// Best-effort source line; `None` for errors that don't have one handy
    /// (e.g. some runtime errors evaluate multiple sub-expressions at once).
    pub line: Option<usize>,
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

/// Parses and runs `source` against `input`, returning the effects the
/// script asked for (environment variable writes, header writes, test
/// results). The interpreter never touches host state directly — the host
/// applies `RunOutcome::effects` itself, which keeps this crate a pure
/// function of its input and easy to unit test.
pub fn run(source: &str, input: HostInput) -> Result<RunOutcome, ScriptError> {
    let tokens = lexer::tokenize(source).map_err(|e| ScriptError {
        message: e.message,
        line: Some(e.line),
    })?;
    let program = parser::parse(&tokens).map_err(|e| ScriptError {
        message: e.message,
        line: Some(e.line),
    })?;
    let mut interpreter = interpreter::new_interpreter(input);
    interpreter::run_program(&mut interpreter, &program).map_err(|e| ScriptError {
        message: e.message,
        line: None,
    })
}
