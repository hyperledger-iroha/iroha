//! Provides [`TimeSource`], a mockable abstraction over [`std::time::SystemTime`].
//!
//! Real time is used in production, while tests can substitute a manual
//! clock via [`MockTimeHandle`].

use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use parking_lot::Mutex;

#[derive(Debug, Clone, Default)]
enum TimeSourceInner {
    /// The time will come from the system clock ([`std::time::SystemTime::now()`]
    #[default]
    SystemTime,
    /// The time will come from the mock implementation
    MockTime(Arc<Mutex<Duration>>),
}

/// A time source that either relies on [`std::time::SystemTime::now()`] or uses a mock clock that must be advanced manually
#[derive(Debug, Clone, Default)]
pub struct TimeSource(TimeSourceInner);

impl TimeSource {
    /// Creates a real [`TimeSource`] backed by [`std::time::SystemTime::now()`]
    pub fn new_system() -> Self {
        Self(TimeSourceInner::SystemTime)
    }

    /// Creates a mock [`TimeSource`] that must be advanced manually via
    /// [`MockTimeHandle`].
    pub fn new_mock(start_unix_time: Duration) -> (MockTimeHandle, Self) {
        let handle = MockTimeHandle::new(start_unix_time);

        let source = handle.source();
        (handle, source)
    }

    /// Returns the [`SystemTime`] corresponding to "now".
    ///
    /// It can either come from [`SystemTime::now()`] or from a mock time source
    pub fn get_system_time(&self) -> SystemTime {
        match &self.0 {
            TimeSourceInner::SystemTime => SystemTime::now(),
            TimeSourceInner::MockTime(time) => SystemTime::UNIX_EPOCH + *time.lock(),
        }
    }

    /// Returns the duration since unix epoch corresponding to "now".
    ///
    /// It can either come from [`SystemTime::now()`] or from a mock time source
    pub fn get_unix_time(&self) -> Duration {
        match &self.0 {
            TimeSourceInner::SystemTime => SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("assuming that now is later than 1970/01/01"),
            TimeSourceInner::MockTime(time) => *time.lock(),
        }
    }

    /// Returns the duration since unix epoch corresponding to "now".
    #[inline]
    pub fn now(&self) -> Duration {
        self.get_unix_time()
    }
}

/// A handle that can be used to advance the mock [`TimeSource`].
#[derive(Clone)]
pub struct MockTimeHandle(Arc<Mutex<Duration>>);

impl MockTimeHandle {
    /// Creates a [`MockTimeHandle`] set to a specific unix timestamp.
    pub fn new(start_unix_time: Duration) -> Self {
        Self(Arc::new(Mutex::new(start_unix_time)))
    }

    /// Gets a [`TimeSource`] corresponding to this mock handle
    pub fn source(&self) -> TimeSource {
        TimeSource(TimeSourceInner::MockTime(self.0.clone()))
    }

    /// Sets the mock time to a specific unix timestamp.
    pub fn set(&self, unix_time: Duration) {
        let mut time = self.0.lock();
        *time = unix_time;
    }

    /// Moves the mock clock forward by `advance_time`.
    pub fn advance(&self, advance_time: Duration) {
        let mut time = self.0.lock();
        *time = time.saturating_add(advance_time);
    }

    /// Moves the mock clock backward by `advance_time`.
    pub fn rewind(&self, advance_time: Duration) {
        let mut time = self.0.lock();
        *time = time.saturating_sub(advance_time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewind_saturates_at_zero() {
        let handle = MockTimeHandle::new(Duration::from_secs(5));
        let source = handle.source();

        handle.rewind(Duration::from_secs(10));

        assert_eq!(source.now(), Duration::from_secs(0));
    }

    #[test]
    fn advance_saturates_at_max() {
        let start = Duration::MAX.saturating_sub(Duration::from_secs(1));
        let handle = MockTimeHandle::new(start);
        let source = handle.source();

        handle.advance(Duration::from_secs(5));

        assert_eq!(source.now(), Duration::MAX);
    }
}
