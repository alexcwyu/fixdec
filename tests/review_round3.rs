//! Regression tests for deep-review round 3 (2026-06-19).
//!
//! Each test pins a confirmed finding from an adversarial multi-agent review so
//! that an independent cross-review (GPT / DeepSeek) can re-verify the fix.
//! Findings are grouped: D96 96-bit invariant + signed-MIN (multiply/divide),
//! parser overflow & lossy/exact parity, Display rounding carry & banker's,
//! lossy with_scale range checks, D96 integer From → TryFrom, and assorted
//! predicate/interop correctness.

use fixdec::D96;

/// -1.0 as a D96 without depending on a public SCALE constant.
fn d96_neg_one() -> D96 {
    D96::from_raw(-D96::ONE.to_raw())
}

// ===========================================================================
// [7][8][9][11][12][18] D96 multiply/divide: signed-MIN results & 96-bit range
// ===========================================================================

#[test]
fn d96_min_mul_one_is_min() {
    // |MIN| = 2^95 is representable when the result is negative, so MIN*1 == MIN.
    assert_eq!(D96::MIN.checked_mul(D96::ONE), Some(D96::MIN));
    assert_eq!(D96::ONE.checked_mul(D96::MIN), Some(D96::MIN));
    assert_eq!(D96::MIN.try_mul(D96::ONE), Ok(D96::MIN));
}

#[test]
fn d96_min_mul_add_is_min_no_debug_panic() {
    // Previously the debug_assert (a <= MAX) panicked in debug builds for MIN.
    assert_eq!(D96::MIN.mul_add(D96::ONE, D96::ZERO), Some(D96::MIN));
}

#[test]
fn d96_min_div_one_is_min() {
    assert_eq!(D96::MIN.checked_div(D96::ONE), Some(D96::MIN));
    assert_eq!(D96::MIN.try_div(D96::ONE), Ok(D96::MIN));
    // The Div operator must not panic ("divide by zero or overflow").
    assert_eq!(D96::MIN / D96::ONE, D96::MIN);
}

#[test]
fn d96_positive_2pow95_still_overflows() {
    // The asymmetry must be preserved: +2^95 is NOT representable (MAX = 2^95-1).
    let neg_one = d96_neg_one();
    assert_eq!(D96::MIN.checked_mul(neg_one), None); // MIN * -1 = +2^95
    assert_eq!(D96::MIN.checked_div(neg_one), None); // MIN / -1 = +2^95
    assert_eq!(D96::MAX.checked_mul(D96::MAX), None);
}

#[test]
fn d96_wrapping_mul_stays_in_96_bit_range() {
    for r in [
        D96::MAX.wrapping_mul(D96::MAX),
        D96::MIN.wrapping_mul(d96_neg_one()),
        D96::MIN.wrapping_mul(D96::MIN),
    ] {
        assert!(
            D96::from_raw_checked(r.to_raw()).is_some(),
            "wrapping_mul emitted out-of-96-bit raw {}",
            r.to_raw()
        );
    }
}

#[test]
fn d96_wrapping_div_stays_in_96_bit_range() {
    let r = D96::MIN.wrapping_div(d96_neg_one());
    assert!(
        D96::from_raw_checked(r.to_raw()).is_some(),
        "wrapping_div emitted out-of-96-bit raw {}",
        r.to_raw()
    );
}
