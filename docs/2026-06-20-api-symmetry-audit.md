# fixdec API Symmetry Audit

Date: 2026-06-20

Scope: public `Option`-returning APIs on `D64` and `D96`, with focus on
D64/D96 parity, typed-error twins, overflow-mode families, and edge coverage.

## Current Status

The core arithmetic surface is symmetric and const-friendly on both decimal
types:

| Family | D64 | D96 | Notes |
|---|---|---|---|
| `checked_add/sub/mul/div` | yes | yes | `const fn`; `try_*` twins exist |
| `saturating_add/sub/mul/div` | yes | yes | division by zero returns zero |
| `wrapping_add/sub/mul/div` | yes | yes | D96 wraps inside the 96-bit domain |
| `overflowing_add/sub/mul` | yes | yes | no `overflowing_div`; division by zero has no wrapped result |
| `checked_rem`, `try_rem`, `div_rem` | yes | yes | remainder sign follows the dividend |
| `checked_neg/abs`, `try_neg/abs` | yes | yes | MIN boundary covered |
| `sqrt`, `try_sqrt` | yes | yes | exact floor invariant covered by integer-oracle tests |
| `recip`, `try_recip`, `powi`, `try_powi` | yes | yes | `const fn` on both types |
| `mul_add` | yes | yes | `const fn`; D96 precision tests exist |

The previous const-parity risk on D96 is closed. `tests/const_api.rs` pins
`checked_mul`, `checked_div`, `overflowing_mul`, `mul_add`, `recip`, and `powi`
in const contexts for both types.

## Intentional Asymmetries

- `D96::from_raw_checked` exists because `D96` stores a 96-bit signed domain
  inside `i128`; `D64::from_raw` is total over `i64`.
- `D64::to_i64` is infallible. `D96::to_i64` returns `Option<i64>` because a
  valid D96 value can exceed the `i64` integer range.
- `D64::to_d96` is infallible and exact. `D96::to_d64` and
  `D96::to_d64_round` are fallible because narrowing can overflow or lose
  precision.
- D96's `bytemuck` and `zerocopy` support is write-oriented for raw byte views;
  untrusted byte ingestion must go through checked readers or the rkyv validated
  path so the 96-bit invariant is preserved.

## Optional API Additions

These are not release blockers; add them only if real call sites need them.

| Candidate | Current state | Recommendation |
|---|---|---|
| `try_quantize`, `try_floor_to_tick`, `try_ceil_to_tick` | `checked_*` returns `Option` on both types | Add only if callers need typed `DecimalError`; keep symmetric |
| `try_from_basis_points` | `from_basis_points` returns `Option` on both types | Useful for typed construction errors in fee/spread code |
| `try_div_rem`, `try_mul_add` | `div_rem` and `mul_add` return `Option` on both types | Low value unless callers need distinct error variants |
| `try_from_f32` | `from_f32` returns `Option`; `try_from_f64` exists | Low priority; add to both types or leave absent |
| `D96::to_i64_round` | absent; `D96::to_i128_round` and `D96::to_i64` exist | Consider `Option<i64>` if rounded integer narrowing is common |

## Coverage Notes

- Arithmetic correctness is covered by unit tests, property tests, and
  differential tests against integer or `rust_decimal` oracles.
- `sqrt` has exact floor-invariant coverage for D64 and D96, including negative
  inputs, smallest ULPs, D96 max values, the u128/wide-path boundary, and
  wide-path off-by-one cases.
- Quantization and selectable rounding strategies are tested across signs,
  ties, directed modes, and D96's wide divide-then-round path.
- rkyv, zerocopy, and bytemuck tests cover the D96 range invariant and reject
  out-of-range byte patterns through checked surfaces.

## Release Conclusion

The D64/D96 core API is symmetric where it should be symmetric. Remaining
differences are intentional domain differences or optional typed-error
convenience APIs.
