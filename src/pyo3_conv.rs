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
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyFloat, PyInt, PyType};

use crate::{D64, D96};

/// The `decimal.Decimal` class, imported once per interpreter and cached. Avoids
/// a per-conversion `import("decimal").getattr("Decimal")` on every hot call.
fn decimal_type(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
    static DECIMAL: PyOnceLock<Py<PyType>> = PyOnceLock::new();
    DECIMAL.import(py, "decimal", "Decimal")
}

/// Builds a Python `decimal.Decimal` from an exact decimal string.
fn to_py_decimal<'py>(py: Python<'py>, s: &str) -> PyResult<Bound<'py, PyAny>> {
    decimal_type(py)?.call1((s,))
}

/// Normalizes a Python `Decimal` or numeric `str` to a fixed-point decimal
/// string (never scientific). `str(Decimal("0.0000001"))` is `"1E-7"`, which the
/// parsers reject; `Decimal(ob).__format__("f")` (i.e. `format(., "f")`) yields
/// `"0.0000001"` instead, so every representable value round-trips. Floats and
/// ints are handled by the callers before reaching here.
fn fixed_point_string(ob: &Bound<'_, PyAny>) -> PyResult<String> {
    let decimal = decimal_type(ob.py())?;
    if ob.get_type().is(decimal) {
        // Fast path: an EXACT `Decimal`, so format it directly and skip the
        // `Decimal(ob)` re-wrap (the common case for Decimal arguments). We
        // intentionally require the exact type, not `is_instance`: a `Decimal`
        // subclass may override `__format__` and return something other than the
        // numeric value, so subclasses fall through to the base wrap below.
        ob.call_method1("__format__", ("f",))?.extract::<String>()
    } else {
        // `str` (or anything Decimal-constructible): build a `Decimal` first so
        // the value is parsed and de-scientific-ified, then format it.
        decimal
            .call1((ob,))?
            .call_method1("__format__", ("f",))?
            .extract::<String>()
    }
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
        // bool is a subclass of int in Python; reject it so `True`/`False` are
        // not silently coerced to 1/0.
        if ob.is_instance_of::<PyBool>() {
            return Err(PyValueError::new_err("bool is not a valid D64 value"));
        }
        if ob.is_instance_of::<PyFloat>() {
            let f: f64 = ob.extract()?;
            return D64::from_f64(f).ok_or_else(|| {
                PyValueError::new_err(format!("float {f} is not representable as D64"))
            });
        }
        if ob.is_instance_of::<PyInt>() {
            let i: i64 = ob
                .extract()
                .map_err(|_| PyValueError::new_err("integer out of D64 range"))?;
            return D64::from_i64(i)
                .ok_or_else(|| PyValueError::new_err("integer out of D64 range"));
        }
        // Decimal or str: normalize to a fixed-point string, then parse exactly.
        // fixed_point_string fails (-> ValueError) for objects that are not
        // Decimal-constructible (TypeError / InvalidOperation). Non-finite
        // Decimals (Infinity / NaN) instead FORMAT successfully here as
        // "Infinity"/"NaN" and are rejected by from_str below. Either way the
        // caller sees a ValueError, so the contract stays uniform.
        let s = fixed_point_string(ob)
            .map_err(|_| PyValueError::new_err("value is not a decimal representable as D64"))?;
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
        // bool is a subclass of int in Python; reject it (see D64).
        if ob.is_instance_of::<PyBool>() {
            return Err(PyValueError::new_err("bool is not a valid D96 value"));
        }
        if ob.is_instance_of::<PyFloat>() {
            let f: f64 = ob.extract()?;
            return D96::from_f64(f).ok_or_else(|| {
                PyValueError::new_err(format!("float {f} is not representable as D96"))
            });
        }
        if ob.is_instance_of::<PyInt>() {
            let i: i128 = ob
                .extract()
                .map_err(|_| PyValueError::new_err("integer out of D96 range"))?;
            return D96::from_i128(i)
                .ok_or_else(|| PyValueError::new_err("integer out of D96 range"));
        }
        // Decimal or str: normalize to a fixed-point string, then parse exactly.
        // fixed_point_string fails (-> ValueError) for objects that are not
        // Decimal-constructible (TypeError / InvalidOperation). Non-finite
        // Decimals (Infinity / NaN) instead FORMAT successfully here as
        // "Infinity"/"NaN" and are rejected by from_str below. Either way the
        // caller sees a ValueError, so the contract stays uniform.
        let s = fixed_point_string(ob)
            .map_err(|_| PyValueError::new_err("value is not a decimal representable as D96"))?;
        D96::from_str(&s).map_err(|e| PyValueError::new_err(e.to_string()))
    }
}
