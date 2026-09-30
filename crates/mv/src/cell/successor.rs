//! Exact original-pool identity allocation before a Cell's physical acquisition.

use super::NextPublication;
use iroha_allocation::{AllocationBudget, AllocationCharge};
use std::{alloc::Layout, fmt};

/// One move-only prepaid identity for an original Cell publication.
///
/// Allocate the complete aggregate's token inventory before acquiring any field
/// writer. Each token moves through its original acquisition, block, capture and
/// publication; it is never replaced on Busy. The actual shared allocation holds
/// its original charge until the last identity reference is destroyed.
///
/// This funds only that identity's concrete control allocation. It grants no
/// State/finality authority and does not fund Cell EBR generations, nested values,
/// initial owner/version identity, release notifications or World metadata.
#[must_use = "retain the original successor through acquisition and publication"]
pub struct CellPublicationSuccessor {
    next: NextPublication,
    source: AllocationBudget,
}

/// Local refusal before an original successor identity can be allocated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellPublicationSuccessorError {
    /// The charge belongs to a different pool, regardless of equal limits.
    ForeignPool,
    /// Both size and alignment must match the exact retained shared allocation.
    LayoutMismatch {
        /// Concrete identity layout required by the existing publication engine.
        expected: Layout,
        /// Unchanged layout owned by the refused original charge.
        actual: Layout,
    },
    /// The physical allocator refused the already admitted exact layout.
    Allocator {
        /// The exact requested backing layout.
        layout: Layout,
    },
}

impl fmt::Display for CellPublicationSuccessorError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignPool => output.write_str("Cell successor charge belongs to another pool"),
            Self::LayoutMismatch { expected, actual } => write!(
                output,
                "Cell successor requires {expected:?}, original charge covers {actual:?}"
            ),
            Self::Allocator { layout } => write!(
                output,
                "allocator refused admitted Cell successor layout {layout:?}"
            ),
        }
    }
}
impl std::error::Error for CellPublicationSuccessorError {}

impl CellPublicationSuccessor {
    /// Exact requested shared identity allocation, including its original charge.
    pub fn allocation_layout() -> Layout {
        NextPublication::allocation_layout()
    }

    /// Consume one exact prepaid charge without reserving or substituting capacity.
    ///
    /// On every refusal, including physical allocator failure, the same unchanged
    /// charge is returned. There are no field writers in this operation; the
    /// aggregate must complete every required token before physical acquisition.
    ///
    /// # Errors
    /// Returns wrong original source, exact-layout mismatch or allocator refusal.
    pub fn try_from_charge(
        budget: &AllocationBudget,
        charge: AllocationCharge,
    ) -> Result<Self, (AllocationCharge, CellPublicationSuccessorError)> {
        if !charge.belongs_to(budget) {
            return Err((charge, CellPublicationSuccessorError::ForeignPool));
        }
        let expected = Self::allocation_layout();
        let actual = charge.layout();
        if expected != actual {
            return Err((
                charge,
                CellPublicationSuccessorError::LayoutMismatch { expected, actual },
            ));
        }
        match NextPublication::try_from_charge(charge) {
            Ok(next) => Ok(Self {
                next,
                source: budget.clone(),
            }),
            Err((charge, error)) => Err((
                charge,
                CellPublicationSuccessorError::Allocator {
                    layout: error.layout(),
                },
            )),
        }
    }

    /// Compare the exact original pool without creating capacity or a new token.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.source.same_pool(budget)
    }

    pub(super) fn into_original(self) -> NextPublication {
        self.next
    }
}
