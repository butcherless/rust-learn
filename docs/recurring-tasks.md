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
- Avoid deprecated crates, APIs and attributes: the build must have no `deprecated` warnings.
- Avoid code duplication, also between projects (see `libs/` in the root `CLAUDE.md`).

## Continuous integration

- Review the GitHub Actions workflows for efficiency and security, following the official GitHub
  Actions documentation; `zizmor .github/` must report no findings.

## Pending issues

- **TOOL-1: move from Rust 1.99.1 to stable.** `rust-toolchain.toml` pins 1.99.1 because the
  line coverage of 1.100.0 counts blank lines and test attributes as uncovered. Measured on
  2026-11-15 with `cargo +<toolchain> llvm-cov -p <package> --summary-only`:

  | Package | 1.99.1 | 1.100.0 |
  |---|---|---|
  | pr-checker | 92.69 % (89/1218 missed) | 88.24 % (145/1233), below the 90 % minimum |
  | severance-pay-calculator | 98.24 % (19/1079) | 93.51 % (71/1094) |
  | calendario-laboral | 98.24 % (116/6580) | 93.17 % (466/6823) |

  On 2026-11-21 and 2026-11-24, 1.100.0 was still the latest stable, so nothing changed. On each new stable
  release, repeat the comparison. When every package keeps its coverage, set
  `channel = "stable"`, update the toolchain in the workflow and the root `CLAUDE.md`, and remove
  this entry. Don't lower the minimum to get there.

- **DEP-1: rdkafka 0.39 → 0.40** (librdkafka 2.12 → 2.16), used by escrutinio-simulador and
  escrutinio-servicio. 0.40.0 came out on 2026-11-21, so Dependabot's 7-day cooldown ends on
  2026-11-28. Being 0.x, a minor bump can break the API: read its changelog, update
  `[workspace.dependencies]`, run the integration tests (`ESCRUTINIO_INTEGRACION=1`) and a full
  load against the compose file, then fix the rdkafka notes in both projects' `CLAUDE.md` and
  remove this entry.

## Last check

2026-11-24
