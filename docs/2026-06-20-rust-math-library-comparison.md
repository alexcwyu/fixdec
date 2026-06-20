# Rust Numeric Library Comparison

Date: 2026-06-20

This document compares common Rust numeric choices for financial, trading, cryptocurrency, and general math workloads. The right choice depends more on semantics than raw speed: binary floats, binary fixed-point, fixed-scale decimals, variable-scale decimals, and arbitrary-precision decimals solve different problems.

## Summary

| Library/type | Representation | Best use case | Main caveat |
|---|---|---|---|
| `fixdec::D64` | fixed base-10, 8 decimal places, `i64` | low-latency financial values with 8 dp or less | fixed precision and limited range |
| `fixdec::D96` | fixed base-10, 12 decimal places, 96-bit signed range | low-latency crypto/price values needing more precision/range than `D64` | slower than `D64`; fixed 12 dp |
| `f64` | IEEE-754 binary float | scientific/engineering math, approximate simulation, graphics, ML features | not decimal exact; unsuitable for money settlement |
| `rust_decimal` | base-10 decimal, variable scale up to 28 digits | general-purpose decimal APIs, accounting-style values, serde/database interop | slower arithmetic, especially division |
| `fpdec` | base-10 decimal, variable scale up to 18 fractional digits | decimal values needing flexible scale and better speed than heap decimals | division requires explicit rounding for non-terminating results |
| `fixed` | binary fixed-point | deterministic binary fixed-point DSP/control/game-style math | not base-10 exact; decimals like 0.1 are inexact |
| `bigdecimal` | arbitrary precision decimal over big integers | arbitrary precision, user-entered finance/science values, very large numbers | heap allocation and very slow division |
| `num-bigint` / `rug` | arbitrary precision integers/floats | cryptography, exact integer math, high-precision scientific computation | not low-latency fixed decimal by default |

## Feature Positioning (at a glance)

This condensed trade-off table originated in an OpenAI Codex review and was
independently re-verified against each crate's documentation on 2026-06-20 (every
cell checked accurate; peer strengths are credited, not strawmanned).

| Type | Best use case | Strength | Caveat |
|---|---|---|---|
| `D64` | Prices, rates, fees needing ≤ 8 dp | Very fast, tiny, decimal-exact, `no_std` | Range and scale are fixed |
| `D96` | Larger financial / crypto decimal values needing ≤ 12 dp | Fast decimal with a bigger range | More expensive than `D64`; still fixed 12 dp |
| `fixed` | Binary fixed-point DSP / control / generic numeric code | Huge API, many overflow modes, bit-level control | Not decimal-exact for money |
| `fpdec` | Decimal values with variable fractional digits | Good rounding / quantize / ratio model | Slower and broader than the `D64`/`D96` hot-path style |
| `rust_decimal` | General application / business decimal | Mature feature surface: scientific parsing, `maths`, rounding strategies | Much slower division; heavier abstraction |
| `f64` | Simulation / analytics where tiny binary error is acceptable | Hardware-fast mul/div, rich math | Not exact decimal; NaN/Inf; unsafe for money invariants |

## `fixdec::D64`

`D64` is a fixed-scale decimal with 8 fractional digits stored in an `i64`. It is designed for low-latency arithmetic where the scale is known and uniform.

Best use cases:

- Prices, quantities, fees, rates, and notional values that fit in 8 decimal places.
- Trading/HFT paths where heap allocation and variable-scale normalization are unacceptable.
- Wire-format or database values that already use a fixed decimal scale.
- Deterministic arithmetic where truncation behavior is explicit and stable.

Pros:

- Very fast add/sub/div for exact base-10 decimal.
- Compact `Copy` type.
- No heap allocation.
- Stable canonical representation.
- Simple binary serialization.

Cons:

- Only 8 decimal places.
- Range is much smaller than `D96`.
- Fixed scale means it is not a general decimal algebra type.

Use `D64` when the domain naturally fits 8 dp and latency matters.

## `fixdec::D96`

`D96` is a wider fixed-scale decimal with 12 fractional digits and a 96-bit signed range stored inside an `i128`.

Best use cases:

- Crypto, DeFi, token amounts, and exchange prices requiring more than 8 dp.
- Larger notional values that exceed `D64`.
- Fixed-scale decimal systems where 12 dp is enough and performance still matters.

Pros:

- More precision and range than `D64`.
- Exact base-10 arithmetic.
- No heap allocation.
- Faster decimal division than general-purpose decimal libraries in the benchmark.

Cons:

- Slower than `D64`.
- Still fixed to 12 dp.
- Wider arithmetic has more complex edge cases than `D64`.

Use `D96` when `D64` is too small or not precise enough but the application still needs low latency and deterministic decimal semantics.

## `f64`

`f64` is hardware binary floating point. It is the standard choice for approximate numeric computing, not for exact decimal accounting.

Best use cases:

- Scientific and engineering calculations.
- Graphics, physics, ML features, statistics, and approximate analytics.
- Situations where small binary rounding error is acceptable.

