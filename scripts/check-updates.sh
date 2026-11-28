#!/usr/bin/env bash
# Lists the dependency updates of the workspace, without changing anything:
#
#   minor / patch  what `cargo update` applies (docs/recurring-tasks.md, "Code review")
#   held           a compatible version exists but another crate pins the current one with `=`
#   breaking       needs a new requirement in a Cargo.toml (a major bump, or a minor one of a 0.x crate)
#
# Run it from anywhere: ./scripts/check-updates.sh
set -euo pipefail

cd "$(dirname "$0")/.."

cargo update --dry-run --verbose 2>&1 | awk '
  # "1.2.3+spec-1.1.0" -> "1 2 3"
  function parts(v, a) { sub(/\+.*/, "", v); split(v, a, "."); return a[1] " " a[2] " " a[3] }
  function kind(from, to,   f, t) {
    split(parts(from), f, " "); split(parts(to), t, " ")
    if (f[1] > 0) return (t[1] != f[1]) ? "breaking" : (t[2] != f[2]) ? "minor" : "patch"
    return (t[2] != f[2]) ? "breaking" : "patch"
  }
  function add(k, line) { n[k]++; list[k] = list[k] "  " line "\n" }
  /^ *Updating .* -> / { name = $2; from = substr($3, 2); to = substr($5, 2); add(kind(from, to), name " " from " -> " to) }
  /^ *Removing / { removed = removed " " $2 }
  /^ *Unchanged .*\(available: / {
    name = $2; from = substr($3, 2); to = $5; sub(/^v/, "", to); sub(/\)$/, "", to)
    k = kind(from, to)
    add(k == "breaking" ? "breaking" : "held", name " " from " -> " to)
  }
  END {
    order[1] = "minor"; order[2] = "patch"; order[3] = "held"; order[4] = "breaking"
    for (i = 1; i <= 4; i++) {
      k = order[i]
      printf "%s (%d)\n%s", toupper(k), n[k], list[k]
    }
    if (removed != "") print "REMOVED from Cargo.lock:" removed
    if (n["held"] > 0) print "\nA held crate is pinned by another one: `cargo update --dry-run -p <crate> --precise <version>` says which."
  }'

if command -v cargo-outdated >/dev/null 2>&1; then
  echo
  echo "DIRECT dependencies behind the latest release (cargo outdated):"
  cargo outdated --workspace --root-deps-only 2>&1 | sed 's/^/  /'
else
  echo
  echo "cargo-outdated is not installed: cargo install cargo-outdated"
fi
