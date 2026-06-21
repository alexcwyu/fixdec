//! Targeted edge-case tests for D64 and D96 covering overflow boundaries,
//! signed-min handling, rounding ties, parsing limits, Display round-trips,
//! division corner cases, and the math helpers.

use core::str::FromStr;
use fixdec::{D64, D96, DecimalError};

// ===========================================================================
// D64 — overflow & signed-min boundaries
// ===========================================================================

#[test]
fn d64_add_sub_overflow_boundaries() {
    assert_eq!(D64::MAX.checked_add(D64::from_raw(1)), None);
    assert_eq!(D64::MAX.saturating_add(D64::ONE), D64::MAX);
    assert_eq!(D64::MIN.checked_sub(D64::from_raw(1)), None);
    assert_eq!(D64::MIN.saturating_sub(D64::ONE), D64::MIN);
    assert_eq!(D64::MAX.wrapping_add(D64::from_raw(1)), D64::MIN);
}

#[test]
fn d64_signed_min_abs_neg() {
    // i64::MIN has no positive counterpart.
    assert_eq!(D64::MIN.checked_neg(), None);
    assert_eq!(D64::MIN.checked_abs(), None);
    assert_eq!(D64::MIN.saturating_neg(), D64::MAX);
    assert_eq!(D64::MIN.saturating_abs(), D64::MAX);
    assert_eq!(D64::MAX.checked_neg(), Some(D64::from_raw(-i64::MAX)));
}

#[test]
fn d64_mul_overflow() {
    assert_eq!(D64::MAX.checked_mul(D64::MAX), None);
    assert_eq!(D64::MAX.checked_mul(D64::ONE), Some(D64::MAX));
    assert_eq!(D64::MAX.saturating_mul(D64::MAX), D64::MAX);
    assert_eq!(D64::MIN.saturating_mul(D64::from_i32(2)), D64::MIN);
}

// ===========================================================================
// D64 — division corner cases
// ===========================================================================

#[test]
fn d64_division_corner_cases() {
    assert_eq!(D64::ONE.checked_div(D64::ZERO), None);
    assert_eq!(
        D64::ONE.try_div(D64::ZERO),
        Err(DecimalError::DivisionByZero)
    );
    assert_eq!(D64::ZERO.checked_div(D64::ONE), Some(D64::ZERO));

    // 1/3 truncates toward zero, *3 = 0.99999999
    let third = D64::ONE.checked_div(D64::from_i32(3)).unwrap();
    assert_eq!(third.to_raw(), 33_333_333);
    assert_eq!((third * D64::from_i32(3)).to_raw(), 99_999_999);

    // dividing MIN by -1 overflows (|result| = 2^63 > i64::MAX)
    assert_eq!(D64::MIN.checked_div(D64::from_i32(-1)), None);

    // huge / tiny overflows the result range
    assert_eq!(D64::MAX.checked_div(D64::from_raw(1)), None);
}

// ===========================================================================
// D64 — rounding (floor/ceil/trunc/fract/round) incl. negatives & banker's
// ===========================================================================

#[test]
fn d64_rounding_negatives() {
    let n = D64::from_str("-1.5").unwrap();
    assert_eq!(n.floor().to_string(), "-2");
    assert_eq!(n.ceil().to_string(), "-1");
    assert_eq!(n.trunc().to_string(), "-1");
    assert_eq!(n.fract().to_string(), "-0.5");

    let p = D64::from_str("1.5").unwrap();
    assert_eq!(p.floor().to_string(), "1");
    assert_eq!(p.ceil().to_string(), "2");
}

