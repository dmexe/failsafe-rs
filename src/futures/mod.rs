//! Futures aware circuit breaker.
//!
//! # Example
//!
//! Using default backoff strategy and failure accrual policy.
//!
//! ```
//! # extern crate failsafe;
//! # extern crate rand;
//! # extern crate futures;
//! # use rand::{thread_rng, Rng};
//!
//! use futures::{future, Future};
//! use failsafe::Config;
//! use failsafe::futures::CircuitBreaker;
//!
//! // A function that sometimes failed.
//! fn dangerous_call() -> impl Future<Item = (), Error = ()> {
//!   future::lazy(|| {
//!     if thread_rng().gen_range(0, 2) == 0 {
//!       return Err(())
//!     }
//!     Ok(())
//!   })
//! }
//!
//! // Create a circuit breaker which configured by reasonable default backoff and
//! // failure accrual policy.
//! let circuit_breaker = Config::new().build();
//!
//! // Wraps `dangerous_call` result future within circuit breaker.
//! let future = circuit_breaker.call(dangerous_call());
//! let result = future.wait();

use lib_futures::{Async, Future, Poll};

use super::error::Error;
use super::failure_policy::FailurePolicy;
use super::failure_predicate::{self, FailurePredicate};
use super::instrument::Instrument;
use super::state_machine::StateMachine;

/// A futures aware circuit breaker's public interface.
pub trait CircuitBreaker {
    #[doc(hidden)]
    type FailurePolicy: FailurePolicy + Send + Sync;
    #[doc(hidden)]
    type Instrument: Instrument + Send + Sync;

    /// Requests permission to call.
    ///
    /// It returns `true` if a call is allowed, or `false` if prohibited.
    fn is_call_permitted(&self) -> bool;

    /// Executes a given future within circuit breaker.
    ///
    /// Depending on future result value, the call will be recorded as success or failure.
    #[inline]
    fn call<F>(
        &self,
        f: F,
    ) -> ResponseFuture<F, Self::FailurePolicy, Self::Instrument, failure_predicate::Any>
    where
        F: Future,
    {
        self.call_with(failure_predicate::Any, f)
    }

    /// Executes a given future within circuit breaker.
    ///
    /// Depending on future result value, the call will be recorded as success or failure.
    /// It checks error by the provided predicate. If the predicate returns `true` for the
    /// error, the call is recorded as failure otherwise considered this error as a success.
    fn call_with<F, P>(
        &self,
        predicate: P,
        f: F,
    ) -> ResponseFuture<F, Self::FailurePolicy, Self::Instrument, P>
    where
        F: Future,
        P: FailurePredicate<F::Error>;
}

impl<POLICY, INSTRUMENT> CircuitBreaker for StateMachine<POLICY, INSTRUMENT>
where
    POLICY: FailurePolicy + Send + Sync,
    INSTRUMENT: Instrument + Send + Sync,
{
    type FailurePolicy = POLICY;
    type Instrument = INSTRUMENT;

    #[inline]
    fn is_call_permitted(&self) -> bool {
        self.is_call_permitted()
    }

    #[inline]
    fn call_with<F, P>(
        &self,
        predicate: P,
        f: F,
    ) -> ResponseFuture<F, Self::FailurePolicy, Self::Instrument, P>
    where
        F: Future,
        P: FailurePredicate<F::Error>,
    {
        ResponseFuture {
            future: f,
            state_machine: self.clone(),
            predicate,
            state: State::Request,
        }
    }
}

enum State {
    Request,
    Permitted,
    Rejected,
}

/// A circuit breaker's future.
#[allow(missing_debug_implementations)]
pub struct ResponseFuture<FUTURE, POLICY, INSTRUMENT, PREDICATE> {
    future: FUTURE,
    state_machine: StateMachine<POLICY, INSTRUMENT>,
    predicate: PREDICATE,
    state: State,
}

impl<FUTURE, POLICY, INSTRUMENT, PREDICATE> Future
    for ResponseFuture<FUTURE, POLICY, INSTRUMENT, PREDICATE>
where
    FUTURE: Future,
    POLICY: FailurePolicy,
    INSTRUMENT: Instrument,
    PREDICATE: FailurePredicate<FUTURE::Error>,
{
    type Item = FUTURE::Item;
    type Error = Error<FUTURE::Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let State::Request = self.state {
            if self.state_machine.is_call_permitted() {
                self.state = State::Permitted
            } else {
                self.state = State::Rejected
            }
        }

        if let State::Rejected = self.state {
            return Err(Error::Rejected);
        }

        match self.future.poll() {
            Ok(Async::Ready(ok)) => {
                self.state_machine.on_success();
                Ok(Async::Ready(ok))
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => {
                if self.predicate.is_err(&err) {
                    self.state_machine.on_error();
                } else {
                    self.state_machine.on_success();
                }
                Err(Error::Inner(err))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use lib_futures::future;
    use tokio::runtime::Runtime;
    use tokio::timer::Delay;

    use super::super::backoff;
    use super::super::config::Config;
    use super::super::failure_policy;
    use super::*;

    #[test]
    fn call_ok() {
        let mut runtime = Runtime::new().unwrap();
        let circuit_breaker = new_circuit_breaker();
        let future = Delay::new(Instant::now() + Duration::from_millis(100));
        let future = circuit_breaker.call(future);

        runtime.block_on(future).unwrap();
        assert_eq!(true, circuit_breaker.is_call_permitted());
    }

    #[test]
    fn call_err() {
        let mut runtime = Runtime::new().unwrap();
        let circuit_breaker = new_circuit_breaker();

        let future = future::lazy(|| Err::<(), ()>(()));
        let future = circuit_breaker.call(future);
        match runtime.block_on(future) {
            Err(Error::Inner(_)) => {}
            err => unreachable!("{:?}", err),
        }
        assert_eq!(false, circuit_breaker.is_call_permitted());

        let future = Delay::new(Instant::now() + Duration::from_secs(1));
        let future = circuit_breaker.call(future);
        match runtime.block_on(future) {
            Err(Error::Rejected) => {}
            err => unreachable!("{:?}", err),
        }
        assert_eq!(false, circuit_breaker.is_call_permitted());
    }

    #[test]
    fn call_with() {
        let mut runtime = Runtime::new().unwrap();
        let circuit_breaker = new_circuit_breaker();
        let is_err = |err: &bool| !(*err);

        for _ in 0..2 {
            let future = future::lazy(|| Err::<(), _>(true));
            let future = circuit_breaker.call_with(is_err, future);
            match runtime.block_on(future) {
                Err(Error::Inner(true)) => {}
                err => unreachable!("{:?}", err),
            }
            assert_eq!(true, circuit_breaker.is_call_permitted());
        }

        let future = future::lazy(|| Err::<(), _>(false));
        let future = circuit_breaker.call_with(is_err, future);
        match runtime.block_on(future) {
            Err(Error::Inner(false)) => {}
            err => unreachable!("{:?}", err),
        }
        assert_eq!(false, circuit_breaker.is_call_permitted());
    }

    fn new_circuit_breaker() -> impl CircuitBreaker {
        let backoff = backoff::constant(Duration::from_secs(5));
        let policy = failure_policy::consecutive_failures(1, backoff);
        Config::new().failure_policy(policy).build()
    }
}
