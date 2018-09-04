use std::cell::Cell;
use std::time::{Duration, Instant};

thread_local!(static CLOCK: Cell<Option<*const MockClock>> = Cell::new(None));

#[derive(Debug)]
pub struct MockClock(Instant);

impl MockClock {
    fn new() -> MockClock {
        MockClock(Instant::now())
    }

    pub fn now(&self) -> Instant {
        self.0
    }

    pub fn advance(&mut self, diff: Duration) {
        self.0 += diff
    }
}

pub fn freeze<F, R>(f: F) -> R
where
    F: FnOnce(&mut MockClock) -> R,
{
    CLOCK.with(|cell| {
        let mut clock = MockClock::new();

        assert!(
            cell.get().is_none(),
            "default clock already set for execution context"
        );

        // Ensure that the clock is removed from the thread-local context
        // when leaving the scope. This handles cases that involve panicking.
        struct Reset<'a>(&'a Cell<Option<*const MockClock>>);

        impl<'a> Drop for Reset<'a> {
            fn drop(&mut self) {
                self.0.set(None);
            }
        }

        let _reset = Reset(cell);

        cell.set(Some(&clock as *const MockClock));

        f(&mut clock)
    })
}

pub fn now() -> Instant {
    CLOCK.with(|current| match current.get() {
        Some(ptr) => unsafe { (*ptr).now() },
        None => Instant::now(),
    })
}
