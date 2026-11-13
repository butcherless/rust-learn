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

## Last check

2026-11-13
