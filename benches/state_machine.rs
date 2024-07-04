#![deny(warnings)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;

use failsafe::{backoff, clock, failure_policy, StateMachine};

#[allow(clippy::unit_arg)]
fn consecutive_failures_policy(c: &mut Criterion) {
    let backoff = backoff::constant(Duration::from_secs(5));
    let policy = failure_policy::consecutive_failures(3, backoff);
    let state_machine = StateMachine::new(policy, ());

    c.bench_function("consecutive_failures_policy", |b| {
        b.iter(|| {
            black_box(state_machine.is_call_permitted());
            black_box(state_machine.on_success());
            black_box(state_machine.on_error());
        })
    });
}

#[allow(clippy::unit_arg)]
fn success_rate_over_time_window_policy(c: &mut Criterion) {
    let backoff = backoff::constant(Duration::from_secs(5));
    let policy =
        failure_policy::success_rate_over_time_window(0.5, 0, Duration::from_secs(10), backoff);
    let state_machine = StateMachine::new(policy, ());

    clock::freeze(|time| {
        c.bench_function("success_rate_over_time_window_policy", |b| {
            b.iter(|| {
                time.advance(Duration::from_secs(1));
                black_box(state_machine.is_call_permitted());
                black_box(state_machine.on_success());
                black_box(state_machine.on_error());
            })
        })
    });
}

criterion_group!(
    benches,
    consecutive_failures_policy,
    success_rate_over_time_window_policy
);
criterion_main!(benches);
