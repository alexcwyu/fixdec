//! Head-to-head arithmetic latency benchmark:
//! fixdec `D64`/`D96` vs `rust_decimal`, `fixed` (binary fixed-point),
//! `fpdec`, and `bigdecimal`.
//!
//! Runs `N` operations each of add / sub / mul / div per library and reports
//! ns/op and millions-of-ops per second (Mops/s).
//!
//! Run with (release is REQUIRED for a meaningful result):
//! ```text
//! cargo run --release --example bench_vs_libs
//! ITERS=5000000 cargo run --release --example bench_vs_libs   # override N
//! ```
//!
//! Fairness: every contender parses the SAME decimal operand strings, is timed
//! in the same process under the same optimisation profile, and uses
//! `black_box` on inputs and outputs. Per-library caveats are printed at the end.

use std::hint::black_box;
use std::str::FromStr;
use std::time::Instant;

use bigdecimal::{BigDecimal, RoundingMode};
use fixed::types::I64F64;
use fpdec::{Decimal as FpDecimal, DivRounded};
use rust_decimal::Decimal as RDecimal;

use fixdec::{D64, D96};

const PAIRS: usize = 256;
const MASK: usize = PAIRS - 1;

/// `PAIRS` bounded decimal operand strings `(a, b)`; `b` is always non-zero and
/// magnitudes stay small so no contender overflows on add/sub/mul/div.
fn operand_strings() -> Vec<(String, String)> {
    let mut v = Vec::with_capacity(PAIRS);
    for i in 0..PAIRS {
        let ai = 1 + (i % 99); // 1..=99
        let af = i % 100; // .00..=.99
        let bi = 1 + (i % 9); // 1..=9
        let bf = (i * 7) % 100;
        v.push((format!("{ai}.{af:02}"), format!("{bi}.{bf:02}")));
    }
    v
}

struct Row {
    name: &'static str,
    kind: &'static str,
    add: f64,
    sub: f64,
    mul: f64,
    div: f64,
}

/// Time `n` applications of `op` over `Copy` operands, black-boxing in and out.
/// Returns ns per op.
fn time_copy<T: Copy>(n: u64, a: &[T], b: &[T], op: impl Fn(T, T) -> T) -> f64 {
    // Warm up caches / CPU frequency.
    let mut acc = a[0];
    for i in 0..(n / 16) {
        let idx = (i as usize) & MASK;
        acc = op(black_box(a[idx]), black_box(b[idx]));
    }
    black_box(acc);

    let start = Instant::now();
    let mut acc = a[0];
    for i in 0..n {
        let idx = (i as usize) & MASK;
        acc = black_box(op(black_box(a[idx]), black_box(b[idx])));
    }
    black_box(acc);
    start.elapsed().as_nanos() as f64 / n as f64
}

/// Same as [`time_copy`] but for non-`Copy` (heap) types operated on by ref.
fn time_ref<T>(n: u64, a: &[T], b: &[T], op: impl Fn(&T, &T) -> T) -> f64 {
    for i in 0..(n / 16) {
        let idx = (i as usize) & MASK;
        black_box(op(black_box(&a[idx]), black_box(&b[idx])));
    }

    let start = Instant::now();
    for i in 0..n {
        let idx = (i as usize) & MASK;
        let r = op(black_box(&a[idx]), black_box(&b[idx]));
        black_box(r);
    }
    start.elapsed().as_nanos() as f64 / n as f64
}

#[allow(clippy::too_many_arguments)]
fn bench_copy<T: Copy>(
    name: &'static str,
    kind: &'static str,
    n: u64,
    a: &[T],
    b: &[T],
    add: impl Fn(T, T) -> T,
    sub: impl Fn(T, T) -> T,
    mul: impl Fn(T, T) -> T,
    div: impl Fn(T, T) -> T,
) -> Row {
    Row {
        name,
        kind,
        add: time_copy(n, a, b, add),
        sub: time_copy(n, a, b, sub),
        mul: time_copy(n, a, b, mul),
        div: time_copy(n, a, b, div),
    }
}