#[test]
fn d64_bankers_rounding_ties() {
    // round-half-to-even on exact .5 ties
    assert_eq!(D64::from_str("0.5").unwrap().round().to_i64(), 0);
    assert_eq!(D64::from_str("1.5").unwrap().round().to_i64(), 2);
    assert_eq!(D64::from_str("2.5").unwrap().round().to_i64(), 2);
    assert_eq!(D64::from_str("3.5").unwrap().round().to_i64(), 4);
    assert_eq!(D64::from_str("-2.5").unwrap().round().to_i64(), -2);

    // round_dp banker's rounding
    assert_eq!(D64::from_str("1.005").unwrap().round_dp(2).to_string(), "1");
    assert_eq!(
        D64::from_str("1.015").unwrap().round_dp(2).to_string(),
        "1.02"
    );
}

// ===========================================================================
// D64 — parsing edge cases
// ===========================================================================

#[test]
fn d64_parse_valid_forms() {
    assert_eq!(D64::from_str("0").unwrap(), D64::ZERO);
    assert_eq!(D64::from_str("-0").unwrap(), D64::ZERO);
    assert_eq!(D64::from_str("+1").unwrap(), D64::ONE);
    assert_eq!(D64::from_str("  1.5  ").unwrap().to_string(), "1.5");
    assert_eq!(
        D64::from_str("000123.45000000").unwrap().to_string(),
        "123.45"
    );
    assert_eq!(D64::from_str("0.00000001").unwrap().to_raw(), 1);
}

#[test]
fn d64_parse_invalid_forms() {
    // Note: "1e5" is now VALID (scientific notation). Malformed scientific forms
    // ("1e", "e", "1e+", "1e5e3") must still be rejected.
    for bad in [
        "", "  ", "-", "+", ".", "abc", "1.2.3", "--1", "1 2", "0x10", "1.-2", "1e", "e", "1e+",
        "1e5e3",
    ] {
        assert!(
            D64::from_str(bad).is_err(),
            "expected parse error for {bad:?}"
        );
    }
}

#[test]
fn d64_parse_precision_and_overflow() {
    // exact rejects >8 decimals; lossy rounds (banker's)
    assert_eq!(
        D64::from_str_exact("1.123456789"),
        Err(DecimalError::PrecisionLoss)
    );
    assert_eq!(
        D64::from_str_lossy("0.123456785").unwrap().to_string(),
        "0.12345678" // round-half-to-even
    );

    // MAX parses; one ulp past MAX overflows
    assert_eq!(D64::from_str("92233720368.54775807").unwrap(), D64::MAX);
    assert_eq!(
        D64::from_str("92233720368.54775808"),
        Err(DecimalError::Overflow)
    );
}

#[test]
fn d64_max_string_roundtrip() {
    assert_eq!(D64::MAX.to_string(), "92233720368.54775807");
    assert_eq!(D64::from_str(&D64::MAX.to_string()).unwrap(), D64::MAX);

    // MIN now round-trips too (parsing builds the magnitude in i128 and applies
    // the sign with asymmetric bounds, so -2^63 is reachable).
    assert_eq!(D64::MIN.to_string(), "-92233720368.54775808");
    assert_eq!(D64::from_str(&D64::MIN.to_string()).unwrap(), D64::MIN);
}

// ===========================================================================
// D64 — float, display, math helpers
// ===========================================================================

#[test]
fn d64_from_f64_edge() {
    assert_eq!(D64::from_f64(f64::NAN), None);
    assert_eq!(D64::from_f64(f64::INFINITY), None);
    assert_eq!(D64::from_f64(f64::NEG_INFINITY), None);
    assert_eq!(D64::from_f64(1e30), None); // out of range
    assert_eq!(D64::from_f64(0.0).unwrap(), D64::ZERO);
}

#[test]
fn d64_display_forms() {
    assert_eq!(D64::ZERO.to_string(), "0");
    assert_eq!(D64::ONE.to_string(), "1");
    assert_eq!(D64::from_raw(1).to_string(), "0.00000001");
    assert_eq!(D64::from_str("-0.1").unwrap().to_string(), "-0.1");
    assert_eq!(D64::from_str("100.00").unwrap().to_string(), "100");
}

