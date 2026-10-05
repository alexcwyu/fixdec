#![cfg(feature = "num-traits")]
//! `num-traits` impls of `D64` / `D96` against an arbitrary-precision oracle.
//!
//! `tests/num_traits_impls.rs` pins a handful of hand-picked values. This file runs the
//! trait surface over the boundary values the way `oracle_arith.rs` runs the inherent API,
//! and checks the *default* trait methods (`from_i128`, `to_u128`, `from_f32`, ...) that
//! the impls inherit instead of overriding: those route through `i64` / `u64` / `f64`, so
//! they are only right if every representable value fits that detour.
//!
//! Run with: `cargo test --features num-traits --test oracle_num_traits`
mod common;
use common::*;

use core::str::FromStr;
use fixdec::{D64, D96, DecimalError};
use num_bigint::BigInt;
use num_traits::{
    Bounded, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, FromPrimitive, Inv, Num, One,
    Saturating, Signed, ToPrimitive, Zero,
};

trait Nt:
    Dec
    + Copy
    + FromPrimitive
    + ToPrimitive
    + Num<FromStrRadixErr = DecimalError>
    + Signed
    + Bounded
    + CheckedAdd
    + CheckedSub
    + CheckedMul
    + CheckedDiv
    + Saturating
    + Inv<Output = Self>
    + FromStr
    + Ord
{
}
impl Nt for D64 {}
impl Nt for D96 {}

fn trunc_div(raw: &BigInt, t: &BigInt) -> BigInt {
    raw / t // BigInt division truncates toward zero
}

/// `f64` within one unit in the last place of `want` (the documented accuracy of `to_f64`).
fn within_one_ulp(got: f64, want: f64) -> bool {
    if want == 0.0 || got == 0.0 {
        return got == want;
    }
    got.is_sign_positive() == want.is_sign_positive() && got.to_bits().abs_diff(want.to_bits()) <= 1
}

