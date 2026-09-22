//! Original State acquisition custody before executing-block materialization.
//!
//! World is the existing inventory-owned aggregate. The four runtime Cells use
//! their original native acquisition slots, retained here during callee unwind.

use super::*;
use mv::{BlockAcquisition, BlockMode, BlockRetirement};

// One declaration generates pending/completed Cell phases and their retirement.
// No second World inventory or reconstructed payload generation is introduced.
macro_rules! runtime_cells {
    ($($field:ident: $value:ty),+ $(,)?) => {
        struct PendingCells<'state> {
            $($field: Option<mv::cell::BlockAcquisitionSlot<'state, $value>>,)+
        }

        struct AcquiredCells<'state> {
            $($field: Option<CellBlock<'state, $value>>,)+
        }

        enum CellPhase<'state> {
            Empty,
            Pending(PendingCells<'state>),
            Acquired(AcquiredCells<'state>),
        }

        impl<'state> CellPhase<'state> {
            fn new(state: &'state State) -> Self {
                Self::Pending(PendingCells {
                    $($field: Some(state.$field.block_acquisition()),)+
                })
            }

            fn initialize(&mut self, mode: BlockMode) {
                let Self::Pending(pending) = self else {
                    panic!("original pending State Cells");
                };
                $(pending.$field.as_mut().expect("original State Cell slot").initialize(mode);)+
                // All native/user work completed while the enclosing State owner
                // retained these slots. These transfers only move checked originals.
                let Self::Pending(mut pending) = std::mem::replace(self, Self::Empty) else {
                    unreachable!("original initialized State Cell slots");
                };
                *self = Self::Acquired(AcquiredCells {
                    $($field: Some(pending.$field.take().expect("original State Cell slot").into_block()),)+
                });
            }

            fn release(&mut self) {
                match self {
                    Self::Empty => {},
                    Self::Pending(pending) => {
                        $(if let Some(field) = pending.$field.as_mut() { field.release(); })+
                    }
                    Self::Acquired(acquired) => {
                        $(if let Some(field) = acquired.$field.as_mut() { field.release_writers(); })+
                    }
                }
            }
        }

        /// Exact completed originals; consuming these fields ends joint custody.
        pub(in crate::state) struct AcquiredRuntimeBlockFields<'state> {
            pub(in crate::state) world: WorldBlock<'state>,
            pub(in crate::state) transactions: TransactionsBlock<'state>,
            $(pub(in crate::state) $field: CellBlock<'state, $value>,)+
            pub(in crate::state) projection: CanonicalRuntimeProjection,
            pub(in crate::state) sccp_registry: Arc<ValidatedSccpRegistryV1>,
            pub(in crate::state) block_hashes: BlockHashesBlock<'state>,
            pub(in crate::state) da_rewind_releases: Option<da_hydration::DaRewindReleases<'state>>,
        }

        impl AcquiredRuntimeBlockFields<'_> {
            fn release(&mut self) {
                self.world.release_writers();
                self.transactions.release_writers();
                $(self.$field.release_writers();)+
            }
        }

        impl<'state> RuntimeBlockAcquisition<'state> {
            pub(super) fn finish(
                &mut self,
                projection: &mut Option<CanonicalRuntimeProjection>,
                sccp_registry: &mut Option<Arc<ValidatedSccpRegistryV1>>,
            ) -> AcquiredRuntimeBlock<'state> {
                // Borrow the caller's pending owner and metadata. A by-value
                // receiver adds another complete World temporary to the native
                // acquisition frame, even before the final handoff is reached.
                let original = self;
                assert!(original.complete, "original State acquisition did not complete");
                let CellPhase::Acquired(cells) = &original.cells else {
                    unreachable!("original acquired State Cells");
                };
                assert!(original.world.is_some() && original.transactions.is_some());
                assert!(original.block_hashes.is_some());
                $(assert!(cells.$field.is_some());)+
                assert!(projection.is_some() && sccp_registry.is_some());
                // All checks precede any extraction. A second finish remains
                // terminal, and the outlined closure borrows every original.
                original.complete = false;
                finish_runtime_acquisition(|| {
                    let CellPhase::Acquired(cells) = &mut original.cells else {
                        unreachable!("original acquired State Cells");
                    };
                    AcquiredRuntimeBlock {
                        target: original.target,
                        fields: Some(AcquiredRuntimeBlockFields {
                            world: original.world.take().expect("original State World"),
                            transactions: original.transactions.take().expect("original State membership"),
                            $($field: cells.$field.take().expect("original State Cell"),)+
                            projection: projection.take().expect("original prepared runtime projection"),
                            sccp_registry: sccp_registry.take().expect("original prepared SCCP registry"),
                            block_hashes: original.block_hashes.take().expect("original funded hash successor"),
                            da_rewind_releases: None,
                        }),
                    }
                })
            }
        }
    };
}

