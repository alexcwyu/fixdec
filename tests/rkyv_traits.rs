#![cfg(feature = "rkyv")]
//! rkyv 0.8 zero-copy archival: round-trip for D64/D96 and, critically, that the
//! safe access path REJECTS archived D96 bytes whose raw value is outside the
//! 96-bit range (the rkyv mirror of withholding zerocopy `FromBytes` / bytemuck
//! `Pod` for D96).

use fixdec::{D64, D96};
use rkyv::rancor::Error;

#[test]
fn d64_rkyv_roundtrip() {
    let cases = [
        D64::ZERO,
        D64::ONE,
        D64::MAX,
        D64::MIN,
        D64::from_raw(-123_456_789),
        D64::from_raw(987_654_321),
    ];
    for d in cases {
        let bytes = rkyv::to_bytes::<Error>(&d).unwrap();
        // Safe access validates (always succeeds: every i64 is a valid D64).
        assert!(rkyv::access::<rkyv::Archived<D64>, Error>(&bytes).is_ok());
        // Owned, validated deserialize round-trips exactly.
        let back: D64 = rkyv::from_bytes::<D64, Error>(&bytes).unwrap();
        assert_eq!(back, d);
        // Portable little-endian: the archive is the raw i64 in LE.
        assert_eq!(&bytes[..8], &d.to_raw().to_le_bytes());
    }
}

#[test]
fn d96_rkyv_roundtrip() {
    let cases = [
        D96::ZERO,
        D96::ONE,
        D96::MAX,
        D96::MIN,
        D96::from_raw(-123_456_789_012),
        D96::from_raw(987_654_321_098),
    ];
    for d in cases {
        let bytes = rkyv::to_bytes::<Error>(&d).unwrap();
        assert!(rkyv::access::<rkyv::Archived<D96>, Error>(&bytes).is_ok());
        let back: D96 = rkyv::from_bytes::<D96, Error>(&bytes).unwrap();
        assert_eq!(back, d);
        assert_eq!(&bytes[..16], &d.to_raw().to_le_bytes());
    }
}

#[test]
fn d96_rkyv_rejects_out_of_96bit_range() {
    // Serialize a valid D96, then overwrite the archived i128 with an out-of-range
    // raw value. The safe access + owned paths must both reject it.
    let mut bytes = rkyv::to_bytes::<Error>(&D96::ONE).unwrap();
    assert_eq!(bytes.len(), 16, "a lone D96 archives to exactly its 16 LE bytes");

    for bad in [
        D96::MAX.to_raw() + 1,        // one past the 96-bit max
        D96::MIN.to_raw() - 1,        // one past the 96-bit min
        i128::MAX,
        i128::MIN,
    ] {
        bytes.as_mut_slice()[..16].copy_from_slice(&bad.to_le_bytes());
        assert!(
            rkyv::access::<rkyv::Archived<D96>, Error>(&bytes).is_err(),
            "access must reject out-of-range raw {bad}"
        );
        assert!(
            rkyv::from_bytes::<D96, Error>(&bytes).is_err(),
            "from_bytes must reject out-of-range raw {bad}"
        );
    }

    // In-range boundary values still validate after the same round-trip path.
    for good in [D96::MAX.to_raw(), D96::MIN.to_raw(), 0] {
        bytes.as_mut_slice()[..16].copy_from_slice(&good.to_le_bytes());
        assert!(rkyv::access::<rkyv::Archived<D96>, Error>(&bytes).is_ok());
    }
}
