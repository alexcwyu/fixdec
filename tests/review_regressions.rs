//! Regression tests for bugs found in the 2026-06 deep review.
//!
//! Each test is named after the finding it locks down. They are written
//! test-first (red) before the corresponding fix.

use core::str::FromStr;
use fixdec::{D64, D96, DecimalError};

// ============================================================================
// [8] D64::from_f64 / try_from_f64 must REJECT out-of-range inputs, not
//     silently saturate to MAX/MIN.
// ============================================================================

#[test]
fn d64_from_f64_rejects_above_max() {
    // True D64::MAX ≈ 92233720368.54775807. Inputs above it must be None / Overflow.
    assert_eq!(D64::from_f64(92_233_720_369.0), None);
    assert_eq!(D64::try_from_f64(92_233_720_369.0), Err(DecimalError::Overflow));
    // Just-above-max from the finding's repro.
    assert_eq!(D64::from_f64(92_233_720_368.547_76), None);
}

#[test]
fn d64_from_f64_rejects_below_min() {
    assert_eq!(D64::from_f64(-92_233_720_369.0), None);
    assert_eq!(
        D64::try_from_f64(-92_233_720_369.0),
        Err(DecimalError::Underflow)
    );
}

#[test]
fn d64_from_f64_accepts_in_range() {
    assert!(D64::from_f64(92_233_720_368.0).is_some());
    assert_eq!(D64::from_f64(2.5), Some(D64::from_str("2.5").unwrap()));
    assert_eq!(D64::try_from_f64(-1.25), Ok(D64::from_str("-1.25").unwrap()));
}

// ============================================================================
// [9] D96::from_f64 / try_from_f64 must ACCEPT representable in-range values.
//     The magnitude guard was 1000x too small (≈3.96e13 vs true ≈3.96e16).
// ============================================================================

#[test]
fn d96_from_f64_accepts_large_in_range() {
    // 1e15 is far below D96::MAX (≈3.96e16); it was wrongly rejected.
    // (Value is approximate — 1e27 is not exactly f64-representable — so we check
    // acceptance and an approximate round-trip, not bit-exactness.)
    let got = D96::from_f64(1.0e15);
    assert!(got.is_some(), "D96::from_f64(1e15) must be Some");
    assert!(
        (got.unwrap().to_f64() - 1.0e15).abs() < 1.0e3,
        "round-trip ≈ 1e15"
    );

    // ~3e16 is still in range.
    assert!(D96::from_f64(3.0e16).is_some());
    assert!(D96::try_from_f64(1.0e15).is_ok());
}

#[test]
fn d96_from_f64_rejects_truly_out_of_range() {
    // Above ≈3.96e16 must still be rejected.
    assert_eq!(D96::from_f64(5.0e16), None);
    assert_eq!(D96::try_from_f64(5.0e16), Err(DecimalError::Overflow));
    assert_eq!(D96::try_from_f64(-5.0e16), Err(DecimalError::Underflow));
}

// ============================================================================
// [2] D64::abs() must not overflow at MIN (was: panic in debug / negative in
//     release). It now saturates to MAX, matching saturating_abs.
// ============================================================================

#[test]
fn d64_abs_saturates_at_min() {
    assert_eq!(D64::MIN.abs(), D64::MAX);
    assert_eq!(D64::from_raw(-5).abs(), D64::from_raw(5));
    assert_eq!(D64::from_raw(7).abs(), D64::from_raw(7));
    // num_traits::Signed::abs delegates to this and must also be safe.
}

// ============================================================================
// [3] D96::abs()/wrapping_abs()/wrapping_neg() must never emit an out-of-96-bit
//     value at MIN.
// ============================================================================

#[test]
fn d96_abs_saturates_at_min() {
    assert_eq!(D96::MIN.abs(), D96::MAX);
    assert!(D96::from_raw_checked(D96::MIN.abs().to_raw()).is_some());
}

#[test]
fn d96_wrapping_neg_abs_stay_in_range() {
    // Like i64: wrapping neg/abs of MIN wraps back to MIN (a valid value).
    assert!(D96::from_raw_checked(D96::MIN.wrapping_neg().to_raw()).is_some());
    assert!(D96::from_raw_checked(D96::MIN.wrapping_abs().to_raw()).is_some());
    assert_eq!(D96::MIN.wrapping_neg(), D96::MIN);
    assert_eq!(D96::MIN.wrapping_abs(), D96::MIN);
    // Ordinary values are unaffected.
    assert_eq!(D96::from_raw(-5).wrapping_neg(), D96::from_raw(5));
    assert_eq!(D96::from_raw(-5).wrapping_abs(), D96::from_raw(5));
}

