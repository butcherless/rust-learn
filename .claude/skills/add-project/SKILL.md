---
name: add-project
description: Steps to add a new app (binary package) to this Cargo workspace, with or without its git history.
---

# Adding a project

1. Put it in `apps/<name>/`, where `<name>` is its package name. Bring its history with `git filter-repo --to-subdirectory-filter apps/<name>` (`brew install git-filter-repo`; git's own manual advises against `git filter-branch`) in a fresh clone, then merge it with `--allow-unrelated-histories`, or copy it if it has none.
2. Delete its `Cargo.lock`, `target/`, `rust-toolchain.toml`, `.gitignore`, `.idea/` and CI files; the root ones apply. Add any file it must ignore (such as real data) to the root `.gitignore`.
3. In its `Cargo.toml`, inherit `version`, `edition` and `publish` with `.workspace = true`. Move each dependency that another member already uses to `[workspace.dependencies]`, at the latest version, and check `cargo tree --workspace -d`.
4. Add a run alias to `.cargo/config.toml`, passing any data file by its path from the root.
5. Give it a `README.md` (usage) and a `CLAUDE.md` (commands, architecture, project rules) that link to the root ones instead of repeating them. Add it to the projects table in the root `README.md` and to the list under Layout in the root `CLAUDE.md`.
6. Run fmt, clippy and `cargo llvm-cov -p <name> --fail-under-lines 90`. CI picks it up by itself.
