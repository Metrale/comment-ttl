# comment-ttl

A GitHub Action and CLI that fails CI when a code comment has not been
re-verified within its time-to-live.

Every comment starts with the date it was last verified. A comment older than
`comment_ttl` fails the check until someone re-reads it and fixes it, deletes
it, or re-dates it.

```rust
// 2026-09-24: `plan_write` MUTATES despite its name — it evicts the victim
// and installs `key` before any bytes move.
```

## The problem

Code changes; comments don't. A stale comment is worse than none: it sends the
next reader — human or AI agent — confidently in the wrong direction, and
nothing in CI can catch it because nothing knows how old a claim is. Comments
are the only part of a codebase with no compiler and no test.

`comment-ttl` gives every comment an expiry. Reviewing a comment is cheap;
re-dating it is a one-line change; and the check turns "somebody should
re-read that" into a failing build.

### The date means "last verified", not "written"

`2026-09-24` says *someone confirmed this was true on 2026-09-24*. When you
re-read a comment and it is still correct, bump the date. When it is wrong,
fix it and bump the date. When it is no longer needed, delete it. A fresh date
on an old comment is the intended outcome, not a smell.

## Quickstart

```yaml
# .github/workflows/comments.yml
name: comments
on: [pull_request]
permissions:
  contents: read
jobs:
  comment-ttl:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@<sha>
      - uses: Metrale/comment-ttl@<full commit SHA of a release> # vX.Y.Z
        with:
          comment_ttl: 1 month
          warn_within: 7d
          exempt_paths: ^(vendor|generated)/
```

The action prints annotations on the pull request, writes a job summary with
every finding, and uploads the full report as the `comment-ttl-report`
artifact. Pin the action by the full commit SHA printed in each release's
notes; `v1` is a floating tag for people who prefer it.

Locally:

```sh
cargo install comment-ttl        # or download a release binary
comment-ttl check --comment-ttl "1 month"
```

The CLI exits `0` when nothing is flagged, `1` when something is flagged and
`fail_on_flag` is true, and `2` on a configuration error.

## Inputs

Every input can also be set in an optional `.comment-ttl.toml` at the
repository root; an explicit action input (or CLI flag) wins over the file.

