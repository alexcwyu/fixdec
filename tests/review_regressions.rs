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
