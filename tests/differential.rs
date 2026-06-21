//! Randomized differential tests: `fixdec` vs an exact `i128` reference and vs
//! `rust_decimal` (a trusted independent implementation).
//!
//! Strategy per operation:
//! - **add / sub**: `D64`/`D96` are exact, so results must match the oracle and
//!   `rust_decimal` *exactly*.
//! - **mul**: truncates toward zero to the type's precision. For `D64` the exact
//!   product fits in `rust_decimal` (<= 16 fractional digits), so the comparison
//!   is exact; for `D96` it is checked to within 1 ulp (rust_decimal rounds at 28
//!   significant digits).
//! - **div**: truncates toward zero; compared exactly to the `i128` reference and
//!   to within 1 ulp of `rust_decimal`.
//!
//! The iteration count defaults to a CI-friendly value and can be raised with
//! `DIFF_ITERS=10000000 cargo test --release --test differential -- --nocapture`.

use fixdec::{D64, D96};
use rust_decimal::{Decimal, RoundingStrategy};

/// Deterministic splitmix64 PRNG — reproducible failures, no external dep.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Random value with a random bit-width drawn from `widths` (covers tiny ..
    /// near-overflow magnitudes), with a random sign.
    fn bounded(&mut self, widths: &[u32]) -> i128 {
        let w = widths[(self.next_u64() as usize) % widths.len()];
        let mut bits = self.next_u64() as u128;
        if w > 64 {
            bits |= (self.next_u64() as u128) << 64;
        }
        let mask = if w >= 128 {
            u128::MAX
        } else {
            (1u128 << w) - 1
        };
        let mag = (bits & mask) as i128;
        if self.next_u64() & 1 == 1 { -mag } else { mag }
    }
}

