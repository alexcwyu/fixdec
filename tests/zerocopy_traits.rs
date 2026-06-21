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
    let v = [
        D96::from_str("2500.123456789012").unwrap(),
        D96::MAX,
        D96::MIN,
    ];
    // D96 implements `IntoBytes` (value -> bytes) so the write direction is
    // D64 supports zero-copy reads.
    let b: &[u8] = v.as_bytes();
    assert_eq!(b.len(), 48);
    // D96 deliberately does not implement `FromBytes`, so `ref_from_bytes`
    // does not exist for D96 (it would smuggle out-of-96-bit values into D96
    // arithmetic). Read back through the checked reader instead.
    for (i, &d) in v.iter().enumerate() {
        assert_eq!(D96::try_read_ne_bytes(&b[i * 16..(i + 1) * 16]), Some(d));
    }
}

// D96 is not `FromBytes`. An out-of-96-bit pattern
// cannot be reinterpreted as a D96 via zerocopy; the checked reader rejects it.
#[test]
fn d96_out_of_range_bytes_rejected_not_constructed() {
    let oob = i128::MAX.to_ne_bytes();
    assert_eq!(D96::try_read_ne_bytes(&oob), None);
}
