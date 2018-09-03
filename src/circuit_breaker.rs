use std::sync::{Arc, Mutex};

use super::backoff;
use super::error::Error;
use super::failure_accrual_policy::{
    self, ConsecutiveFailures, FailureAccrualPolicy, SuccessRateOverTimeWindow,
};
use super::failure_predicate::{self, FailurePredicate};
use super::state_machine::{Instrument, NoopInstrument, StateMachine};

/// A `CircuitBreaker`'s builder.
#[derive(Debug)]
pub struct Builder<POLICY, INSTRUMENT> {
    failure_accrual_policy: POLICY,
    instrument: INSTRUMENT,
}

/// TODO.
#[derive(Debug)]
pub struct CircuitBreaker<POLICY, INSTRUMENT> {
    state_machine: Arc<Mutex<StateMachine<POLICY, INSTRUMENT>>>,
}

/// A circuit breaker's public interface.
pub trait Callable {
    /// Requests permission to call.
    ///
    /// It returns `true` if a call is allowed, or `false` if prohibited.
    fn is_call_permitted(&self) -> bool;

    /// Executes a given function within circuit breaker.
    ///
    /// Depending on function result value, the call will be recorded as success or failure.
    #[inline]
    fn call<F, E, R>(&self, f: F) -> Result<R, Error<E>>
    where
        F: FnOnce() -> Result<R, E>,
    {
        self.call_with(failure_predicate::Any, f)
    }

    /// Executes a given function within circuit breaker.
    ///
    /// Depending on function result value, the call will be recorded as success or failure.
    /// It also checks error by the provided predicate. If the predicate returns `true` for the
    /// error, the call is recorded as failure otherwise considered this error as a success.
    fn call_with<P, F, E, R>(&self, predicate: P, f: F) -> Result<R, Error<E>>
    where
        F: FnOnce() -> Result<R, E>,
        P: FailurePredicate<E>;
}

impl<POLICY, INSTRUMENT> Builder<POLICY, INSTRUMENT> {
    /// Configures `FailureAccrualPolicy` for a circuit breaker.
    pub fn failure_accrual_policy<T>(self, failure_accrual_policy: T) -> Builder<T, INSTRUMENT> {
        Builder {
            failure_accrual_policy,
            instrument: self.instrument,
        }
    }

    /// Configures `Instrument` for a circuit breaker.
    pub fn instrument<T>(self, instrument: T) -> Builder<POLICY, T> {
        Builder {
            failure_accrual_policy: self.failure_accrual_policy,
            instrument,
        }
    }

    /// Builds a new circuit breaker instance.
    pub fn build(self) -> CircuitBreaker<POLICY, INSTRUMENT>
    where
        POLICY: FailureAccrualPolicy,
        INSTRUMENT: Instrument,
    {
        let state_machine = StateMachine::new(self.failure_accrual_policy, self.instrument);
        CircuitBreaker::new(state_machine)
    }
}

impl Default
    for Builder<
        failure_accrual_policy::OrElse<
            SuccessRateOverTimeWindow<backoff::EqualJittered>,
            ConsecutiveFailures<backoff::EqualJittered>,
        >,
        NoopInstrument,
    >
{
    fn default() -> Self {
        let failure_accrual_policy =
            SuccessRateOverTimeWindow::default().or_else(ConsecutiveFailures::default());
        let instrument = NoopInstrument;

        Self {
            failure_accrual_policy,
            instrument,
        }
    }
}

impl Default
    for CircuitBreaker<
        failure_accrual_policy::OrElse<
            SuccessRateOverTimeWindow<backoff::EqualJittered>,
            ConsecutiveFailures<backoff::EqualJittered>,
        >,
        NoopInstrument,
    >
{
    fn default() -> Self {
        Builder::default().build()
    }
}

impl CircuitBreaker<(), ()> {
    /// Returns a circuit breaker's builder.
    pub fn builder() -> Builder<
        failure_accrual_policy::OrElse<
            SuccessRateOverTimeWindow<backoff::EqualJittered>,
            ConsecutiveFailures<backoff::EqualJittered>,
        >,
        NoopInstrument,
    > {
        Builder::default()
    }
}

impl<POLICY, INSTRUMENT> CircuitBreaker<POLICY, INSTRUMENT>
where
    POLICY: FailureAccrualPolicy,
    INSTRUMENT: Instrument,
{
    /// Creates a new circuit breaker using given state machine.
    fn new(state_machine: StateMachine<POLICY, INSTRUMENT>) -> Self {
        Self {
            state_machine: Arc::new(Mutex::new(state_machine)),
        }
    }

    /// Invoked after success call.
    #[inline]
    fn on_success(&self) {
        let mut state_machine = self.state_machine.lock().unwrap();
        state_machine.on_success();
    }

    /// Invoked after failed call.
    #[inline]
    fn on_error(&self) {
        let mut state_machine = self.state_machine.lock().unwrap();
        state_machine.on_error();
    }
}

impl<POLICY, INSTRUMENT> Callable for CircuitBreaker<POLICY, INSTRUMENT>
where
    POLICY: FailureAccrualPolicy,
    INSTRUMENT: Instrument,
{
    #[inline]
    fn is_call_permitted(&self) -> bool {
        let mut state_machine = self.state_machine.lock().unwrap();
        state_machine.is_call_permitted()
    }

    #[inline]
    fn call_with<P, F, E, R>(&self, predicate: P, f: F) -> Result<R, Error<E>>
    where
        F: FnOnce() -> Result<R, E>,
        P: FailurePredicate<E>,
    {
        if !self.is_call_permitted() {
            return Err(Error::Rejected);
        }

        match f() {
            Ok(ok) => {
                self.on_success();
                Ok(ok)
            }
            Err(err) => {
                if predicate.is_err(&err) {
                    self.on_error();
                } else {
                    self.on_success();
                }
                Err(Error::Inner(err))
            }
        }
    }
}

impl<POLICY, INSTRUMENT> Clone for CircuitBreaker<POLICY, INSTRUMENT> {
    fn clone(&self) -> Self {
        Self {
            state_machine: self.state_machine.clone(),
        }
    }
}
