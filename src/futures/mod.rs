//! Futures aware circuit breaker.

use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use lib_futures::{Async, Future, Poll, Stream};

use super::backoff::Backoff;
use super::error::Error;
use super::failure_accrual_policy::FailureAccrualPolicy;
use super::failure_predicate::{self, FailurePredicate};
use super::state_machine::{Instrument, StateMachine};

/// A circuit breaker's public interface.
pub trait Callable {
    /// Executes a given future within circuit breaker.
    ///
    /// Depending on future result value, the call will be recorded as success or failure.
    #[inline]
    fn call<FUNC, FUTURE, ITEM, ERR>(&self, f: FUNC) -> FutureResult<FUNC, ITEM, ERR>
    where
        FUNC: FnOnce() -> FUTURE,
        FUTURE: Future<Item = ITEM, Error = ERR>,
        ITEM: Send + 'static,
        ERR: Send + 'static,
    {
        self.call_with(failure_predicate::Any, f)
    }

    /// Executes a given future within circuit breaker.
    ///
    /// Depending on future result value, the call will be recorded as success or failure.
    /// It also checks error by the provided predicate. If the predicate returns `true` for the
    /// error, the call is recorded as failure otherwise considered this error as a success.
    fn call_with<FUNC, FUTURE, PREDICATE, ITEM, ERR>(
        &self,
        predicate: PREDICATE,
        f: FUNC,
    ) -> FutureResult<FUNC, ITEM, ERR>
    where
        FUNC: FnOnce() -> FUTURE,
        FUTURE: Future<Item = ITEM, Error = ERR>,
        ITEM: Send + 'static,
        ERR: Send + 'static,
        PREDICATE: FailurePredicate<ERR>;
}

#[derive(Debug)]
pub struct CircuitBreaker<POLICY, INSTRUMENT> {
    state_machine: Arc<Mutex<StateMachine<POLICY, INSTRUMENT>>>,
}

impl<POLICY, INSTRUMENT> CircuitBreaker<POLICY, INSTRUMENT>
where
    POLICY: FailureAccrualPolicy + Send + 'static,
    INSTRUMENT: Instrument + Send + 'static,
{
    pub fn new(state_machine: StateMachine<POLICY, INSTRUMENT>) -> Self {
        Self {
            state_machine: Arc::new(Mutex::new(state_machine)),
        }
    }
}

impl<POLICY, INSTRUMENT> Callable for CircuitBreaker<POLICY, INSTRUMENT> {
    fn call_with<FUNC, FUTURE, PREDICATE, ITEM, ERR>(
        &self,
        predicate: PREDICATE,
        f: FUNC,
    ) -> FutureResult<FUNC, ITEM, ERR>
    where
        FUNC: FnOnce() -> FUTURE,
        FUTURE: Future<Item = ITEM, Error = ERR>,
        ITEM: Send + 'static,
        ERR: Send + 'static,
        PREDICATE: FailurePredicate<ERR>,
    {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct FutureResult<F, ITEM, ERR> {
    f: F,
    phantom: PhantomData<(ITEM, ERR)>,
}
