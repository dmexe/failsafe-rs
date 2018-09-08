use std::marker::PhantomData;

use super::backoff;
use super::failure_policy::{self, ConsecutiveFailures, FailurePolicy, SuccessRateOverTimeWindow};
use super::instrument::{Instrument, NoopInstrument};

/// A `CircuitBreaker`'s configuration.
#[derive(Debug)]
pub struct Config<POLICY, INSTRUMENT, TAG> {
    pub(crate) failure_policy: POLICY,
    pub(crate) instrument: INSTRUMENT,
    phantom: PhantomData<TAG>,
}

impl Config<(), (), ()> {
    pub(crate) fn new<TAG>() -> Config<
        failure_policy::OrElse<
            SuccessRateOverTimeWindow<backoff::EqualJittered>,
            ConsecutiveFailures<backoff::EqualJittered>,
        >,
        NoopInstrument,
        TAG,
    > {
        let failure_policy =
            SuccessRateOverTimeWindow::default().or_else(ConsecutiveFailures::default());
        let instrument = NoopInstrument;

        Config {
            failure_policy,
            instrument,
            phantom: PhantomData,
        }
    }
}

impl<POLICY, INSTRUMENT, TAG> Config<POLICY, INSTRUMENT, TAG> {
    /// Configures `FailurePolicy` for a circuit breaker.
    pub fn failure_policy<T>(self, failure_policy: T) -> Config<T, INSTRUMENT, TAG> {
        Config {
            failure_policy,
            instrument: self.instrument,
            phantom: self.phantom,
        }
    }

    /// Configures `Instrument` for a circuit breaker.
    pub fn instrument<T>(self, instrument: T) -> Config<POLICY, T, TAG> {
        Config {
            failure_policy: self.failure_policy,
            instrument,
            phantom: self.phantom,
        }
    }

    /// Builds a new circuit breaker instance.
    pub fn build(self) -> <Self as IntoCircuitBreaker>::Output
    where
        POLICY: FailurePolicy,
        INSTRUMENT: Instrument,
        Self: IntoCircuitBreaker,
    {
        IntoCircuitBreaker::into_circuit_breaker(self)
    }
}

#[doc(hidden)]
pub trait IntoCircuitBreaker {
    type Output;

    fn into_circuit_breaker(self) -> Self::Output;
}
