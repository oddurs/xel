# xel

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

A local-first macOS spreadsheet where the file is a database.

Excel is the world's most used programming environment, and it stores data in a grid
with no types, no queries, and no version control. xel keeps what makes a spreadsheet
fast to think in — cells, formulas, instant recalculation — and puts a real database
underneath: a workbook is a SQLite file, tables are typed SQL tables, and formulas,
SQL, and Python work over the same data.

## Status

Early development. Nothing is usable yet. Today the repository contains:

- `crates/engine` — the Rust engine. It currently parses and prints A1-style cell
  references within Excel's grid limits.
- `app` — the native macOS app shell (SwiftUI), which opens an empty window.

The design these pieces are building toward lives in [`docs/`](docs/).

## Install

There is no release yet. To build from source you need macOS 26, Swift 6.2+ (Xcode or
the Command Line Tools), and `rustup`:

```sh
git clone https://github.com/oddurs/xel
cd xel
scripts/setup
scripts/task build
swift run --package-path app
```

## Development

All checks run through one entry point, the same one CI and the git hooks use:

```sh
scripts/task check   # format check, lint (warnings are errors), tests, build
```

Work happens on a branch in its own worktree and lands through a pull request:

```sh
scripts/agent doctor                  # verify tools, auth, and hooks
scripts/agent start feat/my-change    # new branch + worktree; prints the path
scripts/agent commit "feat: ..."      # Conventional Commit, validated
scripts/agent pr                      # check, push, open the pull request
scripts/agent done                    # after merge: remove worktree and branch
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full workflow.

## License

[Apache-2.0](LICENSE). Copyright 2026 Oddur Sigurdsson.
