#![cfg(feature = "num-traits")]
//! `num-traits` integration tests for D64 and D96.
//!
//! Run with: `cargo test --features num-traits`
//!
//! Every assertion uses fully-qualified trait syntax (e.g. `Signed::abs(&x)`,
//! `<D64 as FromPrimitive>::from_i64(n)`) so it exercises the *trait* impl and
//! not the inherent method of the same name.

use core::str::FromStr;
use fixdec::{D64, D96, DecimalError};
use num_traits::{
    Bounded, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, FromPrimitive, Inv, Num, One,
    Saturating, Signed, ToPrimitive, Zero,
};

/// Generic sum constrained only by `num_traits::Zero` — proves the decimals plug
/// into generic numeric code that knows nothing about fixdec.
fn gsum<T: Zero + Copy>(xs: &[T]) -> T {
    xs.iter().fold(T::zero(), |acc, &x| acc + x)
}

// ---------- generic gsum ----------

#[test]
fn gsum_d64() {
    let xs = [D64::ONE, D64::from_i32(2), D64::from_i32(3)];
    assert_eq!(gsum(&xs), D64::from_i32(6));
    let empty: [D64; 0] = [];
    assert_eq!(gsum(&empty), D64::ZERO);
}

#[test]
fn gsum_d96() {
    let xs = [D96::ONE, D96::from_i32(2), D96::from_i32(3)];
    assert_eq!(gsum(&xs), D96::from_i32(6));
    let empty: [D96; 0] = [];
    assert_eq!(gsum(&empty), D96::ZERO);
}

// ---------- Zero / One ----------

#[test]
fn zero_one_d64() {
    assert_eq!(<D64 as Zero>::zero(), D64::ZERO);
    assert!(Zero::is_zero(&D64::ZERO));
    assert!(!Zero::is_zero(&D64::ONE));
    assert_eq!(<D64 as One>::one(), D64::ONE);
    assert!(One::is_one(&D64::ONE));
    assert!(!One::is_one(&D64::ZERO));
}

#[test]
fn zero_one_d96() {
    assert_eq!(<D96 as Zero>::zero(), D96::ZERO);
    assert!(Zero::is_zero(&D96::ZERO));
    assert!(!Zero::is_zero(&D96::ONE));
    assert_eq!(<D96 as One>::one(), D96::ONE);
    assert!(One::is_one(&D96::ONE));
    assert!(!One::is_one(&D96::ZERO));
}

// ---------- Bounded ----------

#[test]
fn bounded_d64() {
    assert_eq!(<D64 as Bounded>::max_value(), D64::MAX);
    assert_eq!(<D64 as Bounded>::min_value(), D64::MIN);
}

#[test]
fn bounded_d96() {
    assert_eq!(<D96 as Bounded>::max_value(), D96::MAX);
    assert_eq!(<D96 as Bounded>::min_value(), D96::MIN);
}

// ---------- Signed ----------

#[test]
fn signed_d64() {
    let neg = D64::from_str("-2.5").unwrap();
    let pos = D64::from_str("2.5").unwrap();

    assert_eq!(Signed::abs(&neg), pos);
    assert_eq!(Signed::abs(&pos), pos);

    // signum returns Self (ONE / -ONE / ZERO), not i32.
    assert_eq!(Signed::signum(&pos), D64::ONE);
    assert_eq!(Signed::signum(&neg), -D64::ONE);
    assert_eq!(Signed::signum(&D64::ZERO), D64::ZERO);

    assert!(Signed::is_positive(&pos));
    assert!(!Signed::is_positive(&neg));
    assert!(!Signed::is_positive(&D64::ZERO));
    assert!(Signed::is_negative(&neg));
    assert!(!Signed::is_negative(&pos));
    assert!(!Signed::is_negative(&D64::ZERO));

    // abs_sub: if self <= other -> ZERO, else self - other.
    assert_eq!(Signed::abs_sub(&pos, &neg), pos - neg); // 2.5 - (-2.5) = 5.0
    assert_eq!(Signed::abs_sub(&neg, &pos), D64::ZERO);
    assert_eq!(Signed::abs_sub(&pos, &pos), D64::ZERO); // equal -> ZERO
}

