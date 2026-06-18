use core::fmt;
use core::iter::{Product, Sum};
use core::ops::{
    Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign, Sub, SubAssign,
};
use core::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{D64, DecimalError};

/// 96-bit fixed-point decimal with 12 decimal places of precision.
///
/// Uses 128-bit storage but only uses 96 bits of the mantissa for faster arithmetic.
/// Range: ±39,614,081,257,132.168796771975167
/// Precision: 0.000000000001 (1 microGwei = 1000 wei)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "zerocopy",
    derive(
        zerocopy::FromBytes,
        zerocopy::IntoBytes,
        zerocopy::Immutable,
        zerocopy::KnownLayout
    )
)]
#[repr(transparent)]
pub struct D96 {
    value: i128,
}

// ============================================================================
// Constants
// ============================================================================

impl D96 {
    /// The scale factor: 10^12
    pub const SCALE: i128 = 1_000_000_000_000;

    /// The number of decimal places
    pub const DECIMALS: u8 = 12;

    /// Maximum 96-bit value: 2^95 - 1
    /// Range: ±39.6 trillion with 12 decimals
    pub const MAX: Self = Self {
        value: 39_614_081_257_132_168_796_771_975_167,
    };

    /// Minimum 96-bit value: -2^95
    pub const MIN: Self = Self {
        value: -39_614_081_257_132_168_796_771_975_168,
    };

    /// Zero
    pub const ZERO: Self = Self { value: 0 };

    /// One (1.0)
    pub const ONE: Self = Self { value: Self::SCALE };

    /// Ten (10.0)
    pub const TEN: Self = Self {
        value: 10 * Self::SCALE,
    };

    /// One hundred (100.0)
    pub const HUNDRED: Self = Self {
        value: 100 * Self::SCALE,
    };

    /// One thousand (1000.0)
    pub const THOUSAND: Self = Self {
        value: 1000 * Self::SCALE,
    };

    // ===== Currency Fractions =====

    /// One cent (0.01) - USD, EUR, and most currency smallest unit
    pub const CENT: Self = Self {
        value: Self::SCALE / 100,
    };

    /// One mil (0.001) - used in some pricing contexts
    pub const MIL: Self = Self {
        value: Self::SCALE / 1000,
    };

    // ===== Basis Points =====

    /// One basis point (0.0001) - 1 bps, common in interest rates
    pub const BASIS_POINT: Self = Self {
        value: Self::SCALE / 10_000,
    };

    /// One half basis point (0.00005)
    pub const HALF_BASIS_POINT: Self = Self {
        value: Self::SCALE / 20_000,
    };

    /// One quarter basis point (0.000025)
    pub const QUARTER_BASIS_POINT: Self = Self {
        value: Self::SCALE / 40_000,
    };

    // ===== Bond Price Fractions (32nds and 64ths) =====

    /// One thirty-second (1/32 = 0.03125) - US Treasury bond price tick
    pub const THIRTY_SECOND: Self = Self {
        value: Self::SCALE / 32,
    };

    /// One sixty-fourth (1/64 = 0.015625) - US Treasury bond price tick
    pub const SIXTY_FOURTH: Self = Self {
        value: Self::SCALE / 64,
    };

    /// One half of a thirty-second (1/64) - alternative name
    pub const HALF_THIRTY_SECOND: Self = Self::SIXTY_FOURTH;

    /// One quarter of a thirty-second (1/128 = 0.0078125)
    pub const QUARTER_THIRTY_SECOND: Self = Self {
        value: Self::SCALE / 128,
    };

    /// One eighth of a thirty-second (1/256 = 0.00390625)
    pub const EIGHTH_THIRTY_SECOND: Self = Self {
        value: Self::SCALE / 256,
    };

    // ===== Equity Price Fractions =====

    /// One eighth (0.125) - legacy stock pricing increment
    pub const EIGHTH: Self = Self {
        value: Self::SCALE / 8,
    };

    /// One sixteenth (0.0625) - legacy stock pricing increment
    pub const SIXTEENTH: Self = Self {
        value: Self::SCALE / 16,
    };

    /// One quarter (0.25)
    pub const QUARTER: Self = Self {
        value: Self::SCALE / 4,
    };

    /// One half (0.5)
    pub const HALF: Self = Self {
        value: Self::SCALE / 2,
    };

    // ===== Percentage Helpers =====

    /// One percent (0.01) - same as CENT but semantic clarity
    pub const PERCENT: Self = Self::CENT;

    /// Ten basis points (0.001) - same as MIL
    pub const TEN_BPS: Self = Self::MIL;

    /// One hundred basis points (0.01) - same as PERCENT
    pub const HUNDRED_BPS: Self = Self::PERCENT;

    // ===== Crypto-specific constants =====

    /// One satoshi in Bitcoin terms (0.00000001) - 8 decimal places
    pub const SATOSHI: Self = Self {
        value: Self::SCALE / 100_000_000,
    };

    /// One gwei (0.000000001) - 9 decimal places, common gas unit
    /// With 12 decimals, 1 gwei = 1000 units
    pub const GWEI: Self = Self {
        value: Self::SCALE / 1_000_000_000,
    };

    /// One microGwei (0.000000000001) - minimum representable unit
    /// Equal to 1000 wei
    pub const MICRO_GWEI: Self = Self { value: 1 };

    /// One thousand wei (0.000000000001 ETH)
    pub const KILO_WEI: Self = Self { value: 1 };
}

// ============================================================================
// Constructors and Raw Access
// ============================================================================

impl Default for D96 {
    fn default() -> Self {
        Self::ZERO
    }
}

impl D96 {
    /// The maximum valid 96-bit value
    const MAX_96BIT: i128 = 39_614_081_257_132_168_796_771_975_167;
    const MIN_96BIT: i128 = -39_614_081_257_132_168_796_771_975_168;

    /// Reduces an i128 into the signed 96-bit range `[MIN_96BIT, MAX_96BIT]` by
    /// wrapping modulo 2^96. Used by the `wrapping_*` operators so they wrap at
    /// the type's 96-bit boundary (like a native integer would at its own
    /// boundary) and never emit an out-of-range `D96`.
    #[inline(always)]
    const fn wrap_to_96(v: i128) -> i128 {
        const TWO_POW_96: i128 = 1i128 << 96;
        let mut r = v % TWO_POW_96;
        if r > Self::MAX_96BIT {
            r -= TWO_POW_96;
        } else if r < Self::MIN_96BIT {
            r += TWO_POW_96;
        }
        r
    }

    /// Clamps an i128 intermediate to the representable 96-bit range. Used by the
    /// rounding ops so rounding *up* past MAX (or *down* past MIN) saturates to
    /// the boundary instead of yielding an out-of-96-bit `D96`.
    #[inline(always)]
    const fn clamp_to_96(v: i128) -> i128 {
        if v > Self::MAX_96BIT {
            Self::MAX_96BIT
        } else if v < Self::MIN_96BIT {
            Self::MIN_96BIT
        } else {
            v
        }
    }

    /// Creates a new D96 from a raw scaled value.
    ///
    /// # Panics
    /// Panics (in both debug and release) if `value` exceeds the 96-bit range
    /// `[MIN_96BIT, MAX_96BIT]`. Use [`from_raw_checked`](Self::from_raw_checked)
    /// for a non-panicking variant.
    #[inline(always)]
    pub const fn from_raw(value: i128) -> Self {
        assert!(
            value <= Self::MAX_96BIT && value >= Self::MIN_96BIT,
            "D96::from_raw: value exceeds 96-bit range"
        );
        Self { value }
    }

    /// Creates a new D96 from a raw scaled value, checking bounds.
    #[inline(always)]
    pub const fn from_raw_checked(value: i128) -> Option<Self> {
        if value > Self::MAX_96BIT || value < Self::MIN_96BIT {
            None
        } else {
            Some(Self { value })
        }
    }

    /// Returns the raw internal value (scaled by 10^12).
    #[inline(always)]
    pub const fn to_raw(self) -> i128 {
        self.value
    }

    /// Creates a D96 from integer and fractional parts at compile time
    /// Example: `new(123, 450_000_000_000)` → 123.45
    ///
    /// The fractional part should always be positive.
    /// For negative numbers, use a negative integer part:
    /// `new(-123, 450_000_000_000)` → -123.45
    ///
    /// # Panics
    /// Panics if the value would overflow the 96-bit range.
    pub const fn new(integer: i128, fractional: i128) -> Self {
        let scaled = match integer.checked_mul(Self::SCALE) {
            Some(v) => v,
            None => panic!("overflow in D96::new: integer part too large"),
        };

        let value = if integer >= 0 {
            match scaled.checked_add(fractional) {
                Some(v) => v,
                None => panic!("overflow in D96::new: result too large"),
            }
        } else {
            match scaled.checked_sub(fractional) {
                Some(v) => v,
                None => panic!("overflow in D96::new: result too large"),
            }
        };

        // Verify it fits in 96-bit range
        if value > Self::MAX.value || value < Self::MIN.value {
            panic!("overflow in D96::new: result exceeds 96-bit range");
        }

        Self { value }
    }

    /// Create from basis points (1 bp = 0.0001)
    /// Example: `from_basis_points(100)` → 0.01 (1%)
    pub const fn from_basis_points(bps: i128) -> Option<Self> {
        let numerator = match bps.checked_mul(Self::SCALE) {
            Some(v) => v,
            None => return None,
        };

        let value = numerator / 10_000;

        // Check 96-bit range
        if value > Self::MAX.value || value < Self::MIN.value {
            return None;
        }

        Some(Self { value })
    }

    /// Convert to basis points
    /// Example: `D96::from_str("0.01").unwrap().to_basis_points()` → 100
    pub const fn to_basis_points(self) -> i128 {
        (self.value * 10_000) / Self::SCALE
    }

    /// Creates a D96 from a mantissa and scale (like rust_decimal).
    ///
    /// The scale represents the number of decimal places.
    /// For example: `with_scale(12345, 2)` = 123.45
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - The scale is greater than 12 (our max precision)
    /// - The resulting value is out of bounds for D96
    #[inline]
    pub fn with_scale(mantissa: i128, scale: u32) -> Self {
        assert!(
            scale <= Self::DECIMALS as u32,
            "scale exceeds D96 precision (max 12 decimals)"
        );

        let scale_diff = Self::DECIMALS as u32 - scale;

        if scale_diff == 0 {
            assert!(
                (Self::MIN_96BIT..=Self::MAX_96BIT).contains(&mantissa),
                "overflow: mantissa exceeds D96 96-bit range"
            );
            Self { value: mantissa }
        } else {
            let multiplier = const_pow10_i128(scale_diff as u8);

            match mantissa.checked_mul(multiplier) {
                Some(value) if (Self::MIN_96BIT..=Self::MAX_96BIT).contains(&value) => {
                    Self { value }
                }
                _ => panic!("overflow: mantissa * 10^{} exceeds D96 range", scale_diff),
            }
        }
    }

    /// Creates a D96 from a mantissa and scale, returning None on error.
    ///
    /// Like `with_scale` but returns None instead of panicking.
    #[inline]
    #[allow(clippy::manual_range_contains)] // RangeInclusive::contains is not const
    pub const fn try_with_scale(mantissa: i128, scale: u32) -> Option<Self> {
        if scale > Self::DECIMALS as u32 {
            return None;
        }

        let scale_diff = Self::DECIMALS as u32 - scale;

        if scale_diff == 0 {
            if mantissa > Self::MAX_96BIT || mantissa < Self::MIN_96BIT {
                return None;
            }
            Some(Self { value: mantissa })
        } else {
            let multiplier = const_pow10_i128(scale_diff as u8);

            match mantissa.checked_mul(multiplier) {
                Some(value) => {
                    if value > Self::MAX_96BIT || value < Self::MIN_96BIT {
                        None
                    } else {
                        Some(Self { value })
                    }
                }
                None => None,
            }
        }
    }

    /// Creates a D96 from a mantissa and scale, rounding if necessary.
    ///
    /// If the scale is greater than 12, the mantissa will be divided by 10^(scale-12)
    /// to fit our precision, rounding to the nearest even number (banker's rounding).
    ///
    /// # Panics
    ///
    /// Panics if the resulting value (after rounding) is out of bounds for D96.
    #[inline]
    pub fn with_scale_lossy(mantissa: i128, scale: u32) -> Self {
        match Self::try_with_scale_lossy(mantissa, scale) {
            Some(value) => value,
            None => panic!("overflow: scaled mantissa exceeds D96 96-bit range"),
        }
    }

    /// Returns `true` if `value` is a legal raw D96 (within the 96-bit range).
    #[inline(always)]
    #[allow(clippy::manual_range_contains)] // const fn: RangeInclusive::contains isn't const
    const fn is_in_96bit(value: i128) -> bool {
        value >= Self::MIN_96BIT && value <= Self::MAX_96BIT
    }

    /// Creates a D96 from a mantissa and scale, rounding if necessary, returns None on error.
    ///
    /// Like `with_scale_lossy` but returns None instead of panicking on overflow.
    #[inline]
    pub const fn try_with_scale_lossy(mantissa: i128, scale: u32) -> Option<Self> {
        if scale <= Self::DECIMALS as u32 {
            // Fast path: no rounding needed
            let scale_diff = Self::DECIMALS as u32 - scale;

            if scale_diff == 0 {
                // The mantissa is stored verbatim, so it must already be a legal
                // 96-bit value.
                if Self::is_in_96bit(mantissa) {
                    Some(Self { value: mantissa })
                } else {
                    None
                }
            } else {
                let multiplier = const_pow10_i128(scale_diff as u8);
                match mantissa.checked_mul(multiplier) {
                    Some(value) if Self::is_in_96bit(value) => Some(Self { value }),
                    _ => None,
                }
            }
        } else {
            // Need to round: divide by 10^(scale - 12) with banker's rounding,
            // applied to the FULL dropped fraction in a single i128 step. The
            // rounded result must still fit the 96-bit range.
            let scale_diff = scale - Self::DECIMALS as u32;
            let value = round_div_pow10_i128(mantissa, scale_diff);
            if Self::is_in_96bit(value) {
                Some(Self { value })
            } else {
                None
            }
        }
    }
}

// ============================================================================
// Arithmetic Operations - Addition
// ============================================================================

impl D96 {
    /// Checked addition. Returns `None` if overflow occurred.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        if let Some(result) = self.value.checked_add(rhs.value) {
            // Check 96-bit bounds
            if result > Self::MAX.value || result < Self::MIN.value {
                None
            } else {
                Some(Self { value: result })
            }
        } else {
            None
        }
    }

    /// Saturating addition. Clamps on overflow.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        let result = self.value.saturating_add(rhs.value);
        if result > Self::MAX.value {
            Self::MAX
        } else if result < Self::MIN.value {
            Self::MIN
        } else {
            Self { value: result }
        }
    }

    /// Wrapping addition. Wraps at the 96-bit boundary on overflow.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn wrapping_add(self, rhs: Self) -> Self {
        Self {
            value: Self::wrap_to_96(self.value.wrapping_add(rhs.value)),
        }
    }

    /// Checked addition. Returns an error if overflow occurred.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn try_add(self, rhs: Self) -> crate::Result<Self> {
        match self.checked_add(rhs) {
            Some(result) => Ok(result),
            None => Err(DecimalError::Overflow),
        }
    }
}

// ============================================================================
// Arithmetic Operations - Subtraction
// ============================================================================

impl D96 {
    /// Checked subtraction. Returns `None` if overflow occurred.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        if let Some(result) = self.value.checked_sub(rhs.value) {
            // Check 96-bit bounds
            if result > Self::MAX.value || result < Self::MIN.value {
                None
            } else {
                Some(Self { value: result })
            }
        } else {
            None
        }
    }

    /// Saturating subtraction. Clamps on overflow.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        let result = self.value.saturating_sub(rhs.value);
        if result > Self::MAX.value {
            Self::MAX
        } else if result < Self::MIN.value {
            Self::MIN
        } else {
            Self { value: result }
        }
    }

    /// Wrapping subtraction. Wraps at the 96-bit boundary on overflow.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn wrapping_sub(self, rhs: Self) -> Self {
        Self {
            value: Self::wrap_to_96(self.value.wrapping_sub(rhs.value)),
        }
    }

    /// Checked subtraction. Returns an error if overflow occurred.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn try_sub(self, rhs: Self) -> crate::Result<Self> {
        match self.checked_sub(rhs) {
            Some(result) => Ok(result),
            None => Err(DecimalError::Overflow),
        }
    }
}

// ============================================================================
// Arithmetic Operations - Multiplication
// ============================================================================

impl D96 {
    /// Optimized multiplication for 96-bit values
    ///
    /// Since we guarantee 96-bit inputs, we can use optimized 192-bit arithmetic
    #[inline(always)]
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        // Early exit for zero
        if self.value == 0 || rhs.value == 0 {
            return Some(Self::ZERO);
        }

        // Handle signs
        let result_negative = (self.value < 0) != (rhs.value < 0);
        let a = self.value.unsigned_abs();
        let b = rhs.value.unsigned_abs();

        // If product fits in 128 bits, we can use fast division
        // sqrt(2^128 / 1) ≈ 2^64 ≈ 18 quintillion raw
        // But we need: (a * b) / 10^12 to fit
        // So: a * b < 2^128
        // Safe threshold: both values < 2^64 (then product < 2^128)
        const FAST_THRESHOLD: u128 = 1u128 << 64; // 2^64

        if a < FAST_THRESHOLD && b < FAST_THRESHOLD {
            // Simple 128-bit multiplication and division
            let product = a * b;
            let quotient = product / (Self::SCALE as u128);

            // A negative result may legitimately reach magnitude |MIN| = 2^95,
            // while a positive one caps at MAX = 2^95 - 1. Bounding the unsigned
            // magnitude before the signed cast keeps `-(quotient as i128)` from
            // overflowing and makes the post-sign re-check redundant.
            if quotient > Self::max_magnitude(result_negative) {
                return None;
            }

            let result = if result_negative {
                -(quotient as i128)
            } else {
                quotient as i128
            };

            return Some(Self { value: result });
        }

        // SLOW PATH: Use 192-bit arithmetic for larger values
        let (prod_low, prod_high) = mul_96x96_to_192(a, b);
        let quotient = div_192_by_1e12(prod_low, prod_high)?;

        if quotient > Self::max_magnitude(result_negative) {
            return None;
        }

        let result = if result_negative {
            -(quotient as i128)
        } else {
            quotient as i128
        };

        Some(Self { value: result })
    }

    /// Largest representable unsigned magnitude for a result of the given sign.
    ///
    /// A negative D96 may reach `|MIN| = 2^95`; a positive one caps at
    /// `MAX = 2^95 - 1`. Used to bound multiply/divide quotients before the
    /// signed cast so `-(quotient as i128)` never overflows and so MIN-valued
    /// results (e.g. `MIN * 1`, `MIN / 1`) are not falsely rejected.
    #[inline(always)]
    const fn max_magnitude(result_negative: bool) -> u128 {
        if result_negative {
            Self::MIN.value.unsigned_abs()
        } else {
            Self::MAX.value as u128
        }
    }

    /// Saturating multiplication
    #[inline(always)]
    pub fn saturating_mul(self, rhs: Self) -> Self {
        match self.checked_mul(rhs) {
            Some(result) => result,
            None => {
                if (self.value > 0) == (rhs.value > 0) {
                    Self::MAX
                } else {
                    Self::MIN
                }
            }
        }
    }

    /// Wrapping multiplication
    #[inline(always)]
    pub fn wrapping_mul(self, rhs: Self) -> Self {
        if self.value == 0 || rhs.value == 0 {
            return Self::ZERO;
        }

        let result_negative = (self.value < 0) != (rhs.value < 0);
        let a = self.value.unsigned_abs();
        let b = rhs.value.unsigned_abs();

        // Use optimized 96×96 → 192-bit multiplication
        let (prod_low, prod_high) = mul_96x96_to_192(a, b);

        // For wrapping, we don't care about overflow
        let quotient = div_192_by_1e12_wrapping(prod_low, prod_high);

        let result = if result_negative {
            (quotient as i128).wrapping_neg()
        } else {
            quotient as i128
        };

        // Reduce into the signed 96-bit range so no wrapping op ever emits an
        // out-of-range D96 (matches wrapping_add/sub/neg/abs).
        Self {
            value: Self::wrap_to_96(result),
        }
    }

    /// Multiply by an integer (faster than general multiplication)
    /// Useful for: quantity * price, shares * rate, etc.
    #[inline(always)]
    pub const fn mul_i128(self, rhs: i128) -> Option<Self> {
        match self.value.checked_mul(rhs) {
            Some(result) => {
                // Check 96-bit bounds
                if result > Self::MAX.value || result < Self::MIN.value {
                    None
                } else {
                    Some(Self { value: result })
                }
            }
            None => None,
        }
    }

    /// Computes (self * mul) + add with only one rounding step
    /// More accurate and faster than separate mul + add
    #[inline(always)]
    pub fn mul_add(self, mul: Self, add: Self) -> Option<Self> {
        if self.value == 0 {
            return Some(add);
        }
        if mul.value == 0 {
            return Some(add);
        }

        // Compute self * mul first using 96-bit optimized path
        let mul_negative = (self.value < 0) != (mul.value < 0);
        let a = self.value.unsigned_abs();
        let b = mul.value.unsigned_abs();

        // Use optimized 96×96 → 192-bit multiplication. Operand magnitudes may
        // reach |MIN| = 2^95 (not MAX = 2^95-1), so assert against that bound.
        debug_assert!(a <= Self::MIN.value.unsigned_abs());
        debug_assert!(b <= Self::MIN.value.unsigned_abs());

        let (prod_low, prod_high) = mul_96x96_to_192(a, b);
        let quotient = div_192_by_1e12(prod_low, prod_high)?;

        if quotient > Self::max_magnitude(mul_negative) {
            return None;
        }

        let product = if mul_negative {
            -(quotient as i128)
        } else {
            quotient as i128
        };

        // Now add
        let result = product.checked_add(add.value)?;

        // Check 96-bit bounds
        if result > Self::MAX.value || result < Self::MIN.value {
            return None;
        }

        Some(Self { value: result })
    }

    /// Checked multiplication. Returns an error if overflow occurred.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn try_mul(self, rhs: Self) -> crate::Result<Self> {
        match self.checked_mul(rhs) {
            Some(result) => Ok(result),
            None => Err(DecimalError::Overflow),
        }
    }

    /// Multiply by an integer, returning an error on overflow
    #[inline(always)]
    pub const fn try_mul_i128(self, rhs: i128) -> crate::Result<Self> {
        match self.mul_i128(rhs) {
            Some(v) => Ok(v),
            None => Err(DecimalError::Overflow),
        }
    }
}

