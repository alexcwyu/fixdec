//! Codex review round 8, F3: selectable rounding strategies.
//! `round_dp_with_strategy` and `checked_div_rounded` opt into a
//! [`RoundingStrategy`]; every implicit rounding op stays banker's, and `*` / `/`
//! stay truncating.

use core::str::FromStr;
use fixdec::RoundingStrategy::*;
use fixdec::{D64, D96, RoundingStrategy};

#[test]
fn default_strategy_is_banker() {
    assert_eq!(RoundingStrategy::default(), MidpointNearestEven);
}

#[test]
fn round_dp_with_strategy_all_modes_on_ties() {
    // 2.5 -> 0 dp: q=2 (even).
    let v = D64::from_str("2.5").unwrap();
    assert_eq!(v.round_dp_with_strategy(0, MidpointNearestEven), D64::from_str("2").unwrap());
    assert_eq!(v.round_dp_with_strategy(0, MidpointAwayFromZero), D64::from_str("3").unwrap());
    assert_eq!(v.round_dp_with_strategy(0, MidpointTowardZero), D64::from_str("2").unwrap());
    assert_eq!(v.round_dp_with_strategy(0, ToZero), D64::from_str("2").unwrap());
    assert_eq!(v.round_dp_with_strategy(0, AwayFromZero), D64::from_str("3").unwrap());
    assert_eq!(v.round_dp_with_strategy(0, ToPositiveInfinity), D64::from_str("3").unwrap());
    assert_eq!(v.round_dp_with_strategy(0, ToNegativeInfinity), D64::from_str("2").unwrap());

    // 3.5 -> 0 dp: q=3 (odd), so banker's rounds up to 4.
    let w = D64::from_str("3.5").unwrap();
    assert_eq!(w.round_dp_with_strategy(0, MidpointNearestEven), D64::from_str("4").unwrap());
    assert_eq!(w.round_dp_with_strategy(0, MidpointTowardZero), D64::from_str("3").unwrap());

    // -2.5 -> 0 dp: floor=-3, ceil=-2, banker's -2 (even).
    let n = D64::from_str("-2.5").unwrap();
    assert_eq!(n.round_dp_with_strategy(0, MidpointNearestEven), D64::from_str("-2").unwrap());
    assert_eq!(n.round_dp_with_strategy(0, MidpointAwayFromZero), D64::from_str("-3").unwrap());
    assert_eq!(n.round_dp_with_strategy(0, ToNegativeInfinity), D64::from_str("-3").unwrap());
    assert_eq!(n.round_dp_with_strategy(0, ToPositiveInfinity), D64::from_str("-2").unwrap());
    assert_eq!(n.round_dp_with_strategy(0, ToZero), D64::from_str("-2").unwrap());
}

#[test]
fn round_dp_non_tie_directional() {
    let v = D64::from_str("2.4").unwrap();
    assert_eq!(v.round_dp_with_strategy(0, MidpointNearestEven), D64::from_str("2").unwrap());
    assert_eq!(v.round_dp_with_strategy(0, AwayFromZero), D64::from_str("3").unwrap());
    assert_eq!(v.round_dp_with_strategy(0, ToPositiveInfinity), D64::from_str("3").unwrap());
    let n = D64::from_str("-2.4").unwrap();
    assert_eq!(n.round_dp_with_strategy(0, ToNegativeInfinity), D64::from_str("-3").unwrap());
    assert_eq!(n.round_dp_with_strategy(0, ToZero), D64::from_str("-2").unwrap());
}

#[test]
fn round_dp_with_strategy_banker_equals_round_dp() {
    let d64s = [
        D64::from_str("123.456789").unwrap(),
        D64::from_str("-0.12345678").unwrap(),
        D64::MAX,
        D64::MIN,
        D64::from_raw(250_000_005),
    ];
    for v in d64s {
        for dp in 0..=8u8 {
            assert_eq!(v.round_dp_with_strategy(dp, MidpointNearestEven), v.round_dp(dp));
        }
    }
    let d96s = [
        D96::from_str("123.456789012345").unwrap(),
        D96::from_str("-0.123456789012").unwrap(),
        D96::MAX,
        D96::MIN,
    ];
    for v in d96s {
        for dp in 0..=12u8 {
            assert_eq!(v.round_dp_with_strategy(dp, MidpointNearestEven), v.round_dp(dp));
        }
    }
}

