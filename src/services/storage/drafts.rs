//! SQLite tables for workstream 01. Owned by 01; see contract C11.

use rusqlite::Connection;

/// Additive DDL for this workstream. Called from `storage::migrate`; each
/// statement must be idempotent (`CREATE TABLE IF NOT EXISTS`, ignored
/// `ALTER` errors) like the rest of `migrate`.
pub(super) fn migrate(_conn: &Connection) {}
