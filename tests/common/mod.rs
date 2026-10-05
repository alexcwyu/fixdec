//! Shared machinery for the oracle tests: an arbitrary-precision ORACLE for the
//! fixed-point semantics of `D64` and `D96`, generators for the boundary values
//! that actually break them, and failure aggregation.
//!
//! The oracle shares no code with the crate: every operation is specified from its
//! documented definition on `BigInt` raw values (`value = raw / 10^DECIMALS`), and
//! the rounding modes are written out from their definitions.
#![allow(dead_code)]

use fixdec::{D64, D96, DecimalError, RoundingStrategy};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

pub fn pow10(n: u32) -> BigInt {
    BigInt::from(10u8).pow(n)
}

pub const STRATEGIES: [RoundingStrategy; 7] = [
    RoundingStrategy::MidpointNearestEven,
    RoundingStrategy::MidpointAwayFromZero,
    RoundingStrategy::MidpointTowardZero,
    RoundingStrategy::ToZero,
    RoundingStrategy::AwayFromZero,
    RoundingStrategy::ToNegativeInfinity,
    RoundingStrategy::ToPositiveInfinity,
];

/// `n / d` rounded per `s`, from the definition. `d != 0`.
pub fn round_div(n: &BigInt, d: &BigInt, s: RoundingStrategy) -> BigInt {
    use RoundingStrategy::*;
    assert!(!d.is_zero());
    let q = n / d; // truncates toward zero
    let rem = n - &q * d; // sign of `n`
    if rem.is_zero() {
        return q;
    }
    let neg = n.is_negative() != d.is_negative(); // sign of the exact quotient
    let away = if neg { &q - 1 } else { &q + 1 };
    let twice = rem.abs() * 2;
    let half = d.abs();
    match s {
        ToZero => q,
        AwayFromZero => away,
        ToNegativeInfinity => {
            if neg {
                away
            } else {
                q
            }
        }
        ToPositiveInfinity => {
            if neg {
                q
            } else {
                away
            }
        }
        MidpointAwayFromZero => {
            if twice >= half {
                away
            } else {
                q
            }
        }
        MidpointTowardZero => {
            if twice > half {
                away
            } else {
                q
            }
        }
        MidpointNearestEven => {
            if twice > half {
                away
            } else if twice < half {
                q
            } else if (&q % 2u8).is_zero() {
                q
            } else {
                away
            }
        }
    }
}

/// A fixed-point decimal type, viewed through the oracle. Everything the oracle tests
/// need from `D64` / `D96`, delegated to the inherent methods.
pub trait Dec: Copy + core::fmt::Debug + core::fmt::Display + PartialEq + Sized {
    const NAME: &'static str;
    const DECIMALS: u32;
    const BITS: u32;
    fn lo() -> BigInt;
    fn hi() -> BigInt;
    fn raw_big(self) -> BigInt;
    /// `None` outside `[lo, hi]`.
    fn from_raw_big(b: &BigInt) -> Option<Self>;
    fn scale_big() -> BigInt {
        pow10(Self::DECIMALS)
    }

