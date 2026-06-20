//! High-performance fixed-point decimal types for financial calculations
//!
//! This library provides two decimal types optimized for speed and precision:
//!
//! - **`D64`**: 64-bit with 8 decimal places
//!   - Range: ±92,233,720,368.54775807 (±92 billion)
//!   - Precision: 0.00000001
//!   - Use cases: Traditional finance, USD/EUR pricing, portfolio P&L
//!
//! - **`D96`**: 96-bit with 12 decimal places
//!   - Range: ±39,614,081,257,132.168796771975 (±39 trillion)
//!   - Precision: 0.000000000001 (1 microGwei)
//!   - Use cases: Cryptocurrency, high-precision DeFi, extreme price ranges
//!
//! ## Features
//!
//! - **Fast arithmetic**: Optimized multiplication using reciprocal division
//! - **Exact decimal math**: No floating-point rounding errors
//! - **no_std compatible**: Works in embedded and WebAssembly environments
//! - **Serde support**: Efficient serialization for JSON and binary formats
//! - **Comprehensive operations**: Checked, saturating, and wrapping variants
//!
//! ## Example
//!
//! ```rust
//! use fixdec::{D64, D96};
//! use std::str::FromStr;
//!
//! // D64 for traditional finance
//! let price = D64::from_str("1234.56").unwrap();
//! let quantity = D64::from_i32(100);
//! let total = price * quantity; // 123,456.00
//!
//! // D96 for cryptocurrency
//! let eth_price = D96::from_str("2500.123456789012").unwrap();
//! let amount = D96::from_str("0.5").unwrap();
//! let value = eth_price * amount;
//! ```

#![no_std]
#![cfg_attr(test, allow(unused_imports))]

#[cfg(test)]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

mod d64;
mod d96;
mod internal;

#[cfg(feature = "pyo3")]
mod pyo3_conv;

pub use d64::D64;
pub use d96::D96;
#[cfg(feature = "rkyv")]
pub use d96::D96RangeError;

use thiserror::Error;

/// Rounding mode for the explicit `*_with_strategy` / `*_rounded` operations.
///
/// The default ([`MidpointNearestEven`](RoundingStrategy::MidpointNearestEven),
/// banker's rounding) is what every *implicit* rounding op in the crate already
/// uses (`round`, `round_dp`, `from_str_lossy`, `Display` `{:.N}`, `to_d64_round`,
/// `with_scale_lossy`). The other strategies are opt-in via
/// [`D64::round_dp_with_strategy`] / [`D96::round_dp_with_strategy`] and
/// [`D64::checked_div_rounded`] / [`D96::checked_div_rounded`]. Variant names
/// match `rust_decimal::RoundingStrategy`, with which the crate already interops.
///
/// The `*` and `/` operators and `checked_mul` / `checked_div` are **not**
/// affected: they always TRUNCATE toward zero. Strategies apply only to the
/// explicit rounding methods above.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoundingStrategy {
    /// Round to nearest; on a tie, to the nearest even digit (banker's rounding).
    /// The crate-wide default.
    #[default]
    MidpointNearestEven,
    /// Round to nearest; on a tie, away from zero (half-up by magnitude).
    MidpointAwayFromZero,
    /// Round to nearest; on a tie, toward zero (half-down by magnitude).
    MidpointTowardZero,
    /// Truncate toward zero (drop the rounded-off digits).
    ToZero,
    /// Round away from zero whenever any rounded-off digit is non-zero.
    AwayFromZero,
    /// Round toward negative infinity (floor).
    ToNegativeInfinity,
    /// Round toward positive infinity (ceil).
    ToPositiveInfinity,
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecimalError {
    #[error("overflow: value too large to represent")]
    Overflow,

    #[error("underflow: value too small to represent")]
    Underflow,

    #[error("division by zero")]
    DivisionByZero,

    #[error("invalid string format")]
    InvalidFormat,

    #[error("precision loss would occur")]
    PrecisionLoss,
}

pub type Result<T> = core::result::Result<T, DecimalError>;
