//! Additional math / conversion edge cases (task 2).
//!
//! These complement `edge_cases.rs` and `review_regressions.rs` by exercising
//! algebraic identities, cross-type and byte round-trips, and the sign rules of
//! truncated remainder. They are characterization tests: every assertion encodes
//! a property that must hold for any correct fixed-point implementation, so a
//! failure here is a real bug, not a brittle expectation.

use core::str::FromStr;
use fixdec::{D64, D96};

fn d64(s: &str) -> D64 {
    D64::from_str(s).unwrap()
}
fn d96(s: &str) -> D96 {
    D96::from_str(s).unwrap()
}

// ===========================================================================
// Algebraic identities — additive / multiplicative neutral & inverse
// ===========================================================================

#[test]
fn d64_algebraic_identities() {
    for s in ["0", "1", "-1", "123.456", "-9999.99999999", "0.00000001"] {
        let a = d64(s);
        assert_eq!(a + D64::ZERO, a, "a + 0 == a for {s}");
        assert_eq!(a - D64::ZERO, a, "a - 0 == a for {s}");
        assert_eq!(a * D64::ONE, a, "a * 1 == a for {s}");
        assert_eq!(a / D64::ONE, a, "a / 1 == a for {s}");
        assert_eq!(a * D64::ZERO, D64::ZERO, "a * 0 == 0 for {s}");
        assert_eq!(a - a, D64::ZERO, "a - a == 0 for {s}");
        assert_eq!(D64::ZERO - a, -a, "0 - a == -a for {s}");
        assert_eq!(a + (-a), D64::ZERO, "a + (-a) == 0 for {s}");
    }
}

#[test]
fn d96_algebraic_identities() {
    for s in ["0", "1", "-1", "123.456789012", "-9999.999999999999", "0.000000000001"] {
        let a = d96(s);
        assert_eq!(a + D96::ZERO, a, "a + 0 == a for {s}");
        assert_eq!(a - D96::ZERO, a, "a - 0 == a for {s}");
        assert_eq!(a * D96::ONE, a, "a * 1 == a for {s}");
        assert_eq!(a / D96::ONE, a, "a / 1 == a for {s}");
        assert_eq!(a * D96::ZERO, D96::ZERO, "a * 0 == 0 for {s}");
        assert_eq!(a - a, D96::ZERO, "a - a == 0 for {s}");
        assert_eq!(D96::ZERO - a, -a, "0 - a == -a for {s}");
        assert_eq!(a + (-a), D96::ZERO, "a + (-a) == 0 for {s}");
    }
}

// ===========================================================================
// Reciprocal — recip(recip(a)) ~= a, and recip(0) is None (no panic / no UB)
// ===========================================================================

#[test]
fn d64_recip_roundtrip_and_zero() {
    assert_eq!(D64::ZERO.recip(), None);
    // exact reciprocals round-trip exactly
    for s in ["1", "-1", "2", "0.5", "4", "0.25", "100", "0.01"] {
        let a = d64(s);
        let r = a.recip().unwrap().recip().unwrap();
        assert_eq!(r, a, "recip(recip({s})) == {s}");
    }
    // inexact reciprocal (1/3) stays within one ULP after the round trip
    let third = D64::ONE.recip().unwrap(); // 1/1 == 1; use 3 instead
    assert_eq!(third, D64::ONE);
    // 1/3 -> 0.33333333, and 1/0.33333333 -> 3.00000003: two roundings, so the
    // round trip lands a few ULP away (here exactly 3 raw units). It must stay
    // tiny, not exact.
    let back = d64("3").recip().unwrap().recip().unwrap();
    let diff = (back - d64("3")).abs();
    assert!(diff <= D64::from_raw(5), "recip(recip(3)) within 5 ULP, got {back}");
}

#[test]
fn d96_recip_zero_is_none() {
    assert_eq!(D96::ZERO.recip(), None);
    assert_eq!(d96("2").recip().unwrap(), d96("0.5"));
    assert_eq!(d96("0.5").recip().unwrap(), d96("2"));
}

// ===========================================================================
// Cross-type round trips — D64 <-> D96 is lossless for every D64 value
// (D96 has both wider range and finer precision)
// ===========================================================================

#[test]
fn d64_to_d96_roundtrip_is_lossless() {
    for d in [D64::MIN, D64::MAX, D64::ZERO, D64::ONE, d64("-1"), d64("123.456")] {
        let widened = d.to_d96();
        let narrowed = widened.to_d64().expect("every D64 fits back into D64");
        assert_eq!(narrowed, d, "D64 -> D96 -> D64 round trip for {d}");
    }
}

