//! Collections under version control: a git working tree with one JSON file
//! per collection, and the registry of repositories the app knows about.

mod atomic;
pub mod repos;
pub mod vcs;
