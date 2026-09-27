//! Read-only current source assignments for an independently administered resolver.
//!
//! A qualified assignment is one observation of the daemon's committed State/Kura archive
//! head. It carries no admission, advert, revocation, endpoint, credential, grant, or signing
//! authority. A resolver must join an independently authenticated governance snapshot to this
//! exact head and repeat all currentness checks before fetch, after fetch, and at reader EOF.

use super::{ArchivedProviderIngestFinalizedLedgerV1, ProviderIngestCurrentAssignmentSnapshotV1};
use iroha_core::query::provider_ingest_finalized::{
    ProviderIngestFinalizedArchiveErrorV1, ProviderIngestFinalizedArchiveKeyV1,
    ProviderIngestFinalizedArchiveV1,
};
use iroha_data_model::NetworkId;
use sorafs_node::{
    FinalizedProviderIngestAuthorizationV1, ProviderIngestFinalizedCursorV1,
    ProviderIngestFinalizedLedgerErrorV1, provider_ingest_runtime::ProviderIngestSourceRequestV1,
};

/// Check that the immutable job cursor is represented on the current exact archive chain.
///
/// The caller must first select a visible committed key from the Kura-qualified archive, then
/// recheck that head and the archive generation after this read. Missing compacted history,
/// substituted hashes, and future anchors cannot become source authority.
pub(super) fn authenticate_retained_admission_ancestor(
    archive: &ProviderIngestFinalizedArchiveV1,
    network_id: NetworkId,
    current_key: &ProviderIngestFinalizedArchiveKeyV1,
    authorization: &FinalizedProviderIngestAuthorizationV1,
) -> Result<(), ProviderIngestFinalizedLedgerErrorV1> {
    if authorization.validate().is_err() || current_key.network_id != network_id {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    let admission = authorization.admission_finalized_cursor();
    if admission.height > current_key.height
        || (admission.height == current_key.height
            && admission.block_hash != current_key.block_hash)
    {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    let resolve = |height, block_hash| {
        archive
            .resolve_exact_key(&network_id, height, block_hash)
            .map_err(|error| match error {
                ProviderIngestFinalizedArchiveErrorV1::InvalidKey { .. }
                | ProviderIngestFinalizedArchiveErrorV1::BelowActivationFloor { .. }
                | ProviderIngestFinalizedArchiveErrorV1::BelowRetentionFloor { .. }
                | ProviderIngestFinalizedArchiveErrorV1::UnknownExactAnchor { .. }
                | ProviderIngestFinalizedArchiveErrorV1::FinalizedFork { .. } => {
                    ProviderIngestFinalizedLedgerErrorV1::Rejected
                }
                _ => ProviderIngestFinalizedLedgerErrorV1::Unavailable,
            })
    };
    if resolve(current_key.height, current_key.block_hash)? != *current_key {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    resolve(admission.height, admission.block_hash)?;
    Ok(())
}

/// One canonical source request observed at an exact committed archive head.
///
/// The request is payload-free. This value is read-only evidence for a separate resolver,
/// not a durable authorization or a grant to fetch, sign, or complete an order.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct ProviderIngestCurrentSourceAssignmentV1 {
    network_id: NetworkId,
    source_provider_id: [u8; 32],
    finalized_head: ProviderIngestFinalizedCursorV1,
    finalized_at_unix_ms: u64,
    provider_state_root: [u8; 32],
    assignment_revision: u64,
    canonical_request: ProviderIngestSourceRequestV1,
}

impl ProviderIngestCurrentSourceAssignmentV1 {
    /// Exact genesis-derived network selected by the daemon reader.
    #[must_use]
    pub const fn network_id(&self) -> NetworkId {
        self.network_id
    }

    /// Source whose current membership was checked against the canonical inventory.
    #[must_use]
    pub const fn source_provider_id(&self) -> [u8; 32] {
        self.source_provider_id
    }

    /// Exact visible committed State/Kura head bound to this observation.
    #[must_use]
    pub const fn finalized_head(&self) -> ProviderIngestFinalizedCursorV1 {
        self.finalized_head
    }

    /// Archive's finalized timestamp for the exact committed head.
    #[must_use]
    pub const fn finalized_at_unix_ms(&self) -> u64 {
        self.finalized_at_unix_ms
    }

    /// Root of the destination provider projection at the exact head.
    #[must_use]
    pub const fn provider_state_root(&self) -> [u8; 32] {
        self.provider_state_root
    }

    /// Current nonzero revision of the pending order's canonical assignment.
    #[must_use]
    pub const fn assignment_revision(&self) -> u64 {
        self.assignment_revision
    }

    /// Canonical request whose authorization, source inventory, and Musubi binding were checked.
    #[must_use]
    pub const fn canonical_request(&self) -> &ProviderIngestSourceRequestV1 {
        &self.canonical_request
    }

    /// Sorted exact source-provider inventory derived from the current canonical order.
    #[must_use]
    pub fn source_provider_ids(&self) -> &[[u8; 32]] {
        self.canonical_request.source_provider_ids()
    }
}

/// Convert an authenticated archive lookup into the resolver's read-only projection.
///
/// The worker-supplied request is accepted only when it is exactly the canonical request
/// derivable from the current assignment. Its Musubi binding remains informational transport
/// data and cannot create the worker's private finalized claim.
pub(super) fn materialize_current_source_assignment(
    network_id: NetworkId,
    source_provider_id: [u8; 32],
    request: &ProviderIngestSourceRequestV1,
    expected_assignment_revision: u64,
    snapshot: ProviderIngestCurrentAssignmentSnapshotV1,
) -> Result<ProviderIngestCurrentSourceAssignmentV1, ProviderIngestFinalizedLedgerErrorV1> {
    if snapshot.key.network_id != network_id
        || !snapshot.matches_source_request(
            source_provider_id,
            request,
            expected_assignment_revision,
        )
    {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    let canonical_request = ProviderIngestSourceRequestV1::new(
        request.authorization().clone(),
        snapshot.source_provider_ids,
        request.musubi_archive().cloned(),
    )
    .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    if &canonical_request != request {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    Ok(ProviderIngestCurrentSourceAssignmentV1 {
        network_id,
        source_provider_id,
        finalized_head: ProviderIngestFinalizedCursorV1 {
            height: snapshot.key.height,
            block_hash: snapshot.key.block_hash,
        },
        finalized_at_unix_ms: snapshot.key.finalized_at_unix_ms,
        provider_state_root: snapshot.provider_state_root,
        assignment_revision: snapshot.assignment.expected_assignment_revision,
        canonical_request,
    })
}

impl ArchivedProviderIngestFinalizedLedgerV1 {
    /// Read one current source assignment for a separately administered resolver.
    ///
    /// This directly requalifies the archive against Kura and selects the visible committed
    /// State head. It does not consume the worker's scan cursor, create an opaque claim, or issue
    /// a transport grant. The caller's old admission cursor is resolved as a retained ancestor of
    /// that head. Current council admission, advert, revocation, and governed transport pins still
    /// require separate authenticated producers before the resolver may issue a grant.
    ///
    /// # Errors
    /// Rejects an invalid or stale source, revision, request, or network. Missing or unstable
    /// finalized State/Kura/archive authority is unavailable rather than silently accepted.
    pub fn read_current_source_assignment_for_resolver(
        &self,
        network_id: NetworkId,
        source_provider_id: [u8; 32],
        request: &ProviderIngestSourceRequestV1,
        expected_assignment_revision: u64,
    ) -> Result<ProviderIngestCurrentSourceAssignmentV1, ProviderIngestFinalizedLedgerErrorV1> {
        if expected_assignment_revision == 0 {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
        }
        let snapshot = self.lookup_current_assignment(
            network_id,
            source_provider_id,
            request.authorization(),
        )?;
        materialize_current_source_assignment(
            network_id,
            source_provider_id,
            request,
            expected_assignment_revision,
            snapshot,
        )
    }

    /// Recheck that an earlier read is still identical at the visible committed head.
    ///
    /// A successful recheck has no lease lifetime; the separate resolver must repeat it at
    /// each fetch/use boundary and independently recheck its governance and transport inputs.
    ///
    /// # Errors
    /// Rejects any changed head, provider root, source inventory, or assignment revision.
    pub fn recheck_current_source_assignment_for_resolver(
        &self,
        expected: &ProviderIngestCurrentSourceAssignmentV1,
    ) -> Result<(), ProviderIngestFinalizedLedgerErrorV1> {
        let current = self.read_current_source_assignment_for_resolver(
            expected.network_id,
            expected.source_provider_id,
            &expected.canonical_request,
            expected.assignment_revision,
        )?;
        if &current != expected {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
        }
        Ok(())
    }
}

#[cfg(test)]
mod stream_token_custody_tests {
    //! Adversarial joins between current source assignments and native signer control.
    use super::super::current_source_stream_token_custody::join_current_source_stream_token_custody;
    use super::*;
    include!("current_source_stream_token_custody_tests.rs");
}
