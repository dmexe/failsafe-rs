use std::time::{Duration, Instant};

use super::clock;

/// Time windowed counter.
#[derive(Debug)]
pub struct WindowedAdder {
    window: u64,
    slices: Vec<i64>,
    index: usize,
    elapsed: Instant,
}

impl WindowedAdder {
    /// Creates a new counter.
    ///
    /// * `window` - The range of time to be kept in the counter.
    /// * `slices` - The number of slices that are maintained; a higher number of slices
    ///   means finer granularity but also more memory consumption. Must be more than 1 and
    ///   less then 10.
    ///
    /// # Panics
    ///
    /// * When `slices` isn't in range [1;10].
    pub fn new(window: Duration, slices: u8) -> Self {
        assert!(slices <= 10);
        assert!(slices > 1);

        let window = window.millis() / u64::from(slices);

        Self {
            window,
            slices: vec![0; slices as usize],
            index: 0,
            elapsed: clock::now(),
        }
    }

    /// Purge outdated slices.
    pub fn expire(&mut self) {
        let now = clock::now();
        let time_diff = (now - self.elapsed).millis();

        if time_diff < self.window {
            return;
        }

        let len = self.slices.len();
        let mut idx = (self.index + 1) % len;

        let n_skip = ((time_diff / self.window) - 1).min(len as u64);
        if n_skip > 0 {
            let r = n_skip.min((len - idx) as u64);
            self.zero_slices(idx, idx + r as usize);
            self.zero_slices(0usize, (n_skip - r) as usize);
            //println!("zero {}-{} {}-{}", idx, idx + r as usize, 0, n_skip - r);
            idx = (idx + n_skip as usize) % len;
        }

        self.slices[idx] = 0;
        self.index = idx;
        self.elapsed = now;

        //println!("inc {} vec={:?}", idx, self.slices);
    }

    /// Resets state of the counter.
    pub fn reset(&mut self) {
        self.slices.iter_mut().for_each(|it| *it = 0);
        self.elapsed = clock::now();
    }

    /// Increments counter by `value`.
    pub fn add(&mut self, value: i64) {
        self.expire();
        self.slices[self.index] += value;
        //println!("add {} {:?}", value, self.slices);
    }

    /// Returns the current sum of the counter.
    pub fn sum(&mut self) -> i64 {
        self.expire();
        self.slices.iter().sum()
    }

    /// Writes zero into slices starting `from` and ending `to`.
    fn zero_slices(&mut self, from: usize, to: usize) {
        self.slices
            .iter_mut()
            .take(to)
            .skip(from)
            .for_each(|it| *it = 0);
    }
}

/// `Duration::as_millis` is unstable at the current(1.28) rust version, so it returns milliseconds
/// in given duration.
trait Millis {
    fn millis(&self) -> u64;
}

impl Millis for Duration {
    fn millis(&self) -> u64 {
        const MILLIS_PER_SEC: u64 = 1_000;
        (self.as_secs() * MILLIS_PER_SEC) + u64::from(self.subsec_millis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sum_when_time_stands_still() {
        clock::freeze(|_| {
            let mut adder = new_windowed_adder();

            adder.add(1);
            assert_eq!(1, adder.sum());
            adder.add(1);
            assert_eq!(2, adder.sum());
            adder.add(3);
            assert_eq!(5, adder.sum());
        });
    }

    #[test]
    fn sliding_over_small_window() {
        clock::freeze(|time| {
            let mut adder = new_windowed_adder();

            adder.add(1);
            assert_eq!(1, adder.sum());

            time.advance(1.seconds());
            assert_eq!(1, adder.sum());

            adder.add(2);
            assert_eq!(3, adder.sum());

            time.advance(1.seconds());
            assert_eq!(3, adder.sum());

            time.advance(1.seconds());
            assert_eq!(2, adder.sum());

            time.advance(1.seconds());
            assert_eq!(0, adder.sum());
        })
    }

    #[test]
    fn sliding_over_large_window() {
        clock::freeze(|time| {
            let mut adder = WindowedAdder::new(20.seconds(), 10);

            for i in 0..21 {
                adder.add(i % 3);
                time.advance(1.seconds());
            }

            assert_eq!(20, adder.sum());

            time.advance(1.seconds());
            assert_eq!(18, adder.sum());

            time.advance(1.seconds());
            assert_eq!(18, adder.sum());

            time.advance(5.seconds());
            assert_eq!(12, adder.sum());
            adder.add(1);

            time.advance(10.seconds());
            assert_eq!(3, adder.sum());
        })
    }

    #[test]
    fn sliding_window_when_slices_are_skipped() {
        clock::freeze(|time| {
            let mut adder = new_windowed_adder();

            adder.add(1);
            assert_eq!(1, adder.sum());

            time.advance(1.seconds());
            adder.add(2);
            assert_eq!(3, adder.sum());

            time.advance(1.seconds());
            adder.add(1);
            assert_eq!(4, adder.sum());

            time.advance(2.seconds());
            assert_eq!(1, adder.sum());

            time.advance(100.seconds());
            assert_eq!(0, adder.sum());

            adder.add(100);
            time.advance(1.seconds());
            assert_eq!(100, adder.sum());

            adder.add(100);
            time.advance(1.seconds());

            adder.add(100);
            assert_eq!(300, adder.sum());

            time.advance(100.seconds());
            assert_eq!(0, adder.sum());
        })
    }

    #[test]
    fn negative_sums() {
        clock::freeze(|time| {
            let mut adder = new_windowed_adder();

            // net: 2
            adder.add(-2);
            assert_eq!(-2, adder.sum());

            adder.add(4);
            assert_eq!(2, adder.sum());

            // net: -4
            time.advance(1.seconds());
            adder.add(-2);
            assert_eq!(0, adder.sum());

            adder.add(-2);
            assert_eq!(-2, adder.sum());

            // net: -2
            time.advance(1.seconds());
            adder.add(-2);
            assert_eq!(-4, adder.sum());

            time.advance(1.seconds());
            assert_eq!(-6, adder.sum());

            time.advance(1.seconds());
            assert_eq!(-2, adder.sum());

            time.advance(1.seconds());
            assert_eq!(0, adder.sum());

            time.advance(100.seconds());
            assert_eq!(0, adder.sum());
        });
    }

    fn new_windowed_adder() -> WindowedAdder {
        WindowedAdder::new(3.seconds(), 3)
    }

    trait IntoDuration {
        fn seconds(self) -> Duration;
    }

    impl IntoDuration for u64 {
        fn seconds(self) -> Duration {
            Duration::from_secs(self)
        }
    }
}
