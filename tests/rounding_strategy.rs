//! Selectable rounding-strategy tests.
//!
//! `round_dp_with_strategy` and `checked_div_rounded` opt into a
//! [`RoundingStrategy`]; every implicit rounding op stays banker's, and `*` / `/`
//! stay truncating.

use core::str::FromStr;
use fixdec::RoundingStrategy::*;
use fixdec::{D64, D96, RoundingStrategy};
use proptest::prelude::*;

const STRATS: [RoundingStrategy; 7] = [
    MidpointNearestEven,
    MidpointAwayFromZero,
    MidpointTowardZero,
    ToZero,
    AwayFromZero,
    ToNegativeInfinity,
    ToPositiveInfinity,
];

/// Independent (structurally different) reference rounding of `num/den`
/// (`den > 0`) to an integer, used as an oracle for `checked_div_rounded`.
fn ref_round(num: i128, den: i128, s: RoundingStrategy) -> i128 {
    use core::cmp::Ordering::*;
    let q = num / den;
    let r = num - q * den;
    if r == 0 {
        return q;
    }
    let neg = num < 0;
    let cmp = (2 * r.abs()).cmp(&den);
    let away = match s {
        ToZero => false,
        AwayFromZero => true,
        ToPositiveInfinity => !neg,
        ToNegativeInfinity => neg,
        MidpointNearestEven => match cmp {
            Greater => true,
            Less => false,
            Equal => q % 2 != 0,
        },
        MidpointAwayFromZero => cmp != Less,
        MidpointTowardZero => cmp == Greater,
    };
    if away {
        if neg { q - 1 } else { q + 1 }
    } else {
        q
    }
}

/// i128 reference for `D96::checked_div_rounded`, valid only where `self_raw *
/// 10^dp` fits `i128` (the proptest filters on that). Returns the expected raw.
fn ref_div_rounded(self_raw: i128, rhs_raw: i128, dp: u8, s: RoundingStrategy) -> Option<i128> {
    let factor = 10i128.checked_pow(dp as u32)?;
    let num = self_raw.checked_mul(factor)?;
    let den = rhs_raw;
    let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
    let at_dp = ref_round(num, den, s);
    let full = at_dp.checked_mul(10i128.pow((12 - dp) as u32))?;
    if full > D96::MAX.to_raw() || full < D96::MIN.to_raw() {
        None
    } else {
        Some(full)
    }
}

const MIN96: i128 = -39_614_081_257_132_168_796_771_975_168;
const MAX96: i128 = 39_614_081_257_132_168_796_771_975_167;
// |self_raw| * 10^12 stays under i128::MAX (~1.7e38) for this bound, so the i128
// oracle is always valid here (no proptest rejects). This still spans the whole
// fast u128 path; the 192-bit wide path is covered by the full-range test below.
const ORACLE_LIM: i128 = 100_000_000_000_000_000_000_000_000; // 1e26

proptest! {
    // Exact rounding direction vs the independent i128 oracle, over all 7
    // strategies / signs / ties (divisor spans the full range, so quotients can
    // still overflow -> both sides agree on None).
    #[test]
    fn d96_div_rounded_matches_oracle(
        self_raw in -ORACLE_LIM..=ORACLE_LIM,
        rhs_raw in MIN96..=MAX96,
        dp in 0u8..=12,
        si in 0usize..7,
    ) {
        prop_assume!(rhs_raw != 0);
        let s = STRATS[si];
        let got = D96::from_raw(self_raw)
            .checked_div_rounded(D96::from_raw(rhs_raw), dp, s)
            .map(|d| d.to_raw());
        prop_assert_eq!(got, ref_div_rounded(self_raw, rhs_raw, dp, s));
    }

    // Full 96-bit range, exercising the 192-bit wide numerator path: ToZero at
    // full scale equals the truncating checked_div, and every strategy stays
    // within one full-scale ulp of it.
    #[test]
    fn d96_div_rounded_wide_consistent(
        self_raw in MIN96..=MAX96,
        rhs_raw in MIN96..=MAX96,
        si in 0usize..7,
    ) {
        prop_assume!(rhs_raw != 0);
        let a = D96::from_raw(self_raw);
        let b = D96::from_raw(rhs_raw);
        let trunc = a.checked_div(b);
        prop_assert_eq!(a.checked_div_rounded(b, 12, ToZero), trunc);
        if let (Some(r), Some(t)) = (a.checked_div_rounded(b, 12, STRATS[si]), trunc) {
            prop_assert!((r.to_raw() - t.to_raw()).abs() <= 1);
        }
    }
}

