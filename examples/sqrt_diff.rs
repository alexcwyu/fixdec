//! Randomized differential + edge-case regression for `D64`/`D96` `sqrt`.
//!
//! For N random non-negative inputs (default 10_000_000) of every bit-width —
//! plus an explicit edge-case set — it checks each `sqrt` against three
//! references:
//!
//!   1. **Exact integer oracle** (ground truth): `y_raw^2 <= raw*SCALE <
//!      (y_raw+1)^2`, computed in wide integer math. ANY failure here is a real
//!      bug. Expected count: 0.
//!   2. **rust_decimal** (`MathematicalOps::sqrt`, ~28 significant digits): the
//!      floored result must sit in `[y, y + ULP)`. Reported as max deviation
//!      (should be < 1 ULP) and a violation count (expected 0).
//!   3. **f64** hardware `sqrt`: reported as max absolute / relative deviation
//!      (f64 loses precision converting large inputs, so this is a sanity band,
//!      not a hard bound).
//!
//! It then times `D64`/`D96` vs `rust_decimal` vs `f64` over the identical
//! workload (a coarse throughput cross-check; criterion benches are the
//! authoritative perf numbers).
//!
//! Run:  cargo run --release --example sqrt_diff -- [N]        (default 1e7)

use fixdec::{D64, D96};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, MathematicalOps};
use std::hint::black_box;
use std::time::{Duration, Instant};

const D64_SCALE: i128 = 100_000_000; // 1e8
const D96_SCALE: u128 = 1_000_000_000_000; // 1e12
const D96_MAX_RAW: i128 = 39_614_081_257_132_168_796_771_975_167; // 2^95 - 1

/// Deterministic splitmix64 PRNG (same as the main differential harness).
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed)
    }
    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Non-negative i64 raw of a random bit-width (covers tiny .. i64::MAX).
    #[inline]
    fn d64_raw(&mut self) -> i64 {
        const W: [u32; 8] = [1, 4, 12, 24, 36, 48, 60, 63];
        let w = W[(self.next_u64() % W.len() as u64) as usize];
        let mask = (1u64 << w) - 1;
        ((self.next_u64() & mask) & (i64::MAX as u64)) as i64
    }
    /// Non-negative i128 raw of a random bit-width, clamped to D96 max. Widths
    /// span the u128 fast path (`raw*1e12 < 2^128`, i.e. < ~2^88) and the wide
    /// binary-search path (>= ~2^88), incl. values right at MAX.
    #[inline]
    fn d96_raw(&mut self) -> i128 {
        const W: [u32; 11] = [1, 4, 12, 24, 40, 60, 80, 87, 88, 90, 95];
        let w = W[(self.next_u64() % W.len() as u64) as usize];
        let mut bits = self.next_u64() as u128;
        if w > 64 {
            bits |= (self.next_u64() as u128) << 64;
        }
        let mask = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
        ((bits & mask) as i128).min(D96_MAX_RAW)
    }
}

// --- exact wide-integer oracle (operands < 2^96) ---

/// `a * b` as a 192-bit `(low, high)` for `a, b < 2^96`.
fn mul192(a: u128, b: u128) -> (u128, u128) {
    let a_lo = a as u64 as u128;
    let a_hi = a >> 64;
    let b_lo = b as u64 as u128;
    let b_hi = b >> 64;
    let p0 = a_lo * b_lo;
    let mid = a_lo * b_hi + a_hi * b_lo;
    let (low, carry) = p0.overflowing_add(mid << 64);
    let high = a_hi * b_hi + (mid >> 64) + carry as u128;
    (low, high)
}

fn le192(a: (u128, u128), b: (u128, u128)) -> bool {
    if a.1 != b.1 { a.1 < b.1 } else { a.0 <= b.0 }
}

#[derive(Default)]
struct Stats {
    n: u64,
    oracle_fail: u64,
    rd_violation: u64,    // floored rust_decimal sqrt left [y, y+ULP)
    rd_max_ulp: f64,      // worst (rd - y) in ULP units
    f64_max_abs: f64,
    f64_max_rel: f64,
    neg_fail: u64,        // sqrt(neg) was not None, or try_sqrt(neg) not Err
    first_fail: Option<(String, i128)>,
}

impl Stats {
    fn record_fail(&mut self, kind: &str, raw: i128) {
        if self.first_fail.is_none() {
            self.first_fail = Some((kind.to_string(), raw));
        }
    }
}

