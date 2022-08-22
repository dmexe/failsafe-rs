### [Unreleased]

### [1.2.0] - 2022-08-22

Added:
* the `reset` method to the `StateMachine`, (thanks to https://github.com/eg-fxia)

Breaking changes:
* minimum rust version is 1.49

Updates:
* `pin_project` has been updated to `0.12`
* `criterion` has been updated to `0.3.6`


### [1.1.0] - 2021-09-18

Fixes:
* fix `FullJittered` implementation: the exponential behavior was not being applied

Updates:
* `pin_project` and `rand` has been updated to the latest version

Breaking changes:
* minimum rust version is 1.45.0

### [1.0.0] - 2020-08-17

Breaking changes:
* use rust 2018 edition
* use `std::future::Future` and `futures==0.3`, supporting `async`/`await`
* minimum rust version is 1.39.0

Improvements:
* drop `spin` dependency, use `parking_lot`
* add `futures-support` feature, to allow opt-out for `futures` support

### [0.3.1] - 2019-06-10

Fixes:
* add explicitly `dyn` definition to trait objects

### [0.3.0] - 2018-10-26

Breaking changes:
* remove `instrument::NoopInstrument`, use `()` instead.
* added optional feature `parking_lot_mutex` when it exists the crate `parking_lot`
  would be using for `Mutex` instead of the default `spin`.

### [0.2.0] - 2018-09-10

Breaking changes:
* `success_rate` policy now accepts `min_request_threshold`.
* the `CircuitBreaker` turned into a trait which implements `call` and `call_with` methods.
* the trait `Callable` was removed

Improvements:
* remove `tokio-timer` dependency.
* use spin lock instead `std::sync::Mutex`

[Unreleased]: https://github.com/dmexe/failsafe-rs/compare/v1.2.0...master
[1.2.0]: https://github.com/dmexe/failsafe-rs/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/dmexe/failsafe-rs/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/dmexe/failsafe-rs/compare/v0.3.1...v1.0.0
[0.3.1]: https://github.com/dmexe/failsafe-rs/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/dmexe/failsafe-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/dmexe/failsafe-rs/releases/tag/v0.2.0
