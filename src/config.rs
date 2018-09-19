use super::backoff;
use super::failure_policy::{self, ConsecutiveFailures, FailurePolicy, SuccessRateOverTimeWindow};
use super::instrument::Instrument;
use super::state_machine::StateMachine;

/// A `CircuitBreaker`'s configuration.
#[derive(Debug)]
pub struct Config<POLICY, INSTRUMENT> {
    pub(crate) failure_policy: POLICY,
    pub(crate) instrument: INSTRUMENT,
}

impl Config<(), ()> {
    /// Creates a new circuit breaker's default configuration.
    pub fn new() -> Config<
        failure_policy::OrElse<
            SuccessRateOverTimeWindow<backoff::EqualJittered>,
            ConsecutiveFailures<backoff::EqualJittered>,
        >,
        (),
    > {
        let failure_policy =
            SuccessRateOverTimeWindow::default().or_else(ConsecutiveFailures::default());

        Config {
            failure_policy,
            instrument: (),
        }
    }
}

impl<POLICY, INSTRUMENT> Config<POLICY, INSTRUMENT> {
    /// Configures `FailurePolicy` for a circuit breaker.
    pub fn failure_policy<T>(self, failure_policy: T) -> Config<T, INSTRUMENT>
    where
        T: FailurePolicy,
    {
        Config {
            failure_policy,
            instrument: self.instrument,
        }
    }

    /// Configures `Instrument` for a circuit breaker.
    pub fn instrument<T>(self, instrument: T) -> Config<POLICY, T>
    where
        T: Instrument,
    {
        Config {
            failure_policy: self.failure_policy,
            instrument,
        }
    }

    /// Builds a new circuit breaker instance.
    pub fn build(self) -> StateMachine<POLICY, INSTRUMENT>
    where
        POLICY: FailurePolicy,
        INSTRUMENT: Instrument,
    {
        StateMachine::new(self.failure_policy, self.instrument)
    }
}
