//! Times 1,000,000 operations of add / sub / mul / div for D64 and D96.
//! Operands are pre-generated (deterministic) so timing isolates the arithmetic.
//! Run the same binary against the pre-fix and post-fix source to compare.
//!
//!   cargo run --release --example perf_compare

use std::hint::black_box;
use std::time::Instant;

use fixdec::{D64, D96};

const N: usize = 1_000_000;
const ROUNDS: usize = 7; // report the best (min) of several runs

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
}

fn best_ns<F: FnMut() -> i128>(mut f: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..ROUNDS {
        let start = Instant::now();
        let acc = f();
        black_box(acc);
        let ns = start.elapsed().as_secs_f64() * 1e9 / N as f64;
        if ns < best {
            best = ns;
        }
    }
    best
}

fn main() {
    println!("1,000,000 ops per operation, best of {ROUNDS} runs (ns/op)\n");

    // ---- D64 operands: values in [1, ~1e5) so +,-,*,/ stay in range ----
    let mut rng = Rng::new(0x6464);
    let d64: Vec<(D64, D64)> = (0..N)
        .map(|_| {
            let mk = |r: &mut Rng| {
                let int = (r.next_u64() % 100_000) as i64 + 1;
                let frac = (r.next_u64() % 100_000_000) as i64;
                let raw = int * D64::SCALE + frac;
                D64::from_raw(if r.next_u64() & 1 == 1 { -raw } else { raw })
            };
            (mk(&mut rng), mk(&mut rng))
        })
        .collect();

    let add = best_ns(|| d64.iter().fold(0i128, |s, (a, b)| s.wrapping_add(a.checked_add(*b).map_or(0, |v| v.to_raw() as i128))));
    let sub = best_ns(|| d64.iter().fold(0i128, |s, (a, b)| s.wrapping_add(a.checked_sub(*b).map_or(0, |v| v.to_raw() as i128))));
    let mul = best_ns(|| d64.iter().fold(0i128, |s, (a, b)| s.wrapping_add(a.checked_mul(*b).map_or(0, |v| v.to_raw() as i128))));
    let div = best_ns(|| d64.iter().fold(0i128, |s, (a, b)| s.wrapping_add(a.checked_div(*b).map_or(0, |v| v.to_raw() as i128))));
    println!("D64   add {add:5.2}   sub {sub:5.2}   mul {mul:5.2}   div {div:5.2}");

    // ---- D96 operands: values in [1, ~1e6) with SMALL divisors (fast div path) ----
    let mut rng = Rng::new(0x9696);
    let d96: Vec<(D96, D96)> = (0..N)
        .map(|_| {
            let mk = |r: &mut Rng| {
                let int = (r.next_u64() % 1_000_000) as i128 + 1;
                let frac = (r.next_u64() % 1_000_000_000_000) as i128;
                let raw = int * D96::SCALE + frac;
                D96::from_raw(if r.next_u64() & 1 == 1 { -raw } else { raw })
            };
            (mk(&mut rng), mk(&mut rng))
        })
        .collect();

    let add = best_ns(|| d96.iter().fold(0i128, |s, (a, b)| s.wrapping_add(a.checked_add(*b).map_or(0, |v| v.to_raw()))));
    let sub = best_ns(|| d96.iter().fold(0i128, |s, (a, b)| s.wrapping_add(a.checked_sub(*b).map_or(0, |v| v.to_raw()))));
    let mul = best_ns(|| d96.iter().fold(0i128, |s, (a, b)| s.wrapping_add(a.checked_mul(*b).map_or(0, |v| v.to_raw()))));
    let div = best_ns(|| d96.iter().fold(0i128, |s, (a, b)| s.wrapping_add(a.checked_div(*b).map_or(0, |v| v.to_raw()))));
    println!("D96   add {add:5.2}   sub {sub:5.2}   mul {mul:5.2}   div {div:5.2}   (small divisor / fast path)");

    // ---- D96 division with LARGE divisors (raw >= 2^64 -> base-2^32 slow path) ----
    let mut rng = Rng::new(0x5105);
    let d96_big: Vec<(D96, D96)> = (0..N)
        .map(|_| {
            let num = {
                let int = (rng.next_u64() % 1_000_000) as i128 + 1;
                D96::from_raw(int * D96::SCALE + (rng.next_u64() % 1_000_000_000_000) as i128)
            };
            // divisor value in [2e7, ~1e8): raw >= 2e19 > 2^64
            let den = {
                let int = 20_000_000i128 + (rng.next_u64() % 80_000_000) as i128;
                D96::from_raw(int * D96::SCALE + (rng.next_u64() % 1_000_000_000_000) as i128)
            };
            (num, den)
        })
        .collect();
    let div_big = best_ns(|| d96_big.iter().fold(0i128, |s, (a, b)| s.wrapping_add(a.checked_div(*b).map_or(0, |v| v.to_raw()))));
    println!("D96   div {div_big:5.2}   (LARGE divisor >= 2^64 / slow path)");
}
