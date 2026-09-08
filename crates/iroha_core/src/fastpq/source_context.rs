//! Transactional capture of FASTPQ transcript execution context.
//!
//! These records preserve local execution provenance only. They are not finality
//! proofs, qualified compact statements or spend-authority tokens.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_data_model::{
    fastpq::{FastpqSourceExecutionKindV1, FastpqSourceLaneV1, FastpqSourceStatementContextV1},
    nexus::{DataSpaceId, LaneId},
};
use thiserror::Error;

pub use iroha_data_model::fastpq::FastpqSourceRouteV1 as FastpqCapturedSourceRoute;

/// Captured local context for one finalized transcript-map key.
///
/// Only execution can construct this record. It still provides no consensus
/// finality or remote authority. Complete entry inventory and statement binding
/// must be established separately before creating a source-manifest attestation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastpqCapturedTranscriptSource {
    source: FastpqSourceStatementContextV1,
    entry_hash: Hash,
    route: FastpqCapturedSourceRoute,
    dataspace_id: DataSpaceId,
    first_fragment_index: u64,
    execution_kind: FastpqSourceExecutionKindV1,
}

impl FastpqCapturedTranscriptSource {
    /// Network and height of the local execution scope.
    pub const fn source(&self) -> FastpqSourceStatementContextV1 {
        self.source
    }
    /// Exact key used by the finalized transcript accumulator.
    pub const fn entry_hash(&self) -> Hash {
        self.entry_hash
    }
    /// Captured runtime route, with an explicit absence of lane context.
    pub const fn route(&self) -> FastpqCapturedSourceRoute {
        self.route
    }
    /// Effective runtime dataspace, including the normal universal default.
    pub const fn dataspace_id(&self) -> DataSpaceId {
        self.dataspace_id
    }
    /// First applied state-fragment index which contributed to this transcript key.
    /// Batched calls may share this index; it is not a complete source-entry order.
    pub const fn first_fragment_index(&self) -> u64 {
        self.first_fragment_index
    }
    /// Whether the key came from typed native protocol purpose rather than an execution call.
    pub const fn is_protocol_purpose(&self) -> bool {
        matches!(
            self.execution_kind,
            FastpqSourceExecutionKindV1::ProtocolPurpose
        )
    }
    /// Exact execution kind retained for canonical source-statement construction.
    pub const fn execution_kind(&self) -> FastpqSourceExecutionKindV1 {
        self.execution_kind
    }
}

/// Failure while retaining the local source context of a transfer transcript.
/// Errors remain latched for the block scope; callers never receive a partial map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum FastpqSourceCaptureError {
    /// An explicit runtime lane has no active incarnation at the source height.
    #[error(
        "FASTPQ source lane {lane_id} for transcript {entry_hash} has no frozen active incarnation"
    )]
    MissingLaneIncarnation {
        /// Explicit execution lane which failed resolution.
        lane_id: LaneId,
        /// Transcript key for which the source lane could not be resolved.
        entry_hash: Hash,
    },
    /// Transcript recording used a hash different from the active execution identity.
    #[error("FASTPQ transcript hash differs from its active execution call")]
    ExecutionIdentityMismatch,
    /// One transcript key was reused with conflicting source, route or identity kind.
    #[error("FASTPQ transcript key {entry_hash} has conflicting execution source context")]
    ConflictingSource {
        /// Conflicting transcript-map key.
        entry_hash: Hash,
    },
    /// A local execution-fragment index cannot fit the canonical scalar.
    #[error("FASTPQ source execution-fragment index exceeds u64")]
    FragmentIndexOutOfRange,
    /// Final source access was requested before the local capture boundary was sealed.
    #[error("FASTPQ source capture has not been sealed")]
    CaptureNotSealed,
    /// A healthy local capture boundary was already sealed and cannot be sealed again.
    #[error("FASTPQ source capture has already been sealed")]
    CaptureAlreadySealed,
    /// Transcript capture was applied after the local source inventory was sealed.
    #[error("FASTPQ transcript capture was applied after source sealing")]
    AppliedAfterSeal,
}

/// Immutable height context shared by every transaction of one block scope.
#[derive(Clone, Debug)]
pub(crate) struct FastpqBlockStartSourceContext {
    pub(crate) source: FastpqSourceStatementContextV1,
    pub(crate) lane_incarnations: BTreeMap<LaneId, Option<Hash>>,
}

