#![cfg(feature = "zerocopy")]
//! Zero-copy (`zerocopy` FromBytes/IntoBytes) tests for D64 and D96.
//!
//! Run with: `cargo test --features zerocopy`

use core::str::FromStr;
use fixdec::{D64, D96};
use zerocopy::{FromBytes, IntoBytes};

#[test]
fn d64_as_bytes_and_back() {
    let v = D64::from_str("1234.56789").unwrap();
    let b: &[u8] = v.as_bytes();
    assert_eq!(b.len(), 8);
    assert_eq!(b, &v.to_ne_bytes());
    assert_eq!(D64::read_from_bytes(b).unwrap(), v);
}

#[test]
fn d64_slice_roundtrip() {
    let v = [D64::ONE, D64::from_i32(2), D64::MAX, D64::MIN];
    let b: &[u8] = v.as_bytes();
    assert_eq!(b.len(), 32);
    let back = <[D64]>::ref_from_bytes(b).unwrap();
    assert_eq!(back, &v);
}

#[test]
fn d96_slice_roundtrip() {
    let v = [D96::from_str("2500.123456789012").unwrap(), D96::MAX, D96::MIN];
    let b: &[u8] = v.as_bytes();
    assert_eq!(b.len(), 48);
    let back = <[D96]>::ref_from_bytes(b).unwrap();
    assert_eq!(back, &v);
}
