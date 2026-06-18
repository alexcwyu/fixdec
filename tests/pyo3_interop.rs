//! PyO3 interop (feature `pyo3`). Run with `cargo test --features pyo3`.
//!
//! These spin up an embedded CPython via pyo3's `auto-initialize` dev feature
//! and exercise D64/D96 <-> Python `decimal.Decimal` / int / float / str.

#![cfg(feature = "pyo3")]

use core::str::FromStr;

use fixdec::{D64, D96};
use pyo3::prelude::*;
use pyo3::types::PyAnyMethods;

fn py_decimal<'py>(py: Python<'py>, s: &str) -> Bound<'py, PyAny> {
    py.import("decimal")
        .unwrap()
        .getattr("Decimal")
        .unwrap()
        .call1((s,))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Rust -> Python: produces an exact decimal.Decimal
// ---------------------------------------------------------------------------

#[test]
fn d64_into_python_is_exact_decimal() {
    Python::attach(|py| {
        for s in ["0", "1234.5678", "-0.00000001", "92233720368.54775807"] {
            let obj = D64::from_str(s).unwrap().into_pyobject(py).unwrap();
            // It is a decimal.Decimal...
            let is_dec = obj
                .is_instance(&py.import("decimal").unwrap().getattr("Decimal").unwrap())
                .unwrap();
            assert!(is_dec, "D64 -> decimal.Decimal for {s}");
            // ...equal to Decimal(canonical_string).
            let expected = py_decimal(py, &D64::from_str(s).unwrap().to_string());
            assert!(obj.eq(&expected).unwrap(), "value preserved for {s}");
        }
    });
}

#[test]
fn d96_into_python_is_exact_decimal() {
    Python::attach(|py| {
        for s in ["0", "123.456789012", "-0.000000000001"] {
            let obj = D96::from_str(s).unwrap().into_pyobject(py).unwrap();
            let expected = py_decimal(py, &D96::from_str(s).unwrap().to_string());
            assert!(obj.eq(&expected).unwrap(), "D96 -> Decimal for {s}");
        }
    });
}

// ---------------------------------------------------------------------------
// Python -> Rust: Decimal / int / float / str all extract
// ---------------------------------------------------------------------------

#[test]
fn d64_from_python_decimal_and_str() {
    Python::attach(|py| {
        // Decimal whose str() would be scientific ("1E-7") must still extract.
        let dec = py_decimal(py, "0.0000001");
        assert_eq!(dec.extract::<D64>().unwrap(), D64::from_str("0.0000001").unwrap());
        // plain Decimal
        let dec2 = py_decimal(py, "1234.5678");
        assert_eq!(dec2.extract::<D64>().unwrap(), D64::from_str("1234.5678").unwrap());
        // Python str
        let s = "-42.5".into_pyobject(py).unwrap();
        assert_eq!(s.extract::<D64>().unwrap(), D64::from_str("-42.5").unwrap());
    });
}

#[test]
fn d64_from_python_int_and_float() {
    Python::attach(|py| {
        // int
        let i = 100_i64.into_pyobject(py).unwrap();
        assert_eq!(i.extract::<D64>().unwrap(), D64::from_str("100").unwrap());
        // float goes through from_f64 (rounds to 8 dp); 0.5 is exact
        let f = 0.5_f64.into_pyobject(py).unwrap();
        assert_eq!(f.extract::<D64>().unwrap(), D64::from_str("0.5").unwrap());
    });
}

#[test]
fn d96_from_python_types() {
    Python::attach(|py| {
        let dec = py_decimal(py, "0.000000000001");
        assert_eq!(dec.extract::<D96>().unwrap(), D96::from_str("0.000000000001").unwrap());
        let i = 100_i64.into_pyobject(py).unwrap();
        assert_eq!(i.extract::<D96>().unwrap(), D96::from_str("100").unwrap());
        let f = 0.25_f64.into_pyobject(py).unwrap();
        assert_eq!(f.extract::<D96>().unwrap(), D96::from_str("0.25").unwrap());
    });
}

// ---------------------------------------------------------------------------
// Round trip and error behavior
// ---------------------------------------------------------------------------

#[test]
fn d64_python_roundtrip() {
    Python::attach(|py| {
        for s in ["0", "1", "-1", "1234.5678", "0.00000001"] {
            let d = D64::from_str(s).unwrap();
            let obj = d.into_pyobject(py).unwrap();
            assert_eq!(obj.extract::<D64>().unwrap(), d, "round trip {s}");
        }
    });
}

#[test]
fn out_of_range_and_precision_loss_raise() {
    Python::attach(|py| {
        // int beyond D64 range -> ValueError
        let big = py
            .import("builtins")
            .unwrap()
            .getattr("int")
            .unwrap()
            .call1(("100000000000",))
            .unwrap();
        assert!(big.extract::<D64>().is_err(), "1e11 int overflows D64");
        // Decimal with 9 dp -> strict parse rejects (PrecisionLoss) -> ValueError
        let too_precise = py_decimal(py, "0.000000001");
        assert!(too_precise.extract::<D64>().is_err(), "9 dp not representable in D64");
        // ...but D96 (12 dp) accepts the same value
        assert_eq!(
            too_precise.extract::<D96>().unwrap(),
            D96::from_str("0.000000001").unwrap()
        );
    });
}
