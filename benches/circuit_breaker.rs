#![feature(test)]
#![deny(warnings)]

extern crate failsafe;
extern crate futures;
extern crate test;
extern crate tokio_threadpool;

use std::cell::RefCell;
use std::sync::mpsc::channel;

use failsafe::{Callable, CircuitBreaker, Error};
use futures::future;
use tokio_threadpool::ThreadPool;

#[bench]
fn single_threaded(b: &mut test::Bencher) {
    let circuit_breaker = CircuitBreaker::builder().build();
    let mut n = 0;

    b.iter(move || {
        match circuit_breaker.call(|| dangerous_call(n)) {
            Ok(_) => {}
            Err(Error::Inner(_)) => {}
            Err(err) => unreachable!("{:?}", err),
        }
        n += 1;
    })
}

#[bench]
fn multi_threaded_in_batch(b: &mut test::Bencher) {
    let circuit_breaker = CircuitBreaker::builder().build();
    let thread_pool = RefCell::new(ThreadPool::new());
    let batch_size = 10;

    b.iter(move || {
        let (tx, rx) = channel();

        for n in 0..batch_size {
            let circuit_breaker = circuit_breaker.clone();
            let tx = tx.clone();

            let future = future::lazy(move || {
                let res = match circuit_breaker.call(|| dangerous_call(n)) {
                    Ok(n) => n,
                    Err(Error::Inner(n)) => n,
                    Err(err) => unreachable!("{:?}", err),
                };
                tx.send(res).unwrap();
                Ok(())
            });

            let thread_pool = thread_pool.borrow();
            thread_pool.spawn(future);
        }

        drop(tx);

        let res = rx.iter().sum();
        assert_eq!(45usize, res);
    });
}

fn dangerous_call(n: usize) -> Result<usize, usize> {
    if n % 5 == 0 {
        test::black_box(Err(n))
    } else {
        test::black_box(Ok(n))
    }
}
