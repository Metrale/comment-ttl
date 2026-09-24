// SPDX-License-Identifier: MIT

//! 2026-09-20: fresh inner doc comment.

/// 2026-07-01: stale doc comment, expired on 2026-08-01.
pub fn stale() {}

/// Undated doc comment with prose.
pub fn undated() {}

// 2026-02-30: malformed date.
pub fn malformed() {}

// 2026-12-01: future-dated comment.
pub fn future() {}

// no-rot: mirrors a fixed wire format that does not change.
pub fn escaped() {}

// no-rot
pub fn escape_without_reason() {}

// 2026-08-27: expiring soon (expires 2026-09-27).
pub fn expiring() {}

// 2026-09-25: one day ahead is still fresh.
pub fn tomorrow() {}

pub fn strings() -> &'static str {
    "// not a comment"
}

/// @deprecated
pub fn tags_only() {}