fn run<T: Nt>() {
    let mut f = Failures::default();
    let name = T::NAME;
    let s = T::scale_big();

    // ---- ToPrimitive: truncation toward zero; u* refuse values <= -1; f64 within 1 ulp
    for a in interesting::<T>() {
        let raw = a.raw_big();
        let at = || raw.to_string();
        let q = trunc_div(&raw, &s);
        let int_fits = |lo: &BigInt, hi: &BigInt| (&q >= lo && &q <= hi).then(|| q.clone());
        let (i64lo, i64hi) = (BigInt::from(i64::MIN), BigInt::from(i64::MAX));
        let (i128lo, i128hi) = (BigInt::from(i128::MIN), BigInt::from(i128::MAX));
        let zero = BigInt::from(0);
        let (u64hi, u128hi) = (BigInt::from(u64::MAX), BigInt::from(u128::MAX));
        macro_rules! prim {
            ($kind:expr, $got:expr, $want:expr) => {
                f.check(|| format!("{name} {}", $kind), at, Ok($got), $want)
            };
        }
        prim!(
            "to_i64",
            ToPrimitive::to_i64(&a).map(BigInt::from),
            int_fits(&i64lo, &i64hi)
        );
        prim!(
            "to_u64",
            ToPrimitive::to_u64(&a).map(BigInt::from),
            int_fits(&zero, &u64hi)
        );
        // the defaults: they detour through i64 / u64, which holds every in-range value
        prim!(
            "to_i128",
            ToPrimitive::to_i128(&a).map(BigInt::from),
            int_fits(&i128lo, &i128hi)
        );
        prim!(
            "to_u128",
            ToPrimitive::to_u128(&a).map(BigInt::from),
            int_fits(&zero, &u128hi)
        );
        prim!(
            "to_i32",
            ToPrimitive::to_i32(&a).map(BigInt::from),
            int_fits(&BigInt::from(i32::MIN), &BigInt::from(i32::MAX))
        );
        prim!(
            "to_u32",
            ToPrimitive::to_u32(&a).map(BigInt::from),
            int_fits(&zero, &BigInt::from(u32::MAX))
        );
        prim!(
            "to_i8",
            ToPrimitive::to_i8(&a).map(BigInt::from),
            int_fits(&BigInt::from(i8::MIN), &BigInt::from(i8::MAX))
        );
        prim!(
            "to_usize",
            ToPrimitive::to_usize(&a).map(BigInt::from),
            int_fits(&zero, &BigInt::from(usize::MAX))
        );
        let want = a.to_string().parse::<f64>().unwrap();
        f.check(
            || format!("{name} to_f64"),
            at,
            Ok(within_one_ulp(ToPrimitive::to_f64(&a).unwrap(), want)),
            true,
        );
        f.check(
            || format!("{name} to_f64 == inherent"),
            at,
            Ok(ToPrimitive::to_f64(&a)),
            Some(Dec::to_f64(a)),
        );
        let g32 = ToPrimitive::to_f32(&a).unwrap();
        f.check(
            || format!("{name} to_f32 = to_f64 as f32"),
            at,
            Ok(g32),
            ToPrimitive::to_f64(&a).unwrap() as f32,
        );
    }

    // ---- FromPrimitive: exact integers or None, including the i128/u128 defaults
    let in_range = |n: &BigInt| T::from_raw_big(&(n * &s));
    let mut ints: Vec<BigInt> = vec![
        0,
        1,
        -1,
        2,
        10,
        -10,
        127,
        128,
        -128,
        255,
        256,
        65_535,
        65_536,
        i32::MAX as i64,
        i32::MIN as i64,
    ]
    .into_iter()
    .map(BigInt::from)
    .collect();
    for edge in [
        &T::hi() / &s,
        &T::lo() / &s,
        BigInt::from(i64::MAX),
        BigInt::from(i64::MIN),
        BigInt::from(u64::MAX),
        BigInt::from(i128::MAX),
        BigInt::from(i128::MIN),
        BigInt::from(u128::MAX),
    ] {
        for d in -2..=2 {
            ints.push(&edge + d);
        }
    }
    for n in &ints {
        let at = || n.to_string();
        let want = in_range(n);
        if let Some(v) = n.to_i64() {
            f.check(
                || format!("{name} from_i64"),
                at,
                Ok(<T as FromPrimitive>::from_i64(v)),
                want,
            );
        }
        if let Some(v) = n.to_u64() {
            f.check(
                || format!("{name} from_u64"),
                at,
                Ok(<T as FromPrimitive>::from_u64(v)),
                want,
            );
        }
        if let Some(v) = n.to_i128() {
            f.check(
                || format!("{name} from_i128 (default)"),
                at,
                Ok(<T as FromPrimitive>::from_i128(v)),
                want,
            );
        }
        if let Some(v) = n.to_u128() {
            f.check(
                || format!("{name} from_u128 (default)"),
                at,
                Ok(<T as FromPrimitive>::from_u128(v)),
                want,
            );
        }
        if let Some(v) = n.to_i32() {
            f.check(
                || format!("{name} from_i32 (default)"),
                at,
                Ok(<T as FromPrimitive>::from_i32(v)),
                want,
            );
        }
        if let Some(v) = n.to_u8() {
            f.check(
                || format!("{name} from_u8 (default)"),
                at,
                Ok(<T as FromPrimitive>::from_u8(v)),
                want,
            );
        }
    }
    for x in [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        1e-9,
        5e-9,
        123.456,
        9.2e10,
        9.3e10,
        3.9e16,
        4e16,
        1e30,
        -1e30,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MAX,
        f64::MIN_POSITIVE,
    ] {
        f.check(
            || format!("{name} from_f64"),
            || format!("{x:e}"),
            Ok(<T as FromPrimitive>::from_f64(x)),
            Dec::from_f64(x),
        );
        f.check(
            || format!("{name} from_f32 (default)"),
            || format!("{x:e}"),
            Ok(<T as FromPrimitive>::from_f32(x as f32)),
            Dec::from_f64(x as f32 as f64),
        );
    }

    // ---- Zero / One / Bounded / Num / checked / saturating / Inv, against the inherent API
    f.check(
        || format!("{name} Zero"),
        || String::new(),
        Ok(<T as Zero>::zero().raw_big()),
        BigInt::from(0),
    );
    f.check(
        || format!("{name} One"),
        || String::new(),
        Ok(<T as One>::one().raw_big()),
        s.clone(),
    );
    f.check(
        || format!("{name} Bounded"),
        || String::new(),
        Ok((T::min_value().raw_big(), T::max_value().raw_big())),
        (T::lo(), T::hi()),
    );
    let all = interesting::<T>();
    let stride = (all.len() / 40).max(1);
    for &a in all.iter().step_by(stride) {
        let at = || a.raw_big().to_string();
        f.check(
            || format!("{name} is_zero"),
            at,
            Ok(Zero::is_zero(&a)),
            a.raw_big() == BigInt::from(0),
        );
        f.check(
            || format!("{name} is_one"),
            at,
            Ok(One::is_one(&a)),
            a.raw_big() == s,
        );
        // Num::from_str_radix(10) is from_str_exact; every other radix is rejected outright
        let text = a.to_string();
        f.check(
            || format!("{name} from_str_radix 10"),
            at,
            Ok(<T as Num>::from_str_radix(&text, 10)),
            T::from_str_exact(&text),
        );
        f.check(
            || format!("{name} FromStr == from_str_exact"),
            at,
            Ok(T::from_str_trait(&text)),
            T::from_str_exact(&text),
        );
        for radix in [2, 8, 16, 36] {
            f.check(
                || format!("{name} from_str_radix {radix}"),
                at,
                Ok(<T as Num>::from_str_radix("1", radix)),
                Err(DecimalError::InvalidFormat),
            );
        }
        f.check(
            || format!("{name} Inv"),
            at,
            Ok(guarded(|| Inv::inv(a)).ok()),
            a.recip(),
        );
        for &b in all.iter().step_by(stride * 3) {
            let at2 = || format!("{} , {}", a.raw_big(), b.raw_big());
            f.check(
                || format!("{name} CheckedAdd"),
                at2,
                Ok(CheckedAdd::checked_add(&a, &b)),
                Dec::checked_add(a, b),
            );
            f.check(
                || format!("{name} CheckedSub"),
                at2,
                Ok(CheckedSub::checked_sub(&a, &b)),
                Dec::checked_sub(a, b),
            );
            f.check(
                || format!("{name} CheckedMul"),
                at2,
                Ok(CheckedMul::checked_mul(&a, &b)),
                Dec::checked_mul(a, b),
            );
            f.check(
                || format!("{name} CheckedDiv"),
                at2,
                Ok(CheckedDiv::checked_div(&a, &b)),
                Dec::checked_div(a, b),
            );
            f.check(
                || format!("{name} Saturating add"),
                at2,
                Ok(Saturating::saturating_add(a, b)),
                Dec::saturating_add(a, b),
            );
            f.check(
                || format!("{name} Saturating sub"),
                at2,
                Ok(Saturating::saturating_sub(a, b)),
                Dec::saturating_sub(a, b),
            );
        }
    }
    f.finish(&format!("num-traits {name}"));
}

