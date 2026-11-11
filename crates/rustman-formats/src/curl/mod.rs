//! cURL import and export.
//!
//! Import runs in three layers, each testable alone: [`shell`] splits the
//! pasted text into words (bash, cmd.exe or PowerShell quoting), [`options`]
//! groups the words by curl's option grammar, and [`parser`] turns the
//! options into a request. [`generator`] goes the other way.

mod generator;
mod options;
mod parser;
mod shell;

pub use generator::export;
pub use parser::{CurlForm, ParsedCurl, parse};
pub use rustman_core::domain::headers::Headers;

pub(crate) use parser::looks_like_command;
pub(crate) use shell::split as tokenize;
