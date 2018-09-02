use std::iter::Iterator;
use std::time::Duration;

use super::backoff::{self, Backoff};
use super::failure_predicate::{self, FailurePredicate};
use super::failure_accrual::{self, FailureAccrualPolicy};
use super::state_machine::Instrumentation;

pub struct Builder<B, P, A, I> {
  backoff: Option<B>,
  failure_predicate: Option<P>,
  failure_accrual_policy: Option<A>,
  instrumentation: Option<I>
}

impl<B, P, A, I> Builder<B, P, A, I> {
  pub fn new() -> Self {
    Self {
      backoff: None,
      failure_predicate: None,
      failure_accrual_policy: None,
      instrumentation: None
    }
  }

  pub fn backoff<T>(self, backoff: T) -> Builder<T, P, A, I> {
    Builder {
      backoff: Some(backoff),
      failure_predicate: self.failure_predicate,
      failure_accrual_policy: self.failure_accrual_policy,
      instrumentation: self.instrumentation
    }
  }

  pub fn failure_predicate<T>(self, failure_predicate: T) -> Builder<B, T, A, I> {
    Builder {
      backoff: self.backoff,
      failure_predicate: Some(failure_predicate),
      failure_accrual_policy: self.failure_accrual_policy,
      instrumentation: self.instrumentation
    }
  }

  pub fn failure_accrual_policy<T>(self, failure_accrual_policy: T) -> Builder<B, P, T, I> {
    Builder {
      backoff: self.backoff,
      failure_predicate: self.failure_predicate,
      failure_accrual_policy: Some(failure_accrual_policy),
      instrumentation: self.instrumentation
    }
  }

  pub fn instrumentation<T>(self, instrumentation: T) -> Builder<B, P, A, T> {
    Builder {
      backoff: self.backoff,
      failure_predicate: self.failure_predicate,
      failure_accrual_policy: self.failure_accrual_policy,
      instrumentation: Some(instrumentation),
    }
  }
}