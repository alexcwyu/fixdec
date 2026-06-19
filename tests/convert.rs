//! D64 <-> D96 conversion tests.
//!
//! D64 (8dp) widens to D96 (12dp) losslessly (raw * 10_000). Narrowing D96 -> D64
//! is fallible: it loses the low 4 decimal digits (PrecisionLoss unless they are
//! zero) and can exceed D64's range (Overflow/Underflow).

use core::str::FromStr;
use fixdec::{D64, D96, DecimalError};
use proptest::prelude::*;
use rust_decimal::{Decimal, RoundingStrategy};

#[test]
fn d64_to_d96_exact_and_value_preserved() {
    let a = D64::from_str("1234.56").unwrap();
    let b: D96 = a.into();
    assert_eq!(b.to_string(), "1234.56");
    assert_eq!(b, D96::from_d64(a));
    assert_eq!(a.to_d96(), b);
    assert_eq!(b.to_raw(), a.to_raw() as i128 * 10_000); // raw widening
}

#[test]
fn d64_to_d96_extremes() {
    assert_eq!(D96::from_d64(D64::MAX).to_raw(), i64::MAX as i128 * 10_000);
    assert_eq!(D96::from_d64(D64::MIN).to_raw(), i64::MIN as i128 * 10_000);
    assert_eq!(D96::from_d64(D64::ZERO), D96::ZERO);
}

#[test]
fn d96_to_d64_exact() {
    let v = D96::from_str("1234.56").unwrap();
    assert_eq!(D64::try_from(v), Ok(D64::from_str("1234.56").unwrap()));
    assert_eq!(v.to_d64(), Ok(D64::from_str("1234.56").unwrap()));
    assert_eq!(D96::from_str("0.00000001").unwrap().to_d64().unwrap().to_raw(), 1);
    assert_eq!(D96::from_str("-1234.56").unwrap().to_d64().unwrap().to_string(), "-1234.56");
}

#[test]
fn d96_to_d64_precision_loss() {
    // 9th+ decimal can't be represented exactly in D64 (8dp)
    assert_eq!(D96::from_str("1.000000000001").unwrap().to_d64(), Err(DecimalError::PrecisionLoss));
    assert_eq!(D64::try_from(D96::from_str("1.000000000001").unwrap()), Err(DecimalError::PrecisionLoss));
    assert_eq!(D96::from_str("-0.000000005").unwrap().to_d64(), Err(DecimalError::PrecisionLoss));
}

#[test]
fn d96_to_d64_round_bankers() {
    // raw96 / 10_000 with round-half-to-even
    assert_eq!(D96::from_raw(125_000).to_d64_round().unwrap().to_raw(), 12); // 12.5 -> 12
    assert_eq!(D96::from_raw(135_000).to_d64_round().unwrap().to_raw(), 14); // 13.5 -> 14
    assert_eq!(D96::from_raw(127_000).to_d64_round().unwrap().to_raw(), 13); // 12.7 -> 13
    assert_eq!(D96::from_raw(124_999).to_d64_round().unwrap().to_raw(), 12); // 12.4999 -> 12
    assert_eq!(D96::from_raw(-125_000).to_d64_round().unwrap().to_raw(), -12);
    assert_eq!(D96::from_raw(-135_000).to_d64_round().unwrap().to_raw(), -14);
    assert_eq!(D96::from_str("1234.56").unwrap().to_d64_round().unwrap().to_string(), "1234.56");
}

#[test]
fn d96_to_d64_out_of_range() {
    let big = D96::from_i64(100_000_000_000).unwrap(); // 1e11 > D64 max ~9.2e10
    assert_eq!(big.to_d64(), Err(DecimalError::Overflow));
    assert_eq!(big.to_d64_round(), Err(DecimalError::Overflow));
    let small = D96::from_i64(-100_000_000_000).unwrap();
    assert_eq!(small.to_d64(), Err(DecimalError::Underflow));
    assert_eq!(small.to_d64_round(), Err(DecimalError::Underflow));

    let max96 = D96::from_d64(D64::MAX);
    assert_eq!(D64::try_from(max96), Ok(D64::MAX));
    assert_eq!(
        D64::try_from(D96::from_raw(max96.to_raw() + 10_000)),
        Err(DecimalError::Overflow)
    );
}

proptest! {
    // widen then narrow is a lossless round trip for every D64
    #[test]
    fn d64_d96_roundtrip(a in i64::MIN..=i64::MAX) {
        let d = D64::from_raw(a);
        prop_assert_eq!(D64::try_from(D96::from(d)), Ok(d));
        prop_assert_eq!(D96::from(d).to_d64_round(), Ok(d));
    }

    // to_d64_round matches rust_decimal's round-half-to-even at 8dp (operands in D64 range)
    #[test]
    fn d96_to_d64_round_matches_rust_decimal(
        raw in -(9i128 * 10i128.pow(22))..=(9i128 * 10i128.pow(22))
    ) {
        let got = D96::from_raw(raw).to_d64_round().unwrap();
        let dec = Decimal::from_i128_with_scale(raw, 12)
            .round_dp_with_strategy(8, RoundingStrategy::MidpointNearestEven);
        let want = Decimal::from_i128_with_scale(got.to_raw() as i128, 8);
        prop_assert_eq!(want, dec, "raw={}", raw);
    }
}
