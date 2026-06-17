#![cfg(feature = "bytemuck")]
//! Zero-copy (`bytemuck` Pod/Zeroable) tests for D64 and D96.
//!
//! Run with: `cargo test --features bytemuck`

use bytemuck::{Zeroable, bytes_of, cast_slice, from_bytes};
use core::str::FromStr;
use fixdec::{D64, D96};

#[test]
fn zeroable_is_zero() {
    assert_eq!(D64::zeroed(), D64::ZERO);
    assert_eq!(D96::zeroed(), D96::ZERO);
}

#[test]
fn d64_slice_byte_roundtrip() {
    let v = [
        D64::from_str("1.5").unwrap(),
        D64::from_str("-2.25").unwrap(),
        D64::MAX,
        D64::MIN,
    ];
    let bytes: &[u8] = cast_slice(&v);
    assert_eq!(bytes.len(), v.len() * 8);
    let back: &[D64] = cast_slice(bytes); // same buffer => aligned
    assert_eq!(back, &v);
}

#[test]
fn d96_slice_byte_roundtrip() {
    let v = [
        D96::from_str("2500.123456789012").unwrap(),
        D96::from_str("-0.5").unwrap(),
        D96::MAX,
        D96::MIN,
    ];
    let bytes: &[u8] = cast_slice(&v);
    assert_eq!(bytes.len(), v.len() * 16);
    let back: &[D96] = cast_slice(bytes);
    assert_eq!(back, &v);
}

#[test]
fn bytes_of_matches_ne_bytes() {
    let a = D64::from_str("1234.56789").unwrap();
    let b = bytes_of(&a);
    assert_eq!(b.len(), 8);
    assert_eq!(b, &a.to_ne_bytes());
    assert_eq!(*from_bytes::<D64>(b), a);

    let c = D96::from_str("1234.567890123456").unwrap();
    let d = bytes_of(&c);
    assert_eq!(d.len(), 16);
    assert_eq!(d, &c.to_ne_bytes());
    assert_eq!(*from_bytes::<D96>(d), c);
}

#[test]
fn vec_in_buffer_roundtrip() {
    let v: Vec<D64> = vec![D64::ONE, D64::from_i32(2), D64::from_i32(3)];
    let bytes: &[u8] = cast_slice(&v);
    assert_eq!(bytes.len(), 24);
    let back: &[D64] = cast_slice(bytes);
    assert_eq!(back, v.as_slice());
}

// A #[repr(C)] struct of decimals is itself Pod, so whole records cast to bytes.
#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(C)]
struct Trade {
    price: D64,
    qty: D64,
    fee: D64,
}
// SAFETY: repr(C) struct of three Pod D64 fields; all 8-byte aligned so there is
// no padding, and every bit pattern is valid.
unsafe impl Zeroable for Trade {}
unsafe impl bytemuck::Pod for Trade {}

#[test]
fn struct_of_decimals_cast() {
    let t = Trade {
        price: D64::from_str("100.5").unwrap(),
        qty: D64::from_i32(10),
        fee: D64::CENT,
    };
    let bytes = bytes_of(&t);
    assert_eq!(bytes.len(), 24);
    assert_eq!(*from_bytes::<Trade>(bytes), t);

    let arr = [t, t];
    let sb: &[u8] = cast_slice(&arr);
    assert_eq!(sb.len(), 48);
    let back: &[Trade] = cast_slice(sb);
    assert_eq!(back, &arr);
}
