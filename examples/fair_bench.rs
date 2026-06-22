//! Fixed-batch cross-library benchmark for decimal/math operations.
//!
//! Defaults:
//! - 10 runs
//! - 5,000,000 operations per run
//! - 1,000 operations per timed chunk
//! - drops best and worst run before reporting run mean
//!
//! Example:
//! ```text
//! cargo run --release --example fair_bench
//! RUNS=3 ITERS=100000 CHUNK=1000 cargo run --release --example fair_bench
//! ```

use std::fmt::Write as _;
use std::hint::black_box;
use std::str::FromStr;
use std::time::Instant;

use bigdecimal::{BigDecimal, RoundingMode};
use fixdec::{D64, D96};
use fixed::types::I64F64;
use fpdec::{Decimal as FpDecimal, DivRounded};
use rust_decimal::Decimal as RustDecimal;

const PAIRS: usize = 256;
const MASK: usize = PAIRS - 1;

#[derive(Clone, Copy)]
struct Config {
    runs: usize,
    iters: usize,
    chunk: usize,
}

#[derive(Clone, Copy)]
struct Stats {
    run_mean_ns: f64,
    run_median_ns: f64,
    chunk_p90_ns: f64,
    chunk_p99_ns: f64,
    chunk_p999_ns: f64,
}

struct Row {
    library: &'static str,
    kind: &'static str,
    op: &'static str,
    stats: Stats,
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&v| v > 0)
        .unwrap_or(default)
}

fn operand_strings() -> Vec<(String, String)> {
    let mut v = Vec::with_capacity(PAIRS);
    for i in 0..PAIRS {
        let ai = 1 + (i % 99);
        let af = (i * 37) % 100;
        let bi = 1 + (i % 9);
        let bf = (i * 17) % 100;
        v.push((format!("{ai}.{af:02}"), format!("{bi}.{bf:02}")));
    }
    v
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() as f64 * q).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[idx]
}

fn summarize(mut run_ns: Vec<f64>, mut chunk_ns: Vec<f64>) -> Stats {
    run_ns.sort_by(|a, b| a.total_cmp(b));
    chunk_ns.sort_by(|a, b| a.total_cmp(b));

    let kept = if run_ns.len() > 2 {
        &run_ns[1..run_ns.len() - 1]
    } else {
        &run_ns[..]
    };
    let run_mean_ns = kept.iter().sum::<f64>() / kept.len() as f64;
    let run_median_ns = percentile(&run_ns, 0.50);

    Stats {
        run_mean_ns,
        run_median_ns,
        chunk_p90_ns: percentile(&chunk_ns, 0.90),
        chunk_p99_ns: percentile(&chunk_ns, 0.99),
        chunk_p999_ns: percentile(&chunk_ns, 0.999),
    }
}

fn bench_copy<T: Copy>(
    cfg: Config,
    values_a: &[T],
    values_b: &[T],
    mut op: impl FnMut(T, T) -> T,
) -> Stats {
    let mut run_ns = Vec::with_capacity(cfg.runs);
    let mut chunk_ns = Vec::with_capacity(cfg.runs * (cfg.iters / cfg.chunk).max(1));

    for run in 0..cfg.runs {
        let mut acc = values_a[run & MASK];
        for i in 0..cfg.iters.min(100_000) {
            let idx = i & MASK;
            acc = op(black_box(values_a[idx]), black_box(values_b[idx]));
        }
        black_box(acc);

        let run_start = Instant::now();
        let mut done = 0usize;
        while done < cfg.iters {
            let this_chunk = cfg.chunk.min(cfg.iters - done);
            let chunk_start = Instant::now();
            for j in 0..this_chunk {
                let idx = (done + j) & MASK;
                acc = black_box(op(black_box(values_a[idx]), black_box(values_b[idx])));
            }
            let elapsed = chunk_start.elapsed().as_secs_f64() * 1e9;
            chunk_ns.push(elapsed / this_chunk as f64);
            done += this_chunk;
        }
        black_box(acc);
        run_ns.push(run_start.elapsed().as_secs_f64() * 1e9 / cfg.iters as f64);
    }

    summarize(run_ns, chunk_ns)
}