impl FastpqBlockStartSourceContext {
    pub(crate) fn capture_transcript(
        &self,
        call_hash: Option<Hash>,
        batch_hash: Hash,
        lane_id: Option<LaneId>,
        dataspace_id: Option<DataSpaceId>,
        fragment_index: usize,
    ) -> Result<FastpqCapturedTranscriptSource, FastpqSourceCaptureError> {
        if call_hash.is_some_and(|call| call != batch_hash) {
            return Err(FastpqSourceCaptureError::ExecutionIdentityMismatch);
        }
        let dataspace_id = dataspace_id.unwrap_or(DataSpaceId::UNIVERSAL);
        let route = match lane_id {
            None => FastpqCapturedSourceRoute::Unrouted,
            Some(lane_id) => FastpqCapturedSourceRoute::Lane(FastpqSourceLaneV1 {
                lane_id,
                lane_incarnation: self
                    .lane_incarnations
                    .get(&lane_id)
                    .copied()
                    .flatten()
                    .ok_or(FastpqSourceCaptureError::MissingLaneIncarnation {
                        lane_id,
                        entry_hash: batch_hash,
                    })?,
            }),
        };
        Ok(FastpqCapturedTranscriptSource {
            source: self.source,
            entry_hash: batch_hash,
            route,
            dataspace_id,
            first_fragment_index: u64::try_from(fragment_index)
                .map_err(|_| FastpqSourceCaptureError::FragmentIndexOutOfRange)?,
            execution_kind: if call_hash.is_none() {
                FastpqSourceExecutionKindV1::ProtocolPurpose
            } else {
                FastpqSourceExecutionKindV1::ExecutionCall
            },
        })
    }
}

/// Rollback-local accumulator with sticky failure and a one-shot final capture boundary.
///
/// Transaction-local accumulators start open and are merged only when applied. Sealing the
/// block accumulator rejects every later applied occurrence, even an identical existing-key
/// capture. Final access requires a healthy seal; ordinary inspection remains available while
/// constructing the inventory. Neither accessor grants consensus finality or remote authority.
#[derive(Clone, Debug, Default)]
pub(crate) struct FastpqSourceCaptureAccumulator {
    entries: BTreeMap<Hash, FastpqCapturedTranscriptSource>,
    error: Option<FastpqSourceCaptureError>,
    sealed: bool,
}

impl FastpqSourceCaptureAccumulator {
    /// Seal a healthy capture boundary once, preserving any existing sticky failure.
    /// A repeated seal is an error but does not invalidate an already healthy sealed map.
    pub(crate) fn seal(&mut self) -> Result<(), FastpqSourceCaptureError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.sealed {
            return Err(FastpqSourceCaptureError::CaptureAlreadySealed);
        }
        self.sealed = true;
        Ok(())
    }

    /// Borrow the complete final map only after successful sealing and without a later failure.
    pub(crate) fn sealed_sources(
        &self,
    ) -> Result<&BTreeMap<Hash, FastpqCapturedTranscriptSource>, FastpqSourceCaptureError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if !self.sealed {
            return Err(FastpqSourceCaptureError::CaptureNotSealed);
        }
        Ok(&self.entries)
    }

    /// Record one local occurrence, latching any failure or attempted post-seal application.
    pub(crate) fn record(
        &mut self,
        captured: Result<FastpqCapturedTranscriptSource, FastpqSourceCaptureError>,
    ) {
        if self.error.is_some() {
            return;
        }
        if self.sealed {
            self.entries.clear();
            self.error = Some(FastpqSourceCaptureError::AppliedAfterSeal);
            return;
        }
        let captured = match captured {
            Ok(captured) => captured,
            Err(error) => {
                self.entries.clear();
                self.error = Some(error);
                return;
            }
        };
        if let Some(previous) = self.entries.get_mut(&captured.entry_hash) {
            if previous.source != captured.source
                || previous.route != captured.route
                || previous.dataspace_id != captured.dataspace_id
                || previous.execution_kind != captured.execution_kind
            {
                self.entries.clear();
                self.error = Some(FastpqSourceCaptureError::ConflictingSource {
                    entry_hash: captured.entry_hash,
                });
                return;
            }
            previous.first_fragment_index = previous
                .first_fragment_index
                .min(captured.first_fragment_index);
        } else {
            self.entries.insert(captured.entry_hash, captured);
        }
    }

    /// Apply another local capture scope without propagating its sealed-state bit.
    /// Healthy empty scopes are inert; any nonempty or failed scope applied after sealing
    /// invalidates the destination even if it would introduce no new map keys.
    pub(crate) fn merge(&mut self, other: Self) {
        if self.error.is_some() {
            return;
        }
        if self.sealed {
            if other.error.is_some() || !other.entries.is_empty() {
                self.record(Err(FastpqSourceCaptureError::AppliedAfterSeal));
            }
            return;
        }
        if let Some(error) = other.error {
            self.record(Err(error));
        } else {
            for entry in other.entries.into_values() {
                self.record(Ok(entry));
            }
        }
    }

    /// Inspect a healthy map during inventory construction, without requiring its final seal.
    pub(crate) fn sources(
        &self,
    ) -> Result<&BTreeMap<Hash, FastpqCapturedTranscriptSource>, &FastpqSourceCaptureError> {
        match &self.error {
            Some(error) => Err(error),
            None => Ok(&self.entries),
        }
    }
}

#[cfg(test)]
mod tests;