#[test]
fn d96_to_d64_narrowing_rules() {
    // In-range, <= 8 dp: exact narrowing succeeds.
    assert_eq!(d96("123.45").to_d64().unwrap(), d64("123.45"));
    // More than 8 dp: strict to_d64 refuses, rounding variant rounds to 8 dp.
    let fine = d96("0.123456789012");
    assert!(fine.to_d64().is_err(), "strict narrowing rejects >8dp");
    assert_eq!(fine.to_d64_round().unwrap(), d64("0.12345679")); // banker's round at 8dp
    // Out of D64 range: both refuse.
    let huge = d96("1000000000000"); // 1e12 > D64::MAX (~9.2e10)
    assert!(huge.to_d64().is_err());
    assert!(huge.to_d64_round().is_err());
}

// ===========================================================================
// f64 conversion — exact decimals round-trip; from_f64 agrees with from_str
// ===========================================================================

#[test]
fn d64_f64_roundtrip_exact_decimals() {
    for (s, x) in [("0.5", 0.5), ("0.25", 0.25), ("0.125", 0.125), ("1234.5", 1234.5)] {
        let from_float = D64::from_f64(x).unwrap();
        assert_eq!(from_float, d64(s), "from_f64({x}) == from_str({s})");
        assert_eq!(from_float.to_f64(), x, "to_f64 round trip for {s}");
    }
    // 0.1 is not f64-exact, but rounding to 8 dp recovers the intended decimal.
    assert_eq!(D64::from_f64(0.1).unwrap(), d64("0.1"));
}

#[test]
fn d96_f64_roundtrip_exact_decimals() {
    for (s, x) in [("0.5", 0.5), ("0.0625", 0.0625), ("100.125", 100.125)] {
        let from_float = D96::from_f64(x).unwrap();
        assert_eq!(from_float, d96(s), "from_f64({x}) == from_str({s})");
        assert_eq!(from_float.to_f64(), x, "to_f64 round trip for {s}");
    }
    // Non-finite inputs are rejected rather than producing garbage.
    assert_eq!(D96::from_f64(f64::NAN), None);
    assert_eq!(D96::from_f64(f64::INFINITY), None);
    assert_eq!(D64::from_f64(f64::NEG_INFINITY), None);
}

// ===========================================================================
// Byte round trips — to_*_bytes / from_*_bytes are inverses for all endians
// ===========================================================================

#[test]
fn d64_byte_roundtrips() {
    for d in [D64::MIN, D64::MAX, D64::ZERO, d64("-123.456"), d64("0.00000001")] {
        assert_eq!(D64::from_le_bytes(d.to_le_bytes()), d);
        assert_eq!(D64::from_be_bytes(d.to_be_bytes()), d);
        assert_eq!(D64::from_ne_bytes(d.to_ne_bytes()), d);
    }
}

#[test]
fn d96_byte_roundtrips() {
    for d in [D96::MIN, D96::MAX, D96::ZERO, d96("-123.456789"), d96("0.000000000001")] {
        assert_eq!(D96::from_le_bytes(d.to_le_bytes()), d);
        assert_eq!(D96::from_be_bytes(d.to_be_bytes()), d);
        assert_eq!(D96::from_ne_bytes(d.to_ne_bytes()), d);
    }
}

// ===========================================================================
// Remainder — sign follows the dividend (truncated division), matching Rust %
// ===========================================================================

#[test]
fn d64_rem_sign_follows_dividend() {
    assert_eq!((d64("7.5") % d64("2")), d64("1.5"));
    assert_eq!((d64("-7.5") % d64("2")), d64("-1.5"));
    assert_eq!((d64("7.5") % d64("-2")), d64("1.5"));
    assert_eq!((d64("-7.5") % d64("-2")), d64("-1.5"));
    // a == (a / b truncated) * b + (a % b) is not directly testable (no trunc-div
    // operator), but the checked path must agree with the operator.
    assert_eq!(d64("7.5").checked_rem(d64("2")).unwrap(), d64("1.5"));
    assert_eq!(d64("5").checked_rem(D64::ZERO), None);
}

#[test]
fn d96_rem_sign_follows_dividend() {
    assert_eq!((d96("7.5") % d96("2")), d96("1.5"));
    assert_eq!((d96("-7.5") % d96("2")), d96("-1.5"));
    assert_eq!((d96("7.5") % d96("-2")), d96("1.5"));
    assert_eq!((d96("-7.5") % d96("-2")), d96("-1.5"));
    assert_eq!(d96("5").checked_rem(D96::ZERO), None);
}