fn bench_ref<T>(
    cfg: Config,
    values_a: &[T],
    values_b: &[T],
    mut op: impl FnMut(&T, &T) -> T,
) -> Stats {
    let mut run_ns = Vec::with_capacity(cfg.runs);
    let mut chunk_ns = Vec::with_capacity(cfg.runs * (cfg.iters / cfg.chunk).max(1));

    for _ in 0..cfg.runs {
        for i in 0..cfg.iters.min(10_000) {
            let idx = i & MASK;
            black_box(op(black_box(&values_a[idx]), black_box(&values_b[idx])));
        }

        let run_start = Instant::now();
        let mut done = 0usize;
        while done < cfg.iters {
            let this_chunk = cfg.chunk.min(cfg.iters - done);
            let chunk_start = Instant::now();
            for j in 0..this_chunk {
                let idx = (done + j) & MASK;
                black_box(op(black_box(&values_a[idx]), black_box(&values_b[idx])));
            }
            let elapsed = chunk_start.elapsed().as_secs_f64() * 1e9;
            chunk_ns.push(elapsed / this_chunk as f64);
            done += this_chunk;
        }
        run_ns.push(run_start.elapsed().as_secs_f64() * 1e9 / cfg.iters as f64);
    }

    summarize(run_ns, chunk_ns)
}

fn bench_parse<T>(cfg: Config, inputs: &[&str], mut parse: impl FnMut(&str) -> T) -> Stats {
    let mut run_ns = Vec::with_capacity(cfg.runs);
    let mut chunk_ns = Vec::with_capacity(cfg.runs * (cfg.iters / cfg.chunk).max(1));

    for _ in 0..cfg.runs {
        for i in 0..cfg.iters.min(10_000) {
            black_box(parse(black_box(inputs[i & MASK])));
        }

        let run_start = Instant::now();
        let mut done = 0usize;
        while done < cfg.iters {
            let this_chunk = cfg.chunk.min(cfg.iters - done);
            let chunk_start = Instant::now();
            for j in 0..this_chunk {
                black_box(parse(black_box(inputs[(done + j) & MASK])));
            }
            let elapsed = chunk_start.elapsed().as_secs_f64() * 1e9;
            chunk_ns.push(elapsed / this_chunk as f64);
            done += this_chunk;
        }
        run_ns.push(run_start.elapsed().as_secs_f64() * 1e9 / cfg.iters as f64);
    }

    summarize(run_ns, chunk_ns)
}

fn bench_format_copy<T: Copy + std::fmt::Display>(cfg: Config, values: &[T]) -> Stats {
    let mut run_ns = Vec::with_capacity(cfg.runs);
    let mut chunk_ns = Vec::with_capacity(cfg.runs * (cfg.iters / cfg.chunk).max(1));

    for _ in 0..cfg.runs {
        let mut buf = String::with_capacity(64);
        for i in 0..cfg.iters.min(10_000) {
            buf.clear();
            write!(&mut buf, "{}", black_box(values[i & MASK])).unwrap();
            black_box(&buf);
        }

        let run_start = Instant::now();
        let mut done = 0usize;
        while done < cfg.iters {
            let this_chunk = cfg.chunk.min(cfg.iters - done);
            let chunk_start = Instant::now();
            for j in 0..this_chunk {
                buf.clear();
                write!(&mut buf, "{}", black_box(values[(done + j) & MASK])).unwrap();
                black_box(&buf);
            }
            let elapsed = chunk_start.elapsed().as_secs_f64() * 1e9;
            chunk_ns.push(elapsed / this_chunk as f64);
            done += this_chunk;
        }
        run_ns.push(run_start.elapsed().as_secs_f64() * 1e9 / cfg.iters as f64);
    }

    summarize(run_ns, chunk_ns)
}

