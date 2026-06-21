//! Rem / modulo tests for D64 and D96: normal cases, edge cases, and properties.
//!
//! Because both operands share the same SCALE, the decimal remainder is exactly
//! the raw integer remainder, and matches truncated-division remainder semantics
//! (sign follows the dividend), same as `rust_decimal` and Rust's integer `%`.

use core::str::FromStr;
use fixdec::{D64, D96, DecimalError};
use proptest::prelude::*;

// ===========================================================================
// D64
// ===========================================================================

#[test]
fn d64_rem_basic() {
    assert_eq!(
        (D64::from_str("10.5").unwrap() % D64::from_str("3").unwrap()).to_string(),
        "1.5"
    );
    assert_eq!((D64::from_i32(10) % D64::from_i32(3)).to_string(), "1");
    assert_eq!(D64::from_i32(9) % D64::from_i32(3), D64::ZERO); // exact multiple
    assert_eq!(
        (D64::from_str("0.00000005").unwrap() % D64::from_str("0.00000003").unwrap()).to_raw(),
        2
    );
}

#[test]
fn d64_rem_sign_follows_dividend() {
    assert_eq!(
        (D64::from_str("-10.5").unwrap() % D64::from_str("3").unwrap()).to_string(),
        "-1.5"
    );
    assert_eq!(
        (D64::from_str("10.5").unwrap() % D64::from_str("-3").unwrap()).to_string(),
        "1.5"
    );
    assert_eq!(
        (D64::from_str("-10.5").unwrap() % D64::from_str("-3").unwrap()).to_string(),
        "-1.5"
    );
}

#[test]
fn d64_rem_smaller_dividend() {
    let a = D64::from_str("2.5").unwrap();
    assert_eq!(a % D64::from_i32(10), a); // a < b -> remainder is a
}

#[test]
fn d64_rem_assign() {
    let mut a = D64::from_str("10.5").unwrap();
    a %= D64::from_i32(3);
    assert_eq!(a.to_string(), "1.5");
}

#[test]
fn d64_checked_and_try_rem() {
    assert_eq!(D64::ONE.checked_rem(D64::ZERO), None);
    assert_eq!(
        D64::ONE.try_rem(D64::ZERO),
        Err(DecimalError::DivisionByZero)
    );
    assert_eq!(
        D64::from_i32(10).checked_rem(D64::from_i32(3)),
        Some(D64::ONE)
    );
    // i64::MIN % -1 (raw) is the division-overflow case: checked_rem returns None
    // (same as i64::checked_rem), so it never panics.
    assert_eq!(D64::MIN.checked_rem(D64::from_raw(-1)), None);
}

#[test]
#[should_panic]
fn d64_rem_zero_panics() {
    let _ = D64::ONE % D64::ZERO;
}

#[test]
fn d64_is_multiple_of_and_div_rem() {
    assert!(D64::from_i32(10).is_multiple_of(D64::from_str("2.5").unwrap()));
    assert!(!D64::from_i32(10).is_multiple_of(D64::from_i32(3)));
    assert!(D64::ZERO.is_multiple_of(D64::ZERO));
    assert!(!D64::ONE.is_multiple_of(D64::ZERO));

    let (q, r) = D64::from_str("10.5")
        .unwrap()
        .div_rem(D64::from_i32(3))
        .unwrap();
    assert_eq!(q, 3);
    assert_eq!(r.to_string(), "1.5");
    // identity: a == b*q + r
    let a = D64::from_str("10.5").unwrap();
    let b = D64::from_i32(3);
    assert_eq!(b.mul_i64(q).unwrap() + r, a);
    assert_eq!(D64::ONE.div_rem(D64::ZERO), None);
}

proptest! {
    #[test]
    fn d64_rem_invariants(a in i64::MIN..=i64::MAX, b in i64::MIN..=i64::MAX) {
        prop_assume!(b != 0);
        prop_assume!(!(a == i64::MIN && b == -1)); // matches checked_rem contract
        let r = D64::from_raw(a) % D64::from_raw(b);
        prop_assert!(r.to_raw().unsigned_abs() < b.unsigned_abs()); // |a%b| < |b|
        prop_assert_eq!((a - r.to_raw()) % b, 0);                   // (a - a%b) divisible by b
        prop_assert_eq!(r.to_raw(), a % b);                         // exact raw oracle
    }
}

// ===========================================================================
// D96
// ===========================================================================

#[test]
fn d96_rem_basic() {
    assert_eq!(
        (D96::from_str("10.5").unwrap() % D96::from_str("3").unwrap()).to_string(),
        "1.5"
    );
    assert_eq!(D96::from_i32(9) % D96::from_i32(3), D96::ZERO);
    assert_eq!(
        (D96::from_str("-10.5").unwrap() % D96::from_str("3").unwrap()).to_string(),
        "-1.5"
    );
}

#[test]
fn d96_checked_and_try_rem() {
    assert_eq!(D96::ONE.checked_rem(D96::ZERO), None);
    assert_eq!(
        D96::ONE.try_rem(D96::ZERO),
        Err(DecimalError::DivisionByZero)
    );
    assert_eq!(
        D96::from_i32(10).checked_rem(D96::from_i32(3)),
        Some(D96::ONE)
    );
}

#[test]
fn d96_is_multiple_of_and_div_rem() {
    assert!(D96::from_i32(10).is_multiple_of(D96::from_str("2.5").unwrap()));
    assert!(!D96::from_i32(10).is_multiple_of(D96::from_i32(3)));
    let (q, r) = D96::from_str("10.5")
        .unwrap()
        .div_rem(D96::from_i32(3))
        .unwrap();
    assert_eq!(q, 3);
    assert_eq!(r.to_string(), "1.5");
}

#[test]
#[should_panic]
fn d96_rem_zero_panics() {
    let _ = D96::ONE % D96::ZERO;
}

proptest! {
    #[test]
    fn d96_rem_invariants(a in any::<i64>(), b in any::<i64>()) {
        prop_assume!(b != 0);
        let (a, b) = (a as i128, b as i128);
        let r = D96::from_raw(a) % D96::from_raw(b);
        prop_assert!(r.to_raw().unsigned_abs() < b.unsigned_abs());
        prop_assert_eq!((a - r.to_raw()) % b, 0);
        prop_assert_eq!(r.to_raw(), a % b);
    }
}