    fn checked_add(self, o: Self) -> Option<Self>;
    fn checked_sub(self, o: Self) -> Option<Self>;
    fn checked_mul(self, o: Self) -> Option<Self>;
    fn checked_div(self, o: Self) -> Option<Self>;
    fn checked_rem(self, o: Self) -> Option<Self>;
    fn saturating_add(self, o: Self) -> Self;
    fn saturating_sub(self, o: Self) -> Self;
    fn saturating_mul(self, o: Self) -> Self;
    fn saturating_div(self, o: Self) -> Self;
    fn wrapping_add(self, o: Self) -> Self;
    fn wrapping_sub(self, o: Self) -> Self;
    fn wrapping_mul(self, o: Self) -> Self;
    fn wrapping_div(self, o: Self) -> Self;
    fn overflowing_add(self, o: Self) -> (Self, bool);
    fn overflowing_sub(self, o: Self) -> (Self, bool);
    fn overflowing_mul(self, o: Self) -> (Self, bool);
    fn try_add(self, o: Self) -> Result<Self, DecimalError>;
    fn try_sub(self, o: Self) -> Result<Self, DecimalError>;
    fn try_mul(self, o: Self) -> Result<Self, DecimalError>;
    fn try_div(self, o: Self) -> Result<Self, DecimalError>;
    fn try_rem(self, o: Self) -> Result<Self, DecimalError>;
    fn checked_neg(self) -> Option<Self>;
    fn saturating_neg(self) -> Self;
    fn wrapping_neg(self) -> Self;
    fn try_neg(self) -> Result<Self, DecimalError>;
    fn abs(self) -> Self;
    fn checked_abs(self) -> Option<Self>;
    fn saturating_abs(self) -> Self;
    fn wrapping_abs(self) -> Self;
    fn try_abs(self) -> Result<Self, DecimalError>;
    fn mul_add(self, m: Self, a: Self) -> Option<Self>;
    fn is_multiple_of(self, o: Self) -> bool;
    fn div_rem_big(self, o: Self) -> Option<(BigInt, Self)>;
    fn mul_int(self, n: i64) -> Option<Self>;
    fn add_int(self, n: i64) -> Option<Self>;
    fn sub_int(self, n: i64) -> Option<Self>;
    fn div_int(self, n: i64) -> Option<Self>;
    fn try_mul_int(self, n: i64) -> Result<Self, DecimalError>;
    fn try_div_int(self, n: i64) -> Result<Self, DecimalError>;
    fn sqrt(self) -> Option<Self>;
    fn try_sqrt(self) -> Result<Self, DecimalError>;
    fn recip(self) -> Option<Self>;
    fn signum(self) -> i32;
    fn min(self, o: Self) -> Self;
    fn max(self, o: Self) -> Self;
    fn floor(self) -> Self;
    fn ceil(self) -> Self;
    fn trunc(self) -> Self;
    fn fract(self) -> Self;
    fn round(self) -> Self;
    fn round_dp(self, dp: u8) -> Self;
    fn round_dp_with_strategy(self, dp: u8, s: RoundingStrategy) -> Self;
    fn checked_div_rounded(self, o: Self, dp: u8, s: RoundingStrategy) -> Option<Self>;
    fn checked_quantize(self, tick: Self, s: RoundingStrategy) -> Option<Self>;
    fn checked_floor_to_tick(self, tick: Self) -> Option<Self>;
    fn checked_ceil_to_tick(self, tick: Self) -> Option<Self>;
    fn powi(self, e: i32) -> Option<Self>;

    // text
    fn from_str_exact(s: &str) -> Result<Self, DecimalError>;
    fn from_str_lossy(s: &str) -> Result<Self, DecimalError>;
    fn from_fixed_point_str(s: &str, decimals: u8) -> Result<Self, DecimalError>;
    fn from_str_trait(s: &str) -> Result<Self, DecimalError>;
    // constructors / integer conversions (operands within i64)
    fn new_parts(integer: i64, fractional: i64) -> Self;
    fn from_bps(bps: i64) -> Option<Self>;
    fn to_bps_big(self) -> BigInt;
    fn with_scale_n(m: i64, scale: u32) -> Self;
    fn try_with_scale_n(m: i64, scale: u32) -> Option<Self>;
    fn with_scale_lossy_n(m: i64, scale: u32) -> Self;
    fn try_with_scale_lossy_n(m: i64, scale: u32) -> Option<Self>;
    fn from_i64_n(n: i64) -> Option<Self>;
    fn from_u64_n(n: u64) -> Option<Self>;
    fn from_i32_n(n: i32) -> Self;
    fn from_u32_n(n: u32) -> Self;
    fn try_from_i64_n(n: i64) -> Result<Self, DecimalError>;
    fn try_from_u64_n(n: u64) -> Result<Self, DecimalError>;
    fn to_int_big(self) -> BigInt;
    fn to_int_round_big(self) -> BigInt;
    // inspection
    fn mantissa_big(self) -> BigInt;
    fn scale_u32(self) -> u32;
    fn is_integer(self) -> bool;
    fn normalize(self) -> Self;
    fn ratio_big(self) -> (BigInt, BigInt);
    // floats
    fn from_f64(x: f64) -> Option<Self>;
    fn try_from_f64_n(x: f64) -> Result<Self, DecimalError>;
    fn to_f64(self) -> f64;
    fn percent_of(self, p: Self) -> Option<Self>;
    fn add_percent(self, p: Self) -> Option<Self>;
}

