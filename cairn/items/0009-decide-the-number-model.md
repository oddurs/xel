---
id: 9
uid: a8b6d792-977b-47a3-8a64-bd86688957f6
title: Decide the number model
type: spike
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: s
area: formula
---

## Question

Store and compute numbers as IEEE doubles like Excel, as decimals, or doubles plus a decimal currency type? How are values shown so they match Excel's 15 significant digits?

## Why it matters

It fixes the storage type of number columns and the evaluator's arithmetic. Changing it after files exist needs a format migration.

## Timebox

One day. Answer with the decision and the test vectors (0.1+0.2, 1e308, large currency sums) that justify it.

## Answer

**IEEE 754 doubles, exactly as Excel.** No decimal type in v0.1.

- **Storage.** Number columns are SQLite `REAL`. Integers stay exact up to 2^53, which
  covers currency in cents up to about 90 trillion.
- **Arithmetic.** Plain `f64`. Excel uses doubles, so any other model would make xel
  disagree with Excel on the files it imports, which the Excel bridge cannot afford.
- **Non-finite results.** Never stored. Overflow and NaN become `#NUM!`; `-0` is
  normalized to `0` (Excel never shows a negative zero).
- **Display.** Excel's General format: at most 15 significant digits, so
  `0.1+0.2` shows `0.3`. Column display formats round further.
- **Comparison.** Excel treats numbers equal when they agree to 15 significant digits
  (`=0.1+0.2=0.3` is TRUE). The evaluator compares that way; implemented and tested in
  0020.
- **Decimal currency.** Deferred. Excel has none, so it would only add export
  mismatches. Revisit if users report rounding complaints that Excel also has.

Evidence (IEEE doubles, checked with Python's float, which is the same type):

| Expression | Raw double | 15 significant digits |
|---|---|---|
| `0.1+0.2` | 0.30000000000000004 | 0.3 |
| `1e308*10` | inf | must become `#NUM!` |
| `2^53+1` | 9007199254740992 | precision limit, beyond any realistic cents total |
| sum of 0.01 one million times | 10000.0 | 10000 |
| `-0.0` | -0.0 | must show 0 |

Consequences for other items: 0014 stores numbers as `REAL`; 0020 implements
15-digit comparison, `#NUM!` for non-finite results, and zero normalization.
