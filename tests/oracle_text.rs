//! Differential tests of parsing, formatting and conversions for `D64` and `D96`
//! against the arbitrary-precision oracle (`tests/common`).
//!
//! Strings are generated from boundary raw values (the limits and their neighbours,
//! powers of ten, ties) and then mangled -- signs, leading/trailing zeros, a bare or
//! trailing point, whitespace, scientific forms, garbage -- because the bugs live at
//! the seams of the grammar. `spec_*` re-derive what a string means from the grammar
//! with `BigInt`, never calling the crate.
//!
//! Policy the spec pins down (all documented in the crate):
//! * `from_str_exact` accepts a trailing-zero fraction beyond the scale ("1.230000000")
//!   and rejects a SIGNIFICANT digit past it with `PrecisionLoss`;
//! * `from_str_lossy` rounds the excess half-to-even and never fails on precision;
//! * every spelling of zero is zero ("0", "-0", "0.000", "0e99");
//! * when a string is both too precise and too big, or malformed in a way that is also
//!   too precise, the error KIND is not specified -- only that it is an error.
//!
//! Scale the sampling with `FIXDEC_ORACLE_SCALE=10 cargo test --release --test oracle_text`.
mod common;
use common::*;

use fixdec::{DecimalError, DecimalError as E, RoundingStrategy};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{Signed, Zero};

