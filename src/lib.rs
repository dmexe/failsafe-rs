//! CircuitBreaker is used to detect failures and encapsulates the logic of preventing a failure
//! from constantly recurring, during maintenance, temporary external system failure or unexpected
//! system difficulties.
//!
//! See https://martinfowler.com/bliki/CircuitBreaker.html

#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![cfg_attr(test, deny(warnings))]

extern crate futures;
extern crate rand;
extern crate tokio_timer;

#[cfg(test)]
extern crate tokio_executor;

mod ema;
mod failure_predicate;
mod state_machine;
mod circuit_breaker;

pub mod backoff;
pub mod failure_accrual;

#[cfg(test)]
mod mock_clock;

pub use self::failure_predicate::FailurePredicate;
pub use self::state_machine::{Instrumentation, StateMachine};