/// Optimized 96×96 multiplication
/// Since inputs are ≤ 96 bits, we can use simpler arithmetic
#[inline(always)]
const fn mul_96x96_to_192(a: u128, b: u128) -> (u128, u64) {
    // Split into 64-bit parts
    let a_lo = a as u64;
    let a_hi = (a >> 64) as u64;
    let b_lo = b as u64;
    let b_hi = (b >> 64) as u64;

    // Do the multiplications
    let p0 = a_lo as u128 * b_lo as u128;
    let p1 = a_lo as u128 * b_hi as u128;
    let p2 = a_hi as u128 * b_lo as u128;
    let p3 = (a_hi as u128) * (b_hi as u128);

    // Combine - optimized for the case where high parts are small
    let mid = p1 + p2;
    let (low, carry) = p0.overflowing_add(mid << 64);
    let high = p3 + (mid >> 64) + (carry as u128);

    (low, high as u64)
}

/// Optimized division of 192-bit by 10^12
#[inline(always)]
const fn div_192_by_1e12(low: u128, high: u64) -> Option<u128> {
    const DIVISOR: u128 = 1_000_000_000_000;

    if high as u128 >= DIVISOR {
        return None;
    }

    // Optimize for the common case where high is 0
    if high == 0 {
        return Some(low / DIVISOR);
    }

    // Full division path
    let r_high = high as u128;

    let low_hi = low >> 64;
    let low_lo = low & 0xFFFF_FFFF_FFFF_FFFF;

    let dividend_mid = (r_high << 64) | low_hi;
    let q_mid = dividend_mid / DIVISOR;
    let r_mid = dividend_mid % DIVISOR;

    let dividend_low = (r_mid << 64) | low_lo;
    let q_low = dividend_low / DIVISOR;

    Some((q_mid << 64) | q_low)
}

/// Wrapping version of 192-bit division by 10^12
#[inline(always)]
const fn div_192_by_1e12_wrapping(low: u128, high: u64) -> u128 {
    const DIVISOR: u128 = 1_000_000_000_000;

    let r_high = (high as u128) % DIVISOR;

    let low_hi = low >> 64;
    let low_lo = low & 0xFFFF_FFFF_FFFF_FFFF;

    let dividend_mid = (r_high << 64) | low_hi;
    let q_mid = dividend_mid / DIVISOR;
    let r_mid = dividend_mid % DIVISOR;

    let dividend_low = (r_mid << 64) | low_lo;
    let q_low = dividend_low / DIVISOR;

    (q_mid << 64) | q_low
}

// ============================================================================
// Arithmetic Operations - Division
// ============================================================================

impl D96 {
    /// Checked division. Returns `None` if `rhs` is zero or overflow occurred.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.value == 0 {
            return None;
        }

        let self_negative = self.value < 0;
        let rhs_negative = rhs.value < 0;
        let result_negative = self_negative != rhs_negative;

        let a = self.value.unsigned_abs();
        let b = rhs.value.unsigned_abs();
        let scale = Self::SCALE as u128;

        // Multiply a (≤96 bits) by scale (≈40 bits) = ≤136 bits
        let (prod_low, prod_high) = mul_u128_by_small(a, scale);

        // Divide 192-bit number by b
        let quotient = div_192_by_u128(prod_low, prod_high, b)?;

        // A negative result may reach |MIN| = 2^95 (e.g. MIN / 1 == MIN); a
        // positive one caps at MAX = 2^95 - 1. Bound the unsigned magnitude
        // before the signed cast so MIN is reachable and the cast cannot wrap.
        if quotient > Self::max_magnitude(result_negative) {
            return None;
        }

        let result = if result_negative {
            -(quotient as i128)
        } else {
            quotient as i128
        };

        Some(Self { value: result })
    }

    /// Wrapping division. Wraps on overflow. Returns zero if `rhs` is zero.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn wrapping_div(self, rhs: Self) -> Self {
        if rhs.value == 0 {
            return Self::ZERO;
        }

        let self_negative = self.value < 0;
        let rhs_negative = rhs.value < 0;
        let result_negative = self_negative != rhs_negative;

        let a = self.value.unsigned_abs();
        let b = rhs.value.unsigned_abs();
        let scale = Self::SCALE as u128;

        let (prod_low, prod_high) = mul_u128_by_small(a, scale);
        let quotient = div_192_by_u128_wrapping(prod_low, prod_high, b);

        let result = if result_negative {
            (quotient as i128).wrapping_neg()
        } else {
            quotient as i128
        };

        // Reduce into the signed 96-bit range (matches the other wrapping ops).
        Self {
            value: Self::wrap_to_96(result),
        }
    }

    /// Saturating division. Clamps on overflow. Returns zero if `rhs` is zero.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn saturating_div(self, rhs: Self) -> Self {
        match self.checked_div(rhs) {
            Some(result) => result,
            None => {
                if rhs.value == 0 {
                    Self::ZERO
                } else if (self.value > 0) == (rhs.value > 0) {
                    Self::MAX
                } else {
                    Self::MIN
                }
            }
        }
    }

    /// Checked division. Returns an error if `rhs` is zero or overflow occurred.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn try_div(self, rhs: Self) -> crate::Result<Self> {
        if rhs.value == 0 {
            return Err(DecimalError::DivisionByZero);
        }
        match self.checked_div(rhs) {
            Some(result) => Ok(result),
            None => Err(DecimalError::Overflow),
        }
    }
}

// ============================================================================
// Arithmetic Operations - Remainder (modulo)
// ============================================================================

impl D96 {
    /// Checked remainder. Returns `None` if `rhs` is zero.
    ///
    /// Because both operands share the same `SCALE`, the decimal remainder is
    /// exactly the raw integer remainder; e.g. `10.5 % 3.0 == 1.5`. The sign of
    /// the result follows the dividend (truncated-division remainder), matching
    /// Rust's integer `%` and `rust_decimal`.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn checked_rem(self, rhs: Self) -> Option<Self> {
        match self.value.checked_rem(rhs.value) {
            Some(r) => Some(Self { value: r }),
            None => None,
        }
    }

    /// Checked remainder. Returns an error if `rhs` is zero.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn try_rem(self, rhs: Self) -> crate::Result<Self> {
        if rhs.value == 0 {
            return Err(DecimalError::DivisionByZero);
        }
        match self.checked_rem(rhs) {
            Some(result) => Ok(result),
            None => Err(DecimalError::Overflow),
        }
    }

    /// Returns `true` if `self` is an exact integer multiple of `rhs`.
    ///
    /// By convention `x.is_multiple_of(ZERO)` is `true` only when `x` is zero.
    #[inline(always)]
    pub const fn is_multiple_of(self, rhs: Self) -> bool {
        if rhs.value == 0 {
            self.value == 0
        } else {
            // Route through checked_rem for totality (matches D64): the native
            // `%` overflows for i128::MIN % -1. D96::MIN is -2^95 (not i128::MIN)
            // so it cannot overflow today, but this keeps the twins identical.
            match self.value.checked_rem(rhs.value) {
                Some(r) => r == 0,
                None => true,
            }
        }
    }

    /// Returns the integer quotient and decimal remainder of `self / rhs`.
    ///
    /// Returns `None` if `rhs` is zero. The results satisfy the identity
    /// `self == rhs.mul_i128(q).unwrap() + r`, where `q` is the truncated integer
    /// quotient and `r` is the remainder (`self % rhs`).
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn div_rem(self, rhs: Self) -> Option<(i128, Self)> {
        if rhs.value == 0 {
            return None;
        }
        match (
            self.value.checked_div(rhs.value),
            self.value.checked_rem(rhs.value),
        ) {
            (Some(q), Some(r)) => Some((q, Self { value: r })),
            _ => None,
        }
    }
}

/// Multiply a u128 by a small constant (like 10^12)
/// Returns (low 128 bits, high 128 bits) but high will be small
#[inline(always)]
const fn mul_u128_by_small(a: u128, b: u128) -> (u128, u128) {
    // For our use case: a ≤ 96 bits, b = 10^12 ≈ 2^40
    // Product ≤ 2^136, so high part will be at most 2^8 (very small)

    let a_lo = a as u64 as u128;
    let a_hi = (a >> 64) as u128;

    // Multiply both parts by b
    let p0 = a_lo * b;
    let p1 = a_hi * b;

    // Combine
    let (low, carry) = p0.overflowing_add(p1 << 64);
    let high = (p1 >> 64) + (carry as u128);

    (low, high)
}

/// Long division of a 256-bit numerator (`high:low`) by a u128 `divisor` using
/// base-2^32 limbs. Correct for ANY divisor up to 2^96: every remainder is
/// `< divisor`, so `(rem << 32) | limb < divisor * 2^32 <= 2^128` never
/// overflows a u128. The quotient is returned modulo 2^128 (the caller's early
/// `high >= divisor` check guarantees it actually fits when used by the checked
/// path).
#[inline(always)]
const fn div_256_by_u128_base32(low: u128, high: u128, divisor: u128) -> u128 {
    let limbs = [
        (high >> 96) & 0xFFFF_FFFF,
        (high >> 64) & 0xFFFF_FFFF,
        (high >> 32) & 0xFFFF_FFFF,
        high & 0xFFFF_FFFF,
        (low >> 96) & 0xFFFF_FFFF,
        (low >> 64) & 0xFFFF_FFFF,
        (low >> 32) & 0xFFFF_FFFF,
        low & 0xFFFF_FFFF,
    ];
    let mut rem: u128 = 0;
    let mut quo: u128 = 0;
    let mut i = 0;
    while i < 8 {
        let cur = (rem << 32) | limbs[i];
        quo = (quo << 32) | (cur / divisor); // cur / divisor < 2^32
        rem = cur % divisor;
        i += 1;
    }
    quo
}

/// Divide a 192-bit number by a u128 divisor.
/// Returns None if result doesn't fit in u128 or divisor is zero.
#[inline(always)]
const fn div_192_by_u128(low: u128, high: u128, divisor: u128) -> Option<u128> {
    if divisor == 0 {
        return None;
    }

    // If high part is >= divisor, the quotient won't fit in u128.
    if high >= divisor {
        return None;
    }

    if divisor <= u64::MAX as u128 {
        // Fast path: base-2^64 long division. Safe because every remainder is
        // < divisor <= u64::MAX, so `(rem << 64) | limb` fits in u128.
        let low_hi = low >> 64;
        let low_lo = low & 0xFFFF_FFFF_FFFF_FFFF;

        let dividend_mid = (high << 64) | low_hi;
        let q_mid = dividend_mid / divisor;
        let r_mid = dividend_mid % divisor;

        let dividend_low = (r_mid << 64) | low_lo;
        let q_low = dividend_low / divisor;

        Some((q_mid << 64) | q_low)
    } else {
        // divisor > 2^64: base-2^64 remainders can exceed 2^64 and would overflow
        // when shifted, so use base-2^32 long division instead.
        Some(div_256_by_u128_base32(low, high, divisor))
    }
}

/// Wrapping version of 192-bit division by u128.
#[inline(always)]
const fn div_192_by_u128_wrapping(low: u128, high: u128, divisor: u128) -> u128 {
    if divisor == 0 {
        return 0;
    }

    if divisor <= u64::MAX as u128 {
        let r_high = high % divisor;

        let low_hi = low >> 64;
        let low_lo = low & 0xFFFF_FFFF_FFFF_FFFF;

        let dividend_mid = (r_high << 64) | low_hi;
        let q_mid = dividend_mid / divisor;
        let r_mid = dividend_mid % divisor;

        let dividend_low = (r_mid << 64) | low_lo;
        let q_low = dividend_low / divisor;

        (q_mid << 64) | q_low
    } else {
        div_256_by_u128_base32(low, high, divisor)
    }
}

// ============================================================================
// Arithmetic Operations - Negation
// ============================================================================

impl D96 {
    /// Checked negation. Returns `None` if the result would overflow.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn checked_neg(self) -> Option<Self> {
        if let Some(result) = self.value.checked_neg() {
            // Check 96-bit bounds
            if result > Self::MAX.value || result < Self::MIN.value {
                None
            } else {
                Some(Self { value: result })
            }
        } else {
            None
        }
    }

    /// Saturating negation. Clamps on overflow.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn saturating_neg(self) -> Self {
        let result = self.value.saturating_neg();
        if result > Self::MAX.value {
            Self::MAX
        } else if result < Self::MIN.value {
            Self::MIN
        } else {
            Self { value: result }
        }
    }

    /// Wrapping negation. Wraps at the 96-bit boundary on overflow.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn wrapping_neg(self) -> Self {
        Self {
            value: Self::wrap_to_96(self.value.wrapping_neg()),
        }
    }

    /// Checked negation. Returns an error if overflow occurred.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn try_neg(self) -> crate::Result<Self> {
        match self.checked_neg() {
            Some(result) => Ok(result),
            None => Err(DecimalError::Overflow),
        }
    }
}

// ============================================================================
// Arithmetic Operations - Absolute Value
// ============================================================================

impl D96 {
    /// Returns the absolute value of `self`.
    ///
    /// `abs(MIN)` is not representable in 96 bits (`-(-2^95) = 2^95` is one past
    /// [`MAX`](Self::MAX)), so it saturates to `MAX` rather than returning an
    /// out-of-range value. Use [`checked_abs`](Self::checked_abs) to detect it.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn abs(self) -> Self {
        if self.value == Self::MIN.value {
            Self::MAX
        } else {
            Self {
                value: if self.value < 0 {
                    -self.value
                } else {
                    self.value
                },
            }
        }
    }

    /// Checked absolute value. Returns `None` if the result would overflow.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn checked_abs(self) -> Option<Self> {
        if self.value == Self::MIN.value {
            None
        } else {
            Some(self.abs())
        }
    }

    /// Saturating absolute value. Clamps on overflow.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn saturating_abs(self) -> Self {
        if self.value == Self::MIN.value {
            Self::MAX
        } else {
            self.abs()
        }
    }

    /// Wrapping absolute value. Wraps at the 96-bit boundary on overflow.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn wrapping_abs(self) -> Self {
        Self {
            value: Self::wrap_to_96(self.value.wrapping_abs()),
        }
    }

    /// Checked absolute value. Returns an error if overflow occurred.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn try_abs(self) -> crate::Result<Self> {
        match self.checked_abs() {
            Some(result) => Ok(result),
            None => Err(DecimalError::Overflow),
        }
    }
}

// ============================================================================
// Sign Operations
// ============================================================================

impl D96 {
    /// Returns `true` if `self` is positive.
    #[inline(always)]
    pub const fn is_positive(self) -> bool {
        self.value > 0
    }

    /// Returns `true` if `self` is negative.
    #[inline(always)]
    pub const fn is_negative(self) -> bool {
        self.value < 0
    }

    /// Returns `true` if `self` is zero.
    #[inline(always)]
    pub const fn is_zero(self) -> bool {
        self.value == 0
    }

    /// Returns the sign of `self` as -1, 0, or 1.
    #[inline(always)]
    pub const fn signum(self) -> i32 {
        if self.value > 0 {
            1
        } else if self.value < 0 {
            -1
        } else {
            0
        }
    }
}

// ============================================================================
// Comparison Utilities
// ============================================================================

impl D96 {
    /// Returns the minimum of two values.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn min(self, other: Self) -> Self {
        if self.value < other.value {
            self
        } else {
            other
        }
    }

    /// Returns the maximum of two values.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn max(self, other: Self) -> Self {
        if self.value > other.value {
            self
        } else {
            other
        }
    }

    /// Restricts a value to a certain interval.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn clamp(self, min: Self, max: Self) -> Self {
        assert!(
            min.value <= max.value,
            "min must be less than or equal to max"
        );
        if self.value < min.value {
            min
        } else if self.value > max.value {
            max
        } else {
            self
        }
    }
}

// ============================================================================
// Rounding Operations
// ============================================================================

impl D96 {
    /// Returns the largest integer less than or equal to `self`.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn floor(self) -> Self {
        let remainder = self.value % Self::SCALE;
        if remainder >= 0 {
            Self {
                value: self.value - remainder,
            }
        } else {
            Self {
                value: Self::clamp_to_96(self.value - remainder - Self::SCALE),
            }
        }
    }

    /// Returns the smallest integer greater than or equal to `self`.
    ///
    /// Saturates to [`MAX`](Self::MAX) if ceiling would exceed the 96-bit range.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn ceil(self) -> Self {
        let remainder = self.value % Self::SCALE;
        if remainder > 0 {
            Self {
                value: Self::clamp_to_96(self.value - remainder + Self::SCALE),
            }
        } else {
            Self {
                value: self.value - remainder,
            }
        }
    }

    /// Returns the integer part of `self`, truncating any fractional part.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn trunc(self) -> Self {
        Self {
            value: (self.value / Self::SCALE) * Self::SCALE,
        }
    }

    /// Returns the fractional part of `self`.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn fract(self) -> Self {
        Self {
            value: self.value % Self::SCALE,
        }
    }

    /// Rounds to the nearest integer, using banker's rounding (round half to even).
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn round(self) -> Self {
        let quotient = self.value / Self::SCALE;
        let remainder = self.value % Self::SCALE;
        let half = Self::SCALE / 2;

        let rounded_quotient = banker_round(quotient, remainder, half);

        Self {
            value: Self::clamp_to_96(rounded_quotient * Self::SCALE),
        }
    }

    /// Rounds to the specified number of decimal places.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn round_dp(self, decimal_places: u8) -> Self {
        assert!(
            decimal_places <= Self::DECIMALS,
            "decimal_places must be <= DECIMALS"
        );

        if decimal_places == Self::DECIMALS {
            return self;
        }

        let scale_reduction = Self::DECIMALS - decimal_places;
        let rounding_factor = const_pow10_i128(scale_reduction);

        let quotient = self.value / rounding_factor;
        let remainder = self.value % rounding_factor;
        let half = rounding_factor / 2;

        let rounded = banker_round(quotient, remainder, half);

        Self {
            value: Self::clamp_to_96(rounded * rounding_factor),
        }
    }
}

// ============================================================================
// Mathematical Operations
// ============================================================================

impl D96 {
    /// Returns the reciprocal (multiplicative inverse) of `self`.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn recip(self) -> Option<Self> {
        if self.value == 0 {
            None
        } else {
            Self::ONE.checked_div(self)
        }
    }

    /// Checked reciprocal. Returns an error if `self` is zero.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn try_recip(self) -> crate::Result<Self> {
        match self.recip() {
            Some(result) => Ok(result),
            None => Err(DecimalError::DivisionByZero),
        }
    }

    /// Raises `self` to an integer power.
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn powi(self, mut exp: i32) -> Option<Self> {
        if exp == 0 {
            return Some(Self::ONE);
        }

        if exp < 0 {
            let pos_result = match self.powi(-exp) {
                Some(r) => r,
                None => return None,
            };
            return Self::ONE.checked_div(pos_result);
        }

        let mut base = self;
        let mut result = Self::ONE;

        while exp > 0 {
            if exp % 2 == 1 {
                result = match result.checked_mul(base) {
                    Some(r) => r,
                    None => return None,
                };
            }
            if exp > 1 {
                base = match base.checked_mul(base) {
                    Some(b) => b,
                    None => return None,
                };
            }
            exp /= 2;
        }

        Some(result)
    }

    /// Checked integer power. Returns an error if overflow occurred.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn try_powi(self, exp: i32) -> crate::Result<Self> {
        match self.powi(exp) {
            Some(result) => Ok(result),
            None => Err(DecimalError::Overflow),
        }
    }

    /// Returns the square root of `self` using Newton's method.
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn sqrt(self) -> Option<Self> {
        if self.value < 0 {
            return None;
        }

        if self.value == 0 {
            return Some(Self::ZERO);
        }

        if self.value == Self::SCALE {
            return Some(Self::ONE);
        }

        // Newton's method for 96-bit values
        // We work with 128-bit to avoid overflow during iteration

        // Initial guess: use integer square root of the raw value
        let raw_sqrt_approx = isqrt_u128(self.value.unsigned_abs());

        // Scale the initial guess appropriately
        // Since self.value represents X * 10^12, we want sqrt(X * 10^12) = sqrt(X) * 10^6
        let sqrt_scale = 1_000_000i128; // 10^6 (sqrt of 10^12)
        let mut x = raw_sqrt_approx as i128 * sqrt_scale;

        const MAX_ITERATIONS: u32 = 20;
        let s = self.value * Self::SCALE; // This is safe for 96-bit values

        for _ in 0..MAX_ITERATIONS {
            if x == 0 {
                break;
            }

            let x_next = (x + s / x) / 2;

            // Check for convergence
            let diff = if x_next > x { x_next - x } else { x - x_next };

            if diff <= 1 {
                // Converged - check if result fits in 96-bit range
                if x_next > Self::MAX.value || x_next < Self::MIN.value {
                    return None;
                }
                return Some(Self { value: x_next });
            }

            x = x_next;
        }

        // Return best approximation after max iterations
        if x > Self::MAX.value || x < Self::MIN.value {
            None
        } else {
            Some(Self { value: x })
        }
    }

    /// Checked square root. Returns an error if `self` is negative.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn try_sqrt(self) -> crate::Result<Self> {
        match self.sqrt() {
            Some(result) => Ok(result),
            None => Err(DecimalError::InvalidFormat),
        }
    }
}