fn bench_format_ref<T: std::fmt::Display>(cfg: Config, values: &[T]) -> Stats {
    let mut run_ns = Vec::with_capacity(cfg.runs);
    let mut chunk_ns = Vec::with_capacity(cfg.runs * (cfg.iters / cfg.chunk).max(1));

    for _ in 0..cfg.runs {
        let mut buf = String::with_capacity(64);
        for i in 0..cfg.iters.min(10_000) {
            buf.clear();
            write!(&mut buf, "{}", black_box(&values[i & MASK])).unwrap();
            black_box(&buf);
        }

        let run_start = Instant::now();
        let mut done = 0usize;
        while done < cfg.iters {
            let this_chunk = cfg.chunk.min(cfg.iters - done);
            let chunk_start = Instant::now();
            for j in 0..this_chunk {
                buf.clear();
                write!(&mut buf, "{}", black_box(&values[(done + j) & MASK])).unwrap();
                black_box(&buf);
            }
            let elapsed = chunk_start.elapsed().as_secs_f64() * 1e9;
            chunk_ns.push(elapsed / this_chunk as f64);
            done += this_chunk;
        }
        run_ns.push(run_start.elapsed().as_secs_f64() * 1e9 / cfg.iters as f64);
    }

    summarize(run_ns, chunk_ns)
}

fn add_row(
    rows: &mut Vec<Row>,
    library: &'static str,
    kind: &'static str,
    op: &'static str,
    stats: Stats,
) {
    rows.push(Row {
        library,
        kind,
        op,
        stats,
    });
}