#[test]
fn num_traits_d64() {
    run::<D64>();
}
#[test]
fn num_traits_d96() {
    run::<D96>();
}

/// Numeric-generic code that knows nothing about this crate: Horner evaluation, a mean
/// and a power through the trait surface alone must agree with the inherent API.
fn horner<T: Num + Copy>(coeffs: &[T], x: T) -> T {
    coeffs.iter().rev().fold(T::zero(), |acc, &c| acc * x + c)
}
fn mean<T: Num + Copy + FromPrimitive>(xs: &[T]) -> T {
    let sum = xs.iter().fold(T::zero(), |a, &b| a + b);
    sum / T::from_usize(xs.len()).unwrap()
}
#[test]
fn generic_numeric_code_works() {
    let c = [D64::from_i32(1), D64::from_i32(-2), D64::from_i32(3)]; // 1 - 2x + 3x^2
    assert_eq!(
        horner(&c, D64::from_str("0.5").unwrap()),
        D64::from_str("0.75").unwrap()
    );
    let c = [D96::from_i32(1), D96::from_i32(-2), D96::from_i32(3)];
    assert_eq!(
        horner(&c, D96::from_str("0.5").unwrap()),
        D96::from_str("0.75").unwrap()
    );
    assert_eq!(
        mean(&[D64::from_i32(1), D64::from_i32(2), D64::from_i32(4)]),
        D64::from_str("2.33333333").unwrap()
    );
    assert_eq!(
        mean(&[D96::from_i32(1), D96::from_i32(2), D96::from_i32(4)]),
        D96::from_str("2.333333333333").unwrap()
    );
}

/// `Signed::abs_sub` is `self - other` when `self > other`, so a spread wider than the
/// type can hold panics instead of saturating (as `i32::abs_sub` does in debug builds).
#[test]
fn abs_sub_of_a_spread_wider_than_the_type_panics() {
    assert!(guarded(|| Signed::abs_sub(&D64::MAX, &D64::MIN)).is_err());
    assert!(guarded(|| Signed::abs_sub(&D96::MAX, &D96::MIN)).is_err());
    assert_eq!(Signed::abs_sub(&D64::MIN, &D64::MAX), D64::ZERO);
    assert_eq!(Signed::abs_sub(&D64::MAX, &D64::ZERO), D64::MAX);
}

/// `abs(MIN)` is unrepresentable; both the inherent and the trait method document that it
/// saturates to `MAX` (no panic, no wrap to a negative). `checked_abs` is how to detect it.
#[test]
fn signed_abs_of_min_saturates() {
    assert_eq!(Signed::abs(&D64::MIN), D64::MAX);
    assert_eq!(Signed::abs(&D96::MIN), D96::MAX);
    assert_eq!(D64::MIN.abs(), D64::MAX);
    assert_eq!(D64::MIN.checked_abs(), None);
    assert_eq!(D96::MIN.checked_abs(), None);
    assert_eq!(
        Signed::abs(&D64::MIN.checked_add(D64::from_raw(1)).unwrap()),
        D64::MAX
    );
}
