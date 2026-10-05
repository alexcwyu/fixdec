//! Crate-private helpers shared between the `D64` and `D96` implementations.
//!
//! `D64` (`src/d64.rs`) and `D96` (`src/d96.rs`) are near-duplicate fixed-point
//! types. Centralising the power-of-ten tables and the rounding helpers here
//! removes the divergence hazard where a fix lands in one type but not the
//! other (the root cause of several bugs found in review). Everything is
//! `no_std`-compatible and `const` where the standard library allows.

/// Powers of ten that fit in an `i128`: `POW10_I128[k] == 10^k` for
/// `k in 0..=38` (10^39 > i128::MAX, so the table stops at 38). Built at compile
/// time from `i128::pow`, so the values are correct by construction.
pub(crate) const POW10_I128: [i128; 39] = {
    let mut table = [0i128; 39];
    let mut k = 0;
    while k < 39 {
        table[k] = 10i128.pow(k as u32);
        k += 1;
    }
    table
};

/// `10^k` as `i128`. Panics if `k > 38` (the value would overflow `i128`).
#[inline(always)]
pub(crate) const fn pow10_i128(k: u8) -> i128 {
    POW10_I128[k as usize]
}

/// `10^k` as `u128` (`k <= 38`). Panics on a larger exponent.
#[inline(always)]
pub(crate) const fn pow10_u128(k: u8) -> u128 {
    // 10^k for k <= 38 is positive and fits i128, so the cast is exact.
    POW10_I128[k as usize] as u128
}

/// `10^k` as `i64` (`k <= 18`). Panics on a larger exponent (would overflow i64).
#[inline(always)]
pub(crate) const fn pow10_i64(k: u8) -> i64 {
    assert!(k <= 18, "pow10_i64: exponent too large for i64");
    POW10_I128[k as usize] as i64
}

/// `10^k` as `u64` (`k <= 19`). Panics on a larger exponent.
#[inline(always)]
pub(crate) const fn pow10_u64(k: u8) -> u64 {
    assert!(k <= 19, "pow10_u64: exponent too large for u64");
    POW10_I128[k as usize] as u64
}

/// Divides `m` by `10^k` with banker's rounding (round half to even) applied to
/// the full dropped fraction in a single step. Returns 0 for `k >= 39` (the
/// divisor exceeds any representable mantissa, so the quotient rounds to 0).
pub(crate) const fn round_div_pow10_i128(m: i128, k: u32) -> i128 {
    if k >= 39 {
        return 0;
    }
    let d = POW10_I128[k as usize];
    let q = m / d;
    let r = m % d;
    let half = d / 2;
    if r > half {
        q + 1
    } else if r < -half {
        q - 1
    } else if r == half {
        if q % 2 == 0 { q } else { q + 1 }
    } else if r == -half {
        if q % 2 == 0 { q } else { q - 1 }
    } else {
        q
    }
}

/// Generates a `banker_round_*` for a given signed integer type. The body is
/// identical across `i64` (D64) and `i128` (D96); the macro keeps it as a single
/// source of truth while preserving each type's native width (no perf cost).
macro_rules! define_banker_round {
    ($name:ident, $t:ty) => {
        /// Rounds `quotient` half-to-even using the division `remainder` and
        /// `half` (= `divisor / 2`). `remainder` and `half` carry the dividend's
        /// sign, so this handles negative values symmetrically.
        #[inline(always)]
        pub(crate) const fn $name(quotient: $t, remainder: $t, half: $t) -> $t {
            if remainder > half {
                quotient + 1
            } else if remainder < -half {
                quotient - 1
            } else if remainder == half {
                if quotient % 2 == 0 {
                    quotient
                } else {
                    quotient + 1
                }
            } else if remainder == -half {
                if quotient % 2 == 0 {
                    quotient
                } else {
                    quotient - 1
                }
            } else {
                quotient
            }
        }
    };
}

define_banker_round!(banker_round_i64, i64);
define_banker_round!(banker_round_i128, i128);

