# pr-checker

A command-line tool that lists the open pull requests of a set of GitHub repositories, as a colored
table or in an interactive terminal UI, where it can also merge or close them.

```
╭────────────────┬───────┬──────────────────────────────┬───────┬───────────╮
│ Project        ┆ PR #  ┆ Title                        ┆ State ┆ Mergeable │
╞════════════════╪═══════╪══════════════════════════════╪═══════╪═══════════╡
│ rust-lang/rust ┆ 12345 ┆ Fix borrowck ICE in closures ┆ open  ┆ yes       │  orange
│ rust-lang/rust ┆ 12350 ┆ Stabilize foo                ┆ draft ┆ no        │  orange
│ tokio-rs/tokio ┆ —     ┆ no open pull requests        ┆ —     ┆ —         │  green
│ acme/missing   ┆ —     ┆ error: repository not found  ┆ —     ┆ —         │  red
╰────────────────┴───────┴──────────────────────────────┴───────┴───────────╯
```

## Quick start

From the workspace root (see its [README](../../README.md#requirements) for requirements), the `cargo pr` alias
runs this project with `apps/pr-checker/projects.toml`:

```bash
export GITHUB_TOKEN=$(gh auth token)   # or: export GITHUB_TOKEN=<your token>
cargo pr                               # print the table
cargo pr --tui                         # open the interactive UI
```

[`gh auth token`](https://cli.github.com/manual/gh_auth_token) prints the GitHub CLI's token.

To run the compiled binary instead:

```bash
cargo build --release -p pr-checker
./target/release/pr-checker --config apps/pr-checker/projects.toml          # table
./target/release/pr-checker --config apps/pr-checker/projects.toml --tui    # interactive UI
```

## Usage

| Flag | Effect |
|---|---|
| *(none)* | Print the table once and exit. Works in scripts and pipes; colors only appear on a terminal. |
| `--tui` | Show the same rows in a full-screen view. Needs a real terminal. |
| `--config <path>` | Read the repositories from `<path>` instead of `./projects.toml`. |

Flags can be combined. The alias already passes `--config`, so for another file run
`cargo run -p pr-checker -- --tui --config path/to/projects.toml`; the `--` passes the flags after it
to the program instead of to `cargo`.

### Interactive UI

Each repository's rows appear as soon as it is fetched, in the order of the file; until then it
shows a gray `loading…` row, or its previous rows when fetching again. The light at the start of
the status line blinks orange while fetching, with the progress (`Loading… 5/13`), and then stays
green, or red if a repository failed. The keys work while fetching.

Merge and Close columns after Mergeable show a checkbox on each pull request: empty and gray,
`[ ]`, while it is open, and a tick, `[✓]`, once you merge it (green) or close it (red) from here.
Both ask first: `m` writes `Press m again to merge owner/repo#N` in the status line, and `c`
writes `Press c again to close owner/repo#N`, turning that checkbox into an orange `[?]`. The same
key again merges it with a merge commit, or closes it without merging; any other key cancels.
While GitHub does it the checkbox shows `[~]` in orange, and only one merge or close runs at a
time.

Once merged, the whole row turns green, the selection included: State says `merged`, Mergeable
`—`, and the summary counts it as merged instead of open. Once closed, the row turns gray, State
says `closed`, and the summary counts it as closed. The other column's checkbox disappears. The row
stays like that until a fetch no longer lists it. If GitHub refuses (conflicts, a draft or a branch
rule for a merge, missing permission for either), the row stays orange and the status line says
why.

| Key | Action |
|---|---|
| `↑` `↓` / `j` `k` | Move the selection |
| `g` `G` / `Home` `End` | Jump to the first or last row |
| `Enter` / `o` | Open the selected pull request in your browser |
| `m` `m` | Merge the selected pull request (the first `m` asks for confirmation) |
| `c` `c` | Close the selected pull request without merging (the first `c` asks for confirmation) |
| `r` | Fetch the pull requests again (ignored while fetching) |
| `q` / `Esc` / `Ctrl+C` | Quit |

## Configuration

**Repositories:** a TOML file with one `owner/repo` entry per repository, in the order to show them.

```toml
projects = [
  "rust-lang/rust",
  "tokio-rs/tokio",
]
```

An invalid entry stops the program with a message naming it. An empty list shows an empty table.

**Token:** the `GITHUB_TOKEN` environment variable is required. For private repositories, a
[personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens)
needs [read access to pull requests](https://docs.github.com/en/rest/authentication/permissions-required-for-fine-grained-personal-access-tokens)
if it is fine-grained, or the `repo` scope if it is classic. Merging from the interactive UI also
needs [write access to contents](https://docs.github.com/en/rest/pulls/pulls#merge-a-pull-request)
for a fine-grained token, and closing needs
[write access to pull requests](https://docs.github.com/en/rest/pulls/pulls#update-a-pull-request);
without them the merge or close fails with `unauthorized`.

## Behavior

- At most 2 repositories are fetched at the same time, and rows follow the order of the file.
- A failing repository is reported in its own red row and doesn't stop the others.
- Mergeable is `no` when the pull request has conflicts with its base branch. It is `unknown`
  while GitHub is still computing it, usually on the first fetch after a change; fetch again
  (`r` in the interactive UI) to see it.
- Each pull request costs one more API request, to get its Mergeable value.

| Exit code | Meaning |
|---|---|
| `0` | Every repository succeeded |
| `1` | At least one repository failed (the results are still shown) |
| `2` | Startup error: configuration, token, or a terminal UI that could not start |

In the interactive UI, the exit code reflects the last fetch before quitting.

**Limitations:**

- Only the first 100 open pull requests of each repository are listed.
- A [secondary rate limit](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
  returned as 403 without `x-ratelimit-remaining: 0` is reported as `unauthorized`.

## Development

Tests run offline, with no token or network: `cargo test -p pr-checker`. The workspace
[README](../../README.md) has the rest of the commands and CI.
[IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) holds the requirements and design, and
[CLAUDE.md](CLAUDE.md) the architecture and code rules.
