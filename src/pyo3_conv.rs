//! PyO3 interop (feature = "pyo3").
//!
//! Lets `D64`/`D96` cross the Rust↔Python boundary as native Python objects so
//! they can be used directly as `#[pyfunction]` arguments and return values.
//!
//! - **Rust → Python** ([`IntoPyObject`]): produces an exact `decimal.Decimal`
//!   built from the value's canonical decimal string (the `Display` form, which
//!   never uses scientific notation), so no precision is lost crossing over.
//! - **Python → Rust** ([`FromPyObject`]): accepts a Python `decimal.Decimal`,
//!   `str`, `int`, or `float`. A `float` goes through
//!   [`from_f64`](crate::D64::from_f64) (honest about the binary-float source);
//!   an `int` is converted exactly (range-checked); everything else is read via
//!   its fixed-point string form and parsed exactly, so a `Decimal` or numeric
//!   `str` round-trips losslessly. Out-of-range or unrepresentable inputs raise
//!   `ValueError`.

use core::str::FromStr;

use alloc::format;
use alloc::string::{String, ToString};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyFloat, PyInt};

use crate::{D64, D96};

/// Builds a Python `decimal.Decimal` from an exact decimal string.
fn to_py_decimal<'py>(py: Python<'py>, s: &str) -> PyResult<Bound<'py, PyAny>> {
    py.import("decimal")?.getattr("Decimal")?.call1((s,))
}

/// Normalizes a Python `Decimal` or numeric `str` to a fixed-point decimal
/// string (never scientific). `str(Decimal("0.0000001"))` is `"1E-7"`, which the
/// parsers reject; `format(Decimal(ob), "f")` yields `"0.0000001"` instead, so
/// every representable value round-trips. Floats and ints are handled by the
/// callers before reaching here.
fn fixed_point_string(ob: &Bound<'_, PyAny>) -> PyResult<String> {
    let py = ob.py();
    let as_decimal = py.import("decimal")?.getattr("Decimal")?.call1((ob,))?;
    py.import("builtins")?
        .getattr("format")?
        .call1((as_decimal, "f"))?
        .extract::<String>()
}

// ---------------------------------------------------------------------------
// D64
// ---------------------------------------------------------------------------

impl<'py> IntoPyObject<'py> for D64 {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        to_py_decimal(py, &self.to_string())
    }
}

impl<'py> IntoPyObject<'py> for &D64 {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        (*self).into_pyobject(py)
    }
}

impl<'py> FromPyObject<'py> for D64 {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        if ob.is_instance_of::<PyFloat>() {
            let f: f64 = ob.extract()?;
            return D64::from_f64(f)
                .ok_or_else(|| PyValueError::new_err(format!("float {f} is not representable as D64")));
        }
        if ob.is_instance_of::<PyInt>() {
            let i: i64 = ob
                .extract()
                .map_err(|_| PyValueError::new_err("integer out of D64 range"))?;
            return D64::from_i64(i).ok_or_else(|| PyValueError::new_err("integer out of D64 range"));
        }
        // Decimal or str: normalize to a fixed-point string, then parse exactly.
        let s = fixed_point_string(ob)?;
        D64::from_str(&s).map_err(|e| PyValueError::new_err(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// D96
// ---------------------------------------------------------------------------

impl<'py> IntoPyObject<'py> for D96 {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        to_py_decimal(py, &self.to_string())
    }
}

impl<'py> IntoPyObject<'py> for &D96 {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        (*self).into_pyobject(py)
    }
}

impl<'py> FromPyObject<'py> for D96 {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        if ob.is_instance_of::<PyFloat>() {
            let f: f64 = ob.extract()?;
            return D96::from_f64(f)
                .ok_or_else(|| PyValueError::new_err(format!("float {f} is not representable as D96")));
        }
        if ob.is_instance_of::<PyInt>() {
            let i: i128 = ob
                .extract()
                .map_err(|_| PyValueError::new_err("integer out of D96 range"))?;
            return D96::from_i128(i).ok_or_else(|| PyValueError::new_err("integer out of D96 range"));
        }
        // Decimal or str: normalize to a fixed-point string, then parse exactly.
        let s = fixed_point_string(ob)?;
        D96::from_str(&s).map_err(|e| PyValueError::new_err(e.to_string()))
    }
}