// ============================================================================
// D64 <-> D96 Conversion
// ============================================================================

impl D96 {
    /// Widens a `D64` (8 decimals) into a `D96` (12 decimals).
    ///
    /// Always exact and infallible: the raw value is scaled by `10^(12-8)`, and
    /// `i64::MAX * 10_000` is well within the 96-bit range.
    #[inline(always)]
    pub const fn from_d64(value: D64) -> Self {
        Self {
            value: value.to_raw() as i128 * 10_000,
        }
    }

    /// Narrows this `D96` to a `D64` exactly.
    ///
    /// Returns `Err(PrecisionLoss)` if the value has more than 8 significant
    /// decimals, or `Err(Overflow)`/`Err(Underflow)` if the magnitude exceeds
    /// `D64`'s range. Use [`to_d64_round`](Self::to_d64_round) to round instead.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn to_d64(self) -> crate::Result<D64> {
        // The low 4 decimal digits must be zero for an exact conversion.
        if self.value % 10_000 != 0 {
            return Err(DecimalError::PrecisionLoss);
        }
        let q = self.value / 10_000;
        if q > i64::MAX as i128 {
            Err(DecimalError::Overflow)
        } else if q < i64::MIN as i128 {
            Err(DecimalError::Underflow)
        } else {
            Ok(D64::from_raw(q as i64))
        }
    }

    /// Narrows this `D96` to a `D64`, rounding the extra 4 decimals using
    /// banker's rounding (round half to even).
    ///
    /// Only fails (`Overflow`/`Underflow`) when the magnitude exceeds `D64`'s
    /// range; never loses precision silently without rounding.
    #[inline(always)]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn to_d64_round(self) -> crate::Result<D64> {
        let quotient = self.value / 10_000;
        let remainder = self.value % 10_000;
        let rounded = banker_round(quotient, remainder, 5_000);
        if rounded > i64::MAX as i128 {
            Err(DecimalError::Overflow)
        } else if rounded < i64::MIN as i128 {
            Err(DecimalError::Underflow)
        } else {
            Ok(D64::from_raw(rounded as i64))
        }
    }
}

impl TryFrom<D96> for D64 {
    type Error = DecimalError;

    /// Exact narrowing of a `D96` to a `D64` (see [`D96::to_d64`]).
    #[inline(always)]
    fn try_from(value: D96) -> crate::Result<Self> {
        value.to_d64()
    }
}

// ============================================================================
// rust_decimal Interop (feature = "rust-decimal")
// ============================================================================

#[cfg(feature = "rust-decimal")]
impl D96 {
    /// Converts this `D96` to a [`rust_decimal::Decimal`].
    ///
    /// Always exact and infallible: the 96-bit mantissa fits `Decimal`'s 96-bit
    /// representation and the fixed scale of 12 is within `Decimal`'s maximum
    /// scale of 28, so the constructor cannot fail.
    #[inline]
    #[must_use]
    pub fn to_rust_decimal(self) -> rust_decimal::Decimal {
        rust_decimal::Decimal::from_i128_with_scale(self.value, Self::DECIMALS as u32)
    }

    /// Converts a [`rust_decimal::Decimal`] to a `D96` exactly.
    ///
    /// Returns `Err(PrecisionLoss)` if the value has more than 12 decimal
    /// places, or `Err(Overflow)`/`Err(Underflow)` if the magnitude exceeds
    /// `D96`'s range. Use [`from_rust_decimal_round`](Self::from_rust_decimal_round)
    /// to round the extra decimals instead of failing.
    pub fn from_rust_decimal(value: rust_decimal::Decimal) -> crate::Result<Self> {
        let mantissa = value.mantissa();
        let scale = value.scale();
        let dp = Self::DECIMALS as u32;
        let raw = if scale <= dp {
            // Scale the mantissa up to 12 dp. A None here means the magnitude is
            // far out of range; classify by sign so a huge negative reports
            // Underflow rather than Overflow (zero cannot reach this arm).
            match mantissa.checked_mul(10i128.pow(dp - scale)) {
                Some(v) => v,
                None => {
                    return Err(if mantissa < 0 {
                        DecimalError::Underflow
                    } else {
                        DecimalError::Overflow
                    });
                }
            }
        } else {
            // More than 12 dp: the dropped digits must be zero to be exact.
            let divisor = 10i128.pow(scale - dp);
            if mantissa % divisor != 0 {
                return Err(DecimalError::PrecisionLoss);
            }
            mantissa / divisor
        };
        if raw > Self::MAX.value {
            Err(DecimalError::Overflow)
        } else if raw < Self::MIN.value {
            Err(DecimalError::Underflow)
        } else {
            Ok(Self::from_raw(raw))
        }
    }

    /// Converts a [`rust_decimal::Decimal`] to a `D96`, rounding any decimals
    /// beyond 12 places with banker's rounding (round half to even).
    ///
    /// Only fails with `Overflow`/`Underflow` when the magnitude exceeds `D96`'s
    /// range; a value smaller than one ULP simply rounds to zero.
    pub fn from_rust_decimal_round(value: rust_decimal::Decimal) -> crate::Result<Self> {
        let mantissa = value.mantissa();
        let scale = value.scale();
        let dp = Self::DECIMALS as u32;
        let raw = if scale <= dp {
            // See `from_rust_decimal`: sign-classify an i128 overflow here.
            match mantissa.checked_mul(10i128.pow(dp - scale)) {
                Some(v) => v,
                None => {
                    return Err(if mantissa < 0 {
                        DecimalError::Underflow
                    } else {
                        DecimalError::Overflow
                    });
                }
            }
        } else {
            round_div_pow10_i128(mantissa, scale - dp)
        };
        if raw > Self::MAX.value {
            Err(DecimalError::Overflow)
        } else if raw < Self::MIN.value {
            Err(DecimalError::Underflow)
        } else {
            Ok(Self::from_raw(raw))
        }
    }
}

#[cfg(feature = "rust-decimal")]
impl From<D96> for rust_decimal::Decimal {
    /// Lossless conversion of a `D96` into a `rust_decimal::Decimal`.
    #[inline]
    fn from(value: D96) -> Self {
        value.to_rust_decimal()
    }
}

#[cfg(feature = "rust-decimal")]
impl TryFrom<rust_decimal::Decimal> for D96 {
    type Error = DecimalError;

    /// Exact conversion of a `rust_decimal::Decimal` to a `D96` (see
    /// [`D96::from_rust_decimal`]).
    #[inline]
    fn try_from(value: rust_decimal::Decimal) -> crate::Result<Self> {
        Self::from_rust_decimal(value)
    }
}

// ============================================================================
// Integer Conversions
// ============================================================================

impl D96 {
    /// Creates a D96 from an i128 integer.
    #[inline(always)]
    pub const fn from_i128(value: i128) -> Option<Self> {
        match value.checked_mul(Self::SCALE) {
            Some(scaled) => {
                // Check 96-bit bounds
                if scaled > Self::MAX.value || scaled < Self::MIN.value {
                    None
                } else {
                    Some(Self { value: scaled })
                }
            }
            None => None,
        }
    }

    /// Creates a D96 from an i64 integer.
    ///
    /// NOTE: this does NOT range-check. D96's integer range is only ≈±3.96e16,
    /// so `|value|` above that yields an out-of-96-bit (invalid) D96. For
    /// untrusted input use `<D96 as num_traits::FromPrimitive>::from_i64` (with
    /// the `num-traits` feature), which returns `None` when out of range.
    #[inline(always)]
    pub const fn from_i64(value: i64) -> Self {
        Self {
            value: value as i128 * Self::SCALE,
        }
    }

    /// Creates a D96 from an i32 integer (always succeeds).
    #[inline(always)]
    pub const fn from_i32(value: i32) -> Self {
        Self {
            value: value as i128 * Self::SCALE,
        }
    }

    /// Creates a D96 from a u32 integer (always succeeds).
    #[inline(always)]
    pub const fn from_u32(value: u32) -> Self {
        Self {
            value: value as i128 * Self::SCALE,
        }
    }

    /// Creates a D96 from a u64 integer.
    ///
    /// NOTE: this does NOT range-check (see [`from_i64`](Self::from_i64)). A
    /// `value` above ≈3.96e16 yields an out-of-96-bit (invalid) D96; use the
    /// `num-traits` `FromPrimitive::from_u64` for a checked conversion.
    #[inline(always)]
    pub const fn from_u64(value: u64) -> Self {
        Self {
            value: value as i128 * Self::SCALE,
        }
    }

    /// Creates a D96 from a u128 integer.
    #[inline(always)]
    pub const fn from_u128(value: u128) -> Option<Self> {
        const MAX_SAFE: u128 = 39_614_081_257_132_168_796_771_975_167 / D96::SCALE as u128;

        if value > MAX_SAFE {
            None
        } else {
            Some(Self {
                value: (value * Self::SCALE as u128) as i128,
            })
        }
    }

    /// Converts to i128, truncating any fractional part.
    #[inline(always)]
    pub const fn to_i128(self) -> i128 {
        self.value / Self::SCALE
    }

    /// Converts to i64, truncating any fractional part.
    /// Returns None if the value doesn't fit in i64.
    #[inline(always)]
    pub const fn to_i64(self) -> Option<i64> {
        let result = self.value / Self::SCALE;
        if result > i64::MAX as i128 || result < i64::MIN as i128 {
            None
        } else {
            Some(result as i64)
        }
    }

    /// Converts to i128, rounding to nearest (banker's rounding on ties).
    #[inline(always)]
    pub const fn to_i128_round(self) -> i128 {
        let quotient = self.value / Self::SCALE;
        let remainder = self.value % Self::SCALE;
        let half = Self::SCALE / 2;

        banker_round(quotient, remainder, half)
    }

    /// Creates a D96 from an i128, returning an error on overflow.
    #[inline(always)]
    pub const fn try_from_i128(value: i128) -> crate::Result<Self> {
        match Self::from_i128(value) {
            Some(v) => Ok(v),
            None => Err(DecimalError::Overflow),
        }
    }

    /// Creates a D96 from a u128, returning an error on overflow.
    #[inline(always)]
    pub const fn try_from_u128(value: u128) -> crate::Result<Self> {
        match Self::from_u128(value) {
            Some(v) => Ok(v),
            None => Err(DecimalError::Overflow),
        }
    }
}

// ============================================================================
// Float Conversions
// ============================================================================

impl D96 {
    /// Creates a D96 from an f64.
    ///
    /// Returns `None` if the value is NaN, infinite, or out of range.
    #[inline(always)]
    pub fn from_f64(value: f64) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }

        // NOTE: there is deliberately no cheap magnitude pre-guard here. A prior
        // `value.abs() > 39_614_081_257_132.0` guard was 1000x too small (that
        // constant is MAX.value's leading digits, not the decimal max of
        // ~3.96e16) and wrongly rejected representable values. The `scaled`
        // bounds plus the post-round 96-bit check below are exact.
        let scaled = value * Self::SCALE as f64;

        if scaled > Self::MAX.value as f64 || scaled < Self::MIN.value as f64 {
            return None;
        }

        let result = round_half_away_f64(scaled) as i128;

        // CRITICAL: Final 96-bit bounds check (catches rounding up past MAX).
        if result > Self::MAX.value || result < Self::MIN.value {
            return None;
        }

        Some(Self { value: result })
    }

    /// Converts to f64.
    ///
    /// Note: the result is only approximately the nearest `f64`. Because it is
    /// reconstructed from separate integer and fractional parts, it can differ
    /// from the exact decimal value by up to ~1 ULP at any magnitude (not only
    /// for very large values).
    #[inline(always)]
    pub fn to_f64(self) -> f64 {
        let integer_part = self.value / Self::SCALE;
        let fractional_part = (self.value % Self::SCALE) as f64 / Self::SCALE as f64;
        integer_part as f64 + fractional_part
    }

    /// Creates a D96 from an f32.
    #[inline(always)]
    pub fn from_f32(value: f32) -> Option<Self> {
        Self::from_f64(value as f64)
    }

    /// Converts to f32.
    ///
    /// Note: May lose precision.
    #[inline(always)]
    pub fn to_f32(self) -> f32 {
        self.to_f64() as f32
    }

    /// Creates a D96 from an f64, returning an error if invalid.
    #[inline(always)]
    pub fn try_from_f64(value: f64) -> crate::Result<Self> {
        if value.is_nan() || value.is_infinite() {
            return Err(DecimalError::InvalidFormat);
        }

        // No 1000x-too-small magnitude pre-guard (see `from_f64`); the `scaled`
        // and post-round 96-bit checks below are exact and sign-aware.
        let scaled = value * Self::SCALE as f64;

        if scaled > Self::MAX.value as f64 {
            return Err(DecimalError::Overflow);
        }
        if scaled < Self::MIN.value as f64 {
            return Err(DecimalError::Underflow);
        }

        let result = round_half_away_f64(scaled) as i128;

        if result > Self::MAX.value {
            return Err(DecimalError::Overflow);
        }
        if result < Self::MIN.value {
            return Err(DecimalError::Underflow);
        }

        Ok(Self { value: result })
    }
}

// ============================================================================
// Percentage Calculations
// ============================================================================

impl D96 {
    /// Calculate percentage: self * (percent / 100)
    #[inline(always)]
    pub fn percent_of(self, percent: Self) -> Option<Self> {
        self.checked_mul(percent)?.checked_div(Self::HUNDRED)
    }

    /// Add percentage: self * (1 + percent/100)
    #[inline(always)]
    pub fn add_percent(self, percent: Self) -> Option<Self> {
        let multiplier = Self::HUNDRED.checked_add(percent)?;
        self.checked_mul(multiplier)?.checked_div(Self::HUNDRED)
    }
}

// ============================================================================
// String Parsing
// ============================================================================

impl D96 {
    /// Parses a decimal string into a D96.
    ///
    /// Supports formats like: "123", "123.45", "-123.45", "0.000000000001"
    /// Fast parsing using SWAR (SIMD Within A Register) for both integer and fractional parts
    pub fn from_str_exact(s: &str) -> crate::Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            return Err(DecimalError::InvalidFormat);
        }

        let bytes = s.as_bytes();

        // Scientific / E-notation: dispatch to a dedicated digit-level parser.
        if bytes.iter().any(|&b| b == b'e' || b == b'E') {
            return Self::from_scientific_bytes(bytes);
        }

        // Quick length check
        if bytes.len() > 48 {
            return Err(DecimalError::InvalidFormat);
        }

        let (is_negative, start) = match bytes[0] {
            b'-' => (true, 1),
            b'+' => (false, 1),
            _ => (false, 0),
        };

        if start >= bytes.len() {
            return Err(DecimalError::InvalidFormat);
        }

        // Find decimal point
        let mut decimal_idx = None;
        for (i, &b) in bytes[start..].iter().enumerate() {
            if b == b'.' {
                decimal_idx = Some(start + i);
                break;
            }
        }

        let int_end = decimal_idx.unwrap_or(bytes.len());
        let int_bytes = &bytes[start..int_end];

        // Parse integer part with SWAR
        let integer_part = parse_integer_swar(int_bytes)?;

        // Parse fractional part
        let fractional_value = if let Some(dp_idx) = decimal_idx {
            let frac_bytes = &bytes[dp_idx + 1..];

            // A trailing dot ("1.") is valid: the fractional part is empty (= 0),
            // matching the scientific path and rust_decimal. A lone "." with no
            // integer digits stays invalid.
            if frac_bytes.is_empty() && int_bytes.is_empty() {
                return Err(DecimalError::InvalidFormat);
            }

            // Trailing zeros are insignificant: "1.230000000000000" is exactly
            // 1.23, not precision loss. Trim them before the precision check so
            // only a SIGNIFICANT digit past 12 dp is rejected.
            let mut frac_end = frac_bytes.len();
            while frac_end > 0 && frac_bytes[frac_end - 1] == b'0' {
                frac_end -= 1;
            }
            let frac_bytes = &frac_bytes[..frac_end];

            if frac_bytes.len() > Self::DECIMALS as usize {
                return Err(DecimalError::PrecisionLoss);
            }

            // Parse the fractional digits (empty after trimming -> 0)
            let frac_digits = parse_integer_swar(frac_bytes)?;

            // Scale to full precision
            let remaining_decimals = Self::DECIMALS as usize - frac_bytes.len();
            (frac_digits as u128) * fast_pow10(remaining_decimals as u8)
        } else {
            0
        };

        // Combine and validate
        let int_scaled = integer_part
            .checked_mul(Self::SCALE)
            .ok_or(DecimalError::Overflow)?;

        let abs_value = int_scaled
            .checked_add(fractional_value as i128)
            .ok_or(DecimalError::Overflow)?;

        let value = if is_negative {
            abs_value.checked_neg().ok_or(DecimalError::Overflow)?
        } else {
            abs_value
        };

        if value > Self::MAX.value || value < Self::MIN.value {
            return Err(DecimalError::Overflow);
        }

        Ok(Self { value })
    }

    /// Digit-level parser for scientific / E-notation (`"1.5e3"`, `"2.5E-7"`).
    ///
    /// Computes `magnitude = M * 10^(exp - frac_len + DECIMALS)` directly from
    /// the significand digits, so a positive exponent can rescue an otherwise
    /// over-precise mantissa. Allocation free, so it stays usable in `no_std`.
    fn from_scientific_bytes(bytes: &[u8]) -> crate::Result<Self> {
        let e_pos = bytes
            .iter()
            .position(|&b| b == b'e' || b == b'E')
            .ok_or(DecimalError::InvalidFormat)?;
        let mantissa = &bytes[..e_pos];
        let exp_bytes = &bytes[e_pos + 1..];

        // --- exponent (signed integer) ---
        if exp_bytes.is_empty() {
            return Err(DecimalError::InvalidFormat);
        }
        let (exp_neg, exp_start) = match exp_bytes[0] {
            b'-' => (true, 1),
            b'+' => (false, 1),
            _ => (false, 0),
        };
        if exp_start >= exp_bytes.len() {
            return Err(DecimalError::InvalidFormat);
        }
        let mut exp: i64 = 0;
        for &b in &exp_bytes[exp_start..] {
            let digit = b.wrapping_sub(b'0');
            if digit > 9 {
                return Err(DecimalError::InvalidFormat);
            }
            // Cap the magnitude: anything past 10^6 is unconditionally out of
            // range / precision-lost, and the cap keeps `net` within i64.
            exp = (exp * 10 + digit as i64).min(1_000_000);
        }
        let exp = if exp_neg { -exp } else { exp };

        // --- mantissa: magnitude M and number of fractional digits ---
        if mantissa.is_empty() {
            return Err(DecimalError::InvalidFormat);
        }
        let (is_negative, mant_start) = match mantissa[0] {
            b'-' => (true, 1),
            b'+' => (false, 1),
            _ => (false, 0),
        };
        if mant_start >= mantissa.len() {
            return Err(DecimalError::InvalidFormat);
        }
        let mut m: i128 = 0;
        let mut frac_len: i64 = 0;
        let mut seen_dot = false;
        let mut seen_digit = false;
        for &b in &mantissa[mant_start..] {
            if b == b'.' {
                if seen_dot {
                    return Err(DecimalError::InvalidFormat);
                }
                seen_dot = true;
                continue;
            }
            let digit = b.wrapping_sub(b'0');
            if digit > 9 {
                return Err(DecimalError::InvalidFormat);
            }
            seen_digit = true;
            m = m
                .checked_mul(10)
                .and_then(|v| v.checked_add(digit as i128))
                .ok_or(DecimalError::Overflow)?;
            if seen_dot {
                frac_len += 1;
            }
        }
        if !seen_digit {
            return Err(DecimalError::InvalidFormat);
        }
        if m == 0 {
            return Ok(Self::ZERO); // 0eN == 0 regardless of the exponent
        }

        // raw magnitude = M * 10^(exp - frac_len + DECIMALS)
        let net = exp - frac_len + Self::DECIMALS as i64;
        let magnitude: i128 = if net >= 0 {
            if net > 38 {
                return Err(DecimalError::Overflow); // 10^net exceeds i128 (m != 0)
            }
            m.checked_mul(10i128.pow(net as u32))
                .ok_or(DecimalError::Overflow)?
        } else {
            let k = -net;
            if k > 38 {
                return Err(DecimalError::PrecisionLoss); // |value| below one ULP, nonzero
            }
            let div = 10i128.pow(k as u32);
            if m % div != 0 {
                return Err(DecimalError::PrecisionLoss);
            }
            m / div
        };

        // Apply sign and validate against the 96-bit range.
        let value = if is_negative {
            magnitude.checked_neg().ok_or(DecimalError::Overflow)?
        } else {
            magnitude
        };
        if !(Self::MIN.value..=Self::MAX.value).contains(&value) {
            return Err(DecimalError::Overflow);
        }
        Ok(Self { value })
    }

    /// Parse a string, rounding to 12 decimal places if necessary
    ///
    /// Unlike `from_str_exact`, this will succeed even if the input has more than
    /// 12 decimal places, rounding the excess digits using banker's rounding.
    pub fn from_str_lossy(s: &str) -> crate::Result<Self> {
        let s = s.trim();

        if s.is_empty() {
            return Err(DecimalError::InvalidFormat);
        }

        let bytes = s.as_bytes();

        if bytes.len() > 48 {
            return Err(DecimalError::InvalidFormat);
        }

        let (is_negative, start) = match bytes[0] {
            b'-' => (true, 1),
            b'+' => (false, 1),
            _ => (false, 0),
        };

        if start >= bytes.len() {
            return Err(DecimalError::InvalidFormat);
        }

        // Find decimal point
        let mut decimal_idx = None;
        for (i, &b) in bytes[start..].iter().enumerate() {
            if b == b'.' {
                decimal_idx = Some(start + i);
                break;
            }
        }

        let int_end = decimal_idx.unwrap_or(bytes.len());
        let int_bytes = &bytes[start..int_end];

        // Parse integer part
        let integer_part = parse_integer_swar(int_bytes)?;

        // Parse fractional part with rounding
        let fractional_value = if let Some(dp_idx) = decimal_idx {
            let frac_bytes = &bytes[dp_idx + 1..];

            // A trailing dot ("1.") is valid (empty fractional = 0), matching
            // the scientific path / rust_decimal; a lone "." is invalid.
            if frac_bytes.is_empty() && int_bytes.is_empty() {
                return Err(DecimalError::InvalidFormat);
            }

            let frac_len = frac_bytes.len();

            if frac_len <= Self::DECIMALS as usize {
                // Fast path: fits exactly, no rounding needed
                let frac_digits = parse_integer_swar(frac_bytes)?;
                let remaining_decimals = Self::DECIMALS as usize - frac_len;
                (frac_digits as u128) * fast_pow10(remaining_decimals as u8)
            } else {
                // Slow path: need to round
                // Parse all digits we can use (DECIMALS)
                let usable_frac = &frac_bytes[..Self::DECIMALS as usize];
                let mut frac_digits = parse_integer_swar(usable_frac)?;

                // Get the next digit for rounding
                let next_digit = frac_bytes[Self::DECIMALS as usize].wrapping_sub(b'0');

                if next_digit > 9 {
                    return Err(DecimalError::InvalidFormat);
                }

                // Validate every remaining trailing byte regardless of the
                // rounding branch. Without this, the >5 and <5 branches never
                // scan the tail, so garbage like "1.0000000000001x" was parsed.
                for &b in &frac_bytes[Self::DECIMALS as usize + 1..] {
                    if b.wrapping_sub(b'0') > 9 {
                        return Err(DecimalError::InvalidFormat);
                    }
                }

                // Banker's rounding (round half to even)
                if next_digit > 5 {
                    // Round up
                    frac_digits += 1;
                } else if next_digit == 5 {
                    // Check if there are more non-zero digits
                    let mut has_more = false;
                    for &b in &frac_bytes[Self::DECIMALS as usize + 1..] {
                        let d = b.wrapping_sub(b'0');
                        if d > 9 {
                            return Err(DecimalError::InvalidFormat);
                        }
                        if d != 0 {
                            has_more = true;
                            break;
                        }
                    }

                    if has_more {
                        // Round up
                        frac_digits += 1;
                    } else {
                        // Exactly half - round to even
                        if frac_digits % 2 == 1 {
                            frac_digits += 1;
                        }
                    }
                }
                // next_digit < 5: round down (do nothing)

                frac_digits as u128
            }
        } else {
            0
        };

        // Combine parts
        let int_scaled = integer_part
            .checked_mul(Self::SCALE)
            .ok_or(DecimalError::Overflow)?;

        let abs_value = int_scaled
            .checked_add(fractional_value as i128)
            .ok_or(DecimalError::Overflow)?;

        // Apply sign
        let value = if is_negative {
            abs_value.checked_neg().ok_or(DecimalError::Overflow)?
        } else {
            abs_value
        };

        // Check 96-bit bounds
        if value > Self::MAX.value || value < Self::MIN.value {
            return Err(DecimalError::Overflow);
        }

        Ok(Self { value })
    }

    /// Parse from fixed-point string (no decimal point)
    pub fn from_fixed_point_str(s: &str, decimals: u8) -> crate::Result<Self> {
        let value = s.parse::<i128>().map_err(|_| DecimalError::InvalidFormat)?;

        if decimals > Self::DECIMALS {
            return Err(DecimalError::PrecisionLoss);
        }

        let scale_diff = Self::DECIMALS - decimals;
        let multiplier = const_pow10_i128(scale_diff);

        let scaled = value
            .checked_mul(multiplier)
            .ok_or(DecimalError::Overflow)?;

        // CRITICAL: Check 96-bit bounds
        if scaled > Self::MAX.value || scaled < Self::MIN.value {
            return Err(DecimalError::Overflow);
        }

        Ok(Self { value: scaled })
    }
}

