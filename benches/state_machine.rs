#![feature(test)]
#![deny(warnings)]

extern crate failsafe;
extern crate test;

use std::time::Duration;

use failsafe::{backoff, clock, failure_policy, StateMachine};

#[bench]
fn consecutive_failures_policy(b: &mut test::Bencher) {
    let backoff = backoff::constant(Duration::from_secs(5));
    let policy = failure_policy::consecutive_failures(3, backoff);
    let state_machine = StateMachine::new(policy, ());

    b.iter(move || {
        test::black_box(state_machine.is_call_permitted());
        test::black_box(state_machine.on_success());
        test::black_box(state_machine.on_error());
    })
}

#[bench]
fn success_rate_over_time_window_policy(b: &mut test::Bencher) {
    let backoff = backoff::constant(Duration::from_secs(5));
    let policy =
        failure_policy::success_rate_over_time_window(0.5, 0, Duration::from_secs(10), backoff);
    let state_machine = StateMachine::new(policy, ());

    clock::freeze(|time| {
        b.iter(move || {
            time.advance(Duration::from_secs(1));
            test::black_box(state_machine.is_call_permitted());
            test::black_box(state_machine.on_success());
            test::black_box(state_machine.on_error());
        })
    })
}