runtime_cells! {
    commit_topology: Vec<PeerId>,
    prev_commit_topology: Vec<PeerId>,
    lane_consensus_contexts: LaneConsensusContextsV1,
    canonical_runtime: SnapshotNexusRuntime,
}

/// Complete original acquisition, armed throughout State input preparation.
pub(in crate::state) struct AcquiredRuntimeBlock<'state> {
    target: &'state State,
    fields: Option<AcquiredRuntimeBlockFields<'state>>,
}

impl<'state> AcquiredRuntimeBlock<'state> {
    /// Borrow the exact acquired originals and their checked projections.
    pub(in crate::state) fn fields(&self) -> &AcquiredRuntimeBlockFields<'state> {
        self.fields
            .as_ref()
            .expect("original acquired State fields")
    }

    /// Retain the actual rewind notices before entering its fallible engine.
    pub(in crate::state) fn rewind_da_indexes_to_height(
        &mut self,
        target_height: u64,
    ) -> Result<(), da_hydration::DaIndexHydrationError> {
        let fields = self
            .fields
            .as_mut()
            .expect("original acquired State fields");
        let releases = fields
            .da_rewind_releases
            .get_or_insert_with(|| da_hydration::DaRewindReleases::new(self.target));
        self.target
            .rewind_da_indexes_to_height_with_releases(target_height, releases)
    }

    /// Transfer only after every fallible constructor input is prepared.
    /// The caller must immediately install these originals in the executing owner.
    pub(in crate::state) fn into_fields(mut self) -> AcquiredRuntimeBlockFields<'state> {
        self.fields.take().expect("original acquired State fields")
    }
}

impl Drop for AcquiredRuntimeBlock<'_> {
    fn drop(&mut self) {
        if let Some(fields) = self.fields.as_mut() {
            fields.release();
        }
    }
}

// Slot acquisition and completed result temporaries must not share a deep
// native constructor frame. The closure borrows the original aggregate.
#[inline(never)]
fn finish_runtime_acquisition<'state>(
    finish: impl FnOnce() -> AcquiredRuntimeBlock<'state>,
) -> AcquiredRuntimeBlock<'state> {
    finish()
}

/// Partial original State ownership; installed before any later Cell initializes.
pub(super) struct RuntimeBlockAcquisition<'state> {
    world: Option<WorldBlock<'state>>,
    transactions: Option<TransactionsBlock<'state>>,
    cells: CellPhase<'state>,
    block_hashes: Option<BlockHashesBlock<'state>>,
    target: &'state State,
    started: bool,
    complete: bool,
}

impl<'state> RuntimeBlockAcquisition<'state> {
    /// Inert slots and the already detached original funded successor only.
    pub(super) fn new(target: &'state State, block_hashes: BlockHashesBlock<'state>) -> Self {
        Self {
            world: None,
            transactions: None,
            cells: CellPhase::new(target),
            block_hashes: Some(block_hashes),
            target,
            started: false,
            complete: false,
        }
    }

    pub(super) fn initialize(&mut self, replacement: bool) {
        assert!(!self.started, "original State acquisition is one-shot");
        self.started = true;
        self.world = Some(if replacement {
            self.target.world.block_and_revert()
        } else {
            self.target.world.block()
        });
        self.transactions = Some(if replacement {
            self.target.transactions.block_and_revert()
        } else {
            self.target.transactions.block()
        });
        self.cells.initialize(if replacement {
            BlockMode::Replace
        } else {
            BlockMode::Ordinary
        });
        self.complete = true;
    }

    pub(super) fn world(&self) -> &WorldBlock<'state> {
        self.world.as_ref().expect("original acquired World")
    }

    pub(super) fn world_mut(&mut self) -> &mut WorldBlock<'state> {
        self.world.as_mut().expect("original acquired World")
    }

    pub(super) fn canonical_runtime(&self) -> &CellBlock<'state, SnapshotNexusRuntime> {
        let CellPhase::Acquired(cells) = &self.cells else {
            panic!("original acquired runtime Cell");
        };
        cells
            .canonical_runtime
            .as_ref()
            .expect("original canonical runtime")
    }

    fn release(&mut self) {
        self.complete = false;
        if let Some(world) = self.world.as_mut() {
            world.release_writers();
        }
        if let Some(transactions) = self.transactions.as_mut() {
            transactions.release_writers();
        }
        self.cells.release();
    }
}

impl Drop for RuntimeBlockAcquisition<'_> {
    fn drop(&mut self) {
        self.release();
    }
}