#[test]
fn signed_d96() {
    let neg = D96::from_str("-2.5").unwrap();
    let pos = D96::from_str("2.5").unwrap();

    assert_eq!(Signed::abs(&neg), pos);
    assert_eq!(Signed::signum(&pos), D96::ONE);
    assert_eq!(Signed::signum(&neg), -D96::ONE);
    assert_eq!(Signed::signum(&D96::ZERO), D96::ZERO);
    assert!(Signed::is_positive(&pos));
    assert!(Signed::is_negative(&neg));
    assert!(!Signed::is_positive(&D96::ZERO));

    assert_eq!(Signed::abs_sub(&pos, &neg), pos - neg);
    assert_eq!(Signed::abs_sub(&neg, &pos), D96::ZERO);
    assert_eq!(Signed::abs_sub(&pos, &pos), D96::ZERO);
}

// ---------- Num::from_str_radix ----------

#[test]
fn num_from_str_radix_d64() {
    assert_eq!(
        <D64 as Num>::from_str_radix("12.5", 10),
        Ok(D64::from_str("12.5").unwrap())
    );
    assert_eq!(
        <D64 as Num>::from_str_radix("-0.00000001", 10),
        Ok(D64::from_str("-0.00000001").unwrap())
    );
    // Non-decimal radix is rejected.
    assert_eq!(
        <D64 as Num>::from_str_radix("ff", 16),
        Err(DecimalError::InvalidFormat)
    );
    assert_eq!(
        <D64 as Num>::from_str_radix("101", 2),
        Err(DecimalError::InvalidFormat)
    );
    // Garbage in radix 10 still propagates the parse error.
    assert!(<D64 as Num>::from_str_radix("not a number", 10).is_err());
}

#[test]
fn num_from_str_radix_d96() {
    assert_eq!(
        <D96 as Num>::from_str_radix("12.5", 10),
        Ok(D96::from_str("12.5").unwrap())
    );
    assert_eq!(
        <D96 as Num>::from_str_radix("ff", 16),
        Err(DecimalError::InvalidFormat)
    );
}

// ---------- Checked* ----------

#[test]
fn checked_d64() {
    assert_eq!(CheckedAdd::checked_add(&D64::MAX, &D64::ONE), None);
    assert_eq!(
        CheckedAdd::checked_add(&D64::ONE, &D64::ONE),
        Some(D64::from_i32(2))
    );
    assert_eq!(CheckedSub::checked_sub(&D64::MIN, &D64::ONE), None);
    assert_eq!(
        CheckedSub::checked_sub(&D64::from_i32(5), &D64::from_i32(3)),
        Some(D64::from_i32(2))
    );
    assert_eq!(CheckedMul::checked_mul(&D64::MAX, &D64::MAX), None);
    assert_eq!(
        CheckedMul::checked_mul(&D64::from_i32(6), &D64::from_i32(7)),
        Some(D64::from_i32(42))
    );
    assert_eq!(CheckedDiv::checked_div(&D64::ONE, &D64::ZERO), None);
    assert_eq!(
        CheckedDiv::checked_div(&D64::from_i32(6), &D64::from_i32(2)),
        Some(D64::from_i32(3))
    );
}

#[test]
fn checked_d96() {
    assert_eq!(CheckedAdd::checked_add(&D96::MAX, &D96::ONE), None);
    assert_eq!(CheckedSub::checked_sub(&D96::MIN, &D96::ONE), None);
    assert_eq!(CheckedDiv::checked_div(&D96::ONE, &D96::ZERO), None);
    assert_eq!(
        CheckedDiv::checked_div(&D96::from_i32(6), &D96::from_i32(2)),
        Some(D96::from_i32(3))
    );
    assert_eq!(
        CheckedMul::checked_mul(&D96::from_i32(6), &D96::from_i32(7)),
        Some(D96::from_i32(42))
    );
}

// ---------- Saturating ----------

#[test]
fn saturating_d64() {
    assert_eq!(Saturating::saturating_add(D64::MAX, D64::ONE), D64::MAX);
    assert_eq!(Saturating::saturating_sub(D64::MIN, D64::ONE), D64::MIN);
    assert_eq!(
        Saturating::saturating_add(D64::ONE, D64::ONE),
        D64::from_i32(2)
    );
}

