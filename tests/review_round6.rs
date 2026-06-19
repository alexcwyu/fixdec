//! Regression tests for Codex adversarial review round 6 (2026-06-19).
//!
//! Findings (all reproduced before fixing):
//!   #2 powi(i32::MIN) overflowed `-exp` -> debug panic / release stack overflow.
//!   #3 D64::from_basis_points rejected representable large inputs (early overflow).
//!   #4 new(int, frac) silently accepted an out-of-domain fractional part.

use core::str::FromStr;
use fixdec::{D64, D96};

// [#2] `powi` negated `-exp` for negative exponents; for `i32::MIN` that overflows
// i32 (debug: "attempt to negate with overflow"; release: wraps back to i32::MIN
// and recurses forever -> stack overflow). It must compute via the unsigned
// magnitude (`i32::MIN.unsigned_abs()` is exact) and return a value with no panic.
#[test]
fn powi_i32_min_does_not_panic() {
    // |base| > 1: squaring overflows long before 2^2147483648 -> None.
    assert_eq!(D64::from_i32(2).powi(i32::MIN), None);
    assert_eq!(D96::from_i32(2).powi(i32::MIN), None);
    // base == 1: 1^n == 1 for any n, and 1 / 1 == 1.
    assert_eq!(D64::ONE.powi(i32::MIN), Some(D64::ONE));
    assert_eq!(D96::ONE.powi(i32::MIN), Some(D96::ONE));
    // base == 0: 0^(negative) == 1/0 -> None (no panic).
    assert_eq!(D64::ZERO.powi(i32::MIN), None);
    assert_eq!(D96::ZERO.powi(i32::MIN), None);
    // The whole negative range stays consistent with the small-exponent path.
    assert_eq!(D64::ONE.powi(i32::MIN + 1), Some(D64::ONE));
    assert_eq!(D96::from_i32(10).powi(-2), D96::try_with_scale(1, 2));
}

// [#3] `from_basis_points` multiplied by the FULL scale before dividing by 10_000,
// overflowing far earlier than the final raw value would. Since `SCALE / 10_000`
// is exact, multiplying by that smaller factor accepts the full representable
// range without changing any in-range result.
#[test]
fn from_basis_points_accepts_representable_large_inputs() {
    // 1e11 bps == 10_000_000.0; raw 1e15 fits i64, but the old D64 code returned None.
    assert_eq!(
        D64::from_basis_points(100_000_000_000),
        Some(D64::from_i32(10_000_000))
    );
    // Existing small cases are unchanged.
    assert_eq!(D64::from_basis_points(100), Some(D64::from_str("0.01").unwrap()));
    assert_eq!(D64::from_basis_points(0), Some(D64::ZERO));
    assert_eq!(D64::from_basis_points(-100), Some(D64::from_str("-0.01").unwrap()));
    // Boundary: largest representable raw (= i64::MAX rounded down to a 1e4 step),
    // and one basis point past it.
    assert!(D64::from_basis_points(922_337_203_685_477).is_some());
    assert!(D64::from_basis_points(922_337_203_685_478).is_none());
    // D96 parity: same SCALE/10_000 form, behaviour-preserving for in-range values.
    assert_eq!(
        D96::from_basis_points(100_000_000_000),
        Some(D96::from_i32(10_000_000))
    );
    assert_eq!(D96::from_basis_points(100), Some(D96::from_str("0.01").unwrap()));
}