macro_rules! impl_dec {
    ($t:ident, $raw:ty, $name:literal, $decimals:literal, $bits:literal, $lo:expr, $hi:expr,
     $from_raw:expr, $mul_int:ident, $add_int:ident, $sub_int:ident, $div_int:ident,
     $try_mul_int:ident, $try_div_int:ident, $try_from_i64:expr, $try_from_u64:expr, $to_int:ident, $to_int_round:ident) => {
        impl Dec for $t {
            const NAME: &'static str = $name;
            const DECIMALS: u32 = $decimals;
            const BITS: u32 = $bits;
            fn lo() -> BigInt {
                BigInt::from($lo)
            }
            fn hi() -> BigInt {
                BigInt::from($hi)
            }
            fn raw_big(self) -> BigInt {
                BigInt::from(self.to_raw())
            }
            fn from_raw_big(b: &BigInt) -> Option<Self> {
                if *b < Self::lo() || *b > Self::hi() {
                    return None;
                }
                let f: fn(&BigInt) -> Option<$raw> = $from_raw;
                f(b).map(<$t>::from_raw)
            }
            fn checked_add(self, o: Self) -> Option<Self> {
                <$t>::checked_add(self, o)
            }
            fn checked_sub(self, o: Self) -> Option<Self> {
                <$t>::checked_sub(self, o)
            }
            fn checked_mul(self, o: Self) -> Option<Self> {
                <$t>::checked_mul(self, o)
            }
            fn checked_div(self, o: Self) -> Option<Self> {
                <$t>::checked_div(self, o)
            }
            fn checked_rem(self, o: Self) -> Option<Self> {
                <$t>::checked_rem(self, o)
            }
            fn saturating_add(self, o: Self) -> Self {
                <$t>::saturating_add(self, o)
            }
            fn saturating_sub(self, o: Self) -> Self {
                <$t>::saturating_sub(self, o)
            }
            fn saturating_mul(self, o: Self) -> Self {
                <$t>::saturating_mul(self, o)
            }
            fn saturating_div(self, o: Self) -> Self {
                <$t>::saturating_div(self, o)
            }
            fn wrapping_add(self, o: Self) -> Self {
                <$t>::wrapping_add(self, o)
            }
            fn wrapping_sub(self, o: Self) -> Self {
                <$t>::wrapping_sub(self, o)
            }
            fn wrapping_mul(self, o: Self) -> Self {
                <$t>::wrapping_mul(self, o)
            }
            fn wrapping_div(self, o: Self) -> Self {
                <$t>::wrapping_div(self, o)
            }
            fn overflowing_add(self, o: Self) -> (Self, bool) {
                <$t>::overflowing_add(self, o)
            }
            fn overflowing_sub(self, o: Self) -> (Self, bool) {
                <$t>::overflowing_sub(self, o)
            }
            fn overflowing_mul(self, o: Self) -> (Self, bool) {
                <$t>::overflowing_mul(self, o)
            }
            fn try_add(self, o: Self) -> Result<Self, DecimalError> {
                <$t>::try_add(self, o)
            }
            fn try_sub(self, o: Self) -> Result<Self, DecimalError> {
                <$t>::try_sub(self, o)
            }
            fn try_mul(self, o: Self) -> Result<Self, DecimalError> {
                <$t>::try_mul(self, o)
            }
            fn try_div(self, o: Self) -> Result<Self, DecimalError> {
                <$t>::try_div(self, o)
            }
            fn try_rem(self, o: Self) -> Result<Self, DecimalError> {
                <$t>::try_rem(self, o)
            }
            fn checked_neg(self) -> Option<Self> {
                <$t>::checked_neg(self)
            }
            fn saturating_neg(self) -> Self {
                <$t>::saturating_neg(self)
            }
            fn wrapping_neg(self) -> Self {
                <$t>::wrapping_neg(self)
            }
            fn try_neg(self) -> Result<Self, DecimalError> {
                <$t>::try_neg(self)
            }
            fn abs(self) -> Self {
                <$t>::abs(self)
            }
            fn checked_abs(self) -> Option<Self> {
                <$t>::checked_abs(self)
            }
            fn saturating_abs(self) -> Self {
                <$t>::saturating_abs(self)
            }
            fn wrapping_abs(self) -> Self {
                <$t>::wrapping_abs(self)
            }
            fn try_abs(self) -> Result<Self, DecimalError> {
                <$t>::try_abs(self)
            }
            fn mul_add(self, m: Self, a: Self) -> Option<Self> {
                <$t>::mul_add(self, m, a)
            }
            fn is_multiple_of(self, o: Self) -> bool {
                <$t>::is_multiple_of(self, o)
            }
            fn div_rem_big(self, o: Self) -> Option<(BigInt, Self)> {
                <$t>::div_rem(self, o).map(|(q, r)| (BigInt::from(q), r))
            }
            fn mul_int(self, n: i64) -> Option<Self> {
                <$t>::$mul_int(self, n.into())
            }
            fn add_int(self, n: i64) -> Option<Self> {
                <$t>::$add_int(self, n.into())
            }
            fn sub_int(self, n: i64) -> Option<Self> {
                <$t>::$sub_int(self, n.into())
            }
            fn div_int(self, n: i64) -> Option<Self> {
                <$t>::$div_int(self, n.into())
            }
            fn try_mul_int(self, n: i64) -> Result<Self, DecimalError> {
                <$t>::$try_mul_int(self, n.into())
            }
            fn try_div_int(self, n: i64) -> Result<Self, DecimalError> {
                <$t>::$try_div_int(self, n.into())
            }
            fn sqrt(self) -> Option<Self> {
                <$t>::sqrt(self)
            }
            fn try_sqrt(self) -> Result<Self, DecimalError> {
                <$t>::try_sqrt(self)
            }
            fn recip(self) -> Option<Self> {
                <$t>::recip(self)
            }
            fn signum(self) -> i32 {
                <$t>::signum(self)
            }
            fn min(self, o: Self) -> Self {
                <$t>::min(self, o)
            }
            fn max(self, o: Self) -> Self {
                <$t>::max(self, o)
            }
            fn floor(self) -> Self {
                <$t>::floor(self)
            }
            fn ceil(self) -> Self {
                <$t>::ceil(self)
            }
            fn trunc(self) -> Self {
                <$t>::trunc(self)
            }
            fn fract(self) -> Self {
                <$t>::fract(self)
            }
            fn round(self) -> Self {
                <$t>::round(self)
            }
            fn round_dp(self, dp: u8) -> Self {
                <$t>::round_dp(self, dp)
            }
            fn round_dp_with_strategy(self, dp: u8, s: RoundingStrategy) -> Self {
                <$t>::round_dp_with_strategy(self, dp, s)
            }
            fn checked_div_rounded(self, o: Self, dp: u8, s: RoundingStrategy) -> Option<Self> {
                <$t>::checked_div_rounded(self, o, dp, s)
            }
            fn checked_quantize(self, tick: Self, s: RoundingStrategy) -> Option<Self> {
                <$t>::checked_quantize(self, tick, s)
            }
            fn checked_floor_to_tick(self, tick: Self) -> Option<Self> {
                <$t>::checked_floor_to_tick(self, tick)
            }
            fn checked_ceil_to_tick(self, tick: Self) -> Option<Self> {
                <$t>::checked_ceil_to_tick(self, tick)
            }
            fn powi(self, e: i32) -> Option<Self> {
                <$t>::powi(self, e)
            }
            fn from_str_exact(s: &str) -> Result<Self, DecimalError> {
                <$t>::from_str_exact(s)
            }
            fn from_str_lossy(s: &str) -> Result<Self, DecimalError> {
                <$t>::from_str_lossy(s)
            }
            fn from_fixed_point_str(s: &str, decimals: u8) -> Result<Self, DecimalError> {
                <$t>::from_fixed_point_str(s, decimals)
            }
            fn from_str_trait(s: &str) -> Result<Self, DecimalError> {
                <$t as core::str::FromStr>::from_str(s)
            }
            fn new_parts(integer: i64, fractional: i64) -> Self {
                <$t>::new(integer.into(), fractional.into())
            }
            fn from_bps(bps: i64) -> Option<Self> {
                <$t>::from_basis_points(bps.into())
            }
            fn to_bps_big(self) -> BigInt {
                BigInt::from(<$t>::to_basis_points(self))
            }
            fn with_scale_n(m: i64, scale: u32) -> Self {
                <$t>::with_scale(m.into(), scale)
            }
            fn try_with_scale_n(m: i64, scale: u32) -> Option<Self> {
                <$t>::try_with_scale(m.into(), scale)
            }
            fn with_scale_lossy_n(m: i64, scale: u32) -> Self {
                <$t>::with_scale_lossy(m.into(), scale)
            }
            fn try_with_scale_lossy_n(m: i64, scale: u32) -> Option<Self> {
                <$t>::try_with_scale_lossy(m.into(), scale)
            }
            fn from_i64_n(n: i64) -> Option<Self> {
                <$t>::from_i64(n)
            }
            fn from_u64_n(n: u64) -> Option<Self> {
                <$t>::from_u64(n)
            }
            fn from_i32_n(n: i32) -> Self {
                <$t>::from_i32(n)
            }
            fn from_u32_n(n: u32) -> Self {
                <$t>::from_u32(n)
            }
            fn try_from_i64_n(n: i64) -> Result<Self, DecimalError> {
                let f: fn(i64) -> Result<$t, DecimalError> = $try_from_i64;
                f(n)
            }
            fn try_from_u64_n(n: u64) -> Result<Self, DecimalError> {
                let f: fn(u64) -> Result<$t, DecimalError> = $try_from_u64;
                f(n)
            }
            fn to_int_big(self) -> BigInt {
                BigInt::from(<$t>::$to_int(self))
            }
            fn to_int_round_big(self) -> BigInt {
                BigInt::from(<$t>::$to_int_round(self))
            }
            fn mantissa_big(self) -> BigInt {
                BigInt::from(<$t>::mantissa(self))
            }
            fn scale_u32(self) -> u32 {
                <$t>::scale(self)
            }
            fn is_integer(self) -> bool {
                <$t>::is_integer(self)
            }
            fn normalize(self) -> Self {
                <$t>::normalize(self)
            }
            fn ratio_big(self) -> (BigInt, BigInt) {
                let (n, d) = <$t>::as_integer_ratio(self);
                (BigInt::from(n), BigInt::from(d))
            }
            fn from_f64(x: f64) -> Option<Self> {
                <$t>::from_f64(x)
            }
            fn try_from_f64_n(x: f64) -> Result<Self, DecimalError> {
                <$t>::try_from_f64(x)
            }
            fn to_f64(self) -> f64 {
                <$t>::to_f64(self)
            }
            fn percent_of(self, p: Self) -> Option<Self> {
                <$t>::percent_of(self, p)
            }
            fn add_percent(self, p: Self) -> Option<Self> {
                <$t>::add_percent(self, p)
            }
        }
    };
}

