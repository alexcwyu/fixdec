//! Regression tests for Codex adversarial review round 8 (2026-06-20).
//!
//! [F2] `format!("{:.N}", x)` capped/truncated N at the native scale (8 for D64,
//!      12 for D96) instead of zero-padding to N like `core`'s float Display.
//!      Worse: for a whole number at N >= native scale, D64 emitted no decimal
//!      point at all (`{:.10}` of 1.0 -> "1").

use core::str::FromStr;
use fixdec::{D64, D96};

#[test]
fn d64_format_precision_pads_above_native_scale() {
    // Whole number: previously produced "1" (no decimal point at all).
    assert_eq!(format!("{:.10}", D64::ONE), "1.0000000000");
    assert_eq!(format!("{:.8}", D64::ONE), "1.00000000");
    // Fractional value padded beyond the 8 native digits.
    assert_eq!(format!("{:.10}", D64::from_str("1.5").unwrap()), "1.5000000000");
    assert_eq!(format!("{:.12}", D64::from_str("1.5").unwrap()), "1.500000000000");
    // Negative, including |int| == 0.
    assert_eq!(format!("{:.10}", D64::from_str("-1.5").unwrap()), "-1.5000000000");
    assert_eq!(format!("{:.12}", D64::from_str("-0.25").unwrap()), "-0.250000000000");
    // Parity with core float Display at the same precision.
    assert_eq!(format!("{:.10}", D64::from_str("1.5").unwrap()), format!("{:.10}", 1.5f64));
}

#[test]
fn d64_format_precision_below_native_scale_unchanged() {
    // The < native-scale branch already rounded+padded correctly; keep it.
    assert_eq!(format!("{:.2}", D64::from_str("1.5").unwrap()), "1.50");
    assert_eq!(format!("{:.0}", D64::from_str("1.7").unwrap()), "2"); // banker's
    assert_eq!(format!("{:.4}", D64::from_str("3.14159").unwrap()), "3.1416");
}

#[test]
fn d96_format_precision_pads_above_native_scale() {
    // Native scale is 12; request 15 -> three padding zeros.
    assert_eq!(format!("{:.15}", D96::ONE), "1.000000000000000");
    assert_eq!(format!("{:.12}", D96::ONE), "1.000000000000");
    assert_eq!(format!("{:.15}", D96::from_str("1.5").unwrap()), "1.500000000000000");
    assert_eq!(format!("{:.15}", D96::from_str("-0.25").unwrap()), "-0.250000000000000");
    assert_eq!(format!("{:.15}", D96::from_str("1.5").unwrap()), format!("{:.15}", 1.5f64));
}

#[test]
fn d96_format_precision_below_native_scale_unchanged() {
    assert_eq!(format!("{:.2}", D96::from_str("1.5").unwrap()), "1.50");
    assert_eq!(format!("{:.0}", D96::from_str("2.5").unwrap()), "2"); // banker's
    assert_eq!(format!("{:.6}", D96::from_str("3.14159265").unwrap()), "3.141593");
}
