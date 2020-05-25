#![deny(warnings)]

use std::cell::RefCell;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use futures::{future, stream, Future, Stream};

use failsafe::{futures::CircuitBreaker, Config, Error};

fn multi_threaded_in_batch(c: &mut Criterion) {
    let circuit_breaker = Config::new().build();
    let runtime = RefCell::new(tokio::runtime::Runtime::new().unwrap());
    let batch_size = 10;

    c.bench_function("multi_threaded_in_batch", |b| {
        b.iter(|| {
            let circuit_breaker = circuit_breaker.clone();

            let batch = (0..batch_size).map(move |n| {
                circuit_breaker
                    .call(dangerous_call(n))
                    .then(|res| match res {
                        Ok(n) => Ok(n),
                        Err(Error::Inner(n)) => Ok(n),
                        Err(Error::Rejected) => Err(0),
                    })
            });

            let batch = stream::iter_ok(batch)
                .buffer_unordered(batch_size)
                .collect();

            let mut runtime = runtime.borrow_mut();
            let res = runtime.block_on(batch).unwrap();
            assert_eq!(45usize, res.iter().sum::<usize>());
        })
    });
}

fn dangerous_call(n: usize) -> impl Future<Item = usize, Error = usize> {
    future::lazy(move || {
        if n % 5 == 0 {
            black_box(future::err(n))
        } else {
            black_box(future::ok(n))
        }
    })
}

criterion_group!(benches, multi_threaded_in_batch);
criterion_main!(benches);
