# fixdec - v5

**High-performance fixed-point decimal arithmetic for financial calculations and cryptocurrency.**

`fixdec` provides two blazingly fast decimal types with **fixed precision**, optimized for performance-critical applications where the precision requirements are known at compile time. If you need configurable precision at runtime, use [`rust_decimal`](https://crates.io/crates/rust_decimal). If you need maximum speed with fixed precision, use `fixdec`.

## When to Use fixdec

✅ **Use fixdec when:**
- You need **maximum performance** in financial calculations
- Your precision requirements are **fixed and known** (8 or 12 decimal places)
- You're building **high-frequency trading** systems
- You're working with **cryptocurrency** (DeFi, DEX, pricing engines)
- You need `no_std` support for embedded or WASM
- You want exact decimal arithmetic without floating-point errors

❌ **Don't use fixdec when:**
- You need **configurable precision** at runtime
- You need more than 12 decimal places
- You need arbitrary precision arithmetic
- Performance is not a critical concern

## Performance

Each value is a single machine integer, so `fixdec`'s hot-path add/sub is on par
with raw `f64` while staying exact in base-10. Measured with
`cargo run --release --example bench_vs_libs` (1,000,000 ops/op, all libraries
parsing the same operands, `black_box`-guarded). Numbers are **indicative** and
vary by machine (these are from an Apple Silicon laptop):

| Operation      |  D64 (8dp) | D96 (12dp) | rust_decimal | fpdec  | bigdecimal |    f64 |
|----------------|-----------:|-----------:|-------------:|-------:|-----------:|-------:|
| Addition       |   ~0.6 ns  |   ~0.7 ns  |     ~2.4 ns  | ~1.2ns |   ~26 ns   | ~0.6ns |
| Subtraction    |   ~0.6 ns  |   ~0.7 ns  |     ~2.4 ns  | ~1.2ns |   ~27 ns   | ~0.6ns |
| Multiplication |   ~1.9 ns  |   ~5.4 ns  |     ~2.4 ns  | ~2.1ns |   ~15 ns   | ~0.6ns |
| Division       |   ~2.0 ns  |   ~8.4 ns  |     ~44 ns   | ~12 ns | ~3500 ns   | ~0.6ns |

Against `rust_decimal`, **D64 is ~4× faster on add/sub and ~22× faster on
division**. The example also benchmarks the base-2 `fixed` crate. Run it yourself
with `cargo run --release --example bench_vs_libs`.

> `f64` is shown only as the hardware speed ceiling — it is **not** decimal-exact
> (`0.1` is inexact) and must never be used for money or prices.

## Types

### `D64` - Traditional Finance
- **Storage**: 64-bit (8 bytes)
- **Precision**: 8 decimal places (0.00000001)
- **Range**: ±92,233,720,368.54775807 (±92 billion)
- **Use cases**: Traditional financial applications & trading systems

### `D96` - Cryptocurrency
- **Storage**: 128-bit (16 bytes, but only 96 bits used)
- **Precision**: 12 decimal places (0.000000000001)
- **Range**: ±39,614,081,257,132,168.796771975167 (±39.6 quadrillion)
- **Use cases**: Cryptocurrency pricing, DeFi protocols, gas calculations, extreme price ranges

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
fixdec = "0.1"
```

### Basic Usage

```rust
use fixdec::D64;
use core::str::FromStr;

// Create from strings
let price = D64::from_str("1234.56")?;
let quantity = D64::from_i32(100);

// Fast arithmetic
let total = price * quantity;
assert_eq!(total.to_string(), "123456");

// Checked arithmetic
let result = price.checked_mul(quantity).ok_or("overflow")?;

// Financial operations (percent_of returns Option)
let fee = total.percent_of(D64::from_str("0.1")?).ok_or("overflow")?; // 0.1% fee
```

### Cryptocurrency Example

```rust
use fixdec::D96;
use core::str::FromStr;

// High precision for crypto
let eth_price = D96::from_str("2500.123456789012")?;
let amount = D96::from_str("0.5")?;
let total_value = eth_price * amount;

// Built-in crypto constants
let gas_price = D96::from_i64(50).unwrap() * D96::GWEI; // 50 gwei (from_i64 is range-checked)
let tx_value = D96::from_str("0.00000001")?;   // 1 satoshi equivalent
```

## Features

### Core Features
- **Fixed precision, maximum speed**: Compile-time precision means zero runtime overhead
- **Exact decimal math**: No floating-point rounding errors
- **Comprehensive arithmetic**: Checked, saturating, and wrapping variants
- **Financial constants**: Basis points, bond fractions (32nds, 64ths), percentages
- **Crypto constants**: Satoshi, gwei, microGwei for blockchain applications

### Optimizations
- **Reciprocal multiplication**: Uses "magic division" with precomputed constants
- **Fast string parsing**: SWAR (SIMD Within A Register) techniques
- **Branchless validation**: Optimized digit checking
- **Binary serialization**: Raw integer encoding for minimal overhead

### Platform Support
- **`no_std` compatible**: Works in embedded systems and WebAssembly
- **Optional `alloc`**: For `Vec` and `String` support
- **Optional `std`**: For `Error` trait and standard library features
- **Serde support**: Efficient JSON and binary serialization

## Feature Flags

```toml
[dependencies]
fixdec = { version = "0.1", features = ["serde"] }
```

| Feature | Description |
|---------|-------------|
| `default` | No additional features (pure `no_std`) |
| `alloc` | Enable `Vec` and `String` support |
| `std` | Enable standard library and `Error` trait |
| `serde` | Enable Serde serialization (requires `alloc`) |
| `bytemuck` | Zero-copy byte casts: `Pod` for `D64`, write-only `NoUninit` for `D96` (`no_std`) |
| `zerocopy` | Byte-view derives: full `FromBytes`/`IntoBytes` for `D64`, write-only `IntoBytes` for `D96` (`no_std`) |
| `num-traits` | `Zero`/`One`/`Bounded`/`Signed`/`Num`/`Checked*`/`Saturating`/`From`/`ToPrimitive`/`Inv` impls (`no_std`) |
| `rust-decimal` | Conversions to/from [`rust_decimal`](https://crates.io/crates/rust_decimal)'s `Decimal` (`no_std`, requires `alloc`) |
| `pyo3` | [PyO3](https://crates.io/crates/pyo3) interop: D64/D96 ↔ Python `decimal.Decimal` (`std`-only; not in `full`) |
| `full` | Enable all pure-Rust features (everything except `pyo3`) |

### rust_decimal interop (`rust-decimal`)

Convert between `fixdec` types and `rust_decimal::Decimal`. Widening is exact and
infallible (`From`); narrowing is fallible (`TryFrom`) and mirrors the crate's own
strict-vs-rounding convention:

```rust
use core::str::FromStr;
use fixdec::D64;
use rust_decimal::Decimal;

// D64/D96 -> Decimal is always exact:
let d = D64::from_str("123.45").unwrap();
let dec: Decimal = d.into();              // or d.to_rust_decimal()

// Decimal -> D64: strict refuses extra precision, rounding keeps it:
assert!(D64::from_rust_decimal(Decimal::from_str("0.000000001").unwrap()).is_err()); // PrecisionLoss (9th dp)
assert_eq!(
    D64::from_rust_decimal_round(Decimal::from_str("0.000000001").unwrap()).unwrap(),
    D64::ZERO,                            // banker's-rounded to 8 dp
);
// Out-of-range reports Overflow / Underflow by sign.
```

### PyO3 interop (`pyo3`)

With the `pyo3` feature, `D64`/`D96` can be passed to and returned from
`#[pyfunction]`s directly — they convert to/from Python's exact `decimal.Decimal`
(and accept `int`, `float`, or `str` on the way in):

```rust,ignore
use fixdec::D64;
use pyo3::prelude::*;

#[pyfunction]
fn add_fee(price: D64, fee: D64) -> D64 {
    // `price`/`fee` arrive as Python Decimals/ints/floats/strs; the return value
    // becomes a Python `decimal.Decimal`, so no precision is lost either way.
    price.saturating_add(fee)
}
```

`float` arguments go through `from_f64` (rounded to the type's precision); a
`Decimal`/`str` with more precision than the type holds raises `ValueError`. The
feature is `std`-only (PyO3 links libpython), so it is not part of `full`.

### Zero-copy (bytemuck / zerocopy)

Both `D64` and `D96` are `#[repr(transparent)]` plain-old-data, so with the
`bytemuck` or `zerocopy` feature you can reinterpret raw byte buffers as decimals
with **no parsing and no allocation** — ideal for binary market-data feeds and
memory-mapped tick stores:

```rust
// with features = ["bytemuck"]
let prices: &[D64] = bytemuck::cast_slice(&packet_bytes);  // zero copy
let bytes:  &[u8]  = bytemuck::cast_slice(&price_slice);

// with features = ["zerocopy"]
use zerocopy::{FromBytes, IntoBytes};
let bytes = price_slice.as_bytes();
let back  = <[D64]>::ref_from_bytes(bytes).unwrap();
```

> **`D96` is write-only for these traits.** `D64` uses all 64 bits, so any byte
> pattern is a valid `D64` and the *bytes → value* direction is sound. `D96`
> stores its value in only 96 of 128 bits, and its arithmetic relies on that
> range — so reinterpreting arbitrary bytes as a `D96` could smuggle an
> out-of-range value into a multiply and corrupt the result. `D96` therefore
> implements only the *value → bytes* direction (`bytemuck::NoUninit`,
> `zerocopy::IntoBytes`); to decode bytes into a `D96`, use the **checked**
> `D96::try_read_le_bytes` / `try_read_be_bytes` / `try_read_ne_bytes` (which
> return `None` on an out-of-range pattern) or `from_*_bytes` (which panics on
> one).

### Generic numerics (num-traits)

With the `num-traits` feature, `D64` and `D96` implement the standard
[`num-traits`](https://docs.rs/num-traits) abstractions, so they drop into
generic numeric code (`Zero`, `One`, `Bounded`, `Signed`, `Num`, `CheckedAdd`/
`CheckedSub`/`CheckedMul`/`CheckedDiv`, `Saturating`, `FromPrimitive`,
`ToPrimitive`, and `Inv`):

```rust
// with features = ["num-traits"]
use num_traits::{Zero, Signed};

fn sum<T: Zero + Copy>(xs: &[T]) -> T {
    xs.iter().fold(T::zero(), |acc, &x| acc + x)
}

let total = sum(&[D64::from_i32(1), D64::from_i32(2), D64::from_i32(3)]);
assert_eq!(total, D64::from_i32(6));
assert_eq!(Signed::signum(&D64::from_i32(-5)), -D64::ONE); // sign as a decimal
```

`Num::from_str_radix` only accepts `radix == 10` (any other radix returns
`DecimalError::InvalidFormat`), and `Inv::inv` panics on zero — use
[`recip`](https://docs.rs) for a non-panicking `Option`. `Float`/`Real`/
`Integer` are intentionally not implemented.

## API Overview

### Construction

```rust
// From integers
D64::from_i32(42)           // Always succeeds
D64::from_i64(1000000)?     // Checked, may overflow
D64::from_u64(1000000)?     // Checked

// From strings (exact)
D64::from_str("123.45")?                    // Errors if > 8 decimals
D64::from_str_lossy("123.456789123")?      // Rounds to 8 decimals

// From floats (lossy)
D64::from_f64(123.45)?      // May lose precision

// From raw scaled values (advanced)
D64::from_raw(12345000000)  // 123.45 in raw form

// From mantissa and scale (rust_decimal compatibility)
D64::with_scale(12345, 2)             // 123.45 (returns Self; panics if out of range)
D64::try_with_scale(12345, 2)         // -> Option<D64> (non-panicking)
```

### Arithmetic Operations

```rust
// Standard operators (panic on overflow / zero divisor)
let z = x + y;
let z = x - y;
let z = x * y;
let z = x / y;
let z = x % y;  // remainder: 10.5 % 3 == 1.5 (exact; sign follows dividend)

// Remainder helpers
x.checked_rem(y)      // None if y is zero
x.is_multiple_of(y)   // true if x is an exact multiple of y
x.div_rem(y)          // Some((integer_quotient, remainder)) or None

// Checked (returns Option)
x.checked_add(y)?
x.checked_sub(y)?
x.checked_mul(y)?
x.checked_div(y)?

// Saturating (clamps to min/max)
x.saturating_add(y)
x.saturating_sub(y)
x.saturating_mul(y)
x.saturating_div(y)

// Wrapping (wraps on overflow)
x.wrapping_add(y)
x.wrapping_sub(y)
x.wrapping_mul(y)
x.wrapping_div(y)

// Fast integer multiplication (quantity * price)
price.mul_i64(quantity)?

// Fused multiply-add (one rounding step)
x.mul_add(y, z)?  // (x * y) + z
```

### Type Conversions (D64 ↔ D96)

```rust
// Widen D64 (8 decimals) -> D96 (12 decimals): always exact and infallible
let wide: D96 = D64::from_str("1234.56")?.into();   // or d64.to_d96()

// Narrow D96 -> D64: fallible (8 decimals max, smaller range)
let narrow: D64 = D96::from_str("1234.56")?.try_into()?;   // exact, else PrecisionLoss / Overflow
let rounded = D96::from_str("1.123456785")?.to_d64_round()?; // banker's-rounds the extra digits
```

### Rounding

```rust
let x = D64::from_str("123.456789")?;

x.floor()         // 123.00000000
x.ceil()          // 124.00000000
x.round()         // 123.00000000 (banker's rounding)
x.round_dp(2)     // 123.46000000 (round to 2 decimals)
x.trunc()         // 123.00000000 (truncate)
x.fract()         // 0.45678900 (fractional part)
```

### Financial Operations

```rust
// Basis points (1 bp = 0.0001)
let rate = D64::from_basis_points(50)?;  // 0.005 (50 bps)
let bps = rate.to_basis_points();        // 50

// Percentage calculations
let tax = price.percent_of(D64::from_str("8.5")?)?;      // 8.5% of price
let with_markup = price.add_percent(D64::from_str("10")?)?; // price * 1.10

// Financial constants
D64::BASIS_POINT          // 0.0001 (1 bp)
D64::HALF_BASIS_POINT     // 0.00005
D64::THIRTY_SECOND        // 0.03125 (US Treasury bond tick)
D64::SIXTY_FOURTH         // 0.015625
D64::CENT                 // 0.01 (1 cent)
D64::PERCENT              // 0.01 (1%)
```

### Mathematical Operations

```rust
// Square root
let sqrt = x.sqrt()?;

// Integer powers
let squared = x.powi(2)?;
let cubed = x.powi(3)?;

// Reciprocal
let recip = x.recip()?;  // 1/x

// Absolute value
let abs = x.abs();

// Sign
let sign = x.signum();  // -1, 0, or 1
```

### Serialization

With the `serde` feature enabled:

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Trade {
    price: D64,
    quantity: D64,
}

// JSON: serializes as a string (Display trims trailing zeros).
// With price = 1234.56 and quantity = 100:
let json = serde_json::to_string(&trade)?;
// {"price":"1234.56","quantity":"100"}

// Deserialize accepts BOTH quoted strings and bare JSON numbers, so payloads
// like {"price": 1234.56, "quantity": 100} from non-fixdec producers also parse
// (numbers are rounded to the type's precision; the canonical form is a string).

// Bincode: uses raw i64 (extremely fast)
let bytes = bincode::serialize(&trade)?;
// Just 16 bytes (8 bytes per D64)
```

### Constants

#### D64 Constants
```rust
D64::ZERO                    // 0
D64::ONE                     // 1.0
D64::TEN                     // 10.0
D64::HUNDRED                 // 100.0
D64::THOUSAND                // 1000.0

// Currency
D64::CENT                    // 0.01
D64::MIL                     // 0.001

// Basis points
D64::BASIS_POINT             // 0.0001
D64::HALF_BASIS_POINT        // 0.00005

// Bond pricing
D64::THIRTY_SECOND           // 1/32
D64::SIXTY_FOURTH            // 1/64

// Legacy equity fractions
D64::EIGHTH                  // 1/8
D64::SIXTEENTH               // 1/16
```

#### D96 Constants (includes all D64 constants plus)
```rust
// Cryptocurrency
D96::SATOSHI                 // 0.00000001 (Bitcoin)
D96::GWEI                    // 0.000000001 (Ethereum gas unit)
D96::MICRO_GWEI              // 0.000000000001 (minimum precision)
D96::KILO_WEI                // 0.000000000001 (1000 wei)
```

## no_std Usage

`fixdec` works in `no_std` environments by default:

```rust
#![no_std]

use fixdec::D64;

// All core operations work without std
let x = D64::from_i32(42);
let y = D64::from_i32(10);
let z = x / y;

// String formatting requires alloc
#[cfg(feature = "alloc")]
extern crate alloc;
```

## Real-World Examples

### Portfolio P&L Calculation
```rust
use fixdec::D64;

struct Position {
    symbol: &'static str,
    quantity: i64,
    entry_price: D64,
    current_price: D64,
}

fn calculate_pnl(position: &Position) -> D64 {
    let entry_value = position.entry_price.mul_i64(position.quantity).unwrap();
    let current_value = position.current_price.mul_i64(position.quantity).unwrap();
    current_value - entry_value
}

let pos = Position {
    symbol: "AAPL",
    quantity: 1000,
    entry_price: D64::from_str("150.25")?,
    current_price: D64::from_str("155.75")?,
};

let pnl = calculate_pnl(&pos);
assert_eq!(pnl.to_string(), "5500");  // $5,500 profit
```

### DeFi Token Swap Calculation
```rust
use fixdec::D96;

fn calculate_swap_output(
    amount_in: D96,
    reserve_in: D96,
    reserve_out: D96,
    fee_bps: i64,  // e.g., 30 for 0.3%
) -> Option<D96> {
    let fee_multiplier = D96::from_basis_points(10000 - fee_bps)?;
    let amount_in_with_fee = amount_in.checked_mul(fee_multiplier)?
        .checked_div(D96::from_i32(10000))?;
    
    let numerator = amount_in_with_fee.checked_mul(reserve_out)?;
    let denominator = reserve_in.checked_add(amount_in_with_fee)?;
    
    numerator.checked_div(denominator)
}

let amount_out = calculate_swap_output(
    D96::from_str("1.0")?,      // 1 ETH in
    D96::from_str("1000")?,     // Reserve: 1000 ETH
    D96::from_str("2000000")?,  // Reserve: 2M USDC
    30,                          // 0.3% fee
)?;
```

### Bond Price Calculation (32nds)
```rust
use fixdec::D64;

// US Treasury bonds are quoted in 32nds
// e.g., "99-16" means 99 + 16/32 = 99.5
fn parse_bond_price(whole: i64, thirty_seconds: i64) -> D64 {
    let whole_part = D64::from_i64(whole).unwrap();
    let fraction = D64::THIRTY_SECOND.mul_i64(thirty_seconds).unwrap();
    whole_part + fraction
}

let price = parse_bond_price(99, 16);
assert_eq!(price.to_string(), "99.5");
```

## Comparison with rust_decimal

| Feature | fixdec | rust_decimal |
|---------|--------|--------------|
| **Precision** | Fixed (8 or 12 decimals) | Configurable (0-28 decimals) |
| **Performance** | **6-12x faster** | Slower due to flexibility |
| **Use case** | Performance-critical with known precision | General purpose, configurable precision |
| **no_std** | ✅ Full support | ✅ Full support |
| **Serialization** | ✅ Optimized for binary | ✅ General purpose |
| **API similarity** | High (easy migration) | - |

`fixdec` is built for **speed** when you know your precision requirements. `rust_decimal` is built for **flexibility** when you need configurable precision.

## Safety and Correctness

- **Overflow behavior**: All arithmetic operations have `checked`, `saturating`, and `wrapping` variants
- **Minimal unsafe**: Safe Rust throughout the core; the only `unsafe` is the feature-gated `bytemuck` `Pod`/`NoUninit`/`Zeroable` impls (sound via `#[repr(transparent)]`)
- **Extensive testing**: Property-based tests with `proptest` verify correctness against baseline implementations
- **Banker's rounding**: IEEE 754 round-half-to-even for tie-breaking

## Contributing

Contributions are welcome! Areas of interest:
- Performance optimizations
- Additional financial operations
- Documentation improvements
- Bug reports and fixes

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
