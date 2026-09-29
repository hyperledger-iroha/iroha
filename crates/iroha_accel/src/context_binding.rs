//! Synchronous origin-context binding with restoration armed before switching.

/// Private native adapter. No asynchronous ownership or native handle creation.
pub trait ContextDriver {
    type Handle: Copy;
    type Error;
    fn current(&self) -> Result<Self::Handle, Self::Error>;
    fn set_current(&self, handle: Self::Handle) -> Result<(), Self::Error>;
}

/// Preserve thread context even when a failing switch has changed it already.
/// No allocation occurs, and error/unwind cleanup conservatively quarantines.
pub fn with_context<D: ContextDriver, T>(
    driver: &D,
    target: D::Handle,
    quarantine: impl Fn(),
    body: impl FnOnce() -> Result<T, D::Error>,
) -> Result<T, D::Error> {
    struct Restore<'a, D: ContextDriver, Q: Fn()> {
        driver: &'a D,
        previous: D::Handle,
        quarantine: &'a Q,
        armed: bool,
    }
    impl<D: ContextDriver, Q: Fn()> Drop for Restore<'_, D, Q> {
        fn drop(&mut self) {
            if self.armed {
                (self.quarantine)();
                // Failure cannot authorize reclamation; quarantine is already set.
                let _ = self.driver.set_current(self.previous);
            }
        }
    }
    let previous = driver.current().inspect_err(|_| quarantine())?;
    let mut restore = Restore {
        driver,
        previous,
        quarantine: &quarantine,
        armed: true,
    };
    driver.set_current(target)?;
    let result = body();
    let restored = driver.set_current(previous);
    restore.armed = false;
    if restored.is_err() {
        quarantine();
    }
    restored?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
    };

    struct Driver {
        current: Cell<u32>,
        fail_read: bool,
        fail_target: Option<u32>,
        writes: RefCell<Vec<u32>>,
    }
    impl ContextDriver for Driver {
        type Handle = u32;
        type Error = &'static str;
        fn current(&self) -> Result<u32, &'static str> {
            if self.fail_read {
                Err("read")
            } else {
                Ok(self.current.get())
            }
        }
        fn set_current(&self, handle: u32) -> Result<(), &'static str> {
            // Deliberately mutate first: an error is not proof of no side effect.
            self.current.set(handle);
            self.writes.borrow_mut().push(handle);
            if self.fail_target == Some(handle) {
                Err("switch")
            } else {
                Ok(())
            }
        }
    }
    fn driver() -> Driver {
        Driver {
            current: Cell::new(7),
            fail_read: false,
            fail_target: None,
            writes: RefCell::new(Vec::new()),
        }
    }

    #[test]
    fn successful_call_restores_the_original_context() {
        let driver = driver();
        let quarantined = Cell::new(false);
        let result = with_context(
            &driver,
            9,
            || quarantined.set(true),
            || {
                assert_eq!(driver.current.get(), 9);
                Ok(42)
            },
        );
        assert_eq!(result, Ok(42));
        assert_eq!(driver.current.get(), 7);
        assert!(!quarantined.get());
        assert_eq!(*driver.writes.borrow(), [9, 7]);
    }

    #[test]
    fn failed_switch_restores_even_when_the_failed_call_mutated_context() {
        let mut driver = driver();
        driver.fail_target = Some(9);
        let quarantined = Cell::new(false);
        let result: Result<(), _> = with_context(
            &driver,
            9,
            || quarantined.set(true),
            || panic!("body after failed switch"),
        );
        assert_eq!(result, Err("switch"));
        assert_eq!(driver.current.get(), 7);
        assert!(quarantined.get());
        assert_eq!(*driver.writes.borrow(), [9, 7]);
    }

    #[test]
    fn failed_read_never_uses_an_unknown_previous_context() {
        let mut driver = driver();
        driver.fail_read = true;
        let quarantined = Cell::new(false);
        let result: Result<(), _> = with_context(
            &driver,
            9,
            || quarantined.set(true),
            || panic!("body after failed read"),
        );
        assert_eq!(result, Err("read"));
        assert!(driver.writes.borrow().is_empty());
        assert!(quarantined.get());
    }

    #[test]
    fn body_error_is_returned_after_successful_restoration() {
        let driver = driver();
        let quarantined = Cell::new(false);
        let result: Result<(), _> =
            with_context(&driver, 9, || quarantined.set(true), || Err("body"));
        assert_eq!(result, Err("body"));
        assert_eq!(driver.current.get(), 7);
        assert!(!quarantined.get());
    }

    #[test]
    fn restoration_failure_quarantines_and_does_not_publish_body_result() {
        let mut driver = driver();
        driver.fail_target = Some(7);
        let quarantined = Cell::new(false);
        let result = with_context(&driver, 9, || quarantined.set(true), || Ok(42));
        assert_eq!(result, Err("switch"));
        assert!(quarantined.get());
        assert_eq!(*driver.writes.borrow(), [9, 7]);
    }

    #[test]
    fn body_unwind_restores_and_quarantines() {
        let driver = driver();
        let quarantined = Cell::new(false);
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_context(
                &driver,
                9,
                || quarantined.set(true),
                || -> Result<(), &'static str> { panic!("body") },
            )
        }));
        assert!(result.is_err());
        assert_eq!(driver.current.get(), 7);
        assert!(quarantined.get());
    }
}