fn main() {
    let n: u64 = std::env::var("ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);

    let pairs = operand_strings();
    let sa: Vec<&str> = pairs.iter().map(|(a, _)| a.as_str()).collect();
    let sb: Vec<&str> = pairs.iter().map(|(_, b)| b.as_str()).collect();

    // ---- construct identical operands per library (outside timing) ----
    let d64_a: Vec<D64> = sa.iter().map(|s| D64::from_str(s).unwrap()).collect();
    let d64_b: Vec<D64> = sb.iter().map(|s| D64::from_str(s).unwrap()).collect();
    let d96_a: Vec<D96> = sa.iter().map(|s| D96::from_str(s).unwrap()).collect();
    let d96_b: Vec<D96> = sb.iter().map(|s| D96::from_str(s).unwrap()).collect();

    let rd_a: Vec<RDecimal> = sa.iter().map(|s| RDecimal::from_str(s).unwrap()).collect();
    let rd_b: Vec<RDecimal> = sb.iter().map(|s| RDecimal::from_str(s).unwrap()).collect();

    let fx_a: Vec<I64F64> = sa.iter().map(|s| I64F64::from_str(s).unwrap()).collect();
    let fx_b: Vec<I64F64> = sb.iter().map(|s| I64F64::from_str(s).unwrap()).collect();

    let fp_a: Vec<FpDecimal> = sa.iter().map(|s| FpDecimal::from_str(s).unwrap()).collect();
    let fp_b: Vec<FpDecimal> = sb.iter().map(|s| FpDecimal::from_str(s).unwrap()).collect();

    let bd_a: Vec<BigDecimal> = sa
        .iter()
        .map(|s| BigDecimal::from_str(s).unwrap())
        .collect();
    let bd_b: Vec<BigDecimal> = sb
        .iter()
        .map(|s| BigDecimal::from_str(s).unwrap())
        .collect();

    // Raw IEEE-754 f64: the hardware floating-point speed ceiling, not decimal-exact.
    let f64_a: Vec<f64> = sa.iter().map(|s| s.parse::<f64>().unwrap()).collect();
    let f64_b: Vec<f64> = sb.iter().map(|s| s.parse::<f64>().unwrap()).collect();

    let mut rows = Vec::new();

    rows.push(bench_copy(
        "fixdec D64",
        "dec fixed 8dp",
        n,
        &d64_a,
        &d64_b,
        |x, y| x + y,
        |x, y| x - y,
        |x, y| x * y,
        |x, y| x / y,
    ));
    rows.push(bench_copy(
        "fixdec D96",
        "dec fixed 12dp",
        n,
        &d96_a,
        &d96_b,
        |x, y| x + y,
        |x, y| x - y,
        |x, y| x * y,
        |x, y| x / y,
    ));
    rows.push(bench_copy(
        "rust_decimal",
        "dec scale 0-28",
        n,
        &rd_a,
        &rd_b,
        |x, y| x + y,
        |x, y| x - y,
        |x, y| x * y,
        |x, y| (x / y).round_dp(8),
    ));
    rows.push(bench_copy(
        "fixed I64F64",
        "binary fixed",
        n,
        &fx_a,
        &fx_b,
        |x, y| x + y,
        |x, y| x - y,
        |x, y| x * y,
        |x, y| x / y,
    ));
    rows.push(bench_copy(
        "fpdec",
        "dec scale 0-18",
        n,
        &fp_a,
        &fp_b,
        |x, y| x + y,
        |x, y| x - y,
        |x, y| x * y,
        |x, y| x.div_rounded(y, 8),
    ));
    rows.push(bench_copy(
        "f64 (double)",
        "binary float",
        n,
        &f64_a,
        &f64_b,
        |x, y| x + y,
        |x, y| x - y,
        |x, y| x * y,
        |x, y| x / y,
    ));
    rows.push(Row {
        name: "bigdecimal",
        kind: "arbprec heap",
        add: time_ref(n, &bd_a, &bd_b, |x, y| x + y),
        sub: time_ref(n, &bd_a, &bd_b, |x, y| x - y),
        mul: time_ref(n, &bd_a, &bd_b, |x, y| x * y),
        div: time_ref(n, &bd_a, &bd_b, |x, y| {
            (x / y).with_scale_round(8, RoundingMode::HalfEven)
        }),
    });

    // ---------------- report ----------------
    println!();
    println!(
        "fixdec arithmetic benchmark — {} ops/op, {} operand pairs",
        n, PAIRS
    );
    println!("(release build; black_box guarded; div bounded to ~8 dp where the lib allows)\n");

    println!("ns per operation (lower = faster):");
    println!(
        "{:<14} {:<16} {:>9} {:>9} {:>9} {:>9}",
        "library", "kind", "add", "sub", "mul", "div"
    );
    println!("{}", "-".repeat(70));
    for r in &rows {
        println!(
            "{:<14} {:<16} {:>9.2} {:>9.2} {:>9.2} {:>9.2}",
            r.name, r.kind, r.add, r.sub, r.mul, r.div
        );
    }

    println!("\nthroughput — millions of ops per second (higher = faster):");
    println!(
        "{:<14} {:<16} {:>9} {:>9} {:>9} {:>9}",
        "library", "kind", "add", "sub", "mul", "div"
    );
    println!("{}", "-".repeat(70));
    let mops = |ns: f64| 1_000.0 / ns;
    for r in &rows {
        println!(
            "{:<14} {:<16} {:>9.1} {:>9.1} {:>9.1} {:>9.1}",
            r.name,
            r.kind,
            mops(r.add),
            mops(r.sub),
            mops(r.mul),
            mops(r.div)
        );
    }

    println!("\nrelative to fixdec D64 (x = how many times slower than D64; <1 = faster):");
    let base = &rows[0];
    println!(
        "{:<14} {:>9} {:>9} {:>9} {:>9}",
        "library", "add", "sub", "mul", "div"
    );
    println!("{}", "-".repeat(54));
    for r in &rows {
        println!(
            "{:<14} {:>8.2}x {:>8.2}x {:>8.2}x {:>8.2}x",
            r.name,
            r.add / base.add,
            r.sub / base.sub,
            r.mul / base.mul,
            r.div / base.div
        );
    }

    println!(
        "\nNotes / fairness caveats:\n\
         - fixdec D64 = i64 @ 8dp, D96 = i128 @ 12dp (exact base-10, Copy, no heap).\n\
         - rust_decimal: 128-bit base-10, per-value scale 0-28; mul renormalises scale, \n\
           div goes to 28 digits then we round_dp(8). 16 bytes, Copy.\n\
         - fixed I64F64: BINARY fixed-point (base-2) — cannot represent 0.1 exactly; \n\
           fastest reference but not decimal-exact. div truncates toward zero.\n\
         - fpdec: i128 base-10, per-value scale 0-18; div uses div_rounded(.,8) (bare `/`\n\
           panics on non-terminating quotients). Copy.\n\
         - bigdecimal: arbitrary-precision heap BigInt — every op allocates; div bounded\n\
           with with_scale_round(8, HalfEven). Measures heap+bignum overhead, by design.\n\
         - f64 (double): raw IEEE-754 hardware float — the absolute speed ceiling, but NOT\n\
           decimal-exact (0.1 is inexact, rounding error accumulates). Shown as a reference\n\
           floor; it is the wrong type for money/prices, which is fixdec's entire reason to exist.\n\
         - div dp differs by type (D96=12dp, fixed=~19 binary digits, others=8dp); \n\
           treat div cross-library numbers as indicative, not exact apples-to-apples."
    );
}
