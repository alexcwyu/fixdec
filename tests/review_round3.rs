//! Regression tests for deep-review round 3 (2026-06-19).
//!
//! Each test pins a confirmed finding from an adversarial multi-agent review so
//! that an independent cross-review (GPT / DeepSeek) can re-verify the fix.
//! Findings are grouped: D96 96-bit invariant + signed-MIN (multiply/divide),
//! parser overflow & lossy/exact parity, Display rounding carry & banker's,
//! lossy with_scale range checks, D96 integer From → TryFrom, and assorted
//! predicate/interop correctness.

use core::str::FromStr;
use fixdec::{D64, D96, DecimalError};

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

// ===========================================================================
// [0] D64 integer-string parsing overflow (wraps i64, evades sign guard)
// ===========================================================================

#[test]
fn d64_parse_out_of_range_integer_is_overflow() {
    // 2^64, 2^64+1, 2^65 used to wrap into small valid-looking values.
    for s in [
        "18446744073709551616",   // 2^64        -> was Ok(0)
        "18446744073709551617",   // 2^64 + 1    -> was Ok(1)
        "36893488147419103232",   // 2^65        -> was Ok(0)
        "184467440737095516170",  // 10 * 2^64   -> was Ok(10)
        "18446744073709551616.5", // 2^64 + .5   -> was Ok(0.5)
    ] {
        assert_eq!(D64::from_str(s), Err(DecimalError::Overflow), "exact {s:?}");
        assert_eq!(D64::from_str_lossy(s), Err(DecimalError::Overflow), "lossy {s:?}");
    }
    // Genuine boundary values still parse.
    assert_eq!(D64::from_str("92233720368.54775807").unwrap(), D64::MAX);
    assert_eq!(D64::from_str_lossy("92233720368.54775807").unwrap(), D64::MAX);
}

// ===========================================================================
// [1] D96 integer-string parsing overflow (panics in debug, wraps in release)
// ===========================================================================

#[test]
fn d96_parse_huge_integer_is_overflow_not_panic() {
    for s in [
        "340282366920938463463374607431768211456",  // 2^128 (39 digits)
        "340282366920938463463374607431768211457",  // 2^128 + 1
        "999999999999999999999999999999999999999",  // 39 nines
    ] {
        assert_eq!(D96::from_str(s), Err(DecimalError::Overflow), "exact {s:?}");
        assert_eq!(D96::from_str_lossy(s), Err(DecimalError::Overflow), "lossy {s:?}");
    }
}

// ===========================================================================
// [2] D96::from_str_lossy must reject malformed trailing characters
// ===========================================================================

#[test]
fn d96_lossy_rejects_malformed_trailing_chars() {
    for s in [
        "1.0000000000001x",            // <5 branch: 'x' was silently ignored
        "1.0000000000009x",            // >5 branch: 'x' was silently ignored
        "-2.0000000000004hello",       // <5 branch with a word
        "1.0000000000001234567890abc", // garbage deep in the tail
    ] {
        assert!(
            D96::from_str_lossy(s).is_err(),
            "lossy should reject malformed {s:?}, got {:?}",
            D96::from_str_lossy(s)
        );
    }
    // A well-formed over-precise value still rounds (banker's).
    assert_eq!(
        D96::from_str_lossy("1.0000000000009").unwrap().to_string(),
        "1.000000000001"
    );
}

// ===========================================================================
// [3] D64::from_str_lossy must accept D64::MIN (parity with from_str_exact)
// ===========================================================================

#[test]
fn d64_lossy_accepts_min() {
    let s = "-92233720368.54775808";
    assert_eq!(D64::from_str_exact(s).unwrap(), D64::MIN);
    assert_eq!(D64::from_str_lossy(s).unwrap(), D64::MIN);
    // and they still agree on MIN+1
    let s1 = "-92233720368.54775807";
    assert_eq!(D64::from_str_lossy(s1).unwrap(), D64::from_str_exact(s1).unwrap());
}

// ===========================================================================
// [19] D96 `{:.N}` Display must propagate the rounding carry into the integer
// ===========================================================================

