//! Inspector / decomposition API: `mantissa()`, `scale()`, `is_integer()`.
//!
//! Semantics mirror `rust_decimal` (the project's reference) as closely as a
//! FIXED-scale type allows: mantissa()/scale() return the *minimal* (trailing-
//! zeros-trimmed) decomposition such that `value == mantissa * 10^(-scale)`.
//! `normalize()` is intentionally NOT provided — D64/D96 store exactly one bit
//! pattern per value (1.5 and 1.50 are the same raw), so it would be a no-op.

use core::str::FromStr;
use fixdec::{D64, D96};
use proptest::prelude::*;

#[test]
fn d64_decomposition_examples() {
    // (value, expected_mantissa, expected_scale, expected_is_integer)
    let cases: &[(D64, i64, u32, bool)] = &[
        (D64::ZERO, 0, 0, true),
        (D64::ONE, 1, 0, true),
        (D64::from_str("1.5").unwrap(), 15, 1, false),
        (D64::from_str("100").unwrap(), 100, 0, true),
        (D64::from_raw(1), 1, 8, false), // smallest ulp 1e-8
        (D64::from_str("-2.5").unwrap(), -25, 1, false),
        (D64::MAX, i64::MAX, 8, false),
        (D64::MIN, i64::MIN, 8, false),
    ];
    for &(v, m, s, isint) in cases {
        assert_eq!(v.mantissa(), m, "mantissa of {v}");
        assert_eq!(v.scale(), s, "scale of {v}");
        assert_eq!(v.is_integer(), isint, "is_integer of {v}");
    }
}

#[test]
fn d96_decomposition_examples() {
    let cases: &[(D96, i128, u32, bool)] = &[
        (D96::ZERO, 0, 0, true),
        (D96::ONE, 1, 0, true),
        (D96::from_str("1.5").unwrap(), 15, 1, false),
        (D96::from_str("100").unwrap(), 100, 0, true),
        (D96::from_raw(1), 1, 12, false), // smallest ulp 1e-12
        (D96::from_str("-2.5").unwrap(), -25, 1, false),
        (D96::MAX, D96::MAX.to_raw(), 12, false),
        (D96::MIN, D96::MIN.to_raw(), 12, false),
    ];
    for &(v, m, s, isint) in cases {
        assert_eq!(v.mantissa(), m, "mantissa of {v}");
        assert_eq!(v.scale(), s, "scale of {v}");
        assert_eq!(v.is_integer(), isint, "is_integer of {v}");
    }
}

#[test]
fn trailing_zeros_are_trimmed() {
    // 1.50 == 1.5 in fixed-point, so both decompose minimally to (15, 1).
    assert_eq!(D64::from_str("1.50").unwrap().mantissa(), 15);
    assert_eq!(D64::from_str("1.50").unwrap().scale(), 1);
    assert_eq!(D64::from_str("3.100").unwrap(), D64::from_str("3.1").unwrap());
    assert_eq!(D64::from_str("3.100").unwrap().mantissa(), 31);
    assert_eq!(D64::from_str("3.100").unwrap().scale(), 1);
}

#[test]
fn scale_and_is_integer_are_consistent() {
    for s in ["0", "1", "100", "1.5", "-2.5", "0.00000001", "92233720368.54775807"] {
        let v = D64::from_str(s).unwrap();
        assert!(v.scale() <= D64::DECIMALS as u32, "scale <= DECIMALS for {s}");
        assert_eq!(v.is_integer(), v.scale() == 0, "is_integer == (scale==0) for {s}");
    }
}

#[test]
fn min_decomposition_does_not_panic_or_overflow() {
    // i64::MIN / 96-bit min end in 8, so trimming leaves them unchanged and the
    // mantissa still fits the backing integer. is_integer divides by +SCALE, so
    // the i64::MIN % -1 overflow hazard cannot occur.
    assert_eq!(D64::MIN.mantissa(), i64::MIN);
    assert!(!D64::MIN.is_integer());
    assert_eq!(D96::MIN.mantissa(), D96::MIN.to_raw());
    assert!(!D96::MIN.is_integer());
}

#[test]
fn normalize_is_identity() {
    // 1.5 and 1.50 are the same raw value, so normalize() returns self.
    assert_eq!(D64::from_str("1.50").unwrap().normalize(), D64::from_str("1.5").unwrap());
    assert_eq!(D64::ZERO.normalize(), D64::ZERO);
    assert_eq!(D64::MIN.normalize(), D64::MIN);
    assert_eq!(D96::from_str("3.100").unwrap().normalize(), D96::from_str("3.1").unwrap());
    assert_eq!(D96::MAX.normalize(), D96::MAX);
    const N: D64 = D64::from_raw(150_000_000).normalize();
    assert_eq!(N, D64::from_raw(150_000_000));
}

#[test]
fn const_context() {
    const M: i64 = D64::from_raw(150_000_000).mantissa();
    const S: u32 = D64::from_raw(150_000_000).scale();
    const IS_INT: bool = D64::from_raw(150_000_000).is_integer();
    assert_eq!((M, S, IS_INT), (15, 1, false));
}

proptest! {
    #[test]
    fn d64_decomposition_round_trips(raw in any::<i64>()) {
        let v = D64::from_raw(raw);
        prop_assert_eq!(D64::try_with_scale(v.mantissa(), v.scale()), Some(v));
        prop_assert!(v.scale() <= D64::DECIMALS as u32);
        prop_assert_eq!(v.is_integer(), v.scale() == 0);
    }

    #[test]
    fn d96_decomposition_round_trips(seed in any::<i128>()) {
        // map into the legal 96-bit raw range
        const TWO_POW_96: i128 = 1i128 << 96;
        let mut raw = seed % TWO_POW_96;
        if raw > D96::MAX.to_raw() { raw -= TWO_POW_96; }
        else if raw < D96::MIN.to_raw() { raw += TWO_POW_96; }
        let v = D96::from_raw(raw);
        prop_assert_eq!(D96::try_with_scale(v.mantissa(), v.scale()), Some(v));
        prop_assert!(v.scale() <= D96::DECIMALS as u32);
        prop_assert_eq!(v.is_integer(), v.scale() == 0);
    }
}

#[cfg(feature = "rust-decimal")]
#[test]
fn matches_rust_decimal_decomposition() {
    use rust_decimal::prelude::*;
    for s in ["0", "1", "1.5", "100", "0.00000001", "-2.5", "12345.6789"] {
        let v = D64::from_str(s).unwrap();
        let d = v.to_rust_decimal().normalize();
        assert_eq!(v.mantissa() as i128, d.mantissa(), "mantissa vs rust_decimal for {s}");
        assert_eq!(v.scale(), d.scale(), "scale vs rust_decimal for {s}");
    }
}
