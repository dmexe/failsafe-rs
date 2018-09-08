use std::sync::{Arc, Mutex};

use super::backoff;
use super::config::{Config, IntoCircuitBreaker};
use super::error::Error;
use super::failure_policy::{self, ConsecutiveFailures, FailurePolicy, SuccessRateOverTimeWindow};
use super::failure_predicate::{self, FailurePredicate};
use super::instrument::{Instrument, NoopInstrument};
use super::state_machine::StateMachine;

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

impl CircuitBreaker<(), ()> {
    /// Returns a circuit breaker's builder.
    pub fn builder() -> Config<
        failure_policy::OrElse<
            SuccessRateOverTimeWindow<backoff::EqualJittered>,
            ConsecutiveFailures<backoff::EqualJittered>,
        >,
        NoopInstrument,
        Tag,
    > {
        Config::new()
    }
}

impl Default
    for CircuitBreaker<
        failure_policy::OrElse<
            SuccessRateOverTimeWindow<backoff::EqualJittered>,
            ConsecutiveFailures<backoff::EqualJittered>,
        >,
        NoopInstrument,
    >
{
    fn default() -> Self {
        CircuitBreaker::builder().build()
    }
}

impl<POLICY, INSTRUMENT> CircuitBreaker<POLICY, INSTRUMENT>
where
    POLICY: FailurePolicy,
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
    POLICY: FailurePolicy,
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

#[doc(hidden)]
#[derive(Debug)]
pub struct Tag;

impl<POLICY, INSTRUMENT> IntoCircuitBreaker for Config<POLICY, INSTRUMENT, Tag>
where
    POLICY: FailurePolicy,
    INSTRUMENT: Instrument,
{
    type Output = CircuitBreaker<POLICY, INSTRUMENT>;

    fn into_circuit_breaker(self) -> CircuitBreaker<POLICY, INSTRUMENT> {
        let state_machine = StateMachine::new(self.failure_policy, self.instrument);
        CircuitBreaker::new(state_machine)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::backoff;
    use super::super::failure_policy::consecutive_failures;
    use super::*;

    #[test]
    fn call_with() {
        let circuit_breaker = new_circuit_breaker();
        let is_err = |err: &bool| !(*err);

        for _ in 0..2 {
            match circuit_breaker.call_with(is_err, || Err::<(), _>(true)) {
                Err(Error::Inner(true)) => {}
                x => unreachable!("{:?}", x),
            }
            assert_eq!(true, circuit_breaker.is_call_permitted());
        }

        match circuit_breaker.call_with(is_err, || Err::<(), _>(false)) {
            Err(Error::Inner(false)) => {}
            x => unreachable!("{:?}", x),
        }
        assert_eq!(false, circuit_breaker.is_call_permitted());
    }

    #[test]
    fn call_ok() {
        let circuit_breaker = new_circuit_breaker();

        circuit_breaker.call(|| Ok::<_, ()>(())).unwrap();
        assert_eq!(true, circuit_breaker.is_call_permitted());
    }

    #[test]
    fn call_err() {
        let circuit_breaker = new_circuit_breaker();

        match circuit_breaker.call(|| Err::<(), _>(())) {
            Err(Error::Inner(())) => {}
            x => unreachable!("{:?}", x),
        }
        assert_eq!(false, circuit_breaker.is_call_permitted());

        match circuit_breaker.call(|| Err::<(), _>(())) {
            Err(Error::Rejected) => {}
            x => unreachable!("{:?}", x),
        }
        assert_eq!(false, circuit_breaker.is_call_permitted());
    }

    fn new_circuit_breaker() -> impl Callable {
        let backoff = backoff::constant(Duration::from_secs(5));
        let policy = consecutive_failures(1, backoff);
        CircuitBreaker::builder().failure_policy(policy).build()
    }
}