impl FromStr for D96 {
    type Err = DecimalError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_exact(s)
    }
}

/// Parse digits using SWAR (processes 4 digits at a time)
#[inline(always)]
fn parse_integer_swar(bytes: &[u8]) -> crate::Result<i128> {
    let mut result = 0i128;
    let mut i = 0;

    // Process 4 digits at a time
    while i + 3 < bytes.len() {
        // Read 4 bytes in order: most significant first
        let b0 = bytes[i];
        let b1 = bytes[i + 1];
        let b2 = bytes[i + 2];
        let b3 = bytes[i + 3];

        // Check all are ASCII digits
        let d0 = b0.wrapping_sub(b'0');
        let d1 = b1.wrapping_sub(b'0');
        let d2 = b2.wrapping_sub(b'0');
        let d3 = b3.wrapping_sub(b'0');

        if d0 > 9 || d1 > 9 || d2 > 9 || d3 > 9 {
            return Err(DecimalError::InvalidFormat);
        }

        // Combine with checked arithmetic: the 48-byte length cap admits up to
        // 48 integer digits, far beyond i128 (~39 digits), so an unchecked
        // `* 10000` would panic in debug / wrap silently in release.
        let chunk = (d0 as i128) * 1000 + (d1 as i128) * 100 + (d2 as i128) * 10 + (d3 as i128);
        result = result
            .checked_mul(10000)
            .and_then(|v| v.checked_add(chunk))
            .ok_or(DecimalError::Overflow)?;
        i += 4;
    }

    // Handle remaining 1-3 digits
    while i < bytes.len() {
        let digit = bytes[i].wrapping_sub(b'0');
        if digit > 9 {
            return Err(DecimalError::InvalidFormat);
        }
        result = result
            .checked_mul(10)
            .and_then(|v| v.checked_add(digit as i128))
            .ok_or(DecimalError::Overflow)?;
        i += 1;
    }

    Ok(result)
}

/// Fast power of 10 computation using bit shifts where possible
#[inline(always)]
const fn fast_pow10(n: u8) -> u128 {
    // For small powers, use precomputed constants
    match n {
        0 => 1,
        1 => 10,
        2 => 100,
        3 => 1_000,
        4 => 10_000,
        5 => 100_000,
        6 => 1_000_000,
        7 => 10_000_000,
        8 => 100_000_000,
        9 => 1_000_000_000,
        10 => 10_000_000_000,
        11 => 100_000_000_000,
        12 => 1_000_000_000_000,
        _ => panic!("fast_pow10: exponent too large"),
    }
}

// ============================================================================
// Bytes Operations
// ============================================================================

impl D96 {
    /// The size of this type in bytes.
    pub const BYTES: usize = core::mem::size_of::<i128>();

    /// Parse from byte slice
    pub fn from_utf8_bytes(bytes: &[u8]) -> crate::Result<Self> {
        let s = core::str::from_utf8(bytes).map_err(|_| DecimalError::InvalidFormat)?;
        Self::from_str_exact(s) // ✅ This will check bounds
    }

    /// Read D96 from little-endian bytes
    #[inline(always)]
    pub const fn from_le_bytes(bytes: [u8; Self::BYTES]) -> Self {
        let value = i128::from_le_bytes(bytes);

        // Debug assertion to catch violations during development
        assert!(
            value <= Self::MAX.value && value >= Self::MIN.value,
            "D96::from_le_bytes: value exceeds 96-bit range"
        );

        Self { value }
    }

    /// Read D96 from big-endian bytes
    #[inline(always)]
    pub const fn from_be_bytes(bytes: [u8; Self::BYTES]) -> Self {
        let value = i128::from_be_bytes(bytes);

        // Debug assertion to catch violations during development
        assert!(
            value <= Self::MAX.value && value >= Self::MIN.value,
            "D96::from_be_bytes: value exceeds 96-bit range"
        );

        Self { value }
    }

    /// Read D96 from native-endian bytes
    #[inline(always)]
    pub const fn from_ne_bytes(bytes: [u8; Self::BYTES]) -> Self {
        let value = i128::from_ne_bytes(bytes);

        // Debug assertion to catch violations during development
        assert!(
            value <= Self::MAX.value && value >= Self::MIN.value,
            "D96::from_ne_bytes: value exceeds 96-bit range"
        );

        Self { value }
    }

    /// Read D96 from a byte slice (little-endian)
    #[inline(always)]
    pub const fn read_le_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() >= Self::BYTES, "buffer too short");

        let mut array = [0u8; Self::BYTES];
        let mut i = 0;
        while i < Self::BYTES {
            array[i] = bytes[i];
            i += 1;
        }

        Self::from_le_bytes(array)
    }

    /// Read D96 from a byte slice (big-endian)
    #[inline(always)]
    pub const fn read_be_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() >= Self::BYTES, "buffer too short");

        let mut array = [0u8; Self::BYTES];
        let mut i = 0;
        while i < Self::BYTES {
            array[i] = bytes[i];
            i += 1;
        }

        Self::from_be_bytes(array)
    }

    /// Read D96 from a byte slice (native-endian)
    #[inline(always)]
    pub const fn read_ne_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() >= Self::BYTES, "buffer too short");

        let mut array = [0u8; Self::BYTES];
        let mut i = 0;
        while i < Self::BYTES {
            array[i] = bytes[i];
            i += 1;
        }

        Self::from_ne_bytes(array)
    }

    /// Try to read D96 from a byte slice (little-endian), checking bounds
    #[inline(always)]
    pub const fn try_read_le_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::BYTES {
            return None;
        }

        let mut array = [0u8; Self::BYTES];
        let mut i = 0;
        while i < Self::BYTES {
            array[i] = bytes[i];
            i += 1;
        }

        let value = i128::from_le_bytes(array);

        // Check 96-bit bounds
        if value > Self::MAX.value || value < Self::MIN.value {
            return None;
        }

        Some(Self { value })
    }

    /// Try to read D96 from a byte slice (big-endian), checking bounds
    #[inline(always)]
    pub const fn try_read_be_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::BYTES {
            return None;
        }

        let mut array = [0u8; Self::BYTES];
        let mut i = 0;
        while i < Self::BYTES {
            array[i] = bytes[i];
            i += 1;
        }

        let value = i128::from_be_bytes(array);

        // Check 96-bit bounds
        if value > Self::MAX.value || value < Self::MIN.value {
            return None;
        }

        Some(Self { value })
    }

    /// Try to read D96 from a byte slice (native-endian), checking bounds
    #[inline(always)]
    pub const fn try_read_ne_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::BYTES {
            return None;
        }

        let mut array = [0u8; Self::BYTES];
        let mut i = 0;
        while i < Self::BYTES {
            array[i] = bytes[i];
            i += 1;
        }

        let value = i128::from_ne_bytes(array);

        // Check 96-bit bounds
        if value > Self::MAX.value || value < Self::MIN.value {
            return None;
        }

        Some(Self { value })
    }

    /// Returns the memory representation as a byte array in big-endian byte order.
    #[inline(always)]
    pub const fn to_be_bytes(self) -> [u8; 16] {
        self.value.to_be_bytes()
    }

    /// Returns the memory representation as a byte array in little-endian byte order.
    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; 16] {
        self.value.to_le_bytes()
    }

    /// Returns the memory representation as a byte array in native byte order.
    #[inline(always)]
    pub const fn to_ne_bytes(self) -> [u8; 16] {
        self.value.to_ne_bytes()
    }

    /// Writes the decimal as bytes in little-endian order.
    #[inline(always)]
    pub fn write_le_bytes(&self, buf: &mut [u8]) {
        buf[..16].copy_from_slice(&self.to_le_bytes());
    }

    /// Writes the decimal as bytes in big-endian order.
    #[inline(always)]
    pub fn write_be_bytes(&self, buf: &mut [u8]) {
        buf[..16].copy_from_slice(&self.to_be_bytes());
    }

    /// Writes the decimal as bytes in native-endian order.
    #[inline(always)]
    pub fn write_ne_bytes(&self, buf: &mut [u8]) {
        buf[..16].copy_from_slice(&self.to_ne_bytes());
    }

    /// Tries to write the decimal as bytes in little-endian order.
    #[inline(always)]
    pub fn try_write_le_bytes(&self, buf: &mut [u8]) -> Option<()> {
        if buf.len() < 16 {
            return None;
        }
        buf[..16].copy_from_slice(&self.to_le_bytes());
        Some(())
    }

    /// Tries to write the decimal as bytes in big-endian order.
    #[inline(always)]
    pub fn try_write_be_bytes(&self, buf: &mut [u8]) -> Option<()> {
        if buf.len() < 16 {
            return None;
        }
        buf[..16].copy_from_slice(&self.to_be_bytes());
        Some(())
    }

    /// Tries to write the decimal as bytes in native-endian order.
    #[inline(always)]
    pub fn try_write_ne_bytes(&self, buf: &mut [u8]) -> Option<()> {
        if buf.len() < 16 {
            return None;
        }
        buf[..16].copy_from_slice(&self.to_ne_bytes());
        Some(())
    }
}

// ============================================================================
// Operator Overloading
// ============================================================================

impl Add for D96 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs).expect("attempt to add with overflow")
    }
}

impl Sub for D96 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub(rhs)
            .expect("attempt to subtract with overflow")
    }
}

impl Mul for D96 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        self.checked_mul(rhs)
            .expect("attempt to multiply with overflow")
    }
}

impl Div for D96 {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        self.checked_div(rhs)
            .expect("attempt to divide by zero or overflow")
    }
}

impl Rem for D96 {
    type Output = Self;

    /// Remainder. Panics if `rhs` is zero (like integer `%`).
    #[inline(always)]
    fn rem(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value % rhs.value,
        }
    }
}

impl Neg for D96 {
    type Output = Self;

    #[inline(always)]
    fn neg(self) -> Self::Output {
        self.checked_neg().expect("attempt to negate with overflow")
    }
}

impl AddAssign for D96 {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for D96 {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl MulAssign for D96 {
    #[inline(always)]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl DivAssign for D96 {
    #[inline(always)]
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

impl RemAssign for D96 {
    #[inline(always)]
    fn rem_assign(&mut self, rhs: Self) {
        *self = *self % rhs;
    }
}

// ============================================================================
// Standard Library Trait Implementations
// ============================================================================

impl TryFrom<i128> for D96 {
    type Error = DecimalError;

    #[inline(always)]
    fn try_from(value: i128) -> crate::Result<Self> {
        Self::try_from_i128(value)
    }
}

impl TryFrom<u128> for D96 {
    type Error = DecimalError;

    #[inline(always)]
    fn try_from(value: u128) -> crate::Result<Self> {
        Self::try_from_u128(value)
    }
}

impl TryFrom<f64> for D96 {
    type Error = DecimalError;

    #[inline(always)]
    fn try_from(value: f64) -> crate::Result<Self> {
        Self::try_from_f64(value)
    }
}

impl TryFrom<f32> for D96 {
    type Error = DecimalError;

    #[inline(always)]
    fn try_from(value: f32) -> crate::Result<Self> {
        Self::try_from_f64(value as f64)
    }
}

// i64 / u64 are NOT total conversions: D96's integer range (~±3.96e16) is
// smaller than i64/u64, so a `From` impl would have to either panic or mint an
// out-of-96-bit value. We expose fallible `TryFrom` instead (mirroring D64).
impl TryFrom<i64> for D96 {
    type Error = DecimalError;

    #[inline(always)]
    fn try_from(value: i64) -> crate::Result<Self> {
        Self::try_from_i128(value as i128)
    }
}

impl TryFrom<u64> for D96 {
    type Error = DecimalError;

    #[inline(always)]
    fn try_from(value: u64) -> crate::Result<Self> {
        Self::try_from_i128(value as i128)
    }
}

impl From<i32> for D96 {
    #[inline(always)]
    fn from(value: i32) -> Self {
        Self::from_i32(value)
    }
}

impl From<u32> for D96 {
    #[inline(always)]
    fn from(value: u32) -> Self {
        Self::from_u32(value)
    }
}

impl From<i16> for D96 {
    #[inline(always)]
    fn from(value: i16) -> Self {
        Self::from_i32(value as i32)
    }
}

impl From<u16> for D96 {
    #[inline(always)]
    fn from(value: u16) -> Self {
        Self::from_u32(value as u32)
    }
}

impl From<i8> for D96 {
    #[inline(always)]
    fn from(value: i8) -> Self {
        Self::from_i32(value as i32)
    }
}

impl From<u8> for D96 {
    #[inline(always)]
    fn from(value: u8) -> Self {
        Self::from_u32(value as u32)
    }
}

impl D96 {
    /// Format to a byte buffer, returns length written
    /// Uses reciprocal multiplication for fast division by powers of 10
    #[inline]
    fn format_to_buffer(&self, buffer: &mut [u8]) -> usize {
        let abs_value = self.value.unsigned_abs();
        let integer_part = abs_value / Self::SCALE as u128;
        let fractional_part = abs_value % Self::SCALE as u128;

        let mut pos = 0;

        // Sign
        if self.value < 0 {
            buffer[pos] = b'-';
            pos += 1;
        }

        // Integer part
        if integer_part == 0 {
            buffer[pos] = b'0';
            pos += 1;
        } else {
            pos += format_u128_reciprocal(integer_part, &mut buffer[pos..]);
        }

        // Fractional part
        if fractional_part > 0 {
            buffer[pos] = b'.';
            pos += 1;

            // Format fractional with trailing zero removal
            pos += format_fractional_reciprocal(fractional_part, &mut buffer[pos..]);
        }

        pos
    }

    /// Format with specific precision (for fmt::Formatter precision)
    #[inline]
    fn fmt_with_precision(&self, f: &mut fmt::Formatter<'_>, precision: usize) -> fmt::Result {
        let abs_value = self.value.unsigned_abs();
        let precision_capped = precision.min(Self::DECIMALS as usize);

        // Round the COMBINED value to `precision_capped` fractional digits (not
        // the fractional part in isolation) so a rounding carry out of the
        // fractional field propagates into the integer part. Rounding only the
        // fractional part dropped that carry (e.g. 0.95 at {:.1} -> "0.0").
        let scale_down = Self::DECIMALS as u32 - precision_capped as u32;
        let divisor = const_pow10_u128(scale_down as u8);
        let scaled_value = abs_value / divisor;
        let remainder = abs_value % divisor;

        // Banker's rounding (round half to even).
        let rounded_value = if remainder * 2 > divisor {
            scaled_value + 1
        } else if remainder * 2 == divisor {
            if scaled_value % 2 == 1 { scaled_value + 1 } else { scaled_value }
        } else {
            scaled_value
        };

        let precision_scale = const_pow10_u128(precision_capped as u8);
        let int_part = rounded_value / precision_scale;
        let frac_part = rounded_value % precision_scale;

        let mut buffer = [0u8; 64];
        let mut pos = 0;

        // Sign
        if self.value < 0 {
            buffer[pos] = b'-';
            pos += 1;
        }

        // Integer part
        if int_part == 0 {
            buffer[pos] = b'0';
            pos += 1;
        } else {
            pos += format_u128_reciprocal(int_part, &mut buffer[pos..]);
        }

        // Fractional part (already rounded; carry has propagated into int_part)
        if precision > 0 {
            buffer[pos] = b'.';
            pos += 1;
            pos += format_fractional_fixed_width(frac_part, precision_capped, &mut buffer[pos..]);
        }

        // The buffer is always ASCII (digits, '.', '-'), so this never errors.
        let s = core::str::from_utf8(&buffer[..pos]).unwrap();
        f.write_str(s)
    }
}

impl fmt::Display for D96 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(precision) = f.precision() {
            return self.fmt_with_precision(f, precision);
        }

        // Fast path for zero
        if self.value == 0 {
            return f.write_str("0");
        }

