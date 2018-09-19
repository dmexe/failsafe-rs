#![feature(test)]
#![deny(warnings)]

extern crate failsafe;
extern crate futures;
extern crate test;

use std::thread;

use failsafe::{CircuitBreaker, Config, Error};

#[bench]
fn single_threaded(b: &mut test::Bencher) {
    let circuit_breaker = Config::new().build();
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
    let circuit_breaker = Config::new().build();
    let batch_size = 10;

    b.iter(move || {
        let mut threads = Vec::new();

        for n in 0..batch_size {
            let circuit_breaker = circuit_breaker.clone();
            let thr = thread::spawn(move || {
                let res = match circuit_breaker.call(|| dangerous_call(n)) {
                    Ok(_) => true,
                    Err(Error::Inner(_)) => false,
                    Err(err) => unreachable!("{:?}", err),
                };
                test::black_box(res);
            });

            threads.push(thr);
        }

        threads.into_iter().for_each(|it| it.join().unwrap());
    });
}

fn dangerous_call(n: usize) -> Result<usize, usize> {
    if n % 5 == 0 {
        test::black_box(Err(n))
    } else {
        test::black_box(Ok(n))
    }
}
