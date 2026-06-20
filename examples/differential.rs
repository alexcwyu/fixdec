//! Randomized differential + performance harness:
//! `fixdec` (D64/D96) vs `rust_decimal` vs `f64` (double).
//!
//! For N random iterations it cycles through add / sub / mul / div, and:
//!   1. counts how often D64/D96 disagree with `rust_decimal` (the trusted
//!      oracle) — should be 0 for +,-,* and within 1 ulp for /;
//!   2. counts how often `f64` produces the wrong 8/12-decimal answer;
//!   3. times each backend over the identical workload.
//!
//! Run:  cargo run --release --example differential -- [N]      (default 10_000_000)

use core::str::FromStr;
use std::time::Instant;

use fixdec::{D64, D96};
use rust_decimal::{Decimal, RoundingStrategy};

/// Deterministic splitmix64 PRNG.
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
    /// Random raw with random bit-width and sign.
    #[inline]
    fn bounded(&mut self, widths: &[u32]) -> i128 {
        let w = widths[(self.next_u64() as usize) % widths.len()];
        let mut bits = self.next_u64() as u128;
        if w > 64 {
            bits |= (self.next_u64() as u128) << 64;
        }
        let mask = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
        let mag = (bits & mask) as i128;
        if self.next_u64() & 1 == 1 { -mag } else { mag }
    }
}

#[derive(Clone, Copy)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
}
const OPS: [Op; 4] = [Op::Add, Op::Sub, Op::Mul, Op::Div];

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000_000);

    println!("fixdec differential / performance harness — {n} iterations per type\n");
    run_d64(n);
    println!();
    run_d96(n);
    println!();
    f64_precision_demo();
}

// ---------------------------------------------------------------------------
// D64
// ---------------------------------------------------------------------------

fn run_d64(n: usize) {
    let widths = [8u32, 20, 33, 47, 60, 63];

    // ---- correctness vs rust_decimal ----
    let mut rng = Rng::new(0xD64);
    let (mut checked, mut mismatch, mut f64_wrong) = (0u64, 0u64, 0u64);
    for k in 0..n {
        let a = D64::from_raw(rng.bounded(&widths) as i64);
        let b = D64::from_raw(rng.bounded(&widths) as i64);
        let op = OPS[k % 4];
        let da = Decimal::from_i128_with_scale(a.to_raw() as i128, 8);
        let db = Decimal::from_i128_with_scale(b.to_raw() as i128, 8);

        let (got, want): (Option<D64>, Option<Decimal>) = match op {
            Op::Add => (a.checked_add(b), da.checked_add(db)),
            Op::Sub => (a.checked_sub(b), da.checked_sub(db)),
            Op::Mul => (
                a.checked_mul(b),
                da.checked_mul(db)
                    .map(|d| d.round_dp_with_strategy(8, RoundingStrategy::ToZero)),
            ),
            Op::Div => (
                a.checked_div(b),
                da.checked_div(db)
                    .map(|d| d.round_dp_with_strategy(8, RoundingStrategy::ToZero)),
            ),
        };

        if let (Some(g), Some(w)) = (got, want) {
            checked += 1;
            let gd = Decimal::from_i128_with_scale(g.to_raw() as i128, 8);
            let tol = Decimal::from_i128_with_scale(1, 8); // 1 ulp for div rounding
            if (gd - w).abs() > tol {
                mismatch += 1;
            }
            // f64 accuracy on the same op
            let fa = a.to_f64();
            let fb = b.to_f64();
            let f = match op {
                Op::Add => fa + fb,
                Op::Sub => fa - fb,
                Op::Mul => fa * fb,
                Op::Div => fa / fb,
            };
            if let Some(fd) = D64::from_f64(f) {
                let fdd = Decimal::from_i128_with_scale(fd.to_raw() as i128, 8);
                if (fdd - w).abs() > tol {
                    f64_wrong += 1;
                }
            }
        }
    }

    // ---- timing (identical workload per backend) ----
    let t_d64 = time_d64(n, &widths);
    let t_dec = time_decimal(n, &widths, 8);
    let t_f64 = time_f64(n, &widths, D64::SCALE as f64);

    println!("D64  ({n} ops, +,-,*,/ cycled):");
    println!("  vs rust_decimal : {mismatch} mismatches / {checked} comparable ops");
    println!("  f64 wrong (>1ulp): {f64_wrong} / {checked}");
    println!(
        "  time/op  D64={:.2}ns  rust_decimal={:.2}ns  f64={:.2}ns  (D64 is {:.1}x faster than rust_decimal)",
        t_d64, t_dec, t_f64, t_dec / t_d64
    );
}

fn time_d64(n: usize, widths: &[u32]) -> f64 {
    let mut rng = Rng::new(0xD64);
    let mut acc = 0i64;
    let start = Instant::now();
    for k in 0..n {
        let a = D64::from_raw(rng.bounded(widths) as i64);
        let b = D64::from_raw(rng.bounded(widths) as i64);
        let r = match OPS[k % 4] {
            Op::Add => a.checked_add(b),
            Op::Sub => a.checked_sub(b),
            Op::Mul => a.checked_mul(b),
            Op::Div => a.checked_div(b),
        };
        acc = acc.wrapping_add(r.map(D64::to_raw).unwrap_or(0));
    }
    std::hint::black_box(acc);
    start.elapsed().as_secs_f64() * 1e9 / n as f64
}

// ---------------------------------------------------------------------------
// D96
// ---------------------------------------------------------------------------