        let mut buffer = [0u8; 48];
        let len = self.format_to_buffer(&mut buffer);
        // The buffer is always ASCII (digits, '.', '-'), so this never errors.
        let s = core::str::from_utf8(&buffer[..len]).unwrap();
        f.write_str(s)
    }
}

// ============================================================================
// Helpers for digit extraction during formatting
// ============================================================================

/// Exact division by 100, used to emit two decimal digits at a time.
///
/// History: this previously used a 73-bit round-down reciprocal
/// (`floor(2^73 / 100)`) with `wrapping_mul`. That constant carries far too few
/// fractional bits, so the approximation drifts badly for large operands and
/// `wrapping_mul` could even discard high bits — producing non-digit bytes in
/// the output (e.g. formatting `20000000000.0` yielded `"19949B1>960"`). Native
/// division by the constant 100 is exact.
#[inline(always)]
const fn div100_u128(n: u128) -> u128 {
    n / 100
}

/// Format fractional part with exact fixed width (always write exactly `width` digits)
#[inline]
fn format_fractional_fixed_width(mut frac: u128, width: usize, buffer: &mut [u8]) -> usize {
    // Write digits from right to left into temp buffer
    let mut temp = [0u8; 20];

    for i in 0..width {
        temp[width - 1 - i] = b'0' + (frac % 10) as u8;
        frac /= 10;
    }

    // Copy to output buffer
    for i in 0..width {
        buffer[i] = temp[i];
    }

    width
}

// ============================================================================
// Formatting Functions Using Reciprocal Multiplication
// ============================================================================

/// Format u128 using reciprocal multiplication
#[inline]
fn format_u128_reciprocal(mut n: u128, buffer: &mut [u8]) -> usize {
    let mut temp = [0u8; 40]; // Max 39 digits for u128
    let mut len = 0;

    // Process pairs of digits using reciprocal division
    while n >= 100 {
        let q = div100_u128(n);
        let r = (n - q * 100) as u8;

        // Write two digits
        temp[len] = b'0' + (r % 10);
        temp[len + 1] = b'0' + (r / 10);
        len += 2;

        n = q;
    }

    // Last 1-2 digits
    if n >= 10 {
        let r = n as u8;
        temp[len] = b'0' + (r % 10);
        temp[len + 1] = b'0' + (r / 10);
        len += 2;
    } else if n > 0 {
        temp[len] = b'0' + n as u8;
        len += 1;
    }

    // Reverse into output buffer
    for i in 0..len {
        buffer[i] = temp[len - 1 - i];
    }

    len
}

/// Format fractional part with trailing zero removal
#[inline]
fn format_fractional_reciprocal(frac: u128, buffer: &mut [u8]) -> usize {
    // Convert to 12-digit string with leading zeros, then strip trailing zeros
    let mut digits = [0u8; 12];
    let mut temp = frac;

    // Extract all 12 digits
    for i in (0..12).rev() {
        digits[i] = (temp % 10) as u8;
        temp /= 10;
    }

    // Find last non-zero digit
    let mut last_nonzero = 0;
    for i in 0..12 {
        if digits[i] != 0 {
            last_nonzero = i;
        }
    }

    // Write digits up to last non-zero
    for i in 0..=last_nonzero {
        buffer[i] = b'0' + digits[i];
    }

    last_nonzero + 1
}

/// Compute 10^n at compile time (for u128)
#[inline(always)]
const fn const_pow10_u128(n: u8) -> u128 {
    match n {
        0 => 1,
        1 => 10,
        2 => 100,
        3 => 1_000,
        4 => 10_000,
        5 => 100_000,
        6 => 1_000_000,
        7 => 10_000_000,
        8 => 100_000_000,
        9 => 1_000_000_000,
        10 => 10_000_000_000,
        11 => 100_000_000_000,
        12 => 1_000_000_000_000,
        _ => panic!("pow10: exponent too large"),
    }
}

impl fmt::Debug for D96 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            f.debug_struct("D96").field("value", &self.value).finish()
        } else {
            write!(f, "D96({})", self)
        }
    }
}

// ============================================================================
// Iterator Trait Implementations
// ============================================================================

impl Sum for D96 {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |acc, x| acc + x)
    }
}

impl<'a> Sum<&'a D96> for D96 {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |acc, x| acc + *x)
    }
}

impl Product for D96 {
    fn product<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ONE, |acc, x| acc * x)
    }
}

impl<'a> Product<&'a D96> for D96 {
    fn product<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Self::ONE, |acc, x| acc * *x)
    }
}

// ============================================================================
// Serde Support
// ============================================================================

#[cfg(feature = "serde")]
impl Serialize for D96 {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            // JSON, TOML, etc. - use string representation
            serializer.collect_str(self)
        } else {
            // Bincode, MessagePack, etc. - serialize raw i128
            self.value.serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for D96 {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            // JSON, TOML, etc. - parse from string
            // Use visitor pattern to avoid allocation
            struct D96Visitor;

            impl<'de> de::Visitor<'de> for D96Visitor {
                type Value = D96;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a decimal number string")
                }

                fn visit_str<E>(self, v: &str) -> core::result::Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    D96::from_str_exact(v).map_err(de::Error::custom)
                }

                fn visit_borrowed_str<E>(self, v: &'de str) -> core::result::Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    D96::from_str_exact(v).map_err(de::Error::custom)
                }

                fn visit_string<E>(
                    self,
                    v: alloc::string::String,
                ) -> core::result::Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    D96::from_str_exact(&v).map_err(de::Error::custom)
                }

                fn visit_i64<E>(self, v: i64) -> core::result::Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    match (v as i128).checked_mul(D96::SCALE) {
                        Some(scaled) => D96::from_raw_checked(scaled)
                            .ok_or_else(|| de::Error::custom("integer out of D96 range")),
                        None => Err(de::Error::custom("integer out of D96 range")),
                    }
                }

                fn visit_u64<E>(self, v: u64) -> core::result::Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    match (v as i128).checked_mul(D96::SCALE) {
                        Some(scaled) => D96::from_raw_checked(scaled)
                            .ok_or_else(|| de::Error::custom("integer out of D96 range")),
                        None => Err(de::Error::custom("integer out of D96 range")),
                    }
                }

                fn visit_f64<E>(self, v: f64) -> core::result::Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    // Float inputs are rounded to 12 dp; the canonical form is a string.
                    D96::from_f64(v).ok_or_else(|| de::Error::custom("f64 not representable as D96"))
                }
            }

            deserializer.deserialize_any(D96Visitor)
        } else {
            // Bincode, MessagePack, etc. - deserialize raw i128
            let value = i128::deserialize(deserializer)?;

            // CRITICAL: Validate 96-bit bounds
            if value > Self::MAX.value || value < Self::MIN.value {
                return Err(de::Error::custom("value exceeds D96 96-bit range"));
            }

            Ok(Self { value })
        }
    }
}

// ============================================================================
// bytemuck Pod / Zeroable (zero-copy reinterpretation)
// ============================================================================

// SAFETY: `D96` is `#[repr(transparent)]` over `i128` (itself `Pod`), so it has
// identical layout with no padding bytes, and every bit pattern is a valid
// *i128*. D96's 96-bit range is a logical/API invariant only — NO `unsafe` code
// in this crate relies on it for memory safety — so Pod is sound even though
// zero-copy construction can yield a logically-out-of-96-bit value (which would
// be rejected by `from_raw`). The all-zero bit pattern is `D96::ZERO`, so
// `Zeroable` holds as well.
#[cfg(feature = "bytemuck")]
unsafe impl bytemuck::Zeroable for D96 {}
#[cfg(feature = "bytemuck")]
unsafe impl bytemuck::Pod for D96 {}

// ============================================================================
// num-traits Integration
// ============================================================================
//
// All methods delegate to the inherent `D96` API. Inherent associated
// functions take precedence over trait ones, so `D96::checked_add(...)` /
// `Self::from_i64(...)` always resolve to the inherent versions (never the
// trait method being defined). Only base-10 `Num::from_str_radix` is supported;
// `Float`/`Real`/`Integer` are intentionally NOT implemented (D96 is not a
// field and has no fractional reciprocal closure).
//
// Note vs D64: `FromPrimitive::from_i64`/`from_u64` here range-check against
// D96's ~±3.96e16 integer range and return `None` when exceeded (the inherent
// `D96::from_i64`/`from_u64` do NOT range-check — see their docs). `to_i64`
// returns `Option` for signature symmetry, but since D96's integer part is
// bounded by ~3.96e16 < i64::MAX it never actually returns `None` for an
// in-range value.

#[cfg(feature = "num-traits")]
impl num_traits::Zero for D96 {
    #[inline]
    fn zero() -> Self {
        Self::ZERO
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.value == 0
    }
}

#[cfg(feature = "num-traits")]
impl num_traits::One for D96 {
    #[inline]
    fn one() -> Self {
        Self::ONE
    }

    #[inline]
    fn is_one(&self) -> bool {
        self.value == Self::SCALE
    }
}

#[cfg(feature = "num-traits")]
impl num_traits::Bounded for D96 {
    #[inline]
    fn min_value() -> Self {
        Self::MIN
    }

    #[inline]
    fn max_value() -> Self {
        Self::MAX
    }
}

#[cfg(feature = "num-traits")]
impl num_traits::Signed for D96 {
    #[inline]
    fn abs(&self) -> Self {
        D96::abs(*self)
    }

    /// Positive difference: `self - other` when `self > other`, else `ZERO`.
    #[inline]
    fn abs_sub(&self, other: &Self) -> Self {
        if *self <= *other {
            Self::ZERO
        } else {
            *self - *other
        }
    }

    /// Sign as a decimal value (`ONE`, `-ONE`, or `ZERO`) — not an `i32`.
    #[inline]
    fn signum(&self) -> Self {
        if self.value > 0 {
            Self::ONE
        } else if self.value < 0 {
            -Self::ONE
        } else {
            Self::ZERO
        }
    }

    #[inline]
    fn is_positive(&self) -> bool {
        self.value > 0
    }

    #[inline]
    fn is_negative(&self) -> bool {
        self.value < 0
    }
}

#[cfg(feature = "num-traits")]
impl num_traits::Num for D96 {
    type FromStrRadixErr = DecimalError;

    /// Parses a base-10 decimal string via [`D96::from_str_exact`]. Only
    /// `radix == 10` is supported; any other radix returns
    /// [`DecimalError::InvalidFormat`].
    #[inline]
    fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
        if radix == 10 {
            Self::from_str_exact(str)
        } else {
            Err(DecimalError::InvalidFormat)
        }
    }
}

#[cfg(feature = "num-traits")]
impl num_traits::CheckedAdd for D96 {
    #[inline]
    fn checked_add(&self, v: &Self) -> Option<Self> {
        D96::checked_add(*self, *v)
    }
}

#[cfg(feature = "num-traits")]
impl num_traits::CheckedSub for D96 {
    #[inline]
    fn checked_sub(&self, v: &Self) -> Option<Self> {
        D96::checked_sub(*self, *v)
    }
}

#[cfg(feature = "num-traits")]
impl num_traits::CheckedMul for D96 {
    #[inline]
    fn checked_mul(&self, v: &Self) -> Option<Self> {
        D96::checked_mul(*self, *v)
    }
}

#[cfg(feature = "num-traits")]
impl num_traits::CheckedDiv for D96 {
    #[inline]
    fn checked_div(&self, v: &Self) -> Option<Self> {
        D96::checked_div(*self, *v)
    }
}

#[cfg(feature = "num-traits")]
impl num_traits::Saturating for D96 {
    #[inline]
    fn saturating_add(self, v: Self) -> Self {
        D96::saturating_add(self, v)
    }

    #[inline]
    fn saturating_sub(self, v: Self) -> Self {
        D96::saturating_sub(self, v)
    }
}

#[cfg(feature = "num-traits")]
impl num_traits::FromPrimitive for D96 {
    /// Returns `None` if `n` exceeds D96's representable integer range
    /// (≈±3.96e16). Does NOT call the unchecked inherent `from_i64`.
    #[inline]
    fn from_i64(n: i64) -> Option<Self> {
        let scaled = (n as i128).checked_mul(Self::SCALE)?;
        if (Self::MIN.value..=Self::MAX.value).contains(&scaled) {
            Some(Self { value: scaled })
        } else {
            None
        }
    }

    /// Returns `None` if `n` exceeds D96's representable integer range (≈3.96e16).
    #[inline]
    fn from_u64(n: u64) -> Option<Self> {
        let scaled = (n as i128).checked_mul(Self::SCALE)?;
        if scaled > Self::MAX.value {
            None
        } else {
            Some(Self { value: scaled })
        }
    }

    #[inline]
    fn from_f64(n: f64) -> Option<Self> {
        Self::from_f64(n)
    }
}

#[cfg(feature = "num-traits")]
impl num_traits::ToPrimitive for D96 {
    /// Truncates toward zero. Returns `Option` for trait symmetry, but D96's
    /// integer part is bounded by ≈3.96e16 < i64::MAX, so for any in-range D96
    /// it is always `Some`.
    #[inline]
    fn to_i64(&self) -> Option<i64> {
        D96::to_i64(*self)
    }

    /// Truncates toward zero; negative or out-of-`i64`-range values return `None`.
    #[inline]
    fn to_u64(&self) -> Option<u64> {
        match D96::to_i64(*self) {
            Some(v) if v >= 0 => Some(v as u64),
            _ => None,
        }
    }

    #[inline]
    fn to_f64(&self) -> Option<f64> {
        Some(D96::to_f64(*self))
    }
}

/// Multiplicative inverse `1 / self`.
///
/// # Panics
/// Panics if `self` is zero. Unlike floating point, `D96` has no infinity, so
/// there is no value to return. Use [`D96::recip`] for a non-panicking
/// `Option` result.
#[cfg(feature = "num-traits")]
impl num_traits::Inv for D96 {
    type Output = Self;