#[test]
fn default_strategy_is_banker() {
    assert_eq!(RoundingStrategy::default(), MidpointNearestEven);
}

#[test]
fn round_dp_with_strategy_all_modes_on_ties() {
    // 2.5 -> 0 dp: q=2 (even).
    let v = D64::from_str("2.5").unwrap();
    assert_eq!(
        v.round_dp_with_strategy(0, MidpointNearestEven),
        D64::from_str("2").unwrap()
    );
    assert_eq!(
        v.round_dp_with_strategy(0, MidpointAwayFromZero),
        D64::from_str("3").unwrap()
    );
    assert_eq!(
        v.round_dp_with_strategy(0, MidpointTowardZero),
        D64::from_str("2").unwrap()
    );
    assert_eq!(
        v.round_dp_with_strategy(0, ToZero),
        D64::from_str("2").unwrap()
    );
    assert_eq!(
        v.round_dp_with_strategy(0, AwayFromZero),
        D64::from_str("3").unwrap()
    );
    assert_eq!(
        v.round_dp_with_strategy(0, ToPositiveInfinity),
        D64::from_str("3").unwrap()
    );
    assert_eq!(
        v.round_dp_with_strategy(0, ToNegativeInfinity),
        D64::from_str("2").unwrap()
    );

    // 3.5 -> 0 dp: q=3 (odd), so banker's rounds up to 4.
    let w = D64::from_str("3.5").unwrap();
    assert_eq!(
        w.round_dp_with_strategy(0, MidpointNearestEven),
        D64::from_str("4").unwrap()
    );
    assert_eq!(
        w.round_dp_with_strategy(0, MidpointTowardZero),
        D64::from_str("3").unwrap()
    );

    // -2.5 -> 0 dp: floor=-3, ceil=-2, banker's -2 (even).
    let n = D64::from_str("-2.5").unwrap();
    assert_eq!(
        n.round_dp_with_strategy(0, MidpointNearestEven),
        D64::from_str("-2").unwrap()
    );
    assert_eq!(
        n.round_dp_with_strategy(0, MidpointAwayFromZero),
        D64::from_str("-3").unwrap()
    );
    assert_eq!(
        n.round_dp_with_strategy(0, ToNegativeInfinity),
        D64::from_str("-3").unwrap()
    );
    assert_eq!(
        n.round_dp_with_strategy(0, ToPositiveInfinity),
        D64::from_str("-2").unwrap()
    );
    assert_eq!(
        n.round_dp_with_strategy(0, ToZero),
        D64::from_str("-2").unwrap()
    );
}

#[test]
fn round_dp_non_tie_directional() {
    let v = D64::from_str("2.4").unwrap();
    assert_eq!(
        v.round_dp_with_strategy(0, MidpointNearestEven),
        D64::from_str("2").unwrap()
    );
    assert_eq!(
        v.round_dp_with_strategy(0, AwayFromZero),
        D64::from_str("3").unwrap()
    );
    assert_eq!(
        v.round_dp_with_strategy(0, ToPositiveInfinity),
        D64::from_str("3").unwrap()
    );
    let n = D64::from_str("-2.4").unwrap();
    assert_eq!(
        n.round_dp_with_strategy(0, ToNegativeInfinity),
        D64::from_str("-3").unwrap()
    );
    assert_eq!(
        n.round_dp_with_strategy(0, ToZero),
        D64::from_str("-2").unwrap()
    );
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
            assert_eq!(
                v.round_dp_with_strategy(dp, MidpointNearestEven),
                v.round_dp(dp)
            );
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
            assert_eq!(
                v.round_dp_with_strategy(dp, MidpointNearestEven),
                v.round_dp(dp)
            );
        }
    }
}