fn main() {
    let cfg = Config {
        runs: env_usize("RUNS", 10),
        iters: env_usize("ITERS", 5_000_000),
        chunk: env_usize("CHUNK", 1_000),
    };

    let pairs = operand_strings();
    let sa: Vec<&str> = pairs.iter().map(|(a, _)| a.as_str()).collect();
    let sb: Vec<&str> = pairs.iter().map(|(_, b)| b.as_str()).collect();

    let d64_a: Vec<D64> = sa.iter().map(|s| D64::from_str(s).unwrap()).collect();
    let d64_b: Vec<D64> = sb.iter().map(|s| D64::from_str(s).unwrap()).collect();
    let d96_a: Vec<D96> = sa.iter().map(|s| D96::from_str(s).unwrap()).collect();
    let d96_b: Vec<D96> = sb.iter().map(|s| D96::from_str(s).unwrap()).collect();
    let f64_a: Vec<f64> = sa.iter().map(|s| s.parse::<f64>().unwrap()).collect();
    let f64_b: Vec<f64> = sb.iter().map(|s| s.parse::<f64>().unwrap()).collect();
    let rust_a: Vec<RustDecimal> = sa
        .iter()
        .map(|s| RustDecimal::from_str(s).unwrap())
        .collect();
    let rust_b: Vec<RustDecimal> = sb
        .iter()
        .map(|s| RustDecimal::from_str(s).unwrap())
        .collect();
    let fp_a: Vec<FpDecimal> = sa.iter().map(|s| FpDecimal::from_str(s).unwrap()).collect();
    let fp_b: Vec<FpDecimal> = sb.iter().map(|s| FpDecimal::from_str(s).unwrap()).collect();
    let fixed_a: Vec<I64F64> = sa.iter().map(|s| I64F64::from_str(s).unwrap()).collect();
    let fixed_b: Vec<I64F64> = sb.iter().map(|s| I64F64::from_str(s).unwrap()).collect();
    let big_a: Vec<BigDecimal> = sa
        .iter()
        .map(|s| BigDecimal::from_str(s).unwrap())
        .collect();
    let big_b: Vec<BigDecimal> = sb
        .iter()
        .map(|s| BigDecimal::from_str(s).unwrap())
        .collect();

    let mut rows = Vec::new();

    add_row(
        &mut rows,
        "D64",
        "dec fixed 8dp",
        "add",
        bench_copy(cfg, &d64_a, &d64_b, |a, b| a + b),
    );
    add_row(
        &mut rows,
        "D64",
        "dec fixed 8dp",
        "sub",
        bench_copy(cfg, &d64_a, &d64_b, |a, b| a - b),
    );
    add_row(
        &mut rows,
        "D64",
        "dec fixed 8dp",
        "mul",
        bench_copy(cfg, &d64_a, &d64_b, |a, b| a * b),
    );
    add_row(
        &mut rows,
        "D64",
        "dec fixed 8dp",
        "div",
        bench_copy(cfg, &d64_a, &d64_b, |a, b| a / b),
    );
    add_row(
        &mut rows,
        "D64",
        "dec fixed 8dp",
        "parse",
        bench_parse(cfg, &sa, |s| D64::from_str(s).unwrap()),
    );
    add_row(
        &mut rows,
        "D64",
        "dec fixed 8dp",
        "format",
        bench_format_copy(cfg, &d64_a),
    );

    add_row(
        &mut rows,
        "D96",
        "dec fixed 12dp",
        "add",
        bench_copy(cfg, &d96_a, &d96_b, |a, b| a + b),
    );
    add_row(
        &mut rows,
        "D96",
        "dec fixed 12dp",
        "sub",
        bench_copy(cfg, &d96_a, &d96_b, |a, b| a - b),
    );
    add_row(
        &mut rows,
        "D96",
        "dec fixed 12dp",
        "mul",
        bench_copy(cfg, &d96_a, &d96_b, |a, b| a * b),
    );
    add_row(
        &mut rows,
        "D96",
        "dec fixed 12dp",
        "div",
        bench_copy(cfg, &d96_a, &d96_b, |a, b| a / b),
    );
    add_row(
        &mut rows,
        "D96",
        "dec fixed 12dp",
        "parse",
        bench_parse(cfg, &sa, |s| D96::from_str(s).unwrap()),
    );
    add_row(
        &mut rows,
        "D96",
        "dec fixed 12dp",
        "format",
        bench_format_copy(cfg, &d96_a),
    );

    add_row(
        &mut rows,
        "f64",
        "binary float",
        "add",
        bench_copy(cfg, &f64_a, &f64_b, |a, b| a + b),
    );
    add_row(
        &mut rows,
        "f64",
        "binary float",
        "sub",
        bench_copy(cfg, &f64_a, &f64_b, |a, b| a - b),
    );
    add_row(
        &mut rows,
        "f64",
        "binary float",
        "mul",
        bench_copy(cfg, &f64_a, &f64_b, |a, b| a * b),
    );
    add_row(
        &mut rows,
        "f64",
        "binary float",
        "div",
        bench_copy(cfg, &f64_a, &f64_b, |a, b| a / b),
    );
    add_row(
        &mut rows,
        "f64",
        "binary float",
        "parse",
        bench_parse(cfg, &sa, |s| s.parse::<f64>().unwrap()),
    );
    add_row(
        &mut rows,
        "f64",
        "binary float",
        "format",
        bench_format_copy(cfg, &f64_a),
    );

    add_row(
        &mut rows,
        "rust_decimal",
        "dec scale 0-28",
        "add",
        bench_copy(cfg, &rust_a, &rust_b, |a, b| a + b),
    );
    add_row(
        &mut rows,
        "rust_decimal",
        "dec scale 0-28",
        "sub",
        bench_copy(cfg, &rust_a, &rust_b, |a, b| a - b),
    );
    add_row(
        &mut rows,
        "rust_decimal",
        "dec scale 0-28",
        "mul",
        bench_copy(cfg, &rust_a, &rust_b, |a, b| a * b),
    );
    add_row(
        &mut rows,
        "rust_decimal",
        "dec scale 0-28",
        "div",
        bench_copy(cfg, &rust_a, &rust_b, |a, b| (a / b).round_dp(8)),
    );
    add_row(
        &mut rows,
        "rust_decimal",
        "dec scale 0-28",
        "parse",
        bench_parse(cfg, &sa, |s| RustDecimal::from_str(s).unwrap()),
    );
    add_row(
        &mut rows,
        "rust_decimal",
        "dec scale 0-28",
        "format",
        bench_format_copy(cfg, &rust_a),
    );

    add_row(
        &mut rows,
        "fpdec",
        "dec scale 0-18",
        "add",
        bench_copy(cfg, &fp_a, &fp_b, |a, b| a + b),
    );
    add_row(
        &mut rows,
        "fpdec",
        "dec scale 0-18",
        "sub",
        bench_copy(cfg, &fp_a, &fp_b, |a, b| a - b),
    );
    add_row(
        &mut rows,
        "fpdec",
        "dec scale 0-18",
        "mul",
        bench_copy(cfg, &fp_a, &fp_b, |a, b| a * b),
    );
    add_row(
        &mut rows,
        "fpdec",
        "dec scale 0-18",
        "div",
        bench_copy(cfg, &fp_a, &fp_b, |a, b| a.div_rounded(b, 8)),
    );
    add_row(
        &mut rows,
        "fpdec",
        "dec scale 0-18",
        "parse",
        bench_parse(cfg, &sa, |s| FpDecimal::from_str(s).unwrap()),
    );
    add_row(
        &mut rows,
        "fpdec",
        "dec scale 0-18",
        "format",
        bench_format_copy(cfg, &fp_a),
    );

    add_row(
        &mut rows,
        "fixed I64F64",
        "binary fixed",
        "add",
        bench_copy(cfg, &fixed_a, &fixed_b, |a, b| a + b),
    );
    add_row(
        &mut rows,
        "fixed I64F64",
        "binary fixed",
        "sub",
        bench_copy(cfg, &fixed_a, &fixed_b, |a, b| a - b),
    );
    add_row(
        &mut rows,
        "fixed I64F64",
        "binary fixed",
        "mul",
        bench_copy(cfg, &fixed_a, &fixed_b, |a, b| a * b),
    );
    add_row(
        &mut rows,
        "fixed I64F64",
        "binary fixed",
        "div",
        bench_copy(cfg, &fixed_a, &fixed_b, |a, b| a / b),
    );
    add_row(
        &mut rows,
        "fixed I64F64",
        "binary fixed",
        "parse",
        bench_parse(cfg, &sa, |s| I64F64::from_str(s).unwrap()),
    );
    add_row(
        &mut rows,
        "fixed I64F64",
        "binary fixed",
        "format",
        bench_format_copy(cfg, &fixed_a),
    );

    add_row(
        &mut rows,
        "bigdecimal",
        "arbprec heap",
        "add",
        bench_ref(cfg, &big_a, &big_b, |a, b| a + b),
    );
    add_row(
        &mut rows,
        "bigdecimal",
        "arbprec heap",
        "sub",
        bench_ref(cfg, &big_a, &big_b, |a, b| a - b),
    );
    add_row(
        &mut rows,
        "bigdecimal",
        "arbprec heap",
        "mul",
        bench_ref(cfg, &big_a, &big_b, |a, b| a * b),
    );
    add_row(
        &mut rows,
        "bigdecimal",
        "arbprec heap",
        "div",
        bench_ref(cfg, &big_a, &big_b, |a, b| {
            (a / b).with_scale_round(8, RoundingMode::HalfEven)
        }),
    );
    add_row(
        &mut rows,
        "bigdecimal",
        "arbprec heap",
        "parse",
        bench_parse(cfg, &sa, |s| BigDecimal::from_str(s).unwrap()),
    );
    add_row(
        &mut rows,
        "bigdecimal",
        "arbprec heap",
        "format",
        bench_format_ref(cfg, &big_a),
    );

    println!(
        "\nfair benchmark: runs={} iters/run={} chunk={} operand_pairs={}",
        cfg.runs, cfg.iters, cfg.chunk, PAIRS
    );
    println!("run_mean drops best and worst run when runs > 2; pXX is over timed chunks.\n");
    println!(
        "{:<14} {:<16} {:<7} {:>12} {:>12} {:>12} {:>12} {:>12} {:>10}",
        "library", "kind", "op", "mean ns/op", "median", "p90", "p99", "p99.9", "Mops/s"
    );
    println!("{}", "-".repeat(112));
    for r in rows {
        println!(
            "{:<14} {:<16} {:<7} {:>12.3} {:>12.3} {:>12.3} {:>12.3} {:>12.3} {:>10.1}",
            r.library,
            r.kind,
            r.op,
            r.stats.run_mean_ns,
            r.stats.run_median_ns,
            r.stats.chunk_p90_ns,
            r.stats.chunk_p99_ns,
            r.stats.chunk_p999_ns,
            1_000.0 / r.stats.run_mean_ns,
        );
    }

    println!(
        "\nCaveats:\n\
         - f64 and fixed are binary, not decimal-exact; they are speed references.\n\
         - bigdecimal is heap/arbitrary precision; allocation cost is part of the point.\n\
         - division semantics differ: D64/D96 use native fixed scale, rust_decimal/fpdec/bigdecimal are rounded to 8 dp here.\n\
         - p99.9 is chunk-level latency per operation inside {}-op chunks, not single-operation hardware latency.",
        cfg.chunk
    );
}
