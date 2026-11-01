# xel design

This is the target, not a description of today's code. Status lives in the README.

## The pitch

Excel is the most used programming environment in the world, and its data has no
types, no relationships, no queries, and no history. Airtable added types and
relationships but gave up formulas, SQL, local files, and ownership. xel is the tool
in between: a spreadsheet anyone who knows Excel can use within five minutes, on top of
a real database, with a native Mac interface.

## Principles

1. **The file is a database.** A `.xel` file is a SQLite file with a documented,
   versioned schema. Your tables are real SQL tables with the names you gave them.
   `sqlite3 budget.xel 'select * from expenses'` works.
2. **Speaks Excel.** Formulas, keyboard model, fill handle, and number formats behave
   the way an Excel user expects. Anything built in xel can be explained in Excel terms
   and exported to `.xlsx`.
3. **Structure is opt-in, never required.** Start typing in a free grid. Press ⌘T and
   that range becomes a typed table, with types inferred from what you typed. Nobody
   has to design a schema before entering a number.
4. **Everything is inspectable.** Every value can answer "where did you come from?"
   Formulas, queries, and scripts are text you can read, diff, and version.
5. **Native and quiet.** It should feel like Apple shipped it. No ribbon, no web view,
   no custom chrome where a system control exists.

## What we take from Excel

| Keep | Why |
|---|---|
| Formula syntax, `A1` and `$A$1` references, the ~150 functions people actually use | Muscle memory is the adoption path |
| Structured references, `Expenses[Amount]` | Excel's own bridge to columns; maps directly onto SQL |
| `LET` and `LAMBDA` as named functions | Reusable logic without macros |
| Dynamic arrays and spill | The natural shape for query and Python results |
| Keyboard model: arrows, ⌘-arrows, Return/Tab, F2, ⌘D/⌘R fill | Experts never touch the mouse |
| Fill handle, autofill series, Flash Fill | The fastest data entry ever designed |
| Pivot tables, conditional formatting, Goal Seek | The analysis people already know |
| Instant recalculation | The feedback loop that makes spreadsheets addictive |

| Leave behind | Instead |
|---|---|
| Silent type coercion (`MARCH1` becoming a date) | Typed columns; free-grid guesses are shown and reversible |
| Dates as serial numbers | Real date and time types, converted only at the `.xlsx` boundary |
| Merged cells inside data | Merging only in calc blocks; tables stay rectangular |
| VBA | Python and SQL, run locally |
| The 1,048,576-row ceiling for data | Tables are limited by SQLite, not by the grid |
| Copy-pasted `VLOOKUP` chains | Linked records and lookups |

## What we take from Airtable

| Keep | Why |
|---|---|
| Rich field types: single and multiple select, checkbox, date, currency, percent, rating, URL, email, attachment | Types that match how people think, not just number and text |
| Linked records with lookup and rollup fields | Relationships without `VLOOKUP`; stored as real foreign keys |
| Views: grid, kanban, calendar, gallery, form | One table, many saved ways of looking at it |
| Per-view filter, sort, group, and hidden fields | Shareable perspectives that don't change the data |
| Expanded record (a row shown as a form) | Wide tables are unreadable as rows |
| Group headers with summaries, column footers with aggregates | Totals without writing formulas |

| Leave behind | Instead |
|---|---|
| Cloud-only, per-seat pricing, row limits | A local file you own |
| A weak formula language | Excel formulas, SQL, and Python |
| No real querying | SQL query blocks |

## The data environment: beyond both

- **Query blocks.** A SQL `SELECT` over any tables in the workbook, run by DuckDB
  against the SQLite file. The result spills like a dynamic array and recalculates
  when its inputs change. Other formulas can reference it like any table.
- **Python blocks.** Local CPython with the user's own environment. Inputs are tables
  (as pandas or polars frames), output is a table. No cloud round-trip, unlike Python
  in Excel.
- **Sources.** Import CSV, Parquet, JSON, Postgres, or another `.xel` as a refreshable
  source. Each import records its steps as SQL, like Power Query but readable.
- **Lineage.** Select any value and trace it back through formulas, queries, and
  sources. The dependency graph is a view you can open, not hidden engine state.
- **Constraints and checks.** Columns can be unique, required, or range-limited.
  Violations show inline as cell warnings, never as a modal dialog. Workbook-level
  checks (`total_assets = total_liabilities + equity`) run on every recalculation.
- **Column profiles.** Each column header can show its distribution, null count, and
  distinct values at a glance.
- **History.** Every edit is a transaction in the file. Browse versions, name
  snapshots, and compare two versions cell by cell. Formulas and queries also save to
  a text sidecar so they can be reviewed in git.
- **Command palette.** ⌘K reaches every command, function, table, and column.
- **Automation.** The `xel` Python package opens a workbook headlessly or attaches to
  the open document. An MCP server on the same API lets agents read and edit live
  workbooks.