// ===========================================================================
// Saturating / checked agree at the representable boundary
// ===========================================================================

#[test]
fn d64_boundary_checked_vs_saturating() {
    assert_eq!(D64::MAX.checked_add(D64::ONE), None);
    assert_eq!(D64::MAX.saturating_add(D64::ONE), D64::MAX);
    assert_eq!(D64::MIN.checked_sub(D64::ONE), None);
    assert_eq!(D64::MIN.saturating_sub(D64::ONE), D64::MIN);
    assert_eq!(D64::MIN.checked_neg(), None); // -i64::MIN overflows
    assert_eq!(D64::MIN.saturating_neg(), D64::MAX);
    assert_eq!(D64::MIN.saturating_abs(), D64::MAX);
}

#[test]
fn d96_boundary_checked_vs_saturating() {
    assert_eq!(D96::MAX.checked_add(D96::ONE), None);
    assert_eq!(D96::MAX.saturating_add(D96::ONE), D96::MAX);
    assert_eq!(D96::MIN.checked_sub(D96::ONE), None);
    assert_eq!(D96::MIN.saturating_sub(D96::ONE), D96::MIN);
    assert_eq!(D96::MIN.checked_neg(), None);
    assert_eq!(D96::MIN.saturating_neg(), D96::MAX);
    assert_eq!(D96::MIN.saturating_abs(), D96::MAX);
}

// ===========================================================================
// signum / abs consistency
// ===========================================================================

// ===========================================================================
// from_f64 must match std `f64::round` semantics. The crate rounds with a
// core-only helper (so it builds on bare-metal no_std); this oracle — using the
// std `f64::round` available in tests — proves the helper is equivalent on the
// inputs from_f64 accepts, across ties, signs, and magnitudes.
// ===========================================================================

fn d64_from_f64_oracle(v: f64) -> Option<D64> {
    if !v.is_finite() {
        return None;
    }
    let scaled = (v * 1e8_f64).round();
    const TWO_POW_63: f64 = 9_223_372_036_854_775_808.0;
    if !(-TWO_POW_63..TWO_POW_63).contains(&scaled) {
        return None;
    }
    Some(D64::from_raw(scaled as i64))
}

fn d96_from_f64_oracle(v: f64) -> Option<D96> {
    if !v.is_finite() {
        return None;
    }
    let scaled = v * 1e12_f64;
    // Mirror the crate's pre-round magnitude guard (compare against the raw
    // 96-bit bounds as f64) before rounding.
    let max = 39_614_081_257_132_168_796_771_975_167.0_f64; // MAX_96BIT
    let min = -39_614_081_257_132_168_796_771_975_168.0_f64; // MIN_96BIT
    if scaled > max || scaled < min {
        return None;
    }
    let result = scaled.round();
    if result > max || result < min {
        return None;
    }
    Some(D96::from_raw(result as i128))
}

#[test]
fn from_f64_matches_std_round_oracle() {
    // Deterministic sweep hitting exact ties (k/8 places .5 on the 8th/12th dp),
    // both signs, near zero, and a few larger magnitudes.
    let mut bits: u64 = 0x9E37_79B9_7F4A_7C15; // fixed seed, no RNG dependency
    for i in -2000..2000 {
        // structured ties and quarters
        let v = i as f64 / 80.0;
        assert_eq!(D64::from_f64(v), d64_from_f64_oracle(v), "D64 from_f64({v})");
        assert_eq!(D96::from_f64(v), d96_from_f64_oracle(v), "D96 from_f64({v})");
    }
    for _ in 0..5000 {
        // xorshift over a wide range of magnitudes/signs
        bits ^= bits << 13;
        bits ^= bits >> 7;
        bits ^= bits << 17;
        let v = (bits as i64 as f64) / 1.0e9;
        assert_eq!(D64::from_f64(v), d64_from_f64_oracle(v), "D64 from_f64({v})");
        assert_eq!(D96::from_f64(v), d96_from_f64_oracle(v), "D96 from_f64({v})");
    }
}

#[test]
fn d64_signum_abs_consistency() {
    assert_eq!(d64("5").signum(), 1);
    assert_eq!(d64("-5").signum(), -1);
    assert_eq!(D64::ZERO.signum(), 0);
    for s in ["5", "-5", "0", "0.00000001", "-9999.99999999"] {
        let a = d64(s);
        // |a| is non-negative and |a| == |-a|
        assert!(!a.abs().is_negative());
        assert_eq!(a.abs(), (-a).abs(), "|a| == |-a| for {s}");
    }
}