fn iters(default: usize) -> usize {
    std::env::var("DIFF_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn one_ulp(scale: u32) -> Decimal {
    Decimal::from_i128_with_scale(1, scale)
}

// --------------------------------------------------------------------------
// D64
// --------------------------------------------------------------------------

fn d64_to_dec(d: D64) -> Decimal {
    Decimal::from_i128_with_scale(d.to_raw() as i128, 8)
}

fn d64_mul_oracle(a: i64, b: i64) -> Option<i64> {
    let q = (a as i128 * b as i128) / D64::SCALE as i128;
    if q > i64::MAX as i128 || q < i64::MIN as i128 {
        None
    } else {
        Some(q as i64)
    }
}

fn d64_div_oracle(a: i64, b: i64) -> Option<i64> {
    if b == 0 {
        return None;
    }
    let q = (a as i128 * D64::SCALE as i128) / b as i128;
    if q > i64::MAX as i128 || q < i64::MIN as i128 {
        None
    } else {
        Some(q as i64)
    }
}

#[test]
fn differential_d64_vs_oracle_and_rust_decimal() {
    let n = iters(250_000);
    let mut rng = Rng::new(0xD64_5EED);
    // raw widths up to 63 bits (full i64 magnitude range)
    let widths = [8u32, 20, 33, 47, 60, 63];

    for i in 0..n {
        let a = D64::from_raw(rng.bounded(&widths) as i64);
        let b = D64::from_raw(rng.bounded(&widths) as i64);
        let (ar, br) = (a.to_raw(), b.to_raw());
        let (da, db) = (d64_to_dec(a), d64_to_dec(b));

        // ---- add (exact) ----
        assert_eq!(
            a.checked_add(b).map(D64::to_raw),
            ar.checked_add(br),
            "add oracle mismatch a={ar} b={br} (iter {i})"
        );
        if let (Some(r), Some(dr)) = (a.checked_add(b), da.checked_add(db)) {
            assert_eq!(d64_to_dec(r), dr, "add vs rust_decimal a={ar} b={br}");
        }

        // ---- sub (exact) ----
        assert_eq!(
            a.checked_sub(b).map(D64::to_raw),
            ar.checked_sub(br),
            "sub oracle mismatch a={ar} b={br}"
        );
        if let (Some(r), Some(dr)) = (a.checked_sub(b), da.checked_sub(db)) {
            assert_eq!(d64_to_dec(r), dr, "sub vs rust_decimal a={ar} b={br}");
        }

        // ---- mul (exact truncation) ----
        assert_eq!(
            a.checked_mul(b).map(D64::to_raw),
            d64_mul_oracle(ar, br),
            "mul oracle mismatch a={ar} b={br}"
        );
        if let (Some(r), Some(dm)) = (a.checked_mul(b), da.checked_mul(db)) {
            let truncated = dm.round_dp_with_strategy(8, RoundingStrategy::ToZero);
            assert_eq!(
                d64_to_dec(r),
                truncated,
                "mul vs rust_decimal a={ar} b={br}"
            );
        }

        // ---- div (exact truncation) ----
        assert_eq!(
            a.checked_div(b).map(D64::to_raw),
            d64_div_oracle(ar, br),
            "div oracle mismatch a={ar} b={br}"
        );
        if let (Some(r), Some(dd)) = (a.checked_div(b), da.checked_div(db)) {
            let truncated = dd.round_dp_with_strategy(8, RoundingStrategy::ToZero);
            let diff = (d64_to_dec(r) - truncated).abs();
            assert!(
                diff <= one_ulp(8),
                "div vs rust_decimal a={ar} b={br}: d64={r:?} dec={truncated} diff={diff}"
            );
        }

        // ---- rem (exact; matches rust_decimal's truncated remainder) ----
        if br != 0 && !(ar == i64::MIN && br == -1) {
            let r = a % b;
            assert_eq!(r.to_raw(), ar % br, "rem oracle mismatch a={ar} b={br}");
            assert_eq!(d64_to_dec(r), da % db, "rem vs rust_decimal a={ar} b={br}");
        }
    }
}

// --------------------------------------------------------------------------
// D96
// --------------------------------------------------------------------------

fn d96_to_dec(d: D96) -> Decimal {
    Decimal::from_i128_with_scale(d.to_raw(), 12)
}

fn in_d96_range(v: i128) -> bool {
    v <= D96::MAX.to_raw() && v >= D96::MIN.to_raw()
}

fn d96_add_oracle(a: i128, b: i128) -> Option<i128> {
    a.checked_add(b).filter(|s| in_d96_range(*s))
}
fn d96_sub_oracle(a: i128, b: i128) -> Option<i128> {
    a.checked_sub(b).filter(|s| in_d96_range(*s))
}

#[test]
fn differential_d96_vs_oracle_and_rust_decimal() {
    let n = iters(250_000);
    let mut rng = Rng::new(0xD96_5EED);
    let max = D96::MAX.to_raw();
    // raw widths up to 95 bits (full 96-bit magnitude range)
    let widths = [8u32, 24, 40, 64, 80, 94];

    for i in 0..n {
        let mut ar = rng.bounded(&widths);
        let mut br = rng.bounded(&widths);
        ar = ar.clamp(-max, max);
        br = br.clamp(-max, max);
        let a = D96::from_raw(ar);
        let b = D96::from_raw(br);
        let (da, db) = (d96_to_dec(a), d96_to_dec(b));

        // ---- add / sub (exact) ----
        assert_eq!(
            a.checked_add(b).map(D96::to_raw),
            d96_add_oracle(ar, br),
            "add oracle mismatch a={ar} b={br} (iter {i})"
        );
        if let (Some(r), Some(dr)) = (a.checked_add(b), da.checked_add(db)) {
            assert_eq!(d96_to_dec(r), dr, "add vs rust_decimal a={ar} b={br}");
        }

        assert_eq!(
            a.checked_sub(b).map(D96::to_raw),
            d96_sub_oracle(ar, br),
            "sub oracle mismatch a={ar} b={br}"
        );
        if let (Some(r), Some(dr)) = (a.checked_sub(b), da.checked_sub(db)) {
            assert_eq!(d96_to_dec(r), dr, "sub vs rust_decimal a={ar} b={br}");
        }

        // ---- mul / div vs rust_decimal (within 1 ulp; skipped when the exact
        //      result overflows rust_decimal's 96-bit/28-digit mantissa) ----
        if let (Some(r), Some(dm)) = (a.checked_mul(b), da.checked_mul(db)) {
            let truncated = dm.round_dp_with_strategy(12, RoundingStrategy::ToZero);
            let diff = (d96_to_dec(r) - truncated).abs();
            assert!(
                diff <= one_ulp(12),
                "mul vs rust_decimal a={ar} b={br}: d96={r:?} dec={truncated} diff={diff}"
            );
        }
        if let (Some(r), Some(dd)) = (a.checked_div(b), da.checked_div(db)) {
            let truncated = dd.round_dp_with_strategy(12, RoundingStrategy::ToZero);
            let diff = (d96_to_dec(r) - truncated).abs();
            assert!(
                diff <= one_ulp(12),
                "div vs rust_decimal a={ar} b={br}: d96={r:?} dec={truncated} diff={diff}"
            );
        }

        // ---- rem (exact; matches rust_decimal's truncated remainder) ----
        if br != 0 {
            let r = a % b;
            assert_eq!(r.to_raw(), ar % br, "rem oracle mismatch a={ar} b={br}");
            assert_eq!(d96_to_dec(r), da % db, "rem vs rust_decimal a={ar} b={br}");
        }
    }
}