Pros:

- Hardware accelerated.
- Excellent ecosystem support.
- Supports huge dynamic range, infinities, NaN, transcendental functions.

Cons:

- Cannot exactly represent common decimal values such as 0.1.
- Accumulated rounding error can be financially meaningful.
- NaN and signed-zero semantics can surprise ordering and equality logic.

Use `f64` when approximate real-number math is correct for the domain. Avoid it for settlement, accounting, ledgers, and exact price/quantity systems.

## `rust_decimal`

`rust_decimal` is a widely used general-purpose decimal type with variable scale and up to 28 significant decimal digits.

Best use cases:

- Business/accounting applications.
- APIs and databases where a general decimal type is expected.
- Applications prioritizing ergonomic decimal semantics and ecosystem compatibility over absolute speed.

Pros:

- Mature and popular.
- Variable scale.
- Good serde/database ecosystem.
- Good parsing performance in this benchmark.

Cons:

- Arithmetic is slower than fixed-scale specialized types.
- Division is much slower in the benchmark.
- Variable scale can require normalization and rounding decisions.

Use `rust_decimal` for general application-layer decimal values where ergonomics and interoperability matter more than nanosecond latency.

## `fpdec`

`fpdec` is a fixed-point decimal implementation with flexible scale behavior and up to 18 fractional digits.

Best use cases:

- Decimal arithmetic needing more flexible scale than `D64`/`D96`.
- Application paths where `rust_decimal` is too general or too slow, but fixed-scale `fixdec` is too constrained.

Pros:

- Competitive add/sub/mul performance.
- Decimal exactness.
- `Copy` type.

Cons:

- Division of non-terminating decimals needs explicit rounded division.
- Formatting was slower than `D64`, `D96`, and `rust_decimal` in the benchmark.
- Less universal ecosystem footprint than `rust_decimal`.

Use `fpdec` when you want decimal exactness with flexible scale and are willing to manage division rounding explicitly.

## `fixed`

The `fixed` crate provides binary fixed-point numbers such as `I64F64`.

Best use cases:

- DSP, embedded, control systems, simulations, and deterministic binary numeric pipelines.
- Fast fixed-point math where base-2 representation is acceptable or desired.

Pros:

- Fast binary fixed-point arithmetic.
- Deterministic and allocation-free.
- Useful when integer/fractional bit widths are part of the design.

Cons:

- Not decimal exact.
- Common decimal values such as 0.1 are not exactly representable.
- Division can be relatively expensive depending on type width.

Use `fixed` for binary fixed-point problems. Do not treat it as a replacement for base-10 money math unless the domain explicitly accepts binary approximation.

## `bigdecimal`

`bigdecimal` stores arbitrary-precision decimal numbers using heap-backed big integers and scale metadata.

Best use cases:

- User-entered arbitrary-precision decimals.
- Very large or very precise values where fixed width is not enough.
- Offline analytics, conversion tools, or correctness oracles.

Pros:

- Arbitrary precision.
- Handles values far outside fixed-width decimal types.
- Useful as a reference implementation in tests.

Cons:

- Heap allocation.
- Much slower arithmetic, especially division.
- Not appropriate for low-latency inner loops.

Use `bigdecimal` when precision/range matters far more than speed.

## `num-bigint` and `rug`

These are arbitrary-precision math libraries rather than finance-oriented fixed decimal libraries.

Best use cases:

- Cryptographic integer math.
- Exact large integer algorithms.
- High-precision scientific calculations.
- Reference or oracle implementations.

Pros:

- Very broad numeric range.
- Strong fit for algorithms that genuinely require arbitrary precision.

Cons:

- Heap-backed and slower than fixed-width arithmetic.
- Decimal scale semantics must often be built on top.
- Usually too heavy for hot trading or pricing paths.

Use these when arbitrary precision is the primary requirement, not when you need a compact fixed decimal type.

## Selection Guide

| Requirement | Recommended choice |
|---|---|
| Low-latency exact decimal, 8 dp enough | `D64` |
| Low-latency exact decimal, 12 dp or wider range needed | `D96` |
| General app decimal with ecosystem support | `rust_decimal` |
| Flexible decimal scale with competitive arithmetic | `fpdec` |
| Approximate scientific/math workloads | `f64` |
| Deterministic binary fixed-point | `fixed` |
| Arbitrary precision decimal | `bigdecimal` |
| Arbitrary precision integer/scientific math | `num-bigint` / `rug` |

## Practical Recommendation

For trading and exchange systems:

- Use `D64` for the hottest paths when 8 dp and its range are sufficient.
- Use `D96` for crypto or wider fixed-scale markets.
- Use `rust_decimal` at API boundaries if ecosystem compatibility matters.
- Use `bigdecimal` or `rust_decimal` as differential-test or import/export helpers, not hot-path arithmetic.
- Keep `f64` out of exact money paths. It is useful for analytics and speed baselines, not settlement truth.

