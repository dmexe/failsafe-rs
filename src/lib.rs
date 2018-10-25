//! CircuitBreaker is used to detect failures and encapsulates the logic of preventing a failure
//! from constantly recurring, during maintenance, temporary external system failure or unexpected
//! system difficulties.
//!
//! # Links
//!
//! * Future aware circuit breaker's interface [futures::CircuitBreaker](futures/index.html).
//! * The state machine which is used [StateMachine](state_machine/StateMachine.t.html).
//! * More about circuit breakers [https://martinfowler.com/bliki/CircuitBreaker.html](https://martinfowler.com/bliki/CircuitBreaker.html)
//!
//! # Example
//!
//! Using default backoff strategy and failure accrual policy.
//!
//! ```
//! # extern crate failsafe;
//! # extern crate rand;
//! # use rand::{thread_rng, Rng};
//!
//! use failsafe::{Config, CircuitBreaker, Error};
//!
//! // A function that sometimes failed.
//! fn dangerous_call() -> Result<(), ()> {
//!   if thread_rng().gen_range(0, 2) == 0 {
//!     return Err(())
//!   }
//!   Ok(())
//! }
//!
//! // Create a circuit breaker which configured by reasonable default backoff and
//! // failure accrual policy.
//! let circuit_breaker = Config::new().build();
//!
//! // Call the function in a loop, after some iterations the circuit breaker will
//! // be in a open state and reject next calls.
//! for n in 0..100 {
//!   match circuit_breaker.call(|| dangerous_call()) {
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
//!
//! Or configure custom backoff and policy:
//!
//! ```
//! # extern crate failsafe;
//! # extern crate rand;
//!
//! use std::time::Duration;
//! use failsafe::{backoff, failure_policy, Config, CircuitBreaker};
//!
//! fn circuit_breaker() -> impl CircuitBreaker {
//!   // Create an exponential growth backoff which starts from 10s and ends with 60s.
//!   let backoff = backoff::exponential(Duration::from_secs(10), Duration::from_secs(60));
//!
//!   // Create a policy which failed when three consecutive failures were made.
//!   let policy = failure_policy::consecutive_failures(3, backoff);
//!
//!   // Creates a circuit breaker with given policy.
//!   Config::new()
//!     .failure_policy(policy)
//!     .build()
//! }
//! ```

#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![cfg_attr(test, deny(warnings))]

extern crate futures as lib_futures;
extern crate rand;
extern crate parking_lot;

#[cfg(test)]
extern crate tokio;

mod circuit_breaker;
mod config;
mod ema;
mod error;
mod failure_predicate;
mod instrument;
mod state_machine;
mod windowed_adder;

pub mod backoff;
pub mod failure_policy;
pub mod futures;

#[doc(hidden)]
pub mod clock;

pub use self::circuit_breaker::CircuitBreaker;
pub use self::config::Config;
pub use self::error::Error;
pub use self::failure_policy::FailurePolicy;
pub use self::failure_predicate::FailurePredicate;
pub use self::instrument::Instrument;
pub use self::state_machine::StateMachine;
pub use self::windowed_adder::WindowedAdder;
