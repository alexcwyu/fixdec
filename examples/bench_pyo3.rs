//! Benchmark of the PyO3 interop conversions (feature `pyo3`).
//!
//! Run: `cargo run --release --features pyo3 --example bench_pyo3`
//!
//! Everything happens with the GIL held. Each "Rust -> Python" op builds a real
//! `decimal.Decimal` object; each "Python -> Rust" op extracts from a pre-built
//! Python object. Pure-Rust parse/format and a raw `f64` extract are included as
//! reference floors so the FFI / Python-object-construction overhead is visible.

use core::hint::black_box;
use core::str::FromStr;
use std::time::Instant;

use fixdec::{D64, D96};
use pyo3::prelude::*;
use pyo3::types::PyAnyMethods;
use rust_decimal::Decimal as RustDecimal;

const N: usize = 1_000_000;

fn bench<F: FnMut()>(label: &str, mut f: F) {
    for _ in 0..N / 10 {
        f();
    }
    let t = Instant::now();
    for _ in 0..N {
        f();
    }
    let ns = t.elapsed().as_nanos() as f64 / N as f64;
    let mops = 1000.0 / ns;
    println!("{label:<46}{ns:>10.1} ns/op {mops:>9.2} Mops/s");
}

fn py_decimal<'py>(py: Python<'py>, s: &str) -> Bound<'py, PyAny> {
    py.import("decimal")
        .unwrap()
        .getattr("Decimal")
        .unwrap()
        .call1((s,))
        .unwrap()
}

fn main() {
    Python::attach(|py| {
        println!("PyO3 interop benchmark — {N} ops/op (GIL held, black_box guarded)\n");

        let d64 = D64::from_str("1234.5678").unwrap();
        let d96 = D96::from_str("1234.567890123").unwrap();
        // rust_decimal has NO native pyo3 support, so the fair comparison is the
        // manual via-string conversion you would otherwise hand-write.
        let rd = RustDecimal::from_str("1234.5678").unwrap();

        // Pre-built Python objects for the extraction benchmarks.
        let py_dec = py_decimal(py, "1234.5678");
        let py_int = 100_i64.into_pyobject(py).unwrap();
        let py_float = 1234.5_f64.into_pyobject(py).unwrap();
        let py_str = "1234.5678".into_pyobject(py).unwrap();

        println!("Rust -> Python (build a decimal.Decimal):");
        bench("  D64  -> Decimal (into_pyobject)", || {
            black_box(black_box(d64).into_pyobject(py).unwrap());
        });
        bench("  D96  -> Decimal (into_pyobject)", || {
            black_box(black_box(d96).into_pyobject(py).unwrap());
        });
        bench("  rust_decimal -> Decimal (pyo3 native)", || {
            black_box(black_box(rd).into_pyobject(py).unwrap());
        });

        println!("\nPython -> Rust (extract):");
        bench("  Decimal -> D64 (extract)", || {
            black_box(black_box(&py_dec).extract::<D64>().unwrap());
        });
        bench("  int     -> D64 (extract)", || {
            black_box(black_box(&py_int).extract::<D64>().unwrap());
        });
        bench("  float   -> D64 (extract, from_f64)", || {
            black_box(black_box(&py_float).extract::<D64>().unwrap());
        });
        bench("  str     -> D64 (extract)", || {
            black_box(black_box(&py_str).extract::<D64>().unwrap());
        });
        bench("  Decimal -> D96 (extract)", || {
            black_box(black_box(&py_dec).extract::<D96>().unwrap());
        });
        bench("  Decimal -> rust_decimal (pyo3 native)", || {
            black_box(black_box(&py_dec).extract::<RustDecimal>().unwrap());
        });

        println!("\nRound trip:");
        bench("  D64 -> Decimal -> D64", || {
            let obj = black_box(d64).into_pyobject(py).unwrap();
            black_box(obj.extract::<D64>().unwrap());
        });

        println!("\nReference floors:");
        bench("  raw f64 extract (pyo3 builtin)", || {
            black_box(black_box(&py_float).extract::<f64>().unwrap());
        });
        bench("  pure Rust: D64::from_str", || {
            black_box(D64::from_str(black_box("1234.5678")).unwrap());
        });
        bench("  pure Rust: D64::to_string", || {
            black_box(black_box(d64).to_string());
        });
        bench("  pure Rust: RustDecimal::from_str", || {
            black_box(RustDecimal::from_str(black_box("1234.5678")).unwrap());
        });
        bench("  pure Rust: RustDecimal::to_string", || {
            black_box(black_box(rd).to_string());
        });
    });
}