#[test]
fn d96_display_precision_carry_propagates() {
    assert_eq!(format!("{:.1}", D96::from_str("0.95").unwrap()), "1.0");
    assert_eq!(format!("{:.1}", D96::from_str("9.99").unwrap()), "10.0");
    assert_eq!(format!("{:.0}", D96::from_str("0.9999999").unwrap()), "1");
    assert_eq!(format!("{:.1}", D96::from_str("-0.96").unwrap()), "-1.0");
    assert_eq!(format!("{:.2}", D96::from_str("0.999").unwrap()), "1.00");
}

// ===========================================================================
// [20] D64 `{:.N}` Display must use banker's rounding (match round_dp / D96)
// ===========================================================================

#[test]
fn d64_display_precision_uses_bankers() {
    // Exact ties round to even, agreeing with round_dp and with D96 Display.
    assert_eq!(format!("{:.0}", D64::from_str("2.5").unwrap()), "2");
    assert_eq!(format!("{:.0}", D64::from_str("3.5").unwrap()), "4");
    assert_eq!(format!("{:.2}", D64::from_str("0.125").unwrap()), "0.12");

    for s in ["2.5", "0.125", "1.015", "1.005", "-2.5"] {
        let d = D64::from_str(s).unwrap();
        assert_eq!(
            format!("{:.0}", d),
            d.round_dp(0).to_string(),
            "Display {{:.0}} vs round_dp(0) for {s}"
        );
        assert_eq!(
            format!("{:.2}", d),
            format!("{:.2}", D96::from_str(s).unwrap()),
            "D64 vs D96 Display for {s}"
        );
    }
}

// ===========================================================================
// [21] D64 and D96 must agree on `{:.N}` of zero
// ===========================================================================

#[test]
fn d64_d96_zero_precision_agree() {
    assert_eq!(format!("{:.2}", D64::ZERO), "0.00");
    assert_eq!(format!("{:.2}", D64::ZERO), format!("{:.2}", D96::ZERO));
    assert_eq!(format!("{:.0}", D64::ZERO), format!("{:.0}", D96::ZERO));
    assert_eq!(format!("{:.5}", D64::ZERO), format!("{:.5}", D96::ZERO));
}

// ===========================================================================
// [13] D96 lossy with_scale constructors must enforce the 96-bit range
// ===========================================================================

#[test]
fn d96_try_with_scale_lossy_range_checks() {
    let over = D96::MAX.to_raw() + 1000;
    // scale_diff == 0 store path
    assert_eq!(D96::try_with_scale_lossy(over, 12), None);
    // fast multiply path (scale < 12): MAX * 10 overflows the 96-bit range
    assert_eq!(D96::try_with_scale_lossy(D96::MAX.to_raw(), 11), None);
    // slow rounding path (scale > 12): 1e35 / 10 = 1e34 >> MAX
    let huge: i128 = 100_000_000_000_000_000_000_000_000_000_000_000; // 1e35
    assert_eq!(D96::try_with_scale_lossy(huge, 13), None);
    // representable values still succeed
    assert_eq!(
        D96::try_with_scale_lossy(12345, 2),
        Some(D96::from_str("123.45").unwrap())
    );
}

#[test]
#[should_panic]
fn d96_with_scale_lossy_store_path_panics_on_overflow() {
    let _ = D96::with_scale_lossy(D96::MAX.to_raw() + 1000, 12);
}

#[test]
#[should_panic]
fn d96_with_scale_lossy_fast_path_panics_on_overflow() {
    let _ = D96::with_scale_lossy(D96::MAX.to_raw(), 11);
}

#[test]
#[should_panic]
fn d96_with_scale_lossy_slow_path_panics_on_overflow() {
    let huge: i128 = 100_000_000_000_000_000_000_000_000_000_000_000; // 1e35
    let _ = D96::with_scale_lossy(huge, 13);
}

// ===========================================================================
// [15] D96 integer conversions from i64/u64 must be fallible & range-checked
// ===========================================================================

#[test]
fn d96_integer_tryfrom_is_range_checked() {
    assert_eq!(D96::try_from(42_i64).unwrap().to_i128(), 42);
    assert_eq!(D96::try_from(1_000_000_u64).unwrap().to_i128(), 1_000_000);

    // floor(D96::MAX integer part) is representable; one past it overflows.
    let max_int: i64 = 39_614_081_257_132_168;
    assert!(D96::try_from(max_int).is_ok());
    assert!(D96::try_from(-max_int).is_ok());
    assert_eq!(D96::try_from(max_int + 1), Err(DecimalError::Overflow));

    // Values beyond D96's ~±3.96e16 integer range are rejected, never silently
    // truncated to an out-of-96-bit value.
    assert_eq!(D96::try_from(i64::MAX), Err(DecimalError::Overflow));
    assert_eq!(D96::try_from(u64::MAX), Err(DecimalError::Overflow));
    assert_eq!(D96::try_from(i64::MIN), Err(DecimalError::Overflow));
}