// ============================================================================
// [7] D96 wrapping_add/sub must wrap at the 96-bit boundary, and
//     with_scale/try_with_scale must reject out-of-96-bit mantissas.
// ============================================================================

#[test]
fn d96_wrapping_add_sub_wrap_into_range() {
    let ulp = D96::from_raw(1);
    let r = D96::MAX.wrapping_add(ulp);
    assert!(D96::from_raw_checked(r.to_raw()).is_some(), "wrap stays valid");
    assert_eq!(r, D96::MIN); // MAX + 1 wraps to MIN
    let r2 = D96::MIN.wrapping_sub(ulp);
    assert_eq!(r2, D96::MAX); // MIN - 1 wraps to MAX
}

#[test]
fn d96_with_scale_rejects_out_of_range() {
    let over = D96::MAX.to_raw() + 1000;
    assert_eq!(D96::try_with_scale(over, 12), None); // fast path (scale == DECIMALS)
    // multiply branch: MAX * 10 exceeds the 96-bit range
    assert_eq!(D96::try_with_scale(D96::MAX.to_raw(), 11), None);
    // a valid in-range mantissa still works
    assert!(D96::try_with_scale(12345, 12).is_some());
}

// ============================================================================
// [1],[4],[5] round()/round_dp()/ceil()/floor() must not overflow near MAX/MIN
//     (was: panic in debug, sign-flip in release for D64; out-of-range for D96).
//     They now saturate to the representable boundary.
// ============================================================================

#[test]
fn d64_round_ceil_saturate_near_max() {
    let r = D64::MAX.round();
    assert!(r > D64::ZERO && r <= D64::MAX, "round(MAX) stays valid positive");
    let c = D64::MAX.ceil();
    assert!(c > D64::ZERO && c <= D64::MAX, "ceil(MAX) stays valid positive");
    let rd = D64::MAX.round_dp(0);
    assert!(rd > D64::ZERO && rd <= D64::MAX, "round_dp(MAX,0) stays valid");
}

#[test]
fn d64_floor_saturates_near_min() {
    let f = D64::MIN.floor();
    assert!(f < D64::ZERO && f >= D64::MIN, "floor(MIN) stays valid negative");
}

#[test]
fn d96_round_ceil_saturate_near_max() {
    let r = D96::MAX.round();
    assert!(D96::from_raw_checked(r.to_raw()).is_some());
    assert!(r > D96::ZERO && r <= D96::MAX);
    let c = D96::MAX.ceil();
    assert!(D96::from_raw_checked(c.to_raw()).is_some());
    assert!(c <= D96::MAX);
    let rd = D96::MAX.round_dp(0);
    assert!(D96::from_raw_checked(rd.to_raw()).is_some());
}

#[test]
fn d96_floor_saturates_near_min() {
    let f = D96::MIN.floor();
    assert!(D96::from_raw_checked(f.to_raw()).is_some());
    assert!(f >= D96::MIN);
}

// ============================================================================
// [10] D64::MIN must round-trip through its own Display -> from_str_exact.
// ============================================================================

#[test]
fn d64_min_string_roundtrips() {
    let s = format!("{}", D64::MIN);
    assert_eq!(s, "-92233720368.54775808");
    assert_eq!(D64::from_str_exact(&s), Ok(D64::MIN));
    assert_eq!(D64::from_str(&s), Ok(D64::MIN));
    // genuine over/underflow is still rejected
    assert_eq!(
        D64::from_str_exact("-92233720368.54775809"),
        Err(DecimalError::Overflow)
    );
    // +2^63 (one past i64::MAX) is too big on the positive side
    assert_eq!(
        D64::from_str_exact("92233720368.54775808"),
        Err(DecimalError::Overflow)
    );
}

// ============================================================================
// [6] D64 `%` operator: MIN % -ulp overflows (like integer %). It now routes
//     through checked_rem and panics with a clear message (documented).
// ============================================================================

#[test]
fn d64_rem_operator_matches_checked_for_normal_inputs() {
    let a = D64::from_str("10.5").unwrap();
    let b = D64::from_str("3.2").unwrap();
    assert_eq!(a % b, a.checked_rem(b).unwrap());
}

#[test]
#[should_panic]
fn d64_rem_min_by_neg_ulp_panics() {
    // checked_rem returns None here; the operator must panic, not silently wrap.
    let _ = D64::MIN % D64::from_raw(-1);
}
