//! Round-4 review regression: untrusted bytes must never *silently* construct an
//! out-of-96-bit `D96`.
//!
//! D96 arithmetic assumes operands are <= 96 bits (e.g. `checked_mul` feeds
//! `unsigned_abs()` into `mul_96x96_to_192`, whose 64-bit limb splitting silently
//! truncates a >96-bit operand). Codex flagged that the `bytemuck::Pod` /
//! `zerocopy::FromBytes` impls reinterpreted ANY 128-bit pattern as a `D96`,
//! smuggling out-of-range values into that arithmetic. The fix drops the
//! bytes->`D96` trait directions (see `src/d96.rs`); the remaining bytes->`D96`
//! paths are now either CHECKED (`try_read_*_bytes` -> `Option`) or LOUD
//! (`from_*_bytes` panics). This file pins that contract. `D64` has no sub-range
//! invariant (every `i64` is a valid `D64`), so it keeps the full byte surface.

use fixdec::D96;

// First raw values just outside the legal 96-bit range, in both directions.
fn over_max() -> i128 {
    D96::MAX.to_raw() + 1
}
fn under_min() -> i128 {
    D96::MIN.to_raw() - 1
}

#[test]
fn try_read_rejects_out_of_range_bytes() {
    for oob in [i128::MAX, i128::MIN, over_max(), under_min()] {
        assert_eq!(D96::try_read_le_bytes(&oob.to_le_bytes()), None, "le {oob}");
        assert_eq!(D96::try_read_be_bytes(&oob.to_be_bytes()), None, "be {oob}");
        assert_eq!(D96::try_read_ne_bytes(&oob.to_ne_bytes()), None, "ne {oob}");
    }
}

#[test]
fn try_read_accepts_in_range_bytes_and_round_trips() {
    for v in [D96::ZERO, D96::ONE, D96::MAX, D96::MIN, D96::from_raw(-123_456_789)] {
        assert_eq!(D96::try_read_le_bytes(&v.to_le_bytes()), Some(v));
        assert_eq!(D96::try_read_be_bytes(&v.to_be_bytes()), Some(v));
        assert_eq!(D96::try_read_ne_bytes(&v.to_ne_bytes()), Some(v));
    }
}

#[test]
#[should_panic(expected = "96-bit range")]
fn from_le_bytes_panics_on_out_of_range() {
    let _ = D96::from_le_bytes(i128::MAX.to_le_bytes());
}

#[test]
#[should_panic(expected = "96-bit range")]
fn from_be_bytes_panics_on_out_of_range() {
    let _ = D96::from_be_bytes(i128::MIN.to_be_bytes());
}

#[test]
#[should_panic(expected = "96-bit range")]
fn from_ne_bytes_panics_just_over_max() {
    let _ = D96::from_ne_bytes(over_max().to_ne_bytes());
}

// The value->bytes direction is always safe and must keep round-tripping a valid
// D96 through the checked reader (this is the supported replacement for a
// `bytemuck`/`zerocopy` bytes->D96 reinterpretation).
#[test]
fn write_then_checked_read_round_trips() {
    for v in [D96::MAX, D96::MIN, D96::from_raw(1), D96::from_raw(-1)] {
        let ne = v.to_ne_bytes();
        assert_eq!(D96::try_read_ne_bytes(&ne), Some(v));
        assert_eq!(D96::read_ne_bytes(&ne), v);
    }
}