impl_dec!(
    D64,
    i64,
    "D64",
    8,
    64,
    i64::MIN,
    i64::MAX,
    |b| b.to_i64(),
    mul_i64,
    add_i64,
    sub_i64,
    div_i64,
    try_mul_i64,
    try_div_i64,
    |n| D64::try_from_i64(n),
    |n| D64::try_from_u64(n),
    to_i64,
    to_i64_round
);
impl_dec!(
    D96,
    i128,
    "D96",
    12,
    96,
    -(1i128 << 95),
    (1i128 << 95) - 1,
    |b| b.to_i128(),
    mul_i128,
    add_i128,
    sub_i128,
    div_i128,
    try_mul_i128,
    try_div_i128,
    |n| D96::try_from_i128(n.into()),
    |n| D96::try_from_u128(n.into()),
    to_i128,
    to_i128_round
);

/// `from_f64` / `try_from_f64`: `round_half_away(x * 10^DECIMALS)` of the EXACT binary value of
/// `x`, or `Overflow` / `Underflow` (by sign) when that is outside the type, `InvalidFormat` for
/// NaN and infinities. Computed from the double's bits and a BigInt; forming `x * SCALE` in
/// floating point first would round the product, and a rounded product can cross a range limit
/// or a tie.
pub fn spec_f64<T: Dec>(x: f64) -> Result<BigInt, DecimalError> {
    if !x.is_finite() {
        return Err(DecimalError::InvalidFormat);
    }
    let bits = x.to_bits();
    let ex = ((bits >> 52) & 0x7ff) as i64;
    let frac = bits & ((1u64 << 52) - 1);
    let (m, e) = if ex == 0 {
        (frac, -1074i64)
    } else {
        (frac | (1 << 52), ex - 1075)
    };
    let n = BigInt::from(m) * pow10(T::DECIMALS);
    let q = if e >= 0 {
        n << (e as usize)
    } else {
        let sh = (-e) as usize;
        let q = &n >> sh;
        let r = &n - (&q << sh);
        if (&r << 1usize) >= (BigInt::from(1) << sh) {
            q + 1
        } else {
            q
        }
    };
    let raw = if x.is_sign_negative() { -q } else { q };
    if raw >= T::lo() && raw <= T::hi() {
        Ok(raw)
    } else {
        Err(if x.is_sign_negative() {
            DecimalError::Underflow
        } else {
            DecimalError::Overflow
        })
    }
}