## Excel translation

xel has to work alongside Excel, not just replace it.

- **Import `.xlsx`.** Excel Tables become xel tables with inferred types. Other ranges
  become calc blocks. Formulas are translated. A formula xel can't evaluate is kept
  verbatim with its last value and flagged, never dropped.
- **Export `.xlsx`.** Tables become Excel Tables with structured references. Linked
  records become `XLOOKUP`s. Query and Python results export as values, with their
  source text attached as a cell note. Every export comes with a report listing what
  became static.
- **Clipboard.** Copying from Excel pastes with formulas translated. Copying to Excel
  produces Excel formulas.
- **Excel lens.** Any xel formula, lookup, or rollup can be shown as its Excel
  equivalent. This is how an Excel user learns xel, and how they explain a model to a
  colleague who only has Excel.

## Native, elegant interface

Target macOS 26 and its design language. Use the system wherever it has an answer.

- **Document app.** Built on `DocumentGroup`/`NSDocument`, so autosave, Versions,
  Revert, iCloud Drive, tabs, and window restoration all come from the system.
- **Window structure.** A `NavigationSplitView` with a sidebar for tables, views,
  sheets, and scripts; the content area; and an inspector for formatting, field types,
  and view settings. The toolbar uses Liquid Glass and stays at about seven items.
  Everything else lives in the inspector, menus, and ⌘K.
- **The grid.** A custom AppKit view built on layer-backed tiles, because nothing in
  the system scrolls a million rows smoothly. It must scroll at 120 Hz on ProMotion,
  edit in place with the system field editor, select with the system accent color, and
  honor Dark Mode and Increase Contrast.
- **Typography.** SF Pro for text. Numbers use tabular figures so columns align.
  Formulas use SF Mono with syntax coloring.
- **Formula bar.** Native text editing with autocomplete, inline function signatures,
  and reference highlighting that matches the colored outlines in the grid.
- **Menus.** A complete menu bar with standard shortcuts. Every command is reachable
  from a menu, which makes the app discoverable and accessible.
- **System integration.** Quick Look previews of `.xel` files, Spotlight indexing of
  table contents, App Intents for Shortcuts ("Add a row to Expenses"), the Share menu,
  and Services.
- **Accessibility.** VoiceOver can navigate the grid by row and column and read
  headers. Reduce Motion and the system text size settings are respected.
- **Restraint.** Animation only where it explains a change, such as a spill expanding
  or a row moving into a group. No splash screen, no onboarding carousel.

## Architecture

```
app/ (Swift)                 SwiftUI chrome, AppKit grid, document lifecycle
  └─ Swift bindings (UniFFI)
crates/engine (Rust)         model, formula parser and evaluator, dependency graph,
                             incremental recalculation, storage, xlsx interop
  ├─ SQLite                  the file format and transactional storage
  └─ DuckDB                  analytical execution for query blocks
crates/python (PyO3)         the `xel` Python package; embeds CPython for Python blocks
```

The engine is headless and owns all semantics. The app never computes a value, and
the Python package is the same engine without a window. That keeps one source of truth
for behavior and makes the engine testable without UI.

### File format sketch

- User tables are SQL tables with their user-visible names. Column types are SQLite
  storage types plus xel metadata (select options, currency, format) in `xel_columns`.
- Linked records are foreign-key columns; lookups and rollups are metadata, not stored
  copies.
- Calc blocks store cells in `xel_cells(block, row, col, value, formula)`.
- Formulas, queries, views, and blocks live in `xel_*` tables. `xel_meta` holds the
  format version.
- History is an append-only `xel_changes` log, compactable into snapshots.

## First release

**v0.1: the file is a database.** Create, open, and save `.xel`; typed tables in a
native grid; Excel-style formulas (the 80 most used functions) with incremental
recalc; linked records with lookups; SQL query blocks; CSV and `.xlsx` import; the
`xel` Python package for reading and writing tables.

**v0.2: views and the Excel bridge.** Filter, sort, and group views; kanban and form
views; `.xlsx` export with its report; Python blocks; history and version compare.

**Later.** Pivot tables, charts, sources with refresh, Quick Look and Spotlight, App
Intents, the MCP server.

## Open questions

- **Number precision.** Excel uses IEEE doubles and shows 15 significant digits.
  Matching it keeps results identical across export; decimals would fix `0.1 + 0.2`.
  The likely answer is doubles by default and a decimal type for currency columns.
- **Canvas or flow.** Blocks placed freely on a canvas (like Numbers) are flexible but
  can get messy. Blocks flowing top to bottom (like a notebook) are tidier and easier
  to diff. Prototype both before committing.
- **Trust.** Opening a workbook can run Python. It needs a trust prompt like Excel's
  "Enable Content", plus a sandboxed default.
- **Collaboration.** Out of scope for now. SQLite plus a change log leaves room for
  sync later.
