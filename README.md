# Failsafe

[![Сrate](https://img.shields.io/crates/v/failsafe.svg)](https://crates.io/crates/failsafe)
[![Вocumentation](https://docs.rs/failsafe/badge.svg)](https://docs.rs/failsafe)
[![CircleCI](https://circleci.com/gh/dmexe/failsafe-rs.svg?style=svg)](https://circleci.com/gh/dmexe/failsafe-rs)
[![Appveyor](https://ci.appveyor.com/api/projects/status/c0qrj9dbskneunjg/branch/master?svg=true)](https://ci.appveyor.com/project/dmexe/failsafe-rs/branch/master)

A circuit breaker implementation which used to detect failures and encapsulates the logic of preventing a
failure from constantly recurring, during maintenance, temporary external system failure or unexpected
system difficulties.

* [https://martinfowler.com/bliki/CircuitBreaker.html](https://martinfowler.com/bliki/CircuitBreaker.html)
* [Read documentation](https://docs.rs/failsafe/1.1.0/failsafe)

# Features

* Working with both `Fn() -> Result` and `Future` (optional via default
  `futures-support` feature).
* Backoff strategies: `constant`, `exponential`, `equal_jittered`, `full_jittered`
* Failure detection policies: `consecutive_failures`, `success_rate_over_time_window`
* Minimum rust version: 1.45

# Usage

Add this to your Cargo.toml:

```toml
failsafe = "1.1.0"
```

# Example

Using default backoff strategy and failure accrual policy.

```rust
use failsafe::{Config, CircuitBreaker, Error};

// A function that sometimes failed.
fn dangerous_call() -> Result<(), ()> {
  if thread_rng().gen_range(0, 2) == 0 {
    return Err(())
  }
  Ok(())
}

// Create a circuit breaker which configured by reasonable default backoff and
// failure accrual policy.
let circuit_breaker = Config::new().build();

// Call the function in a loop, after some iterations the circuit breaker will
// be in a open state and reject next calls.
for n in 0..100 {
  match circuit_breaker.call(|| dangerous_call()) {
    Err(Error::Inner(_)) => {
      eprintln!("{}: fail", n);
    },
    Err(Error::Rejected) => {
       eprintln!("{}: rejected", n);
       break;
    },
    _ => {}
  }
}
```

Or configure custom backoff and policy:

```rust
use std::time::Duration;
use failsafe::{backoff, failure_policy, CircuitBreaker};

// Create an exponential growth backoff which starts from 10s and ends with 60s.
let backoff = backoff::exponential(Duration::from_secs(10), Duration::from_secs(60));

// Create a policy which failed when three consecutive failures were made.
let policy = failure_policy::consecutive_failures(3, backoff);

// Creates a circuit breaker with given policy.
let circuit_breaker = Config::new()
  .failure_policy(policy)
  .build();
```