// ---------------------------------------------------------------------------
// Boundary-value generation
// ---------------------------------------------------------------------------

/// Deterministic xorshift64*: a failing seed reproduces.
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    pub fn u128(&mut self) -> u128 {
        (self.next() as u128) << 64 | self.next() as u128
    }
    /// A raw value of the type with a uniform BIT LENGTH (tiny, medium and huge are
    /// equally likely) and a random sign.
    pub fn raw<T: Dec>(&mut self) -> BigInt {
        let bits = self.below(T::BITS as u64 + 1) as u32;
        let mag = if bits == 0 {
            0
        } else {
            self.u128() >> (128 - bits)
        };
        let b = if self.next() & 1 == 1 {
            -BigInt::from(mag)
        } else {
            BigInt::from(mag)
        };
        b.max(T::lo()).min(T::hi())
    }
    pub fn val<T: Dec>(&mut self) -> T {
        T::from_raw_big(&self.raw::<T>()).unwrap()
    }
}

/// Raw values on the edges of the algorithms: 0, ±1, powers of 10 and 2 and their
/// neighbours, exact half-way ties, the type's limits, and the `u64`/`u128` word
/// thresholds the wide arithmetic switches on.
pub fn interesting<T: Dec>() -> Vec<T> {
    let (lo, hi) = (T::lo(), T::hi());
    let mut set: Vec<BigInt> = Vec::new();
    let mut push = |b: BigInt| {
        for v in [b.clone(), -b] {
            if v >= lo && v <= hi {
                set.push(v);
            }
        }
    };
    for v in [
        0, 1, 2, 3, 5, 7, 9, 11, 49, 50, 51, 99, 101, 127, 128, 255, 256,
    ] {
        push(BigInt::from(v));
    }
    let digits = hi.to_string().len() as u32;
    for k in 0..=digits {
        let p = pow10(k);
        for d in -2..=2i32 {
            push(&p + d);
            push(&p * 5 + d);
            push(&p * 3 + d);
        }
    }
    for k in 0..T::BITS {
        let p = BigInt::from(1u8) << k;
        for d in -2..=2i32 {
            push(&p + d);
        }
    }
    for d in 0..=3i32 {
        push(&hi - d);
        push(&hi / 2 + d);
        push(&hi / 3 + d);
        push(&hi / 10 + d);
        push(&hi / &T::scale_big() + d); // the integer part limit, in raw units
    }
    // the scale itself and its neighbours: 1.0, 0.999.., 1.000..1
    let s = T::scale_big();
    for d in -3..=3i32 {
        push(&s + d);
        push(&s * 2 + d);
        push(&s / 2 + d);
    }
    set.push(lo.clone());
    set.push(&lo + 1);
    set.push(&lo + 2);
    set.push(&lo / 2);
    set.push(&lo / 2 + 1);
    set.push(BigInt::from(0));
    set.sort();
    set.dedup();
    set.into_iter()
        .filter_map(|v| T::from_raw_big(&v))
        .collect()
}

