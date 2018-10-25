use std::fmt::{self, Debug};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use super::clock;
use super::failure_policy::FailurePolicy;
use super::instrument::Instrument;

const ON_CLOSED: u8 = 0b0000_0001;
const ON_HALF_OPEN: u8 = 0b0000_0010;
const ON_REJECTED: u8 = 0b0000_0100;
const ON_OPEN: u8 = 0b0000_1000;

/// States of the state machine.
#[derive(Debug)]
enum State {
    /// A closed breaker is operating normally and allowing.
    Closed,
    /// An open breaker has tripped and will not allow requests through until an interval expired.
    Open(Instant, Duration),
    /// A half open breaker has completed its wait interval and will allow requests. The state keeps
    /// the previous duration in an open state.
    HalfOpen(Duration),
}

struct Shared<POLICY> {
    state: State,
    failure_policy: POLICY,
}

struct Inner<POLICY, INSTRUMENT> {
    shared: Mutex<Shared<POLICY>>,
    instrument: INSTRUMENT,
}

/// A circuit breaker implementation backed by state machine.
///
/// It is implemented via a finite state machine with three states: `Closed`, `Open` and `HalfOpen`.
/// The state machine does not know anything about the backend's state by itself, but uses the
/// information provided by the method via `on_success` and `on_error` events. Before communicating
/// with the backend, the the permission to do so must be obtained via the method `is_call_permitted`.
///
/// The state of the state machine changes from `Closed` to `Open` when the `FailurePolicy`
/// reports that the failure rate is above a (configurable) threshold. Then, all access to the backend
/// is blocked for a time duration provided by `FailurePolicy`.
///
/// After the time duration has elapsed, the state changes from `Open` to `HalfOpen` and allows
/// calls to see if the backend is still unavailable or has become available again. If the circuit
/// breaker receives a failure on the next call, the state will change back to `Open`. Otherwise
/// it changes to `Closed`.
pub struct StateMachine<POLICY, INSTRUMENT> {
    inner: Arc<Inner<POLICY, INSTRUMENT>>,
}

impl State {
    /// Returns a string value for the state identifier.
    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            State::Open(_, _) => "open",
            State::Closed => "closed",
            State::HalfOpen(_) => "half_open",
        }
    }
}

impl<POLICY, INSTRUMENT> Debug for StateMachine<POLICY, INSTRUMENT> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let shared = self.inner.shared.lock();
        f.debug_struct("StateMachine")
            .field("state", &(shared.state.as_str()))
            .finish()
    }
}

impl<POLICY, INSTRUMENT> Clone for StateMachine<POLICY, INSTRUMENT> {
    fn clone(&self) -> Self {
        StateMachine {
            inner: self.inner.clone(),
        }
    }
}

impl<POLICY> Shared<POLICY>
where
    POLICY: FailurePolicy,
{
    #[inline]
    fn transit_to_closed(&mut self) {
        self.state = State::Closed;
        self.failure_policy.revived();
    }

    #[inline]
    fn transit_to_half_open(&mut self, delay: Duration) {
        self.state = State::HalfOpen(delay);
    }

    #[inline]
    fn transit_to_open(&mut self, delay: Duration) {
        let until = clock::now() + delay;
        self.state = State::Open(until, delay);
    }
}

