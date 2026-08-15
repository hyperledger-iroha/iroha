//! Provides [`TimeSource`], a mockable abstraction over [`std::time::SystemTime`].
//!
//! Callers can use real time, capture one immutable instant for deterministic
//! work, or substitute a manually controlled clock via [`MockTimeHandle`].
use parking_lot::Mutex;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
#[derive(Debug, Clone, Default)]
enum TimeSourceInner {
    /// The time will come from the system clock ([`std::time::SystemTime::now()`]
    #[default]
    SystemTime,
    /// The time comes from a captured or manually controlled instant.
    ControlledTime(Arc<Mutex<Duration>>),
}
/// A time source backed by the system clock, one fixed instant, or a manually controlled clock.
#[derive(Debug, Clone, Default)]
pub struct TimeSource(TimeSourceInner);
impl TimeSource {
    /// Creates a real [`TimeSource`] backed by [`std::time::SystemTime::now()`]
    pub fn new_system() -> Self {
        Self(TimeSourceInner::SystemTime)
    }
    /// Creates a fixed time source that always reports `unix_time`.
    ///
    /// This is useful when one logical operation must evaluate every step at
    /// the same instant, such as deterministic validation of an atomic replay
    /// range. Unlike [`Self::new_mock`], no mutation handle is returned.
    #[must_use]
    pub fn new_fixed(unix_time: Duration) -> Self {
        Self(TimeSourceInner::ControlledTime(Arc::new(Mutex::new(
            unix_time,
        ))))
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
            TimeSourceInner::ControlledTime(time) => SystemTime::UNIX_EPOCH + *time.lock(),
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
            TimeSourceInner::ControlledTime(time) => *time.lock(),
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
        TimeSource(TimeSourceInner::ControlledTime(self.0.clone()))
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
    fn fixed_source_reports_the_captured_instant() {
        let captured = Duration::from_millis(9_876_543);
        let source = TimeSource::new_fixed(captured);
        assert_eq!(source.now(), captured);
        assert_eq!(source.get_unix_time(), captured);
        assert_eq!(source.get_system_time(), SystemTime::UNIX_EPOCH + captured);
        assert_eq!(source.now(), captured);
    }
    #[test]
    fn mock_source_reports_start_time_as_unix_and_system_time() {
        let start = Duration::from_millis(1_234_567);
        let (handle, source) = TimeSource::new_mock(start);
        assert_eq!(source.now(), start);
        assert_eq!(source.get_unix_time(), start);
        assert_eq!(source.get_system_time(), SystemTime::UNIX_EPOCH + start);
        handle.set(Duration::from_secs(42));
        assert_eq!(
            source.get_system_time(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(42)
        );
    }
    #[test]
    fn cloned_mock_handles_and_sources_share_time_state() {
        let handle = MockTimeHandle::new(Duration::from_secs(10));
        let cloned_handle = handle.clone();
        let source = handle.source();
        let cloned_source = source.clone();
        cloned_handle.set(Duration::from_secs(20));
        assert_eq!(source.now(), Duration::from_secs(20));
        handle.advance(Duration::from_millis(250));
        assert_eq!(cloned_source.now(), Duration::from_millis(20_250));
        cloned_handle.rewind(Duration::from_secs(5));
        assert_eq!(source.get_unix_time(), Duration::from_millis(15_250));
    }
    #[test]
    fn system_source_reports_time_within_call_window() {
        let source = TimeSource::new_system();
        let before = SystemTime::now();
        let observed = source.get_system_time();
        let after = SystemTime::now();
        assert!(observed >= before);
        assert!(observed <= after);
    }
    #[test]
    fn default_source_uses_system_unix_time() {
        let source = TimeSource::default();
        let before = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("current time should be after unix epoch");
        let observed = source.now();
        let after = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("current time should be after unix epoch");
        assert!(observed >= before);
        assert!(observed <= after);
    }
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