#[test]
fn d64_powi_recip() {
    assert_eq!(D64::from_i32(2).powi(0), Some(D64::ONE));
    assert_eq!(D64::from_i32(2).powi(10).unwrap().to_i64(), 1024);
    assert_eq!(
        D64::from_i32(2).powi(-1).unwrap(),
        D64::from_str("0.5").unwrap()
    );
    assert_eq!(D64::ZERO.recip(), None);
    assert_eq!(
        D64::from_i32(4).recip().unwrap(),
        D64::from_str("0.25").unwrap()
    );
}

#[test]
fn d64_basis_points_roundtrip() {
    assert_eq!(D64::from_basis_points(50).unwrap().to_raw(), 500_000); // 0.005
    assert_eq!(D64::from_basis_points(10_000).unwrap(), D64::ONE);
    assert_eq!(D64::from_str("0.005").unwrap().to_basis_points(), 50);
    // large value: no i64 overflow in the conversion
    assert_eq!(D64::from_i32(10_000_000).to_basis_points(), 100_000_000_000);
}

// ===========================================================================
// D96 — boundaries, constants, large division, parsing
// ===========================================================================

#[test]
fn d96_overflow_boundaries() {
    assert_eq!(D96::MAX.checked_add(D96::from_raw(1)), None);
    assert_eq!(D96::MAX.saturating_add(D96::ONE), D96::MAX);
    assert_eq!(D96::MIN.checked_neg(), None); // -(-2^95) = 2^95 > MAX
    assert_eq!(D96::MIN.saturating_neg(), D96::MAX);
    assert_eq!(D96::MAX.checked_mul(D96::MAX), None);
}

#[test]
fn d96_crypto_constants() {
    assert_eq!(D96::SATOSHI.to_raw(), 10_000); // 1e-8 * 1e12
    assert_eq!(D96::GWEI.to_raw(), 1_000); // 1e-9 * 1e12
    assert_eq!(D96::MICRO_GWEI.to_raw(), 1); // 1e-12 * 1e12
    assert_eq!(D96::SATOSHI.to_string(), "0.00000001");
}

#[test]
fn d96_large_division_correct() {
    // Regression for the base-2^64 remainder-overflow bug (divisor >= 2^64).
    let num = D96::from_i64(1_000_000_000_000_000).unwrap(); // 1e15
    let den = D96::from_i64(20_000_000).unwrap(); // raw 2e19 >= 2^64
    assert_eq!(num.checked_div(den).unwrap().to_string(), "50000000");
    assert_eq!(D96::ONE.checked_div(den).unwrap().to_string(), "0.00000005");
}

#[test]
fn d96_slow_path_mul() {
    // raw 2e19 >= 2^64 forces the 192-bit multiply path
    let a = D96::from_i64(20_000_000).unwrap();
    assert_eq!(
        a.checked_mul(D96::from_i64(1_000).unwrap())
            .unwrap()
            .to_string(),
        "20000000000"
    );
}

#[test]
fn d96_parse_precision() {
    assert_eq!(D96::from_str("0.000000000001").unwrap(), D96::MICRO_GWEI);
    assert_eq!(
        D96::from_str("2500.123456789012").unwrap().to_string(),
        "2500.123456789012"
    );
    // 13 decimals: exact rejects, lossy rounds to 12
    assert_eq!(
        D96::from_str_exact("1.1234567890123"),
        Err(DecimalError::PrecisionLoss)
    );
}

#[test]
fn d96_max_string_roundtrip() {
    let s = D96::MAX.to_string();
    assert_eq!(D96::from_str(&s).unwrap(), D96::MAX);
}

#[test]
fn d96_to_i128_truncation() {
    let v = D96::from_str("123.999999999999").unwrap();
    assert_eq!(v.to_i128(), 123);
    let n = D96::from_str("-123.999999999999").unwrap();
    assert_eq!(n.to_i128(), -123);
}
