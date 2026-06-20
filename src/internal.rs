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

/// Replacement for the standard library's `f64::round` (which is unavailable in
/// `core`): rounds half away from zero. The fixed-point `from_f64` paths
/// range-check the result afterwards, so the `as i128` truncation for huge
/// inputs is harmless (the caller rejects the out-of-range value either way).
#[inline]
pub(crate) fn round_half_away_f64(value: f64) -> f64 {
    let truncated = (value as i128) as f64; // toward zero (saturating for huge inputs)
    let frac = value - truncated;
    if frac >= 0.5 {
        truncated + 1.0
    } else if frac <= -0.5 {
        truncated - 1.0
    } else {
        truncated
    }
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
                if quotient % 2 == 0 { quotient } else { quotient + 1 }
            } else if remainder == -half {
                if quotient % 2 == 0 { quotient } else { quotient - 1 }
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
}
