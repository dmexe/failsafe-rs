#![deny(warnings)]

use std::thread;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use failsafe::{CircuitBreaker, Config, Error};

fn single_threaded(c: &mut Criterion) {
    let circuit_breaker = Config::new().build();
    let mut n = 0;

    c.bench_function("single_threaded", |b| {
        b.iter(|| {
            match circuit_breaker.call(|| dangerous_call(n)) {
                Ok(_) => {}
                Err(Error::Inner(_)) => {}
                Err(err) => unreachable!("{:?}", err),
            }
            n += 1;
        })
    });
}

fn multi_threaded_in_batch(c: &mut Criterion) {
    let circuit_breaker = Config::new().build();
    let batch_size = 10;

    c.bench_function("multi_threaded_in_batch", |b| {
        b.iter(|| {
            let mut threads = Vec::new();

            for n in 0..batch_size {
                let circuit_breaker = circuit_breaker.clone();
                let thr = thread::spawn(move || {
                    let res = match circuit_breaker.call(|| dangerous_call(n)) {
                        Ok(_) => true,
                        Err(Error::Inner(_)) => false,
                        Err(err) => unreachable!("{:?}", err),
                    };
                    black_box(res);
                });

                threads.push(thr);
            }

            threads.into_iter().for_each(|it| it.join().unwrap());
        })
    });
}

fn dangerous_call(n: usize) -> Result<usize, usize> {
    if n % 5 == 0 {
        black_box(Err(n))
    } else {
        black_box(Ok(n))
    }
}

criterion_group!(benches, single_threaded, multi_threaded_in_batch);
criterion_main!(benches);
