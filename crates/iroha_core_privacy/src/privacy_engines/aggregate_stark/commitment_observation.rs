//! Test-only operation windows under one explicitly delegated commitment parent.
//!
//! The seven labels describe joined coordinator operations, including their
//! synchronous Rayon work. They do not measure individual worker CPU. A context
//! never reads a thread-local or current session: an inactive or ended parent
//! cannot adopt an unrelated observation. Repeated batches and tiles aggregate
//! under the same operation, without row, column, or task labels.

use iroha_measurement::{PhaseGuard, PhaseParent};

/// Closed semantic inventory for the original retained row commitment.
#[derive(Clone, Copy)]
pub(crate) enum OperationV1 {
    /// Allocate and initialize the original row hash streams.
    StreamSetup,
    /// Validate the complete column batch before changing hash state.
    ColumnAdmission,
    /// Absorb an admitted batch and publish the joined result.
    RowAbsorption,
    /// Admit finalization and create the retained cut and accumulator.
    FinishPreparation,
    /// Finalize the original leaf streams in one bounded tile.
    LeafFinalization,
    /// Reduce and append a completed digest tile.
    MerkleReduction,
    /// Finish the accumulator and bind the retained root.
    RootBinding,
}

impl OperationV1 {
    const fn label(self) -> &'static str {
        match self {
            Self::StreamSetup => "RowCommitmentStreamSetup",
            Self::ColumnAdmission => "RowCommitmentColumnAdmission",
            Self::RowAbsorption => "RowCommitmentRowAbsorption",
            Self::FinishPreparation => "RowCommitmentFinishPreparation",
            Self::LeafFinalization => "RowCommitmentLeafFinalization",
            Self::MerkleReduction => "RowCommitmentMerkleReduction",
            Self::RootBinding => "RowCommitmentRootBinding",
        }
    }
}

/// Exact original parent, or an inactive context for an unobserved invocation.
#[derive(Clone, Default)]
pub(crate) struct ContextV1 {
    parent: Option<PhaseParent>,
}

impl ContextV1 {
    /// Delegate only the supplied parent; never discover an ambient recorder.
    pub(crate) fn from_parent(parent: Option<PhaseParent>) -> Self {
        Self { parent }
    }

    /// Open one semantic operation on this thread under the original parent.
    pub(crate) fn enter(&self, operation: OperationV1) -> OperationGuardV1 {
        OperationGuardV1(
            self.parent
                .as_ref()
                .map(|parent| parent.enter(operation.label())),
        )
    }
}

/// Thread-bound operation guard preserving unfinished and unwound outcomes.
pub(crate) struct OperationGuardV1(Option<PhaseGuard>);

impl OperationGuardV1 {
    /// Mark successful completion after the original work and its joins finish.
    pub(crate) fn complete(self) {
        if let Some(guard) = self.0 {
            guard.complete();
        }
    }
}
