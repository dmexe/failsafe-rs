use std::fmt::{self, Display};
use std::time::{Duration, Instant};

use tokio_timer::clock;

use super::failure_accrual_policy::FailureAccrualPolicy;

/// States of the state machine.
#[derive(Clone, Debug)]
pub enum State {
    /// A closed breaker is operating normally and allowing.
    Closed,
    /// An open breaker has tripped and will not allow requests through until an interval expired.
    Open(Instant, Duration),
    /// A half open breaker has completed its wait interval and will allow requests. The state keeps
    /// the previous duration in an open state.
    HalfOpen(Duration),
}

/// A circuit breaker' state machine manages the state of a backend system.
///
/// It is implemented via a finite state machine with three states: `Closed`, `Open` and `HalfOpen`.
/// The state machine does not know anything about the backend's state by itself, but uses the
/// information provided by the method via `on_success` and `on_error` events. Before communicating
/// with the backend, the the permission to do so must be obtained via the method `is_call_permitted`.
///
/// The state of the state machine changes from `Closed` to `Open` when the `FailureAccrualPolicy`
/// reports that the failure rate is above a (configurable) threshold. Then, all access to the backend
/// is blocked for a time duration provided by `FailureAccrualPolicy`.
///
/// After the time duration has elapsed, the state changes from `Open` to `HalfOpen` and allows
/// calls to see if the backend is still unavailable or has become available again. If the circuit
/// breaker receives a failure on the next call, the state will change back to `Open`. Otherwise
/// it changes to `Closed`.
#[derive(Debug)]
pub struct StateMachine<POLICY, INSTRUMENT> {
    failure_accrual_policy: POLICY,
    instrument: INSTRUMENT,
    state: State,
}

/// Consumes the state machine events. May used for metrics and/or logs.
pub trait Instrument {
    /// Calls when state machine reject a call.
    fn on_call_rejected(&self);

    /// Calls when the circuit breaker become to open state.
    fn on_open(&self, duration: &Duration);

    /// Calls when the circuit breaker become to half open state.
    fn on_half_open(&self);

    /// Calls when the circuit breaker become to closed state.
    fn on_closed(&self);
}

/// An instrumentation which does noting.
#[derive(Debug)]
pub struct NoopInstrument;

impl Instrument for NoopInstrument {
    fn on_call_rejected(&self) {}

    fn on_open(&self, _: &Duration) {}

    fn on_half_open(&self) {}

    fn on_closed(&self) {}
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

impl Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.as_str())
    }
}

impl<POLICY, INSTRUMENT> StateMachine<POLICY, INSTRUMENT>
where
    POLICY: FailureAccrualPolicy,
    INSTRUMENT: Instrument,
{
    /// Creates a new state machine with given failure policy and instrument.
    pub fn new(failure_accrual_policy: POLICY, instrument: INSTRUMENT) -> Self {
        StateMachine {
            failure_accrual_policy,
            instrument,
            state: State::Closed,
        }
    }

    /// Requests permission to call this circuit breaker's backend.
    pub fn is_call_permitted(&mut self) -> bool {
        match self.state {
            State::Closed => true,
            State::HalfOpen(_) => true,
            State::Open(until, delay) => {
                if clock::now() > until {
                    self.transit_to_half_open(delay);
                    return true;
                }
                self.instrument.on_call_rejected();
                false
            }
        }
    }

    /// Records a successful call.
    ///
    /// This method must be invoked when a call was success.
    pub fn on_success(&mut self) {
        if let State::HalfOpen(_) = self.state {
            self.reset();
        }
        self.failure_accrual_policy.record_success()
    }

    /// Records a failed call.
    ///
    /// This method must be invoked when a call failed.
    pub fn on_error(&mut self) {
        match self.state {
            State::Closed => {
                if let Some(delay) = self.failure_accrual_policy.mark_dead_on_failure() {
                    self.transit_to_open(delay);
                }
            }
            State::HalfOpen(delay_in_half_open) => {
                // Pick up the next open state's delay from the policy, if policy returns Some(_)
                // use it, otherwise reuse the delay from the current state.
                let delay = self
                    .failure_accrual_policy
                    .mark_dead_on_failure()
                    .unwrap_or(delay_in_half_open);
                self.transit_to_open(delay);
            }
            _ => {}
        }
    }

    /// Returns the circuit breaker to its original closed state, losing statistics.
    #[inline]
    pub fn reset(&mut self) {
        self.state = State::Closed;
        self.failure_accrual_policy.revived();
        self.instrument.on_closed();
    }

    #[inline]
    fn transit_to_half_open(&mut self, delay: Duration) {
        self.state = State::HalfOpen(delay);
        self.instrument.on_half_open();
    }

    #[inline]
    fn transit_to_open(&mut self, delay: Duration) {
        let until = clock::now() + delay;
        self.state = State::Open(until, delay);
        self.instrument.on_open(&delay);
    }
}
