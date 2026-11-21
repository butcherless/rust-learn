# Implementation plan: GitHub open PRs CLI

Requirements and design behind the tool. For setup and usage, see [README.md](README.md). For
commands, code rules and library constraints, see [CLAUDE.md](CLAUDE.md).

**Status (2026-11-21):** FR1-FR17 are implemented on `main` and CI passes. The table mode was run
against the real GitHub API with the 13 repositories in `projects.toml` (exit `0`; one open pull
request, mergeable). The terminal UI has only been checked with a dummy token, not by hand against real
data.

## Requirements

| # | Requirement |
|---|---|
| FR1 | The project list comes from a TOML file: `./projects.toml` by default, or `--config <path>`. Each entry is `owner/repo` (trimmed, both parts non-empty, exactly one `/`). An invalid entry fails loading and is named in the message. An empty list is valid. |
| FR2 | Authentication uses `GITHUB_TOKEN`. If it is missing or empty, the tool exits with a clear error before any HTTP call. |
| FR3 | The target is github.com only, but the adapter's base URL is injectable for tests. |
| FR4 | Each project gets one `GET /repos/{owner}/{repo}/pulls?state=open&per_page=100`, then one `GET /repos/{owner}/{repo}/pulls/{number}` per pull request, at most 4 at a time, because only that endpoint returns `mergeable`. A failing one fails the project. There is no pagination. |
| FR5 | At most 2 repositories are fetched concurrently, and output follows config order, whatever order they finish in. |
| FR6 | A failing repository (404, 401/403, rate limit, network, decode) is reported in its own row, and the others continue. |
| FR7 | Exit code `0`: every project succeeded. `1`: at least one failed, results still shown. `2`: startup error. |
| FR8 | The table columns are Project, PR #, Title, State, Mergeable. |
| FR9 | Green: no open PRs (one row, `no open pull requests`). Orange: one row per open PR. Red: fetch failed (one row, the error message). |
| FR10 | State is `open`, or `draft` when the API's `draft` flag is true. |
| FR11 | `--tui` shows the same rows and colors in an interactive terminal UI. Keys move the selection, open the selected PR's `html_url` in the browser (only `https://` addresses), fetch again, and quit (see the README for the bindings). They work while fetching, except fetch again, which waits for the current fetch to finish. Exit codes follow FR7 using the last fetch; a terminal that cannot be set up exits with `2`. |
| FR12 | Mergeable is `yes` or `no` from the API's `mergeable` flag, or `unknown` while GitHub computes it (`null`). |
| FR13 | In `--tui`, a light (`●`) starts the status line. While fetching it blinks orange (half a second lit, half a second dark) and the status shows the progress (`Loading… 5/13`). When the fetch ends it stays green, or red if a repository failed. |
| FR14 | In `--tui`, each repository's rows appear as soon as it finishes, in config order. On the first fetch a repository not yet finished shows a gray `loading…` row; on a refresh it keeps its previous rows until the new ones arrive. The selection stays on the same pull request when it is still listed, and otherwise on the same line. |
| FR15 | In `--tui`, a Merge column after Mergeable shows a checkbox per pull request: empty and gray (`[ ]`) while open, a green tick (`[✓]`) once merged in this session, and `[~]` in orange while merging. `m` asks for confirmation in the status line, with `[?]` in orange on that pull request, and a second `m` on the same pull request merges it (`merge_method: merge`); any other command cancels. One merge at a time. A merged pull request's row turns green (its inverse-video selection too), with State `merged` and Mergeable `—`, and the summary counts it as merged, not open (`1 open pull requests in 3 projects, 1 merged, 1 failed`), until a fetch drops it. A refused merge (405, 409, 422) is `not mergeable` and leaves the checkbox empty. The table mode has no Merge column. |
| FR16 | In `--tui`, a Close column after Merge shows a checkbox per pull request, like FR15's: `c` asks for confirmation (`Press c again to close owner/repo#N`, `[?]`), a second `c` on the same pull request closes it without merging (`PATCH` with `state: closed`), and `[~]` shows while closing. Merges and closes share the one-at-a-time rule, and the second key must repeat the first one's action. A closed pull request's row turns gray with State `closed`, a red tick in Close and no Merge checkbox (a merged one has no Close checkbox), and the summary counts it as closed, until a fetch drops it. The table mode has no Close column. |
| FR17 | In `--tui`, a Running workflows panel below the pull requests lists the unfinished GitHub Actions runs of each repository (`queued`, `in progress`, or `waiting` for `requested`, `waiting` and `pending`) among its 30 newest, with Project, Workflow, Branch, Event and Status, in config order. In progress is orange, queued and waiting gray, and a repository whose runs fail shows a red error row. Its title counts the runs. It shows `loading…` until the first runs arrive and `No workflows running` when there are none. It takes what the pull requests leave free and at least a third of the screen, and the rows that don't fit are counted in `… N more`. The fetch (and `r`) gets the runs beside the pull requests, each repository's as they arrive, keeping the previous ones until then. |

**Out of scope:** pagination, GitHub Enterprise, listing closed or merged PRs, merge methods other than a merge commit, review or CI status, JSON
output, caching, retries, release packaging.

## Architecture

Hexagonal, with dependencies pointing inward: `adapter → service → domain`. The domain imports only
`std` and `thiserror`.

