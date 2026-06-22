# Fixed-Point Decimal Benchmark Report

Date: 2026-06-20  
Repository: `fixdec`  
Harness: `examples/fair_bench.rs`  

## Setup

Command:

```sh
RUNS=10 ITERS=5000000 CHUNK=1000 cargo run --release --example fair_bench
```

Benchmark configuration:

- 10 independent runs per library/operation.
- 5,000,000 operations per run.
- 1,000 operations per timed chunk.
- `mean ns/op` drops the best and worst run when more than two runs are present.
- `median` is the median run-level ns/op.
- `p90`, `p99`, and `p99.9` are computed over timed chunks.
- Operands are pre-generated outside timing.
- All implementations use the same 256 decimal operand string pairs where possible.
- `black_box` guards inputs and outputs.
- Formatting benchmarks use a reused `String` buffer, so they isolate formatter cost without per-call allocation.

Important caveat: p99.9 is chunk-level latency per operation inside 1,000-operation chunks. It is not single-operation hardware latency.

## Libraries Compared

| Library | Type | Semantics |
|---|---|---|
| `D64` | fixed decimal, 8 dp | exact base-10, `i64`, `Copy`, no heap |
| `D96` | fixed decimal, 12 dp | exact base-10, 96-bit signed range inside `i128`, `Copy`, no heap |
| `f64` | binary float | hardware float, not decimal exact |
| `rust_decimal` | decimal, variable scale | base-10, up to 28 digits, `Copy` |
| `fpdec` | decimal, variable scale | base-10, up to 18 fractional digits, `Copy` |
| `fixed I64F64` | binary fixed-point | base-2 fixed-point, not decimal exact |
| `bigdecimal` | arbitrary precision decimal | heap-backed big integer decimal |

## Results

Mean ns/op, lower is better:

| op | D64 | D96 | f64 | rust_decimal | fpdec | fixed I64F64 | bigdecimal |
|---|---:|---:|---:|---:|---:|---:|---:|
| add | 0.771 | 1.134 | 0.801 | 2.434 | 1.541 | 0.966 | 25.585 |
| sub | 0.760 | 1.140 | 0.785 | 2.543 | 1.543 | 0.968 | 27.047 |
| mul | 2.027 | 4.909 | 0.647 | 2.429 | 2.294 | 1.179 | 15.393 |
| div | 1.994 | 4.935 | 0.613 | 40.979 | 9.946 | 28.300 | 3410.738 |
| parse | 7.580 | 10.437 | 4.864 | 3.174 | 4.936 | 45.459 | 44.620 |
| format | 13.602 | 17.621 | 28.089 | 14.389 | 36.906 | 26.809 | 49.588 |

Throughput, Mops/s, higher is better:

| op | D64 | D96 | f64 | rust_decimal | fpdec | fixed I64F64 | bigdecimal |
|---|---:|---:|---:|---:|---:|---:|---:|
| add | 1297.6 | 881.8 | 1248.1 | 410.9 | 648.8 | 1034.8 | 39.1 |
| sub | 1315.6 | 877.2 | 1273.2 | 393.2 | 648.2 | 1033.5 | 37.0 |
| mul | 493.4 | 203.7 | 1546.6 | 411.8 | 435.9 | 848.3 | 65.0 |
| div | 501.5 | 202.6 | 1632.2 | 24.4 | 100.5 | 35.3 | 0.3 |
| parse | 131.9 | 95.8 | 205.6 | 315.0 | 202.6 | 22.0 | 22.4 |
| format | 73.5 | 56.7 | 35.6 | 69.5 | 27.1 | 37.3 | 20.2 |

## Tail Latency

Chunk-level p99.9 ns/op:

| op | D64 | D96 | f64 | rust_decimal | fpdec | fixed I64F64 | bigdecimal |
|---|---:|---:|---:|---:|---:|---:|---:|
| add | 1.083 | 1.542 | 2.583 | 3.083 | 2.083 | 1.333 | 44.417 |
| sub | 1.000 | 1.542 | 2.542 | 3.375 | 2.042 | 1.250 | 46.542 |
| mul | 2.667 | 10.125 | 0.917 | 9.916 | 3.000 | 1.583 | 37.166 |
| div | 5.791 | 10.000 | 0.875 | 60.292 | 21.875 | 46.583 | 4509.083 |
| parse | 16.458 | 22.625 | 10.541 | 4.209 | 10.375 | 65.334 | 75.291 |
| format | 27.208 | 33.208 | 46.334 | 28.292 | 57.250 | 45.125 | 109.958 |

## Interpretation

- `D64` is the strongest exact decimal option overall in this benchmark. It has near-`f64` add/sub performance and substantially faster decimal division than the general-purpose decimal libraries.
- `D96` is slower than `D64`, as expected from its wider arithmetic, but it still substantially outperforms `rust_decimal`, `fpdec`, `fixed`, and `bigdecimal` for decimal division in this workload.
- `rust_decimal` parses fastest and formats close to `D64`, but division is much slower because it is a general-purpose variable-scale decimal.
- `fpdec` is competitive for add/sub/mul but slower on division and formatting.
- `fixed I64F64` is fast for binary fixed-point arithmetic but is not decimal exact. It should not be treated as semantically equivalent for money or exchange prices.
- `f64` is the hardware speed floor for binary floating point but is not decimal exact.
- `bigdecimal` is not a low-latency contender. It is valuable when arbitrary precision and large dynamic ranges matter more than speed.

## Fairness Notes

- Cross-library division is not perfectly apples-to-apples. `D64` and `D96` use their native fixed scale. `rust_decimal`, `fpdec`, and `bigdecimal` are rounded to 8 decimal places in this harness. `f64` and `fixed` use binary arithmetic.
- Formatting uses reused buffers for all libraries. This measures formatter work and avoids per-call allocation noise.
- Operand magnitudes are intentionally bounded to avoid overflow and keep all libraries on comparable valid inputs.
- `f64` and `fixed` are included as speed references, not decimal correctness references.

