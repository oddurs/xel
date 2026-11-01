# xel

A local-first macOS spreadsheet where the file is a database. Rust engine in
`crates/`, native Swift app in `app/`. Design documents live in `docs/`.

## Workflow

- Never commit to `main`. It advances only through a merged pull request.
- One unit of work: one worktree, one branch, one pull request. Never share a checkout
  with another task or agent.
- Pick work from cairn (`cairn next`, `cairn claim <id>`), then start it with
  `scripts/agent start <type>/<id>-<slug>`, for example `feat/0013-open-workbooks`, and
  `cd` to the printed path. `type` is one of `feat fix chore docs perf refactor test`.
  Spikes use `chore/`. Close the item in the same PR that finishes it.
  Do not run `cairn render` in an item PR; `ROADMAP.md` is re-rendered in its own
  `docs: render roadmap` PR, because rendering in every PR makes them conflict.
- Commit: `scripts/agent commit "<message>"`.
- Finish: `scripts/agent ship --body-file <path>`, with the body written from the PR
  template. It opens the PR, enables auto-merge, keeps the branch current, waits for
  CI, and removes the worktree after the merge. If it stops, fix what it reports and
  run `ship` again.
- Use `scripts/agent pr --body-file <path>` instead when the change needs a human
  decision before merging (design, scope, anything irreversible), and say so in the PR.
- `scripts/agent sweep` cleans up merged worktrees and updates stalled auto-merge PRs.
  Run `scripts/agent doctor` when something is off.
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
  trailing period. The body explains why. Scopes: `engine`, `bridge`, `app`, `ci`, `docs`.
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

<!-- cairn:begin -->
## Roadmap and issues

This project tracks its roadmap and issues with `cairn`. Every item is a Markdown file under `cairn/items`, described by the schema in `cairn.toml`.

**Do not create ad-hoc TODO, PLAN or NOTES files.** Create a cairn item instead, so the work appears on the board and in the generated roadmap.

### The loop

1. `cairn next` — what is ready to start. It excludes anything blocked by unfinished dependencies and puts work already in progress first.
2. `cairn claim <ID>` — take it before you start, so no one duplicates the work. `cairn claim --next` picks and claims the top-ranked unclaimed item in one step. Then read `cairn prompt <ID>`: the item with everything it rests on — the outcome it serves, what its dependencies concluded, what done means and what earlier runs learned.
3. Do the work. Record what you learn: `cairn set <ID> <field>=<value>` for fields, `cairn note <ID> "<TEXT>"` for anything that needs a sentence — why you chose something, what you tried, what to watch for.
4. `cairn tick <ID> <N>` as each acceptance criterion becomes true — `cairn show <ID> --criteria` lists them numbered. Tick what is true, not what would let you close.
5. `cairn close <ID>` when it is done, or `cairn release <ID>` to hand it back.
6. `cairn check` before you report finished. It must pass.

### Commands

```sh
cairn next --json                 # ready work, ranked
cairn claim --next                # take the next ready item
cairn search <TEXT> --json        # titles, bodies and labels
cairn list --json                 # all open items
cairn list --filter 'blocked=false,priority=p0'
cairn prompt <ID>                 # the item as a prompt, with what it rests on
cairn show <ID> --json            # one item, including its body
cairn new "<TITLE>" --type <TYPE> --milestone <MILESTONE>
cairn set <ID> status=<STATUS>    # also labels+=x, or any field below
cairn note <ID> "<TEXT>"          # append reasoning; never replaces
cairn show <ID> --criteria        # acceptance criteria, numbered
cairn tick <ID> <N>               # tick one; --all for every one
cairn close <ID>
cairn check                       # validate; run before finishing
cairn render                      # regenerate ROADMAP.md
```

Claims coordinate writers in the same item directory and are seen across the worktrees of this repository: `next` leaves out what another worktree has claimed, `claim` refuses it, and `cairn worktrees` shows what each is doing. Separate clones are not read. Agree on assignments before splitting work across clones.

Items are numbered: `0012`, and commands accept the bare number too. Write the number in `depends_on` and other id references. Each item also carries a `uid` tag; leave it alone. If a merge gives two items one number, `cairn renumber` moves the one that arrived and retargets the references that came with it.

### Schema

- **Types**: `feature`, `bug`, `spike`, `chore`, `docs`, `milestone`
- **Statuses**: `backlog` (open), `planned` (open), `doing` (active), `blocked` (active), `done` (done), `dropped` (dropped)
- **`due`**: date, YYYY-MM-DD — when a milestone is meant to land
- **`part_of`**: names any items, by id, several allowed — a larger piece of work this belongs to
- **`priority`**: one of p0, p1, p2, p3 — p0 is a release blocker
- **`effort`**: one of s, m, l, xl — Rough size, not an estimate
- **`area`**: one of format, formula, recalc, query, interop, python, bridge, grid, app, infra, docs — Subsystem this touches. format: the .xel SQLite schema and storage. formula:
parser, evaluator, function library. recalc: dependency graph and incremental
recalculation. query: SQL blocks and DuckDB. interop: CSV, xlsx, clipboard. python:
the xel package and Python blocks. bridge: Rust-to-Swift bindings. grid: the AppKit
grid view. app: SwiftUI shell, documents, inspector, menus. infra: CI, packaging,
release. docs: user and contributor documentation.
- **Milestones**: `v0.1` (due 2027-01-31), `v0.2` (due 2027-04-15), `v0.3` (due 2027-06-30), `v0.4` (due 2027-09-15), `v1.0` (due 2027-11-30), `later`
- **Saved views** (`cairn list --view NAME`): `now`, `next`, `triage`

### Rules

1. Before starting work, find or create the item and set it to an active status.
2. Use the fields above rather than inventing new ones; add new fields to `cairn.toml` first.
3. Never hand-edit the generated roadmap file — change items and run `cairn render`.
4. `cairn check` must pass before the work is considered done.

<!-- cairn:end -->
