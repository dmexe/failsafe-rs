//! calls CircuitBreaker in a Stream that can be polled with `next()`
use std::task;

use futures_core::Stream;
use pin_project::pin_project;

use crate::{failure_predicate, FailurePolicy, FailurePredicate, StateMachine};

/// Stream that holds `StateMachine` and calls stream future
#[pin_project]
#[derive(Debug, Clone)]
pub struct BreakerStream<S, P, Pol, Ins> {
    breaker: StateMachine<Pol, Ins>,
    #[pin]
    stream: S,
    predicate: P,
}

impl<T, E, S, Pol, Ins> BreakerStream<S, failure_predicate::Any, Pol, Ins>
where
    S: Stream<Item = Result<T, E>>,
{
    /// create new circuit breaker stream
    pub fn new(breaker: StateMachine<Pol, Ins>, stream: S) -> Self {
        Self {
            breaker,
            stream,
            predicate: crate::failure_predicate::Any,
        }
    }
}

impl<T, E, S, P, Pol, Ins> BreakerStream<S, P, Pol, Ins>
where
    S: Stream<Item = Result<T, E>>,
    P: FailurePredicate<E>,
{
    /// create new circuit breaker with predicate
    pub fn new_with(breaker: StateMachine<Pol, Ins>, stream: S, predicate: P) -> Self {
        Self {
            breaker,
            stream,
            predicate,
        }
    }
    /// return a reference to the underlying state machine
    pub fn state_machine(&self) -> &StateMachine<Pol, Ins> {
        &self.breaker
    }
}

impl<T, E, S, P, Pol, Ins> Stream for BreakerStream<S, P, Pol, Ins>
where
    S: Stream<Item = Result<T, E>>,
    P: FailurePredicate<E>,
    Pol: FailurePolicy,
    Ins: crate::Instrument,
{
    type Item = Result<T, crate::Error<E>>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        use task::Poll;
        let this = self.project();
        if !this.breaker.is_call_permitted() {
            return Poll::Ready(Some(Err(crate::Error::Rejected)));
        }

        match this.stream.poll_next(cx) {
            Poll::Ready(Some(Ok(ok))) => {
                this.breaker.on_success();
                Poll::Ready(Some(Ok(ok)))
            }
            Poll::Ready(Some(Err(err))) => {
                if this.predicate.is_err(&err) {
                    this.breaker.on_error();
                } else {
                    this.breaker.on_success();
                }
                Poll::Ready(Some(Err(crate::Error::Inner(err))))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::StreamExt;

    use crate::{backoff, failure_policy, Config};

    use super::*;

    #[tokio::test]
    async fn call_ok() {
        let circuit_breaker = new_circuit_breaker(Duration::from_secs(5));
        let stream = BreakerStream::new(
            circuit_breaker,
            futures::stream::once(async { delay_for(Duration::from_millis(100)).await }),
        );
        tokio::pin!(stream);
        while let Some(_x) = stream.next().await {}

        assert!(stream.state_machine().is_call_permitted());
    }

    #[tokio::test]
    async fn call_err() {
        let stream = BreakerStream::new(
            new_circuit_breaker(Duration::from_millis(100)),
            futures::stream::iter(vec![Err::<(), ()>(()), Ok(())]),
        );
        tokio::pin!(stream);
        match stream.next().await {
            Some(Err(crate::Error::Inner(_))) => {}
            err => unreachable!("{:?}", err),
        }
        assert!(!stream.state_machine().is_call_permitted());

        match stream.next().await {
            Some(Err(crate::Error::Rejected)) => {}
            err => unreachable!("{:?}", err),
        }
        assert!(!stream.state_machine().is_call_permitted());
        tokio::time::sleep(Duration::from_millis(200)).await;
        // permitted now
        assert!(stream.state_machine().is_call_permitted());
        match stream.next().await {
            Some(Ok(())) => {}
            err => unreachable!("{:?}", err),
        }
    }

    fn new_circuit_breaker(
        duration: Duration,
    ) -> StateMachine<failure_policy::ConsecutiveFailures<std::iter::Repeat<Duration>>, ()> {
        let backoff = backoff::constant(duration);
        let policy = failure_policy::consecutive_failures(1, backoff);
        Config::new().failure_policy(policy).build()
    }

    async fn delay_for(duration: Duration) -> Result<(), ()> {
        tokio::time::sleep(duration).await;
        Ok(())
    }
}
