use super::error::Error;
use super::failure_policy::FailurePolicy;
use super::failure_predicate::{self, FailurePredicate};
use super::instrument::Instrument;
use super::state_machine::StateMachine;

/// A circuit breaker's public interface.
pub trait CircuitBreaker {
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
    /// It checks error by the provided predicate. If the predicate returns `true` for the
    /// error, the call is recorded as failure otherwise considered this error as a success.
    fn call_with<P, F, E, R>(&self, predicate: P, f: F) -> Result<R, Error<E>>
    where
        P: FailurePredicate<E>,
        F: FnOnce() -> Result<R, E>;
}

impl<POLICY, INSTRUMENT> CircuitBreaker for StateMachine<POLICY, INSTRUMENT>
where
    POLICY: FailurePolicy,
    INSTRUMENT: Instrument,
{
    #[inline]
    fn is_call_permitted(&self) -> bool {
        self.is_call_permitted()
    }

    fn call_with<P, F, E, R>(&self, predicate: P, f: F) -> Result<R, Error<E>>
    where
        P: FailurePredicate<E>,
        F: FnOnce() -> Result<R, E>,
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::backoff;
    use super::super::config::Config;
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
            assert!(circuit_breaker.is_call_permitted());
        }

        match circuit_breaker.call_with(is_err, || Err::<(), _>(false)) {
            Err(Error::Inner(false)) => {}
            x => unreachable!("{:?}", x),
        }
        assert!(!circuit_breaker.is_call_permitted());
    }

    #[test]
    fn call_ok() {
        let circuit_breaker = new_circuit_breaker();

        circuit_breaker.call(|| Ok::<_, ()>(())).unwrap();
        assert!(circuit_breaker.is_call_permitted());
    }

    #[test]
    fn call_err() {
        let circuit_breaker = new_circuit_breaker();

        match circuit_breaker.call(|| Err::<(), _>(())) {
            Err(Error::Inner(())) => {}
            x => unreachable!("{:?}", x),
        }
        assert!(!circuit_breaker.is_call_permitted());

        match circuit_breaker.call(|| Err::<(), _>(())) {
            Err(Error::Rejected) => {}
            x => unreachable!("{:?}", x),
        }
        assert!(!circuit_breaker.is_call_permitted());
    }

    fn new_circuit_breaker() -> impl CircuitBreaker {
        let backoff = backoff::constant(Duration::from_secs(5));
        let policy = consecutive_failures(1, backoff);
        Config::new().failure_policy(policy).build()
    }
}