/// A raw magnitude with exactly `bits` bits (top bit set), within the type's range.
fn raw_with_bits<T: Dec>(rng: &mut Rng, bits: u32) -> BigInt {
    if bits == 0 {
        return BigInt::from(0);
    }
    let mag = (rng.u128() >> (128 - bits)) | (1u128 << (bits - 1));
    BigInt::from(mag).min(T::hi())
}

/// A raw magnitude whose bit length is uniform in `[lo_bits, hi_bits]`.
fn raw_in_bit_window<T: Dec>(rng: &mut Rng, lo_bits: u32, hi_bits: u32) -> BigInt {
    let hi_bits = hi_bits.min(T::BITS - 1).max(1);
    let lo_bits = lo_bits.min(hi_bits).max(1);
    let bits = lo_bits + rng.below((hi_bits - lo_bits + 1) as u64) as u32;
    raw_with_bits::<T>(rng, bits)
}

fn bits_of(b: &BigInt) -> u32 {
    b.bits() as u32
}

fn signed_pairs<T: Dec>(out: &mut Vec<(T, T)>, a: &BigInt, b: &BigInt) {
    for sa in [1i32, -1] {
        for sb in [1i32, -1] {
            if let (Some(x), Some(y)) = (T::from_raw_big(&(a * sa)), T::from_raw_big(&(b * sb))) {
                out.push((x, y));
            }
        }
    }
}