impl<POLICY, INSTRUMENT> StateMachine<POLICY, INSTRUMENT>
where
    POLICY: FailurePolicy,
    INSTRUMENT: Instrument,
{
    /// Creates a new state machine with given failure policy and instrument.
    pub fn new(failure_policy: POLICY, instrument: INSTRUMENT) -> Self {
        instrument.on_closed();

        StateMachine {
            inner: Arc::new(Inner {
                shared: Mutex::new(Shared {
                    state: State::Closed,
                    failure_policy,
                }),
                instrument,
            }),
        }
    }

    /// Requests permission to call.
    ///
    /// It returns `true` if a call is allowed, or `false` if prohibited.
    pub fn is_call_permitted(&self) -> bool {
        let mut instrument: u8 = 0;

        let res = {
            let mut shared = self.inner.shared.lock();

            match shared.state {
                State::Closed => true,
                State::HalfOpen(_) => true,
                State::Open(until, delay) => {
                    if clock::now() > until {
                        shared.transit_to_half_open(delay);
                        instrument |= ON_HALF_OPEN;
                        true
                    } else {
                        instrument |= ON_REJECTED;
                        false
                    }
                }
            }
        };

        if instrument & ON_HALF_OPEN != 0 {
            self.inner.instrument.on_half_open();
        }

        if instrument & ON_REJECTED != 0 {
            self.inner.instrument.on_call_rejected();
        }

        res
    }

    /// Records a successful call.
    ///
    /// This method must be invoked when a call was success.
    pub fn on_success(&self) {
        let mut instrument: u8 = 0;
        {
            let mut shared = self.inner.shared.lock();
            if let State::HalfOpen(_) = shared.state {
                shared.transit_to_closed();
                instrument |= ON_CLOSED;
            }
            shared.failure_policy.record_success()
        }

        if instrument & ON_CLOSED != 0 {
            self.inner.instrument.on_closed();
        }
    }

    /// Records a failed call.
    ///
    /// This method must be invoked when a call failed.
    pub fn on_error(&self) {
        let mut instrument: u8 = 0;
        {
            let mut shared = self.inner.shared.lock();
            match shared.state {
                State::Closed => {
                    if let Some(delay) = shared.failure_policy.mark_dead_on_failure() {
                        shared.transit_to_open(delay);
                        instrument |= ON_OPEN;
                    }
                }
                State::HalfOpen(delay_in_half_open) => {
                    // Pick up the next open state's delay from the policy, if policy returns Some(_)
                    // use it, otherwise reuse the delay from the current state.
                    let delay = shared
                        .failure_policy
                        .mark_dead_on_failure()
                        .unwrap_or(delay_in_half_open);
                    shared.transit_to_open(delay);
                    instrument |= ON_OPEN;
                }
                _ => {}
            }
        }

        if instrument & ON_OPEN != 0 {
            self.inner.instrument.on_open();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::super::backoff;
    use super::super::failure_policy::consecutive_failures;
    use super::*;

    /// Perform `Closed` -> `Open` -> `HalfOpen` -> `Open` -> `HalfOpen` -> `Closed` transitions.
    #[test]
    fn state_machine() {
        clock::freeze(move |time| {
            let observe = Observer::new();
            let backoff = backoff::exponential(5.seconds(), 300.seconds());
            let policy = consecutive_failures(3, backoff);
            let state_machine = StateMachine::new(policy, observe.clone());

            assert_eq!(true, state_machine.is_call_permitted());

            // Perform success requests. the circuit breaker must be closed.
            for _i in 0..10 {
                assert_eq!(true, state_machine.is_call_permitted());
                state_machine.on_success();
                assert_eq!(true, observe.is_closed());
            }

            // Perform failed requests, the circuit breaker still closed.
            for _i in 0..2 {
                assert_eq!(true, state_machine.is_call_permitted());
                state_machine.on_error();
                assert_eq!(true, observe.is_closed());
            }

            // Perform a failed request and transit to the open state for 5s.
            assert_eq!(true, state_machine.is_call_permitted());
            state_machine.on_error();
            assert_eq!(true, observe.is_open());

            // Reject call attempts, the circuit breaker in open state.
            for i in 0..10 {
                assert_eq!(false, state_machine.is_call_permitted());
                assert_eq!(i + 1, observe.rejected_calls());
            }

            // Wait 2s, the circuit breaker still open.
            time.advance(2.seconds());
            assert_eq!(false, state_machine.is_call_permitted());
            assert_eq!(true, observe.is_open());

            clock::now();

            // Wait 4s (6s total), the circuit breaker now in the half open state.
            time.advance(4.seconds());
            assert_eq!(true, state_machine.is_call_permitted());
            assert_eq!(true, observe.is_half_open());

            // Perform a failed request and transit back to the open state for 10s.
            state_machine.on_error();
            assert_eq!(false, state_machine.is_call_permitted());
            assert_eq!(true, observe.is_open());

            // Wait 5s, the circuit breaker still open.
            time.advance(5.seconds());
            assert_eq!(false, state_machine.is_call_permitted());
            assert_eq!(true, observe.is_open());

            // Wait 6s (11s total), the circuit breaker now in the half open state.
            time.advance(6.seconds());
            assert_eq!(true, state_machine.is_call_permitted());
            assert_eq!(true, observe.is_half_open());

            // Perform a success request and transit to the closed state.
            state_machine.on_success();
            assert_eq!(true, state_machine.is_call_permitted());
            assert_eq!(true, observe.is_closed());

            // Perform success requests.
            for _i in 0..10 {
                assert_eq!(true, state_machine.is_call_permitted());
                state_machine.on_success();
            }
        });
    }

    #[derive(Debug)]
    enum State {
        Open,
        HalfOpen,
        Closed,
    }

    #[derive(Clone, Debug)]
    struct Observer {
        state: Arc<Mutex<State>>,
        rejected_calls: Arc<AtomicUsize>,
    }

    impl Observer {
        fn new() -> Self {
            Observer {
                state: Arc::new(Mutex::new(State::Closed)),
                rejected_calls: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn is_closed(&self) -> bool {
            match *self.state.lock().unwrap() {
                State::Closed => true,
                _ => false,
            }
        }

        fn is_open(&self) -> bool {
            match *self.state.lock().unwrap() {
                State::Open => true,
                _ => false,
            }
        }

        fn is_half_open(&self) -> bool {
            match *self.state.lock().unwrap() {
                State::HalfOpen => true,
                _ => false,
            }
        }

        fn rejected_calls(&self) -> usize {
            self.rejected_calls.load(Ordering::SeqCst)
        }
    }

    impl Instrument for Observer {
        fn on_call_rejected(&self) {
            self.rejected_calls.fetch_add(1, Ordering::SeqCst);
        }

        fn on_open(&self) {
            println!("state=open");
            let mut own_state = self.state.lock().unwrap();
            *own_state = State::Open
        }

        fn on_half_open(&self) {
            println!("state=half_open");
            let mut own_state = self.state.lock().unwrap();
            *own_state = State::HalfOpen
        }

        fn on_closed(&self) {
            println!("state=closed");
            let mut own_state = self.state.lock().unwrap();
            *own_state = State::Closed
        }
    }

    trait IntoDuration {
        fn seconds(self) -> Duration;
    }

    impl IntoDuration for u64 {
        fn seconds(self) -> Duration {
            Duration::from_secs(self)
        }
    }
}
