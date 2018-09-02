/// Evaluates if an error should be recorded as a failure and thus increase the failure rate.
pub trait FailurePredicate<E> {
    /// Must return `true` if the error should count as a failure, otherwise it must return `false`.
    fn is_err(&self, err: E) -> bool;
}

impl<E, F> FailurePredicate<E> for F
where
    F: Fn(E) -> bool,
{
    fn is_err(&self, err: E) -> bool {
        self(err)
    }
}

/// Classify all error kinds as failures.
struct All;

impl<E> FailurePredicate<E> for All {
    fn is_err(&self, _: E) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_func_as_failure_predicate() {
        fn is_err(err: bool) -> bool {
            err
        }

        assert!(FailurePredicate::is_err(&is_err, true));
    }

    #[test]
    fn all_is_all() {
        assert!(All.is_err(()))
    }
}
