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
    assert_eq!(format!("{:.0}", D96::from_str("3.5").unwrap()), "4"); // banker's (tie -> even)
    assert_eq!(format!("{:.6}", D96::from_str("3.14159265").unwrap()), "3.141593");
}

// ---------------------------------------------------------------------------
// [F1] from_str_lossy must accept scientific / E-notation, rounding to scale
// with banker's half-even (exact parsing rejects sub-scale precision).
// Pre-fix, the verification showed from_str_lossy("1.5e3") = Err(InvalidFormat).
// ---------------------------------------------------------------------------

#[test]
fn d64_from_str_lossy_accepts_scientific() {
    // Representable exactly: lossy agrees with exact.
    assert_eq!(D64::from_str_lossy("1.5e2").unwrap(), D64::from_str("150").unwrap());
    assert_eq!(D64::from_str_lossy("1E-3").unwrap(), D64::from_str("0.001").unwrap());
    assert_eq!(D64::from_str_lossy("-2.5e-1").unwrap(), D64::from_str("-0.25").unwrap());
    assert_eq!(D64::from_str_lossy("+1e0").unwrap(), D64::ONE);
    // Below the 8-dp scale: exact parse rejects (PrecisionLoss); lossy rounds.
    assert_eq!(D64::from_str_exact("1e-13"), Err(fixdec::DecimalError::PrecisionLoss));
    assert_eq!(D64::from_str_lossy("1e-13").unwrap(), D64::ZERO);
    assert_eq!(D64::from_str_lossy("9.9e-9").unwrap(), D64::from_raw(1)); // 0.0000000099 -> 0.00000001
    // Banker's half-even on the exact tie (9th fractional digit == 5).
    assert_eq!(D64::from_str_lossy("1.234567895e0").unwrap(), D64::from_str("1.2345679").unwrap());
    assert_eq!(D64::from_str_lossy("1.234567885e0").unwrap(), D64::from_str("1.23456788").unwrap());
    // Malformed exponent is still rejected.
    assert!(D64::from_str_lossy("1e").is_err());
    assert!(D64::from_str_lossy("1e+").is_err());
}

#[test]
fn d96_from_str_lossy_accepts_scientific() {
    assert_eq!(D96::from_str_lossy("1.5e2").unwrap(), D96::from_str("150").unwrap());
    assert_eq!(D96::from_str_lossy("1E-6").unwrap(), D96::from_str("0.000001").unwrap());
    assert_eq!(D96::from_str_lossy("-2.5e-1").unwrap(), D96::from_str("-0.25").unwrap());
    // Below the 12-dp scale: lossy rounds to zero rather than erroring.
    assert_eq!(D96::from_str_exact("1e-15"), Err(fixdec::DecimalError::PrecisionLoss));
    assert_eq!(D96::from_str_lossy("1e-15").unwrap(), D96::ZERO);
    // Banker's half-even on the exact tie (13th fractional digit == 5).
    assert_eq!(
        D96::from_str_lossy("1.2345678901235e0").unwrap(),
        D96::from_str("1.234567890124").unwrap()
    );
}
