use std::error::Error as StdError;
use std::fmt::{self, Display};

/// A `CircuitBreaker`'s error.
#[derive(Debug, PartialEq)]
pub enum Error<E> {
    /// An error from inner call.
    Inner(E),
    /// An error when call was rejected.
    Rejected,
}

impl<E> Display for Error<E>
where
    E: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Rejected => write!(f, "call was rejected"),
            Error::Inner(err) => write!(f, "{}", err),
        }
    }
}

impl<E> StdError for Error<E>
where
    E: StdError + 'static,
{
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Inner(ref err) => Some(err),
            _ => None,
        }
    }
}