/// Pairs whose raw PRODUCT, divided by the scale, lands next to a target: the type's limits,
/// `2^64` and `2^128` (where the fast paths hand over to the wide ones, and the point where
/// the high limb of the 192-bit product equals `10^12`). The first operand's bit length is
/// drawn from the window in which a partner of the right size still fits the type.
pub fn mul_edge_pairs<T: Dec>(rng: &mut Rng, per_edge: usize) -> Vec<(T, T)> {
    let s = T::scale_big();
    let mut targets: Vec<BigInt> = vec![T::hi(), -T::lo(), T::hi() + 1, -T::lo() + 1, T::hi() / 2];
    for k in [31u32, 32, 63, 64, 65, 95, 96, 127, 128] {
        targets.push(BigInt::from(1u8) << k);
    }
    let mut out = Vec::new();
    for t in targets {
        let want = &t * &s; // a * b ~ want
        let need = bits_of(&want).saturating_sub(T::BITS - 1); // smallest |a| for which |b| can fit
        for _ in 0..per_edge * 4 {
            let a = raw_in_bit_window::<T>(rng, need, T::BITS - 1);
            if a.is_zero() {
                continue;
            }
            for delta in -2..=2i32 {
                signed_pairs::<T>(&mut out, &a, &(&want / &a + delta));
            }
        }
    }
    out
}

/// Pairs whose quotient `a*S/b` lands next to the type's limits and word thresholds.
pub fn div_edge_pairs<T: Dec>(rng: &mut Rng, per_edge: usize) -> Vec<(T, T)> {
    let s = T::scale_big();
    let mut targets: Vec<BigInt> = vec![T::hi(), -T::lo(), T::hi() + 1, -T::lo() + 1];
    for k in [31u32, 32, 63, 64, 95, 96, 127] {
        targets.push(BigInt::from(1u8) << k);
    }
    let mut out = Vec::new();
    for t in targets {
        // b ~ a*S/t must be in [1, hi]: a in [t/S, hi*t/S]
        let lo_bits = bits_of(&(&t / &s));
        let hi_bits = bits_of(&(T::hi() * &t / &s));
        for _ in 0..per_edge * 4 {
            let a = raw_in_bit_window::<T>(rng, lo_bits, hi_bits);
            if a.is_zero() {
                continue;
            }
            for delta in -2..=2i32 {
                let b = (&a * &s) / &t + delta;
                if !b.is_zero() {
                    signed_pairs::<T>(&mut out, &a, &b);
                }
            }
        }
    }
    out
}

