//! Explicit memory budgets for kernel scratch space.
//!
//! Kernels never size their scratch from global state or environment
//! variables. The caller passes a [`MemoryBudget`]; the kernel plans its window
//! size, task split and tables so that the live scratch it owns stays within
//! the budget, or fails with [`BudgetExceeded`] before allocating.
//!
//! TODO: charge these budgets to the workspace allocation accounting
//! (`iroha_allocation`) once the native prover links it; today the budget is a
//! plain byte count chosen by the caller.

/// A limit on the scratch bytes a kernel may hold at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryBudget {
    bytes: usize,
}

impl MemoryBudget {
    /// The default budget: 256 MiB.
    pub const DEFAULT: Self = Self::new(256 << 20);

    /// A budget of `bytes` bytes.
    pub const fn new(bytes: usize) -> Self {
        Self { bytes }
    }

    /// The budget in bytes.
    pub const fn bytes(self) -> usize {
        self.bytes
    }

    /// Returns `Ok(())` when `required` bytes fit the budget.
    ///
    /// # Errors
    ///
    /// [`BudgetExceeded`] when they do not.
    pub const fn check(self, required: usize) -> Result<(), BudgetExceeded> {
        if required <= self.bytes {
            Ok(())
        } else {
            Err(BudgetExceeded {
                required,
                budget: self.bytes,
            })
        }
    }
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A kernel could not plan its work within the memory budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BudgetExceeded {
    /// Bytes the smallest feasible plan needs.
    pub required: usize,
    /// Bytes the budget allows.
    pub budget: usize,
}

impl core::fmt::Display for BudgetExceeded {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "kernel needs {} bytes of scratch, budget is {}",
            self.required, self.budget
        )
    }
}

impl std::error::Error for BudgetExceeded {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_and_default() {
        let b = MemoryBudget::new(100);
        assert_eq!(b.bytes(), 100);
        assert!(b.check(100).is_ok());
        assert_eq!(
            b.check(101),
            Err(BudgetExceeded {
                required: 101,
                budget: 100
            })
        );
        assert_eq!(MemoryBudget::default(), MemoryBudget::DEFAULT);
        assert_eq!(
            BudgetExceeded {
                required: 2,
                budget: 1
            }
            .to_string(),
            "kernel needs 2 bytes of scratch, budget is 1"
        );
    }
}
