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
//! use failsafe::futures::{CircuitBreaker, Callable};
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
//! let circuit_breaker = CircuitBreaker::default();
//!
//! // Wraps `dangerous_call` result future within circuit breaker.
//! let future = circuit_breaker.call(dangerous_call());
//! let result = future.wait();

use std::sync::{Arc, Mutex};

use lib_futures::{Async, Future, Poll};

use super::backoff;
use super::config::{Config, IntoCircuitBreaker};
use super::error::Error;
use super::failure_policy::{self, ConsecutiveFailures, FailurePolicy, SuccessRateOverTimeWindow};
use super::failure_predicate::{self, FailurePredicate};
use super::state_machine::{Instrument, NoopInstrument, StateMachine};

/// A futures aware circuit breaker's public interface.
pub trait Callable {
    #[doc(hidden)]
    type Handle: Handle + Send;

    /// Requests permission to call.
    ///
    /// It returns `true` if a call is allowed, or `false` if prohibited.
    fn is_call_permitted(&self) -> bool;

    /// Executes a given future within circuit breaker.
    ///
    /// Depending on future result value, the call will be recorded as success or failure.
    #[inline]
    fn call<F>(&self, f: F) -> FutureResult<F, Self::Handle, failure_predicate::Any>
    where
        F: Future,
        F::Item: Send + 'static,
        F::Error: Send + 'static,
    {
        self.call_with(failure_predicate::Any, f)
    }

    /// Executes a given future within circuit breaker.
    ///
    /// Depending on future result value, the call will be recorded as success or failure.
    /// It also checks error by the provided predicate. If the predicate returns `true` for the
    /// error, the call is recorded as failure otherwise considered this error as a success.
    fn call_with<F, P>(&self, predicate: P, f: F) -> FutureResult<F, Self::Handle, P>
    where
        F: Future,
        F::Item: Send + 'static,
        F::Error: Send + 'static,
        P: FailurePredicate<F::Error>;
}

#[derive(Debug)]
#[doc(hidden)]
pub struct Inner<POLICY, INSTRUMENT> {
    state_machine: Mutex<StateMachine<POLICY, INSTRUMENT>>,
}

/// Future aware circuit breaker.
#[derive(Debug)]
pub struct CircuitBreaker<POLICY, INSTRUMENT> {
    inner: Arc<Inner<POLICY, INSTRUMENT>>,
}

/// For internal use only.
#[doc(hidden)]
pub trait Handle {
    /// Requests permission to call this circuit breaker's backend.
    fn is_call_permitted(&self) -> bool;

    /// Invoked after success call.
    fn on_success(&self);

    /// Invoked after failed call.
    fn on_error(&self);
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
    POLICY: FailurePolicy + Send + 'static,
    INSTRUMENT: Instrument + Send + 'static,
{
    fn new(state_machine: StateMachine<POLICY, INSTRUMENT>) -> Self {
        Self {
            inner: Arc::new(Inner {
                state_machine: Mutex::new(state_machine),
            }),
        }
    }
}

impl<POLICY, INSTRUMENT> Handle for Arc<Inner<POLICY, INSTRUMENT>>
where
    POLICY: FailurePolicy + Send + 'static,
    INSTRUMENT: Instrument + Send + 'static,
{
    #[inline]
    fn is_call_permitted(&self) -> bool {
        let mut state_machine = self.state_machine.lock().unwrap();
        state_machine.is_call_permitted()
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
    POLICY: FailurePolicy + Send + 'static,
    INSTRUMENT: Instrument + Send + 'static,
{
    type Handle = Arc<Inner<POLICY, INSTRUMENT>>;

    #[inline]
    fn is_call_permitted(&self) -> bool {
        self.inner.is_call_permitted()
    }

    #[inline]
    fn call_with<F, P>(&self, predicate: P, f: F) -> FutureResult<F, Self::Handle, P>
    where
        F: Future,
        F::Item: Send + 'static,
        F::Error: Send + 'static,
        P: FailurePredicate<F::Error>,
    {
        FutureResult {
            future: f,
            handle: self.inner.clone(),
            predicate,
            once: true,
        }
    }
}

/// A circuit breaker's future.
#[allow(missing_debug_implementations)]
pub struct FutureResult<FUT, HANDLE, PREDICATE> {
    future: FUT,
    handle: HANDLE,
    predicate: PREDICATE,
    once: bool,
}

impl<FUT, HANDLE, PREDICATE> Future for FutureResult<FUT, HANDLE, PREDICATE>
where
    FUT: Future,
    HANDLE: Handle,
    PREDICATE: FailurePredicate<FUT::Error>,
{
    type Item = FUT::Item;
    type Error = Error<FUT::Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if self.once {
            self.once = false;

            if !self.handle.is_call_permitted() {
                return Err(Error::Rejected);
            }
        }

        match self.future.poll() {
            Ok(Async::Ready(ok)) => {
                self.handle.on_success();
                Ok(Async::Ready(ok))
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => {
                if self.predicate.is_err(&err) {
                    self.handle.on_error();
                } else {
                    self.handle.on_success();
                }
                Err(Error::Inner(err))
            }
        }
    }
}
impl<POLICY, INSTRUMENT> Clone for CircuitBreaker<POLICY, INSTRUMENT> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct Tag;

impl<POLICY, INSTRUMENT> IntoCircuitBreaker for Config<POLICY, INSTRUMENT, Tag>
where
    POLICY: FailurePolicy + Send + 'static,
    INSTRUMENT: Instrument + Send + 'static,
{
    type Output = CircuitBreaker<POLICY, INSTRUMENT>;

    fn into_circuit_breaker(self) -> CircuitBreaker<POLICY, INSTRUMENT> {
        let state_machine = StateMachine::new(self.failure_policy, self.instrument);
        CircuitBreaker::new(state_machine)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use lib_futures::future;
    use tokio::runtime::Runtime;
    use tokio::timer::Delay;

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

    fn new_circuit_breaker() -> impl Callable {
        let backoff = backoff::constant(Duration::from_secs(5));
        let policy = failure_policy::consecutive_failures(1, backoff);
        CircuitBreaker::builder().failure_policy(policy).build()
    }
}
