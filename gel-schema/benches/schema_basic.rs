use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion};

fn schema_add() -> u64 {
    let schema = gel_schema::Schema::default();

    match n {
        0 => 1,
        1 => 1,
        n => fibonacci(n-1) + fibonacci(n-2),
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("schema add", |b| b.iter(|| fibonacci(black_box(20))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
