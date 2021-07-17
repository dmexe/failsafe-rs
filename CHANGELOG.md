### 1.1.0

Fixes:
* fix `FullJittered` implementation: the exponential behavior was not being applied

Updates:
* `pin_project` and `rand` has been updated to the latest version

Breaking changes:
* minimum rust version is 1.42.0

### 1.0.0

Breaking changes:
* use rust 2018 edition
* use `std::future::Future` and `futures==0.3`, supporting `async`/`await`
* minimum rust version is 1.39.0

Improvements:
* drop `spin` dependency, use `parking_lot`
* add `futures-support` feature, to allow opt-out for `futures` support

### 0.3.1

Fixes:
* add explicitly `dyn` definition to trait objects

### 0.3.0

Breaking changes:
* remove `instrument::NoopInstrument`, use `()` instead.
* added optional feature `parking_lot_mutex` when it exists the crate `parking_lot`
  would be using for `Mutex` instead of the default `spin`.

### 0.2.0

Breaking changes:
* `success_rate` policy now accepts `min_request_threshold`.
* the `CircuitBreaker` turned into a trait which implements `call` and `call_with` methods.
* the trait `Callable` was removed

Improvements:
* remove `tokio-timer` dependency.
* use spin lock instead `std::sync::Mutex`

