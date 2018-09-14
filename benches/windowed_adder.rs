#![feature(test)]
#![deny(warnings)]

extern crate failsafe;
extern crate test;

use std::thread;
use std::time::Duration;

use failsafe::WindowedAdder;

#[bench]
fn add_and_sum(b: &mut test::Bencher) {
    let mut adder = WindowedAdder::new(Duration::from_millis(1000), 10);

    for _ in 0..10 {
        adder.add(42);
        thread::sleep(Duration::from_millis(100));
    }

    b.iter(|| {
        adder.add(42);
        test::black_box(adder.sum());
    });
}