| input | default | description |
|---|---|---|
| `comment_ttl` | *(required)* | Time-to-live after which a dated comment must be re-verified. See [the grammar](#the-comment_ttl-grammar). |
| `scan_dirs` | `.` | Regex over directories to scan, matched against `dir/`; files at the root see `./`. |
| `file_types` | *(all known)* | Regex over file names to include, e.g. `\.(rs\|py)$`. |
| `exempt_paths` | *(none)* | Regex over paths to skip entirely (generated, vendored, build output). |
| `comment_formats` | *(none)* | YAML/JSON mapping of file-name regex to a comment syntax, merged over [the defaults](#default-comment-formats). |
| `exemptions` | *(none)* | YAML/JSON list of regexes that extend [the default exemptions](#default-exemptions), or `{replace: true, patterns: [...]}` to replace them. |
| `output_type` | `json` | `json`, `csv`, `sarif` or `markdown`. |
| `fail_on_flag` | `true` | Fail the step when any comment is stale, undated, malformed or future-dated. `false` makes the check advisory. |
| `reference_date` | `commit` | `now`, `commit` (the head commit's committer date in UTC, so re-runs give the same result) or a fixed `YYYY-MM-DD`. |
| `warn_within` | *(none)* | Report comments whose TTL ends within this duration as `expiring`, without failing. |
| `baseline` | *(none)* | Path to a [baseline file](#adoption-path) of grandfathered comments. |
| `scope` | `all` | `all`, or `changed` to judge only comments inside the pull request's hunks (needs the base commit fetched, e.g. `fetch-depth: 0`). |
| `undated` | `fail` | `fail`, or `blame` to date undated comments from `git blame` instead of flagging them. |
| `annotations` | `true` | Emit GitHub annotations. The runner shows at most 10 per step; the job summary and the report always carry the full list. |
| `artifact_name` | `comment-ttl-report` | Name of the uploaded report artifact (change it when a job runs the action twice). |
| `binary_path` | *(none)* | Path to a locally built `comment-ttl`; skips the download and checksum. For developing this action or air-gapped runners. |

`.comment-ttl.toml` uses the same names:

```toml
comment_ttl = "1 month"
warn_within = "7d"
exempt_paths = "^(vendor|generated)/"
exemptions = ["^// -{3,}$"]

[comment_formats]
"\\.zig$" = "c"
"\\.cobol$" = { line = ["*>"], strings = ["\""] }
```

## Outputs

| output | description |
|---|---|
| `flagged` | stale + undated + malformed + future |
| `stale` | comments past their TTL |
| `undated` | comments with no leading date |
| `malformed` | invalid dates (e.g. `2026-02-30`) and `no-rot` escapes without a reason |
| `future` | comments dated more than one day after the reference date |
| `exempt` | comments matched by an exemption, or grandfathered by the baseline |
| `escape` | comments carrying a `no-rot: <reason>` |
| `expiring` | comments whose TTL ends within `warn_within` |
| `fresh` | comments within their TTL |
| `comments` | total comments scanned |
| `files_scanned` | files scanned |
| `report_path` | path of the written report |

The report (JSON by default) lists every finding with its path, line range,
classification, date, expiry, age in days, a reason and an excerpt. Fresh and
exempt comments are counted but not listed.

## The `comment_ttl` grammar

Case-insensitive. Either:

- terms `<int><unit>` separated by spaces or commas, with unit
  `d|day(s)`, `w|wk|week(s)`, `mo|mon|month(s)`, `y|yr|year(s)`:
  `1 month`, `1mo`, `30d`, `2w`, `1y 2mo`; or
- an ISO-8601 date duration with no time part: `P1M`, `P1Y2M3W4D`.

Rejected, with an error listing the valid forms: bare `m` (ambiguous), hours,
minutes, seconds, zero, negatives and empty values. `warn_within` uses the
same grammar.

Arithmetic is on UTC calendar dates. Years and months are added first and
clamp to the end of the month (`2026-01-31 + 1 month = 2026-02-28`); weeks and
days are exact. A comment is fresh while `reference_date <= date + ttl`.

## What counts as a comment

1. Files come from `git ls-files`, filtered by `scan_dirs`, `file_types` and
   `exempt_paths`. Files with a NUL byte in the first 8 KiB are skipped as
   binary; files with no known comment syntax are not scanned.
2. Each file is lexed with a syntax that skips string literals, template
   literals (including nested `${ }`), Rust raw strings and char literals,
   and JavaScript regex literals, so `"http://…"` is never a comment.
3. Consecutive own-line line comments with the same marker and the same
   indent form one comment. A block comment is one comment. A trailing comment
   (`x = 1; // note`) stands alone.
4. The date must be the first token of the comment text (after the markers):
   `// 2026-09-24: note`, `/** 2026-09-24 note */`, `# 2026-09-24 note`.
   A trailing `:` or `,` is fine. `// see 2026-09-24` is undated.
5. Each comment is classified as **exempt**, **escape**, **undated**,
   **malformed**, **future**, **stale**, **expiring** or **fresh**.
6. Everything is scanned before anything fails: the report is complete even
   when the step is red.

### Escaping a comment

```rust
// no-rot: mirrors the wire format in RFC 8949 §3.1, which does not change.
```

A `no-rot:` comment never needs a date, but the reason is mandatory: a bare
`// no-rot` is reported as malformed. Escapes are counted separately so a repo
can see whether the hatch is becoming the norm.

## Default comment formats

Matched against the file name, first match wins; your `comment_formats`
entries are tried before these.

| files | syntax |
|---|---|
| `.js .mjs .cjs .jsx .ts .mts .cts .tsx .jsonc`, `tsconfig*.json` | `//`, `/* */`; skips `"`, `'`, templates and regex literals |
| `.rs` | `//`, nested `/* */`; skips strings, raw strings, chars vs lifetimes |
| `.go` | `//`, `/* */`; skips `"`, `'`, raw `` ` `` |
| `.java .kt .kts .scala .c .h .cc .cpp .cxx .hpp .hh .hxx .cu .cuh .cs .swift .dart .proto .groovy .gradle .zig .metal .glsl .hlsl .wgsl .scss .less .php` | `//`, `/* */` |
| `.css` | `/* */` |
| `.py .pyi .pyw` | `#`; skips `"`, `'`, triple quotes |
| `.sh .bash .zsh .ksh .fish`, `.bashrc`… | `#`; skips `"` and `'` |
| `Dockerfile Makefile Justfile .gitignore .gitattributes .editorconfig .env*`, `.mk .cfg .conf .properties`, `requirements*.txt` | `#` |
| `.yml .yaml` | `#`; skips `"` and `''`-doubled `'` |
| `.toml` | `#`; skips basic and literal strings |
| `.rb .rake .gemspec`, `Gemfile`, `Rakefile` | `#`, `=begin`/`=end` |
| `.sql .psql` | `--`, `/* */` |
| `.lua` | `--`, `--[[ ]]`; skips `[[ ]]` |
| `.hs` | `--`, nested `{- -}` |
| `.lisp .el .clj .cljs .cljc .edn .scm .ss .rkt .ini .asm` | `;`, `#` |
| `.tex .sty .cls .bib` | `%` (`\%` is not a comment) |
| `.erl .hrl` | `%` |
| `.md .markdown .mdx .xml .svg .xsl .xslt .xsd .plist .csproj …` | HTML comments (&lt;!-- --&gt;) |
| `.html .htm .svelte .vue .astro` | HTML comments in markup, JS inside `<script>`, CSS inside `<style>` |
| `.ps1 .psm1 .psd1` | `#`, `<# #>` |

A `#` comment must start the line or follow whitespace, so `url#frag`, `$#`
and `${#x}` are code. Add or override a format with a preset name (`c`, `js`,
`rust`, `go`, `css`, `python`, `shell`, `hash`, `yaml`, `toml`, `ruby`, `sql`,
`lua`, `haskell`, `semicolon`, `tex`, `erlang`, `xml`, `html`, `powershell`)
or an inline `{line: [...], block: [[open, close]], strings: [...]}`.

## Default exemptions

Comments matching any of these never need a date. Patterns run against the
comment as written (markers included).

| exemption | matches |
|---|---|
| license headers | `SPDX-License-Identifier`, `SPDX-FileCopyrightText`, `Copyright`, `(c) 2026`, `Licensed under…`, `Permission is hereby granted…`, and similar |
| shebangs | `#!…` |
| tool directives | `eslint-disable`, `@ts-expect-error`, `prettier-ignore`, `svelte-ignore`, `cspell:`, `markdownlint-`, `noqa`, `type: ignore`, `#[allow`, `clippy::`, `rustfmt::skip`, `yaml-language-server` |
| doc blocks with only tags | JSDoc/rustdoc blocks whose every line is a `@tag` (or empty), e.g. `/** @param a @returns b */` |
| pinned action versions | `# v4`, `# v4.2.2` after a SHA-pinned `uses:` |

**Doc comments with prose are not exempt.** `///`, `//!`, `/** … */` and
docstrings are the API contract and the first thing to drift when behaviour
changes; a project may exempt them with its own pattern, but that should be a
deliberate choice.

`exemptions: ["^// -{3,}$"]` adds a pattern; `exemptions: {replace: true,
patterns: [...]}` starts from nothing.

## Adoption path

Turning this on cold fails every undated comment in the repository. The
intended path is baseline → advisory → enforcing.

1. **Baseline.** Record the current flagged comments as grandfathered:

   ```sh
   comment-ttl baseline --comment-ttl "1 month" --out .comment-ttl-baseline.json
   ```

   Entries are fingerprinted by path plus a hash of the comment's text, so a
   grandfathered comment stays exempt until its text changes; then it needs a
   date like any other. Pass the file as `baseline:` and commit it.
2. **Advisory.** Run with `fail_on_flag: false` while directories are brought
   into line — review each comment, fix it, delete it, or add today's date.
   Spread that work over days so expiry dates don't all land on one day.
3. **Enforcing.** Flip `fail_on_flag` to `true`, make the check required, and
   shrink the baseline as directories are cleaned. A scheduled run with
   `reference_date: now` and `warn_within: 7d` gives a week's notice before
   comments expire.

`scope: changed` is an alternative to a baseline for repositories that only
want to police comments touched by each pull request.

## Risks

- **Dates bumped without real checking.** A date bump is trivially gameable;
  a script (or a hurried reviewer) that re-dates comments without reading them
  defeats the mechanism and leaves it *looking* healthy. Treat bulk re-dating
  as a code-review smell, keep the sweeps small, and consider sampling
  re-dated comments in review. A check that cannot fail is worse than none.
- **Formatters moving comments.** Formatters can reflow or move comment text;
  grouping is by adjacency and indent, so a formatter that splits a comment
  block can turn one dated comment into a dated one plus an undated one. Run
  the check after formatting.
- **Churn in `git blame`.** Regular sweeps touch many lines. `scope: changed`
  limits the check to lines already being touched.
- **Comment-shaped strings.** The lexers skip strings and template literals in
  every supported language, but a language not on the list (or an inline
  syntax without `strings`) falls back to plain marker matching.

## Security model

- The action is a composite action wrapping a **statically built Rust
  binary**: no Node runtime, no container, no third-party action beyond
  `actions/upload-artifact` (pinned by SHA).
- Each release is **immutable** and built by the release workflow with
  `cargo build --locked` from the tagged commit. Every binary carries a
  [build provenance attestation](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations/using-artifact-attestations-to-establish-provenance-for-builds);
  verify with `gh attestation verify <file> --repo Metrale/comment-ttl`.
- `action.yml` at a release commit carries the **SHA-256 of every binary**,
  written at release time. The action downloads the asset for the runner's OS
  and architecture from that release and refuses to run on a checksum
  mismatch, on an unsupported platform, or on a non-release ref (which still
  has placeholders).
- **Pin by commit SHA.** The release notes print the exact line to use.
- The action needs only `contents: read`. It runs `git ls-files`, `git log`,
  `git diff` (for `scope: changed`) and `git blame` (for `undated: blame`)
  in the checkout, reads files, and writes the report, the job summary and
  the step outputs. It makes no network requests beyond the binary download.
- Action inputs reach the binary as environment variables, never through
  shell interpolation.

## Development

```sh
cargo test                       # unit, property, golden and CLI snapshot tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

CI runs those on Linux, macOS and Windows plus the declared MSRV, `cargo deny`,
`actionlint` and `zizmor`, an end-to-end workflow that runs `uses: ./` against
`tests/fixtures/e2e` and checks every output, and — of course — `comment-ttl`
on this repository's own comments.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