/// The type's limits against the numbers that should leave them unchanged or flip them.
pub fn limit_pairs<T: Dec>() -> Vec<(T, T)> {
    let s = T::scale_big();
    let mut out = Vec::new();
    let limits = [
        T::lo(),
        T::lo() + 1,
        T::lo() + 2,
        T::hi(),
        T::hi() - 1,
        T::lo() / 2,
        T::hi() / 2,
    ];
    let others = [
        BigInt::from(1),
        BigInt::from(2),
        BigInt::from(3),
        BigInt::from(10),
        s.clone(),
        &s * 2,
        &s * 10,
        &s / 2,
        &s / 10,
        &s - 1,
        &s + 1,
        T::hi(),
        T::hi() - 1,
        T::lo() + 1,
        BigInt::from(1) << 63,
        BigInt::from(1) << 64,
        pow10(T::DECIMALS * 2),
    ];
    for l in &limits {
        for o in &others {
            signed_pairs::<T>(&mut out, &l.abs(), &o.abs());
            if let (Some(x), Some(y)) = (T::from_raw_big(l), T::from_raw_big(o)) {
                out.push((x, y));
                out.push((y, x));
            }
            if let (Some(x), Some(y)) = (T::from_raw_big(l), T::from_raw_big(&-o)) {
                out.push((x, y));
                out.push((y, x));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Failure aggregation: a single run shows the SCOPE of a bug, not just the first trip.
// ---------------------------------------------------------------------------

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Once;

thread_local!(static QUIET: Cell<bool> = const { Cell::new(false) });

fn install_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !QUIET.with(|q| q.get()) {
                prev(info)
            }
        }));
    });
}

/// Run `f`, turning a panic into `Err(())` (silently, only inside this call). Debug
/// builds panic on overflow; release builds wrap. A panic where the contract says
/// `None`/`Err` is a defect either way.
pub fn guarded<R>(f: impl FnOnce() -> R) -> Result<R, ()> {
    install_hook();
    QUIET.with(|q| q.set(true));
    let r = catch_unwind(AssertUnwindSafe(f));
    QUIET.with(|q| q.set(false));
    r.map_err(|_| ())
}

#[derive(Default)]
pub struct Failures {
    pub count: usize,
    pub shown: Vec<String>,
    pub by_kind: std::collections::BTreeMap<String, usize>,
}

impl Failures {
    pub fn check<X: PartialEq + core::fmt::Debug>(
        &mut self,
        kind: impl FnOnce() -> String,
        detail: impl FnOnce() -> String,
        got: Result<X, ()>,
        want: X,
    ) {
        if matches!(&got, Ok(g) if *g == want) {
            return;
        }
        self.count += 1;
        let kind = kind();
        let n = self.by_kind.entry(kind.clone()).or_insert(0);
        *n += 1;
        if *n <= 3 && self.shown.len() < 40 {
            let got = match got {
                Ok(g) => format!("{g:?}"),
                Err(()) => "PANIC".to_string(),
            };
            self.shown
                .push(format!("[{kind}] {}  got {got}, want {want:?}", detail()));
        }
    }

    pub fn finish(self, what: &str) {
        if self.count == 0 {
            return;
        }
        eprintln!("\n{what}: {} FAILURES", self.count);
        for (k, n) in &self.by_kind {
            eprintln!("  {n:>7} x {k}");
        }
        eprintln!("first examples:");
        for s in &self.shown {
            eprintln!("  {s}");
        }
        panic!("{what}: {} oracle mismatches (see stderr)", self.count);
    }
}
