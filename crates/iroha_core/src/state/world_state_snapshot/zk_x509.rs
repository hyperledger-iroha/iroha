//! Owned trusted-node X.509 authority from one complete certified World cut.
//!
//! No caller code runs under the complete World writers. Selected records move
//! into prepaid immutable custody before the original generation recheck returns.

use super::*;
use iroha_allocation::RetainedPayload;
use iroha_core_privacy::privacy_state::{
    PrivacyCommitmentKeyV1, PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1, PrivacyRootKeyV1,
    PrivacyRootProvenanceV1, PrivacyStateItemRecordV1, PrivacyZkX509AuthoritativeStateV1,
    load_privacy_zk_x509_authoritative_state_v1,
};
use iroha_data_model::{
    NetworkId,
    block::consensus::{HeightContextId, SumeragiRootScope},
    privacy::{PrivacyConsensusPolicyV1, PrivacyIssuerIdV1, PrivacyPolicyIdV1},
};

/// Owned X.509 selection and fixed certified identity from one trusted node.
///
/// Construction is private. All World writers and snapshot scratch are released,
/// and the original generation rechecked, before this value is returned. Its
/// selected policy lists retain exact original-pool allocation custody. Consensus
/// policy and cut metadata are fixed-size copies with no heap allocation. It
/// retains no complete block, State view, writer, or publication guard.
///
/// This is authority at acquisition, not a promise that State cannot advance.
/// Reacquire before each holder phase, including release. Ledger time is not
/// wall-clock freshness. No clone, extraction, decoder or submission authority
/// is exposed; independently cloned borrowed records are caller-owned copies.
pub struct NativeZkX509StateV1 {
    authoritative: RetainedPayload<PrivacyZkX509AuthoritativeStateV1>,
    policy: PrivacyConsensusPolicyV1,
    network_id: NetworkId,
    height: u64,
    certified_block_id: HeightContextId,
    root_scope: SumeragiRootScope,
    world_state_root: Hash,
    schema_hash: Hash,
    ledger_timestamp_ms: u64,
}

impl std::fmt::Debug for NativeZkX509StateV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeZkX509StateV1")
            .field("network_id", &self.network_id)
            .field("height", &self.height)
            .field("certified_block_id", &self.certified_block_id)
            .field("ledger_timestamp_ms", &self.ledger_timestamp_ms)
            .finish_non_exhaustive()
    }
}

impl NativeZkX509StateV1 {
    /// Complete joined records selected at this original certified acquisition.
    #[must_use]
    pub fn authoritative_state(&self) -> &PrivacyZkX509AuthoritativeStateV1 {
        self.authoritative.get()
    }
    /// Exact committed limits, retention and pending tightening.
    #[must_use]
    pub const fn consensus_policy(&self) -> &PrivacyConsensusPolicyV1 {
        &self.policy
    }
    /// Authenticated network of the original certified cut.
    #[must_use]
    pub const fn network_id(&self) -> NetworkId {
        self.network_id
    }
    /// Exact height of the original certified cut.
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.height
    }
    /// Existing certified block identity binding its native hash and result.
    #[must_use]
    pub const fn certified_block_id(&self) -> HeightContextId {
        self.certified_block_id
    }
    /// Authenticated global or independent data space root scope.
    #[must_use]
    pub const fn root_scope(&self) -> SumeragiRootScope {
        self.root_scope
    }
    /// Complete World root authenticated by the original certified result.
    #[must_use]
    pub const fn world_state_root(&self) -> Hash {
        self.world_state_root
    }
    /// Actual native schema authenticated by that same World root.
    #[must_use]
    pub const fn schema_hash(&self) -> Hash {
        self.schema_hash
    }
    /// Certified ledger time, without any inference about current wall time.
    #[must_use]
    pub const fn ledger_timestamp_ms(&self) -> u64 {
        self.ledger_timestamp_ms
    }
}