    #[inline]
    fn inv(self) -> Self {
        self.recip().expect("Inv::inv called on zero D96")
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Compute 10^n at compile time for rounding operations (i128 version)
/// Round-half-to-even division of an i128 `m` by `10^k`. Used by the lossy
/// scale constructors so the FULL dropped fraction participates in rounding
/// (dividing in stages and truncating first, as the old code did, biased the
/// result toward zero). `k` must be > 0; returns 0 when `10^k` overflows i128.
/// Rounds a finite `f64` to the nearest integer (half away from zero) using only
/// `core` operations — `f64::round` lives in `std`, so it is unavailable on a
/// bare-metal `no_std` target. Equivalent to `value.round()` for every input the
/// `from_f64` paths keep (they range-check the result afterwards): when
/// `|value| < 2^95` the `as i128` truncation is exact, and larger magnitudes
/// round to an out-of-range value that the caller rejects either way.
#[inline]
fn round_half_away_f64(value: f64) -> f64 {
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

const fn round_div_pow10_i128(m: i128, k: u32) -> i128 {
    if k >= 39 {
        return 0;
    }
    let d = 10i128.pow(k);
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

const fn const_pow10_i128(n: u8) -> i128 {
    match n {
        0 => 1,
        1 => 10,
        2 => 100,
        3 => 1_000,
        4 => 10_000,
        5 => 100_000,
        6 => 1_000_000,
        7 => 10_000_000,
        8 => 100_000_000,
        9 => 1_000_000_000,
        10 => 10_000_000_000,
        11 => 100_000_000_000,
        12 => 1_000_000_000_000,
        _ => panic!("pow10 out of range for D96"),
    }
}

/// Integer square root for u128 using binary search
const fn isqrt_u128(n: u128) -> u128 {
    if n < 2 {
        return n;
    }

    let mut left = 1u128;
    let mut right = n;

    while left <= right {
        let mid = left + (right - left) / 2;

        if mid <= n / mid {
            let next_mid = mid + 1;
            if next_mid > n / next_mid {
                return mid;
            }
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }

    right
}

#[inline(always)]
const fn banker_round(quotient : i128, remainder : i128, half : i128) -> i128 {
    if remainder > half {
        quotient + 1
    } else if remainder < -half {
        quotient - 1
    } else if remainder == half {
        // Banker's rounding: round to even
        if quotient % 2 == 0 {
            quotient
        } else {
            quotient + 1
        }
    } else if remainder == -half {
        // Banker's rounding: round to even
        if quotient % 2 == 0 {
            quotient
        } else {
            quotient - 1
        }
    } else {
        quotient
    }
}

#[cfg(test)]
mod tests {
    use std::string::ToString;

    use super::*;

    #[test]
    fn test_d96_constants() {
        assert_eq!(D96::ZERO.to_raw(), 0);
        assert_eq!(D96::ONE.to_raw(), 1_000_000_000_000); // 10^12
        assert_eq!(D96::SCALE, 1_000_000_000_000); // 10^12
    }

    #[test]
    fn test_addition() {
        let a = D96::from_raw(1_000_000_000_000); // 1.0
        let b = D96::from_raw(2_000_000_000_000); // 2.0

        assert_eq!(a.checked_add(b), Some(D96::from_raw(3_000_000_000_000))); // 3.0
        assert_eq!(a.saturating_add(b), D96::from_raw(3_000_000_000_000));
        assert_eq!(a.wrapping_add(b), D96::from_raw(3_000_000_000_000));
    }

    #[test]
    fn test_addition_overflow() {
        let a = D96::MAX;
        let b = D96::ONE;

        assert_eq!(a.checked_add(b), None);
        assert_eq!(a.saturating_add(b), D96::MAX);
    }

    #[test]
    fn test_subtraction() {
        let a = D96::from_raw(3_000_000_000_000); // 3.0
        let b = D96::from_raw(1_000_000_000_000); // 1.0

        assert_eq!(a.checked_sub(b), Some(D96::from_raw(2_000_000_000_000))); // 2.0
    }

    #[test]
    fn test_multiplication() {
        let a = D96::from_raw(2_000_000_000_000); // 2.0
        let b = D96::from_raw(3_000_000_000_000); // 3.0

        assert_eq!(a.checked_mul(b), Some(D96::from_raw(6_000_000_000_000))); // 6.0
    }

    #[test]
    fn test_division() {
        let a = D96::from_raw(6_000_000_000_000); // 6.0
        let b = D96::from_raw(2_000_000_000_000); // 2.0

        assert_eq!(a.checked_div(b), Some(D96::from_raw(3_000_000_000_000))); // 3.0
    }

    #[test]
    fn test_division_by_zero() {
        let a = D96::ONE;
        let b = D96::ZERO;

        assert_eq!(a.checked_div(b), None);
        assert_eq!(a.saturating_div(b), D96::ZERO);
    }

    #[test]
    fn test_negation() {
        let a = D96::from_raw(1_000_000_000_000); // 1.0

        assert_eq!(a.checked_neg(), Some(D96::from_raw(-1_000_000_000_000))); // -1.0
    }

    #[test]
    fn test_abs() {
        let a = D96::from_raw(-1_000_000_000_000); // -1.0

        assert_eq!(a.abs(), D96::from_raw(1_000_000_000_000)); // 1.0
    }

    #[test]
    fn test_sign_checks() {
        assert!(D96::ONE.is_positive());
        assert!(!D96::ONE.is_negative());
        assert!(!D96::ONE.is_zero());

        assert!(D96::ZERO.is_zero());
        assert!(!D96::ZERO.is_positive());
        assert!(!D96::ZERO.is_negative());

        let neg = D96::from_raw(-1_000_000_000_000);
        assert!(neg.is_negative());
        assert!(!neg.is_positive());
    }

    #[test]
    fn test_signum() {
        assert_eq!(D96::ONE.signum(), 1);
        assert_eq!(D96::ZERO.signum(), 0);
        assert_eq!(D96::from_raw(-1_000_000_000_000).signum(), -1);
    }

    // with_scale tests
    #[test]
    fn test_d96_with_scale_basic() {
        let d = D96::with_scale(12345, 2);
        assert_eq!(d, D96::from_str_exact("123.45").unwrap());
    }

    #[test]
    fn test_d96_with_scale_zero_scale() {
        let d = D96::with_scale(123, 0);
        assert_eq!(d, D96::from_str_exact("123").unwrap());
    }

    #[test]
    fn test_d96_with_scale_full_precision() {
        let d = D96::with_scale(1_000_000_000_000, 12);
        assert_eq!(d, D96::ONE);
    }

    #[test]
    fn test_d96_with_scale_negative() {
        let d = D96::with_scale(-12345, 2);
        assert_eq!(d, D96::from_str_exact("-123.45").unwrap());
    }

    #[test]
    #[should_panic(expected = "scale exceeds D96 precision")]
    fn test_d96_with_scale_panic_too_precise() {
        let _ = D96::with_scale(1234567890123456, 13);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn test_d96_with_scale_panic_overflow() {
        let _ = D96::with_scale(i128::MAX, 0);
    }

    // try_with_scale tests
    #[test]
    fn test_d96_try_with_scale_success() {
        let d = D96::try_with_scale(12345, 2).unwrap();
        assert_eq!(d, D96::from_str_exact("123.45").unwrap());
    }

    #[test]
    fn test_d96_try_with_scale_too_precise() {
        assert!(D96::try_with_scale(1234567890123456, 13).is_none());
    }

    #[test]
    fn test_d96_try_with_scale_overflow() {
        assert!(D96::try_with_scale(i128::MAX, 0).is_none());
    }

    // with_scale_lossy tests
    #[test]
    fn test_d96_lossy_exact() {
        let d = D96::with_scale_lossy(12345, 2);
        assert_eq!(d, D96::from_str_exact("123.45").unwrap());
    }

    #[test]
    fn test_d96_lossy_rounds_down() {
        // 0.1234567890124 with 13 decimals -> 0.123456789012
        let d = D96::with_scale_lossy(1234567890124, 13);
        assert_eq!(d, D96::from_str_exact("0.123456789012").unwrap());
    }

    #[test]
    fn test_d96_lossy_rounds_up() {
        // 0.1234567890126 with 13 decimals -> 0.123456789013
        let d = D96::with_scale_lossy(1234567890126, 13);
        assert_eq!(d, D96::from_str_exact("0.123456789013").unwrap());
    }

    #[test]
    fn test_d96_lossy_bankers_rounding_even() {
        // 0.1234567890125 -> round to even (0.123456789012)
        let d = D96::with_scale_lossy(1234567890125, 13);
        assert_eq!(d, D96::from_str_exact("0.123456789012").unwrap());
    }

    #[test]
    fn test_d96_lossy_bankers_rounding_odd() {
        // 0.1234567890135 -> round to even (0.123456789014)
        let d = D96::with_scale_lossy(1234567890135, 13);
        assert_eq!(d, D96::from_str_exact("0.123456789014").unwrap());
    }

    #[test]
    fn test_d96_lossy_large_scale() {
        // 1 with scale 30 should round to 0
        let d = D96::with_scale_lossy(1, 30);
        assert_eq!(d, D96::ZERO);
    }

    #[test]
    fn test_d96_lossy_ethereum_wei() {
        // 1 wei = 10^-18 ETH, D96 supports 10^-12, so 1 wei rounds to 0
        let d = D96::with_scale_lossy(1, 18);
        assert_eq!(d, D96::ZERO);

        // 1 million wei = 10^-12 ETH, fits exactly in D96
        let d = D96::with_scale_lossy(1_000_000, 18);
        assert_eq!(d, D96::from_str_exact("0.000000000001").unwrap());
    }

    #[test]
    fn test_d96_lossy_negative() {
        let d = D96::with_scale_lossy(-1234567890126, 13);
        assert_eq!(d, D96::from_str_exact("-0.123456789013").unwrap());
    }

    // try_with_scale_lossy tests
    #[test]
    fn test_d96_try_lossy_success() {
        let d = D96::try_with_scale_lossy(1234567890126, 13).unwrap();
        assert_eq!(d, D96::from_str_exact("0.123456789013").unwrap());
    }

    #[test]
    fn test_d96_try_lossy_overflow() {
        // Even with rounding, i128::MAX at scale 0 is too large for 96-bit
        assert!(D96::try_with_scale_lossy(i128::MAX, 0).is_none());
    }

    #[test]
    fn test_d96_try_lossy_very_large_scale() {
        let d = D96::try_with_scale_lossy(1, 50).unwrap();
        assert_eq!(d, D96::ZERO);
    }

    // Compatibility with rust_decimal patterns
    #[test]
    fn test_d96_rust_decimal_compat() {
        // rust_decimal::Decimal::new(12345, 2) equivalent
        let d = D96::with_scale(12345, 2);
        assert_eq!(d.to_string(), "123.45");
    }

    #[test]
    fn test_d96_rust_decimal_max_precision() {
        // rust_decimal supports up to 28 decimals, we support 12
        // This tests lossy conversion from rust_decimal's precision
        let mantissa = 1234567890123456789012345678i128;
        let d = D96::with_scale_lossy(mantissa, 28);
        // Should round to something reasonable
        assert!(d != D96::ZERO);
    }
}

#[cfg(test)]
mod operator_tests {
    use super::*;

    #[test]
    fn test_add_operator() {
        let a = D96::from_raw(1_000_000_000_000); // 1.0
        let b = D96::from_raw(2_000_000_000_000); // 2.0
        let c = a + b;
        assert_eq!(c.to_raw(), 3_000_000_000_000); // 3.0
    }

    #[test]
    #[should_panic(expected = "attempt to add with overflow")]
    fn test_add_operator_panic() {
        let _ = D96::MAX + D96::ONE;
    }

    #[test]
    fn test_sub_operator() {
        let a = D96::from_raw(3_000_000_000_000); // 3.0
        let b = D96::from_raw(1_000_000_000_000); // 1.0
        let c = a - b;
        assert_eq!(c.to_raw(), 2_000_000_000_000); // 2.0
    }

    #[test]
    fn test_mul_operator() {
        let a = D96::from_raw(2_000_000_000_000); // 2.0
        let b = D96::from_raw(3_000_000_000_000); // 3.0
        let c = a * b;
        assert_eq!(c.to_raw(), 6_000_000_000_000); // 6.0
    }

    #[test]
    fn test_div_operator() {
        let a = D96::from_raw(6_000_000_000_000); // 6.0
        let b = D96::from_raw(2_000_000_000_000); // 2.0
        let c = a / b;
        assert_eq!(c.to_raw(), 3_000_000_000_000); // 3.0
    }

    #[test]
    #[should_panic(expected = "attempt to divide by zero")]
    fn test_div_by_zero_panic() {
        let _ = D96::ONE / D96::ZERO;
    }

    #[test]
    fn test_neg_operator() {
        let a = D96::from_raw(1_000_000_000_000); // 1.0
        let b = -a;
        assert_eq!(b.to_raw(), -1_000_000_000_000); // -1.0
    }

    #[test]
    fn test_add_assign() {
        let mut a = D96::from_raw(1_000_000_000_000); // 1.0
        a += D96::from_raw(2_000_000_000_000); // 2.0
        assert_eq!(a.to_raw(), 3_000_000_000_000); // 3.0
    }

    #[test]
    fn test_sub_assign() {
        let mut a = D96::from_raw(3_000_000_000_000); // 3.0
        a -= D96::from_raw(1_000_000_000_000); // 1.0
        assert_eq!(a.to_raw(), 2_000_000_000_000); // 2.0
    }

    #[test]
    fn test_mul_assign() {
        let mut a = D96::from_raw(2_000_000_000_000); // 2.0
        a *= D96::from_raw(3_000_000_000_000); // 3.0
        assert_eq!(a.to_raw(), 6_000_000_000_000); // 6.0
    }

    #[test]
    fn test_div_assign() {
        let mut a = D96::from_raw(6_000_000_000_000); // 6.0
        a /= D96::from_raw(2_000_000_000_000); // 2.0
        assert_eq!(a.to_raw(), 3_000_000_000_000); // 3.0
    }

    #[test]
    fn test_operator_chaining() {
        let a = D96::from_raw(1_000_000_000_000); // 1.0
        let b = D96::from_raw(2_000_000_000_000); // 2.0
        let c = D96::from_raw(3_000_000_000_000); // 3.0

        let result = a + b * c; // 1.0 + (2.0 * 3.0) = 7.0
        assert_eq!(result.to_raw(), 7_000_000_000_000);
    }
}

#[cfg(test)]
mod conversion_tests {
    use super::*;

    #[test]
    fn test_from_i32() {
        let d = D96::from_i32(42);
        assert_eq!(d.to_i128(), 42);

        let d = D96::from(42i32);
        assert_eq!(d.to_i128(), 42);
    }

    #[test]
    fn test_from_i64() {
        let d = D96::from_i64(100);
        assert_eq!(d.to_i128(), 100);

        // i64 -> D96 is fallible (range-checked); in-range values convert.
        let d = D96::try_from(100i64).unwrap();
        assert_eq!(d.to_i128(), 100);
        assert_eq!(D96::try_from(i64::MAX), Err(DecimalError::Overflow));
    }

    #[test]
    fn test_from_i128() {
        assert_eq!(D96::from_i128(100).unwrap().to_i128(), 100);
        assert!(D96::from_i128(i128::MAX).is_none()); // Would overflow when scaled
    }

    #[test]
    fn test_to_i128_truncate() {
        let d = D96::from_raw(2_500_000_000_000); // 2.5
        assert_eq!(d.to_i128(), 2); // Truncates
    }

    #[test]
    fn test_to_i128_round() {
        assert_eq!(D96::from_raw(1_100_000_000_000).to_i128_round(), 1); // 1.1: Normal rounding
        assert_eq!(D96::from_raw(1_400_000_000_000).to_i128_round(), 1); // 1.4: Normal rounding
        assert_eq!(D96::from_raw(1_500_000_000_000).to_i128_round(), 2); // 1.5: Banker's rounding: round to even
        assert_eq!(D96::from_raw(1_600_000_000_000).to_i128_round(), 2); // 1.6: Normal rounding

        assert_eq!(D96::from_raw(2_400_000_000_000).to_i128_round(), 2); // 2.4: Normal rounding
        assert_eq!(D96::from_raw(2_500_000_000_000).to_i128_round(), 2); // 2.5: Banker's rounding: round to even
        assert_eq!(D96::from_raw(2_600_000_000_000).to_i128_round(), 3); // 2.6: Normal rounding

        assert_eq!(D96::from_raw(3_400_000_000_000).to_i128_round(), 3); // 3.4: Normal rounding
        assert_eq!(D96::from_raw(3_500_000_000_000).to_i128_round(), 4); // 3.5: Banker's rounding: round to even
        assert_eq!(D96::from_raw(3_600_000_000_000).to_i128_round(), 4); // 3.6: Normal rounding

        assert_eq!(D96::from_raw(4_400_000_000_000).to_i128_round(), 4); // 4.4: Normal rounding
        assert_eq!(D96::from_raw(4_500_000_000_000).to_i128_round(), 4); // 4.5: Banker's rounding: round to even
        assert_eq!(D96::from_raw(4_600_000_000_000).to_i128_round(), 5); // 4.6: Normal rounding

        assert_eq!(D96::from_raw(-1_100_000_000_000).to_i128_round(), -1); // -1.1: Normal rounding
        assert_eq!(D96::from_raw(-1_400_000_000_000).to_i128_round(), -1); // -1.4: Normal rounding
        assert_eq!(D96::from_raw(-1_500_000_000_000).to_i128_round(), -2); // -1.5: Banker's rounding: round to even
        assert_eq!(D96::from_raw(-1_600_000_000_000).to_i128_round(), -2); // -1.6: Normal rounding

        assert_eq!(D96::from_raw(-2_400_000_000_000).to_i128_round(), -2); // -2.4: Normal rounding
        assert_eq!(D96::from_raw(-2_500_000_000_000).to_i128_round(), -2); // -2.5: Banker's rounding: round to even
        assert_eq!(D96::from_raw(-2_600_000_000_000).to_i128_round(), -3); // -2.6: Normal rounding

        assert_eq!(D96::from_raw(-3_400_000_000_000).to_i128_round(), -3); // -3.4: Normal rounding
        assert_eq!(D96::from_raw(-3_500_000_000_000).to_i128_round(), -4); // -3.5: Banker's rounding: round to even
        assert_eq!(D96::from_raw(-3_600_000_000_000).to_i128_round(), -4); // -3.6: Normal rounding

        assert_eq!(D96::from_raw(-4_400_000_000_000).to_i128_round(), -4); // -4.4: Normal rounding
        assert_eq!(D96::from_raw(-4_500_000_000_000).to_i128_round(), -4); // -4.5: Banker's rounding: round to even
        assert_eq!(D96::from_raw(-4_600_000_000_000).to_i128_round(), -5); // -4.6: Normal rounding
    }

    #[test]
    fn test_to_i64() {
        let d = D96::from_raw(42_000_000_000_000); // 42.0
        assert_eq!(d.to_i64(), Some(42));

        // D96's maximum integer part is ~39 trillion
        // This always fits in i64 (which can hold up to ~9 quintillion)
        // So to_i64() should always succeed for valid D96 values
        let large = D96::from_str_exact("39614081257132").unwrap(); // ~39.6 trillion
        assert_eq!(large.to_i64(), Some(39614081257132));

        // Test D96::MAX converts successfully
        let max_as_i64 = D96::MAX.to_i64();
        assert!(max_as_i64.is_some());

        // Test negative
        let neg = D96::from_str_exact("-100000").unwrap();
        assert_eq!(neg.to_i64(), Some(-100000));
    }

    #[test]
    fn test_from_f64() {
        let d = D96::from_f64(3.04827165).unwrap();
        let f = d.to_f64();
        assert!((f - 3.04827165).abs() < 1e-10);
    }

    #[test]
    fn test_from_f64_edge_cases() {
        assert!(D96::from_f64(f64::NAN).is_none());
        assert!(D96::from_f64(f64::INFINITY).is_none());
        assert!(D96::from_f64(f64::NEG_INFINITY).is_none());
    }

    #[test]
    fn test_try_from() {
        assert!(D96::try_from(42i128).is_ok());
        assert!(D96::try_from(i128::MAX).is_err());
        assert!(D96::try_from(3.07f64).is_ok());
        assert!(D96::try_from(f64::NAN).is_err());
    }

    #[test]
    fn test_small_int_conversions() {
        let d1: D96 = 42i8.into();
        let d2: D96 = 42u8.into();
        let d3: D96 = 42i16.into();
        let d4: D96 = 42u16.into();
        let d5: D96 = 42i32.into();
        let d6: D96 = 42u32.into();
        // i64 / u64 are fallible (D96's integer range is smaller than theirs).
        let d7: D96 = D96::try_from(42i64).unwrap();
        let d8: D96 = D96::try_from(42u64).unwrap();

        assert_eq!(d1.to_i128(), 42);
        assert_eq!(d2.to_i128(), 42);
        assert_eq!(d3.to_i128(), 42);
        assert_eq!(d4.to_i128(), 42);
        assert_eq!(d5.to_i128(), 42);
        assert_eq!(d6.to_i128(), 42);
        assert_eq!(d7.to_i128(), 42);
        assert_eq!(d8.to_i128(), 42);
    }
}

#[cfg(test)]
mod comparison_tests {
    use super::*;

    #[test]
    fn test_min() {
        let a = D96::from_raw(1_000_000_000_000); // 1.0
        let b = D96::from_raw(2_000_000_000_000); // 2.0

        assert_eq!(a.min(b), a);
        assert_eq!(b.min(a), a);
    }

    #[test]
    fn test_max() {
        let a = D96::from_raw(1_000_000_000_000); // 1.0
        let b = D96::from_raw(2_000_000_000_000); // 2.0

        assert_eq!(a.max(b), b);
        assert_eq!(b.max(a), b);
    }

    #[test]
    fn test_clamp() {
        let min = D96::from_raw(1_000_000_000_000); // 1.0
        let max = D96::from_raw(3_000_000_000_000); // 3.0

        let below = D96::from_raw(500_000_000_000); // 0.5
        let within = D96::from_raw(2_000_000_000_000); // 2.0
        let above = D96::from_raw(4_000_000_000_000); // 4.0

        assert_eq!(below.clamp(min, max), min);
        assert_eq!(within.clamp(min, max), within);
        assert_eq!(above.clamp(min, max), max);
    }

    #[test]
    #[should_panic(expected = "min must be less than or equal to max")]
    fn test_clamp_panic() {
        let min = D96::from_raw(3_000_000_000_000);
        let max = D96::from_raw(1_000_000_000_000);
        let _ = D96::ZERO.clamp(min, max);
    }
}

#[cfg(test)]
mod rounding_tests {
    use super::*;

    #[test]
    fn test_floor() {
        assert_eq!(
            D96::from_raw(2_500_000_000_000).floor().to_raw(),
            2_000_000_000_000
        ); // 2.5 -> 2.0
        assert_eq!(
            D96::from_raw(-2_500_000_000_000).floor().to_raw(),
            -3_000_000_000_000
        ); // -2.5 -> -3.0
        assert_eq!(
            D96::from_raw(2_000_000_000_000).floor().to_raw(),
            2_000_000_000_000
        ); // 2.0 -> 2.0
    }

    #[test]
    fn test_ceil() {
        assert_eq!(
            D96::from_raw(2_500_000_000_000).ceil().to_raw(),
            3_000_000_000_000
        ); // 2.5 -> 3.0
        assert_eq!(
            D96::from_raw(-2_500_000_000_000).ceil().to_raw(),
            -2_000_000_000_000
        ); // -2.5 -> -2.0
        assert_eq!(
            D96::from_raw(2_000_000_000_000).ceil().to_raw(),
            2_000_000_000_000
        ); // 2.0 -> 2.0
    }

    #[test]
    fn test_trunc() {
        assert_eq!(
            D96::from_raw(2_500_000_000_000).trunc().to_raw(),
            2_000_000_000_000
        ); // 2.5 -> 2.0
        assert_eq!(
            D96::from_raw(-2_500_000_000_000).trunc().to_raw(),
            -2_000_000_000_000
        ); // -2.5 -> -2.0
    }

    #[test]
    fn test_fract() {
        assert_eq!(
            D96::from_raw(2_500_000_000_000).fract().to_raw(),
            500_000_000_000
        ); // 2.5 -> 0.5
        assert_eq!(D96::from_raw(2_000_000_000_000).fract().to_raw(), 0); // 2.0 -> 0.0
    }

    #[test]
    fn test_round() {
        assert_eq!(
            D96::from_raw(2_500_000_000_000).round().to_raw(),
            2_000_000_000_000
        ); // 2.5 -> 2.0 (banker's)
        assert_eq!(
            D96::from_raw(3_500_000_000_000).round().to_raw(),
            4_000_000_000_000
        ); // 3.5 -> 4.0 (banker's)
        assert_eq!(
            D96::from_raw(2_600_000_000_000).round().to_raw(),
            3_000_000_000_000
        ); // 2.6 -> 3.0
    }

    #[test]
    fn test_round_dp() {
        let val = D96::from_raw(1_234_567_890_123); // 1.234567890123

        assert_eq!(val.round_dp(0).to_raw(), 1_000_000_000_000); // 1.0
        assert_eq!(val.round_dp(2).to_raw(), 1_230_000_000_000); // 1.23
        assert_eq!(val.round_dp(9).to_raw(), 1_234_567_890_000); // 1.234567890
        assert_eq!(val.round_dp(12).to_raw(), 1_234_567_890_123); // unchanged
    }

    #[test]
    #[should_panic(expected = "decimal_places must be <= DECIMALS")]
    fn test_round_dp_panic() {
        core::hint::black_box(D96::ZERO.round_dp(13));
    }
}

#[cfg(test)]
mod math_tests {
    use super::*;

    #[test]
    fn test_recip() {
        let two = D96::from_raw(2_000_000_000_000); // 2.0
        let half = two.recip().unwrap();
        assert_eq!(half.to_raw(), 500_000_000_000); // 0.5

        assert_eq!(D96::ZERO.recip(), None);
    }

    #[test]
    fn test_powi_positive() {
        let two = D96::from_raw(2_000_000_000_000); // 2.0

        assert_eq!(two.powi(0).unwrap().to_raw(), 1_000_000_000_000); // 2^0 = 1.0
        assert_eq!(two.powi(1).unwrap().to_raw(), 2_000_000_000_000); // 2^1 = 2.0
        assert_eq!(two.powi(2).unwrap().to_raw(), 4_000_000_000_000); // 2^2 = 4.0
        assert_eq!(two.powi(3).unwrap().to_raw(), 8_000_000_000_000); // 2^3 = 8.0
    }

    #[test]
    fn test_powi_negative() {
        let two = D96::from_raw(2_000_000_000_000); // 2.0

        assert_eq!(two.powi(-1).unwrap().to_raw(), 500_000_000_000); // 2^-1 = 0.5
        assert_eq!(two.powi(-2).unwrap().to_raw(), 250_000_000_000); // 2^-2 = 0.25
    }

    #[test]
    fn test_powi_compound_interest() {
        // 1.05^10 for compound interest calculation
        let rate = D96::from_raw(1_050_000_000_000); // 1.05 (5% interest)
        let result = rate.powi(10).unwrap();

        // 1.05^10 ≈ 1.62889462677744140625
        let expected = D96::from_raw(1_628_894_626_777);

        // Allow small rounding difference
        assert!((result.to_raw() - expected.to_raw()).abs() < 1_000_000);
    }

    #[test]
    fn test_sqrt_perfect_squares() {
        let four = D96::from_raw(4_000_000_000_000); // 4.0
        let sqrt_four = four.sqrt().unwrap();
        assert_eq!(sqrt_four.to_raw(), 2_000_000_000_000); // 2.0

        let nine = D96::from_raw(9_000_000_000_000); // 9.0
        let sqrt_nine = nine.sqrt().unwrap();
        assert_eq!(sqrt_nine.to_raw(), 3_000_000_000_000); // 3.0
    }

    #[test]
    fn test_sqrt_non_perfect() {
        let two = D96::from_raw(2_000_000_000_000); // 2.0
        let sqrt_two = two.sqrt().unwrap();

        // sqrt(2) ≈ 1.41421356237309504880
        let expected = D96::from_raw(1_414_213_562_373);

        // Check accuracy within reasonable tolerance
        assert!((sqrt_two.to_raw() - expected.to_raw()).abs() < 100);
    }

    #[test]
    fn test_sqrt_edge_cases() {
        assert_eq!(D96::ZERO.sqrt().unwrap(), D96::ZERO);
        assert_eq!(D96::ONE.sqrt().unwrap(), D96::ONE);

        let neg = D96::from_raw(-1_000_000_000_000);
        assert_eq!(neg.sqrt(), None);
    }

    #[test]
    fn test_sqrt_verify() {
        // Test that sqrt(x)^2 ≈ x
        let x = D96::from_raw(5_000_000_000_000); // 5.0
        let sqrt_x = x.sqrt().unwrap();
        let squared = sqrt_x.checked_mul(sqrt_x).unwrap();

        // Should be very close to original
        assert!((squared.to_raw() - x.to_raw()).abs() < 1000);
    }
}

#[cfg(test)]
mod result_tests {
    use super::*;

    #[test]
    fn test_try_add() {
        let a = D96::from_raw(1_000_000_000_000);
        let b = D96::from_raw(2_000_000_000_000);

        assert!(a.try_add(b).is_ok());
        assert_eq!(a.try_add(b).unwrap().to_raw(), 3_000_000_000_000);

        assert!(D96::MAX.try_add(D96::ONE).is_err());
    }

    #[test]
    fn test_try_sub() {
        let a = D96::from_raw(3_000_000_000_000);
        let b = D96::from_raw(1_000_000_000_000);

        assert!(a.try_sub(b).is_ok());
        assert!(D96::MIN.try_sub(D96::ONE).is_err());
    }

    #[test]
    fn test_try_mul() {
        let a = D96::from_raw(2_000_000_000_000);
        let b = D96::from_raw(3_000_000_000_000);

        assert!(a.try_mul(b).is_ok());
        assert!(D96::MAX.try_mul(D96::from_raw(2_000_000_000_000)).is_err());
    }

    #[test]
    fn test_try_div() {
        let a = D96::from_raw(6_000_000_000_000);
        let b = D96::from_raw(2_000_000_000_000);

        assert!(a.try_div(b).is_ok());

        let result = D96::ONE.try_div(D96::ZERO);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DecimalError::DivisionByZero));
    }

    #[test]
    fn test_try_neg() {
        assert!(D96::ONE.try_neg().is_ok());
        assert!(D96::MIN.try_neg().is_err());
    }

    #[test]
    fn test_try_abs() {
        assert!(D96::from_raw(-1_000_000_000_000).try_abs().is_ok());
        assert!(D96::MIN.try_abs().is_err());
    }

    #[test]
    fn test_try_recip() {
        assert!(D96::from_raw(2_000_000_000_000).try_recip().is_ok());

        let result = D96::ZERO.try_recip();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DecimalError::DivisionByZero));
    }

    #[test]
    fn test_try_powi() {
        let two = D96::from_raw(2_000_000_000_000);
        assert!(two.try_powi(3).is_ok());
        assert!(D96::MAX.try_powi(2).is_err());
    }

    #[test]
    fn test_try_sqrt() {
        let four = D96::from_raw(4_000_000_000_000);
        assert!(four.try_sqrt().is_ok());

        let neg = D96::from_raw(-1_000_000_000_000);
        assert!(neg.try_sqrt().is_err());
    }
}

#[cfg(test)]
mod string_tests {
    use super::*;

    #[test]
    fn test_from_str_integer() {
        assert_eq!(
            D96::from_str_exact("123").unwrap().to_raw(),
            123_000_000_000_000
        );
        assert_eq!(D96::from_str_exact("0").unwrap().to_raw(), 0);
        assert_eq!(
            D96::from_str_exact("-456").unwrap().to_raw(),
            -456_000_000_000_000
        );
    }

    #[test]
    fn test_from_str_decimal() {
        assert_eq!(
            D96::from_str_exact("123.45").unwrap().to_raw(),
            123_450_000_000_000
        );
        assert_eq!(D96::from_str_exact("0.000000000001").unwrap().to_raw(), 1);
        assert_eq!(
            D96::from_str_exact("-123.45").unwrap().to_raw(),
            -123_450_000_000_000
        );
    }

    #[test]
    fn test_from_str_leading_decimal() {
        assert_eq!(
            D96::from_str_exact("0.5").unwrap().to_raw(),
            500_000_000_000
        );
        assert_eq!(D96::from_str_exact(".5").unwrap().to_raw(), 500_000_000_000);
        assert_eq!(D96::from_str_exact("-0.5").unwrap().to_raw(), -500_000_000_000);
        assert_eq!(D96::from_str_exact("-.5").unwrap().to_raw(), -500_000_000_000);
    }

    #[test]
    fn test_from_str_trailing_zeros() {
        assert_eq!(
            D96::from_str_exact("123.450000000000").unwrap().to_raw(),
            123_450_000_000_000
        );
        assert_eq!(
            D96::from_str_exact("1.10").unwrap().to_raw(),
            1_100_000_000_000
        );
    }

    #[test]
    fn test_from_str_plus_sign() {
        assert_eq!(
            D96::from_str_exact("+123.45").unwrap().to_raw(),
            123_450_000_000_000
        );
    }

    #[test]
    fn test_from_str_whitespace() {
        assert_eq!(
            D96::from_str_exact("  123.45  ").unwrap().to_raw(),
            123_450_000_000_000
        );
    }

    #[test]
    fn test_from_str_errors() {
        assert!(D96::from_str_exact("").is_err());
        assert!(D96::from_str_exact("abc").is_err());
        assert!(D96::from_str_exact("12.34.56").is_err());
        assert!(matches!(
            D96::from_str_exact("123.1234567890123"),
            Err(DecimalError::PrecisionLoss)
        ));
    }

    #[test]
    fn test_from_str_overflow() {
        assert!(matches!(
            D96::from_str_exact("999999999999999999999999999"),
            Err(DecimalError::Overflow)
        ));
    }

    #[test]
    fn test_from_str_trait() {
        let d: D96 = "123.45".parse().unwrap();
        assert_eq!(d.to_raw(), 123_450_000_000_000);
    }

    #[test]
    fn test_from_str_edge_cases() {
        // Just decimal point
        assert!(D96::from_str_exact(".").is_err());

        // Multiple signs
        assert!(D96::from_str_exact("--123").is_err());

        // Sign after number
        assert!(D96::from_str_exact("123-").is_err());
    }

    #[test]
    fn test_from_str_max_precision() {
        // Test all 12 decimal places
        let d = D96::from_str_exact("1.234567890123").unwrap();
        assert_eq!(d.to_raw(), 1_234_567_890_123);
    }

    #[test]
    fn test_d96_lossy_exact_precision() {
        let d = D96::from_str_lossy("123.123456789012").unwrap();
        assert_eq!(d, D96::from_str_exact("123.123456789012").unwrap());
    }

    #[test]
    fn test_d96_lossy_rounds_down() {
        let d = D96::from_str_lossy("123.1234567890124").unwrap(); // 13 decimals
        assert_eq!(d, D96::from_str_exact("123.123456789012").unwrap());
    }

    #[test]
    fn test_d96_lossy_rounds_up() {
        let d = D96::from_str_lossy("123.1234567890126").unwrap(); // 13 decimals
        assert_eq!(d, D96::from_str_exact("123.123456789013").unwrap());
    }

    #[test]
    fn test_d96_lossy_bankers_rounding() {
        let d = D96::from_str_lossy("123.1234567890125").unwrap(); // Exactly .5
        assert_eq!(d, D96::from_str_exact("123.123456789012").unwrap()); // Round to even
    }

    #[test]
    fn test_d96_lossy_ethereum_wei() {
        // 1 wei (18 decimals) rounds to 0 in D96 (12 decimals)
        let d = D96::from_str_lossy("0.000000000000000001").unwrap();
        assert_eq!(d, D96::ZERO);

        // 1000 wei = 1 microGwei (fits in D96)
        let d = D96::from_str_lossy("0.000000000001").unwrap();
        assert_eq!(d, D96::from_str_exact("0.000000000001").unwrap());
    }

    #[test]
    fn test_d96_lossy_many_excess_decimals() {
        let d = D96::from_str_lossy("1.12345678901234567890123456789").unwrap();
        assert_eq!(d, D96::from_str_exact("1.123456789012").unwrap());
    }
}

#[cfg(test)]
mod const_new_tests {
    use super::*;

    #[test]
    fn test_new_basic() {
        let d = D96::new(123, 450_000_000_000);
        assert_eq!(d, D96::from_str_exact("123.45").unwrap());
    }

    #[test]
    fn test_new_zero() {
        let d = D96::new(0, 0);
        assert_eq!(d, D96::ZERO);
    }

    #[test]
    fn test_new_integer_only() {
        let d = D96::new(42, 0);
        assert_eq!(d, D96::from_str_exact("42").unwrap());
    }

    #[test]
    fn test_new_fractional_only() {
        let d = D96::new(0, 500_000_000_000);
        assert_eq!(d, D96::from_str_exact("0.5").unwrap());
    }

    #[test]
    fn test_new_negative_integer() {
        let d = D96::new(-123, 450_000_000_000);
        assert_eq!(d, D96::from_str_exact("-123.45").unwrap());
    }

    #[test]
    fn test_new_max_fractional() {
        let d = D96::new(1, 999_999_999_999);
        assert_eq!(d, D96::from_str_exact("1.999999999999").unwrap());
    }

    #[test]
    fn test_new_const() {
        const RATE: D96 = D96::new(2, 500_000_000_000); // 2.5
        assert_eq!(RATE, D96::from_str_exact("2.5").unwrap());
    }

    #[test]
    fn test_new_large_values() {
        let d = D96::new(1_000_000, 123_456_789_012);
        assert_eq!(d, D96::from_str_exact("1000000.123456789012").unwrap());
    }

    #[test]
    fn test_new_micro_gwei_precision() {
        // Test minimum precision (12 decimals = 1 unit = 1000 wei)
        let d = D96::new(0, 1); // 1 unit = 0.000000000001
        assert_eq!(d, D96::from_str_exact("0.000000000001").unwrap());
    }
}

#[cfg(test)]
mod mul_i128_tests {
    use super::*;

    #[test]
    fn test_mul_i128_basic() {
        let price = D96::from_str_exact("10.50").unwrap();
        let quantity = 5;

        let total = price.mul_i128(quantity).unwrap();
        assert_eq!(total, D96::from_str_exact("52.50").unwrap());
    }

    #[test]
    fn test_mul_i128_zero() {
        let price = D96::from_str_exact("100.00").unwrap();
        let total = price.mul_i128(0).unwrap();
        assert_eq!(total, D96::ZERO);
    }

    #[test]
    fn test_mul_i128_one() {
        let price = D96::from_str_exact("42.42").unwrap();
        let total = price.mul_i128(1).unwrap();
        assert_eq!(total, price);
    }

    #[test]
    fn test_mul_i128_negative_quantity() {
        let price = D96::from_str_exact("10.00").unwrap();
        let total = price.mul_i128(-5).unwrap();
        assert_eq!(total, D96::from_str_exact("-50.00").unwrap());
    }

    #[test]
    fn test_mul_i128_negative_price() {
        let price = D96::from_str_exact("-25.50").unwrap();
        let total = price.mul_i128(4).unwrap();
        assert_eq!(total, D96::from_str_exact("-102.00").unwrap());
    }

    #[test]
    fn test_mul_i128_both_negative() {
        let price = D96::from_str_exact("-10.00").unwrap();
        let total = price.mul_i128(-3).unwrap();
        assert_eq!(total, D96::from_str_exact("30.00").unwrap());
    }

    #[test]
    fn test_mul_i128_overflow() {
        let price = D96::MAX;
        let result = price.mul_i128(2);
        assert!(result.is_none());
    }

    #[test]
    fn test_mul_i128_large_quantity() {
        let price = D96::from_str_exact("0.01").unwrap(); // 1 cent
        let quantity = 1_000_000; // 1 million

        let total = price.mul_i128(quantity).unwrap();
        assert_eq!(total, D96::from_str_exact("10000.00").unwrap());
    }

    #[test]
    fn test_try_mul_i128_success() {
        let price = D96::from_str_exact("5.25").unwrap();
        let total = price.try_mul_i128(10).unwrap();
        assert_eq!(total, D96::from_str_exact("52.50").unwrap());
    }

    #[test]
    fn test_try_mul_i128_overflow_error() {
        let price = D96::MAX;
        let result = price.try_mul_i128(2);
        assert!(matches!(result, Err(DecimalError::Overflow)));
    }

    #[test]
    fn test_mul_i128_fractional_result() {
        let price = D96::from_str_exact("3.333333333333").unwrap();
        let total = price.mul_i128(3).unwrap();
        assert_eq!(total, D96::from_str_exact("9.999999999999").unwrap());
    }
}

#[cfg(test)]
mod fixed_point_str_tests {
    use super::*;

    #[test]
    fn test_from_fixed_point_str_2_decimals() {
        // Common for currencies: "12345" with 2 decimals → 123.45
        let d = D96::from_fixed_point_str("12345", 2).unwrap();
        assert_eq!(d, D96::from_str_exact("123.45").unwrap());
    }

    #[test]
    fn test_from_fixed_point_str_0_decimals() {
        // No decimals: "123" with 0 decimals → 123.00
        let d = D96::from_fixed_point_str("123", 0).unwrap();
        assert_eq!(d, D96::from_str_exact("123").unwrap());
    }

    #[test]
    fn test_from_fixed_point_str_12_decimals() {
        // All decimals: "123456789012" with 12 decimals → 0.123456789012
        let d = D96::from_fixed_point_str("123456789012", 12).unwrap();
        assert_eq!(d, D96::from_str_exact("0.123456789012").unwrap());
    }

    #[test]
    fn test_from_fixed_point_str_9_decimals() {
        // Mid-range: "1234567890" with 9 decimals → 1.234567890
        let d = D96::from_fixed_point_str("1234567890", 9).unwrap();
        assert_eq!(d, D96::from_str_exact("1.234567890").unwrap());
    }

    #[test]
    fn test_from_fixed_point_str_negative() {
        let d = D96::from_fixed_point_str("-12345", 2).unwrap();
        assert_eq!(d, D96::from_str_exact("-123.45").unwrap());
    }

    #[test]
    fn test_from_fixed_point_str_zero() {
        let d = D96::from_fixed_point_str("0", 2).unwrap();
        assert_eq!(d, D96::ZERO);
    }

    #[test]
    fn test_from_fixed_point_str_leading_zeros() {
        // "00123" with 2 decimals → 1.23
        let d = D96::from_fixed_point_str("00123", 2).unwrap();
        assert_eq!(d, D96::from_str_exact("1.23").unwrap());
    }

    #[test]
    fn test_from_fixed_point_str_invalid_format() {
        let result = D96::from_fixed_point_str("not_a_number", 2);
        assert!(matches!(result, Err(DecimalError::InvalidFormat)));
    }

    #[test]
    fn test_from_fixed_point_str_overflow() {
        // Very large number that will overflow when scaled
        let result = D96::from_fixed_point_str("170141183460469231731687303715884105727", 2);
        assert!(matches!(result, Err(DecimalError::Overflow)));
    }

    #[test]
    fn test_from_fixed_point_str_parse_error() {
        // Number too large to even parse into i128
        let result = D96::from_fixed_point_str("99999999999999999999999999999999999999999", 2);
        assert!(matches!(result, Err(DecimalError::InvalidFormat)));
    }

    #[test]
    fn test_from_fixed_point_str_gwei() {
        // Gwei (9 decimals) - 1 gwei
        let d = D96::from_fixed_point_str("1000000000", 9).unwrap();
        assert_eq!(d, D96::from_str_exact("1.0").unwrap());
    }
}

#[cfg(test)]
mod basis_points_tests {
    use super::*;

    #[test]
    fn test_from_basis_points_basic() {
        // 100 bps = 1%
        let d = D96::from_basis_points(100).unwrap();
        assert_eq!(d, D96::from_str_exact("0.01").unwrap());
    }

    #[test]
    fn test_from_basis_points_zero() {
        let d = D96::from_basis_points(0).unwrap();
        assert_eq!(d, D96::ZERO);
    }

    #[test]
    fn test_from_basis_points_one() {
        // 1 bp = 0.0001
        let d = D96::from_basis_points(1).unwrap();
        assert_eq!(d, D96::from_str_exact("0.0001").unwrap());
    }

    #[test]
    fn test_from_basis_points_50() {
        // 50 bps = 0.5%
        let d = D96::from_basis_points(50).unwrap();
        assert_eq!(d, D96::from_str_exact("0.005").unwrap());
    }

    #[test]
    fn test_from_basis_points_10000() {
        // 10000 bps = 100%
        let d = D96::from_basis_points(10000).unwrap();
        assert_eq!(d, D96::from_str_exact("1").unwrap());
    }

    #[test]
    fn test_from_basis_points_negative() {
        // -25 bps
        let d = D96::from_basis_points(-25).unwrap();
        assert_eq!(d, D96::from_str_exact("-0.0025").unwrap());
    }

    #[test]
    fn test_to_basis_points_basic() {
        // 1% = 100 bps
        let d = D96::from_str_exact("0.01").unwrap();
        assert_eq!(d.to_basis_points(), 100);
    }

    #[test]
    fn test_to_basis_points_zero() {
        assert_eq!(D96::ZERO.to_basis_points(), 0);
    }

    #[test]
    fn test_to_basis_points_one() {
        let d = D96::from_str_exact("0.0001").unwrap();
        assert_eq!(d.to_basis_points(), 1);
    }

    #[test]
    fn test_to_basis_points_50() {
        let d = D96::from_str_exact("0.005").unwrap();
        assert_eq!(d.to_basis_points(), 50);
    }

    #[test]
    fn test_to_basis_points_negative() {
        let d = D96::from_str_exact("-0.0025").unwrap();
        assert_eq!(d.to_basis_points(), -25);
    }

    #[test]
    fn test_to_basis_points_fractional_truncates() {
        // 0.00015 = 1.5 bps, truncates to 1
        let d = D96::from_str_exact("0.00015").unwrap();
        assert_eq!(d.to_basis_points(), 1);
    }

    #[test]
    fn test_basis_points_round_trip() {
        let original_bps = 250; // 2.5%
        let d = D96::from_basis_points(original_bps).unwrap();
        let back_to_bps = d.to_basis_points();
        assert_eq!(original_bps, back_to_bps);
    }

    #[test]
    fn test_basis_points_interest_rate() {
        // Fed funds rate move of 25 bps
        let rate_change = D96::from_basis_points(25).unwrap();
        let old_rate = D96::from_str_exact("5.25").unwrap(); // 5.25%
        let new_rate = old_rate + rate_change;
        assert_eq!(new_rate, D96::from_str_exact("5.2525").unwrap());
    }

    #[test]
    fn test_basis_points_spread() {
        // Credit spread of 150 bps over treasuries
        let spread = D96::from_basis_points(150).unwrap();
        assert_eq!(spread, D96::from_str_exact("0.015").unwrap());
    }

    #[test]
    fn test_from_basis_points_overflow() {
        // Very large number that will overflow
        let result = D96::from_basis_points(i128::MAX);
        assert!(result.is_none());
    }
}

#[cfg(test)]
mod byte_tests {
    use super::*;

    #[test]
    fn test_to_le_bytes() {
        let d = D96::from_raw(0x0123456789ABCDEF_u128 as i128);
        let bytes = d.to_le_bytes();
        assert_eq!(
            bytes,
            [
                0xEF, 0xCD, 0xAB, 0x89, 0x67, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00
            ]
        );
    }

    #[test]
    fn test_from_le_bytes() {
        let bytes = [
            0xEF, 0xCD, 0xAB, 0x89, 0x67, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let d = D96::from_le_bytes(bytes);
        assert_eq!(d.to_raw(), 0x0123456789ABCDEF_u128 as i128);
    }

    #[test]
    fn test_to_be_bytes() {
        let d = D96::from_raw(0x0123456789ABCDEF_u128 as i128);
        let bytes = d.to_be_bytes();
        assert_eq!(
            bytes,
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB,
                0xCD, 0xEF
            ]
        );
    }

    #[test]
    fn test_from_be_bytes() {
        let bytes = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB,
            0xCD, 0xEF,
        ];
        let d = D96::from_be_bytes(bytes);
        assert_eq!(d.to_raw(), 0x0123456789ABCDEF_u128 as i128);
    }

    #[test]
    fn test_round_trip_le() {
        let original = D96::from_raw(123_456_789_012_345);
        let bytes = original.to_le_bytes();
        let restored = D96::from_le_bytes(bytes);
        assert_eq!(original, restored);
    }

    #[test]
    fn test_round_trip_be() {
        let original = D96::from_raw(-987_654_321_098_765);
        let bytes = original.to_be_bytes();
        let restored = D96::from_be_bytes(bytes);
        assert_eq!(original, restored);
    }

    #[test]
    fn test_round_trip_ne() {
        let original = D96::from_str_exact("123.456789012").unwrap();
        let bytes = original.to_ne_bytes();
        let restored = D96::from_ne_bytes(bytes);
        assert_eq!(original, restored);
    }

    #[test]
    fn test_bytes_constant() {
        assert_eq!(D96::BYTES, 16);
    }

    #[test]
    fn test_zero_bytes() {
        let zero_bytes = D96::ZERO.to_le_bytes();
        assert_eq!(zero_bytes, [0u8; 16]);
        assert_eq!(D96::from_le_bytes(zero_bytes), D96::ZERO);
    }
}

#[cfg(test)]
mod buffer_tests {
    use super::*;

