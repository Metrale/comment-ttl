//! 2026-09-24: inner doc comment at the top
use std::fmt;

/// Doc comment with prose is not exempt
/// and spans two lines
pub fn f<'a>(x: &'a str) -> &'a str {
    let raw = r#"// DECOY "#; // raw string then a real comment
    let byte = b"// DECOY"; // byte string
    let ch = '/'; let quote = '\''; let uni = 'é'; // char literals
    /* outer /* nested */ still outer */
    let s = "string with /* DECOY */"; // trailing
    #[allow(dead_code)]
    // no-rot: mirrors RFC 8949 which does not change
    x
}