| Module | Responsibility |
|---|---|
| `domain::model` | `Project` (validated by `FromStr`), `PullRequest`, `PrNumber`, `PrTitle`, `PrState`, `Mergeable`, `PrUrl` |
| `domain::port` | `PullRequestSource`, `PullRequestMerger`, `PullRequestCloser` and `WorkflowRunSource`: return-position `impl Future`, static dispatch, no `dyn` or `async-trait` |
| `domain::error` | `ProjectParseError`, and `SourceError` (`NotFound`, `Unauthorized`, `RateLimited`, `Unavailable`, `InvalidResponse`, `NotMergeable`) |
| `service::running_workflows` | `stream_running_workflows`, like `stream_open_prs` for each project's unfinished workflow runs (`RunsReport`) |
| `service::list_open_prs` | `stream_open_prs` yields each report with its position as soon as it is ready (`buffer_unordered`); `list_open_prs` collects it back into config order. Also `ProjectReport`, `has_failures`, `MAX_CONCURRENCY` |
| `adapter::config` | `load_projects` (reads the file) and `parse_projects` (pure) |
| `adapter::github` | `GithubClient`, `GithubToken` (redacted `Debug`), wire DTO, pure mapper and status mapping |
| `adapter::view` | Pure presentation model shared by both outputs: `rows` (`DisplayRow`: kind, cells, `PrRef`), `HEADER`, `ORANGE` |
| `adapter::cli` | `Args` (clap) and `render` (comfy-table and colors) |
| `adapter::tui` | `app`: The Elm Architecture (`message`, and `App::update` returning the next state and an `Effect`). `view::draw`: the drawing, tested with `TestBackend`. `run`: the loop that performs effects and waits with `tokio::select!` for keys (`EventStream`), the blink clock and the fetch stream, terminal setup and restore, `open::that_detached` |
| `main.rs` | Composition root. The only place `anyhow` is used |

### HTTP

- Endpoints: [List pull requests](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests),
  [Get a pull request](https://docs.github.com/en/rest/pulls/pulls#get-a-pull-request) and
  [Merge a pull request](https://docs.github.com/en/rest/pulls/pulls#merge-a-pull-request)
  (`PUT …/pulls/{number}/merge` with `{"merge_method": "merge"}`; any 2xx means merged) and
  [Update a pull request](https://docs.github.com/en/rest/pulls/pulls#update-a-pull-request)
  (`PATCH …/pulls/{number}` with `{"state": "closed"}`; any 2xx means closed, and errors map as
  in the table below), and
  [List workflow runs for a repository](https://docs.github.com/en/rest/actions/workflow-runs#list-workflow-runs-for-a-repository)
  (`GET …/actions/runs?per_page=30&exclude_pull_requests=true`, keeping the runs whose status is
  not `completed`).
- At most `MAX_IN_FLIGHT` (4) requests per client are in flight, through a `tokio` semaphore that
  every request waits on, because GitHub
  [advises](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api)
  serial requests to avoid secondary rate limits. A fetch of 13 repositories makes about 30
  requests, far below the 900 points per minute and 5,000 requests per hour.
- One `reqwest::Client` is reused, with a 10-second timeout.
- Headers: `Authorization: Bearer <token>`, `Accept: application/vnd.github+json`,
  `X-GitHub-Api-Version: 2022-11-28`, `User-Agent: pr-checker/<crate version>`.

| Response | `SourceError` |
|---|---|
| 404 | `NotFound` |
| 401, or any other 403 | `Unauthorized` |
| 429, or 403 with `x-ratelimit-remaining: 0` | `RateLimited` |
| Other non-2xx | `Unavailable("HTTP <code>")` |
| Transport error | `Unavailable(message)` |
| Undecodable body | `InvalidResponse(message)` |
| 405, 409 or 422 to a merge | `NotMergeable` |

### Design notes

- API version `2022-11-28` is kept. [GitHub supports it](https://docs.github.com/en/rest/about-the-rest-api/api-versions)
  through March 10, 2028, and `2026-03-10` is the newest version.
- `tokio` enables `time` for the blink clock, which the tests also use.
- `html_url` is required in the API response, so a pull request without it fails decoding
  (`InvalidResponse`) instead of showing a row that cannot be opened.
- The browser is launched with `open::that_detached`, so a slow launcher doesn't block the UI.

### Technical limitations

User-facing limitations are in the README. These only affect unusual input:

- `.` or `..` as owner or repo pass validation but are dropped from the URL, so `foo/..` requests
  `/repos/foo/pulls`. The request still goes only to the configured host.
- `buffer_unordered(0)` is undefined in the `futures` docs. The app only ever passes `MAX_CONCURRENCY`.

## Definition of done

- [x] `cargo build -p pr-checker` and `--release` succeed with no warnings.
- [x] `cargo test -p pr-checker` passes offline without a token.
- [x] clippy with `-D warnings` and `cargo fmt --check` are clean.
- [x] Line coverage is at least 90%, enforced in CI.
- [x] Without a token, it exits 2 with a clear message.
- [x] The domain has no HTTP, serialization or CLI imports.
- [x] `README.md` is updated and `projects.toml` is present.
- [x] `cargo pr` prints the colored table from the real API.
- [ ] `cargo pr --tui` checked by hand against real data, including opening a
  pull request in the browser and merging one.
