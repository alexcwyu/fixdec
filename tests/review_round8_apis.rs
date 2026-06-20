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