fn check_d64(raw: i64, s: &mut Stats) {
    s.n += 1;
    let x = D64::from_raw(raw);
    let y = x.sqrt().expect("non-negative has a root");
    let yr = y.to_raw() as i128;
    // (1) exact oracle in i128 (raw*1e8 <= 9.22e26, (yr+1)^2 <= ~9.2e26).
    let rad = raw as i128 * D64_SCALE;
    if !(yr >= 0 && yr * yr <= rad && rad < (yr + 1) * (yr + 1)) {
        s.oracle_fail += 1;
        s.record_fail("d64_oracle", raw as i128);
    }
    // (2) rust_decimal
    let rd = Decimal::new(raw, 8).sqrt().unwrap();
    let yd = Decimal::new(y.to_raw(), 8);
    let ulp = Decimal::new(1, 8);
    let diff = rd - yd;
    if diff.is_sign_negative() || diff >= ulp {
        s.rd_violation += 1;
        s.record_fail("d64_rd", raw as i128);
    }
    let diff_ulp = diff.to_f64().unwrap_or(f64::NAN) * 1e8;
    if diff_ulp.is_finite() && diff_ulp > s.rd_max_ulp {
        s.rd_max_ulp = diff_ulp;
    }
    // (3) f64
    let f = x.to_f64().sqrt();
    let yf = y.to_f64();
    let abs = (f - yf).abs();
    if abs > s.f64_max_abs {
        s.f64_max_abs = abs;
    }
    if f > 0.0 {
        let rel = abs / f;
        if rel > s.f64_max_rel {
            s.f64_max_rel = rel;
        }
    }
}

fn check_d96(raw: i128, s: &mut Stats) {
    s.n += 1;
    let x = D96::from_raw(raw);
    let y = x.sqrt().expect("non-negative has a root");
    let yr = y.to_raw() as u128;
    // (1) exact oracle via 192-bit math (raw*1e12 < 2^135, (yr+1)^2 < 2^137).
    let rad = mul192(raw as u128, D96_SCALE);
    let lower_ok = le192(mul192(yr, yr), rad); // y^2 <= R
    let upper_ok = !le192(mul192(yr + 1, yr + 1), rad); // R < (y+1)^2
    if !lower_ok || !upper_ok {
        s.oracle_fail += 1;
        s.record_fail("d96_oracle", raw);
    }
    // (2) rust_decimal (Decimal holds every D96 value exactly: 96-bit mantissa).
    let rd = Decimal::from_i128_with_scale(raw, 12).sqrt().unwrap();
    let yd = Decimal::from_i128_with_scale(y.to_raw(), 12);
    let ulp = Decimal::from_i128_with_scale(1, 12);
    let diff = rd - yd;
    if diff.is_sign_negative() || diff >= ulp {
        s.rd_violation += 1;
        s.record_fail("d96_rd", raw);
    }
    let diff_ulp = diff.to_f64().unwrap_or(f64::NAN) * 1e12;
    if diff_ulp.is_finite() && diff_ulp > s.rd_max_ulp {
        s.rd_max_ulp = diff_ulp;
    }
    // (3) f64
    let f = x.to_f64().sqrt();
    let yf = y.to_f64();
    let abs = (f - yf).abs();
    if abs > s.f64_max_abs {
        s.f64_max_abs = abs;
    }
    if f > 0.0 {
        let rel = abs / f;
        if rel > s.f64_max_rel {
            s.f64_max_rel = rel;
        }
    }
}

/// Verifies negatives have no root, on both types.
fn check_negatives(s64: &mut Stats, s96: &mut Stats) {
    use fixdec::DecimalError;
    for &raw in &[-1i64, -100_000_000, i64::MIN, i64::MIN + 1, -2] {
        let x = D64::from_raw(raw);
        if x.sqrt().is_some() || x.try_sqrt() != Err(DecimalError::NegativeValue) {
            s64.neg_fail += 1;
        }
    }
    for &raw in &[-1i128, -1_000_000_000_000, D96::MIN.to_raw(), D96::MIN.to_raw() + 1, -2] {
        let x = D96::from_raw(raw);
        if x.sqrt().is_some() || x.try_sqrt() != Err(DecimalError::NegativeValue) {
            s96.neg_fail += 1;
        }
    }
}

fn edge_raws_d64() -> Vec<i64> {
    let s = D64_SCALE as i64;
    let mut v = vec![0, 1, 2, 3, s, 2 * s, 4 * s, 9 * s, 16 * s, i64::MAX, i64::MAX - 1];
    // perfect squares of integers k (raw = (k*s) so value=k, sqrt=sqrt(k))
    for k in 1..=20i64 {
        v.push(k * k * s); // value k^2 -> sqrt k exactly
    }
    v
}

fn edge_raws_d96() -> Vec<i128> {
    let s = D96_SCALE as i128;
    // fast/wide seam: high==0 iff raw*1e12 <= u128::MAX, i.e. raw <= this `b`.
    // raw=b uses the u128 fast path; raw=b+1 enters the wide binary search.
    let b = (u128::MAX / D96_SCALE) as i128;
    let mut v = vec![
        0, 1, 2, 3, s, 2 * s, 4 * s, 9 * s, 16 * s,
        D96_MAX_RAW, D96_MAX_RAW - 1,
        b - 2, b - 1, b, b + 1, b + 2,                  // around the fast/wide seam
        10_000_000_000_000_000 * s,                     // value 1e16 -> sqrt 1e8 (wide, exact)
        22_500_000_000_000_000 * s,                     // value 2.25e16 -> sqrt 1.5e8 (wide)
    ];
    for k in 1..=20i128 {
        v.push(k * k * s); // value k^2 -> sqrt k exactly
    }
    // large perfect squares in range: value = m^2 for m up to ~1.99e8.
    for m in [100_000_000i128, 150_000_000, 199_000_000] {
        v.push(m * m * s); // raw may exceed MAX for the largest; guard
    }
    v.retain(|&r| (0..=D96_MAX_RAW).contains(&r));
    v
}

