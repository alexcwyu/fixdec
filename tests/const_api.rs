//! Locks the `const fn` parity between D64 and D96 arithmetic, and covers a
//! D96 `mul_add` gap.
//!
//! An API symmetry audit found that the entire D96 mul/div family
//! (`checked_mul`/`checked_div`/`try_*`/`saturating_*`/`wrapping_*`/
//! `overflowing_mul`/`mul_add`/`recip`/`powi`) had silently lost `const` while
//! the D64 twins kept it — the exact d64/d96 divergence the crate fights. These
//! `const` bindings force compile-time evaluation, so the test fails to *compile*
//! if `const` is ever dropped again on either type.

use fixdec::{D64, D96};

#[test]
fn d64_mul_div_family_is_const() {
    const A: D64 = D64::from_i32(6);
    const B: D64 = D64::from_i32(4);
    const MUL: Option<D64> = A.checked_mul(B);
    const DIV: Option<D64> = A.checked_div(B);
    const MA: Option<D64> = A.mul_add(B, D64::ONE);
    const RECIP: Option<D64> = A.recip();
    const POW: Option<D64> = A.powi(2);
    const SAT: D64 = A.saturating_mul(B);
    const WRAP: D64 = A.wrapping_mul(B);
    const OVF: (D64, bool) = A.overflowing_mul(B);
    const TRY_MUL: Result<D64, fixdec::DecimalError> = A.try_mul(B);
    const TRY_DIV: Result<D64, fixdec::DecimalError> = A.try_div(B);

    assert_eq!(MUL, Some(D64::from_i32(24)));
    assert_eq!(DIV, Some(D64::from_str_exact("1.5").unwrap()));
    assert_eq!(MA, Some(D64::from_i32(25))); // 6*4 + 1
    assert_eq!(POW, Some(D64::from_i32(36)));
    assert_eq!(SAT, D64::from_i32(24));
    assert_eq!(WRAP, D64::from_i32(24));
    assert_eq!(OVF, (D64::from_i32(24), false));
    assert_eq!(TRY_MUL, Ok(D64::from_i32(24)));
    assert_eq!(TRY_DIV, Ok(D64::from_str_exact("1.5").unwrap()));
    assert!(RECIP.is_some());
}

#[test]
fn d96_mul_div_family_is_const() {
    const A: D96 = D96::from_i32(6);
    const B: D96 = D96::from_i32(4);
    const MUL: Option<D96> = A.checked_mul(B);
    const DIV: Option<D96> = A.checked_div(B);
    const MA: Option<D96> = A.mul_add(B, D96::ONE);
    const RECIP: Option<D96> = A.recip();
    const TRY_RECIP: Result<D96, fixdec::DecimalError> = A.try_recip();
    const POW: Option<D96> = A.powi(2);
    const TRY_POW: Result<D96, fixdec::DecimalError> = A.try_powi(2);
    const SAT: D96 = A.saturating_mul(B);
    const WRAP: D96 = A.wrapping_mul(B);
    const OVF: (D96, bool) = A.overflowing_mul(B);
    const SAT_DIV: D96 = A.saturating_div(B);
    const WRAP_DIV: D96 = A.wrapping_div(B);
    const TRY_MUL: Result<D96, fixdec::DecimalError> = A.try_mul(B);
    const TRY_DIV: Result<D96, fixdec::DecimalError> = A.try_div(B);

    assert_eq!(MUL, Some(D96::from_i32(24)));
    assert_eq!(DIV, Some(D96::from_str_exact("1.5").unwrap()));
    assert_eq!(MA, Some(D96::from_i32(25)));
    assert_eq!(POW, Some(D96::from_i32(36)));
    assert_eq!(TRY_POW, Ok(D96::from_i32(36)));
    assert_eq!(SAT, D96::from_i32(24));
    assert_eq!(WRAP, D96::from_i32(24));
    assert_eq!(OVF, (D96::from_i32(24), false));
    assert_eq!(SAT_DIV, D96::from_str_exact("1.5").unwrap());
    assert_eq!(WRAP_DIV, D96::from_str_exact("1.5").unwrap());
    assert_eq!(TRY_MUL, Ok(D96::from_i32(24)));
    assert_eq!(TRY_DIV, Ok(D96::from_str_exact("1.5").unwrap()));
    assert!(RECIP.is_some());
    assert!(TRY_RECIP.is_ok());
}

/// D96 `mul_add` had no inline precision test (audit finding F4). It computes a
/// single-rounding `self*mul + add` over the 192-bit path; verify it matches
/// `checked_mul` then `checked_add` across small and wide-path operands.
#[test]
fn d96_mul_add_matches_mul_then_add() {
    for s in [
        "1.5",
        "2.5",
        "0.000000000001",
        "123456.789",
        "150000000.123456789", // wide path: raw ~1.5e20, square ~2.25e16 (in range)
    ] {
        let x = D96::from_str_exact(s).unwrap();
        if let Some(sq) = x.checked_mul(x) {
            let expected = sq.checked_add(D96::ONE).unwrap();
            assert_eq!(x.mul_add(x, D96::ONE), Some(expected), "mul_add mismatch for {s}");
        }
    }
    // Exact small case: 1.5 * 2.5 + 0.25 == 4.0
    let r = D96::from_str_exact("1.5")
        .unwrap()
        .mul_add(
            D96::from_str_exact("2.5").unwrap(),
            D96::from_str_exact("0.25").unwrap(),
        )
        .unwrap();
    assert_eq!(r, D96::from_str_exact("4").unwrap());
}