    #[test]
    fn test_write_read_le_bytes() {
        let d = D96::from_str_exact("123.45").unwrap();
        let mut buf = [0u8; 32];

        d.write_le_bytes(&mut buf[8..]);
        let restored = D96::read_le_bytes(&buf[8..]);

        assert_eq!(d, restored);
    }

    #[test]
    fn test_write_read_be_bytes() {
        let d = D96::from_str_exact("-987.654321098").unwrap();
        let mut buf = [0u8; 32];

        d.write_be_bytes(&mut buf[8..]);
        let restored = D96::read_be_bytes(&buf[8..]);

        assert_eq!(d, restored);
    }

    #[test]
    fn test_write_read_ne_bytes() {
        let d = D96::from_raw(999_888_777_666_555);
        let mut buf = [0u8; 16];

        d.write_ne_bytes(&mut buf);
        let restored = D96::read_ne_bytes(&buf);

        assert_eq!(d, restored);
    }

    #[test]
    #[should_panic]
    fn test_write_le_bytes_panic_short_buffer() {
        let d = D96::ONE;
        let mut buf = [0u8; 8];
        d.write_le_bytes(&mut buf);
    }

    #[test]
    #[should_panic]
    fn test_read_le_bytes_panic_short_buffer() {
        let buf = [0u8; 8];
        let _ = D96::read_le_bytes(&buf);
    }

    #[test]
    fn test_try_write_le_bytes_success() {
        let d = D96::from_raw(123_456_789_012_345);
        let mut buf = [0u8; 20];

        assert!(d.try_write_le_bytes(&mut buf[4..]).is_some());
        assert_eq!(D96::read_le_bytes(&buf[4..]), d);
    }

    #[test]
    fn test_try_write_le_bytes_failure() {
        let d = D96::ONE;
        let mut buf = [0u8; 8];

        assert!(d.try_write_le_bytes(&mut buf).is_none());
    }

    #[test]
    fn test_try_read_le_bytes_success() {
        let d = D96::from_str_exact("42.42").unwrap();
        let bytes = d.to_le_bytes();

        assert_eq!(D96::try_read_le_bytes(&bytes), Some(d));
    }

    #[test]
    fn test_try_read_le_bytes_failure() {
        let buf = [0u8; 8];
        assert!(D96::try_read_le_bytes(&buf).is_none());
    }

    #[test]
    fn test_multiple_writes_to_buffer() {
        let prices = [
            D96::from_str_exact("100.50").unwrap(),
            D96::from_str_exact("200.75").unwrap(),
            D96::from_str_exact("300.25").unwrap(),
        ];

        let mut buf = [0u8; 48];

        for (i, price) in prices.iter().enumerate() {
            price.write_le_bytes(&mut buf[i * 16..]);
        }

        for (i, expected) in prices.iter().enumerate() {
            let actual = D96::read_le_bytes(&buf[i * 16..]);
            assert_eq!(*expected, actual);
        }
    }

    #[test]
    fn test_buffer_at_exact_size() {
        let d = D96::from_raw(-12345);
        let mut buf = [0u8; 16];

        d.write_le_bytes(&mut buf);
        assert_eq!(D96::read_le_bytes(&buf), d);
    }
}

#[cfg(test)]
mod display_tests {
    use std::format;

    use super::*;