/// Applies a [`RoundingStrategy`](crate::RoundingStrategy) to an integer division
/// result. `q` is the truncated-toward-zero quotient and `r` the remainder, which
/// carries the dividend's sign (`dividend == q * divisor + r`, `|r| < divisor`,
/// `divisor > 0`). Returns the rounded quotient. Shared by the explicit
/// `*_with_strategy` / `*_rounded` methods on both `D64` and `D96`.
///
/// Midpoint classification compares `2*|r|` against `divisor` (so it is exact for
/// odd divisors too, e.g. dividing by a tick or a price). `2*|r|` fits `i128` for
/// every in-crate caller (`|r| < divisor <= 2^95`).
#[inline]
pub(crate) const fn apply_rounding(
    q: i128,
    r: i128,
    divisor: i128,
    strategy: crate::RoundingStrategy,
) -> i128 {
    use crate::RoundingStrategy::*;
    if r == 0 {
        return q;
    }
    let neg = r < 0;
    let twice = if neg { -(2 * r) } else { 2 * r }; // 2*|r|
    let round_away = match strategy {
        ToZero => false,
        AwayFromZero => true,
        ToPositiveInfinity => !neg,
        ToNegativeInfinity => neg,
        MidpointNearestEven => {
            if twice > divisor {
                true
            } else if twice < divisor {
                false
            } else {
                q % 2 != 0 // exact tie -> round to even
            }
        }
        MidpointAwayFromZero => twice >= divisor, // tie -> away
        MidpointTowardZero => twice > divisor,    // tie -> toward zero
    };
    if round_away {
        if neg { q - 1 } else { q + 1 }
    } else {
        q
    }
}

/// Unsigned counterpart of [`apply_rounding`] for the wide D96 division path,
/// where the quotient magnitude can exceed `i128::MAX` before the final range
/// check. `q` and `r` are unsigned magnitudes (`r < divisor`, `divisor > 0`) and
/// `neg` is the sign of the true result; returns the rounded magnitude. The
/// midpoint compares `2*r` against `divisor` (fits `u128`: `r < divisor <= 2^95`).
#[inline]
pub(crate) const fn apply_rounding_unsigned(
    q: u128,
    r: u128,
    divisor: u128,
    neg: bool,
    strategy: crate::RoundingStrategy,
) -> u128 {
    use crate::RoundingStrategy::*;
    if r == 0 {
        return q;
    }
    let round_up = match strategy {
        ToZero => false,
        AwayFromZero => true,
        ToPositiveInfinity => !neg,
        ToNegativeInfinity => neg,
        MidpointNearestEven => {
            let twice = 2 * r;
            if twice > divisor {
                true
            } else if twice < divisor {
                false
            } else {
                q % 2 == 1 // exact tie -> round to even
            }
        }
        MidpointAwayFromZero => 2 * r >= divisor, // tie -> away
        MidpointTowardZero => 2 * r > divisor,    // tie -> toward zero
    };
    if round_up { q + 1 } else { q }
}

/// Integer floor square root of a `u128`: the largest `r` with `r*r <= n`.
///
/// Delegates to the standard library's [`u128::isqrt`] (stable, `const`, and
/// `no_std`), which is ~4x faster than a hand-rolled bit-by-bit loop. Backs
/// `D64::sqrt` (whose radicand `raw*1e8` always fits a u128) and the small-value
/// fast path of `D96::sqrt`. Kept as a named helper so the contract has one
/// documented home and a single unit-test target.
#[inline(always)]
pub(crate) const fn isqrt_u128(n: u128) -> u128 {
    n.isqrt()
}

/// Euclid's GCD on unsigned 128-bit values. `gcd(x, 0) == x`, `gcd(0, y) == y`.
/// Used to reduce `as_integer_ratio` to lowest terms (the denominator is always a
/// power of ten, so the gcd is a divisor of `SCALE`).
#[inline]
pub(crate) const fn gcd_u128(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pow10_tables_match_checked_pow() {
        for k in 0..=38u32 {
            assert_eq!(POW10_I128[k as usize], 10i128.checked_pow(k).unwrap());
            assert_eq!(pow10_i128(k as u8), 10i128.pow(k));
            assert_eq!(pow10_u128(k as u8), 10u128.pow(k));
        }
        for k in 0..=18u32 {
            assert_eq!(pow10_i64(k as u8), 10i64.pow(k));
        }
        for k in 0..=19u32 {
            assert_eq!(pow10_u64(k as u8), 10u64.pow(k));
        }
    }

    #[test]
    fn banker_round_matches_for_both_widths() {
        // half = 5 (divisor 10): ties round to even, others round normally.
        assert_eq!(banker_round_i64(2, 5, 5), 2); // 2.5 -> 2 (even)
        assert_eq!(banker_round_i64(3, 5, 5), 4); // 3.5 -> 4 (even)
        assert_eq!(banker_round_i64(2, 6, 5), 3); // > half -> up
        assert_eq!(banker_round_i64(-2, -5, 5), -2); // -2.5 -> -2 (even)
        assert_eq!(banker_round_i128(2, 5, 5), 2);
        assert_eq!(banker_round_i128(3, 5, 5), 4);
        assert_eq!(banker_round_i128(-3, -5, 5), -4);
    }

    #[test]
    fn isqrt_u128_matches_floor_sqrt() {
        // Small exact cases and their neighbours.
        for n in 0u128..=1000 {
            let r = isqrt_u128(n);
            assert!(r * r <= n, "lower bound at {n}");
            assert!(n < (r + 1) * (r + 1), "upper bound at {n}");
        }
        // Perfect squares and one-below across a wide range, including k in the
        // 2^32..2^64 band (radicands up to ~2^126, near u128::MAX). `checked_mul`
        // keeps the oracle overflow-free where `k*k` would wrap.
        for k in [
            1u128,
            2,
            3,
            1_000,
            1u128 << 32,
            1u128 << 48,
            (1u128 << 63) - 1,
            1u128 << 63,
            (1u128 << 64) - 2,
            (1u128 << 64) - 1,
        ] {
            let sq = k.checked_mul(k).expect("k^2 fits u128 for k <= 2^64-1");
            assert_eq!(isqrt_u128(sq), k, "sqrt(k^2) == k for k={k}");
            assert_eq!(isqrt_u128(sq - 1), k - 1, "sqrt(k^2 - 1) == k-1 for k={k}");
        }
        // Saturating top: floor(sqrt(u128::MAX)) = 2^64 - 1, and it really is a
        // lower bound (top^2 doesn't overflow, so the product is the true value).
        let top = (1u128 << 64) - 1;
        assert_eq!(isqrt_u128(u128::MAX), top);
        assert_eq!(top.checked_mul(top), Some(u128::MAX - 2 * top));
    }
}

