# Changelog

## 1.0.1 — 2026-09-24

- The action's checksum step failed on Windows runners: GNU `sha256sum`
  prefixes the digest with a backslash when the path contains one. The
  digest is now computed on the bare file name. v1.0.0 is unaffected on
  Linux and macOS; its binaries are identical.

## 1.0.0 — 2026-09-24

First release.

- Composite action wrapping a static Rust binary for Linux x64/arm64 (musl),
  macOS arm64/x64 and Windows x64, verified against checksums written into
  `action.yml` at release time.
- CLI: `comment-ttl check` and `comment-ttl baseline`.
- Inputs: `comment_ttl`, `scan_dirs`, `file_types`, `exempt_paths`,
  `comment_formats`, `exemptions`, `output_type`, `fail_on_flag`,
  `reference_date`, `warn_within`, `baseline`, `scope`, `undated`,
  `annotations`, plus `artifact_name` and `binary_path`.
- Outputs: flagged, stale, undated, malformed, future, exempt, escape,
  expiring, fresh, comments, files_scanned, report_path; JSON/CSV/SARIF/
  Markdown reports; job summary; annotations; report artifact.
- Lexers for C-style, `#`, `--`, `;`, `%`, HTML-style (&lt;!-- --&gt;) and mixed
  (HTML/Svelte/Vue) files that skip strings and template literals.
