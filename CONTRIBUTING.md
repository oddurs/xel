# Contributing

## Setup

```sh
scripts/setup          # installs git hooks and the pinned Rust toolchain
scripts/agent doctor   # reports anything still missing
```

## Workflow

`main` advances only through a merged pull request. Branch protection enforces it on
GitHub; the `pre-push` hook enforces it locally.

1. One unit of work gets one branch, one worktree, and one pull request.
   `scripts/agent start <type>/<slug>` creates the branch from the latest `main` in
   `../.worktrees/xel/<type>/<slug>/`. `type` is one of `feat fix chore docs perf
   refactor test`.
2. Work in that worktree. Never share a checkout between two tasks or two agents.
3. Commit with `scripts/agent commit "<message>"`.
4. `scripts/agent sync` rebases onto `main` when it moves.
5. `scripts/agent ship --body-file <path>` runs the full check, pushes, opens the
   pull request, and turns on auto-merge. It then waits: it rebases the branch on
   GitHub when `main` moves, stops on a failed check or a conflict, and after the
   squash merge removes the worktree and branch. `--no-wait` returns once auto-merge
   is on. Without `--body-file` the body is the empty template; fill it in.
6. For a change that needs review before merging, use `scripts/agent pr` instead and
   run `scripts/agent done` after it merges.
7. `scripts/agent sweep` rebases auto-merge pull requests that fell behind `main` and
   removes worktrees whose pull requests have merged.

Dependabot minor and patch updates merge on their own once CI passes. Major updates
wait for review.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/): `type(scope)!: subject`.
Imperative mood, subject at most 72 characters, no trailing period. The body explains
why; the diff already shows what. The `commit-msg` hook rejects anything else.

## Checks

```sh
scripts/task fmt        # format in place
scripts/task check      # what CI runs: fmt:check, lint, test, build
```

A branch is green before it becomes a pull request. Never bypass hooks with
`--no-verify`. A bug fix comes with the test that would have caught it.

## Review

Pull requests merge by squash. One status check, `required`, must pass, the branch must
be up to date with `main`, and every conversation must be resolved. The project has a
single maintainer, so no approving review is required; the maintainer still reviews
every outside contribution before merging.

## Conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md).
