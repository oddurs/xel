---
id: 19
uid: bdf3b927-73da-4805-858b-dbf5cb544a7b
title: Parse Excel formula syntax
type: feature
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: l
area: formula
---

## Problem

Formulas are text until they are parsed into something the evaluator can run.

## Proposal

A lexer and parser producing an AST for: numbers, strings, booleans, error literals, operators with Excel's precedence (including unary minus before ^, &, %), function calls, structured references (`[@Amount]`, `Orders[Amount]`), and A1 references and ranges (parsed now, evaluated in v0.2). Errors point at a character offset.

## Acceptance criteria

- [x] `=-2^2` parses to `(-2)^2` like Excel, covered by a test
- [ ] Every example formula in the function reference parses, covered by a table-driven test
- [ ] Malformed input returns an error with the offset, never a panic; fuzzed for 10 minutes in CI

## 2026-11-01

Criterion 2: the function reference does not exist yet (generated in v1.0, see the function-reference docs item); real_world_formulas_parse holds a 21-formula corpus that the reference will extend. Criterion 3: run locally rather than in CI. XEL_FUZZ_SECONDS=600 cargo test --release random_input_never_panics: 3,120,100,000 random formulas over the operator/literal/reference alphabet in 600 s, seed 0x551ff457f8541be1, no panic. CI runs the 200,000-input deterministic version on every push. Unsupported shapes (A:A, 1:1, Sheet2!A1) fail cleanly and are filed as 0119.

## Result

crates/engine/src/formula/{ast,parser}.rs: parse(text) -> Expr with Excel precedence (unary minus over ^ over %... see tests), A1/$A$1 cells and ranges, structured refs [@Col]/Table[Col] with ' escapes, error literals, Missing args, Name; ParseError{offset in chars, message}; depth limit 256, length limit 8192.

## 2026-11-01

Correction after review: criterion 3 says 'in CI', and the 10-minute run was local, so it is unticked. Continuous fuzzing in CI belongs to the v1.0 fuzzing and crash-safety item. Review fixes: error offsets now point at the stray token in '(1 2)' and 'SUM(1 2)'; A01 accepted as A1; reversed ranges normalized; ParseError is now a thiserror struct with a typed ParseErrorKind; escaped '#' allowed in column names; only ASCII whitespace separates tokens; $ in a function or table name is an error. Declined: rejecting cell-shaped function names (LOG10, ATAN2, DAYS360 are real functions). Follow-up 0120 for array constants, @, spill refs, #This Row, and newer error values.
