# fixdec API Symmetry Audit — `Option`-returning methods on `D64` / `D96`

**Date:** 2026-06-20
**Scope:** `src/d64.rs` (`D64`, i64-backed) and `src/d96.rs` (`D96`, 96-bit value in i128).
**Audit type:** API symmetry — for every public method returning `Option<…>`, check whether it has, and *should* have:
1. a `try_*` twin returning `crate::Result<…>` (`Result<_, DecimalError>`);
2. a saturating / wrapping / overflowing variant where meaningful;
3. **D64/D96 parity** — same method, same signature shape, on both types;
4. test coverage for zero / negative / min-max / out-of-range inputs.

**Method:** Enumerated every `pub fn` / `pub const fn` on each type via `rg`, normalised the i64/i128 width suffixes, diffed the two method sets, read the actual signatures (not just names), and cross-referenced inline `#[cfg(test)]` modules plus `tests/*.rs` (notably `tests/differential.rs`, `tests/review_round*.rs`, `tests/math_edge_cases.rs`).

**Error type** (`src/lib.rs:97-117`): `DecimalError { Overflow, Underflow, DivisionByZero, InvalidFormat, PrecisionLoss }`, with `pub type Result<T> = …`. So `try_*` twins are expressible for every failure mode below.

**Important crate-specific naming wrinkle:** the `try_` prefix is **overloaded** in this crate. For arithmetic it means "returns `Result`" (`try_add → Result`). But `try_with_scale`, `try_with_scale_lossy`, `try_read_*_bytes`, `try_write_*_bytes` return **`Option`**, not `Result`. This is itself an asymmetry (see F7) and means a name-only audit would be wrong — signatures were read directly.

---

## Table — public `Option`-returning methods

Legend: ✅ has / yes · ❌ missing / no · N-A not applicable · ⚠️ partial. "Edge-tested" = at least zero + negative + overflow/None paths exercised.

