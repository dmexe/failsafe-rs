#![deny(warnings)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::thread;
use std::time::Duration;

use failsafe::WindowedAdder;

fn add_and_sum(c: &mut Criterion) {
    let mut adder = WindowedAdder::new(Duration::from_millis(1000), 10);

    for _ in 0..10 {
        adder.add(42);
        thread::sleep(Duration::from_millis(100));
    }

    c.bench_function("add", |b| {
        b.iter(|| {
            adder.add(42);
            black_box(adder.sum());
        })
    });
}

criterion_group!(benches, add_and_sum);
criterion_main!(benches);
