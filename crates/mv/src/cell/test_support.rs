//! Read-only physical ownership diagnostics for cross-crate custody tests.

use super::*;

/// Independent observation of one original writer, without reading its payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalWriterProbe {
    /// The actual original physical writer was acquired without blocking.
    pub acquired: bool,
    /// That original writer was already poisoned when observed.
    pub poisoned: bool,
}

impl<V: Value, C: Send + Sync + 'static> Cell<V, C> {
    /// Probe Undo and Current independently, even when either writer is poisoned.
    ///
    /// This test diagnostic acquires only the original raw mutex guards. It does
    /// not clone values, allocate generations, notify release waiters, clear
    /// poison, or grant publication authority. Both guards are retained until
    /// both observations are made. A guard acquired during an existing unwind
    /// is also dropped within that unwind; the probe does not introduce a new
    /// panic while either guard is held.
    pub fn probe_original_writers_for_testing(&self) -> [PhysicalWriterProbe; 2] {
        let undo = self.revert.try_acquire_writer();
        let current = self.blocks.try_acquire_writer();
        let result = [
            PhysicalWriterProbe {
                acquired: undo.is_some(),
                poisoned: self.revert.is_poisoned(),
            },
            PhysicalWriterProbe {
                acquired: current.is_some(),
                poisoned: self.blocks.is_poisoned(),
            },
        ];
        drop((undo, current));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn physical_probe_distinguishes_poison_from_each_held_original_writer() {
        let cell = Cell::new(7_u64);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _undo = cell.revert.try_acquire_writer().unwrap();
                panic!("poison only the original Undo writer");
            }))
            .is_err()
        );
        let current = cell.blocks.try_acquire_writer().unwrap();
        assert_eq!(
            cell.probe_original_writers_for_testing(),
            [
                PhysicalWriterProbe {
                    acquired: true,
                    poisoned: true
                },
                PhysicalWriterProbe {
                    acquired: false,
                    poisoned: false
                },
            ]
        );
        drop(current);
        let undo = cell.revert.try_acquire_writer().unwrap();
        assert_eq!(
            cell.probe_original_writers_for_testing(),
            [
                PhysicalWriterProbe {
                    acquired: false,
                    poisoned: true
                },
                PhysicalWriterProbe {
                    acquired: true,
                    poisoned: false
                },
            ]
        );
        drop(undo);
        assert_eq!(*cell.view().get(), 7);
        assert!(cell.revert.is_poisoned());
        assert!(!cell.blocks.is_poisoned());
    }

    #[test]
    fn physical_probe_during_unwind_preserves_healthy_and_poisoned_originals() {
        struct DuringUnwind<'a> {
            cell: &'a Cell<u64>,
            poisoned: bool,
            observed: &'a std::cell::Cell<bool>,
        }
        impl Drop for DuringUnwind<'_> {
            fn drop(&mut self) {
                assert!(std::thread::panicking());
                assert_eq!(
                    self.cell.probe_original_writers_for_testing(),
                    [PhysicalWriterProbe {
                        acquired: true,
                        poisoned: self.poisoned
                    }; 2]
                );
                assert_eq!(self.cell.revert.is_poisoned(), self.poisoned);
                assert_eq!(self.cell.blocks.is_poisoned(), self.poisoned);
                self.observed.set(true);
            }
        }
        for poisoned in [false, true] {
            let cell = Cell::new(7_u64);
            if poisoned {
                assert!(
                    catch_unwind(AssertUnwindSafe(|| {
                        let _undo = cell.revert.try_acquire_writer().unwrap();
                        let _current = cell.blocks.try_acquire_writer().unwrap();
                        panic!("poison the original pair");
                    }))
                    .is_err()
                );
            }
            let observed = std::cell::Cell::new(false);
            let error = catch_unwind(AssertUnwindSafe(|| {
                let _probe = DuringUnwind {
                    cell: &cell,
                    poisoned,
                    observed: &observed,
                };
                panic!("original owner unwind");
            }))
            .unwrap_err();
            assert_eq!(error.downcast_ref::<&str>(), Some(&"original owner unwind"));
            assert!(observed.get());
            assert_eq!(cell.revert.is_poisoned(), poisoned);
            assert_eq!(cell.blocks.is_poisoned(), poisoned);
            assert_eq!(*cell.view().get(), 7);
        }
    }
}
