// 2026-09-24: Library root. The binary in `main.rs` is a thin shell over
// `cli` and `run`; everything else is pure and unit-tested in place.

pub mod baseline;
pub mod calendar;
pub mod classify;
pub mod cli;
pub mod comment;
pub mod config;
pub mod exemptions;
pub mod formats;
pub mod git;
pub mod lexer;
pub mod report;
pub mod run;
pub mod scan;
pub mod ttl;
