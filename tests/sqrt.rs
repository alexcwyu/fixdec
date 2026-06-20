//! Tests for the restored `D64::sqrt` / `D96::sqrt` (and the `try_*` twins).
//!
//! Contract (floor / truncate, matching the crate's "arithmetic truncates"
//! rule): `y = x.sqrt().unwrap()` is the largest representable value with
//! `y*y <= x`. In raw integer terms `y_raw = floor(isqrt(raw * SCALE))`, so the
//! invariant `y_raw^2 <= raw*SCALE < (y_raw + 1)^2` holds by construction.
//! Negative inputs have no real root: `sqrt -> None`, `try_sqrt -> Err`.
//!
//! History: `D96::sqrt` was removed in f857a3d because it formed `raw * SCALE`
//! in an i128, which overflows for any D96 value above ~1.7e14 (debug panic /
//! release silent-wrap). The restored version uses a wide (192-bit) intermediate
//! and a wide integer sqrt, so `D96::MAX.sqrt()` is now correct — that case is
//! the headline regression test below.

use fixdec::{D64, D96, DecimalError};
use proptest::prelude::*;

const D96_MAX_RAW: i128 = 39_614_081_257_132_168_796_771_975_167; // 2^95 - 1

// --- D64: the radicand `raw * 1e8` always fits i128, so we can assert the exact
// integer invariant across the entire range. ---

fn d64_assert_invariant(raw: i64) {
    let x = D64::from_raw(raw);
    let y = x.sqrt().expect("non-negative has a root");
    let yr = y.to_raw() as i128;
    assert!(yr >= 0, "root is non-negative");
    let radicand = raw as i128 * D64::SCALE as i128;
    assert!(yr * yr <= radicand, "lower: y^2 <= x for raw={raw}");
    assert!(
        radicand < (yr + 1) * (yr + 1),
        "upper: x < (y+ulp)^2 for raw={raw}"
    );
}

#[test]
fn d64_perfect_squares_are_exact() {
    assert_eq!(D64::from_i32(4).sqrt(), Some(D64::from_i32(2)));
    assert_eq!(D64::from_i32(9).sqrt(), Some(D64::from_i32(3)));
    assert_eq!(D64::from_i32(144).sqrt(), Some(D64::from_i32(12)));
    assert_eq!(D64::ZERO.sqrt(), Some(D64::ZERO));
    assert_eq!(D64::ONE.sqrt(), Some(D64::ONE));
}

#[test]
fn d64_sqrt_two_floors_at_scale() {
    // sqrt(2) = 1.41421356237..., floored to 8dp = 1.41421356.
    let y = D64::from_i32(2).sqrt().unwrap();
    assert_eq!(y.to_raw(), 141_421_356);
}

#[test]
fn d64_negative_has_no_root() {
    assert_eq!(D64::from_i32(-4).sqrt(), None);
    assert_eq!(D64::from_i32(-4).try_sqrt(), Err(DecimalError::NegativeValue));
    assert_eq!(D64::MIN.sqrt(), None);
}

#[test]
fn d64_max_does_not_overflow() {
    d64_assert_invariant(i64::MAX);
}

// --- D96: the radicand `raw * 1e12` (< 2^135) and `(y+1)^2` (y < 2^68) both fit
// a 192-bit value, so we assert the *exact* integer invariant across the whole
// range using a small 192-bit oracle — no floats. This covers both the u128
// fast path (small values) and the wide binary-search path (`raw*1e12 > 2^128`,
// i.e. values above ~3.1e14). ---

/// `a * b` as a 192-bit value `(low, high)`, valid for `a, b < 2^96` (schoolbook
/// 64-bit split; mirrors the crate's internal `mul_96x96_to_192`). Both the
/// radicand operands (`raw < 2^95`, `1e12 < 2^40`) and the square operands
/// (`y+1 < 2^68`) are well inside that bound.
fn mul96(a: u128, b: u128) -> (u128, u128) {
    let a_lo = a as u64 as u128;
    let a_hi = a >> 64;
    let b_lo = b as u64 as u128;
    let b_hi = b >> 64;
    let p0 = a_lo * b_lo;
    let mid = a_lo * b_hi + a_hi * b_lo;
    let (low, carry) = p0.overflowing_add(mid << 64);
    let high = a_hi * b_hi + (mid >> 64) + carry as u128;
    (low, high)
}

/// `a <= b` for 192-bit values represented as `(low, high)`.
fn le192(a: (u128, u128), b: (u128, u128)) -> bool {
    if a.1 != b.1 { a.1 < b.1 } else { a.0 <= b.0 }
}

fn d96_assert_invariant(raw: i128) {
    let x = D96::from_raw(raw);
    let y = x.sqrt().expect("non-negative has a root");
    let yr = y.to_raw() as u128;
    let radicand = mul96(raw as u128, D96::SCALE as u128);
    // lower: y^2 <= raw*SCALE
    assert!(
        le192(mul96(yr, yr), radicand),
        "lower: y^2 <= x for raw={raw}"
    );
    // upper: raw*SCALE < (y+1)^2, i.e. (y+1)^2 is NOT <= raw*SCALE
    assert!(
        !le192(mul96(yr + 1, yr + 1), radicand),
        "upper: x < (y+ulp)^2 for raw={raw}"
    );
}

#[test]
fn d96_perfect_squares_are_exact() {
    assert_eq!(D96::from_i32(4).sqrt(), Some(D96::from_i32(2)));
    assert_eq!(D96::from_i32(9).sqrt(), Some(D96::from_i32(3)));
    assert_eq!(D96::ZERO.sqrt(), Some(D96::ZERO));
    assert_eq!(D96::ONE.sqrt(), Some(D96::ONE));
}

#[test]
fn d96_large_perfect_squares_hit_the_wide_path() {
    // raw = value*1e12; radicand = raw*1e12. For these, radicand > 2^128 so the
    // wide (192-bit) sqrt path is exercised, yet the answer is exact.
    // sqrt(1e16) = 1e8.
    let x = D96::from_i64(10_000_000_000_000_000).unwrap(); // 1e16
    assert_eq!(x.sqrt(), Some(D96::from_i64(100_000_000).unwrap())); // 1e8
    // sqrt(2.25e16) = 1.5e8.
    let x = D96::from_i64(22_500_000_000_000_000).unwrap();
    assert_eq!(x.sqrt(), Some(D96::from_i64(150_000_000).unwrap()));
}

#[test]
fn d96_negative_has_no_root() {
    assert_eq!(D96::from_i32(-4).sqrt(), None);
    assert_eq!(D96::from_i32(-4).try_sqrt(), Err(DecimalError::NegativeValue));
    assert_eq!(D96::MIN.sqrt(), None);
}

#[test]
fn d96_max_does_not_overflow() {
    // The original bug: D96::MAX.sqrt() panicked (debug) / wrapped (release).
    d96_assert_invariant(D96_MAX_RAW);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn d64_sqrt_invariant(raw in 0i64..=i64::MAX) {
        d64_assert_invariant(raw);
    }

    #[test]
    fn d96_sqrt_invariant(raw in 0i128..=D96_MAX_RAW) {
        d96_assert_invariant(raw);
    }
}