// ===========================================================================
// [6] is_multiple_of must not panic on the signed-MIN % -ulp overflow case
// ===========================================================================

#[test]
fn is_multiple_of_signed_min_no_overflow_panic() {
    // i64::MIN % -1 overflows the native `%`; the predicate must stay total.
    assert!(D64::MIN.is_multiple_of(D64::from_raw(-1)));
    assert!(D96::MIN.is_multiple_of(D96::from_raw(-1)));

    // Ordinary behaviour is unchanged.
    assert!(D64::from_i32(6).is_multiple_of(D64::from_i32(3)));
    assert!(!D64::from_i32(7).is_multiple_of(D64::from_i32(3)));
    assert!(D64::ZERO.is_multiple_of(D64::ZERO));
    assert!(!D64::ONE.is_multiple_of(D64::ZERO));
}

// ===========================================================================
// [5] Trailing-dot mantissa grammar must be consistent (plain vs scientific)
// ===========================================================================

#[test]
fn trailing_dot_mantissa_is_consistent() {
    // "1." parses as 1 in the PLAIN path, matching the scientific path
    // ("1.e3" = 1000) and rust_decimal; previously only the scientific half
    // accepted a trailing dot.
    assert_eq!(D64::from_str_exact("1.").unwrap(), D64::ONE);
    assert_eq!(D64::from_str_lossy("1.").unwrap(), D64::ONE);
    assert_eq!(D64::from_str_exact("-5.").unwrap().to_string(), "-5");
    assert_eq!(D64::from_str_exact("1.e3").unwrap().to_string(), "1000");
    assert_eq!(D96::from_str_exact("1.").unwrap(), D96::ONE);
    assert_eq!(D96::from_str_lossy("1.").unwrap(), D96::ONE);

    // Leading dot with digits still works; a lone "." stays invalid everywhere.
    assert_eq!(D64::from_str_exact(".5").unwrap().to_string(), "0.5");
    for bad in [".", "-.", "+."] {
        assert!(D64::from_str_exact(bad).is_err(), "exact {bad:?}");
        assert!(D64::from_str_lossy(bad).is_err(), "lossy {bad:?}");
        assert!(D96::from_str_exact(bad).is_err(), "d96 exact {bad:?}");
        assert!(D96::from_str_lossy(bad).is_err(), "d96 lossy {bad:?}");
    }
}

// ===========================================================================
// [Codex adversarial review] D96::from_i64 / from_u64 must preserve the 96-bit
// invariant — they were public, safe-looking, infallible constructors that
// minted out-of-range values (D96 integer range is only ~±3.96e16, far below
// i64/u64). Now they match D64 and return a range-checked Option.
// ===========================================================================

#[test]
fn d96_inherent_int_constructors_are_range_checked() {
    assert_eq!(D96::from_i64(i64::MAX), None);
    assert_eq!(D96::from_i64(i64::MIN), None);
    assert_eq!(D96::from_u64(u64::MAX), None);

    // floor(D96::MAX integer part) is representable; one past it overflows.
    let max_int: i64 = 39_614_081_257_132_168;
    assert!(D96::from_i64(max_int).is_some());
    assert!(D96::from_i64(-max_int).is_some());
    assert_eq!(D96::from_i64(max_int + 1), None);
    assert!(D96::from_u64(max_int as u64).is_some());
    assert_eq!(D96::from_u64(max_int as u64 + 1), None);

    // In-range values still convert.
    assert_eq!(D96::from_i64(100).unwrap().to_i128(), 100);
    assert_eq!(D96::from_u64(1_000_000).unwrap().to_i128(), 1_000_000);

    // No public safe constructor may produce an out-of-96-bit raw value.
    for v in [0_i64, 1, -1, max_int, -max_int, 12_345] {
        let d = D96::from_i64(v).unwrap();
        assert!(
            D96::from_raw_checked(d.to_raw()).is_some(),
            "from_i64({v}) produced out-of-range raw {}",
            d.to_raw()
        );
    }
}