fn run_d96(n: usize) {
    let widths = [8u32, 24, 40, 64, 80, 94];
    let max = D96::MAX.to_raw();

    let mut rng = Rng::new(0xD96);
    let (mut checked, mut mismatch) = (0u64, 0u64);
    for k in 0..n {
        let a = D96::from_raw(rng.bounded(&widths).clamp(-max, max));
        let b = D96::from_raw(rng.bounded(&widths).clamp(-max, max));
        let op = OPS[k % 4];
        let da = Decimal::from_i128_with_scale(a.to_raw(), 12);
        let db = Decimal::from_i128_with_scale(b.to_raw(), 12);

        let (got, want): (Option<D96>, Option<Decimal>) = match op {
            Op::Add => (a.checked_add(b), da.checked_add(db)),
            Op::Sub => (a.checked_sub(b), da.checked_sub(db)),
            Op::Mul => (
                a.checked_mul(b),
                da.checked_mul(db)
                    .map(|d| d.round_dp_with_strategy(12, RoundingStrategy::ToZero)),
            ),
            Op::Div => (
                a.checked_div(b),
                da.checked_div(db)
                    .map(|d| d.round_dp_with_strategy(12, RoundingStrategy::ToZero)),
            ),
        };

        if let (Some(g), Some(w)) = (got, want) {
            checked += 1;
            let gd = Decimal::from_i128_with_scale(g.to_raw(), 12);
            let tol = Decimal::from_i128_with_scale(1, 12);
            if (gd - w).abs() > tol {
                mismatch += 1;
            }
        }
    }

    let t_d96 = time_d96(n, &widths, max);
    let t_dec = time_decimal(n, &widths, 12);

    println!("D96  ({n} ops, +,-,*,/ cycled):");
    println!("  vs rust_decimal : {mismatch} mismatches / {checked} comparable ops");
    println!(
        "  time/op  D96={:.2}ns  rust_decimal={:.2}ns  (D96 is {:.1}x faster than rust_decimal)",
        t_d96, t_dec, t_dec / t_d96
    );
}

fn time_d96(n: usize, widths: &[u32], max: i128) -> f64 {
    let mut rng = Rng::new(0xD96);
    let mut acc = 0i128;
    let start = Instant::now();
    for k in 0..n {
        let a = D96::from_raw(rng.bounded(widths).clamp(-max, max));
        let b = D96::from_raw(rng.bounded(widths).clamp(-max, max));
        let r = match OPS[k % 4] {
            Op::Add => a.checked_add(b),
            Op::Sub => a.checked_sub(b),
            Op::Mul => a.checked_mul(b),
            Op::Div => a.checked_div(b),
        };
        acc = acc.wrapping_add(r.map(D96::to_raw).unwrap_or(0));
    }
    std::hint::black_box(acc);
    start.elapsed().as_secs_f64() * 1e9 / n as f64
}

// ---------------------------------------------------------------------------
// shared timing for rust_decimal and f64
// ---------------------------------------------------------------------------

fn time_decimal(n: usize, widths: &[u32], scale: u32) -> f64 {
    let mut rng = Rng::new(if scale == 8 { 0xD64 } else { 0xD96 });
    let max = D96::MAX.to_raw();
    let mut acc = Decimal::ZERO;
    let start = Instant::now();
    for k in 0..n {
        let (ar, br) = if scale == 8 {
            (rng.bounded(widths) as i64 as i128, rng.bounded(widths) as i64 as i128)
        } else {
            (rng.bounded(widths).clamp(-max, max), rng.bounded(widths).clamp(-max, max))
        };
        let a = Decimal::from_i128_with_scale(ar, scale);
        let b = Decimal::from_i128_with_scale(br, scale);
        let r = match OPS[k % 4] {
            Op::Add => a.checked_add(b),
            Op::Sub => a.checked_sub(b),
            Op::Mul => a.checked_mul(b),
            Op::Div => a.checked_div(b),
        };
        if let Some(r) = r {
            acc = acc.checked_add(r).unwrap_or(Decimal::ZERO).fract();
        }
    }
    std::hint::black_box(acc);
    start.elapsed().as_secs_f64() * 1e9 / n as f64
}

fn time_f64(n: usize, widths: &[u32], scale: f64) -> f64 {
    let mut rng = Rng::new(0xD64);
    let mut acc = 0f64;
    let start = Instant::now();
    for k in 0..n {
        let a = (rng.bounded(widths) as i64) as f64 / scale;
        let b = (rng.bounded(widths) as i64) as f64 / scale;
        let r = match OPS[k % 4] {
            Op::Add => a + b,
            Op::Sub => a - b,
            Op::Mul => a * b,
            Op::Div => {
                if b == 0.0 {
                    0.0
                } else {
                    a / b
                }
            }
        };
        acc += r.fract();
    }
    std::hint::black_box(acc);
    start.elapsed().as_secs_f64() * 1e9 / n as f64
}

// ---------------------------------------------------------------------------
// f64 precision demo: accumulation drift
// ---------------------------------------------------------------------------

fn f64_precision_demo() {
    println!("f64 vs D64 accumulation (add 0.1 one million times):");
    let mut f = 0.0f64;
    let mut d = D64::ZERO;
    let tenth_f = 0.1f64;
    let tenth_d = D64::from_str("0.1").unwrap();
    for _ in 0..1_000_000 {
        f += tenth_f;
        d += tenth_d;
    }
    println!("  f64 result : {f:.8}   (expected 100000.00000000)");
    println!("  D64 result : {d}   (exact)");
}