fn time_pass(n: u64, seed: u64, mut body: impl FnMut(&mut Rng)) -> Duration {
    let mut rng = Rng::new(seed);
    let t = Instant::now();
    for _ in 0..n {
        body(&mut rng);
    }
    t.elapsed()
}

fn ns_per(d: Duration, n: u64) -> f64 {
    d.as_secs_f64() * 1e9 / n as f64
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000_000);

    println!("sqrt differential regression: N = {n} random + edge cases\n");

    // ---- correctness ----
    let mut s64 = Stats::default();
    let mut s96 = Stats::default();
    let mut rng = Rng::new(0x5141_4001_0D96_0D64);
    for _ in 0..n {
        check_d64(rng.d64_raw(), &mut s64);
        check_d96(rng.d96_raw(), &mut s96);
    }
    for r in edge_raws_d64() {
        check_d64(r, &mut s64);
    }
    for r in edge_raws_d96() {
        check_d96(r, &mut s96);
    }
    check_negatives(&mut s64, &mut s96);

    for (name, s) in [("D64", &s64), ("D96", &s96)] {
        println!("== {name} ==  ({} sqrt checked)", s.n);
        println!("  exact-oracle failures : {}", s.oracle_fail);
        println!("  rust_decimal violations: {}  (floored sqrt outside [y, y+ULP))", s.rd_violation);
        // The floor of a non-perfect-square root approaches but never reaches
        // 1 ULP below rust_decimal's ~28-digit root, so the exact (Decimal)
        // violation count is the verdict; the f64 max is shown for context and
        // tagged `<`/`>=` from that count to avoid a rounding false-alarm.
        println!(
            "  rust_decimal max dev   : {} 1 ULP  (~{:.4} ULP observed; {} hard violations)",
            if s.rd_violation == 0 { "<" } else { ">=" },
            s.rd_max_ulp,
            s.rd_violation
        );
        println!("  f64 max abs deviation  : {:.3e}", s.f64_max_abs);
        println!("  f64 max rel deviation  : {:.3e}", s.f64_max_rel);
        println!("  negative-input failures: {}", s.neg_fail);
        if let Some((k, raw)) = &s.first_fail {
            println!("  FIRST FAILURE: {k} at raw={raw}");
        }
        println!();
    }

    let ok = s64.oracle_fail == 0
        && s96.oracle_fail == 0
        && s64.rd_violation == 0
        && s96.rd_violation == 0
        && s64.neg_fail == 0
        && s96.neg_fail == 0;
    println!("RESULT: {}\n", if ok { "PASS — all backends agree within bounds" } else { "FAIL — see failures above" });

    // ---- throughput (coarse; criterion benches are authoritative) ----
    println!("throughput over {n} random inputs (lower ns/op = faster):");
    let base96 = time_pass(n, 7, |r| {
        black_box(r.d96_raw());
    });
    let ours96 = time_pass(n, 7, |r| {
        black_box(D96::from_raw(r.d96_raw()).sqrt());
    });
    let rd96 = time_pass(n, 7, |r| {
        black_box(Decimal::from_i128_with_scale(r.d96_raw(), 12).sqrt());
    });
    let f96 = time_pass(n, 7, |r| {
        let raw = r.d96_raw();
        black_box((raw as f64 / 1e12).sqrt());
    });
    let base64 = time_pass(n, 9, |r| {
        black_box(r.d64_raw());
    });
    let ours64 = time_pass(n, 9, |r| {
        black_box(D64::from_raw(r.d64_raw()).sqrt());
    });
    let rd64 = time_pass(n, 9, |r| {
        black_box(Decimal::new(r.d64_raw(), 8).sqrt());
    });
    let f64p = time_pass(n, 9, |r| {
        let raw = r.d64_raw();
        black_box((raw as f64 / 1e8).sqrt());
    });
    let adj = |d: Duration, base: Duration| ns_per(d.saturating_sub(base), n);
    println!("  D64::sqrt        {:7.2} ns   rust_decimal {:7.2} ns   f64 {:7.2} ns", adj(ours64, base64), adj(rd64, base64), adj(f64p, base64));
    println!("  D96::sqrt(mixed) {:7.2} ns   rust_decimal {:7.2} ns   f64 {:7.2} ns", adj(ours96, base96), adj(rd96, base96), adj(f96, base96));
    println!("  (RNG-only baseline subtracted: D64 {:.2} ns, D96 {:.2} ns/iter)", ns_per(base64, n), ns_per(base96, n));

    std::process::exit(if ok { 0 } else { 1 });
}
