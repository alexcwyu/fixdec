# Benchmark Coverage for New and Changed APIs

Date: 2026-06-20
Branch: `feature/missing-api`

This phase adds focused Criterion coverage for the APIs added or reworked during
the release-hardening pass. The goal is to provide a repeatable baseline for
future optimisation work. **Timings are informational and do not gate CI**; they
exist to catch regressions and to justify future changes with measurements.

## Machine setup

| | |
|---|---|
| CPU | Apple M5 Max (18 physical / 18 logical cores) |
| OS | macOS 26.5.1 (arm64) |
| Toolchain | rustc 1.96.0 (ac68faa20 2026-05-25), release profile |
| Criterion | 0.7.0 |

Numbers are single-machine, laptop-class silicon, no pinning — treat them as
*relative* signal (path A vs path B, D64 vs D96), not absolute guarantees.

## Commands

```sh
# D64 — new/changed APIs
cargo bench --bench bench_d64 --all-features -- \
  'd64_(round_dp_with_strategy|checked_div_rounded|checked_quantize|add_i64|sub_i64|div_i64|sqrt)'

# D96 — new/changed APIs (incl. small vs wide divide-then-round, both sqrt paths)
cargo bench --bench bench_d96 --all-features -- \
  'd96_(round_dp_with_strategy|checked_div_rounded|checked_quantize|add_i128|sub_i128|div_i128|sqrt)'
```

(Raw run used `--measurement-time 3 --warm-up-time 1`; the point estimate is
Criterion's median. Full-default runs reproduce the same ordering.)

## Raw summary (median)

### In-range common path

| Benchmark | D64 | D96 |
|---|---:|---:|
| `round_dp_with_strategy` (banker's / `MidpointNearestEven`) | 0.99 ns | 3.00 ns |
| `round_dp_with_strategy` (directed / `AwayFromZero`) | 0.73 ns | 2.41 ns |
| `checked_div_rounded` (small) | 3.35 ns | 5.16 ns |
| `checked_quantize` (0.01 tick) | 1.41 ns | 5.36 ns |
| `add_i64` / `add_i128` | 0.89 ns | 1.19 ns |
| `sub_i64` / `sub_i128` | 0.88 ns | 1.18 ns |
| `div_i64` / `div_i128` | 0.97 ns | 2.17 ns |
| `sqrt` (fast path) | 3.63 ns | 10.50 ns |

### Boundary / wide path

| Benchmark | D96 |
|---|---:|
| `checked_div_rounded` (wide, 12 dp, dividend > 2^64) | 19.14 ns |
| `sqrt_wide` (radicand > 2^128 → 192-bit shift-correct) | 26.57 ns |

The wide divide-then-round is ~3.7× the in-range path (5.16 → 19.14 ns) — the cost
of the 192-bit divide that fixed the earlier D96 range cliff. `sqrt_wide` is ~2.5×
the fast `sqrt` (10.50 → 26.57 ns), consistent with the Phase-1 sqrt rework
(the earlier wide path was ~416 ns; the shift-correct path is ~26 ns).

### Allocation vs formatter cost

Covered by the pre-existing `*_formatting` (with per-call `String` alloc) vs
`*_formatting_into_buf` (reused buffer) pair in each bench file — the buffer
variant isolates the formatter from allocation. Not re-run here; unchanged this
phase.

### D64 vs D96

D96 carries a consistent i128-vs-i64 tax on every path: ~1.3× on the integer
helpers, ~3× on strategy rounding, and ~2.9× on fast `sqrt`. The directed
rounding strategy is cheaper than banker's on both types (no midpoint-tie
branch), confirming the dispatch cost is the tie logic, not strategy selection.

## Notes / caveats

- New bench functions live in `benches/bench_d64.rs` and `benches/bench_d96.rs`,
  wired into the existing `criterion_group!`. No new dependencies.
- `d64_sqrt`, `d96_sqrt`, and `d96_sqrt_wide` anchor the fast-path vs wide-path
  comparison for the restored square-root API.
- CI is unaffected: benches are `cargo check --benches`-gated and lint-clean under
  `cargo clippy --all-features --all-targets -- -D warnings`, but timings are not
  asserted.
