//! Futures aware circuit breaker.
//!
//! # Example
//!
//! Using default backoff strategy and failure accrual policy.
//!
//! ```
//! # extern crate rand;
//! # use rand::{thread_rng, Rng};
//! # async {
//!
//! use failsafe::Config;
//! use failsafe::futures::CircuitBreaker;
//!
//! // A function that sometimes fails.
//! async fn dangerous_call() -> Result<(), ()> {
//!   if thread_rng().gen_range(0..2) == 0 {
//!     return Err(())
//!   }
//!   Ok(())
//! }
//!
//! // Create a circuit breaker which configured by reasonable default backoff and
//! // failure accrual policy.
//! let circuit_breaker = Config::new().build();
//!
//! // Wraps `dangerous_call` result future within circuit breaker.
//! let future = circuit_breaker.call(dangerous_call());
//! let result = future.await;
//!
//! # }; // async

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::future::TryFuture;

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
        F: TryFuture,
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
        F: TryFuture,
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
        F: TryFuture,
        P: FailurePredicate<F::Error>,
    {
        ResponseFuture {
            future: f,
            state_machine: self.clone(),
            predicate,
            ask: false,
        }
    }
}

pin_project_lite::pin_project! {
    /// A circuit breaker's future.
    #[allow(missing_debug_implementations)]
    pub struct ResponseFuture<FUTURE, POLICY, INSTRUMENT, PREDICATE> {
        #[pin]
        future: FUTURE,
        state_machine: StateMachine<POLICY, INSTRUMENT>,
        predicate: PREDICATE,
        ask: bool,
    }
}

impl<FUTURE, POLICY, INSTRUMENT, PREDICATE> Future
    for ResponseFuture<FUTURE, POLICY, INSTRUMENT, PREDICATE>
where
    FUTURE: TryFuture,
    POLICY: FailurePolicy,
    INSTRUMENT: Instrument,
    PREDICATE: FailurePredicate<FUTURE::Error>,
{
    type Output = Result<FUTURE::Ok, Error<FUTURE::Error>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let this = self.project();

        if !*this.ask {
            *this.ask = true;
            if !this.state_machine.is_call_permitted() {
                return Poll::Ready(Err(Error::Rejected));
            }
        }

        match this.future.try_poll(cx) {
            Poll::Ready(Ok(ok)) => {
                this.state_machine.on_success();
                Poll::Ready(Ok(ok))
            }
            Poll::Ready(Err(err)) => {
                if this.predicate.is_err(&err) {
                    this.state_machine.on_error();
                } else {
                    this.state_machine.on_success();
                }
                Poll::Ready(Err(Error::Inner(err)))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::future;

    use super::super::backoff;
    use super::super::config::Config;
    use super::super::failure_policy;
    use super::*;

    #[tokio::test]
    async fn call_ok() {
        let circuit_breaker = new_circuit_breaker();
        let future = delay_for(Duration::from_millis(100));
        let future = circuit_breaker.call(future);

        future.await.unwrap();
        assert_eq!(true, circuit_breaker.is_call_permitted());
    }

    #[tokio::test]
    async fn call_err() {
        let circuit_breaker = new_circuit_breaker();

        let future = future::err::<(), ()>(());
        let future = circuit_breaker.call(future);
        match future.await {
            Err(Error::Inner(_)) => {}
            err => unreachable!("{:?}", err),
        }
        assert_eq!(false, circuit_breaker.is_call_permitted());

        let future = delay_for(Duration::from_secs(1));
        let future = circuit_breaker.call(future);
        match future.await {
            Err(Error::Rejected) => {}
            err => unreachable!("{:?}", err),
        }
        assert_eq!(false, circuit_breaker.is_call_permitted());
    }

    #[tokio::test]
    async fn call_with() {
        let circuit_breaker = new_circuit_breaker();
        let is_err = |err: &bool| !(*err);

        for _ in 0..2 {
            let future = future::err::<(), _>(true);
            let future = circuit_breaker.call_with(is_err, future);
            match future.await {
                Err(Error::Inner(true)) => {}
                err => unreachable!("{:?}", err),
            }
            assert_eq!(true, circuit_breaker.is_call_permitted());
        }

        let future = future::err::<(), _>(false);
        let future = circuit_breaker.call_with(is_err, future);
        match future.await {
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

    async fn delay_for(duration: Duration) -> Result<(), ()> {
        tokio::time::sleep(duration).await;
        Ok(())
    }
}
