//! Rustman's transport: [`ReqwestTransport`] sends resolved requests over
//! HTTP, decoding bodies itself so the wire headers and sizes survive, and
//! [`websocket`] drives WebSocket connections.

mod auth;
mod client;
mod error;
mod redirect;
mod request;
mod response;
mod transport;
pub mod websocket;

pub use client::{TlsOptions, TlsPin, build_client_with, try_build_client};
pub use rustman_core::jwt::{make_jwt, make_jwt_at};
pub use transport::ReqwestTransport;
