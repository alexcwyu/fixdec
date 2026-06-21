//! Tick-size and lot-size quantization tests.
//!
//! `quantize`/`checked_quantize` round to the nearest multiple of a tick using a
//! `RoundingStrategy`; `checked_floor_to_tick`/`checked_ceil_to_tick` are the
//! unambiguous directional variants.

use core::str::FromStr;
use fixdec::RoundingStrategy::*;
use fixdec::{D64, D96};

#[test]
fn d64_quantize_to_tick() {
    let tick = D64::from_str("0.05").unwrap();
    assert_eq!(
        D64::from_str("1.07")
            .unwrap()
            .quantize(tick, MidpointNearestEven),
        D64::from_str("1.05").unwrap()
    );
    assert_eq!(
        D64::from_str("1.08")
            .unwrap()
            .quantize(tick, MidpointNearestEven),
        D64::from_str("1.10").unwrap()
    );
    // Exact ties land on the even multiple (banker's): 1.075 -> 1.10, 1.025 -> 1.00.
    assert_eq!(
        D64::from_str("1.075")
            .unwrap()
            .quantize(tick, MidpointNearestEven),
        D64::from_str("1.10").unwrap()
    );
    assert_eq!(
        D64::from_str("1.025")
            .unwrap()
            .quantize(tick, MidpointNearestEven),
        D64::from_str("1.00").unwrap()
    );
    // Already on the grid: unchanged.
    assert_eq!(
        D64::from_str("1.05").unwrap().quantize(tick, ToZero),
        D64::from_str("1.05").unwrap()
    );
    // Directional.
    assert_eq!(
        D64::from_str("1.07").unwrap().checked_floor_to_tick(tick),
        Some(D64::from_str("1.05").unwrap())
    );
    assert_eq!(
        D64::from_str("1.07").unwrap().checked_ceil_to_tick(tick),
        Some(D64::from_str("1.10").unwrap())
    );
    assert_eq!(
        D64::from_str("-1.07").unwrap().checked_floor_to_tick(tick),
        Some(D64::from_str("-1.10").unwrap())
    );
    assert_eq!(
        D64::from_str("-1.07").unwrap().checked_ceil_to_tick(tick),
        Some(D64::from_str("-1.05").unwrap())
    );
    // tick <= 0 -> None.
    assert_eq!(
        D64::ONE.checked_quantize(D64::ZERO, MidpointNearestEven),
        None
    );
    assert_eq!(
        D64::ONE.checked_quantize(D64::from_str("-0.05").unwrap(), MidpointNearestEven),
        None
    );
}

#[test]
#[should_panic(expected = "tick must be positive")]
fn d64_quantize_zero_tick_panics() {
    let _ = D64::ONE.quantize(D64::ZERO, MidpointNearestEven);
}

#[test]
fn d96_quantize_to_tick() {
    let tick = D96::from_str("0.25").unwrap();
    assert_eq!(
        D96::from_str("10.30")
            .unwrap()
            .quantize(tick, MidpointNearestEven),
        D96::from_str("10.25").unwrap()
    );
    assert_eq!(
        D96::from_str("10.40")
            .unwrap()
            .quantize(tick, MidpointNearestEven),
        D96::from_str("10.50").unwrap()
    );
    assert_eq!(
        D96::from_str("10.30").unwrap().checked_floor_to_tick(tick),
        Some(D96::from_str("10.25").unwrap())
    );
    assert_eq!(
        D96::from_str("10.30").unwrap().checked_ceil_to_tick(tick),
        Some(D96::from_str("10.50").unwrap())
    );

    // A 1/32 bond tick (0.03125) — exact at 12 dp.
    let thirty_second = D96::from_str("0.03125").unwrap();
    assert_eq!(
        D96::from_str("100.10")
            .unwrap()
            .checked_floor_to_tick(thirty_second),
        Some(D96::from_str("100.09375").unwrap())
    );
    assert_eq!(
        D96::from_str("100.10")
            .unwrap()
            .checked_ceil_to_tick(thirty_second),
        Some(D96::from_str("100.125").unwrap())
    );

    assert_eq!(
        D96::ONE.checked_quantize(D96::ZERO, MidpointNearestEven),
        None
    );
}
