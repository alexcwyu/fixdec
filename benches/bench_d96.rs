use std::hint::black_box;
use std::str::FromStr;

use criterion::{Criterion, criterion_group, criterion_main};
use fixdec::{D96, RoundingStrategy};

fn bench_addition(c: &mut Criterion) {
    c.bench_function("d96_addition", |b| {
        let x = D96::from_str("123.456789").unwrap();
        let y = D96::from_str("987.654321").unwrap();
        b.iter(|| black_box(black_box(x) + black_box(y)));
    });
}

fn bench_subtraction(c: &mut Criterion) {
    c.bench_function("d96_subtraction", |b| {
        let x = D96::from_str("987.654321").unwrap();
        let y = D96::from_str("123.456789").unwrap();
        b.iter(|| black_box(black_box(x) - black_box(y)));
    });
}

fn bench_multiplication(c: &mut Criterion) {
    c.bench_function("d96_multiplication", |b| {
        let x = D96::from_str("123.456789").unwrap();
        let y = D96::from_str("9.876543").unwrap();
        b.iter(|| black_box(black_box(x) * black_box(y)));
    });
}

fn bench_division(c: &mut Criterion) {
    c.bench_function("d96_division", |b| {
        let x = D96::from_str("123.456789").unwrap();
        let y = D96::from_str("9.876543").unwrap();
        b.iter(|| black_box(black_box(x) / black_box(y)));
    });
}

fn bench_parsing(c: &mut Criterion) {
    c.bench_function("d96_parsing", |b| {
        b.iter(|| black_box(D96::from_str("123.456789").unwrap()));
    });
}

fn bench_formatting(c: &mut Criterion) {
    c.bench_function("d96_formatting", |b| {
        let d = D96::from_str("123.456789").unwrap();
        b.iter(|| black_box(format!("{}", d)));
    });
}

// Display cost WITHOUT the per-call allocation: writes into a reused buffer so the
// measurement isolates the formatter from String allocation (the `format!` bench
// above includes allocation, which matters for user-facing formatting).
fn bench_formatting_into_buf(c: &mut Criterion) {
    use core::fmt::Write;
    c.bench_function("d96_formatting_into_buf", |b| {
        let d = D96::from_str("123.456789").unwrap();
        let mut buf = String::with_capacity(32);
        b.iter(|| {
            buf.clear();
            write!(buf, "{}", black_box(d)).unwrap();
            black_box(&buf);
        });
    });
}

fn bench_price_times_quantity_mul_i64(c: &mut Criterion) {
    c.bench_function("d96_price_times_quantity_mul_i64", |b| {
        let price = D96::from_str("123.45").unwrap();
        let quantity = 1000i128;
        b.iter(|| black_box(price.mul_i128(black_box(quantity)).unwrap()));
    });
}

fn bench_price_times_quantity_mul_d96(c: &mut Criterion) {
    c.bench_function("d96_price_times_quantity_mul_d96", |b| {
        let price = D96::from_str("123.45").unwrap();
        let quantity = D96::from_i128(1000).unwrap();
        b.iter(|| black_box(black_box(price) * black_box(quantity)));
    });
}

// Asymmetric large x small: the large operand exceeds 2^64 (raw of 50_000_000 is
// 5e19 > 1.8e19) so checked_mul takes the 192-bit wide path. (Extending the fast
// path to this shape was benchmarked and regressed, so it stays on the wide path.)
fn bench_large_price_times_small_qty_d96(c: &mut Criterion) {
    c.bench_function("d96_large_price_times_small_qty", |b| {
        let price = D96::from_str("50000000").unwrap(); // raw 5e19 > 2^64
        let quantity = D96::from_i128(3).unwrap();
        b.iter(|| black_box(black_box(price) * black_box(quantity)));
    });
}

fn bench_sum(c: &mut Criterion) {
    c.bench_function("d96_sum_1000_values", |b| {
        let values: Vec<D96> = (0..1000)
            .map(|i| D96::from_str(&format!("{}.{:02}", i, i % 100)).unwrap())
            .collect();
        b.iter(|| black_box(values.iter().copied().sum::<D96>()));
    });
}

fn bench_rounding(c: &mut Criterion) {
    c.bench_function("d96_round_to_2_decimals", |b| {
        let d = D96::from_str("123.456789").unwrap();
        b.iter(|| black_box(black_box(d).round_dp(2)));
    });
}

// round_dp_with_strategy on the common (banker's) strategy that round_dp uses,
// so this is directly comparable to bench_rounding above.
fn bench_round_dp_with_strategy_common(c: &mut Criterion) {
    c.bench_function("d96_round_dp_with_strategy_common", |b| {
        let d = D96::from_str("123.456789").unwrap();
        b.iter(|| {
            black_box(black_box(d).round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven))
        });
    });
}

// A non-default directed strategy (no midpoint tie logic) to isolate strategy
// dispatch cost from the banker's-rounding path.
fn bench_round_dp_with_strategy_directed(c: &mut Criterion) {
    c.bench_function("d96_round_dp_with_strategy_directed", |b| {
        let d = D96::from_str("123.456789").unwrap();
        b.iter(|| {
            black_box(black_box(d).round_dp_with_strategy(2, RoundingStrategy::AwayFromZero))
        });
    });
}

