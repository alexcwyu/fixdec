//! `rust_decimal` interop (feature `rust-decimal`).
//!
//! Run with `cargo test --features rust-decimal`. Mirrors the crate's existing
//! narrowing convention: a strict conversion that reports `PrecisionLoss`, and a
//! rounding conversion (banker's) that only fails on `Overflow`/`Underflow`.

#![cfg(feature = "rust-decimal")]

use core::str::FromStr;
use fixdec::{D64, D96, DecimalError};
use rust_decimal::Decimal;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

// ===========================================================================
// Lossless widening: D64 / D96 -> Decimal -> back is exact for every value
// ===========================================================================

#[test]
fn d64_decimal_roundtrip_is_lossless() {
    for s in [
        "0",
        "1",
        "-1",
        "123.45",
        "0.00000001",
        "92233720368.54775807",  // D64::MAX
        "-92233720368.54775808", // D64::MIN
    ] {
        let d = D64::from_str(s).unwrap();
        let as_dec: Decimal = d.into(); // From<D64> for Decimal
        assert_eq!(d.to_rust_decimal(), as_dec);
        let back = D64::try_from(as_dec).unwrap(); // TryFrom<Decimal> for D64
        assert_eq!(back, d, "D64 -> Decimal -> D64 for {s}");
    }
    // The extreme values specifically must round-trip through the trait forms.
    assert_eq!(D64::try_from(Decimal::from(D64::MAX)).unwrap(), D64::MAX);
    assert_eq!(D64::try_from(Decimal::from(D64::MIN)).unwrap(), D64::MIN);
}

#[test]
fn d96_decimal_roundtrip_is_lossless() {
    for s in ["0", "1", "-1", "123.456789012", "0.000000000001"] {
        let d = D96::from_str(s).unwrap();
        let as_dec: Decimal = d.into();
        assert_eq!(d.to_rust_decimal(), as_dec);
        let back = D96::try_from(as_dec).unwrap();
        assert_eq!(back, d, "D96 -> Decimal -> D96 for {s}");
    }
    assert_eq!(D96::try_from(Decimal::from(D96::MAX)).unwrap(), D96::MAX);
    assert_eq!(D96::try_from(Decimal::from(D96::MIN)).unwrap(), D96::MIN);
}

// ===========================================================================
// Decimal -> D64: strict (exact / PrecisionLoss / Overflow / Underflow)
// ===========================================================================

#[test]
fn decimal_to_d64_exact_within_8dp() {
    assert_eq!(D64::from_rust_decimal(dec("123.45")).unwrap(), D64::from_str("123.45").unwrap());
    assert_eq!(D64::from_rust_decimal(dec("0.00000001")).unwrap(), D64::from_str("0.00000001").unwrap());
    // trailing zeros below 8dp still divide evenly -> exact
    assert_eq!(D64::from_rust_decimal(dec("1.230000000")).unwrap(), D64::from_str("1.23").unwrap());
    assert_eq!(D64::from_rust_decimal(dec("0")).unwrap(), D64::ZERO);
    assert_eq!(D64::from_rust_decimal(dec("-7.5")).unwrap(), D64::from_str("-7.5").unwrap());
}

#[test]
fn decimal_to_d64_precision_loss_is_strict() {
    // 9th decimal is significant -> strict conversion refuses
    assert_eq!(D64::from_rust_decimal(dec("0.000000001")), Err(DecimalError::PrecisionLoss));
    assert_eq!(D64::from_rust_decimal(dec("-1.234567891")), Err(DecimalError::PrecisionLoss));
}

#[test]
fn decimal_to_d64_overflow_and_underflow() {
    // 1e11 > D64::MAX (~9.22e10)
    assert_eq!(D64::from_rust_decimal(dec("100000000000")), Err(DecimalError::Overflow));
    assert_eq!(D64::from_rust_decimal(dec("-100000000000")), Err(DecimalError::Underflow));
    // Decimal::MAX (~7.9e28) overflows even after the i128 multiply guard
    assert_eq!(D64::from_rust_decimal(Decimal::MAX), Err(DecimalError::Overflow));
}

// ===========================================================================
// Decimal -> D64: rounding (banker's; only Overflow/Underflow can fail)
// ===========================================================================