/// Number of decimal digits of `n` (`0` has one).
#[inline]
pub(crate) const fn digit_count(n: u64) -> usize {
    match n.checked_ilog10() {
        Some(l) => l as usize + 1,
        None => 1,
    }
}

/// Writes an unsigned, ASCII number the way `core`'s numeric `Display` does: the sign
/// (`-`, or `+` with the flag), then padding to `width` with the fill and alignment
/// (right by default; the `0` flag pads with zeros between sign and digits).
/// `body_len` is the length of what `body` writes.
#[inline]
pub(crate) fn pad_number(
    f: &mut core::fmt::Formatter<'_>,
    negative: bool,
    body_len: usize,
    body: impl FnOnce(&mut core::fmt::Formatter<'_>) -> core::fmt::Result,
) -> core::fmt::Result {
    use core::fmt::{Alignment, Write};
    let sign = if negative {
        "-"
    } else if f.sign_plus() {
        "+"
    } else {
        ""
    };
    let pad = f
        .width()
        .map_or(0, |w| w.saturating_sub(sign.len() + body_len));
    if pad == 0 {
        if !sign.is_empty() {
            f.write_str(sign)?;
        }
        return body(f);
    }
    if f.sign_aware_zero_pad() {
        f.write_str(sign)?;
        for _ in 0..pad {
            f.write_char('0')?;
        }
        return body(f);
    }
    let (left, right) = match f.align() {
        Some(Alignment::Left) => (0, pad),
        Some(Alignment::Center) => (pad / 2, pad - pad / 2),
        _ => (pad, 0),
    };
    let fill = f.fill();
    for _ in 0..left {
        f.write_char(fill)?;
    }
    f.write_str(sign)?;
    body(f)?;
    for _ in 0..right {
        f.write_char(fill)?;
    }
    Ok(())
}

// ============================================================================
// String parsing helpers
// ============================================================================

/// True if `s` is a well-formed numeral: `[+-]` then digits with at most one `.` and at
/// least one digit overall, then (when `scientific`) an optional `[eE][+-]digits`.
/// Says nothing about range or precision.
pub(crate) fn is_numeral(s: &[u8], scientific: bool) -> bool {
    let mut i = usize::from(matches!(s.first(), Some(b'+' | b'-')));
    let (mut digits, mut dots) = (0, 0);
    while i < s.len() && (s[i].is_ascii_digit() || s[i] == b'.') {
        if s[i] == b'.' {
            dots += 1;
        } else {
            digits += 1;
        }
        i += 1;
    }
    if digits == 0 || dots > 1 {
        return false;
    }
    if i == s.len() {
        return true;
    }
    if !scientific || !matches!(s[i], b'e' | b'E') {
        return false;
    }
    i += 1;
    i += usize::from(matches!(s.get(i), Some(b'+' | b'-')));
    i < s.len() && s[i..].iter().all(u8::is_ascii_digit)
}

/// A malformed string is `InvalidFormat` whatever else is wrong with it. The parsers stop
/// at the first problem they meet, so a string like `"1.123456789x"` would otherwise be
/// reported as `PrecisionLoss` (or `Overflow`) merely because that check came first.
/// Applied on the error path only, so valid input pays nothing.
pub(crate) fn parse_error(
    s: &str,
    e: crate::DecimalError,
    scientific: bool,
) -> crate::DecimalError {
    use crate::DecimalError::{InvalidFormat, Overflow, PrecisionLoss, Underflow};
    if matches!(e, Overflow | PrecisionLoss | Underflow)
        && !is_numeral(s.trim().as_bytes(), scientific)
    {
        InvalidFormat
    } else {
        e
    }
}

/// The magnitude, in units of `10^-decimals`, of the decimal numeral `int.frac × 10^exp`
/// (`int` and `frac` are ASCII digits the caller has validated; any length).
///
/// Exact mode (`lossy == false`) rejects a numeral with significant digits below the last
/// place as `PrecisionLoss`; lossy mode rounds them half to even. A magnitude beyond
/// `u128` is `Overflow`; the caller applies the sign and the type's own range.
///
/// Works on the digit strings, so a long numeral (`"1e-40"` written with 41 zeros) needs no
/// wide accumulator: leading zeros are skipped and trailing zeros become exponent.
pub(crate) fn scaled_magnitude(
    int: &[u8],
    frac: &[u8],
    exp: i64,
    decimals: u32,
    lossy: bool,
) -> Result<u128, crate::DecimalError> {
    use crate::DecimalError::{Overflow, PrecisionLoss};
    let total = int.len() + frac.len();
    let digit = |i: usize| if i < int.len() { int[i] } else { frac[i - int.len()] } - b'0';

    let lead = (0..total).take_while(|&i| digit(i) == 0).count();
    if lead == total {
        return Ok(0);
    }
    let trail = (lead..total).rev().take_while(|&i| digit(i) == 0).count();
    let len = total - lead - trail; // significant digits, the last one non-zero
    let digit = |i: usize| digit(lead + i);
    let parse = |n: usize| {
        (0..n).try_fold(0u128, |a, i| {
            a.checked_mul(10)?.checked_add(digit(i) as u128)
        })
    };

    // value = M * 10^e10, M the `len` significant digits
    let e10 = exp + decimals as i64 - frac.len() as i64 + trail as i64;
    if e10 >= 0 {
        if len as i64 + e10 > 39 {
            return Err(Overflow);
        }
        return parse(len)
            .and_then(|m| m.checked_mul(pow10_u128_checked(e10 as u32)?))
            .ok_or(Overflow);
    }
    // digits fall below the last place; the last significant one is non-zero
    if !lossy {
        return Err(PrecisionLoss);
    }
    let k = (-e10) as usize;
    if k > len {
        return Ok(0); // below 0.1 of the last place
    }
    let kept = len - k;
    let q = parse(kept).ok_or(Overflow)?;
    let next = digit(kept);
    let more = kept + 1 < len; // a further digit, hence non-zero somewhere
    let up = next > 5 || (next == 5 && (more || q % 2 == 1));
    q.checked_add(up as u128).ok_or(Overflow)
}

/// `10^k` as `u128`, `None` beyond `10^38`.
fn pow10_u128_checked(k: u32) -> Option<u128> {
    10u128.checked_pow(k)
}

// ============================================================================
// Float conversion
// ============================================================================

/// `round_half_away(|value| × 10^decimals)` of a finite `f64`, computed exactly from its
/// binary expansion, or `None` if it exceeds `u128`. `scale` is `10^decimals` as `f64`.
///
/// Multiplying in floating point first rounds the product, which can push a value that
/// is in range (or on the near side of a tie) across the line -- the largest in-range
/// double was rejected for that reason. The float product is only used when it is far
/// from every rounding decision; otherwise the digits are computed in integers.
pub(crate) fn f64_scaled_magnitude(value: f64, decimals: u32, scale: f64) -> Option<u128> {
    let a = value.abs();
    let p = a * scale;
    if p < 1e12 {
        let fl = (p as u64) as f64; // floor, for 0 <= p < 1e12 (`f64::floor` needs std)
        let d = p - fl;
        if (d - 0.5).abs() > 1e-3 {
            return Some(fl as u128 + (d > 0.5) as u128);
        }
    }
    let bits = a.to_bits();
    let (e, frac) = (((bits >> 52) & 0x7ff) as i32, bits & ((1 << 52) - 1));
    let (m, e) = if e == 0 {
        (frac, -1074)
    } else {
        (frac | 1 << 52, e - 1075)
    };
    let n = m as u128 * 10u128.pow(decimals); // < 2^53 * 10^12 < 2^93
    if n == 0 {
        return Some(0);
    }
    if e >= 0 {
        return (e < 128 && n.leading_zeros() >= e as u32).then(|| n << e);
    }
    let s = (-e) as u32;
    if s >= 128 {
        return Some(0); // below 2^93 / 2^128: far under one half
    }
    let (q, rem) = (n >> s, n & ((1u128 << s) - 1));
    Some(q + (rem >= 1u128 << (s - 1)) as u128)
}
