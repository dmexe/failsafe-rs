//! CircuitBreaker is used to detect failures and encapsulates the logic of preventing a failure
//! from constantly recurring, during maintenance, temporary external system failure or unexpected
//! system difficulties.
//!
//! See https://martinfowler.com/bliki/CircuitBreaker.html
//!
//! # Example
//!
//! ```
//! # extern crate resilience;
//! # extern crate rand;
//! # use rand::{thread_rng, Rng};
//!
//! use resilience::{CircuitBreaker, Callable, Error};
//!
//! // A function that sometimes failed.
//! fn danger_call() -> Result<(), ()> {
//!   if thread_rng().gen_range(0, 2) == 0 {
//!     return Err(())
//!   }
//!   Ok(())
//! }
//!
//! // Create a circuit breaker which configured by reasonable default backoff and
//! // failure accrual policy.
//! let circuit_breaker = CircuitBreaker::default();
//!
//! // In cycle call the function, after some iterations the circuit breaker will
//! // be in a open state and reject calls.
//! for n in 0..100 {
//!   match circuit_breaker.call(|| danger_call()) {
//!     Err(Error::Inner(_)) => {
//!       eprintln!("{}: fail", n);
//!     },
//!     Err(Error::Rejected) => {
//!        eprintln!("{}: rejected", n);
//!        break;
//!     },
//!     _ => {}
//!   }
//! }
//! ```

#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![cfg_attr(test, deny(warnings))]

extern crate futures;
extern crate rand;
extern crate tokio_timer;

#[cfg(test)]
extern crate tokio_executor;

mod circuit_breaker;
mod ema;
mod error;
mod failure_predicate;
mod state_machine;

pub mod backoff;
pub mod failure_accrual_policy;

#[cfg(test)]
mod mock_clock;

pub use self::circuit_breaker::{Builder, Callable, CircuitBreaker};
pub use self::error::Error;
pub use self::failure_predicate::FailurePredicate;
pub use self::state_machine::{Instrument, StateMachine};
