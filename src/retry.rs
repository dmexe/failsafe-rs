use super::error::Error;

pub struct Retry {
    max_attempts: u32
}

impl Retry
{
    fn call<F, E, R>(&self, mut f: F) -> Result<R, Error<E>>
        where
            F: FnMut() -> Result<R, E>,
    {
        for _ in 0..(self.max_attempts) {
            match f() {
                Ok(ok) => {
                    return Ok(ok);
                }
                Err(_) => {}
            }
        }
        Err(Error::Rejected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_success() {
        let retry = new_retry();
        let closure = || -> Result<i32, ()>{
            Ok(0)
        };
        assert_eq!(0, retry.call(closure).expect("Expect success"));
    }

    #[test]
    fn call_retry() {
        let retry = new_retry();
        let mut counter: u32 = 0;
        let closure = || {
            if counter == 2 {
                return Ok(2);
            } else {
                counter += 1;
                Err(())
            }
        };
        assert_eq!(2, retry.call(closure).expect("Expect success"));
    }

    #[test]
    fn call_retry_exceeds_max() {
        let retry = new_retry();
        let mut counter: u32 = 0;
        let closure = || {
            if counter == 5 {
                return Ok(5);
            } else {
                counter += 1;
                Err("fail")
            }
        };
        assert_eq!(Error::Rejected, retry.call(closure).expect_err("Expected error"));
    }


    fn new_retry() -> Retry {
        Retry { max_attempts: 3 }
    }
}
