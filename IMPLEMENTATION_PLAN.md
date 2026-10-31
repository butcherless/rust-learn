# Implementation plan — GitHub open PRs CLI (Rust)

> Brief for a coding agent. Implement exactly what is described here. Where this
> document is silent, prefer the simplest option that respects the principles below.

## Status (2026-10-31)

Implemented in [PR #6](https://github.com/butcherless/rust-learn/pull/6), which is open with CI passing.
The live run in section 8 is not recorded as done. `README.md` is the current user documentation.
The original brief below is unchanged, and these points supersede it:

| Section | Original | Current |
|---|---|---|
| 2 | Do not commit or push | Committed and pushed for PR #6, on request |
| 3 | Sample repos in `projects.toml` | The repo file lists the `butcherless/*` repos from `check-prs.sh` |
| 4.3 | Private-by-default DTO | `PullRequestDto`, its fields and the `dto` module are `pub`, because `state` is never read |
| 4.3 | UTF-8 rounded preset | comfy-table 8 removed `load_preset` and modifiers: use `load_style(UTF8_FULL_CONDENSED.with_rounded_corners())` |
| 4.3 | Query parameters | Built with `reqwest::Url`, since `RequestBuilder::query` needs a feature not listed in section 5 |
| 4.3 | `X-GitHub-Api-Version: 2022-11-28` | Kept. Supported through 2028-03-10; `2026-03-10` is the newest version |
| 5 | Dev `tokio`: `macros`, `rt-multi-thread` | Also `time`, for the concurrency test |
| 5 | `comfy-table` min 7 | Resolved to 8.0.1 |

## 1. Goal

A command-line tool, run on macOS (Apple Silicon and Intel), that:

1. Reads a list of GitHub repositories from `projects.toml`.
2. For each repository, retrieves its **open** pull requests from the GitHub REST API (github.com).
3. Prints a colored table to the console: project name, PR number, PR title, PR state.

## 2. Context

- Implement inside the existing crate in this folder (`rust-learn`, edition 2024).
  Keep the crate name. Replace the hello-world `src/main.rs`.
- Single crate (no workspace). Add a `src/lib.rs` so integration tests in `tests/` can reach the modules; `main.rs` stays a thin composition root.
- CI (`.github/workflows/ci.yml`) runs `cargo build` and `cargo test` on Ubuntu. **Tests must not hit the real network** and must not require `GITHUB_TOKEN`.
- Update `README.md` with: purpose, `projects.toml` format, `GITHUB_TOKEN` setup, how to run, how to test.
- Do not commit or push; leave changes in the working tree.

## 3. Functional requirements

| # | Requirement |
|---|---|
| FR1 | Project list comes from a TOML file, default `./projects.toml`, overridable with `--config <path>`. |
| FR2 | Authentication via the `GITHUB_TOKEN` environment variable. If missing or empty → exit with a clear error message and non-zero code, before any HTTP call. |
| FR3 | Target is github.com only, but the base URL must be injectable into the adapter (needed for wiremock tests). Default `https://api.github.com`. |
| FR4 | For each project call `GET /repos/{owner}/{repo}/pulls?state=open&per_page=100`. **No pagination**: one request per project, ignore the `Link` header. |
| FR5 | Fetch at most **2 repositories concurrently**. Output order must match the order in `projects.toml`. |
| FR6 | Per-project error isolation: if one repository fails (404, 401/403, rate limit, network, decode), report the error for that project in the table and continue with the rest. |
| FR7 | Exit code: `0` if every project succeeded; `1` if at least one project failed (the table is still printed); `2` for startup errors (config, token). |
| FR8 | Output is a table with columns **Project**, **PR #**, **Title**, **State**. |
| FR9 | Row coloring: **green** for a project with no open PRs (single row, title cell shows `no open pull requests`); **orange** for each open-PR row; **red** for a project whose fetch failed (single row, title cell shows the error message). |
| FR10 | State column shows `open`, or `draft` when the API's `draft` flag is true. |

### `projects.toml` format

```toml
projects = [
  "rust-lang/rust",
  "tokio-rs/tokio",
  "seanmonstar/reqwest",
]
```

Each entry must be `owner/repo` (both non-empty, exactly one `/`, trimmed). Invalid entries make config loading fail with a message naming the offending entry. An empty list is valid and prints an empty table. Ship a sample `projects.toml` in the repo root.

### Expected output (illustrative)

```
╭──────────────────────┬───────┬─────────────────────────────────┬───────╮
│ Project              ┆ PR #  ┆ Title                           ┆ State │
╞══════════════════════╪═══════╪═════════════════════════════════╪═══════╡
│ rust-lang/rust       ┆ 12345 ┆ Fix borrowck ICE in closures    ┆ open  │  (orange)
│ rust-lang/rust       ┆ 12350 ┆ Stabilize foo                   ┆ draft │  (orange)
│ tokio-rs/tokio       ┆ —     ┆ no open pull requests           ┆ —     │  (green)
│ acme/missing         ┆ —     ┆ error: repository not found     ┆ —     │  (red)
╰──────────────────────┴───────┴─────────────────────────────────┴───────╯
```

## 4. Architecture

Three layers, hexagonal style. Dependencies point inward only: `adapter → service → domain`. The domain never imports `reqwest`, `serde`, `tokio`, `clap`, `toml` or `comfy-table`.

```
rust-learn/
├── Cargo.toml
├── projects.toml                      # sample config
├── README.md
├── src/
│   ├── main.rs                        # composition root, the only place with side-effect wiring
│   ├── lib.rs                         # pub mod domain; pub mod service; pub mod adapter;
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── model.rs                   # Project, PullRequest, PrNumber, PrTitle, PrState
│   │   ├── port.rs                    # trait PullRequestSource (outbound port)
│   │   └── error.rs                   # ProjectParseError, SourceError
│   ├── service/
│   │   ├── mod.rs
│   │   └── list_open_prs.rs           # use case + ProjectReport
│   └── adapter/
│       ├── mod.rs
│       ├── config.rs                  # inbound: projects.toml → Vec<Project>
│       ├── github/                    # outbound
│       │   ├── mod.rs
│       │   ├── client.rs              # GithubClient implements PullRequestSource
│       │   ├── dto.rs                 # serde wire types (only the fields needed)
│       │   └── mapper.rs              # DTO → domain, pure
│       └── cli/                       # inbound/presentation
│           ├── mod.rs
│           ├── args.rs                # clap derive
│           └── render.rs              # reports → comfy_table::Table, pure
└── tests/
    ├── list_open_prs_service.rs       # service with in-memory fake port
    └── github_client_wiremock.rs      # adapter against wiremock
```

### 4.1 Domain (`src/domain`)

Pure types and the port. No I/O.

- `Project { owner: Owner, repo: RepoName }` — newtypes over `String`, constructed only through validation.
  - `impl FromStr for Project` (or `TryFrom<&str>`) parsing `owner/repo`, returning `ProjectParseError`.
  - `impl Display for Project` → `owner/repo`.
- `PullRequest { number: PrNumber, title: PrTitle, state: PrState }`.
  - `PrNumber(u64)`, `PrTitle(String)`.
  - `enum PrState { Open, Draft }` with `Display` (`open` / `draft`).
- `enum SourceError` (via `thiserror`), domain-meaningful, transport-agnostic:
  `NotFound`, `Unauthorized`, `RateLimited`, `Unavailable(String)`, `InvalidResponse(String)`.
- Port:
  ```rust
  pub trait PullRequestSource {
      fn open_pull_requests(&self, project: &Project)
          -> impl Future<Output = Result<Vec<PullRequest>, SourceError>> + Send;
  }
  ```
  Use native return-position `impl Future` in traits (no `async-trait` crate). Static dispatch via generics, no `dyn`.

### 4.2 Service (`src/service`)

- `pub struct ProjectReport { pub project: Project, pub outcome: Result<Vec<PullRequest>, SourceError> }`
- `pub async fn list_open_prs<S: PullRequestSource>(source: &S, projects: Vec<Project>, max_concurrency: usize) -> Vec<ProjectReport>`
  - Implementation: `futures::stream::iter(projects).map(|p| async { ... }).buffered(max_concurrency).collect().await`.
  - `buffered` (not `buffer_unordered`) to keep config order.
  - Never fails as a whole: errors live inside each `ProjectReport`.
- `pub const MAX_CONCURRENCY: usize = 2;`
- Helper `pub fn has_failures(reports: &[ProjectReport]) -> bool` (pure), used by `main` for the exit code.

### 4.3 Adapters (`src/adapter`)

**config.rs** (inbound)
- `pub fn load_projects(path: &Path) -> Result<Vec<Project>, ConfigError>`.
- Split into effect + pure: read file → `parse_projects(&str) -> Result<Vec<Project>, ConfigError>` (pure, unit-tested). Collect with `Iterator<Item = Result<_, _>>` → `Result<Vec<_>, _>`.
- `ConfigError` via `thiserror`: `Io`, `Toml`, `InvalidProject { entry, source }`.

**github/** (outbound)
- `GithubClient { http: reqwest::Client, base_url: Url-or-String, token: GithubToken }`.
  - `GithubClient::new(base_url, token) -> Result<Self, ...>`; build one `reqwest::Client` with a timeout (10s) and reuse it.
  - `GithubToken(String)` newtype with a **redacting** `Debug` impl; never log or print the token.
- Request headers: `Authorization: Bearer <token>`, `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`, `User-Agent: rust-learn/<crate version>` (GitHub rejects requests without User-Agent).
- Status mapping (pure function `fn map_status(StatusCode, ...) -> SourceError`):
  - `404` → `NotFound`
  - `401` → `Unauthorized`
  - `403` or `429` with `x-ratelimit-remaining: 0` (or 429) → `RateLimited`; other `403` → `Unauthorized`
  - other non-2xx → `Unavailable("HTTP <code>")`
  - transport errors → `Unavailable(msg)`; JSON decode errors → `InvalidResponse(msg)`
- `dto.rs`: `#[derive(Deserialize)] struct PullRequestDto { number: u64, title: String, state: String, #[serde(default)] draft: bool }`. Ignore unknown fields.
- `mapper.rs`: `fn to_domain(dto: PullRequestDto) -> PullRequest` (pure; `draft == true` → `Draft`, else `Open`).

**cli/** (inbound/presentation)
- `args.rs`: `#[derive(Parser)] struct Args { #[arg(long, default_value = "projects.toml")] config: PathBuf }`.
- `render.rs`:
  - `enum RowKind { NoPullRequests, OpenPullRequest, Failed }` and a pure `fn rows(reports: &[ProjectReport]) -> Vec<(RowKind, [String; 4])>` — this is where all display logic lives, testable without ANSI codes.
  - `fn render(reports: &[ProjectReport]) -> comfy_table::Table` maps rows to cells and applies colors:
    green = `Color::Green`, orange = `Color::Rgb { r: 255, g: 165, b: 0 }`, red = `Color::Red`.
  - Use a UTF-8 rounded preset; let comfy-table disable styling when stdout is not a TTY.

### 4.4 Composition root (`src/main.rs`)

`#[tokio::main] async fn main() -> ExitCode`:
1. Parse `Args`.
2. Load projects (`adapter::config`) — on error print to stderr, exit 2.
3. Read `GITHUB_TOKEN` — missing/empty → stderr message, exit 2.
4. Build `GithubClient` with the default base URL.
5. `service::list_open_prs(&client, projects, MAX_CONCURRENCY).await`.
6. `println!("{}", render(&reports))`.
7. Exit `1` if `has_failures`, else `0`.

`anyhow` may be used only in `main.rs` for startup errors; library modules use `thiserror` enums.

## 5. Dependencies

Add with `cargo add` so the latest compatible versions are resolved; verify minimums below.

| Crate | Min version | Features | Purpose |
|---|---|---|---|
| `tokio` | 1.53 | `macros`, `rt-multi-thread` | async runtime |
| `reqwest` | 0.13 | `json` (rustls is the default TLS backend in 0.13 — keep it, no OpenSSL needed on macOS) | HTTP client |
| `serde` | 1 | `derive` | DTO + config deserialization |
| `toml` | latest | — | `projects.toml` parsing |
| `clap` | 4.6 | `derive` | CLI args |
| `futures` | 0.3 | — | `stream::iter` + `buffered` |
| `thiserror` | 2 | — | typed errors |
| `anyhow` | 1 | — | `main.rs` only |
| `comfy-table` | 7 | default | colored table |
| dev: `wiremock` | latest | — | HTTP mocking |
| dev: `tokio` | same | `macros`, `rt-multi-thread` | async tests |
| dev: `serde_json` | 1 | — | building fixture bodies |

Toolchain: latest stable via `rustup` (≥ 1.96), edition 2024. Add a `rust-toolchain.toml` with `channel = "stable"`.

## 6. Functional-style rules

- Immutable by default: no `mut` bindings unless genuinely needed (e.g. builders), and never shared mutable state.
- Iterators and combinators (`map`, `filter_map`, `and_then`, `map_err`, `collect::<Result<_, _>>`) instead of index loops and early `push`.
- Errors as values: no `unwrap`/`expect`/`panic!` outside tests. Propagate with `?`.
- Pure functions for parsing, mapping, status classification and rendering; effects (file read, env read, HTTP, stdout) only in `config::load_projects`, `GithubClient`, and `main`.
- Make illegal states unrepresentable: newtypes with validating constructors, exhaustive `match` on enums (no `_` wildcard on domain enums).
- Dependency injection through generics (`S: PullRequestSource`), not trait objects or globals.
- Small functions; doc comments on public items.

## 7. Tests

All tests must pass with `cargo test`, offline, without `GITHUB_TOKEN`.

**Unit tests (in-module `#[cfg(test)]`)**
- `domain::model`: valid `owner/repo`; rejects `""`, `"owner"`, `"/repo"`, `"owner/"`, `"a/b/c"`; trims whitespace; `Display` round-trip.
- `adapter::config::parse_projects`: valid file; empty list; invalid entry names the entry; malformed TOML.
- `adapter::github::mapper`: `draft: true` → `Draft`; missing `draft` → `Open`.
- `adapter::github` status mapping: 404, 401, 403 with/without rate-limit header, 429, 500.
- `adapter::cli::render::rows`: no PRs → one `NoPullRequests` row; N PRs → N `OpenPullRequest` rows; error → one `Failed` row with message; order preserved across projects.

**`tests/list_open_prs_service.rs`** — in-memory fake implementing `PullRequestSource`:
- Mixed outcomes (success, empty, error) → reports in input order with the right outcomes.
- **Concurrency cap**: fake tracks in-flight calls with an `AtomicUsize` and a short `tokio::time::sleep`; with 5 projects and `max_concurrency = 2`, observed peak is exactly 2.
- `has_failures` true/false cases.

**`tests/github_client_wiremock.rs`** — `GithubClient` pointed at `MockServer::uri()`:
- 200 with two PRs (one draft) → mapped correctly.
- 200 with `[]` → empty vec.
- Asserts request path `/repos/{owner}/{repo}/pulls`, query `state=open` and `per_page=100`, and headers `Authorization: Bearer test-token`, `Accept`, `X-GitHub-Api-Version`, `User-Agent` present.
- 404 → `NotFound`; 401 → `Unauthorized`; 403 + `x-ratelimit-remaining: 0` → `RateLimited`; 500 → `Unavailable`; malformed JSON → `InvalidResponse`.

## 8. Definition of done

- [ ] `cargo build` and `cargo build --release` succeed with no warnings.
- [ ] `cargo test` passes (offline, no token).
- [ ] `cargo clippy --all-targets -- -D warnings` is clean.
- [ ] `cargo fmt --check` is clean.
- [ ] `GITHUB_TOKEN=... cargo run` against the sample `projects.toml` prints the colored table; `cargo run` without the token exits 2 with a clear message.
- [ ] Domain module has no imports from external HTTP/serialization/CLI crates.
- [ ] `README.md` updated; sample `projects.toml` present.
- [ ] Final summary for the user: what was built, resolved crate versions, anything deviating from this plan and why.

## 9. Out of scope

Pagination, GitHub Enterprise, closed/merged PRs, review or CI status, JSON output, caching, retries, release packaging (Homebrew, universal binaries).
