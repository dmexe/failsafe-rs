### 0.2.0

Breaking changes:
* `success_rate` policy now accepts `min_request_threshold`.
* the `CircuitBreaker` turned into a trait which implements `call` and `call_with` methods.
* the trait `Callable` was removed

Improvements:
* remove `tokio-timer` dependency.
* use spin lock instead `std::sync::Mutex`