impl State {
    /// Acquire owned X.509 selection after releasing all complete World writers.
    ///
    /// The existing finite snapshot capture authenticates the original certified
    /// cut and every commitment/root/head plus the exact policy cell. The existing
    /// semantic loader selects and validates all governance lineages; its temporary
    /// index remains bounded ordinary-allocation scratch. Only its selected result
    /// is retained, with both policy lists allocated under exact prepaid custody.
    /// Fixed consensus-policy and certified identity copies allocate nothing.
    ///
    /// No arbitrary caller callback is accepted. The private capture drops World
    /// writers and reconstruction scratch and rechecks generation before returning
    /// the owned result. Perform holder preparation/proving outside State guards;
    /// reacquire before proving and release, preserving the original statement.
    /// A quiet chain does not turn its ledger time into present wall-clock authority.
    ///
    /// # Errors
    /// Preserves State acquisition and retained-allocation refusal; rejects stale,
    /// foreign or restored-only cuts, incomplete/mismatched originals, invalid
    /// committed policy, or absent/revoked/malformed governance.
    pub fn acquire_native_zk_x509_state_v1(
        &self,
        tip: &CommittedBlock,
        trust_anchor_id: PrivacyIssuerIdV1,
        policy_id: PrivacyPolicyIdV1,
        budget: &AllocationBudget,
    ) -> Result<NativeZkX509StateV1, WorldStateSnapshotError> {
        self.with_native_world_state_snapshot_cut_v1(tip, None, budget, |snapshot, world| {
            if snapshot.schema_hash != Self::native_world_schema_hash_v1()?
                || crate::sumeragi::lanes::routing::read_routing_root_scope(world)
                    .map_err(WorldStateSnapshotError::RootScopeRead)?
                    != Some(tip.root_scope())
            {
                return Err("X.509 authority differs from the native schema or root scope".into());
            }
            let policy = world.privacy_consensus_policy.get();
            authenticate_originals(
                snapshot,
                &world.privacy_commitments,
                &world.privacy_roots,
                &world.privacy_root_heads,
                policy,
            )?;
            policy
                .validate_at_committed_height(tip.height())
                .map_err(|e| e.to_string())?;
            let authoritative = load_privacy_zk_x509_authoritative_state_v1(
                trust_anchor_id,
                policy_id,
                policy.current_limits.retained_root_count,
                &world.privacy_commitments,
                &world.privacy_roots,
                &world.privacy_root_heads,
            )?
            .try_into_retained_v1(budget)?;
            Ok(NativeZkX509StateV1 {
                authoritative,
                policy: *policy,
                network_id: tip.commitment().schedule.current.network_id,
                height: tip.height(),
                certified_block_id: tip.id(),
                root_scope: tip.root_scope(),
                world_state_root: tip.commitment().execution.world_state_root,
                schema_hash: snapshot.schema_hash,
                ledger_timestamp_ms: tip.block_time_ms(),
            })
        })
    }
}

fn authenticate_table<K: mv::Key + norito::codec::Encode, V: mv::Value + norito::codec::Encode>(
    snapshot: &WorldStateSnapshotV1,
    field: &str,
    table: &impl mv::storage::StorageReadOnly<K, V>,
) -> Result<(), String> {
    require_complete_table_count(snapshot, field, table.iter().count())?;
    for (key, value) in table.iter() {
        require_target(
            snapshot,
            field,
            WorldStateElementKindV1::Table,
            Some(hash_value(key)?),
            hash_value(value)?,
        )?;
    }
    Ok(())
}

fn authenticate_originals(
    snapshot: &WorldStateSnapshotV1,
    commitments: &impl mv::storage::StorageReadOnly<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
    roots: &impl mv::storage::StorageReadOnly<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    heads: &impl mv::storage::StorageReadOnly<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
    policy: &PrivacyConsensusPolicyV1,
) -> Result<(), String> {
    authenticate_table(snapshot, "world.privacy_commitments", commitments)?;
    authenticate_table(snapshot, "world.privacy_roots", roots)?;
    authenticate_table(snapshot, "world.privacy_root_heads", heads)?;
    require_target(
        snapshot,
        "world.privacy_consensus_policy",
        WorldStateElementKindV1::Cell,
        None,
        hash_value(policy)?,
    )
}

#[cfg(test)]
#[path = "zk_x509/tests.rs"]
mod tests;
