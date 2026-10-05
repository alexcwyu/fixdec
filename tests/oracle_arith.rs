//! Differential tests of `D64` and `D96` arithmetic, rounding and integer-power
//! operations against an arbitrary-precision oracle (`tests/common`).
//!
//! The existing suites check hand-picked values and proptest streams whose operand
//! magnitudes are uniform in bit length; the seams of the wide `D96` arithmetic
//! (the `2^64` fast-path guard in `checked_mul`, the 192-bit division carries, the
//! `|MIN| = MAX + 1` asymmetry, the `u128::MAX / SCALE` guard in `checked_div`) sit
//! at specific magnitudes that a uniform stream almost never hits. This file
//! generates those magnitudes on purpose: products and quotients that land next to
//! `MAX`, `|MIN|`, `2^64` and `2^128`; powers of ten and two and their neighbours;
//! exact rounding ties.
//!
//! Scale the sampling with `FIXDEC_ORACLE_SCALE=10 cargo test --release --test oracle_arith`.
mod common;
use common::*;

use fixdec::{DecimalError, RoundingStrategy};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{Signed, Zero};

fn scale() -> usize {
    std::env::var("FIXDEC_ORACLE_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

// ---------------------------------------------------------------------------
// The specification (BigInt, from the documented definitions)
// ---------------------------------------------------------------------------

fn wrap<T: Dec>(x: &BigInt) -> BigInt {
    let m = BigInt::from(1u8) << T::BITS;
    (x - T::lo()).mod_floor(&m) + T::lo()
}
fn clamp<T: Dec>(x: &BigInt) -> BigInt {
    x.clone().max(T::lo()).min(T::hi())
}
fn fit<T: Dec>(x: &BigInt) -> Option<T> {
    T::from_raw_big(x)
}
fn val<T: Dec>(x: &BigInt) -> T {
    T::from_raw_big(x).expect("oracle produced an out-of-range value")
}
fn zero() -> BigInt {
    BigInt::zero()
}
fn mul_q<T: Dec>(a: &BigInt, b: &BigInt) -> BigInt {
    (a * b) / T::scale_big() // truncates toward zero
}
fn div_q<T: Dec>(a: &BigInt, b: &BigInt) -> BigInt {
    (a * T::scale_big()) / b
}
/// The raw `i64` remainder cannot represent `MIN % -1`; `i128` can (D96 has room).
fn rem_overflows<T: Dec>(a: &BigInt, b: &BigInt) -> bool {
    T::BITS == 64 && *a == T::lo() && *b == BigInt::from(-1)
}

// ---------------------------------------------------------------------------
// The checks
// ---------------------------------------------------------------------------

type R<T> = Result<T, DecimalError>;
fn ok_or<T: Dec>(x: &BigInt, e: DecimalError) -> R<T> {
    fit::<T>(x).ok_or(e).map(|v| v)
}

fn check_binary<T: Dec>(f: &mut Failures, a: T, b: T) {
    let (x, y) = (a.raw_big(), b.raw_big());
    let name = T::NAME;
    let at = || format!("{} , {}", x, y);

    macro_rules! chk {
        ($kind:expr, $got:expr, $want:expr) => {
            f.check(|| format!("{name} {}", $kind), at, guarded(|| $got), $want)
        };
    }

    // ---- add / sub
    let (sum, dif) = (&x + &y, &x - &y);
    chk!("checked_add", a.checked_add(b), fit::<T>(&sum));
    chk!("checked_sub", a.checked_sub(b), fit::<T>(&dif));
    chk!(
        "saturating_add",
        a.saturating_add(b),
        val::<T>(&clamp::<T>(&sum))
    );
    chk!(
        "saturating_sub",
        a.saturating_sub(b),
        val::<T>(&clamp::<T>(&dif))
    );
    chk!(
        "wrapping_add",
        a.wrapping_add(b),
        val::<T>(&wrap::<T>(&sum))
    );
    chk!(
        "wrapping_sub",
        a.wrapping_sub(b),
        val::<T>(&wrap::<T>(&dif))
    );
    chk!(
        "overflowing_add",
        a.overflowing_add(b),
        (val::<T>(&wrap::<T>(&sum)), fit::<T>(&sum).is_none())
    );
    chk!(
        "overflowing_sub",
        a.overflowing_sub(b),
        (val::<T>(&wrap::<T>(&dif)), fit::<T>(&dif).is_none())
    );
    chk!(
        "try_add",
        a.try_add(b),
        ok_or::<T>(&sum, DecimalError::Overflow)
    );
    chk!(
        "try_sub",
        a.try_sub(b),
        ok_or::<T>(&dif, DecimalError::Overflow)
    );

    // ---- mul
    let q = mul_q::<T>(&x, &y);
    chk!("checked_mul", a.checked_mul(b), fit::<T>(&q));
    chk!(
        "saturating_mul",
        a.saturating_mul(b),
        val::<T>(&clamp::<T>(&q))
    );
    chk!("wrapping_mul", a.wrapping_mul(b), val::<T>(&wrap::<T>(&q)));
    chk!(
        "overflowing_mul",
        a.overflowing_mul(b),
        (val::<T>(&wrap::<T>(&q)), fit::<T>(&q).is_none())
    );
    chk!(
        "try_mul",
        a.try_mul(b),
        ok_or::<T>(&q, DecimalError::Overflow)
    );

    // ---- div
    if y.is_zero() {
        chk!("checked_div by 0", a.checked_div(b), None);
        chk!(
            "saturating_div by 0",
            a.saturating_div(b),
            val::<T>(&zero())
        );
        chk!("wrapping_div by 0", a.wrapping_div(b), val::<T>(&zero()));
        chk!(
            "try_div by 0",
            a.try_div(b),
            Err(DecimalError::DivisionByZero)
        );
        chk!("checked_rem by 0", a.checked_rem(b), None);
        chk!(
            "try_rem by 0",
            a.try_rem(b),
            Err(DecimalError::DivisionByZero)
        );
        chk!("is_multiple_of 0", a.is_multiple_of(b), x.is_zero());
        chk!("div_rem by 0", a.div_rem_big(b), None);
    } else {
        let q = div_q::<T>(&x, &y);
        chk!("checked_div", a.checked_div(b), fit::<T>(&q));
        chk!(
            "saturating_div",
            a.saturating_div(b),
            val::<T>(&clamp::<T>(&q))
        );
        chk!("wrapping_div", a.wrapping_div(b), val::<T>(&wrap::<T>(&q)));
        chk!(
            "try_div",
            a.try_div(b),
            ok_or::<T>(&q, DecimalError::Overflow)
        );

        // ---- rem family (the raw remainder; sign follows the dividend)
        let rem = &x % &y;
        let want_rem = if rem_overflows::<T>(&x, &y) {
            None
        } else {
            fit::<T>(&rem)
        };
        chk!("checked_rem", a.checked_rem(b), want_rem);
        let want_try = if rem_overflows::<T>(&x, &y) {
            Err(DecimalError::Overflow)
        } else {
            ok_or::<T>(&rem, DecimalError::Overflow)
        };
        chk!("try_rem", a.try_rem(b), want_try);
        chk!("is_multiple_of", a.is_multiple_of(b), rem.is_zero());
        let qi = &x / &y;
        let want_dr = if rem_overflows::<T>(&x, &y) {
            None
        } else {
            Some((qi, val::<T>(&rem)))
        };
        chk!("div_rem", a.div_rem_big(b), want_dr);
    }
}

/// `mul_add` = `trunc(a * b / S) + c`, where only the FINAL value has to fit: the product may be
/// out of range on its own if `c` brings it back.
fn check_ternary<T: Dec>(f: &mut Failures, a: T, b: T, c: T) {
    let (x, y, z) = (a.raw_big(), b.raw_big(), c.raw_big());
    let want = fit::<T>(&(mul_q::<T>(&x, &y) + &z));
    f.check(
        || format!("{} mul_add", T::NAME),
        || format!("{x} * {y} + {z}"),
        guarded(|| a.mul_add(b, c)),
        want,
    );
}

fn check_unary<T: Dec>(f: &mut Failures, a: T) {
    let x = a.raw_big();
    let name = T::NAME;
    let at = || format!("{x}");
    macro_rules! chk {
        ($kind:expr, $got:expr, $want:expr) => {
            f.check(|| format!("{name} {}", $kind), at, guarded(|| $got), $want)
        };
    }
    // ---- neg / abs
    let neg = -&x;
    let abs = x.abs();
    chk!("checked_neg", a.checked_neg(), fit::<T>(&neg));
    chk!(
        "saturating_neg",
        a.saturating_neg(),
        val::<T>(&clamp::<T>(&neg))
    );
    chk!("wrapping_neg", a.wrapping_neg(), val::<T>(&wrap::<T>(&neg)));
    chk!(
        "try_neg",
        a.try_neg(),
        ok_or::<T>(&neg, DecimalError::Overflow)
    );
    chk!("abs", a.abs(), val::<T>(&clamp::<T>(&abs))); // documented: saturates at MIN
    chk!("checked_abs", a.checked_abs(), fit::<T>(&abs));
    chk!(
        "saturating_abs",
        a.saturating_abs(),
        val::<T>(&clamp::<T>(&abs))
    );
    chk!("wrapping_abs", a.wrapping_abs(), val::<T>(&wrap::<T>(&abs)));
    chk!(
        "try_abs",
        a.try_abs(),
        ok_or::<T>(&abs, DecimalError::Overflow)
    );
    chk!(
        "signum",
        a.signum(),
        if x.is_zero() {
            0
        } else if x.is_positive() {
            1
        } else {
            -1
        }
    );

    // ---- sqrt: floor(sqrt(raw * S)), none for negatives
    let want_sqrt = if x.is_negative() {
        None
    } else {
        Some(val::<T>(&(&x * T::scale_big()).sqrt()))
    };
    chk!("sqrt", a.sqrt(), want_sqrt.clone());
    chk!(
        "try_sqrt",
        a.try_sqrt(),
        want_sqrt.ok_or(DecimalError::NegativeValue)
    );

    // ---- recip = 1 / a on the grid
    let want_recip = if x.is_zero() {
        None
    } else {
        fit::<T>(&div_q::<T>(&T::scale_big(), &x))
    };
    chk!("recip", a.recip(), want_recip);

    // ---- integer-part and rounding family
    let s = T::scale_big();
    let rounded = |strategy, dp: u32| -> BigInt {
        let factor = pow10(T::DECIMALS - dp);
        round_div(&x, &factor, strategy) * factor
    };
    chk!(
        "floor",
        a.floor(),
        val::<T>(&clamp::<T>(&rounded(
            RoundingStrategy::ToNegativeInfinity,
            0
        )))
    );
    chk!(
        "ceil",
        a.ceil(),
        val::<T>(&clamp::<T>(&rounded(
            RoundingStrategy::ToPositiveInfinity,
            0
        )))
    );
    chk!(
        "trunc",
        a.trunc(),
        val::<T>(&rounded(RoundingStrategy::ToZero, 0))
    );
    chk!(
        "fract",
        a.fract(),
        val::<T>(&(&x - rounded(RoundingStrategy::ToZero, 0)))
    );
    chk!(
        "round",
        a.round(),
        val::<T>(&clamp::<T>(&rounded(
            RoundingStrategy::MidpointNearestEven,
            0
        )))
    );
    for dp in 0..=T::DECIMALS {
        chk!(
            format!("round_dp({dp})"),
            a.round_dp(dp as u8),
            val::<T>(&clamp::<T>(&rounded(
                RoundingStrategy::MidpointNearestEven,
                dp
            )))
        );
        for strategy in STRATEGIES {
            chk!(
                format!("round_dp_with_strategy({dp}, {strategy:?})"),
                a.round_dp_with_strategy(dp as u8, strategy),
                val::<T>(&clamp::<T>(&rounded(strategy, dp)))
            );
        }
    }
    let _ = s;
}

/// `checked_div_rounded` and the tick quantizers: the rounding is applied to the exact quotient.
fn check_rounded_ops<T: Dec>(f: &mut Failures, a: T, b: T) {
    let (x, y) = (a.raw_big(), b.raw_big());
    let name = T::NAME;
    for strategy in STRATEGIES {
        for dp in 0..=T::DECIMALS {
            let want = if y.is_zero() {
                None
            } else {
                // a/b at dp decimals: round(a * 10^dp / b) * 10^(DECIMALS - dp)
                let n = round_div(&(&x * pow10(dp)), &y, strategy) * pow10(T::DECIMALS - dp);
                fit::<T>(&n)
            };
            f.check(
                || format!("{name} checked_div_rounded dp={dp} {strategy:?}"),
                || format!("{x} / {y}"),
                guarded(|| a.checked_div_rounded(b, dp as u8, strategy)),
                want,
            );
        }
        // beyond the native precision: None
        f.check(
            || format!("{name} checked_div_rounded dp>DECIMALS"),
            || format!("{x} / {y}"),
            guarded(|| a.checked_div_rounded(b, T::DECIMALS as u8 + 1, strategy)),
            None,
        );

        // quantize to a tick `b` (must be positive)
        let want = if y.is_positive() {
            fit::<T>(&(round_div(&x, &y, strategy) * &y))
        } else {
            None
        };
        f.check(
            || format!("{name} checked_quantize {strategy:?}"),
            || format!("{x} to tick {y}"),
            guarded(|| a.checked_quantize(b, strategy)),
            want,
        );
    }
    let want_floor = if y.is_positive() {
        fit::<T>(&(round_div(&x, &y, RoundingStrategy::ToNegativeInfinity) * &y))
    } else {
        None
    };
    let want_ceil = if y.is_positive() {
        fit::<T>(&(round_div(&x, &y, RoundingStrategy::ToPositiveInfinity) * &y))
    } else {
        None
    };
    f.check(
        || format!("{name} checked_floor_to_tick"),
        || format!("{x} to tick {y}"),
        guarded(|| a.checked_floor_to_tick(b)),
        want_floor,
    );
    f.check(
        || format!("{name} checked_ceil_to_tick"),
        || format!("{x} to tick {y}"),
        guarded(|| a.checked_ceil_to_tick(b)),
        want_ceil,
    );
}

fn check_int_ops<T: Dec>(f: &mut Failures, a: T, n: i64) {
    let (x, nn) = (a.raw_big(), BigInt::from(n));
    let name = T::NAME;
    let at = || format!("{x} with {n}");
    let s = T::scale_big();
    let prod = &x * &nn;
    f.check(
        || format!("{name} mul_int"),
        at,
        guarded(|| a.mul_int(n)),
        fit::<T>(&prod),
    );
    f.check(
        || format!("{name} try_mul_int"),
        at,
        guarded(|| a.try_mul_int(n)),
        ok_or::<T>(&prod, DecimalError::Overflow),
    );
    // add/sub of WHOLE units: the integer must itself be representable as a decimal
    let unit = fit::<T>(&(&nn * &s));
    let want_add = unit.and_then(|u| fit::<T>(&(&x + u.raw_big())));
    let want_sub = unit.and_then(|u| fit::<T>(&(&x - u.raw_big())));
    f.check(
        || format!("{name} add_int"),
        at,
        guarded(|| a.add_int(n)),
        want_add,
    );
    f.check(
        || format!("{name} sub_int"),
        at,
        guarded(|| a.sub_int(n)),
        want_sub,
    );
    // divide by an integer: truncates toward zero
    let want_div = if n == 0 { None } else { fit::<T>(&(&x / &nn)) };
    f.check(
        || format!("{name} div_int"),
        at,
        guarded(|| a.div_int(n)),
        want_div,
    );
    let want_try = if n == 0 {
        Err(DecimalError::DivisionByZero)
    } else {
        ok_or::<T>(&(&x / &nn), DecimalError::Overflow)
    };
    f.check(
        || format!("{name} try_div_int"),
        at,
        guarded(|| a.try_div_int(n)),
        want_try,
    );
}

fn ints() -> Vec<i64> {
    let mut v = vec![
        0,
        1,
        -1,
        2,
        -2,
        3,
        7,
        10,
        -10,
        100,
        1_000,
        1_000_000,
        100_000_000,
        10_000_000_000,
        i64::MAX,
        i64::MIN,
        i64::MAX / 2,
        i64::MIN / 2,
        i64::MAX - 1,
        i64::MIN + 1,
        1 << 31,
        1 << 32,
        (1 << 32) + 1,
        1 << 62,
        92_233_720_368,
        92_233_720_369,
        -92_233_720_368,
        -92_233_720_369,
    ];
    v.sort();
    v.dedup();
    v
}

fn run<T: Dec>() {
    let mut f = Failures::default();
    let k = scale();
    let all = interesting::<T>();
    let mut rng = Rng(0x00F1_7DEC ^ T::BITS as u64);

    // 1. A deterministic grid over the boundary values.
    let stride = (all.len() / (45 * k)).max(1);
    let core: Vec<T> = all.iter().step_by(stride).copied().collect();
    for &a in &core {
        check_unary(&mut f, a);
        for &b in &core {
            check_binary(&mut f, a, b);
        }
    }
    // 2. Every boundary value (unary), and against a rotating sample of the others.
    for (i, &a) in all.iter().enumerate() {
        check_unary(&mut f, a);
        for j in 0..(6 * k) {
            let b = all[(i * 7 + j * 13 + 1) % all.len()];
            check_binary(&mut f, a, b);
        }
    }
    // 3. Products and quotients that land on the seams.
    for (a, b) in mul_edge_pairs::<T>(&mut rng, 2 * k)
        .into_iter()
        .chain(div_edge_pairs::<T>(&mut rng, 2 * k))
    {
        check_binary(&mut f, a, b);
    }
    for (a, b) in limit_pairs::<T>() {
        check_binary(&mut f, a, b);
        check_rounded_ops(&mut f, a, b);
    }
    // 4. Uniform-bit-length random pairs.
    for _ in 0..(1500 * k) {
        let (a, b): (T, T) = (rng.val(), rng.val());
        check_binary(&mut f, a, b);
        check_unary(&mut f, a);
    }
    // 5. mul_add triples.
    for _ in 0..(2000 * k) {
        let (a, b, c): (T, T, T) = (rng.val(), rng.val(), rng.val());
        check_ternary(&mut f, a, b, c);
    }
    for (i, &a) in core.iter().enumerate() {
        for (j, &b) in core.iter().enumerate().step_by(3) {
            check_ternary(&mut f, a, b, core[(i + j) % core.len()]);
        }
    }
    // 6. Rounded division and ticks: boundary values, and ties.
    let rstride = (all.len() / (25 * k)).max(1);
    let rcore: Vec<T> = all.iter().step_by(rstride).copied().collect();
    for &a in &rcore {
        for &b in &rcore {
            check_rounded_ops(&mut f, a, b);
        }
    }
    for _ in 0..(300 * k) {
        let (a, b): (T, T) = (rng.val(), rng.val());
        check_rounded_ops(&mut f, a, b);
        // an exact tie: a = 5 * 10^(i-1) * (odd) over b = 10^i-ish
        let i = 1 + rng.below(T::DECIMALS as u64 - 1) as u32;
        let tie_a = pow10(i - 1) * 5 * BigInt::from(2 * rng.below(9) + 1);
        if let (Some(ta), Some(tb)) = (T::from_raw_big(&tie_a), T::from_raw_big(&pow10(i))) {
            check_rounded_ops(&mut f, ta, tb);
        }
    }
    // 7. Integer operands.
    for &a in &core {
        for n in ints() {
            check_int_ops(&mut f, a, n);
        }
    }
    f.finish(&format!("oracle {}", T::NAME));
}

#[test]
fn oracle_d64() {
    run::<fixdec::D64>();
}
#[test]
fn oracle_d96() {
    run::<fixdec::D96>();
}

/// Regression: `mul_add` used to return `None` as soon as `self * mul` alone left the type's
/// range, even when `+ add` brought the result back -- `D64`: `191444752138 * 4817772194772020 +
/// i64::MIN` (raw), whose exact value is `475`. Products on the seams, against `add` values
/// chosen to cancel them.
fn mul_add_final_value_only<T: Dec>() {
    let mut f = Failures::default();
    let mut rng = Rng(0xADD ^ T::BITS as u64);
    let adds: Vec<T> = [
        T::lo(),
        T::hi(),
        -T::hi() / BigInt::from(2),
        T::hi() / BigInt::from(2),
        BigInt::from(-1),
        BigInt::from(1),
        T::lo() + 1,
    ]
    .iter()
    .filter_map(|v| T::from_raw_big(v))
    .collect();
    for (a, b) in mul_edge_pairs::<T>(&mut rng, 4) {
        for &c in &adds {
            check_ternary(&mut f, a, b, c);
        }
    }
    // the one that was reported
    if T::BITS == 64 {
        check_ternary(
            &mut f,
            T::from_raw_big(&BigInt::from(191444752138i64)).unwrap(),
            T::from_raw_big(&BigInt::from(4817772194772020i64)).unwrap(),
            T::from_raw_big(&T::lo()).unwrap(),
        );
    }
    f.finish(&format!("mul_add final value {}", T::NAME));
}
#[test]
fn mul_add_final_value_only_d64() {
    mul_add_final_value_only::<fixdec::D64>();
}
#[test]
fn mul_add_final_value_only_d96() {
    mul_add_final_value_only::<fixdec::D96>();
}
