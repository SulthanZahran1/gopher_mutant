//! gopher-mutant-core — engine library for gopher_mutant.
//!
//! Pipeline: parse (tree-sitter-go) → discover mutation points → apply
//! mutants via `go test -overlay` byte-patching (source tree untouched) →
//! run `go test` → classify (killed/survived/not_covered/compile_error/timeout).
//!
//! M1 scope (GOAL-1): the 10 generic operator classes shared with dart_mutant.
//! The 12 Go-idiomatic operators land in M2.

pub mod classify;
pub mod discover;
pub mod mutate;
pub mod operators;
pub mod parse;
pub mod runner;

pub use classify::{Classification, Outcome};
pub use discover::{Discovery, FileDiscovery, MutationPoint};
pub use mutate::apply_mutant;
pub use operators::{Operator, ALL_OPERATORS};
pub use parse::parse_go_file;