#[test]
fn decimal_to_d64_round_bankers() {
    // half-to-even at the 8th decimal
    assert_eq!(D64::from_rust_decimal_round(dec("0.000000005")).unwrap(), D64::ZERO); // 0.5 -> 0
    assert_eq!(
        D64::from_rust_decimal_round(dec("0.000000015")).unwrap(),
        D64::from_str("0.00000002").unwrap() // 1.5 -> 2
    );
    assert_eq!(
        D64::from_rust_decimal_round(dec("0.000000025")).unwrap(),
        D64::from_str("0.00000002").unwrap() // 2.5 -> 2
    );
    // a value far below 1 ULP rounds to zero — that is NOT underflow
    assert_eq!(D64::from_rust_decimal_round(dec("0.0000000000001")).unwrap(), D64::ZERO);
    // rounding variant still rejects out-of-range magnitudes
    assert_eq!(D64::from_rust_decimal_round(dec("100000000000")), Err(DecimalError::Overflow));
    assert_eq!(D64::from_rust_decimal_round(dec("-100000000000")), Err(DecimalError::Underflow));
}

// ===========================================================================
// Decimal -> D96: strict + rounding at 12dp
// ===========================================================================

#[test]
fn decimal_to_d96_exact_and_precision_loss() {
    assert_eq!(D96::from_rust_decimal(dec("123.456789012")).unwrap(), D96::from_str("123.456789012").unwrap());
    assert_eq!(D96::from_rust_decimal(dec("0.000000000001")).unwrap(), D96::from_str("0.000000000001").unwrap());
    // 13th decimal significant -> PrecisionLoss
    assert_eq!(D96::from_rust_decimal(dec("0.0000000000001")), Err(DecimalError::PrecisionLoss));
}

#[test]
fn decimal_large_negative_is_underflow_not_overflow() {
    // Regression: a large NEGATIVE Decimal whose scaled mantissa overflows i128
    // must report Underflow (below MIN), not Overflow. Decimal::MIN ~= -7.9e28.
    assert_eq!(D96::from_rust_decimal(Decimal::MIN), Err(DecimalError::Underflow));
    assert_eq!(D96::from_rust_decimal_round(Decimal::MIN), Err(DecimalError::Underflow));
    // ...and the positive extreme is still Overflow (the asymmetry the bug hid).
    assert_eq!(D96::from_rust_decimal(Decimal::MAX), Err(DecimalError::Overflow));
    assert_eq!(D96::from_rust_decimal_round(Decimal::MAX), Err(DecimalError::Overflow));
    // A negative integer below MIN but still inside i128 after scaling also
    // classifies as Underflow (exercises the post-multiply range check path).
    assert_eq!(D96::from_rust_decimal(dec("-100000000000000000")), Err(DecimalError::Underflow));
    // D64 mirror (its overflow path is reached via the range check).
    assert_eq!(D64::from_rust_decimal(Decimal::MIN), Err(DecimalError::Underflow));
    assert_eq!(D64::from_rust_decimal_round(Decimal::MIN), Err(DecimalError::Underflow));
}

#[test]
fn decimal_to_d96_round_and_overflow() {
    // banker's rounding at the 12th decimal
    assert_eq!(D96::from_rust_decimal_round(dec("0.0000000000005")).unwrap(), D96::ZERO); // 0.5 -> 0
    assert_eq!(
        D96::from_rust_decimal_round(dec("0.0000000000015")).unwrap(),
        D96::from_str("0.000000000002").unwrap() // 1.5 -> 2
    );
    // 1e17 > D96::MAX (~3.96e16)
    assert_eq!(D96::from_rust_decimal(dec("100000000000000000")), Err(DecimalError::Overflow));
    assert_eq!(D96::from_rust_decimal(dec("-100000000000000000")), Err(DecimalError::Underflow));
    // Decimal::MAX overflows the i128 multiply (mantissa ~7.9e28 * 10^12) -> Overflow, not a wrong value
    assert_eq!(D96::from_rust_decimal(Decimal::MAX), Err(DecimalError::Overflow));
}

// ===========================================================================
// Trait forms agree with the inherent methods
// ===========================================================================

#[test]
fn trait_and_inherent_forms_agree() {
    let d = D64::from_str("3.14159265").unwrap();
    let v: Decimal = d.into();
    assert_eq!(<D64 as TryFrom<Decimal>>::try_from(v).unwrap(), D64::from_rust_decimal(v).unwrap());

    let d96 = D96::from_str("2.718281828459").unwrap();
    let v96: Decimal = d96.into();
    assert_eq!(<D96 as TryFrom<Decimal>>::try_from(v96).unwrap(), D96::from_rust_decimal(v96).unwrap());
}
