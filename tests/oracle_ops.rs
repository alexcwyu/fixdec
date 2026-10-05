//! The operator and std-trait surface of `D64` / `D96` -- `+ - * / % -x`, the assign
//! forms, `Sum` / `Product`, `Ord` / `Hash`, `Default`, `From` / `TryFrom`, `Debug` --
//! and `powi`, checked against the *checked* API (itself verified against an
//! arbitrary-precision oracle in `oracle_arith.rs`) and against a BigInt model.
//!
//! Contract under test: an operator returns exactly what the matching `checked_*`
//! returns, and PANICS (in every profile, because `checked_*().expect(..)` is not a
//! `debug_assert`) precisely when the checked form returns `None`.
//!
//! Scale the sampling with `FIXDEC_ORACLE_SCALE=10 cargo test --release --test oracle_ops`.
mod common;
use common::*;

use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use core::iter::{Product, Sum};
use core::ops::{
    Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign, Sub, SubAssign,
};
use fixdec::{D64, D96, DecimalError};
use num_bigint::BigInt;
#[cfg(feature = "num-traits")]
use num_traits::Signed;
use num_traits::Zero;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;

fn scale() -> usize {
    std::env::var("FIXDEC_ORACLE_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

trait Ops:
    Dec
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Rem<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + SubAssign
    + MulAssign
    + DivAssign
    + RemAssign
    + Ord
    + Hash
    + Default
    + Sum
    + for<'a> Sum<&'a Self>
    + Product
    + for<'a> Product<&'a Self>
    + From<i8>
    + From<u8>
    + From<i16>
    + From<u16>
    + From<i32>
    + From<u32>
    + TryFrom<i64, Error = DecimalError>
    + TryFrom<u64, Error = DecimalError>
    + TryFrom<f64, Error = DecimalError>
    + TryFrom<f32, Error = DecimalError>
{
    fn inherent_clamp(self, lo: Self, hi: Self) -> Self;
}
macro_rules! impl_ops {
    ($($t:ty),*) => { $( impl Ops for $t { fn inherent_clamp(self, lo: Self, hi: Self) -> Self { <$t>::clamp(self, lo, hi) } } )* };
}
impl_ops!(D64, D96);

fn zero<T: Ops>() -> T {
    T::from_i32_n(0)
}
fn one<T: Ops>() -> T {
    T::from_i32_n(1)
}
fn hash_of<T: Hash>(t: &T) -> u64 {
    let mut h = DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

// ---------------------------------------------------------------------------
// Operators against the checked API
// ---------------------------------------------------------------------------

fn check_pair<T: Ops>(f: &mut Failures, a: T, b: T) {
    let (x, y) = (a.raw_big(), b.raw_big());
    let name = T::NAME;
    let at = || format!("{x} , {y}");
    // `Ok(Some(v))` = returned v, `Ok(None)` = panicked. The expectation is `checked_*`.
    macro_rules! chk {
        ($kind:expr, $got:expr, $want:expr) => {
            f.check(
                || format!("{name} {}", $kind),
                at,
                Ok(guarded(|| $got).ok()),
                $want,
            )
        };
    }
    chk!("a + b", a + b, a.checked_add(b));
    chk!("a - b", a - b, a.checked_sub(b));
    chk!("a * b", a * b, a.checked_mul(b));
    chk!("a / b", a / b, a.checked_div(b));
    chk!("a % b", a % b, a.checked_rem(b));
    chk!(
        "a += b",
        {
            let mut c = a;
            c += b;
            c
        },
        a.checked_add(b)
    );
    chk!(
        "a -= b",
        {
            let mut c = a;
            c -= b;
            c
        },
        a.checked_sub(b)
    );
    chk!(
        "a *= b",
        {
            let mut c = a;
            c *= b;
            c
        },
        a.checked_mul(b)
    );
    chk!(
        "a /= b",
        {
            let mut c = a;
            c /= b;
            c
        },
        a.checked_div(b)
    );
    chk!(
        "a %= b",
        {
            let mut c = a;
            c %= b;
            c
        },
        a.checked_rem(b)
    );
    // `Sum` / `Product` are left folds from ZERO / ONE: they overflow on the INTERMEDIATE value.
    chk!(
        "sum [a, b]",
        [a, b].into_iter().sum::<T>(),
        a.checked_add(b)
    );
    chk!("sum &[a, b]", [a, b].iter().sum::<T>(), a.checked_add(b));
    chk!(
        "product [a, b]",
        [a, b].into_iter().product::<T>(),
        a.checked_mul(b)
    );
    chk!(
        "product &[a, b]",
        [a, b].iter().product::<T>(),
        a.checked_mul(b)
    );

    // Ord / Eq / Hash agree with the raw integers.
    let want = x.cmp(&y);
    f.check(|| format!("{name} cmp"), at, Ok(a.cmp(&b)), want);
    f.check(
        || format!("{name} partial_cmp"),
        at,
        Ok(a.partial_cmp(&b)),
        Some(want),
    );
    f.check(|| format!("{name} =="), at, Ok(a == b), x == y);
    f.check(|| format!("{name} <"), at, Ok(a < b), x < y);
    f.check(|| format!("{name} >="), at, Ok(a >= b), x >= y);
    f.check(
        || format!("{name} min"),
        at,
        Ok(Dec::min(a, b)),
        if x <= y { a } else { b },
    );
    f.check(
        || format!("{name} max"),
        at,
        Ok(Dec::max(a, b)),
        if x >= y { a } else { b },
    );
    f.check(
        || format!("{name} Ord::min"),
        at,
        Ok(Ord::min(a, b)),
        Dec::min(a, b),
    );
    f.check(
        || format!("{name} Ord::max"),
        at,
        Ok(Ord::max(a, b)),
        Dec::max(a, b),
    );
    if want == Ordering::Equal {
        f.check(
            || format!("{name} equal => same hash"),
            at,
            Ok(hash_of(&a)),
            hash_of(&b),
        );
    }
    // clamp: both the inherent method and `Ord::clamp`; a reversed interval panics.
    let (lo, hi) = (Dec::min(a, b), Dec::max(a, b));
    for c in [a, b, zero::<T>(), one::<T>()] {
        let want = if c < lo {
            lo
        } else if c > hi {
            hi
        } else {
            c
        };
        f.check(
            || format!("{name} clamp"),
            at,
            Ok(guarded(|| c.inherent_clamp(lo, hi)).ok()),
            Some(want),
        );
        f.check(
            || format!("{name} Ord::clamp"),
            at,
            Ok(guarded(|| Ord::clamp(c, lo, hi)).ok()),
            Some(want),
        );
    }
    if lo != hi {
        f.check(
            || format!("{name} clamp(min > max) panics"),
            at,
            Ok(guarded(|| a.inherent_clamp(hi, lo)).ok()),
            None,
        );
        f.check(
            || format!("{name} Ord::clamp(min > max) panics"),
            at,
            Ok(guarded(|| Ord::clamp(a, hi, lo)).ok()),
            None,
        );
    }
}

fn check_unary<T: Ops>(f: &mut Failures, a: T) {
    let x = a.raw_big();
    let name = T::NAME;
    let at = || format!("{x}");
    f.check(
        || format!("{name} -a"),
        at,
        Ok(guarded(|| -a).ok()),
        a.checked_neg(),
    );
    f.check(
        || format!("{name} Default"),
        at,
        Ok(T::default()),
        zero::<T>(),
    );
    f.check(|| format!("{name} a.clone()"), at, Ok(a.clone()), a);
    // Debug is "NAME(<Display>)"; the alternate form shows the raw integer.
    f.check(
        || format!("{name} Debug"),
        at,
        Ok(format!("{a:?}")),
        format!("{name}({a})"),
    );
    f.check(
        || format!("{name} Debug#"),
        at,
        Ok(format!("{a:#?}").contains(&x.to_string())),
        true,
    );
    // a sum of one element, and of none
    f.check(
        || format!("{name} sum [a]"),
        at,
        Ok([a].into_iter().sum::<T>()),
        a,
    );
    f.check(
        || format!("{name} product [a]"),
        at,
        Ok([a].into_iter().product::<T>()),
        a,
    );
    f.check(|| format!("{name} a + ZERO"), at, Ok(a + zero::<T>()), a);
    f.check(|| format!("{name} a * ONE"), at, Ok(a * one::<T>()), a);
    f.check(|| format!("{name} a / ONE"), at, Ok(a / one::<T>()), a);
    f.check(|| format!("{name} a - a"), at, Ok(a - a), zero::<T>());
    f.check(
        || format!("{name} a / 0 panics"),
        at,
        Ok(guarded(|| a / zero::<T>()).ok()),
        None,
    );
    f.check(
        || format!("{name} a % 0 panics"),
        at,
        Ok(guarded(|| a % zero::<T>()).ok()),
        None,
    );
    f.check(|| format!("{name} a == a"), at, Ok(a == a), true);
}

fn check_conversions<T: Ops>(f: &mut Failures) {
    let name = T::NAME;
    // From<small ints> is exact for every value
    for v in [i8::MIN, -1, 0, 1, i8::MAX] {
        f.check(
            || format!("{name} From<i8>"),
            || v.to_string(),
            Ok(T::from(v)),
            T::from_i32_n(v as i32),
        );
    }
    for v in [0, 1, u8::MAX] {
        f.check(
            || format!("{name} From<u8>"),
            || v.to_string(),
            Ok(T::from(v)),
            T::from_i32_n(v as i32),
        );
    }
    for v in [i16::MIN, -1, 0, 1, i16::MAX] {
        f.check(
            || format!("{name} From<i16>"),
            || v.to_string(),
            Ok(T::from(v)),
            T::from_i32_n(v as i32),
        );
    }
    for v in [0, 1, u16::MAX] {
        f.check(
            || format!("{name} From<u16>"),
            || v.to_string(),
            Ok(T::from(v)),
            T::from_i32_n(v as i32),
        );
    }
    for v in [i32::MIN, -1, 0, 1, i32::MAX] {
        f.check(
            || format!("{name} From<i32>"),
            || v.to_string(),
            Ok(T::from(v)),
            T::from_i32_n(v),
        );
        // ... and equal to the oracle: v * 10^DECIMALS
        f.check(
            || format!("{name} From<i32> raw"),
            || v.to_string(),
            Ok(T::from(v).raw_big()),
            BigInt::from(v) * T::scale_big(),
        );
    }
    for v in [0, 1, u32::MAX] {
        f.check(
            || format!("{name} From<u32>"),
            || v.to_string(),
            Ok(T::from(v)),
            T::from_u32_n(v),
        );
        f.check(
            || format!("{name} From<u32> raw"),
            || v.to_string(),
            Ok(T::from(v).raw_big()),
            BigInt::from(v) * T::scale_big(),
        );
    }

    // TryFrom<i64/u64>: exact integer, or Overflow; same as the named constructor.
    let i64s = [
        i64::MIN,
        i64::MIN + 1,
        -(1 << 62),
        -92_233_720_369,
        -92_233_720_368,
        -39_614_081_257_132_169,
        -39_614_081_257_132_168,
        -1,
        0,
        1,
        39_614_081_257_132_168,
        39_614_081_257_132_169,
        92_233_720_368,
        92_233_720_369,
        1 << 62,
        i64::MAX - 1,
        i64::MAX,
    ];
    for v in i64s {
        let want = {
            let r = BigInt::from(v) * T::scale_big();
            T::from_raw_big(&r).ok_or(DecimalError::Overflow)
        };
        f.check(
            || format!("{name} TryFrom<i64>"),
            || v.to_string(),
            Ok(T::try_from(v)),
            want.clone(),
        );
        f.check(
            || format!("{name} try_from_i64"),
            || v.to_string(),
            Ok(T::try_from_i64_n(v)),
            want.clone(),
        );
        f.check(
            || format!("{name} from_i64"),
            || v.to_string(),
            Ok(T::from_i64_n(v)),
            want.ok(),
        );
    }
    for v in [
        0u64,
        1,
        92_233_720_367,
        92_233_720_368,
        92_233_720_369,
        39_614_081_257_132_168,
        39_614_081_257_132_169,
        1 << 62,
        1 << 63,
        u64::MAX - 1,
        u64::MAX,
    ] {
        let want =
            T::from_raw_big(&(BigInt::from(v) * T::scale_big())).ok_or(DecimalError::Overflow);
        f.check(
            || format!("{name} TryFrom<u64>"),
            || v.to_string(),
            Ok(T::try_from(v)),
            want.clone(),
        );
        f.check(
            || format!("{name} try_from_u64"),
            || v.to_string(),
            Ok(T::try_from_u64_n(v)),
            want.clone(),
        );
        f.check(
            || format!("{name} from_u64"),
            || v.to_string(),
            Ok(T::from_u64_n(v)),
            want.ok(),
        );
    }

    // TryFrom<f64> / TryFrom<f32> are `from_f64` with the failure as an error.
    let mut floats: Vec<f64> = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        -0.5,
        1e-9,
        -1e-9,
        5e-9,
        4.9e-9,
        0.1,
        123.456,
        -123.456,
        f64::MIN_POSITIVE,
        f64::EPSILON,
        1e10,
        -1e10,
        9.2e10,
        9.3e10,
        -9.2e10,
        -9.3e10,
        3.9e16,
        4e16,
        -4e16,
        1e18,
        -1e18,
        9.223372036854775e18,
        9.3e18,
        -9.3e18,
        1e30,
        -1e30,
        f64::MAX,
        f64::MIN,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    let top = T::hi().to_string().parse::<f64>().unwrap()
        / T::scale_big().to_string().parse::<f64>().unwrap();
    floats.extend([
        top,
        -top,
        top * (1.0 - 1e-15),
        top * (1.0 + 1e-15),
        -top * (1.0 - 1e-15),
        -top * (1.0 + 1e-15),
    ]);
    for x in floats {
        let want = T::from_f64(x);
        f.check(
            || format!("{name} TryFrom<f64> vs from_f64"),
            || format!("{x:e}"),
            Ok(T::try_from(x).ok()),
            want,
        );
        match T::try_from(x) {
            Ok(_) => {}
            Err(e) => {
                let expect = if x.is_nan() || x.is_infinite() {
                    DecimalError::InvalidFormat
                } else if x > 0.0 {
                    DecimalError::Overflow
                } else {
                    DecimalError::Underflow
                };
                f.check(
                    || format!("{name} TryFrom<f64> error kind"),
                    || format!("{x:e}"),
                    Ok(e),
                    expect,
                );
            }
        }
        let xf = x as f32;
        f.check(
            || format!("{name} TryFrom<f32> vs from_f64"),
            || format!("{xf:e}"),
            Ok(T::try_from(xf).ok()),
            T::from_f64(xf as f64),
        );
    }
}

fn run_ops<T: Ops>() {
    let mut f = Failures::default();
    let k = scale();
    let all = interesting::<T>();
    let mut rng = Rng(0x0975 ^ T::BITS as u64);

    let stride = (all.len() / (40 * k)).max(1);
    let core: Vec<T> = all.iter().step_by(stride).copied().collect();
    for &a in &core {
        check_unary(&mut f, a);
        for &b in &core {
            check_pair(&mut f, a, b);
        }
    }
    for (i, &a) in all.iter().enumerate() {
        check_unary(&mut f, a);
        for j in 0..(3 * k) {
            check_pair(&mut f, a, all[(i * 11 + j * 17 + 3) % all.len()]);
        }
    }
    for (a, b) in mul_edge_pairs::<T>(&mut rng, k)
        .into_iter()
        .chain(div_edge_pairs::<T>(&mut rng, k))
        .chain(limit_pairs::<T>())
    {
        check_pair(&mut f, a, b);
    }
    for _ in 0..(800 * k) {
        let (a, b): (T, T) = (rng.val(), rng.val());
        check_pair(&mut f, a, b);
        check_unary(&mut f, a);
    }

    // Left-fold semantics over longer sequences: the INTERMEDIATE partial sum decides.
    for _ in 0..(300 * k) {
        let n = 2 + rng.below(6) as usize;
        let xs: Vec<T> = (0..n).map(|_| rng.val()).collect();
        let (mut s, mut p) = (Some(zero::<T>()), Some(one::<T>()));
        for &x in &xs {
            s = s.and_then(|s| s.checked_add(x));
            p = p.and_then(|p| p.checked_mul(x));
        }
        let at = || {
            format!(
                "{:?}",
                xs.iter()
                    .map(|x| x.raw_big().to_string())
                    .collect::<Vec<_>>()
            )
        };
        f.check(
            || format!("{} sum fold", T::NAME),
            at,
            Ok(guarded(|| xs.iter().copied().sum::<T>()).ok()),
            s,
        );
        f.check(
            || format!("{} product fold", T::NAME),
            at,
            Ok(guarded(|| xs.iter().product::<T>()).ok()),
            p,
        );
    }
    // empty iterators are the identities
    f.check(
        || "empty sum".into(),
        || T::NAME.into(),
        Ok(core::iter::empty::<T>().sum::<T>()),
        zero::<T>(),
    );
    f.check(
        || "empty product".into(),
        || T::NAME.into(),
        Ok(core::iter::empty::<T>().product::<T>()),
        one::<T>(),
    );

    // Hash/Eq as a set: exactly one entry per distinct raw value.
    let set: HashSet<T> = all.iter().copied().collect();
    let distinct: HashSet<BigInt> = all.iter().map(|v| v.raw_big()).collect();
    f.check(
        || "hashset".into(),
        || T::NAME.into(),
        Ok(set.len()),
        distinct.len(),
    );

    check_conversions::<T>(&mut f);
    f.finish(&format!("operators {}", T::NAME));
}

#[test]
fn operators_d64() {
    run_ops::<D64>();
}
#[test]
fn operators_d96() {
    run_ops::<D96>();
}

// ---------------------------------------------------------------------------
// powi
// ---------------------------------------------------------------------------

/// What `powi` documents: exponentiation by squaring on the *truncating* `checked_mul`,
/// `None` on overflow, and for a negative exponent `ONE / x^|exp|`. Evaluated here from
/// BigInt arithmetic alone, so a slip in the crate's loop (an off-by-one in the squaring
/// guard, a mishandled `i32::MIN`) shows up as a difference.
fn model_powi<T: Dec>(x: &BigInt, e: i32) -> Option<BigInt> {
    let s = T::scale_big();
    let fits = |v: &BigInt| *v >= T::lo() && *v <= T::hi();
    let mul = |a: &BigInt, b: &BigInt| {
        let r = (a * b) / &s;
        fits(&r).then_some(r)
    };
    if e == 0 {
        return Some(s);
    }
    let mut n = e.unsigned_abs();
    let (mut base, mut res) = (x.clone(), s.clone());
    while n > 0 {
        if n & 1 == 1 {
            res = mul(&res, &base)?;
        }
        if n > 1 {
            base = mul(&base, &base)?;
        }
        n >>= 1;
    }
    if e < 0 {
        if res.is_zero() {
            return None;
        }
        let r = (&s * &s) / res; // truncates toward zero
        return fits(&r).then_some(r);
    }
    Some(res)
}

fn run_powi<T: Ops>() {
    let mut f = Failures::default();
    let k = scale();
    let name = T::NAME;
    let mut rng = Rng(0x9033 ^ T::BITS as u64);
    let exps: Vec<i32> = (-12..=12)
        .chain([
            17,
            31,
            32,
            33,
            63,
            64,
            65,
            100,
            127,
            128,
            1000,
            65_535,
            65_536,
            1 << 20,
            i32::MAX,
            i32::MAX - 1,
            i32::MIN,
            i32::MIN + 1,
            -17,
            -31,
            -32,
            -33,
            -63,
            -64,
            -65,
            -1000,
            -(1 << 20),
        ])
        .collect();

    // 1. the model, over boundary values and random values (small bases hit the long exponents)
    let mut bases: Vec<T> = interesting::<T>();
    for _ in 0..(300 * k) {
        bases.push(rng.val());
    }
    for _ in 0..(300 * k) {
        // magnitudes near 1.0: the interesting range for large exponents
        let r = &T::scale_big() + BigInt::from(rng.below(2001) as i64 - 1000);
        bases.push(T::from_raw_big(&if rng.below(2) == 0 { r } else { -r }).unwrap());
    }
    for &b in &bases {
        for &e in &exps {
            let want = model_powi::<T>(&b.raw_big(), e).map(|v| T::from_raw_big(&v).unwrap());
            f.check(
                || format!("{name} powi vs model"),
                || format!("{} ^ {e}", b.raw_big()),
                Ok(guarded(|| b.powi(e)).ok().flatten()),
                want,
            );
        }
    }

    // 2. exact for integer bases: k^e needs no rounding when it fits, so it is checkable
    //    without any model of the algorithm.
    for base in -20i64..=20 {
        for e in 0..=70i32 {
            let exact = BigInt::from(base).pow(e as u32) * T::scale_big();
            let want = T::from_raw_big(&exact);
            let got = guarded(|| T::from_i32_n(base as i32).powi(e))
                .ok()
                .flatten();
            f.check(
                || format!("{name} powi integer base exact"),
                || format!("{base}^{e}"),
                Ok(got),
                want,
            );
        }
        // negative exponent: 1 / k^e truncated, defined when k^e itself fits
        for e in 1..=40i32 {
            let kp = BigInt::from(base).pow(e as u32) * T::scale_big();
            let want = if base == 0 || !(kp >= T::lo() && kp <= T::hi()) {
                None
            } else {
                T::from_raw_big(&((T::scale_big() * T::scale_big()) / kp))
            };
            let got = guarded(|| T::from_i32_n(base as i32).powi(-e))
                .ok()
                .flatten();
            f.check(
                || format!("{name} powi integer base negative exp"),
                || format!("{base}^-{e}"),
                Ok(got),
                want,
            );
        }
    }

    // 3. identities for EVERY base, including the limits
    let mut ids = interesting::<T>();
    ids.extend([
        T::from_raw_big(&T::lo()).unwrap(),
        T::from_raw_big(&T::hi()).unwrap(),
    ]);
    for &b in &ids {
        let at = || format!("{}", b.raw_big());
        f.check(
            || format!("{name} x^0 = 1"),
            at,
            Ok(guarded(|| b.powi(0)).ok().flatten()),
            Some(one::<T>()),
        );
        f.check(
            || format!("{name} x^1 = x"),
            at,
            Ok(guarded(|| b.powi(1)).ok().flatten()),
            Some(b),
        );
        f.check(
            || format!("{name} x^2 = x*x"),
            at,
            Ok(guarded(|| b.powi(2)).ok().flatten()),
            b.checked_mul(b),
        );
        f.check(
            || format!("{name} x^3 = x*(x*x)"),
            at,
            Ok(guarded(|| b.powi(3)).ok().flatten()),
            b.checked_mul(b).and_then(|sq| b.checked_mul(sq)),
        );
        f.check(
            || format!("{name} x^-1 = 1/x"),
            at,
            Ok(guarded(|| b.powi(-1)).ok().flatten()),
            one::<T>().checked_div(b),
        );
        f.check(
            || format!("{name} x^-1 = recip"),
            at,
            Ok(guarded(|| b.powi(-1)).ok().flatten()),
            b.recip(),
        );
    }
    // 4. pinned corner cases of the loop
    let m1 = T::from_i32_n(-1);
    let (o, z, half) = (
        one::<T>(),
        zero::<T>(),
        T::from_raw_big(&(T::scale_big() / 2)).unwrap(),
    );
    for e in [i32::MIN, i32::MIN + 1, i32::MAX, i32::MAX - 1, 0, 1, 2] {
        let odd = e & 1 == 1;
        f.check(
            || format!("{name} 1^e"),
            || format!("{e}"),
            Ok(guarded(|| o.powi(e)).ok().flatten()),
            Some(o),
        );
        f.check(
            || format!("{name} (-1)^e"),
            || format!("{e}"),
            Ok(guarded(|| m1.powi(e)).ok().flatten()),
            Some(if odd { m1 } else { o }),
        );
        f.check(
            || format!("{name} 0^e"),
            || format!("{e}"),
            Ok(guarded(|| z.powi(e)).ok().flatten()),
            if e == 0 {
                Some(o)
            } else if e < 0 {
                None
            } else {
                Some(z)
            },
        );
    }
    // x with |x| < 1 underflows to zero, and then a negative power divides by zero
    f.check(
        || format!("{name} 0.5^MAX"),
        || String::new(),
        Ok(guarded(|| half.powi(i32::MAX)).ok().flatten()),
        Some(z),
    );
    f.check(
        || format!("{name} 0.5^MIN"),
        || String::new(),
        Ok(guarded(|| half.powi(i32::MIN)).ok().flatten()),
        None,
    );
    f.check(
        || format!("{name} 0.5^-1"),
        || String::new(),
        Ok(guarded(|| half.powi(-1)).ok().flatten()),
        Some(T::from_i32_n(2)),
    );
    f.check(
        || format!("{name} 2^MAX overflows"),
        || String::new(),
        Ok(guarded(|| T::from_i32_n(2).powi(i32::MAX)).ok().flatten()),
        None,
    );
    f.check(
        || format!("{name} 2^MIN overflows"),
        || String::new(),
        Ok(guarded(|| T::from_i32_n(2).powi(i32::MIN)).ok().flatten()),
        None,
    );
    f.finish(&format!("powi {name}"));
}

#[test]
fn powi_d64() {
    run_powi::<D64>();
}
#[test]
fn powi_d96() {
    run_powi::<D96>();
}

/// `try_powi` and `try_recip` are `powi` / `recip` with a typed error.
#[test]
fn try_powi_and_try_recip_error_kinds() {
    assert_eq!(D64::from_i32(2).try_powi(100), Err(DecimalError::Overflow));
    assert_eq!(D96::from_i32(2).try_powi(100), Err(DecimalError::Overflow));
    assert_eq!(D64::from_i32(3).try_powi(4), Ok(D64::from_i32(81)));
    assert_eq!(
        D64::ZERO.try_powi(-1),
        Err(DecimalError::Overflow),
        "0^-1 is reported as Overflow (it is None from powi)"
    );
    assert_eq!(D64::ZERO.try_recip(), Err(DecimalError::DivisionByZero));
    assert_eq!(D96::ZERO.try_recip(), Err(DecimalError::DivisionByZero));
    assert_eq!(D64::from_i32(4).try_recip(), Ok(D64::from_raw(25_000_000)));
    assert_eq!(
        D64::MAX.try_recip(),
        Ok(D64::ZERO),
        "1 / 92233720368.54775807 truncates to 0 (below the 1e-8 ulp)"
    );
    assert_eq!(D64::MIN.try_recip(), Ok(D64::ZERO));
    // 1 / ulp = 1e8 (exactly representable); 1 / (ulp/2) cannot exist
    assert_eq!(D64::from_raw(1).try_recip(), Ok(D64::from_i32(100_000_000)));
    assert_eq!(
        D64::from_raw(-1).try_recip(),
        Ok(D64::from_i32(-100_000_000))
    );
}

// ---------------------------------------------------------------------------
// Documented panics, pinned
// ---------------------------------------------------------------------------

macro_rules! should_panic_tests {
    ($($name:ident, $t:ident, $body:expr, $msg:literal;)*) => { $(
        #[test]
        #[should_panic(expected = $msg)]
        fn $name() {
            let f: fn() = $body;
            f();
        }
    )* };
}
should_panic_tests! {
    add_overflow_panics_d64, D64, || { let _ = D64::MAX + D64::from_raw(1); }, "attempt to add with overflow";
    sub_overflow_panics_d64, D64, || { let _ = D64::MIN - D64::from_raw(1); }, "attempt to subtract with overflow";
    mul_overflow_panics_d64, D64, || { let _ = D64::MAX * D64::from_i32(2); }, "attempt to multiply with overflow";
    div_zero_panics_d64, D64, || { let _ = D64::ONE / D64::ZERO; }, "attempt to divide by zero or overflow";
    div_overflow_panics_d64, D64, || { let _ = D64::MAX / D64::from_raw(1); }, "attempt to divide by zero or overflow";
    rem_zero_panics_d64, D64, || { let _ = D64::ONE % D64::ZERO; }, "attempt to calculate the remainder with overflow or by zero";
    rem_min_by_minus_ulp_panics_d64, D64, || { let _ = D64::MIN % D64::from_raw(-1); }, "attempt to calculate the remainder with overflow or by zero";
    neg_min_panics_d64, D64, || { let _ = -D64::MIN; }, "attempt to negate with overflow";
    add_overflow_panics_d96, D96, || { let _ = D96::MAX + D96::from_raw(1); }, "attempt to add with overflow";
    sub_overflow_panics_d96, D96, || { let _ = D96::MIN - D96::from_raw(1); }, "attempt to subtract with overflow";
    mul_overflow_panics_d96, D96, || { let _ = D96::MAX * D96::from_i32(2); }, "attempt to multiply with overflow";
    div_zero_panics_d96, D96, || { let _ = D96::ONE / D96::ZERO; }, "attempt to divide by zero or overflow";
    neg_min_panics_d96, D96, || { let _ = -D96::MIN; }, "attempt to negate with overflow";
    sum_partial_overflow_panics_d64, D64, || { let _: D64 = [D64::MAX, D64::ONE, -D64::ONE].into_iter().sum(); }, "attempt to add with overflow";
    clamp_reversed_panics_d64, D64, || { let _ = D64::ONE.clamp(D64::from_i32(2), D64::ZERO); }, "min must be less than or equal to max";
}

/// `Sum` is a left fold: `[MAX, 1, -1]` panics although the exact total is `MAX`. That is
/// the same as `i32`'s `Sum` in a debug build; unlike `i32`, it also panics in release.
/// Pinned so a change to saturating or wrapping behaviour is a conscious decision.
#[test]
fn sum_is_a_left_fold_that_panics_on_the_partial_sum() {
    assert!(guarded(|| [D64::MAX, D64::ONE, -D64::ONE].into_iter().sum::<D64>()).is_err());
    assert_eq!(
        [D64::ONE, -D64::ONE, D64::MAX].into_iter().sum::<D64>(),
        D64::MAX
    );
    assert!(guarded(|| [D96::MAX, D96::ONE, -D96::ONE].into_iter().sum::<D96>()).is_err());
    assert!(guarded(|| [D64::MAX, D64::MAX].iter().sum::<D64>()).is_err());
}

#[cfg(feature = "num-traits")]
#[test]
fn signed_is_consistent_between_traits_and_inherent_methods() {
    // `abs` / `signum` of every boundary value, through `num_traits::Signed` and the
    // inherent methods (they are different functions with the same names).
    fn go<T: Ops + Signed>(f: &mut Failures) {
        for a in interesting::<T>() {
            let x = a.raw_big();
            let at = || x.to_string();
            f.check(
                || format!("{} is_positive", T::NAME),
                at,
                Ok(Signed::is_positive(&a)),
                x.is_positive(),
            );
            f.check(
                || format!("{} is_negative", T::NAME),
                at,
                Ok(Signed::is_negative(&a)),
                x.is_negative(),
            );
            f.check(
                || format!("{} Signed::signum", T::NAME),
                at,
                Ok(Signed::signum(&a)),
                if x.is_zero() {
                    zero::<T>()
                } else if x.is_positive() {
                    one::<T>()
                } else {
                    T::from_i32_n(-1)
                },
            );
            // `abs(MIN)` is documented to saturate to MAX (it is not representable), not to panic or wrap.
            f.check(
                || format!("{} Signed::abs", T::NAME),
                at,
                Ok(guarded(|| Signed::abs(&a)).ok()),
                Some(a.saturating_abs()),
            );
            f.check(
                || format!("{} Signed::abs == inherent abs", T::NAME),
                at,
                Ok(Signed::abs(&a)),
                Dec::abs(a),
            );
            f.check(
                || format!("{} Dec::signum", T::NAME),
                at,
                Ok(Dec::signum(a) as i64),
                match x.sign() {
                    num_bigint::Sign::Minus => -1,
                    num_bigint::Sign::NoSign => 0,
                    num_bigint::Sign::Plus => 1,
                },
            );
            for b in [zero::<T>(), one::<T>(), T::from_i32_n(-1), a] {
                let want = if a <= b {
                    Some(zero::<T>())
                } else {
                    a.checked_sub(b)
                };
                f.check(
                    || format!("{} abs_sub", T::NAME),
                    at,
                    Ok(guarded(|| Signed::abs_sub(&a, &b)).ok()),
                    want,
                );
            }
        }
    }
    let mut f = Failures::default();
    go::<D64>(&mut f);
    go::<D96>(&mut f);
    f.finish("Signed");
}
