# Recurring tasks

## Goal

- Read this document and apply the steps to the workspace.
- Present the plan with coded, prioritized actions, to assess them and carry out the ones that add value.

## Code review

- Apply SOLID as it fits Rust, which has traits and no inheritance:
    - One responsibility per module and type.
    - Ports as traits, implemented by adapters and injected with generics (`S: PullRequestSource`).
    - Small traits; dependencies point inward (`main` → adapters → application → domain).
- Follow the official documentation: the Rust Book, the `std` docs, the
  [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/), and each crate's docs.rs page
  for the version in `Cargo.lock`.
- Use functional techniques where they make the code clearer:
    - Bindings without `mut`: functions that take a value and return a new one.
    - Algebraic data types (enums and structs) for every closed set of cases, instead of flags,
      strings or numeric codes.
    - Exhaustive `match`, without `_` on the project's own enums.
    - `Option` and `Result` with their combinators (`map`, `and_then`, `ok_or`, `?`) instead of
      sentinel values, `unwrap`, `expect` or `panic!`.
    - Closures and higher-order functions.
    - Lazy iterator chains (`map`, `filter`, `filter_map`, `flat_map`, `fold`), collected only at
      the end, instead of index loops.
    - `if`, `match` and blocks as expressions.
    - Traits for shared behavior, and standard ones (`Display`, `From`, `FromStr`) over ad hoc
      methods.
    - Type inference, with annotations only where they help the reader or the compiler.
- Use stable, maintained crates; `cargo deny check` and `cargo tree --workspace -d` show advisories
  and duplicates.
- Keep the dependencies up to date. `scripts/check-updates.sh` lists what can move, sorted as minor,
  patch, held and breaking: it runs `cargo update --dry-run` (what can move in `Cargo.lock`) and
  `cargo outdated` (`cargo install cargo-outdated`; what is behind the latest release).
    - Apply the minor and patch bumps with `cargo update`, raise the minimum of each direct
      dependency that moved, and build and test (with the integration tests of
      escrutinio-servicio) before committing.
    - Don't force a transitive dependency that another crate pins with `=`: leave it until that
      crate releases a new version, and don't add it to a `Cargo.toml` or use `[patch]` to move it.
      `cargo update -p <crate> --precise <version>` says which crate pins it. Today: `matchit`
      (`axum` pins `=0.8.4`) and `generic-array` (`crypto-common` pins `=0.14.7`).
    - A major bump, or a minor one of a 0.x crate, can break the API: it goes to Pending issues
      (DEP-n), not in the same commit.
- Avoid deprecated crates, APIs and attributes: the build must have no `deprecated` warnings.
- Avoid code duplication, also between projects (see `libs/` in the root `CLAUDE.md`).

## Continuous integration

- Review the GitHub Actions workflows for efficiency and security, following the official GitHub
  Actions documentation; `zizmor .github/` must report no findings.

## Documentation and cleanup

- Update the documents with the current state of the work done in the session.
- Simplify the texts where possible.
- Check every external link in each document, so none points nowhere.
- Reorganize the sections of a document when its order no longer fits.
- Clean up what is no longer needed: local documents, test leftovers, objects, files and artifacts.

## Pending issues

- **TOOL-1: move from Rust 1.99.1 to stable.** `rust-toolchain.toml` pins 1.99.1 because the
  line coverage of 1.100.0 counts blank lines and test attributes as uncovered. Measured on
  2026-11-28 with `cargo +<toolchain> llvm-cov -p <package> --summary-only` (escrutinio-servicio with
  `ESCRUTINIO_INTEGRACION=1`):

  | Package | 1.99.1 | 1.100.1 |
  |---|---|---|
  | pr-checker | 92.14 % (193/2455 missed) | 89.61 % (255/2455), below the 90 % minimum |
  | severance-pay-calculator | 97.32 % (39/1454) | 92.92 % (103/1454) |
  | calendario-laboral | 94.59 % (372/6873) | 93.19 % (468/6873) |
  | escrutinio | 90.44 % (170/1778) | 90.66 % (166/1778) |
  | escrutinio-simulador | 90.31 % (108/1114) | 86.45 % (151/1114), below the 90 % minimum |
  | escrutinio-servicio | 93.51 % (78/1202) | 94.09 % (71/1202) |
  | escrutinio-contrato | 91.30 % (24/276) | 93.84 % (17/276) |

  1.100.0 (2026-11-15) and 1.100.1 (2026-11-28) still leave pr-checker and escrutinio-simulador below
  the minimum, so nothing changed. Even with 1.99.1, escrutinio and escrutinio-simulador are just
  above it: add tests there before the next measurement. On each new stable release, repeat the
  comparison. When every package keeps its coverage, set `channel = "stable"`, update the toolchain
  in the workflow and the root `CLAUDE.md`, and remove this entry. Don't lower the minimum to get
  there.

## Last check

2026-11-24
