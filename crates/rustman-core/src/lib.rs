//! Rustman's domain model, the pure helpers every other crate shares, and
//! the ports the others implement: [`run::HttpTransport`],
//! [`script::ScriptHost`] and the [`store`] traits. Nothing here performs
//! I/O.

pub mod domain;
pub mod jwt;
pub mod run;
pub mod script;
pub mod secrets;
pub mod store;
pub mod template;
pub mod text;
pub mod time;
pub mod url;
