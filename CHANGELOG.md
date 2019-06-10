### Unreleased

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