| Method (Option-returning) | D64 line | D96 line | `try_*`→Result twin | sat/wrap/overflow | On both types | Edge-tested | Note |
|---|---|---|---|---|---|---|---|
| `checked_add` | d64:474 | d96:617 | ✅ `try_add` | ✅ sat/wrap/ovf | ✅ | ✅ | full family, differential-tested |
| `checked_sub` | d64:529 | d96:684 | ✅ `try_sub` | ✅ sat/wrap/ovf | ✅ | ✅ | full family, differential-tested |
| `checked_mul` | d64:613 | d96:752 | ✅ `try_mul` | ✅ sat/wrap/ovf | ⚠️ | ✅ | **D96 not `const fn`** (F1) |
| `checked_div` | d64:807 | d96:1172 | ✅ `try_div` | ✅ sat/wrap (no ovf — intentional) | ⚠️ | ✅ | **D96 not `const fn`** (F1); no `overflowing_div` is intentional (F-INT) |
| `checked_rem` | d64:897 | d96:1288 | ✅ `try_rem` | N-A | ✅ | ✅ (rem.rs) | rem can't overflow→wrap meaningless |
| `div_rem` | d64:943 | d96:1333 | ❌ | N-A | ✅ | ⚠️ | returns `Option<(int,Self)>`; no `try_div_rem` (F5) |
| `checked_neg` | d64:965 | d96:1521 | ✅ `try_neg` | ✅ sat/wrap | ✅ | ✅ | MIN-negation covered |
| `checked_abs` | d64:1031 | d96:1597 | ✅ `try_abs` | ✅ sat/wrap | ✅ | ✅ | MIN-abs covered |
| `mul_add` | d64:757 | d96:903 | ❌ | N-A | ⚠️ | ⚠️ | **D96 not `const`** (F1); **no D96 inline test** (F4); no `try_mul_add` (F5) |
| `mul_i64`/`mul_i128` | d64:676 | d96:886 | ✅ `try_mul_iN` | N-A | ✅ | ✅ | width-named, expected |
| `add_i64`/`add_i128` | d64:694 | d96:968 | ✅ `try_add_iN` | N-A | ✅ | ✅ | |
| `sub_i64`/`sub_i128` | d64:714 | d96:988 | ✅ `try_sub_iN` | N-A | ✅ | ✅ | |
| `div_i64`/`div_i128` | d64:735 | d96:1009 | ✅ `try_div_iN` | N-A | ✅ | ✅ | |
| `sqrt` | d64:1084 | d96:1652 | ✅ `try_sqrt` | N-A | ✅ | ✅ (sqrt.rs, 50 refs) | well covered |
| `recip` | d64:1458 | d96:2006 | ✅ `try_recip` | N-A | ⚠️ | ✅ | **D96 not `const`** (F1) |
| `powi` | d64:1483 | d96:2026 | ✅ `try_powi` | N-A | ⚠️ | ✅ | **D96 not `const`** (F1) |
| `checked_quantize` | d64:1401 | d96:1953 | ❌ | N-A | ✅ | ⚠️ | no `try_quantize` (F2) |
| `checked_floor_to_tick` | d64:1436 | d96:1986 | ❌ | N-A | ✅ | ⚠️ | no `try_*` (F2) |
| `checked_ceil_to_tick` | d64:1443 | d96:1993 | ❌ | N-A | ✅ | ⚠️ | no `try_*` (F2) |
| `from_basis_points` | d64:324 | d96:453 | ❌ | N-A | ✅ | ✅ (`*_overflow`) | no `try_from_basis_points` (F3) |
| `from_i64`/`from_i128` | d64:1676 | d96:2256/2277 | ✅ `try_from_iN` | N-A | ✅ | ✅ | |
| `from_u64`/`from_u128` | d64:1702 | d96:2303/2309 | ✅ `try_from_uN` | N-A | ✅ | ✅ | |
| `from_f64` | d64:1761 | d96:2377 | ✅ `try_from_f64` | N-A | ✅ | ✅ | |
| `from_f32` | d64:1796 | d96:2418 | ❌ (only f64 has it) | N-A | ✅ | ⚠️ | minor: `try_from_f32` absent both (F6) |
| `percent_of` | d64:1867 | d96:2468 | ❌ | N-A | ✅ | ✅ (inline) | no Result twin; inline-only, no `tests/` (F8) |
| `add_percent` | d64:1874 | d96:2474 | ❌ | N-A | ✅ | ✅ (inline) | no Result twin; inline-only, no `tests/` (F8) |
| `to_i64` (Option) | — | d96:2330 | ❌ | N-A | ❌ D96-only | ✅ | D64 `to_i64` returns `i64` (infallible); D96 returns `Option<i64>` (range-checked). Asymmetric **by design** but see F9 |
| `from_raw_checked` | — | d96:306 | ❌ | N-A | ❌ D96-only | ✅ | **intentional** — D64 `from_raw` is total (F-INT) |
| `try_with_scale` | d64:384 | d96:519 | — (is itself `try_`-named but →Option) | N-A | ✅ | ✅ | misnamed; Result twin is `with_scale` which *panics* not Result (F7) |
| `try_with_scale_lossy` | d64:441 | d96:574 | — | N-A | ✅ | ✅ | same as above (F7) |
| `try_read_le/be/ne_bytes` | d64:2553-2579 | d96:3041-3089 | — (→Option, `try_`-named) | N-A | ✅ | ✅ | misnamed vs arithmetic `try_*` (F7) |
| `try_write_le/be/ne_bytes` | d64:2517-2541 | d96:3149-3169 | — (→`Option<()>`) | N-A | ✅ | ✅ | misnamed (F7) |

---

## Findings by severity

### HIGH