#[test]
fn checked_div_rounded_d64() {
    let ten = D64::from_str("10").unwrap();
    let three = D64::from_str("3").unwrap();
    // 10/3 = 3.3333...; third digit < 5 so the nearest modes agree.
    assert_eq!(ten.checked_div_rounded(three, 2, MidpointNearestEven), Some(D64::from_str("3.33").unwrap()));
    assert_eq!(ten.checked_div_rounded(three, 2, AwayFromZero), Some(D64::from_str("3.34").unwrap()));
    assert_eq!(ten.checked_div_rounded(three, 2, ToPositiveInfinity), Some(D64::from_str("3.34").unwrap()));

    // 1/8 = 0.125 -> exact tie at 2 dp.
    let one = D64::ONE;
    let eight = D64::from_str("8").unwrap();
    assert_eq!(one.checked_div_rounded(eight, 2, MidpointNearestEven), Some(D64::from_str("0.12").unwrap()));
    assert_eq!(one.checked_div_rounded(eight, 2, MidpointAwayFromZero), Some(D64::from_str("0.13").unwrap()));
    assert_eq!(one.checked_div_rounded(eight, 2, MidpointTowardZero), Some(D64::from_str("0.12").unwrap()));

    // Negative dividend / negative divisor.
    let neg_one = D64::from_str("-1").unwrap();
    assert_eq!(neg_one.checked_div_rounded(eight, 2, ToNegativeInfinity), Some(D64::from_str("-0.13").unwrap()));
    assert_eq!(neg_one.checked_div_rounded(eight, 2, ToPositiveInfinity), Some(D64::from_str("-0.12").unwrap()));
    let neg_three = D64::from_str("-3").unwrap();
    assert_eq!(ten.checked_div_rounded(neg_three, 2, ToZero), Some(D64::from_str("-3.33").unwrap()));
    assert_eq!(ten.checked_div_rounded(neg_three, 2, AwayFromZero), Some(D64::from_str("-3.34").unwrap()));

    // At full scale with ToZero it matches the truncating checked_div.
    assert_eq!(ten.checked_div_rounded(three, 8, ToZero), ten.checked_div(three));

    // Errors: divide by zero, dp out of range.
    assert_eq!(ten.checked_div_rounded(D64::ZERO, 2, MidpointNearestEven), None);
    assert_eq!(ten.checked_div_rounded(three, 9, MidpointNearestEven), None);
}

#[test]
fn checked_div_rounded_d96() {
    let ten = D96::from_str("10").unwrap();
    let three = D96::from_str("3").unwrap();
    assert_eq!(ten.checked_div_rounded(three, 4, MidpointNearestEven), Some(D96::from_str("3.3333").unwrap()));
    assert_eq!(ten.checked_div_rounded(three, 4, AwayFromZero), Some(D96::from_str("3.3334").unwrap()));

    let one = D96::ONE;
    let eight = D96::from_str("8").unwrap();
    assert_eq!(one.checked_div_rounded(eight, 2, MidpointNearestEven), Some(D96::from_str("0.12").unwrap()));
    assert_eq!(one.checked_div_rounded(eight, 2, MidpointAwayFromZero), Some(D96::from_str("0.13").unwrap()));

    assert_eq!(ten.checked_div_rounded(three, 12, ToZero), ten.checked_div(three));
    assert_eq!(ten.checked_div_rounded(D96::ZERO, 2, ToZero), None);

    // Documented intermediate-overflow boundary: |self| near MAX with high dp
    // overflows i128 -> None; a smaller dp on the same operands works.
    let two = D96::from_str("2").unwrap();
    assert_eq!(D96::MAX.checked_div_rounded(two, 12, MidpointNearestEven), None);
    assert!(D96::MAX.checked_div_rounded(two, 2, MidpointNearestEven).is_some());
}
