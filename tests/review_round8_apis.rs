//! Codex adversarial review round 8 — API additions:
//!   F5 `as_integer_ratio`, F6 integer-operand arithmetic, F7 `overflowing_*`.

use core::str::FromStr;
use fixdec::{D64, D96};

/// Local gcd so the tests can independently assert `as_integer_ratio` is reduced.
fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

// ---------------------------------------------------------------------------
// [F5] as_integer_ratio: reduced lowest-terms (numerator, denominator >= 1).
// ---------------------------------------------------------------------------

#[test]
fn d64_as_integer_ratio_reduced() {
    assert_eq!(D64::from_str("1.5").unwrap().as_integer_ratio(), (3, 2));
    assert_eq!(D64::from_str("-0.25").unwrap().as_integer_ratio(), (-1, 4));
    assert_eq!(D64::ZERO.as_integer_ratio(), (0, 1));
    assert_eq!(D64::ONE.as_integer_ratio(), (1, 1));
    assert_eq!(D64::from_str("100").unwrap().as_integer_ratio(), (100, 1));
    assert_eq!(D64::from_raw(1).as_integer_ratio(), (1, 100_000_000)); // 1e-8

    for v in [
        D64::MAX,
        D64::MIN,
        D64::from_str("3.14159265").unwrap(),
        D64::from_raw(-1),
    ] {
        let (n, d) = v.as_integer_ratio();
        // Exact reconstruction: value == n * (SCALE / d) (d divides SCALE).
        assert_eq!(v.to_raw() as i128, n as i128 * (D64::SCALE as i128 / d as i128));
        // Lowest terms.
        assert_eq!(gcd((n as i128).unsigned_abs(), d as u128), 1);
    }
}

#[test]
fn d96_as_integer_ratio_reduced() {
    assert_eq!(D96::from_str("1.5").unwrap().as_integer_ratio(), (3, 2));
    assert_eq!(D96::from_str("-0.25").unwrap().as_integer_ratio(), (-1, 4));
    assert_eq!(D96::ZERO.as_integer_ratio(), (0, 1));
    assert_eq!(D96::ONE.as_integer_ratio(), (1, 1));
    assert_eq!(D96::from_raw(1).as_integer_ratio(), (1, 1_000_000_000_000)); // 1e-12

    for v in [
        D96::MAX,
        D96::MIN,
        D96::from_str("3.141592653589").unwrap(),
        D96::from_raw(-1),
    ] {
        let (n, d) = v.as_integer_ratio();
        assert_eq!(v.to_raw(), n * (D96::SCALE / d as i128));
        assert_eq!(gcd(n.unsigned_abs(), d), 1);
    }
}

// ---------------------------------------------------------------------------
// [F7] overflowing_add/sub/mul -> (wrapped, overflowed). No overflowing_div
// (divide-by-zero has no wrapped value to return).
// ---------------------------------------------------------------------------

#[test]
fn d64_overflowing_matches_checked_and_wrapping() {
    let a = D64::from_str("2").unwrap();
    let b = D64::from_str("3").unwrap();
    assert_eq!(a.overflowing_add(b), (D64::from_str("5").unwrap(), false));
    assert_eq!(a.overflowing_mul(b), (D64::from_str("6").unwrap(), false));
    let ulp = D64::from_raw(1);
    assert!(D64::MAX.overflowing_add(ulp).1);
    let big = D64::from_str("1000000").unwrap();
    assert!(big.overflowing_mul(big).1);
    // Invariant: overflowing == (wrapping, checked.is_none()) for every operand.
    let cases = [
        (D64::MAX, D64::MAX),
        (D64::MIN, D64::from_str("-1").unwrap()),
        (D64::MAX, ulp),
        (a, b),
    ];
    for (x, y) in cases {
        assert_eq!(x.overflowing_add(y), (x.wrapping_add(y), x.checked_add(y).is_none()));
        assert_eq!(x.overflowing_sub(y), (x.wrapping_sub(y), x.checked_sub(y).is_none()));
        assert_eq!(x.overflowing_mul(y), (x.wrapping_mul(y), x.checked_mul(y).is_none()));
    }
}

#[test]
fn d96_overflowing_matches_checked_and_wrapping() {
    let a = D96::from_str("2").unwrap();
    let b = D96::from_str("3").unwrap();
    assert_eq!(a.overflowing_add(b), (D96::from_str("5").unwrap(), false));
    assert_eq!(a.overflowing_mul(b), (D96::from_str("6").unwrap(), false));
    let ulp = D96::from_raw(1);
    assert!(D96::MAX.overflowing_add(ulp).1);
    let cases = [
        (D96::MAX, D96::MAX),
        (D96::MIN, D96::from_str("-1").unwrap()),
        (D96::MAX, ulp),
        (a, b),
    ];
    for (x, y) in cases {
        assert_eq!(x.overflowing_add(y), (x.wrapping_add(y), x.checked_add(y).is_none()));
        assert_eq!(x.overflowing_sub(y), (x.wrapping_sub(y), x.checked_sub(y).is_none()));
        assert_eq!(x.overflowing_mul(y), (x.wrapping_mul(y), x.checked_mul(y).is_none()));
    }
}