fn scale() -> usize {
    std::env::var("FIXDEC_ORACLE_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}
fn fit<T: Dec>(x: &BigInt) -> Option<T> {
    T::from_raw_big(x)
}

// ---------------------------------------------------------------------------
// Parse specification
// ---------------------------------------------------------------------------

enum Lexed {
    Bad,
    Num {
        neg: bool,
        m: BigInt,
        frac_len: i64,
        exp: i64,
    },
}

fn lex(s: &str) -> Lexed {
    let t = s.trim();
    if t.is_empty() {
        return Lexed::Bad;
    }
    // Any 'e'/'E' anywhere sends the string down the scientific path.
    let (mant, exp) = match t.find(['e', 'E']) {
        Some(i) => (&t[..i], Some(&t[i + 1..])),
        None => (t, None),
    };
    let (neg, rest) = match mant.as_bytes().first() {
        Some(b'-') => (true, &mant[1..]),
        Some(b'+') => (false, &mant[1..]),
        _ => (false, mant),
    };
    let mut digits = String::new();
    let (mut frac_len, mut seen_dot) = (0i64, false);
    for c in rest.chars() {
        match c {
            '.' if !seen_dot => seen_dot = true,
            '0'..='9' => {
                digits.push(c);
                if seen_dot {
                    frac_len += 1;
                }
            }
            _ => return Lexed::Bad,
        }
    }
    if digits.is_empty() {
        return Lexed::Bad;
    }
    let e = match exp {
        None => 0,
        Some(x) => {
            let (eneg, ed) = match x.as_bytes().first() {
                Some(b'-') => (true, &x[1..]),
                Some(b'+') => (false, &x[1..]),
                _ => (false, x),
            };
            if ed.is_empty() || !ed.bytes().all(|b| b.is_ascii_digit()) {
                return Lexed::Bad;
            }
            let mag = ed.trim_start_matches('0');
            let mag: i64 = if mag.len() > 7 {
                10_000_000
            } else {
                mag.parse().unwrap_or(0)
            };
            if eneg { -mag } else { mag }
        }
    };
    Lexed::Num {
        neg,
        m: BigInt::parse_bytes(digits.as_bytes(), 10).unwrap(),
        frac_len,
        exp: e,
    }
}

#[derive(Debug, PartialEq, Clone)]
enum Spec {
    Ok(BigInt),
    /// Only these error kinds (more than one when several really apply).
    Kinds(Vec<E>),
}

fn in_range<T: Dec>(x: &BigInt) -> bool {
    *x >= T::lo() && *x <= T::hi()
}

/// A malformed string is `InvalidFormat` whatever else is wrong with it; a well-formed one
/// of ANY length is an exact value, a precision loss, or an overflow.
fn spec_parse<T: Dec>(s: &str, lossy: bool) -> Spec {
    let (neg, m, frac_len, exp) = match lex(s) {
        Lexed::Bad => return Spec::Kinds(vec![E::InvalidFormat]),
        Lexed::Num {
            neg,
            m,
            frac_len,
            exp,
        } => (neg, m, frac_len, exp),
    };
    if m.is_zero() {
        return Spec::Ok(BigInt::zero());
    }
    let net = exp - frac_len + T::DECIMALS as i64;
    let sign = |x: BigInt| if neg { -x } else { x };
    if net >= 0 {
        if net > 400 {
            return Spec::Kinds(vec![E::Overflow]);
        }
        let raw = sign(&m * pow10(net as u32));
        return if in_range::<T>(&raw) {
            Spec::Ok(raw)
        } else {
            Spec::Kinds(vec![E::Overflow])
        };
    }
    let k = -net;
    if lossy {
        let q = if k > 400 {
            BigInt::zero()
        } else {
            round_div(&m, &pow10(k as u32), RoundingStrategy::MidpointNearestEven)
        };
        let raw = sign(q);
        return if in_range::<T>(&raw) {
            Spec::Ok(raw)
        } else {
            Spec::Kinds(vec![E::Overflow])
        };
    }
    // exact: the value must be a whole number of raw units
    let (q, r) = if k > 400 {
        (BigInt::zero(), m.clone())
    } else {
        m.div_rem(&pow10(k as u32))
    };
    if r.is_zero() {
        let raw = sign(q);
        if in_range::<T>(&raw) {
            Spec::Ok(raw)
        } else {
            Spec::Kinds(vec![E::Overflow])
        }
    } else if in_range::<T>(&sign(q)) {
        Spec::Kinds(vec![E::PrecisionLoss])
    } else {
        Spec::Kinds(vec![E::PrecisionLoss, E::Overflow]) // both are true
    }
}

fn check_parse<T: Dec>(f: &mut Failures, s: &str) {
    for lossy in [false, true] {
        let spec = spec_parse::<T>(s, lossy);
        let got = guarded(|| {
            if lossy {
                T::from_str_lossy(s)
            } else {
                T::from_str_exact(s)
            }
        });
        let kind = if lossy {
            "from_str_lossy"
        } else {
            "from_str_exact"
        };
        let ok = match (&got, &spec) {
            (Err(()), _) => false, // a panic is never acceptable
            (Ok(Ok(v)), Spec::Ok(want)) => v.raw_big() == *want,
            (Ok(Err(e)), Spec::Kinds(k)) => k.contains(e),
            _ => false,
        };
        f.check(
            || format!("{} {kind}", T::NAME),
            || format!("{s:?} spec {spec:?}"),
            Ok(ok),
            true,
        );
    }
    // `FromStr` is `from_str_exact`
    let a = guarded(|| T::from_str_trait(s));
    let b = guarded(|| T::from_str_exact(s));
    f.check(
        || format!("{} FromStr == exact", T::NAME),
        || format!("{s:?}"),
        a,
        b.unwrap_or(Err(E::InvalidFormat)),
    );
}

// ---------------------------------------------------------------------------
// String generation
// ---------------------------------------------------------------------------

/// The canonical text of a raw value: every fractional digit.
fn canonical<T: Dec>(raw: &BigInt) -> String {
    let a = raw.abs().to_string();
    let d = T::DECIMALS as usize;
    let a = format!("{:0>w$}", a, w = d + 1);
    let s = format!("{}.{}", &a[..a.len() - d], &a[a.len() - d..]);
    if raw.is_negative() {
        format!("-{s}")
    } else {
        s
    }
}

fn mangle<T: Dec>(raw: &BigInt) -> Vec<String> {
    let c = canonical::<T>(raw);
    let (sign, body) = match c.strip_prefix('-') {
        Some(b) => ("-", b),
        None => ("", c.as_str()),
    };
    let (int, frac) = body.split_once('.').unwrap();
    let digits = raw.abs().to_string();
    let d = T::DECIMALS as i64;
    let exp_base = -d;
    let mut v = vec![
        c.clone(),
        format!("+{body}"),
        format!("{sign}000{body}"),
        format!("{sign}{body}0"),
        format!("{sign}{body}0000000000000"), // many trailing zeros: insignificant
        format!("{sign}{int}"),               // integer part only
        format!("{sign}{int}."),              // trailing point
        format!("{sign}.{frac}"),             // no integer part
        format!(" {c}"),
        format!("{c} "),
        format!("\t{c}\n"),
        format!("{c}1"), // one more digit
        format!("{c}9"),
        format!("{sign}1{body}"),
        format!("{sign}9{body}"),
        format!("--{body}"),
        format!("+-{body}"),
        format!("-+{body}"),
        format!("{c}x"),
        format!("x{c}"),
        format!("{sign}{int}..{frac}"),
        format!("{sign}{int}.{frac}.0"),
        format!("{sign}{int} .{frac}"),
        format!("{sign}{int}_{frac}"),
        format!("{sign}{int},{frac}"),
        format!("{sign}0x{digits}"),
        // scientific forms of the same number
        format!("{sign}{digits}e{exp_base}"),
        format!("{sign}{digits}E{exp_base}"),
        format!("{sign}{digits}e-{:03}", d),
        format!("{sign}{digits}00e{}", exp_base - 2),
        format!(
            "{sign}{}.{}e{}",
            &digits[..1],
            &digits[1..],
            digits.len() as i64 - 1 + exp_base
        ),
        format!("{sign}.{digits}e{}", digits.len() as i64 + exp_base),
        format!("{sign}{digits}.e{exp_base}"),
        format!("{sign}{digits}e+{}", -exp_base - 1),
        format!("{sign}{body}e0"),
        format!("{sign}{body}e+0"),
        format!("{sign}{body}e-0"),
        format!("{sign}{body}e1"),
        format!("{sign}{body}e-1"),
        format!("{sign}{body}e"),
        format!("{sign}{body}e+"),
        format!("e5{body}"),
        format!("{sign}{body}e1e1"),
        format!("{sign}{body}e1.5"),
        format!("{sign}{body}e 1"),
    ];
    if !frac.is_empty() {
        v.push(format!("{sign}{int}.{}", &frac[..frac.len() - 1])); // one fewer digit
    }
    v
}

const FIXED: &[&str] = &[
    "",
    " ",
    "+",
    "-",
    ".",
    "-.",
    "+.",
    "e",
    "E",
    "0",
    "-0",
    "+0",
    "00",
    "000",
    "0.",
    ".0",
    "0.0",
    "-0.0",
    "+0.0",
    "0.00000000000000000000",
    "-0.000000000000000000000000000000000000000000",
    "1",
    "-1",
    "+1",
    "01",
    "1.",
    ".1",
    "1.0",
    "1.5",
    "-1.5",
    ".5",
    "-.5",
    "+.5",
    "5.",
    "1..5",
    "1.2.3",
    "--1",
    "++1",
    "+-1",
    "-+1",
    "1-",
    "1+",
    "1e5",
    "1E5",
    "1e+5",
    "1e-5",
    "1.5e3",
    "-1.5e3",
    ".5e1",
    "5.e1",
    "1e",
    "1e+",
    "1e-",
    "e5",
    ".e5",
    "0e5",
    "0e-5",
    "0e99999999999",
    "-0e5",
    "0.0e99999999999",
    "1e99999999999",
    "1e-99999999999",
    "0x10",
    "1_000",
    "1,5",
    "١٢٣",
    "１２３",
    "1.٥",
    "1 .5",
    "1. 5",
    "\t1",
    "1\n",
    "\u{a0}1\u{a0}",
    "NaN",
    "inf",
    "-inf",
    "Infinity",
    "0.000000001",
    "0.00000001",
    "0.000000015",
    "0.000000005",
    "0.0000000049999999999999999999",
    "0.000000000001",
    "0.0000000000005",
    "0.0000000000015",
    "0.0000000000025",
    "-0.0000000000005",
    "-0.0000000000015",
    "92233720368.54775807",
    "92233720368.54775808",
    "-92233720368.54775808",
    "-92233720368.54775809",
    "92233720368.547758075",
    "92233720368.5477580751",
    "-92233720368.547758085",
    "39614081257132168.796771975167",
    "39614081257132168.796771975168",
    "-39614081257132168.796771975168",
    "-39614081257132168.796771975169",
    "39614081257132168.7967719751675",
    "39614081257132168.7967719751685",
    "9223372036854775807",
    "9223372036854775808",
    "18446744073709551616",
    "340282366920938463463374607431768211456",
    "1000000000000000000000000000000000000000000000000",
    "123456789012345678901234567890123456789012345678901234567890",
    "0.1234567890123456789012345678901234567890123456789",
    "1e38",
    "1e39",
    "1e-38",
    "1e-39",
    "1e-40",
    "92233720368.54775807e0",
    "9.223372036854775807e10",
    "922337203685477580.7e-7",
    "92233720368547758070e-9",
    "39614081257132168796771975167e-12",
    "39614081257132168796771975168e-12",
    "-39614081257132168796771975168e-12",
];

fn run_parse<T: Dec>() {
    let mut f = Failures::default();
    let k = scale();
    let mut rng = Rng(0xFA85_7E27 ^ T::BITS as u64);
    for s in FIXED {
        check_parse::<T>(&mut f, s);
    }
    let all = interesting::<T>();
    let stride = (all.len() / (70 * k)).max(1);
    for v in all.iter().step_by(stride) {
        for s in mangle::<T>(&v.raw_big()) {
            check_parse::<T>(&mut f, &s);
        }
    }
    // the exact limits and one step beyond them, at every spelling
    for edge in [
        T::hi(),
        T::lo(),
        T::hi() + 1,
        T::lo() - 1,
        T::hi() - 1,
        T::lo() + 1,
        BigInt::from(1),
        BigInt::from(-1),
    ] {
        for s in mangle::<T>(&edge) {
            check_parse::<T>(&mut f, &s);
        }
    }
    for _ in 0..(300 * k) {
        for s in mangle::<T>(&rng.raw::<T>()) {
            check_parse::<T>(&mut f, &s);
        }
    }
    // Numerals of any length. A valid string is never `InvalidFormat` for being long: the same
    // value written with 60 leading zeros, or 60 trailing zeros (in the fraction, or as zeros of
    // a significand that an exponent shrinks back), is the same value.
    let z = "0".repeat(60);
    for s in [
        format!("0.{z}"),
        format!("-0.{z}"),
        format!("{z}0"),
        format!("{z}1"),
        format!("{z}1.5"),
        format!("1{z}"),
        format!("-1{z}"),
        format!("1.{z}"),
        format!("1.{z}1"),
        format!("0.{}", "1234567890".repeat(5)),
        format!("-0.{}", "9876543210".repeat(7)),
        format!("{}.{}", "9".repeat(80), "9".repeat(80)),
        format!("1{}e-40", "0".repeat(41)),
        format!("{z}1e-40"),
        format!("0.{z}1e60"),
        format!("{}e-83", "7".repeat(80)),
        format!("{}e-72", "7".repeat(80)),
        format!("{}e-{}", "7".repeat(45), 45 - 1),
    ] {
        check_parse::<T>(&mut f, &s);
    }
    for _ in 0..(300 * k) {
        let raw = rng.raw::<T>();
        let (sign, digits) = (
            if raw.is_negative() { "-" } else { "" },
            raw.abs().to_string(),
        );
        let (lead, trail) = (
            "0".repeat(rng.below(70) as usize),
            "0".repeat(rng.below(70) as usize),
        );
        let t = trail.len() as i64;
        let c = canonical::<T>(&raw);
        let (int, frac) = c.trim_start_matches('-').split_once('.').unwrap();
        for s in [
            format!("{sign}{lead}{digits}{trail}e{}", -(T::DECIMALS as i64) - t), // significand shrunk by the exponent
            format!("{sign}{lead}{int}.{frac}{trail}"), // padded both sides, plain
            format!("{sign}{lead}{int}.{frac}{trail}e0"),
        ] {
            check_parse::<T>(&mut f, &s);
            f.check(
                || format!("{} long numeral is exact", T::NAME),
                || s.clone(),
                guarded(|| T::from_str_exact(&s).map(|d| d.raw_big())),
                Ok(raw.clone()),
            );
        }
        // a random digit string of any length, any dot, any exponent: whatever the value, no panic and a spec'd answer
        let n = 20 + rng.below(100) as usize;
        let body: String = (0..n)
            .map(|_| char::from(b'0' + rng.below(10) as u8))
            .collect();
        let dot = rng.below(n as u64 + 1) as usize;
        let e = rng.below(300) as i64 - 150;
        check_parse::<T>(
            &mut f,
            &format!("{sign}{}.{}e{e}", &body[..dot], &body[dot..]),
        );
        check_parse::<T>(&mut f, &format!("{sign}{}.{}", &body[..dot], &body[dot..]));
    }
    f.finish(&format!("parse {}", T::NAME));
}

// ---------------------------------------------------------------------------
// Display specification
// ---------------------------------------------------------------------------

/// `{}` -> shortest form; `{:.p}` -> exactly `p` fraction digits, the magnitude rounded
/// half-to-even; the sign is that of the raw value (so `-0.4` at `{:.0}` is "-0", as for floats).
fn spec_display<T: Dec>(raw: &BigInt, prec: Option<usize>) -> String {
    let d = T::DECIMALS as usize;
    let a = raw.abs();
    let p = prec.unwrap_or(d);
    let x = if p >= d {
        &a * pow10((p - d) as u32)
    } else {
        round_div(
            &a,
            &pow10((d - p) as u32),
            RoundingStrategy::MidpointNearestEven,
        )
    };
    let mut s = x.to_string();
    if p > 0 {
        while s.len() < p + 1 {
            s.insert(0, '0');
        }
        s.insert(s.len() - p, '.');
    }
    if prec.is_none() && s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if raw.is_negative() {
        format!("-{s}")
    } else {
        s
    }
}

fn run_display<T: Dec>() {
    let mut f = Failures::default();
    let mut rng = Rng(0xD15_91A7 ^ T::BITS as u64);
    let mut vals = interesting::<T>();
    for _ in 0..(400 * scale()) {
        vals.push(rng.val());
    }
    let d = T::DECIMALS as usize;
    let precs: Vec<Option<usize>> = std::iter::once(None)
        .chain([0, 1, 2, 3, d - 1, d, d + 1, d + 2, 20, 40, 60].map(Some))
        .collect();
    for &v in &vals {
        let raw = v.raw_big();
        for &p in &precs {
            let want = spec_display::<T>(&raw, p);
            let got = guarded(|| match p {
                None => format!("{v}"),
                Some(p) => format!("{v:.p$}"),
            });
            f.check(
                || format!("{} display prec={p:?}", T::NAME),
                || format!("{raw}"),
                got,
                want,
            );
        }
        // Debug is `Name(<display>)`; `{:#?}` shows the raw value
        f.check(
            || format!("{} debug", T::NAME),
            || format!("{raw}"),
            guarded(|| format!("{v:?}")),
            format!("{}({})", T::NAME, spec_display::<T>(&raw, None)),
        );
        // round trips
        let text = format!("{v}");
        f.check(
            || format!("{} roundtrip display->exact", T::NAME),
            || format!("{raw} {text:?}"),
            guarded(|| T::from_str_exact(&text).map(|x| x.raw_big())),
            Ok(raw.clone()),
        );
        let full = format!("{v:.d$}");
        f.check(
            || format!("{} roundtrip full->lossy", T::NAME),
            || format!("{raw} {full:?}"),
            guarded(|| T::from_str_lossy(&full).map(|x| x.raw_big())),
            Ok(raw.clone()),
        );
        // `to_string` agrees with `{}`
        f.check(
            || format!("{} to_string", T::NAME),
            || format!("{raw}"),
            guarded(|| v.to_string()),
            text,
        );
    }
    for &v in vals.iter().step_by(5) {
        check_display_flags(&mut f, v);
    }
    f.finish(&format!("display {}", T::NAME));
}

/// What `format!("{:<fill><align><+><0><w>.<p>")` of a number must be, from the sign and digits:
/// the sign (`-`, or `+` with the flag), then padding -- zeros between sign and digits for the
/// `0` flag (fill and alignment ignored), otherwise `fill` on the left / right / both sides
/// (right by default; the odd one of a centred pad goes right).
fn pad_model(
    negative: bool,
    digits: &str,
    width: usize,
    fill: char,
    align: char,
    plus: bool,
    zero: bool,
) -> String {
    let sign = if negative {
        "-"
    } else if plus {
        "+"
    } else {
        ""
    };
    let pad = width.saturating_sub(sign.chars().count() + digits.chars().count());
    let rep = |n: usize, c: char| std::iter::repeat(c).take(n).collect::<String>();
    if zero {
        return format!("{sign}{}{digits}", rep(pad, '0'));
    }
    let (l, r) = match align {
        '<' => (0, pad),
        '^' => (pad / 2, pad - pad / 2),
        _ => (pad, 0),
    };
    format!("{}{sign}{digits}{}", rep(l, fill), rep(r, fill))
}

/// `{:w$}` / `{:.p$}` flags: width, fill, alignment, `+`, `0`; with and without a precision.
fn check_display_flags<T: Dec>(f: &mut Failures, v: T) {
    let raw = v.raw_big();
    let d = T::DECIMALS as usize;
    macro_rules! cases {
        ($p:expr, $($spec:literal => ($fill:expr, $align:expr, $plus:expr, $zero:expr)),* $(,)?) => {{
            let p: Option<usize> = $p;
            let body = spec_display::<T>(&raw, p);
            let (neg, digits) = match body.strip_prefix('-') { Some(b) => (true, b), None => (false, body.as_str()) };
            for w in [0usize, 1, 5, 12, 30, 80] {
                $(
                    let got = guarded(|| match p {
                        Some(pr) => format!(concat!("{:", $spec, "w$.pr$}"), v, w = w, pr = pr),
                        None => format!(concat!("{:", $spec, "w$}"), v, w = w),
                    });
                    f.check(|| format!("{} display flags {:?} p={p:?}", T::NAME, $spec), || format!("{raw} w={w}"), got,
                        pad_model(neg, digits, w, $fill, $align, $plus, $zero));
                )*
            }
        }};
    }
    for p in [None, Some(0), Some(2), Some(d), Some(d + 3)] {
        cases!(p,
            "" => (' ', '>', false, false), "<" => (' ', '<', false, false), "^" => (' ', '^', false, false),
            ">" => (' ', '>', false, false), "*<" => ('*', '<', false, false), "*^" => ('*', '^', false, false),
            "é>" => ('é', '>', false, false), "+" => (' ', '>', true, false), "<+" => (' ', '<', true, false),
            "0" => (' ', '>', false, true), "+0" => (' ', '>', true, true), "*>+" => ('*', '>', true, false),
        );
    }
}

// ---------------------------------------------------------------------------
// Constructors and integer conversions
// ---------------------------------------------------------------------------

fn i64s() -> Vec<i64> {
    let mut v = vec![
        0,
        1,
        -1,
        2,
        -2,
        5,
        9,
        10,
        -10,
        99,
        100,
        12345,
        1_000_000,
        100_000_000,
        10_000_000_000,
        i64::MAX,
        i64::MIN,
        i64::MAX - 1,
        i64::MIN + 1,
        i64::MAX / 2,
        i64::MIN / 2,
        i64::MAX / 3,
        1 << 31,
        1 << 32,
        (1 << 32) + 1,
        1 << 40,
        1 << 62,
        (1 << 62) + 1,
        92_233_720_367,
        92_233_720_368,
        92_233_720_369,
        -92_233_720_368,
        -92_233_720_369,
        92_233_720_368_547_758,
        9_223_372_036_854_775,
        39_614_081_257_132_168,
        39_614_081_257_132_169,
        -39_614_081_257_132_168,
        -39_614_081_257_132_169,
        i32::MAX as i64,
        i32::MIN as i64,
        i32::MAX as i64 + 1,
        u32::MAX as i64,
        u32::MAX as i64 + 1,
    ];
    v.sort();
    v.dedup();
    v
}

fn check_constructors<T: Dec>(f: &mut Failures) {
    let name = T::NAME;
    let s = T::scale_big();
    for n in i64s() {
        let nn = BigInt::from(n);
        let want = fit::<T>(&(&nn * &s));
        f.check(
            || format!("{name} from_i64"),
            || format!("{n}"),
            guarded(|| T::from_i64_n(n)),
            want,
        );
        f.check(
            || format!("{name} try_from_i64"),
            || format!("{n}"),
            guarded(|| T::try_from_i64_n(n)),
            want.ok_or(E::Overflow),
        );
        if n >= 0 {
            let u = n as u64;
            f.check(
                || format!("{name} from_u64"),
                || format!("{u}"),
                guarded(|| T::from_u64_n(u)),
                want,
            );
            f.check(
                || format!("{name} try_from_u64"),
                || format!("{u}"),
                guarded(|| T::try_from_u64_n(u)),
                want.ok_or(E::Overflow),
            );
        }
        // the unsigned side of u64, above i64::MAX
        let hi = n as u64;
        let nn = BigInt::from(hi);
        let want = fit::<T>(&(&nn * &s));
        f.check(
            || format!("{name} from_u64 (wide)"),
            || format!("{hi}"),
            guarded(|| T::from_u64_n(hi)),
            want,
        );
        f.check(
            || format!("{name} try_from_u64 (wide)"),
            || format!("{hi}"),
            guarded(|| T::try_from_u64_n(hi)),
            want.ok_or(E::Overflow),
        );
        // basis points: 1 bp = 0.0001
        let bp = fit::<T>(&(&BigInt::from(n) * &s / 10_000));
        f.check(
            || format!("{name} from_basis_points"),
            || format!("{n}"),
            guarded(|| T::from_bps(n)),
            bp,
        );
    }
    for n in [i32::MIN, -1, 0, 1, 7, i32::MAX] {
        f.check(
            || format!("{name} from_i32"),
            || format!("{n}"),
            guarded(|| T::from_i32_n(n).raw_big()),
            BigInt::from(n) * &s,
        );
    }
    for n in [0u32, 1, 7, u32::MAX] {
        f.check(
            || format!("{name} from_u32"),
            || format!("{n}"),
            guarded(|| T::from_u32_n(n).raw_big()),
            BigInt::from(n) * &s,
        );
    }
    // to_int / to_int_round / to_basis_points: from every boundary value
    for v in interesting::<T>() {
        let raw = v.raw_big();
        f.check(
            || format!("{name} to_int"),
            || format!("{raw}"),
            guarded(|| v.to_int_big()),
            round_div(&raw, &s, RoundingStrategy::ToZero),
        );
        f.check(
            || format!("{name} to_int_round"),
            || format!("{raw}"),
            guarded(|| v.to_int_round_big()),
            round_div(&raw, &s, RoundingStrategy::MidpointNearestEven),
        );
        // 1 bp = S/10^4 raw: truncated toward zero
        f.check(
            || format!("{name} to_basis_points"),
            || format!("{raw}"),
            guarded(|| v.to_bps_big()),
            round_div(&(&raw * 10_000), &s, RoundingStrategy::ToZero),
        );
    }
    // new(integer, fractional)
    for integer in i64s() {
        for frac in [
            0i64,
            1,
            5,
            99,
            49_999_999,
            50_000_000,
            99_999_999,
            100_000_000,
            -1,
            i64::MAX,
        ] {
            let valid = frac >= 0 && BigInt::from(frac) < s;
            let raw = BigInt::from(integer) * &s
                + if integer >= 0 {
                    BigInt::from(frac)
                } else {
                    -BigInt::from(frac)
                };
            let want = if valid { fit::<T>(&raw) } else { None };
            f.check(
                || format!("{name} new"),
                || format!("({integer}, {frac})"),
                guarded(|| T::new_parts(integer, frac))
                    .map(Some)
                    .or(Ok(None)),
                want,
            );
        }
    }
    // with_scale and friends
    for m in i64s() {
        for sc in (0..=T::DECIMALS + 3).chain([20, 38, 39, 40, 45]) {
            let mm = BigInt::from(m);
            let (exact, lossy) = if sc <= T::DECIMALS {
                let r = &mm * pow10(T::DECIMALS - sc);
                (fit::<T>(&r), fit::<T>(&r))
            } else {
                let k = sc - T::DECIMALS;
                let q = if k >= 39 {
                    BigInt::zero()
                } else {
                    round_div(&mm, &pow10(k), RoundingStrategy::MidpointNearestEven)
                };
                (None, fit::<T>(&q))
            };
            let at = || format!("({m}, {sc})");
            f.check(
                || format!("{name} try_with_scale"),
                at,
                guarded(|| T::try_with_scale_n(m, sc)),
                exact,
            );
            f.check(
                || format!("{name} try_with_scale_lossy"),
                at,
                guarded(|| T::try_with_scale_lossy_n(m, sc)),
                lossy,
            );
            // the panicking forms agree: same value, or a panic exactly when `None`
            f.check(
                || format!("{name} with_scale"),
                at,
                Ok(guarded(|| T::with_scale_n(m, sc)).ok()),
                exact,
            );
            f.check(
                || format!("{name} with_scale_lossy"),
                at,
                Ok(guarded(|| T::with_scale_lossy_n(m, sc)).ok()),
                lossy,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Inspection: mantissa / scale / normalize / as_integer_ratio
// ---------------------------------------------------------------------------

fn check_inspection<T: Dec>(f: &mut Failures) {
    let name = T::NAME;
    let s = T::scale_big();
    for v in interesting::<T>() {
        let raw = v.raw_big();
        // minimal form: drop trailing zeros of the FRACTIONAL part only
        let mut tz = 0u32;
        let mut r = raw.clone();
        while tz < T::DECIMALS && !r.is_zero() && (&r % 10u8).is_zero() {
            r /= 10;
            tz += 1;
        }
        let (want_scale, want_mant) = if raw.is_zero() {
            (0, BigInt::zero())
        } else {
            (T::DECIMALS - tz, r.clone())
        };
        f.check(
            || format!("{name} scale"),
            || format!("{raw}"),
            guarded(|| v.scale_u32()),
            want_scale,
        );
        f.check(
            || format!("{name} mantissa"),
            || format!("{raw}"),
            guarded(|| v.mantissa_big()),
            want_mant,
        );
        f.check(
            || format!("{name} is_integer"),
            || format!("{raw}"),
            guarded(|| v.is_integer()),
            (&raw % &s).is_zero(),
        );
        f.check(
            || format!("{name} normalize"),
            || format!("{raw}"),
            guarded(|| v.normalize()),
            v,
        );
        let g = raw.abs().gcd(&s);
        let want_ratio = if raw.is_zero() {
            (BigInt::zero(), BigInt::from(1))
        } else {
            (&raw / &g, &s / &g)
        };
        f.check(
            || format!("{name} as_integer_ratio"),
            || format!("{raw}"),
            guarded(|| v.ratio_big()),
            want_ratio,
        );
    }
}

// ---------------------------------------------------------------------------
// Floats: to_f64 is the correctly rounded quotient where raw is exact; from_f64
// inverts it, and refuses what is not a number or does not fit.
// ---------------------------------------------------------------------------

fn check_floats<T: Dec>(f: &mut Failures) {
    let name = T::NAME;
    let s = T::scale_big();
    let sf = s.to_string().parse::<f64>().unwrap();
    let mut rng = Rng(0xF10A7 ^ T::BITS as u64);
    for _ in 0..3000 {
        // |raw| < 2^50: exactly representable, and the decimal -> f64 -> decimal trip is lossless
        let bits = rng.below(50) as u32;
        let mag = if bits == 0 {
            0
        } else {
            rng.next() >> (64 - bits)
        };
        let raw = BigInt::from(mag) * if rng.next() & 1 == 1 { -1 } else { 1 };
        let v = fit::<T>(&raw).unwrap();
        let want = (mag as f64 * if raw.is_negative() { -1.0 } else { 1.0 }) / sf;
        // documented: "only approximately the nearest f64 ... up to ~1 ULP"
        let x = guarded(|| v.to_f64()).map(|g| {
            g == want
                || (g.to_bits() as i64 - want.to_bits() as i64).abs() <= 1
                || (g == 0.0 && want == 0.0)
        });
        f.check(
            || format!("{name} to_f64 within 1 ulp"),
            || format!("{raw}"),
            x,
            true,
        );
        f.check(
            || format!("{name} from_f64(to_f64)"),
            || format!("{raw}"),
            guarded(|| T::from_f64(want).map(|d| d.raw_big())),
            Some(raw.clone()),
        );
    }
    for bad in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e300,
        -1e300,
        f64::MAX,
        f64::MIN,
    ] {
        f.check(
            || format!("{name} from_f64 rejects"),
            || format!("{bad}"),
            guarded(|| T::from_f64(bad)),
            None,
        );
    }
    f.check(
        || format!("{name} from_f64(0.0)"),
        || String::new(),
        guarded(|| T::from_f64(0.0).map(|d| d.raw_big())),
        Some(BigInt::zero()),
    );
    f.check(
        || format!("{name} from_f64(-0.0)"),
        || String::new(),
        guarded(|| T::from_f64(-0.0).map(|d| d.raw_big())),
        Some(BigInt::zero()),
    );
    // inside / outside the integer range of the type
    let max_int = (T::hi() / &s).to_string().parse::<f64>().unwrap();
    f.check(
        || format!("{name} from_f64(inside)"),
        || String::new(),
        guarded(|| T::from_f64(max_int * 0.99).is_some()),
        true,
    );
    f.check(
        || format!("{name} from_f64(well outside)"),
        || String::new(),
        guarded(|| T::from_f64(max_int * 2.0)),
        None,
    );
    f.check(
        || format!("{name} from_f64(well below)"),
        || String::new(),
        guarded(|| T::from_f64(-max_int * 2.0)),
        None,
    );
}

fn check_f64_exact<T: Dec>(f: &mut Failures) {
    let name = T::NAME;
    let mut rng = Rng(0xF64E ^ T::BITS as u64);
    let s = T::scale_big();
    let to_f = |b: &BigInt| b.to_string().parse::<f64>().unwrap();
    let mut xs: Vec<f64> = vec![
        0.0,
        -0.0,
        f64::MIN_POSITIVE,
        5e-324,
        -5e-324,
        f64::MAX,
        f64::MIN,
        f64::EPSILON,
        0.5,
        1.5,
        2.5,
        1e-9,
        1e-13,
        4.99e-9,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e300,
        9.3e18,
        9.2e10,
        3.96e16,
    ];
    // random doubles of every magnitude
    for _ in 0..(4000 * scale()) {
        let x = f64::from_bits(rng.next());
        xs.push(x);
    }
    // the doubles around each limit of the type, and the integer part of it
    for edge in [T::hi(), T::lo(), T::hi() + 1, T::lo() - 1] {
        for center in [to_f(&edge) / to_f(&s), to_f(&(&edge / &s))] {
            let b = center.to_bits() as i64;
            for k in -40..=40 {
                xs.push(f64::from_bits((b + k) as u64));
            }
        }
    }
    // exact ties: x = m / 2^(DECIMALS+1) with m odd has x * 10^DECIMALS = (odd)/2, and the doubles beside them
    for _ in 0..(1500 * scale()) {
        let m = (rng.next() >> (12 + rng.below(40))) | 1;
        let x = m as f64 / (1u64 << (T::DECIMALS + 1)) as f64;
        let b = x.to_bits() as i64;
        xs.extend([
            x,
            -x,
            f64::from_bits((b + 1) as u64),
            f64::from_bits((b - 1) as u64),
        ]);
    }
    // decimal values as the nearest double, and their neighbours
    for _ in 0..(1500 * scale()) {
        let raw = rng.raw::<T>();
        let x = to_f(&raw) / to_f(&s);
        let b = x.to_bits() as i64;
        xs.extend([
            x,
            f64::from_bits((b + 1) as u64),
            f64::from_bits((b - 1) as u64),
        ]);
    }
    for x in xs {
        let want = spec_f64::<T>(x);
        f.check(
            || format!("{name} try_from_f64 exact"),
            || format!("{x:e} ({:#x})", x.to_bits()),
            guarded(|| T::try_from_f64_n(x).map(|d| d.raw_big())),
            want.clone(),
        );
        f.check(
            || format!("{name} from_f64 exact"),
            || format!("{x:e} ({:#x})", x.to_bits()),
            guarded(|| T::from_f64(x).map(|d| d.raw_big())),
            want.ok(),
        );
    }
}

fn run_conversions<T: Dec>() {
    let mut f = Failures::default();
    check_constructors::<T>(&mut f);
    check_inspection::<T>(&mut f);
    check_floats::<T>(&mut f);
    check_f64_exact::<T>(&mut f);
    f.finish(&format!("conversions {}", T::NAME));
}

macro_rules! per_type {
    ($($t:ty: $parse:ident, $display:ident, $conv:ident);* $(;)?) => { $(
        #[test] fn $parse() { run_parse::<$t>(); }
        #[test] fn $display() { run_display::<$t>(); }
        #[test] fn $conv() { run_conversions::<$t>(); }
    )* };
}
per_type!(fixdec::D64: parse_d64, display_d64, conversions_d64; fixdec::D96: parse_d96, display_d96, conversions_d96);

// ---------------------------------------------------------------------------
// D64 <-> D96
// ---------------------------------------------------------------------------

#[test]
fn d64_d96_conversions_match_oracle() {
    use fixdec::{D64, D96};
    let mut f = Failures::default();
    let mut rng = Rng(0xD64_D96);
    let mut vals: Vec<BigInt> = interesting::<D96>().iter().map(|v| v.raw_big()).collect();
    for _ in 0..4000 {
        vals.push(rng.raw::<D96>());
    }
    for raw in vals {
        let d96 = D96::from_raw_big(&raw).unwrap();
        let e4 = pow10(4);
        // exact narrowing: only when the raw value is a multiple of 10^4, and in range
        let (q, r) = raw.div_rem(&e4);
        let exact = if r.is_zero() {
            D64::from_raw_big(&q)
        } else {
            None
        };
        // documented: `Overflow` above D64::MAX, `Underflow` below D64::MIN; precision is checked first
        let out_of_range = |q: &BigInt| {
            if q.is_negative() {
                DecimalError::Underflow
            } else {
                DecimalError::Overflow
            }
        };
        let want_exact = match (&exact, r.is_zero()) {
            (Some(d), _) => Ok(d.raw_big()),
            (None, false) => Err(DecimalError::PrecisionLoss),
            (None, true) => Err(out_of_range(&q)),
        };
        f.check(
            || "to_d64".into(),
            || format!("{raw}"),
            guarded(|| d96.to_d64().map(|d| d.raw_big())),
            want_exact,
        );
        // rounded narrowing: half-to-even on the dropped digits, then range
        let rounded = round_div(&raw, &e4, RoundingStrategy::MidpointNearestEven);
        let want_round = D64::from_raw_big(&rounded)
            .map(|d| d.raw_big())
            .ok_or_else(|| out_of_range(&rounded));
        f.check(
            || "to_d64_round".into(),
            || format!("{raw}"),
            guarded(|| d96.to_d64_round().map(|d| d.raw_big())),
            want_round,
        );
    }
    for raw in interesting::<D64>()
        .iter()
        .map(|v| v.raw_big())
        .chain([BigInt::from(i64::MIN), BigInt::from(i64::MAX)])
    {
        let d64 = D64::from_raw_big(&raw).unwrap();
        let want = &raw * pow10(4);
        f.check(
            || "to_d96".into(),
            || format!("{raw}"),
            guarded(|| d64.to_d96().raw_big()),
            want.clone(),
        );
        f.check(
            || "from_d64".into(),
            || format!("{raw}"),
            guarded(|| D96::from_d64(d64).raw_big()),
            want.clone(),
        );
        f.check(
            || "d64->d96->d64".into(),
            || format!("{raw}"),
            guarded(|| D96::from_d64(d64).to_d64().map(|d| d.raw_big())),
            Ok(raw.clone()),
        );
    }
    f.finish("D64 <-> D96");
}

// ---------------------------------------------------------------------------
// REGRESSIONS: one named test per defect this review found in the parsers, `Display` and
// `from_f64`. Each states the contract with the value that broke the old code; the oracles
// above cover the same ground in bulk.
// ---------------------------------------------------------------------------

/// `Display` honors the precision but silently ignores width, fill, alignment, the `+`
/// flag and `0`-padding, which every std number type (and `pfpd`) honors -- so a table of
/// prices written with `{:>12.2}` is ragged.
#[test]
fn regression_display_ignores_width_fill_alignment_and_sign() {
    use fixdec::{D64, D96};
    let a = D64::from_str_exact("-12.5").unwrap();
    let b = D96::from_str_exact("12.5").unwrap();
    assert_eq!(format!("[{:>8}]", a), "[   -12.5]");
    assert_eq!(format!("[{:<8}]", a), "[-12.5   ]");
    assert_eq!(format!("[{:^8}]", a), "[ -12.5  ]");
    assert_eq!(format!("[{:*>8.2}]", a), "[**-12.50]");
    assert_eq!(format!("[{:08.2}]", a), "[-0012.50]");
    assert_eq!(format!("[{:+}]", b), "[+12.5]");
    assert_eq!(format!("[{:>8}]", b), "[    12.5]");
    assert_eq!(format!("[{:+08.1}]", b), "[+00012.5]");
}

/// Scientific notation accumulates the significand in an `i128`, so more than 38 significant
/// digits is `Overflow` even when the value is in range: `1` followed by 41 zeros, times `1e-40`,
/// is exactly 10.
#[test]
fn regression_scientific_significand_over_38_digits() {
    use fixdec::{D64, D96};
    let s = format!("1{}e-40", "0".repeat(41));
    assert_eq!(D64::from_str_exact(&s), D64::from_str_exact("10"));
    assert_eq!(D96::from_str_exact(&s), D96::from_str_exact("10"));
    assert_eq!(D64::from_str_lossy(&s), D64::from_str_exact("10"));
    let s = format!("{}1e-40", "0".repeat(60)); // leading zeros are fine
    assert_eq!(D64::from_str_exact(&s), Err(DecimalError::PrecisionLoss));
}

/// A malformed string is `InvalidFormat`, whatever else is wrong with it; a string with a stray
/// character after more than `DECIMALS` fraction digits is reported as `PrecisionLoss`.
#[test]
fn regression_malformed_string_error_kind() {
    use fixdec::{D64, D96};
    for s in [
        "1.123456789x",
        "1.5000000000x",
        "0.000000000001 x",
        "1.1234567890123456789012345 y",
    ] {
        assert_eq!(
            D64::from_str_exact(s),
            Err(DecimalError::InvalidFormat),
            "{s:?}"
        );
        assert_eq!(
            D96::from_str_exact(s),
            Err(DecimalError::InvalidFormat),
            "{s:?}"
        );
    }
}

/// `from_fixed_point_str`: digits that are valid but too big for the parse type are `Overflow`
/// for `D96` when the number fits an `i128` and `InvalidFormat` for `D64` and above `i128` --
/// the same mistake reported three ways.
#[test]
fn regression_fixed_point_str_error_kinds() {
    use fixdec::{D64, D96};
    assert_eq!(
        D64::from_fixed_point_str("99999999999999999999", 2),
        Err(DecimalError::Overflow)
    );
    assert_eq!(
        D96::from_fixed_point_str("99999999999999999999", 2),
        Err(DecimalError::Overflow)
    );
    assert_eq!(
        D96::from_fixed_point_str("170141183460469231731687303715884105728", 12),
        Err(DecimalError::Overflow)
    );
    assert_eq!(
        D64::from_fixed_point_str("12x", 2),
        Err(DecimalError::InvalidFormat)
    );
}

/// `D96` parsing gives up on long numerals: 47 or more fraction digits, or 48 or more integer
/// digits, are `InvalidFormat` -- although the string is valid, `D64` parses the same text, and
/// `from_str_lossy` is documented to round any excess digits.
#[test]
fn regression_d96_rejects_long_numerals() {
    use fixdec::{D64, D96};
    let frac = format!("0.{}", "1234567890".repeat(5));
    assert_eq!(
        D96::from_str_lossy(&frac),
        D96::from_str_exact("0.123456789012")
    );
    assert_eq!(D96::from_str_exact(&frac), Err(DecimalError::PrecisionLoss));
    assert_eq!(D64::from_str_exact(&frac), Err(DecimalError::PrecisionLoss)); // D64 already does
    let big = format!("1{}", "0".repeat(60));
    assert_eq!(D96::from_str_exact(&big), Err(DecimalError::Overflow));
    assert_eq!(D96::from_str_lossy(&big), Err(DecimalError::Overflow));
    assert_eq!(D64::from_str_exact(&big), Err(DecimalError::Overflow));
    // and a long numeral that is zero is zero
    assert_eq!(
        D96::from_str_exact(&format!("0.{}", "0".repeat(60))),
        Ok(D96::ZERO)
    );
}

/// `D96::from_f64` multiplies by `1e12` in floating point first, which rounds `3.9614081257132168e16`
/// (itself exactly representable and in range) up to `2^95 = MAX + 1`, then rejects it; `D64::from_f64`
/// rounds before comparing against the exact boundary and accepts the equivalent value.
#[test]
fn regression_d96_from_f64_top_edge() {
    use fixdec::{D64, D96};
    assert!(D96::from_f64(3.9614081257132168e16).is_some());
    assert!(D96::from_f64(-3.9614081257132168e16).is_some());
    assert!(D64::from_f64(9.223372036854775e10).is_some()); // the equivalent D64 case works
}