**F1 — D96 lost `const fn` on a cluster of hot-path ops (real D64/D96 divergence).**
`checked_mul` (d96:752), `checked_div` (d96:1172), `mul_add` (d96:903), `recip` (d96:2006), `try_recip` (d96:2017), `powi` (d96:2026), `try_powi` (d96:2059) are **`pub fn`** on D96 but **`pub const fn`** on D64 (d64:613, 807, 757, 1458, 1469, 1483, 1523).
*Gap:* D96 cannot use these in `const` contexts; D64 can. This is exactly the d64/d96 divergence the project warns about, and it is invisible to a name-only diff (same name, same `Option` return). It is plausibly an accident from the wide-integer rewrite (the i128 sqrt/mul paths landed in commits `da49614`/`7a996e1`).
*Recommendation:* decide deliberately — either make the D96 versions `const fn` (preferred, restores parity and matches D64's documented const-friendliness) or document the divergence. **This is a d64/d96 parity issue.** Verify whether the i128 helper(s) they call are const-capable; if a non-const dependency forces it, record that as the reason.

### MEDIUM

**F2 — No `try_*` twin for the quantize / tick family.**
`checked_quantize`, `checked_floor_to_tick`, `checked_ceil_to_tick` (d64:1401/1436/1443, d96:1953/1986/1993) return `Option` only. Tick rounding is a hot path for order placement; callers wanting a typed error (`PrecisionLoss`/`Overflow`) must `.ok_or(...)` by hand.
*Recommendation:* consider `try_quantize` / `try_floor_to_tick` / `try_ceil_to_tick` returning `crate::Result`, *symmetrically on both types*. Case-by-case — only if a caller actually wants the error variant; the `checked_` form is already ergonomic with `?` after `.ok_or`. Not a parity gap (both types match).

**F3 — `from_basis_points` has no `try_` twin.**
`from_basis_points` (d64:324, d96:453) returns `Option`; bps construction overflows for large inputs (`test_from_basis_points_overflow` exists). A `try_from_basis_points → Result` would match the `try_from_i64` family. Medium because bps is common in fee/spread code.
*Recommendation:* add `try_from_basis_points` on both types if the error path matters to callers; otherwise leave. Symmetric today — keep it symmetric.

**F4 — `mul_add` test-coverage parity gap (d64/d96).**
D64 has an inline `test_mul_add_large_exact` (d64:5205); D96 has **no inline `mul_add` test**. The only D96 `mul_add` assertion anywhere is the single MIN-no-panic line in `tests/review_round3.rs:33`. The large-exact / precision path is unverified on D96.
*Recommendation:* mirror `test_mul_add_large_exact` for D96. **Parity issue (coverage).** Cheap, high value given the central duplication hazard.

**F9 — `to_i64_round` exists on D64 but not D96; `to_i64` semantics diverge.**
D64: `to_i64 → i64` (infallible truncation, d64:1748) **and** `to_i64_round → i64` (d64:1783, banker's). D96: `to_i64 → Option<i64>` (range-checked, d96:2330), `to_i128_round → i128` (d96:2341), but **no `to_i64_round`** and no `to_i128` rounding-to-i64. A caller narrowing a D96 to a rounded i64 has no single call.
*Recommendation:* consider `to_i64_round → Option<i64>` on D96 for symmetry with D64's `to_i64_round`. The differing `to_i64` return shape (i64 vs Option) is defensible — D96's domain exceeds i64 so it must be fallible — but should be **documented as intentional**. Partial parity issue.

### LOW

**F5 — No `try_div_rem` / `try_mul_add`.** `div_rem` (d64:943, d96:1333) and `mul_add` return `Option` with no Result twin. Niche; `Option` is fine for both. Nice-to-have only; keep symmetric if added.

**F6 — `try_from_f32` absent on both types.** `from_f32` returns `Option` (d64:1796, d96:2418) but only `from_f64` got a `try_from_f64`. Symmetric *between* D64/D96 (both lack it), just asymmetric *within* the float family. Add `try_from_f32` to both, or leave; low impact.

**F7 — `try_` prefix overloaded: `Option` vs `Result`.** `try_with_scale[_lossy]`, `try_read_*_bytes`, `try_write_*_bytes` are `try_`-prefixed but return **`Option`**, while every arithmetic `try_*` returns **`Result`**. The actual `Result` analog of `with_scale`/`with_scale_lossy` does not exist — those panic-free-but-non-Result constructors are `with_scale` (which *panics*/saturates) and the `try_with_scale*` Option forms. This is a naming-consistency wart, not a behavioural bug, and it is symmetric across D64/D96. Cosmetic; a rename is a breaking change, so likely **document rather than rename**.

**F8 — `percent_of` / `add_percent` tested inline only.** Both have thorough inline `#[cfg(test)]` coverage on each type (zero, negative, overflow — d64:4716-4873, d96:5603-5760) but zero references in `tests/`. Coverage exists and is symmetric; only the *location* differs. No action needed beyond awareness.

### INFO — intentional asymmetries (no action)

- **`from_raw_checked` (D96 only, d96:306).** D96 packs a 96-bit value into an i128, so `from_raw` (d96:296) `assert!`s the 96-bit range and `from_raw_checked` is its non-panicking twin. D64's `from_raw` (d64:186) stores i64 in i64 — total, cannot fail — so a `from_raw_checked` would be pointless. **Justified.**
- **No `overflowing_div` / `overflowing_neg` / `overflowing_abs` on either type** (confirmed 0 occurrences). Division by zero has no wrapped value, and the project documents this as deliberate. `wrapping_div` / `saturating_div` exist where they make sense. **Justified.**
- **`to_d96` (D64→D96) is infallible** (d64:1541, returns `D96`) with no `_round`/`Result` twin — widening conversion is exact. Conversely **`to_d64` / `to_d64_round` (D96→D64) return `Result`** (d96:2090/2112) because narrowing can overflow/lose precision. Correct asymmetry. (Note: D64 has no `to_d96_round` because widening never rounds — correct.)
- **`as_integer_ratio` returns reduced form** (d64:283, d96:399) — documented design choice; both types match.
- **D96 zerocopy/byte-write is write-only by design** per project notes; the `try_write_*` Option forms exist on both.

---

## Summary

- **Public `Option`-returning methods audited:** ~32 distinct (after width-normalisation), across 121 `pub fn` on D64 / 127 on D96.
- **Core arithmetic family (`add/sub/mul/div/neg/abs`)**: full `checked_/try_/saturating_/wrapping_/(overflowing_)` parity on both types, differentially tested (`tests/differential.rs` covers `checked_add/sub/mul/div`). Healthy.
- **Real parity divergences found:** 3 — **F1** (D96 lost `const fn` on mul/div/recip/powi/mul_add — *the* significant one), **F4** (D96 missing `mul_add` precision test), **F9** (`to_i64_round` D64-only). F1 and F4 are the kind of silent d64/d96 drift the project specifically guards against.
- **Missing `try_` twins worth considering:** quantize/tick family (F2), `from_basis_points` (F3) — both symmetric today, decide case-by-case.
- **Naming wart:** `try_*` means `Result` for math but `Option` for scale/bytes (F7) — document, don't rename (breaking).
- **Intentional asymmetries correctly identified:** `from_raw_checked`, absence of `overflowing_div/neg/abs`, infallible widening `to_d96` vs fallible narrowing `to_d64*`.

### Top 3 worth doing
1. **F1 — restore `const fn` on D96 `checked_mul`, `checked_div`, `recip`, `try_recip`, `powi`, `try_powi`, `mul_add`** (or document why not). Highest-value: it's exactly the d64/d96 divergence the project flags, and it's invisible to name-only checks.
2. **F4 — add a D96 `mul_add` precision/large-exact test** mirroring D64's `test_mul_add_large_exact`. One cheap test closes a real coverage-parity hole.
3. **F9 — add `to_i64_round` to D96** (returning `Option<i64>`) and document the intentional `to_i64` Option-vs-i64 difference, so the narrowing-conversion surface is symmetric.
