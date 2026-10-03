//! Funded, move-only original scan cells with allocation-lifetime custody.

use super::{F, packet, return_copyback};
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};

/// Candidate private fields, never trusted execution facts.
pub(in super::super) struct Cell {
    pub(super) row: [F; return_copyback::WIDTH],
    pub(super) child: [F; packet::WIDTH],
    pub(super) copyback: [F; packet::WIDTH],
}
impl Cell {
    fn clear(&mut self) {
        for value in self
            .row
            .iter_mut()
            .chain(&mut self.child)
            .chain(&mut self.copyback)
        {
            value.zeroize_v1();
        }
    }
}

impl Drop for Cell {
    fn drop(&mut self) {
        self.clear();
    }
}

#[derive(Debug)]
pub(in super::super) enum Error {
    Allocation(ChargedBufferError),
    Length,
}

/// No cloning or capacity growth; the final backing owner releases its charge.
pub(in super::super) struct Scan {
    cells: ChargedBuffer<Cell>,
}
impl Scan {
    pub(super) const BYTES: usize = return_copyback::CELLS * core::mem::size_of::<Cell>();
    pub(super) fn candidate(
        budget: &AllocationBudget,
        cells: impl IntoIterator<Item = Cell>,
    ) -> Result<Self, Error> {
        let mut backing =
            ChargedBuffer::new(return_copyback::CELLS, budget).map_err(Error::Allocation)?;
        let mut cells = cells.into_iter();
        for _ in 0..return_copyback::CELLS {
            backing.push_reserved(cells.next().ok_or(Error::Length)?);
        }
        if cells.next().is_some() {
            return Err(Error::Length);
        }
        Ok(Self { cells: backing })
    }
    pub(super) fn get(&self, index: usize) -> &Cell {
        &self.cells.as_slice()[index]
    }
    #[cfg(test)]
    pub(super) fn get_mut(&mut self, index: usize) -> &mut Cell {
        &mut self.cells.as_mut_slice()[index]
    }
    #[cfg(test)]
    pub(super) fn swap(&mut self, left: usize, right: usize) {
        self.cells.as_mut_slice().swap(left, right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cell() -> Cell {
        Cell {
            row: [F::ONE; return_copyback::WIDTH],
            child: [F::ONE; packet::WIDTH],
            copyback: [F::ONE; packet::WIDTH],
        }
    }
    #[test]
    fn exact_funding_rejection_and_final_owner_refund() {
        assert_eq!(Scan::BYTES, 11_012_736);
        let small = AllocationBudget::new(Scan::BYTES - 1);
        let mut touched = false;
        let result = Scan::candidate(
            &small,
            (0..return_copyback::CELLS).map(|_| {
                touched = true;
                cell()
            }),
        );
        assert!(matches!(result, Err(Error::Allocation(_))));
        assert!(!touched);
        assert_eq!(small.reserved_bytes(), 0);
        let budget = AllocationBudget::new(Scan::BYTES);
        let scan = Scan::candidate(&budget, (0..return_copyback::CELLS).map(|_| cell())).unwrap();
        assert_eq!(budget.reserved_bytes(), Scan::BYTES);
        assert_eq!(scan.get(return_copyback::CELLS - 1).child[0], F::ONE);
        drop(scan);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    #[test]
    fn malformed_length_and_partial_unwind_refund_original_backing() {
        let budget = AllocationBudget::new(Scan::BYTES);
        for count in [return_copyback::CELLS - 1, return_copyback::CELLS + 1] {
            assert!(matches!(
                Scan::candidate(&budget, (0..count).map(|_| cell())),
                Err(Error::Length)
            ));
            assert_eq!(budget.reserved_bytes(), 0);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = Scan::candidate(
                &budget,
                (0..return_copyback::CELLS).map(|index| {
                    assert_ne!(index, 7, "candidate construction unwind");
                    cell()
                }),
            );
        }));
        assert!(result.is_err());
        assert_eq!(budget.reserved_bytes(), 0);
        let mut value = cell();
        // Exercise the exact cleanup implementation invoked by Drop.
        value.clear();
        assert!(
            value
                .row
                .iter()
                .chain(&value.child)
                .chain(&value.copyback)
                .all(|field| *field == F::ZERO)
        );
    }
    #[test]
    fn construction_uses_bounded_stack_for_the_full_scan() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let budget = AllocationBudget::new(Scan::BYTES);
                let scan =
                    Scan::candidate(&budget, (0..return_copyback::CELLS).map(|_| cell())).unwrap();
                assert_eq!(budget.reserved_bytes(), Scan::BYTES);
                drop(scan);
                assert_eq!(budget.reserved_bytes(), 0);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
