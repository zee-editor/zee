use criterion::{black_box, criterion_group, criterion_main, Criterion};
use crossbeam_channel::{select, Receiver, Sender};

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

pub fn criterion_benchmark(c: &mut Criterion) {
    // let (sender, links) = crossbeam_channel::unbounded();

    c.bench_function("fib 20", |b| b.iter(|| fibonacci(black_box(20))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
