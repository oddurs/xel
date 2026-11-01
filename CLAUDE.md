# xel

A local-first macOS spreadsheet where the file is a database. Rust engine in
`crates/`, native Swift app in `app/`. Design documents live in `docs/`.

## Workflow

- Never commit to `main`. It advances only through a merged pull request.
- One unit of work: one worktree, one branch, one pull request. Never share a checkout
  with another task or agent.
- Start work: `scripts/agent start <type>/<slug>`, then `cd` to the printed path.
  `type` is one of `feat fix chore docs perf refactor test`.
- Commit: `scripts/agent commit "<message>"`. Open the PR: `scripts/agent pr`.
  After merge: `scripts/agent done`. Run `scripts/agent doctor` when something is off.
- Use `scripts/agent sync` to rebase onto `main`; do not merge `main` into a branch.

## Checks

Talk to the toolchains only through `scripts/task`:

```
scripts/task fmt        format in place
scripts/task fmt:check  verify formatting
scripts/task lint       clippy and swift-format lint, warnings are errors
scripts/task test       full test suite
scripts/task build      build engine and app
scripts/task check      all of the above; what CI and pre-push run
```

- A branch passes `scripts/task check` before it becomes a pull request.
- Never use `--no-verify`, `|| true`, or `continue-on-error` to get past a failure.
  Fix the cause.
- A bug fix comes with the test that would have caught it.
- If a new tool is needed, wire it into `scripts/task`, never directly into CI or hooks.

## Commits and pull requests

- Conventional Commits: `type(scope)!: subject`, imperative, at most 72 characters, no
  trailing period. The body explains why. Scopes: `engine`, `app`, `ci`, `docs`.
- The PR description fills in the template: the problem, the approach, and where the
  change is weakest.

## Attribution

Never name an AI, assistant, model, or tool vendor as an author or contributor in a
commit, trailer, PR, comment, doc, changelog, or release note. No co-author trailers,
no "generated with" footers, no robot emoji. Strip such lines if a tool inserts them.

## Code

- Comments say why; the code says what.
- No TODO stubs, commented-out code, or abstractions without a second caller.
- A dependency must earn its place; prefer the standard library.
- Handle an error where you can act on it; otherwise propagate it.
- Language rules: `.claude/rules/rust.md`, `.claude/rules/swift.md`.