// In-range divide-then-round: dividend * 10^dp stays within i128, the fast path.
fn bench_checked_div_rounded_small(c: &mut Criterion) {
    c.bench_function("d96_checked_div_rounded_small", |b| {
        let x = D96::from_str("123.456789").unwrap();
        let y = D96::from_str("9.876543").unwrap();
        b.iter(|| {
            black_box(black_box(x).checked_div_rounded(
                black_box(y),
                2,
                RoundingStrategy::MidpointNearestEven,
            ))
        });
    });
}

// Wide divide-then-round: a large dividend scaled to full 12 dp overflows i128,
// forcing the 192-bit wide divide that fixed the earlier range cliff. Worst case.
fn bench_checked_div_rounded_wide(c: &mut Criterion) {
    c.bench_function("d96_checked_div_rounded_wide", |b| {
        let x = D96::from_str("1234567890123456.789012").unwrap();
        let y = D96::from_str("9.876543").unwrap();
        b.iter(|| {
            black_box(black_box(x).checked_div_rounded(
                black_box(y),
                12,
                RoundingStrategy::MidpointNearestEven,
            ))
        });
    });
}

// Snap to a price tick (0.01) — the exchange-grid rounding path.
fn bench_checked_quantize(c: &mut Criterion) {
    c.bench_function("d96_checked_quantize", |b| {
        let d = D96::from_str("123.456789").unwrap();
        let tick = D96::from_str("0.01").unwrap();
        b.iter(|| {
            black_box(
                black_box(d)
                    .checked_quantize(black_box(tick), RoundingStrategy::MidpointNearestEven),
            )
        });
    });
}

// Scalar-integer helpers: decimal +/- integer count and split-N, no second parse.
fn bench_add_i128(c: &mut Criterion) {
    c.bench_function("d96_add_i128", |b| {
        let d = D96::from_str("123.456789").unwrap();
        b.iter(|| black_box(black_box(d).add_i128(black_box(1000))));
    });
}

fn bench_sub_i128(c: &mut Criterion) {
    c.bench_function("d96_sub_i128", |b| {
        let d = D96::from_str("123.456789").unwrap();
        b.iter(|| black_box(black_box(d).sub_i128(black_box(1000))));
    });
}

fn bench_div_i128(c: &mut Criterion) {
    c.bench_function("d96_div_i128", |b| {
        let d = D96::from_str("123.456789").unwrap();
        b.iter(|| black_box(black_box(d).div_i128(black_box(7))));
    });
}

fn bench_binary_write_read(c: &mut Criterion) {
    c.bench_function("d96_binary_write_read", |b| {
        let d = D96::from_str("123.456789").unwrap();
        let mut buf = [0u8; 16];
        b.iter(|| {
            d.write_le_bytes(&mut buf);
            black_box(D96::read_le_bytes(&buf))
        });
    });
}

fn bench_comparison(c: &mut Criterion) {
    c.bench_function("d96_comparison", |b| {
        let x = D96::from_str("123.456789").unwrap();
        let y = D96::from_str("123.456790").unwrap();
        b.iter(|| black_box(black_box(x) < black_box(y)));
    });
}

fn bench_sqrt(c: &mut Criterion) {
    // Small value (< ~3.1e14): radicand fits u128 -> the isqrt_u128 fast path.
    c.bench_function("d96_sqrt", |b| {
        let d = D96::from_str("123.456789").unwrap();
        b.iter(|| black_box(black_box(d).sqrt().unwrap()));
    });
}

fn bench_sqrt_wide(c: &mut Criterion) {
    // Large value (> ~3.4e14): radicand exceeds 2^128 -> the 192-bit
    // binary-search path. This is the worst case for D96::sqrt.
    c.bench_function("d96_sqrt_wide", |b| {
        let d = D96::from_str("1234567890123456.789012").unwrap();
        b.iter(|| black_box(black_box(d).sqrt().unwrap()));
    });
}

fn bench_powi(c: &mut Criterion) {
    c.bench_function("d96_powi", |b| {
        let d = D96::from_str("1.05").unwrap();
        b.iter(|| black_box(black_box(d).powi(10).unwrap()));
    });
}

fn bench_percentage_of(c: &mut Criterion) {
    c.bench_function("d96_percent_of", |b| {
        let amount = D96::from_str("1000").unwrap();
        let percent = D96::from_str("5").unwrap();
        b.iter(|| black_box(black_box(amount).percent_of(black_box(percent)).unwrap()));
    });
}

fn bench_add_percent(c: &mut Criterion) {
    c.bench_function("d96_add_percent", |b| {
        let amount = D96::from_str("1000").unwrap();
        let percent = D96::from_str("5").unwrap();
        b.iter(|| black_box(black_box(amount).add_percent(black_box(percent)).unwrap()));
    });
}

criterion_group!(
    benches,
    bench_addition,
    bench_subtraction,
    bench_multiplication,
    bench_division,
    bench_parsing,
    bench_formatting,
    bench_formatting_into_buf,
    bench_price_times_quantity_mul_i64,
    bench_price_times_quantity_mul_d96,
    bench_large_price_times_small_qty_d96,
    bench_sum,
    bench_rounding,
    bench_round_dp_with_strategy_common,
    bench_round_dp_with_strategy_directed,
    bench_checked_div_rounded_small,
    bench_checked_div_rounded_wide,
    bench_checked_quantize,
    bench_add_i128,
    bench_sub_i128,
    bench_div_i128,
    bench_binary_write_read,
    bench_comparison,
    bench_sqrt,
    bench_sqrt_wide,
    bench_powi,
    bench_percentage_of,
    bench_add_percent,
);

criterion_main!(benches);