    #[test]
    fn test_display_integer() {
        assert_eq!(format!("{}", D96::from_raw(1_000_000_000_000)), "1");
        assert_eq!(format!("{}", D96::from_raw(42_000_000_000_000)), "42");
        assert_eq!(format!("{}", D96::ZERO), "0");
    }

    #[test]
    fn test_display_decimal() {
        assert_eq!(format!("{}", D96::from_raw(1_234_500_000_000)), "1.2345");
        assert_eq!(
            format!("{}", D96::from_raw(1_000_000_000_001)),
            "1.000000000001"
        );
    }

    #[test]
    fn test_display_negative() {
        assert_eq!(format!("{}", D96::from_raw(-1_000_000_000_000)), "-1");
        assert_eq!(format!("{}", D96::from_raw(-1_234_500_000_000)), "-1.2345");
    }

    #[test]
    fn test_display_trailing_zeros_stripped() {
        assert_eq!(format!("{}", D96::from_raw(1_230_000_000_000)), "1.23");
        assert_eq!(format!("{}", D96::from_raw(1_001_000_000_000)), "1.001");
    }

    #[test]
    fn test_display_precision() {
        let d = D96::from_raw(1_234_567_890_123); // 1.234567890123

        assert_eq!(format!("{:.0}", d), "1");
        assert_eq!(format!("{:.2}", d), "1.23");
        assert_eq!(format!("{:.9}", d), "1.234567890");
        assert_eq!(format!("{:.12}", d), "1.234567890123");
    }

    #[test]
    fn test_display_precision_rounding() {
        let d = D96::from_raw(1_255_000_000_000); // 1.255
        assert_eq!(format!("{:.2}", d), "1.26"); // Rounds up

        let d = D96::from_raw(1_254_000_000_000); // 1.254
        assert_eq!(format!("{:.2}", d), "1.25"); // Rounds down
    }

    #[test]
    fn test_display_zero() {
        assert_eq!(format!("{}", D96::ZERO), "0");
        assert_eq!(format!("{:.2}", D96::ZERO), "0.00");
    }

    #[test]
    fn test_display_micro_gwei() {
        // Display 1 unit = 0.000000000001 (1 microGwei)
        let micro_gwei = D96::from_raw(1);
        assert_eq!(format!("{}", micro_gwei), "0.000000000001");
    }
}

#[cfg(test)]
mod utf8_bytes_tests {
    use super::*;

    #[test]
    fn test_from_utf8_bytes_integer() {
        let bytes = b"123";
        let d = D96::from_utf8_bytes(bytes).unwrap();
        assert_eq!(d, D96::from_str_exact("123").unwrap());
    }

    #[test]
    fn test_from_utf8_bytes_decimal() {
        let bytes = b"123.45";
        let d = D96::from_utf8_bytes(bytes).unwrap();
        assert_eq!(d, D96::from_str_exact("123.45").unwrap());
    }

    #[test]
    fn test_from_utf8_bytes_negative() {
        let bytes = b"-987.654321098";
        let d = D96::from_utf8_bytes(bytes).unwrap();
        assert_eq!(d, D96::from_str_exact("-987.654321098").unwrap());
    }

    #[test]
    fn test_from_utf8_bytes_with_whitespace() {
        let bytes = b"  42.42  ";
        let d = D96::from_utf8_bytes(bytes).unwrap();
        assert_eq!(d, D96::from_str_exact("42.42").unwrap());
    }

    #[test]
    fn test_from_utf8_bytes_invalid_utf8() {
        let bytes = &[0xFF, 0xFE, 0xFD]; // Invalid UTF-8
        let result = D96::from_utf8_bytes(bytes);
        assert!(matches!(result, Err(DecimalError::InvalidFormat)));
    }

    #[test]
    fn test_from_utf8_bytes_invalid_decimal() {
        let bytes = b"not a number";
        let result = D96::from_utf8_bytes(bytes);
        assert!(matches!(result, Err(DecimalError::InvalidFormat)));
    }

    #[test]
    fn test_from_utf8_bytes_empty() {
        let bytes = b"";
        let result = D96::from_utf8_bytes(bytes);
        assert!(matches!(result, Err(DecimalError::InvalidFormat)));
    }

    #[test]
    fn test_from_utf8_bytes_zero() {
        let bytes = b"0";
        let d = D96::from_utf8_bytes(bytes).unwrap();
        assert_eq!(d, D96::ZERO);
    }

    #[test]
    fn test_from_utf8_bytes_from_network_buffer() {
        // Simulate reading from a network packet
        let packet = b"PRICE:100.50;QTY:1000";
        let price_bytes = &packet[6..12]; // "100.50"

        let price = D96::from_utf8_bytes(price_bytes).unwrap();
        assert_eq!(price, D96::from_str_exact("100.50").unwrap());
    }

    #[test]
    fn test_from_utf8_bytes_max_precision() {
        let bytes = b"1.234567890123";
        let d = D96::from_utf8_bytes(bytes).unwrap();
        assert_eq!(d, D96::from_str_exact("1.234567890123").unwrap());
    }
}

#[cfg(test)]
mod percentage_tests {
    use super::*;

    #[test]
    fn test_percent_of_basic() {
        let amount = D96::from_str_exact("1000").unwrap();
        let percent = D96::from_str_exact("5").unwrap(); // 5%

        let result = amount.percent_of(percent).unwrap();
        assert_eq!(result, D96::from_str_exact("50").unwrap()); // 5% of 1000 = 50
    }

    #[test]
    fn test_percent_of_decimal() {
        let amount = D96::from_str_exact("250.50").unwrap();
        let percent = D96::from_str_exact("10").unwrap(); // 10%

        let result = amount.percent_of(percent).unwrap();
        assert_eq!(result, D96::from_str_exact("25.05").unwrap()); // 10% of 250.50 = 25.05
    }

    #[test]
    fn test_percent_of_fractional_percent() {
        let amount = D96::from_str_exact("1000").unwrap();
        let percent = D96::from_str_exact("2.5").unwrap(); // 2.5%

        let result = amount.percent_of(percent).unwrap();
        assert_eq!(result, D96::from_str_exact("25").unwrap()); // 2.5% of 1000 = 25
    }

    #[test]
    fn test_percent_of_zero() {
        let amount = D96::from_str_exact("1000").unwrap();
        let percent = D96::ZERO;

        let result = amount.percent_of(percent).unwrap();
        assert_eq!(result, D96::ZERO);
    }

    #[test]
    fn test_percent_of_hundred() {
        let amount = D96::from_str_exact("500").unwrap();
        let percent = D96::from_str_exact("100").unwrap(); // 100%

        let result = amount.percent_of(percent).unwrap();
        assert_eq!(result, D96::from_str_exact("500").unwrap()); // 100% of 500 = 500
    }

    #[test]
    fn test_percent_of_negative_amount() {
        let amount = D96::from_str_exact("-1000").unwrap();
        let percent = D96::from_str_exact("5").unwrap();

        let result = amount.percent_of(percent).unwrap();
        assert_eq!(result, D96::from_str_exact("-50").unwrap());
    }

    #[test]
    fn test_percent_of_overflow() {
        let amount = D96::MAX;
        let percent = D96::from_str_exact("200").unwrap();

        let result = amount.percent_of(percent);
        assert!(result.is_none());
    }

    #[test]
    fn test_add_percent_basic() {
        let amount = D96::from_str_exact("1000").unwrap();
        let percent = D96::from_str_exact("5").unwrap(); // Add 5%

        let result = amount.add_percent(percent).unwrap();
        assert_eq!(result, D96::from_str_exact("1050").unwrap()); // 1000 + 5% = 1050
    }

    #[test]
    fn test_add_percent_decimal() {
        let amount = D96::from_str_exact("200").unwrap();
        let percent = D96::from_str_exact("10").unwrap(); // Add 10%

        let result = amount.add_percent(percent).unwrap();
        assert_eq!(result, D96::from_str_exact("220").unwrap()); // 200 + 10% = 220
    }

    #[test]
    fn test_add_percent_fractional() {
        let amount = D96::from_str_exact("1000").unwrap();
        let percent = D96::from_str_exact("2.5").unwrap(); // Add 2.5%

        let result = amount.add_percent(percent).unwrap();
        assert_eq!(result, D96::from_str_exact("1025").unwrap()); // 1000 + 2.5% = 1025
    }

    #[test]
    fn test_add_percent_zero() {
        let amount = D96::from_str_exact("1000").unwrap();
        let percent = D96::ZERO;

        let result = amount.add_percent(percent).unwrap();
        assert_eq!(result, amount); // Adding 0% returns original
    }

    #[test]
    fn test_add_percent_negative() {
        let amount = D96::from_str_exact("1000").unwrap();
        let percent = D96::from_str_exact("-10").unwrap(); // Subtract 10%

        let result = amount.add_percent(percent).unwrap();
        assert_eq!(result, D96::from_str_exact("900").unwrap()); // 1000 - 10% = 900
    }

    #[test]
    fn test_add_percent_negative_amount() {
        let amount = D96::from_str_exact("-1000").unwrap();
        let percent = D96::from_str_exact("5").unwrap();

        let result = amount.add_percent(percent).unwrap();
        assert_eq!(result, D96::from_str_exact("-1050").unwrap());
    }

    #[test]
    fn test_add_percent_overflow() {
        let amount = D96::MAX;
        let percent = D96::from_str_exact("50").unwrap();

        let result = amount.add_percent(percent);
        assert!(result.is_none());
    }

    #[test]
    fn test_add_percent_hundred() {
        let amount = D96::from_str_exact("500").unwrap();
        let percent = D96::from_str_exact("100").unwrap(); // Add 100% = double

        let result = amount.add_percent(percent).unwrap();
        assert_eq!(result, D96::from_str_exact("1000").unwrap());
    }

    #[test]
    fn test_percent_commission_calculation() {
        // Real-world example: $10,000 trade with 0.1% commission
        let trade_value = D96::from_str_exact("10000").unwrap();
        let commission_rate = D96::from_str_exact("0.1").unwrap();

        let commission = trade_value.percent_of(commission_rate).unwrap();
        assert_eq!(commission, D96::from_str_exact("10").unwrap());
    }

    #[test]
    fn test_percent_tax_calculation() {
        // Real-world example: $99.99 item with 8.5% tax
        let price = D96::from_str_exact("99.99").unwrap();
        let tax_rate = D96::from_str_exact("8.5").unwrap();

        let total = price.add_percent(tax_rate).unwrap();
        // 99.99 * 1.085 = 108.48915
        // Rounded to 2 decimal places = 108.49
        assert_eq!(total.round_dp(2), D96::from_str_exact("108.49").unwrap());
    }

    #[test]
    fn test_percent_discount_calculation() {
        // Real-world example: $299 item with 15% discount
        let price = D96::from_str_exact("299").unwrap();
        let discount_rate = D96::from_str_exact("-15").unwrap(); // Negative for discount

        let final_price = price.add_percent(discount_rate).unwrap();
        assert_eq!(final_price, D96::from_str_exact("254.15").unwrap());
    }
}

#[cfg(all(test, feature = "serde"))]
mod serde_tests {
    use super::*;

    #[test]
    fn test_serialize() {
        let d = D96::from_str("123.45").unwrap();
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, r#""123.45""#);
    }

    #[test]
    fn test_deserialize() {
        let json = r#""123.45""#;
        let d: D96 = serde_json::from_str(json).unwrap();
        assert_eq!(d, D96::from_str("123.45").unwrap());
    }

    #[test]
    fn test_round_trip() {
        let original = D96::from_str("123.456789012").unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: D96 = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_deserialize_integer() {
        let json = r#""42""#;
        let d: D96 = serde_json::from_str(json).unwrap();
        assert_eq!(d, D96::from_i32(42));
    }

    #[test]
    fn test_serialize_zero() {
        let json = serde_json::to_string(&D96::ZERO).unwrap();
        assert_eq!(json, r#""0""#);
    }

    #[test]
    fn test_serialize_negative() {
        let d = D96::from_str("-123.45").unwrap();
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, r#""-123.45""#);
    }

    #[test]
    fn test_serialize_max_precision() {
        let d = D96::from_str("1.234567890123").unwrap();
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, r#""1.234567890123""#);
    }
}

// Crypto-specific constant tests
#[cfg(test)]
mod crypto_constant_tests {
    use super::*;

    #[test]
    fn test_gwei_constant() {
        assert_eq!(D96::GWEI, D96::from_str_exact("0.000000001").unwrap());
    }

    #[test]
    fn test_satoshi_constant() {
        assert_eq!(D96::SATOSHI, D96::from_str_exact("0.00000001").unwrap());
    }

    #[test]
    fn test_eth_conversion() {
        // 1 ETH = 10^12 units (with 12 decimals)
        let one_eth = D96::ONE;
        let units_per_eth = one_eth.to_raw();
        assert_eq!(units_per_eth, 1_000_000_000_000);
    }

    #[test]
    fn test_gwei_to_eth() {
        // 1 gwei = 10^9 wei = 10^-9 ETH
        // With 12 decimals: 1 gwei = 0.001 units (since 1 unit = 10^-12)
        // So 1 billion gwei = 1 ETH
        let gwei_amount = D96::from_i64(1_000_000_000); // 1 billion gwei
        let eth_amount = gwei_amount * D96::GWEI;
        assert_eq!(eth_amount, D96::ONE); // Should equal 1 ETH
    }

    #[test]
    fn test_satoshi_to_btc() {
        // 1 BTC = 10^8 satoshis
        let satoshi_amount = D96::from_i64(100_000_000); // 100 million satoshis
        let btc_amount = satoshi_amount * D96::SATOSHI;
        assert_eq!(btc_amount, D96::ONE); // Should equal 1 BTC
    }

    #[test]
    fn test_micro_gwei_constant() {
        // 1 microGwei = 1 unit = 0.000000000001
        assert_eq!(D96::MICRO_GWEI.to_raw(), 1);
        assert_eq!(D96::KILO_WEI.to_raw(), 1);
    }
}

// Property-based testing for D96 multiplication, mirroring the D64 suite.
#[cfg(test)]
mod d96_mul_property_tests {
    use super::*;
    use proptest::prelude::*;

    /// Exact reference for the common case where the product of the two raw
    /// operands fits in `i128` (both fit in `i64`, so the product is < 2^127).
    /// Wider operands exercise the 192-bit slow path and are covered by the
    /// explicit unit tests below.
    fn mul_oracle_i128(a: i64, b: i64) -> Option<i128> {
        let product = (a as i128) * (b as i128);
        let quotient = product / D96::SCALE; // truncates toward zero
        if quotient > D96::MAX.to_raw() || quotient < D96::MIN.to_raw() {
            None
        } else {
            Some(quotient)
        }
    }

    proptest! {
        #[test]
        fn prop_mul_matches_oracle(a in i64::MIN..=i64::MAX, b in i64::MIN..=i64::MAX) {
            let got = D96::from_raw(a as i128)
                .checked_mul(D96::from_raw(b as i128))
                .map(D96::to_raw);
            prop_assert_eq!(got, mul_oracle_i128(a, b), "a={}, b={}", a, b);
        }

        #[test]
        fn prop_mul_commutative(a in i64::MIN..=i64::MAX, b in i64::MIN..=i64::MAX) {
            prop_assert_eq!(
                D96::from_raw(a as i128).checked_mul(D96::from_raw(b as i128)),
                D96::from_raw(b as i128).checked_mul(D96::from_raw(a as i128)),
            );
        }

        #[test]
        fn prop_mul_identity(a in i64::MIN..=i64::MAX) {
            let d = D96::from_raw(a as i128);
            prop_assert_eq!(d.checked_mul(D96::ONE), Some(d));
        }

        #[test]
        fn prop_to_i128_truncates(a in i64::MIN..=i64::MAX) {
            prop_assert_eq!(D96::from_raw(a as i128).to_i128(), (a as i128) / D96::SCALE);
        }
    }

    /// Maps an arbitrary i128 into the legal 96-bit raw range [MIN, MAX].
    fn clamp_raw_96(v: i128) -> i128 {
        const TWO_POW_96: i128 = 1i128 << 96;
        let mut r = v % TWO_POW_96;
        if r > D96::MAX.to_raw() {
            r -= TWO_POW_96;
        } else if r < D96::MIN.to_raw() {
            r += TWO_POW_96;
        }
        r
    }

    proptest! {
        // The proptests above cap raw operands at i64 range, so |operand| is
        // always < 2^64 = FAST_THRESHOLD and the 192-bit SLOW path (plus its
        // signed-bound logic and the MIN boundary) is never exercised. These
        // generators span the full 96-bit raw range so they reach the slow path
        // and signed-MIN, which is where the off-by-one/wrap bugs lived.

        /// Force the slow path with an exact i128 oracle: |a| spans the full
        /// 96-bit range (so |a| can exceed 2^64), while |b| stays small enough
        /// that `a * b` still fits i128 (|a| <= 2^95, |b| <= 2^31 -> < 2^127).
        #[test]
        fn prop_mul_slow_path_matches_oracle(
            a_seed in any::<i128>(),
            b in -(1i128 << 31)..=(1i128 << 31),
        ) {
            let a = clamp_raw_96(a_seed);
            let product = a * b;
            let quotient = product / D96::SCALE; // truncates toward zero
            let expect = if quotient > D96::MAX.to_raw() || quotient < D96::MIN.to_raw() {
                None
            } else {
                Some(quotient)
            };
            let got = D96::from_raw(a).checked_mul(D96::from_raw(b)).map(D96::to_raw);
            prop_assert_eq!(got, expect, "a={} b={}", a, b);
        }

        /// Multiplicative identity must hold for EVERY representable raw value,
        /// including those >= 2^64 (slow path) and MIN (= -2^95).
        #[test]
        fn prop_mul_identity_full_range(a_seed in any::<i128>()) {
            let d = D96::from_raw(clamp_raw_96(a_seed));
            prop_assert_eq!(d.checked_mul(D96::ONE), Some(d));
            prop_assert_eq!(D96::ONE.checked_mul(d), Some(d));
        }

        /// Division by one must round-trip for every representable raw value,
        /// exercising the divide slow path and the MIN result boundary.
        #[test]
        fn prop_div_by_one_full_range(a_seed in any::<i128>()) {
            let d = D96::from_raw(clamp_raw_96(a_seed));
            prop_assert_eq!(d.checked_div(D96::ONE), Some(d));
        }

        /// Every wrapping multiply/divide must stay inside the 96-bit range.
        #[test]
        fn prop_wrapping_muldiv_stays_in_range(
            a_seed in any::<i128>(),
            b_seed in any::<i128>(),
        ) {
            let a = D96::from_raw(clamp_raw_96(a_seed));
            let b = D96::from_raw(clamp_raw_96(b_seed));
            let m = a.wrapping_mul(b).to_raw();
            prop_assert!(D96::from_raw_checked(m).is_some(), "wrapping_mul out of range: {}", m);
            if b.to_raw() != 0 {
                let q = a.wrapping_div(b).to_raw();
                prop_assert!(D96::from_raw_checked(q).is_some(), "wrapping_div out of range: {}", q);
            }
        }
    }
}

#[cfg(test)]
mod d96_mul_regression_tests {
    use super::*;
    use std::string::ToString;

    #[test]
    fn test_mul_large_exact_fast_path() {
        assert_eq!(
            (D96::from_i32(100) * D96::from_i32(100)).to_raw(),
            10_000i128 * D96::SCALE
        );
        assert_eq!(
            (D96::from_i32(1_000_000) * D96::from_i32(1_000_000)).to_raw(),
            1_000_000_000_000i128 * D96::SCALE
        );
    }

    #[test]
    fn test_mul_slow_path_exact() {
        // from_i64(20_000_000) has raw 2e19 >= 2^64, forcing the 192-bit path.
        let a = D96::from_i64(20_000_000);
        let b = D96::from_i64(1_000);
        let r = a.checked_mul(b).unwrap();
        assert_eq!(r.to_raw(), 20_000_000_000i128 * D96::SCALE);
        assert_eq!(r.to_string(), "20000000000");
    }

    #[test]
    fn test_mul_fractional_exact() {
        let p = D96::from_str_exact("2500.123456789012").unwrap();
        let q = D96::from_str_exact("0.5").unwrap();
        // 2500.123456789012 * 0.5 = 1250.061728394506
        assert_eq!((p * q).to_string(), "1250.061728394506");
    }
}

#[cfg(test)]
mod d96_div_regression_tests {
    use super::*;
    use std::string::ToString;

    // Regression: div_192_by_u128 used base-2^64 limbs whose remainder shift
    // overflowed u128 when the divisor exceeded 2^64 (rhs value >= ~1.8e7),
    // silently dropping bits and returning a wildly wrong quotient. These cases
    // all force the base-2^32 slow path (divisor raw >= 2^64).
    #[test]
    fn test_div_large_divisor_exact() {
        let num = D96::from_i64(1_000_000_000_000_000); // 1e15
        let den = D96::from_i64(20_000_000); // raw 2e19 >= 2^64
        let r = num.checked_div(den).unwrap();
        assert_eq!(r.to_raw(), 50_000_000i128 * D96::SCALE); // 5e7
        assert_eq!(r.to_string(), "50000000");
    }

    #[test]
    fn test_div_large_divisor_fractional() {
        let num = D96::ONE;
        let den = D96::from_i64(20_000_000); // raw 2e19 >= 2^64
        let r = num.checked_div(den).unwrap();
        assert_eq!(r.to_raw(), 50_000); // 1 / 2e7 = 0.00000005
        assert_eq!(r.to_string(), "0.00000005");
    }

    #[test]
    fn test_div_very_large_operands() {
        let num = D96::from_i64(1_000_000_000_000_000); // 1e15
        let den = D96::from_i64(1_000_000_000_000); // 1e12, raw 1e24 >= 2^64
        let r = num.checked_div(den).unwrap();
        assert_eq!(r.to_raw(), 1_000i128 * D96::SCALE); // 1e3
    }
}
