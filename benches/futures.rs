#![feature(test)]
#![deny(warnings)]

extern crate failsafe;
extern crate futures;
extern crate test;
extern crate tokio;

use failsafe::{
    futures::{Callable, CircuitBreaker},
    Error,
};
use futures::{future, Future};
use std::sync::mpsc::channel;

#[bench]
fn multi_threaded_in_batch(b: &mut test::Bencher) {
    let circuit_breaker = CircuitBreaker::builder().build();
    let mut runtime = tokio::runtime::Runtime::new().unwrap();
    let batch_size = 10;

    b.iter(move || {
        let mut join = Vec::with_capacity(batch_size);
        let mut n = 0;

        for _ in 0..batch_size {
            let circuit_breaker = circuit_breaker.clone();

            let (tx, rx) = channel();
            join.push(rx);

            runtime.spawn(
                circuit_breaker_call(&circuit_breaker, n)
                    .then(move |_| tx.send(()).map_err(|_| ())),
            );
            n += 1;
        }

        for it in join {
            it.recv().unwrap();
        }
    });
}

fn circuit_breaker_call<C: Callable>(call: &C, n: u64) -> impl Future<Item = (), Error = ()> {
    let future = call.call(dangerous_call(n));
    Future::then(future, |res| match res {
        Err(Error::Rejected) => {
            panic!("rejected");
        }
        Err(err) => {
            test::black_box(err);
            Ok(())
        }
        Ok(ok) => {
            test::black_box(ok);
            Ok(())
        }
    })
}

fn dangerous_call(n: u64) -> impl Future<Item = (), Error = ()> {
    if n % 10 == 0 {
        future::err(())
    } else {
        future::ok(())
    }
}
