# Implementation plan: GitHub open PRs CLI

Requirements and design behind the tool. For setup and usage, see [README.md](README.md). For
commands, code rules and library constraints, see [CLAUDE.md](CLAUDE.md).

**Status (2026-10-31):** implemented in commit `bd12ddd` on local `main`, not pushed (`origin/main`
is at `961dbb9`). Offline checks pass locally, and CI passed on that commit in PR #6, which was
closed unmerged. A run against the real GitHub API has not been recorded.

## Requirements

| # | Requirement |
|---|---|
| FR1 | The project list comes from a TOML file: `./projects.toml` by default, or `--config <path>`. Each entry is `owner/repo` (trimmed, both parts non-empty, exactly one `/`). An invalid entry fails loading and is named in the message. An empty list is valid. |
| FR2 | Authentication uses `GITHUB_TOKEN`. If it is missing or empty, the tool exits with a clear error before any HTTP call. |
| FR3 | The target is github.com only, but the adapter's base URL is injectable for tests. |
| FR4 | Each project gets one `GET /repos/{owner}/{repo}/pulls?state=open&per_page=100`. There is no pagination. |
| FR5 | At most 2 repositories are fetched concurrently, and output follows config order. |
| FR6 | A failing repository (404, 401/403, rate limit, network, decode) is reported in its own row, and the others continue. |
| FR7 | Exit code `0`: every project succeeded. `1`: at least one failed, table still printed. `2`: startup error. |
| FR8 | The table columns are Project, PR #, Title, State. |
| FR9 | Green: no open PRs (one row, `no open pull requests`). Orange: one row per open PR. Red: fetch failed (one row, the error message). |
| FR10 | State is `open`, or `draft` when the API's `draft` flag is true. |
| FR11 | `--tui` shows the same rows and colors in an interactive terminal UI: `↑`/`↓` (or `j`/`k`, `g`/`G`) move, `Enter`/`o` opens the selected PR's `html_url` in the browser (only `https://` addresses), `r` fetches again, `q`/`Esc`/`Ctrl+C` quits. Exit codes follow FR7 using the last fetch; a terminal that cannot be set up exits with `2`. |

**Out of scope:** pagination, GitHub Enterprise, closed or merged PRs, review or CI status, JSON
output, caching, retries, release packaging.

## Architecture

Hexagonal, with dependencies pointing inward: `adapter → service → domain`. The domain imports only
`std` and `thiserror`.

| Module | Responsibility |
|---|---|
| `domain::model` | `Project` (validated by `FromStr`), `PullRequest`, `PrNumber`, `PrTitle`, `PrState`, `PrUrl` |
| `domain::port` | `PullRequestSource`: return-position `impl Future`, static dispatch, no `dyn` or `async-trait` |
| `domain::error` | `ProjectParseError`, and `SourceError` (`NotFound`, `Unauthorized`, `RateLimited`, `Unavailable`, `InvalidResponse`) |
| `service::list_open_prs` | `list_open_prs` uses `buffered`, not `buffer_unordered`, to keep order. Also `ProjectReport`, `has_failures`, `MAX_CONCURRENCY` |
| `adapter::config` | `load_projects` (reads the file) and `parse_projects` (pure) |
| `adapter::github` | `GithubClient`, `GithubToken` (redacted `Debug`), wire DTO, pure mapper and status mapping |
| `adapter::view` | Pure presentation model shared by both outputs: `rows` (`DisplayRow`: kind, cells, URL), `HEADER`, `ORANGE` |
| `adapter::cli` | `Args` (clap) and `render` (comfy-table and colors) |
| `adapter::tui` | `app`: The Elm Architecture (`message`, `App::update` returning the next state and an `Effect`, `App::view`), tested with `TestBackend`. `run`: the `EventStream` loop that performs effects, terminal setup and restore, `open::that_detached` |
| `main.rs` | Composition root. The only place `anyhow` is used |

### HTTP

- Endpoint: [List pull requests](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests).
- One `reqwest::Client` is reused, with a 10-second timeout.
- Headers: `Authorization: Bearer <token>`, `Accept: application/vnd.github+json`,
  `X-GitHub-Api-Version: 2022-11-28`, `User-Agent: rust-learn/<crate version>`.

| Response | `SourceError` |
|---|---|
| 404 | `NotFound` |
| 401, or any other 403 | `Unauthorized` |
| 429, or 403 with `x-ratelimit-remaining: 0` | `RateLimited` |
| Other non-2xx | `Unavailable("HTTP <code>")` |
| Transport error | `Unavailable(message)` |
| Undecodable body | `InvalidResponse(message)` |

### Design notes

- API version `2022-11-28` is kept. [GitHub supports it](https://docs.github.com/en/rest/about-the-rest-api/api-versions)
  through March 10, 2028, and `2026-03-10` is the newest version.
- The `tokio` dev-dependency also enables `time`, for the concurrency test.

### Technical limitations

User-facing limitations are in the README. These only affect unusual input:

- `.` or `..` as owner or repo pass validation but are dropped from the URL, so `foo/..` requests
  `/repos/foo/pulls`. The request still goes only to the configured host.
- `buffered(0)` is undefined in the `futures` docs. The app only ever passes `MAX_CONCURRENCY`.

## Definition of done

- [x] `cargo build` and `cargo build --release` succeed with no warnings.
- [x] `cargo test` passes offline without a token.
- [x] `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` are clean.
- [x] `cargo run` without a token exits 2 with a clear message.
- [x] The domain has no HTTP, serialization or CLI imports.
- [x] `README.md` is updated and `projects.toml` is present.
- [ ] `GITHUB_TOKEN=... cargo run` prints the colored table from the real API.