#[test]
fn checked_div_rounded_d64() {
    let ten = D64::from_str("10").unwrap();
    let three = D64::from_str("3").unwrap();
    // 10/3 = 3.3333...; third digit < 5 so the nearest modes agree.
    assert_eq!(
        ten.checked_div_rounded(three, 2, MidpointNearestEven),
        Some(D64::from_str("3.33").unwrap())
    );
    assert_eq!(
        ten.checked_div_rounded(three, 2, AwayFromZero),
        Some(D64::from_str("3.34").unwrap())
    );
    assert_eq!(
        ten.checked_div_rounded(three, 2, ToPositiveInfinity),
        Some(D64::from_str("3.34").unwrap())
    );

    // 1/8 = 0.125 -> exact tie at 2 dp.
    let one = D64::ONE;
    let eight = D64::from_str("8").unwrap();
    assert_eq!(
        one.checked_div_rounded(eight, 2, MidpointNearestEven),
        Some(D64::from_str("0.12").unwrap())
    );
    assert_eq!(
        one.checked_div_rounded(eight, 2, MidpointAwayFromZero),
        Some(D64::from_str("0.13").unwrap())
    );
    assert_eq!(
        one.checked_div_rounded(eight, 2, MidpointTowardZero),
        Some(D64::from_str("0.12").unwrap())
    );

    // Negative dividend / negative divisor.
    let neg_one = D64::from_str("-1").unwrap();
    assert_eq!(
        neg_one.checked_div_rounded(eight, 2, ToNegativeInfinity),
        Some(D64::from_str("-0.13").unwrap())
    );
    assert_eq!(
        neg_one.checked_div_rounded(eight, 2, ToPositiveInfinity),
        Some(D64::from_str("-0.12").unwrap())
    );
    let neg_three = D64::from_str("-3").unwrap();
    assert_eq!(
        ten.checked_div_rounded(neg_three, 2, ToZero),
        Some(D64::from_str("-3.33").unwrap())
    );
    assert_eq!(
        ten.checked_div_rounded(neg_three, 2, AwayFromZero),
        Some(D64::from_str("-3.34").unwrap())
    );

    // At full scale with ToZero it matches the truncating checked_div.
    assert_eq!(
        ten.checked_div_rounded(three, 8, ToZero),
        ten.checked_div(three)
    );

    // Errors: divide by zero, dp out of range.
    assert_eq!(
        ten.checked_div_rounded(D64::ZERO, 2, MidpointNearestEven),
        None
    );
    assert_eq!(ten.checked_div_rounded(three, 9, MidpointNearestEven), None);
}

#[test]
fn checked_div_rounded_d96() {
    let ten = D96::from_str("10").unwrap();
    let three = D96::from_str("3").unwrap();
    assert_eq!(
        ten.checked_div_rounded(three, 4, MidpointNearestEven),
        Some(D96::from_str("3.3333").unwrap())
    );
    assert_eq!(
        ten.checked_div_rounded(three, 4, AwayFromZero),
        Some(D96::from_str("3.3334").unwrap())
    );

    let one = D96::ONE;
    let eight = D96::from_str("8").unwrap();
    assert_eq!(
        one.checked_div_rounded(eight, 2, MidpointNearestEven),
        Some(D96::from_str("0.12").unwrap())
    );
    assert_eq!(
        one.checked_div_rounded(eight, 2, MidpointAwayFromZero),
        Some(D96::from_str("0.13").unwrap())
    );

    assert_eq!(
        ten.checked_div_rounded(three, 12, ToZero),
        ten.checked_div(three)
    );
    assert_eq!(ten.checked_div_rounded(D96::ZERO, 2, ToZero), None);

    // No range cliff: |self| near MAX at full dp is computed exactly via the
    // 192-bit numerator path (this previously returned None). ToZero at dp==12
    // matches the truncating checked_div; the tie rounds up by exactly one ulp.
    let two = D96::from_str("2").unwrap();
    assert_eq!(
        D96::MAX.checked_div_rounded(two, 12, ToZero),
        D96::MAX.checked_div(two)
    );
    let trunc = D96::MAX.checked_div(two).unwrap();
    assert_eq!(
        D96::MAX.checked_div_rounded(two, 12, MidpointNearestEven),
        Some(D96::from_raw(trunc.to_raw() + 1)) // MAX is odd, MAX/2 is an exact .5 tie -> even (up)
    );
    assert_eq!(
        D96::MAX.checked_div_rounded(two, 12, ToPositiveInfinity),
        Some(D96::from_raw(trunc.to_raw() + 1))
    );
    // A high-dp division on a large value that the i128 path used to reject.
    assert!(
        D96::MAX
            .checked_div_rounded(three, 12, MidpointNearestEven)
            .is_some()
    );
}
