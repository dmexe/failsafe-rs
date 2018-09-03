extern crate failsafe;
extern crate tokio_executor;
extern crate tokio_timer;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use failsafe::failure_policy::consecutive_failures;
use failsafe::{backoff, Instrument, StateMachine};

/// Perform `Closed` -> `Open` -> `HalfOpen` -> `Open` -> `HalfOpen` -> `Closed` transitions.
#[test]
fn state_machine() {
    let observe = Observer::new();
    let backoff = backoff::exponential(5.seconds(), 300.seconds());
    let policy = consecutive_failures(3, backoff);

    let mut state_machine = StateMachine::new(policy, observe.clone());

    mock_clock::freeze(move |time| {
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

    fn on_open(&self, duration: &Duration) {
        println!("state=open for {:?}", duration);
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

mod mock_clock {
    include!("../src/mock_clock.rs");
}
