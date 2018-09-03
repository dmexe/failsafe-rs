#![feature(test)]
#![deny(warnings)]

extern crate rayon;
extern crate resilience;
extern crate test;

use std::sync::mpsc::channel;

use rayon::ThreadPoolBuilder;
use resilience::{Callable, CircuitBreaker, Error};

#[bench]
fn single_threaded(b: &mut test::Bencher) {
    let circuit_breaker = CircuitBreaker::default();
    let mut n = 0;

    b.iter(move || {
        circuit_breaker_call(&circuit_breaker, 1);
        n += 1;
    })
}

#[bench]
fn multi_threaded_in_batch(b: &mut test::Bencher) {
    let circuit_breaker = CircuitBreaker::default();
    let pool = ThreadPoolBuilder::new().build().unwrap();
    let count = pool.current_num_threads();

    b.iter(move || {
        let mut join = Vec::with_capacity(count);
        let mut n = 0;

        for _ in 0..count {
            let circuit_breaker = circuit_breaker.clone();
            let (tx, rx) = channel();
            join.push(rx);

            pool.spawn(move || {
                circuit_breaker_call(&circuit_breaker, n);
                tx.send(()).unwrap();
            });
            n += 1;
        }

        for it in join {
            it.recv().unwrap();
        }
    });
}

fn circuit_breaker_call<C: Callable>(call: &C, n: u64) {
    match call.call(|| danger_call(n)) {
        Err(Error::Rejected) => panic!("rejected call"),
        Err(err) => {
            test::black_box(err);
        }
        Ok(ok) => {
            test::black_box(ok);
        }
    }
}

fn danger_call(n: u64) -> Result<(), ()> {
    if n % 10 == 0 {
        Err(())
    } else {
        Ok(())
    }
}