#[test]
fn saturating_d96() {
    assert_eq!(Saturating::saturating_add(D96::MAX, D96::ONE), D96::MAX);
    assert_eq!(Saturating::saturating_sub(D96::MIN, D96::ONE), D96::MIN);
}

// ---------- FromPrimitive ----------

#[test]
fn from_primitive_d64() {
    assert_eq!(<D64 as FromPrimitive>::from_i64(5), Some(D64::from_i32(5)));
    assert_eq!(<D64 as FromPrimitive>::from_u64(7), Some(D64::from_i32(7)));
    assert_eq!(
        <D64 as FromPrimitive>::from_f64(2.5),
        Some(D64::from_str("2.5").unwrap())
    );
    // i64::MAX * 1e8 overflows the i64 backing store -> None.
    assert_eq!(<D64 as FromPrimitive>::from_i64(i64::MAX), None);
    assert_eq!(<D64 as FromPrimitive>::from_u64(u64::MAX), None);
}

#[test]
fn from_primitive_d96() {
    assert_eq!(<D96 as FromPrimitive>::from_i64(5), Some(D96::from_i32(5)));
    assert_eq!(<D96 as FromPrimitive>::from_u64(7), Some(D96::from_i32(7)));
    assert_eq!(
        <D96 as FromPrimitive>::from_f64(2.5),
        Some(D96::from_str("2.5").unwrap())
    );
}

#[test]
fn from_primitive_d96_rejects_out_of_range() {
    // D96's max integer part is ≈3.96e16; FromPrimitive must return None beyond
    // it (contract), not Some(out-of-range value).
    assert_eq!(<D96 as FromPrimitive>::from_i64(i64::MAX), None);
    assert_eq!(<D96 as FromPrimitive>::from_u64(u64::MAX), None);
    assert_eq!(
        <D96 as FromPrimitive>::from_i64(39_614_081_257_132_169),
        None
    );
    // The exact max integer part (and its negation) are still representable.
    assert!(<D96 as FromPrimitive>::from_i64(39_614_081_257_132_168).is_some());
    assert!(<D96 as FromPrimitive>::from_i64(-39_614_081_257_132_168).is_some());
}

// ---------- ToPrimitive ----------

#[test]
fn to_primitive_d64() {
    let v = D64::from_str("2.5").unwrap();
    assert_eq!(ToPrimitive::to_i64(&v), Some(2)); // truncates toward zero
    assert_eq!(ToPrimitive::to_u64(&v), Some(2));
    assert_eq!(ToPrimitive::to_f64(&v), Some(2.5));

    let neg = D64::from_str("-3.5").unwrap();
    assert_eq!(ToPrimitive::to_i64(&neg), Some(-3));
    assert_eq!(ToPrimitive::to_u64(&neg), None); // negative -> no u64
    assert_eq!(ToPrimitive::to_f64(&neg), Some(-3.5));
}

#[test]
fn to_primitive_d96() {
    let v = D96::from_str("2.5").unwrap();
    assert_eq!(ToPrimitive::to_i64(&v), Some(2));
    assert_eq!(ToPrimitive::to_u64(&v), Some(2));
    assert_eq!(ToPrimitive::to_f64(&v), Some(2.5));

    let neg = D96::from_str("-3.5").unwrap();
    assert_eq!(ToPrimitive::to_u64(&neg), None);
}

// ---------- Inv ----------

#[test]
fn inv_d64() {
    let two = D64::from_i32(2);
    assert_eq!(Inv::inv(two), D64::from_str("0.5").unwrap());
}

#[test]
fn inv_d96() {
    let two = D96::from_i32(2);
    assert_eq!(Inv::inv(two), D96::from_str("0.5").unwrap());
}

#[test]
#[should_panic]
fn inv_zero_panics_d64() {
    let _ = Inv::inv(D64::ZERO);
}

#[test]
#[should_panic]
fn inv_zero_panics_d96() {
    let _ = Inv::inv(D96::ZERO);
}
