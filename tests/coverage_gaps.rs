//! Fixtures closing four fixdec test-suite observability gaps identified during
//! the Nautilus->Quasar parity mutation-testing pass (seat FIXDEC-FIXTURES):
//!
//! 1. `Debug for D64` (`src/d64.rs:2952`) - neither format branch (`{:?}` /
//!    `{:#?}`) was exercised by any assertion on the passing-test path.
//! 2. `Hash for D64` - no fixture anywhere in the suite ever called
//!    `Hash::hash` on a `D64` (zero `HashMap`/`HashSet`/`BTreeMap` sites).
//! 3. `D64::saturating_sub` - every prior fixture used
//!    `D64::MIN.saturating_sub(D64::ONE) == D64::MIN`, where the expected
//!    result equals the receiver, so a `{ self }` mutant survives.
//! 4. `TryFrom<i64> for D64` - the only existing fixture
//!    (`src/d64.rs` `test_try_from`) checks `.is_ok()` / `.is_err()` only,
//!    never the produced value.
//!
//! Each capability below was proven non-vacuous by a matching mutation probe:
//! inject mutant -> this new test goes RED -> restore from a pristine backup
//! -> real recompile confirmed. See the seat report for the mutant text and
//! RED/restore evidence. This file only adds fixtures; no production code in
//! `src/` was touched or is touched by this file.

use fixdec::D64;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn hash_of<T: Hash>(v: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    v.hash(&mut hasher);
    hasher.finish()
}

// ===========================================================================
// 1. Debug for D64 - both format branches
// ===========================================================================

#[test]
fn d64_debug_non_alternate_wraps_display() {
    // {:?} takes the `else` branch: `write!(f, "D64({})", self)`.
    let v = D64::ONE;
    assert_eq!(format!("{v:?}"), "D64(1)");
}

#[test]
fn d64_debug_alternate_uses_debug_struct() {
    // {:#?} takes the `f.alternate()` branch:
    // `f.debug_struct("D64").field("value", &self.value).finish()`.
    let v = D64::ONE;
    assert_eq!(format!("{v:#?}"), "D64 {\n    value: 100000000,\n}");
}

// ===========================================================================
// 2. Hash for D64
// ===========================================================================

#[test]
fn d64_hash_equal_values_hash_equal() {
    // Hash/Eq contract: values that compare equal must hash equal.
    let a = D64::from_raw(42);
    let b = D64::from_raw(42);
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
}

#[test]
fn d64_hash_distinguishes_different_values() {
    // Non-vacuity: a degenerate (e.g. no-op) Hash impl would make every value
    // hash identically. Two distinct raw values must NOT collide under
    // DefaultHasher (SipHash-1-3 with fixed keys - deterministic here).
    let a = D64::from_raw(1);
    let b = D64::from_raw(2);
    assert_ne!(
        hash_of(&a),
        hash_of(&b),
        "distinct D64 values hashed identically - Hash impl looks degenerate"
    );
}

#[test]
fn d64_hash_set_contract() {
    // A realistic consumer: dedup + lookup through a HashSet.
    //
    // NOTE: this test alone does NOT prove Hash is non-degenerate - HashSet
    // correctness only requires that equal keys hash equal; a hash that
    // collapses every key into a single bucket is still "correct" here, just
    // slow. The two tests above are the actual non-vacuity proof.
    let mut set = HashSet::new();
    set.insert(D64::from_raw(1));
    set.insert(D64::from_raw(2));
    set.insert(D64::from_raw(1)); // duplicate of an existing key
    assert_eq!(set.len(), 2);
    assert!(set.contains(&D64::from_raw(1)));
    assert!(set.contains(&D64::from_raw(2)));
    assert!(!set.contains(&D64::from_raw(3)));
}

// ===========================================================================
// 3. D64::saturating_sub - non-saturating leg
// ===========================================================================

#[test]
fn d64_saturating_sub_ordinary_case_does_not_saturate() {
    // Every prior fixture (tests/edge_cases.rs:17, tests/math_edge_cases.rs:230,
    // tests/num_traits_impls.rs:206) only exercised the leg where the expected
    // result equals the receiver (D64::MIN.saturating_sub(D64::ONE) ==
    // D64::MIN), which a `fn saturating_sub(self, _rhs) -> Self { self }`
    // mutant satisfies trivially. This fixture is an ordinary subtraction,
    // nowhere near either bound, where the expected result is neither operand.
    let a = D64::from_raw(500);
    let b = D64::from_raw(200);
    let result = a.saturating_sub(b);
    assert_eq!(result, D64::from_raw(300));
    assert_ne!(result, a);
    assert_ne!(result, b);
}

// ===========================================================================
// 4. TryFrom<i64> for D64 - value-checking
// ===========================================================================

#[test]
fn d64_try_from_i64_value_is_checked() {
    // The existing `src/d64.rs` `test_try_from` only asserts `.is_ok()` /
    // `.is_err()`, so a mutant that always returns `Ok(D64::ZERO)` survives
    // it. This fixture checks the produced value.
    let d: D64 = D64::try_from(42i64).expect("42 fits in D64");
    assert_eq!(d, D64::from_i32(42));
    assert_eq!(d.to_i64(), 42);

    let neg: D64 = D64::try_from(-7i64).expect("-7 fits in D64");
    assert_eq!(neg, D64::from_i32(-7));

    // Error leg, included so this test is self-contained (also covered by the
    // existing fixture).
    assert!(D64::try_from(i64::MAX).is_err());
}
