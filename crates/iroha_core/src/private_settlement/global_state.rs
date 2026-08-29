//! Globally replicated public state for finalized private settlement bundles.
//!
//! Application is deliberately split into a read-only planning pass and an
//! infallible mutation pass. A bad leg therefore cannot expose any subset of a
//! bundle's roots, nullifiers, commitments, encrypted outputs, or receipt.

use super::{
    protocol::verify_private_settlement_receipt_v1,
    state::{
        PrivateSettlementPoolGovernanceProjectionV1, PrivateSettlementPoolStateV1,
        PrivateSettlementStateErrorV1,
    },
};
use iroha_crypto::Hash;
#[cfg(test)]
use iroha_data_model::nexus::PrivateSettlementPoolGovernanceV1;
use iroha_data_model::{
    nexus::{
        PrivateSettlementAbortReceiptV1, PrivateSettlementDeltaV1, PrivateSettlementReceiptV1,
        PrivateSettlementRouteV1,
    },
    privacy::{
        PrivacyCommitmentV1, PrivacyEncryptedOutputV1, PrivacyNullifierV1, PrivacyPoolIdV1,
        PrivacyRootV1,
    },
};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};
use norito::derive::{JsonDeserialize, JsonSerialize};
use norito::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const RECEIPT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:global-receipt:v1\0";

pub(crate) fn canonical_receipt_digest_v1(
    receipt: &PrivateSettlementReceiptV1,
) -> Result<Hash, PrivateSettlementGlobalStateErrorV1> {
    let encoded = norito::encode_canonical(receipt)
        .map_err(|_| PrivateSettlementGlobalStateErrorV1::Encoding)?;
    let encoded_len =
        u64::try_from(encoded.len()).map_err(|_| PrivateSettlementGlobalStateErrorV1::Encoding)?;
    Ok(Hash::new_from_chunks(&[
        RECEIPT_DIGEST_DOMAIN_V1,
        &encoded_len.to_le_bytes(),
        encoded.as_slice(),
    ]))
}

/// Exact route-scoped identity of one governed confidential settlement pool.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    JsonDeserialize,
    JsonSerialize,
)]
pub(crate) struct PrivateSettlementPoolKeyV1 {
    route: PrivateSettlementRouteV1,
    pool_id: PrivacyPoolIdV1,
}

impl PrivateSettlementPoolKeyV1 {
    /// Construct the key used by every private-settlement state map.
    pub(crate) fn new(
        route: PrivateSettlementRouteV1,
        pool_id: PrivacyPoolIdV1,
    ) -> Result<Self, PrivateSettlementGlobalStateErrorV1> {
        if route.lane_incarnation == Hash::prehashed([0; Hash::LENGTH]) || pool_id.is_zero() {
            return Err(PrivateSettlementGlobalStateErrorV1::Pool);
        }
        Ok(Self { route, pool_id })
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    JsonDeserialize,
    JsonSerialize,
)]
pub(crate) struct PrivateSettlementRootKeyV1 {
    pub(crate) pool: PrivateSettlementPoolKeyV1,
    pub(crate) epoch: u64,
    pub(crate) root: PrivacyRootV1,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    JsonDeserialize,
    JsonSerialize,
)]
pub(crate) struct PrivateSettlementNullifierKeyV1 {
    pub(crate) pool: PrivateSettlementPoolKeyV1,
    pub(crate) nullifier: PrivacyNullifierV1,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    JsonDeserialize,
    JsonSerialize,
)]
pub(crate) struct PrivateSettlementOutputKeyV1 {
    pub(crate) pool: PrivateSettlementPoolKeyV1,
    pub(crate) commitment: PrivacyCommitmentV1,
}

fn encode_private_settlement_storage_key_v1<T: Encode>(key: &T, out: &mut String) {
    let encoded = norito::to_bytes(key).expect("fixed private-settlement keys always encode");
    json::write_json_string(&hex::encode_upper(encoded), out);
}

fn decode_private_settlement_storage_key_v1<T: Decode + Encode>(
    encoded: &str,
) -> Result<T, json::Error> {
    let bytes = hex::decode(encoded).map_err(|error| {
        json::Error::Message(format!("invalid private-settlement key hex: {error}"))
    })?;
    let key: T = norito::decode_from_bytes(&bytes).map_err(|error| {
        json::Error::Message(format!("invalid private-settlement key encoding: {error}"))
    })?;
    let canonical = norito::to_bytes(&key).map_err(|error| {
        json::Error::Message(format!(
            "failed to re-encode private-settlement key: {error}"
        ))
    })?;
    if encoded != hex::encode_upper(canonical) {
        return Err(json::Error::Message(
            "private-settlement key is not canonical uppercase exact Norito hex".to_owned(),
        ));
    }
    Ok(key)
}

macro_rules! impl_private_settlement_json_key_v1 {
    ($($key:ty),+ $(,)?) => {
        $(
            impl mv::json::JsonKeyCodec for $key {
                fn encode_json_key(&self, out: &mut String) {
                    encode_private_settlement_storage_key_v1(self, out);
                }

                fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
                    decode_private_settlement_storage_key_v1(encoded)
                }
            }
        )+
    };
}

impl_private_settlement_json_key_v1!(
    PrivateSettlementPoolKeyV1,
    PrivateSettlementRootKeyV1,
    PrivateSettlementNullifierKeyV1,
    PrivateSettlementOutputKeyV1,
);

/// Public provenance shared by every state item created by one finalized leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, JsonDeserialize, JsonSerialize)]
pub(crate) struct PrivateSettlementFinalizationReferenceV1 {
    pub(crate) bundle_id: Hash,
    pub(crate) receipt_digest: Hash,
    pub(crate) leg_ordinal: u8,
    pub(crate) finalized_height: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, JsonDeserialize, JsonSerialize)]
#[norito(tag = "origin", content = "record", deny_unknown_fields)]
pub(crate) enum PrivateSettlementRootProvenanceV1 {
    Governance {
        governance_digest: Hash,
        admitted_at_height: u64,
    },
    Settlement(PrivateSettlementFinalizationReferenceV1),
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, JsonDeserialize, JsonSerialize)]
pub(crate) struct PrivateSettlementOutputRecordV1 {
    pub(crate) reference: PrivateSettlementFinalizationReferenceV1,
    pub(crate) encrypted_output: PrivacyEncryptedOutputV1,
}

/// Test/reference aggregate over the production private-settlement planner maps.
#[cfg(test)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Decode, Encode)]
pub(crate) struct PrivateSettlementGlobalStateV1 {
    governance: BTreeMap<PrivateSettlementPoolKeyV1, PrivateSettlementPoolGovernanceProjectionV1>,
    pools: BTreeMap<PrivateSettlementPoolKeyV1, PrivateSettlementPoolStateV1>,
    roots: BTreeMap<PrivateSettlementRootKeyV1, PrivateSettlementRootProvenanceV1>,
    nullifiers: BTreeMap<PrivateSettlementNullifierKeyV1, PrivateSettlementFinalizationReferenceV1>,
    outputs: BTreeMap<PrivateSettlementOutputKeyV1, PrivateSettlementOutputRecordV1>,
    receipts: BTreeMap<Hash, PrivateSettlementReceiptV1>,
    aborts: BTreeMap<Hash, PrivateSettlementAbortReceiptV1>,
}

/// Outcome of an idempotent global-state operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PrivateSettlementGlobalStateOutcomeV1 {
    /// The complete operation was applied.
    Applied,
    /// The exact operation was already durable.
    Idempotent,
}

pub(crate) struct PrivateSettlementPlannedLegV1 {
    pub(crate) key: PrivateSettlementPoolKeyV1,
    pub(crate) next_pool: PrivateSettlementPoolStateV1,
    pub(crate) root_key: PrivateSettlementRootKeyV1,
    pub(crate) reference: PrivateSettlementFinalizationReferenceV1,
    pub(crate) nullifiers: Vec<PrivateSettlementNullifierKeyV1>,
    pub(crate) outputs: Vec<(
        PrivateSettlementOutputKeyV1,
        PrivateSettlementOutputRecordV1,
    )>,
}

/// Complete infallible write set for bootstrapping one governed pool.
pub(crate) struct PrivateSettlementPoolBootstrapPlanV1 {
    pub(crate) key: PrivateSettlementPoolKeyV1,
    pub(crate) governance: PrivateSettlementPoolGovernanceProjectionV1,
    pub(crate) pool: PrivateSettlementPoolStateV1,
    pub(crate) root_key: PrivateSettlementRootKeyV1,
    pub(crate) root_provenance: PrivateSettlementRootProvenanceV1,
}

/// Complete infallible write set for one policy/key rotation.
pub(crate) struct PrivateSettlementPoolRotationPlanV1 {
    pub(crate) key: PrivateSettlementPoolKeyV1,
    pub(crate) governance: PrivateSettlementPoolGovernanceProjectionV1,
    pub(crate) pool: PrivateSettlementPoolStateV1,
}

fn governance_origin_matches_current_v1(
    current: &PrivateSettlementPoolGovernanceProjectionV1,
    origin_governance_digest: Hash,
    origin_admitted_at_height: u64,
) -> bool {
    if origin_admitted_at_height == 0
        || origin_governance_digest == Hash::prehashed([0; Hash::LENGTH])
    {
        return false;
    }
    let origin = current
        .prior_revisions
        .first()
        .copied()
        .unwrap_or_else(|| current.current_revision());
    origin.lifecycle.governance_revision == 1
        && origin.governance_digest == origin_governance_digest
        && origin.lifecycle.is_active_at(origin_admitted_at_height)
}

fn receipt_leg_matches_governance_lineage_v1(
    governance: &PrivateSettlementPoolGovernanceProjectionV1,
    receipt: &PrivateSettlementReceiptV1,
    delta: &PrivateSettlementDeltaV1,
) -> bool {
    let context_revision = governance.revision_at(receipt.manifest.authority_context_height);
    let finalized_revision = governance.revision_at(receipt.finalized_height);
    let Some(revision) = context_revision.filter(|revision| Some(*revision) == finalized_revision)
    else {
        return false;
    };
    delta.asset_binding_commitment == governance.asset_binding_commitment
        && delta.audit_policy_digest == revision.audit_policy_digest
        && delta.audit_key_epoch == revision.audit_key_epoch
        && revision
            .lifecycle
            .retirement_height
            .is_none_or(|retirement| receipt.manifest.expiry_height < retirement)
}

/// Validate and plan one governed pool bootstrap without mutating persistent state.
pub(crate) fn plan_private_settlement_pool_bootstrap_v1(
    governance_store: &impl StorageReadOnly<
        PrivateSettlementPoolKeyV1,
        PrivateSettlementPoolGovernanceProjectionV1,
    >,
    pools: &impl StorageReadOnly<PrivateSettlementPoolKeyV1, PrivateSettlementPoolStateV1>,
    roots: &impl StorageReadOnly<PrivateSettlementRootKeyV1, PrivateSettlementRootProvenanceV1>,
    governance: PrivateSettlementPoolGovernanceProjectionV1,
    initial_commitments: &[PrivacyCommitmentV1],
    admitted_at_height: u64,
) -> Result<Option<PrivateSettlementPoolBootstrapPlanV1>, PrivateSettlementGlobalStateErrorV1> {
    governance
        .validate()
        .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
    if admitted_at_height == 0 || !governance.lifecycle.is_active_at(admitted_at_height) {
        return Err(PrivateSettlementGlobalStateErrorV1::Governance);
    }
    let key = PrivateSettlementPoolKeyV1::new(governance.route, governance.pool_id)?;
    let pool = PrivateSettlementPoolStateV1::bootstrap(
        governance.route,
        governance.pool_id,
        governance.governance_digest,
        initial_commitments,
    )
    .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
    let root_key = PrivateSettlementRootKeyV1 {
        pool: key,
        epoch: pool.epoch(),
        root: pool.root(),
    };
    if let Some(existing) = governance_store.get(&key) {
        return if existing == &governance
            && pools.get(&key) == Some(&pool)
            && matches!(
                roots.get(&root_key),
                Some(PrivateSettlementRootProvenanceV1::Governance {
                    governance_digest,
                    admitted_at_height,
                }) if governance_digest == &governance.governance_digest
                    && governance.lifecycle.is_active_at(*admitted_at_height)
            ) {
            Ok(None)
        } else {
            Err(PrivateSettlementGlobalStateErrorV1::Substitution)
        };
    }
    if pools.get(&key).is_some() || roots.get(&root_key).is_some() {
        return Err(PrivateSettlementGlobalStateErrorV1::Substitution);
    }
    let governance_digest = governance.governance_digest;
    Ok(Some(PrivateSettlementPoolBootstrapPlanV1 {
        key,
        governance,
        pool,
        root_key,
        root_provenance: PrivateSettlementRootProvenanceV1::Governance {
            governance_digest,
            admitted_at_height,
        },
    }))
}

/// Validate and plan one exact governance rotation without changing the pool frontier.
pub(crate) fn plan_private_settlement_pool_rotation_v1(
    governance_store: &impl StorageReadOnly<
        PrivateSettlementPoolKeyV1,
        PrivateSettlementPoolGovernanceProjectionV1,
    >,
    pools: &impl StorageReadOnly<PrivateSettlementPoolKeyV1, PrivateSettlementPoolStateV1>,
    receipts: &impl StorageReadOnly<Hash, PrivateSettlementReceiptV1>,
    expected_governance_digest: Hash,
    replacement: PrivateSettlementPoolGovernanceProjectionV1,
    admitted_at_height: u64,
) -> Result<Option<PrivateSettlementPoolRotationPlanV1>, PrivateSettlementGlobalStateErrorV1> {
    replacement
        .validate_current_fields()
        .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
    if admitted_at_height <= 1
        || replacement.lifecycle.activation_height != admitted_at_height
        || !replacement.lifecycle.is_active_at(admitted_at_height)
    {
        return Err(PrivateSettlementGlobalStateErrorV1::Governance);
    }
    let key = PrivateSettlementPoolKeyV1::new(replacement.route, replacement.pool_id)?;
    let current = governance_store
        .get(&key)
        .ok_or(PrivateSettlementGlobalStateErrorV1::Governance)?;
    let pool = pools
        .get(&key)
        .ok_or(PrivateSettlementGlobalStateErrorV1::Pool)?;
    current
        .validate()
        .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
    if !replacement.prior_revisions.is_empty() {
        return Err(PrivateSettlementGlobalStateErrorV1::Governance);
    }
    if current.current_fields_equal(&replacement)
        && pool.restricted_pool_policy_digest() == replacement.governance_digest
    {
        return Ok(None);
    }
    let expected_revision = current
        .lifecycle
        .governance_revision
        .checked_add(1)
        .ok_or(PrivateSettlementGlobalStateErrorV1::Governance)?;
    if current.governance_digest != expected_governance_digest {
        return Err(PrivateSettlementGlobalStateErrorV1::Substitution);
    }
    if current.version != replacement.version
        || current.route != replacement.route
        || current.pool_id != replacement.pool_id
        || current.asset_binding_commitment != replacement.asset_binding_commitment
        || current.audit_policy_digest == replacement.audit_policy_digest
        || current.audit_key_epoch >= replacement.audit_key_epoch
        || current.governance_digest == replacement.governance_digest
        || replacement.lifecycle.governance_revision != expected_revision
        || !current
            .lifecycle
            .is_active_at(admitted_at_height.saturating_sub(1))
        || pool.route() != current.route
        || pool.pool_id() != current.pool_id
        || pool.restricted_pool_policy_digest() != current.governance_digest
        || receipts.iter().any(|(_, receipt)| {
            receipt.finalized_height == admitted_at_height
                && receipt.legs.iter().any(|leg| {
                    leg.delta.route == replacement.route && leg.delta.pool_id == replacement.pool_id
                })
        })
    {
        return Err(PrivateSettlementGlobalStateErrorV1::Governance);
    }
    let replacement = current
        .with_replacement(replacement)
        .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
    let rotated_pool = pool
        .rotate_governance_digest(current.governance_digest, replacement.governance_digest)
        .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
    Ok(Some(PrivateSettlementPoolRotationPlanV1 {
        key,
        governance: replacement,
        pool: rotated_pool,
    }))
}

/// Validate and plan a complete certified receipt without mutating persistent state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_private_settlement_receipt_v1(
    governance: &impl StorageReadOnly<
        PrivateSettlementPoolKeyV1,
        PrivateSettlementPoolGovernanceProjectionV1,
    >,
    pools: &impl StorageReadOnly<PrivateSettlementPoolKeyV1, PrivateSettlementPoolStateV1>,
    roots: &impl StorageReadOnly<PrivateSettlementRootKeyV1, PrivateSettlementRootProvenanceV1>,
    nullifiers: &impl StorageReadOnly<
        PrivateSettlementNullifierKeyV1,
        PrivateSettlementFinalizationReferenceV1,
    >,
    outputs: &impl StorageReadOnly<PrivateSettlementOutputKeyV1, PrivateSettlementOutputRecordV1>,
    receipts: &impl StorageReadOnly<Hash, PrivateSettlementReceiptV1>,
    aborts: &impl StorageReadOnly<Hash, PrivateSettlementAbortReceiptV1>,
    receipt: &PrivateSettlementReceiptV1,
    current_height: u64,
) -> Result<Option<Vec<PrivateSettlementPlannedLegV1>>, PrivateSettlementGlobalStateErrorV1> {
    verify_private_settlement_receipt_v1(receipt)
        .map_err(|_| PrivateSettlementGlobalStateErrorV1::Receipt)?;
    if current_height == 0 || receipt.finalized_height != current_height {
        return Err(PrivateSettlementGlobalStateErrorV1::Height);
    }
    let bundle_id = receipt.manifest.bundle_id;
    if let Some(existing) = receipts.get(&bundle_id) {
        return if existing == receipt {
            Ok(None)
        } else {
            Err(PrivateSettlementGlobalStateErrorV1::Substitution)
        };
    }
    if aborts.get(&bundle_id).is_some() {
        return Err(PrivateSettlementGlobalStateErrorV1::Terminal);
    }
    let receipt_digest = canonical_receipt_digest_v1(receipt)?;
    let mut seen_pools = BTreeSet::new();
    let mut seen_nullifiers = BTreeSet::new();
    let mut seen_outputs = BTreeSet::new();
    let mut plan = Vec::with_capacity(receipt.legs.len());
    for (index, leg) in receipt.legs.iter().enumerate() {
        let ordinal =
            u8::try_from(index).map_err(|_| PrivateSettlementGlobalStateErrorV1::Receipt)?;
        let key = PrivateSettlementPoolKeyV1::new(leg.delta.route, leg.delta.pool_id)?;
        if !seen_pools.insert(key) {
            return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
        }
        let governed_pool = governance
            .get(&key)
            .ok_or(PrivateSettlementGlobalStateErrorV1::Governance)?;
        let pool = pools
            .get(&key)
            .ok_or(PrivateSettlementGlobalStateErrorV1::Pool)?;
        let next_pool = pool
            .apply_certified_delta(
                &leg.delta,
                governed_pool,
                receipt.manifest.authority_context_height,
                receipt.manifest.expiry_height,
                receipt.finalized_height,
            )
            .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
        let root_key = PrivateSettlementRootKeyV1 {
            pool: key,
            epoch: next_pool.epoch(),
            root: next_pool.root(),
        };
        if roots.get(&root_key).is_some() {
            return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
        }
        let reference = PrivateSettlementFinalizationReferenceV1 {
            bundle_id,
            receipt_digest,
            leg_ordinal: ordinal,
            finalized_height: current_height,
        };
        let mut planned_nullifiers = Vec::with_capacity(leg.delta.nullifiers.len());
        for nullifier in &leg.delta.nullifiers {
            let nullifier_key = PrivateSettlementNullifierKeyV1 {
                pool: key,
                nullifier: *nullifier,
            };
            if !seen_nullifiers.insert(nullifier_key) || nullifiers.get(&nullifier_key).is_some() {
                return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
            }
            planned_nullifiers.push(nullifier_key);
        }
        let mut planned_outputs = Vec::with_capacity(leg.delta.output_commitments.len());
        for (commitment, encrypted_output) in leg
            .delta
            .output_commitments
            .iter()
            .zip(&leg.delta.encrypted_outputs)
        {
            let output_key = PrivateSettlementOutputKeyV1 {
                pool: key,
                commitment: *commitment,
            };
            if !seen_outputs.insert(output_key) || outputs.get(&output_key).is_some() {
                return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
            }
            planned_outputs.push((
                output_key,
                PrivateSettlementOutputRecordV1 {
                    reference,
                    encrypted_output: encrypted_output.clone(),
                },
            ));
        }
        plan.push(PrivateSettlementPlannedLegV1 {
            key,
            next_pool,
            root_key,
            reference,
            nullifiers: planned_nullifiers,
            outputs: planned_outputs,
        });
    }
    Ok(Some(plan))
}

/// Validate an abort marker against terminal WSV state without mutating it.
pub(crate) fn plan_private_settlement_abort_v1(
    receipts: &impl StorageReadOnly<Hash, PrivateSettlementReceiptV1>,
    aborts: &impl StorageReadOnly<Hash, PrivateSettlementAbortReceiptV1>,
    receipt: &PrivateSettlementAbortReceiptV1,
    current_height: u64,
) -> Result<bool, PrivateSettlementGlobalStateErrorV1> {
    receipt
        .validate()
        .map_err(|_| PrivateSettlementGlobalStateErrorV1::Receipt)?;
    if current_height == 0 || receipt.finalized_height != current_height {
        return Err(PrivateSettlementGlobalStateErrorV1::Height);
    }
    if receipts.get(&receipt.bundle_id).is_some() {
        return Err(PrivateSettlementGlobalStateErrorV1::Terminal);
    }
    if let Some(existing) = aborts.get(&receipt.bundle_id) {
        return if existing == receipt {
            Ok(false)
        } else {
            Err(PrivateSettlementGlobalStateErrorV1::Substitution)
        };
    }
    Ok(true)
}

/// Validate every private-settlement map and cross-reference after snapshot recovery.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn validate_private_settlement_persisted_state_v1(
    governance: &impl StorageReadOnly<
        PrivateSettlementPoolKeyV1,
        PrivateSettlementPoolGovernanceProjectionV1,
    >,
    pools: &impl StorageReadOnly<PrivateSettlementPoolKeyV1, PrivateSettlementPoolStateV1>,
    roots: &impl StorageReadOnly<PrivateSettlementRootKeyV1, PrivateSettlementRootProvenanceV1>,
    nullifiers: &impl StorageReadOnly<
        PrivateSettlementNullifierKeyV1,
        PrivateSettlementFinalizationReferenceV1,
    >,
    outputs: &impl StorageReadOnly<PrivateSettlementOutputKeyV1, PrivateSettlementOutputRecordV1>,
    receipts: &impl StorageReadOnly<Hash, PrivateSettlementReceiptV1>,
    aborts: &impl StorageReadOnly<Hash, PrivateSettlementAbortReceiptV1>,
) -> Result<(), PrivateSettlementGlobalStateErrorV1> {
    let mut roots_by_pool =
        BTreeMap::<PrivateSettlementPoolKeyV1, BTreeMap<u64, PrivacyRootV1>>::new();
    for (key, _) in roots.iter() {
        if key.epoch == 0
            || key.root.is_zero()
            || roots_by_pool
                .entry(key.pool)
                .or_default()
                .insert(key.epoch, key.root)
                .is_some()
        {
            return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
        }
    }
    for (key, governed_pool) in governance.iter() {
        governed_pool
            .validate()
            .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
        if key.route != governed_pool.route || key.pool_id != governed_pool.pool_id {
            return Err(PrivateSettlementGlobalStateErrorV1::Governance);
        }
        let pool = pools
            .get(key)
            .ok_or(PrivateSettlementGlobalStateErrorV1::Pool)?;
        pool.validate()
            .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
        if pool.route() != key.route
            || pool.pool_id() != key.pool_id
            || pool.restricted_pool_policy_digest() != governed_pool.governance_digest
            || roots
                .get(&PrivateSettlementRootKeyV1 {
                    pool: *key,
                    epoch: pool.epoch(),
                    root: pool.root(),
                })
                .is_none()
            || roots_by_pool
                .get(key)
                .and_then(|pool_roots| pool_roots.get(&1))
                .is_none_or(|origin_root| {
                    !matches!(
                        roots.get(&PrivateSettlementRootKeyV1 {
                            pool: *key,
                            epoch: 1,
                            root: *origin_root,
                        }),
                        Some(PrivateSettlementRootProvenanceV1::Governance {
                            governance_digest,
                            admitted_at_height,
                        }) if governance_origin_matches_current_v1(
                            governed_pool,
                            *governance_digest,
                            *admitted_at_height,
                        )
                    )
                })
        {
            return Err(PrivateSettlementGlobalStateErrorV1::Pool);
        }
    }
    if pools.iter().count() != governance.iter().count() {
        return Err(PrivateSettlementGlobalStateErrorV1::Pool);
    }

    let mut receipt_digests = BTreeMap::new();
    let mut transitions = BTreeMap::<PrivateSettlementRootKeyV1, PrivateSettlementRootKeyV1>::new();
    let mut transition_targets = BTreeSet::new();
    for (bundle_id, receipt) in receipts.iter() {
        verify_private_settlement_receipt_v1(receipt)
            .map_err(|_| PrivateSettlementGlobalStateErrorV1::Receipt)?;
        if bundle_id != &receipt.manifest.bundle_id || aborts.get(bundle_id).is_some() {
            return Err(PrivateSettlementGlobalStateErrorV1::Receipt);
        }
        receipt_digests.insert(*bundle_id, canonical_receipt_digest_v1(receipt)?);
    }
    let validate_reference = |reference: PrivateSettlementFinalizationReferenceV1| {
        let receipt = receipts
            .get(&reference.bundle_id)
            .ok_or(PrivateSettlementGlobalStateErrorV1::Receipt)?;
        if receipt.finalized_height != reference.finalized_height
            || receipt_digests.get(&reference.bundle_id) != Some(&reference.receipt_digest)
            || usize::from(reference.leg_ordinal) >= receipt.legs.len()
        {
            return Err(PrivateSettlementGlobalStateErrorV1::Receipt);
        }
        Ok(())
    };
    for (bundle_id, receipt) in receipts.iter() {
        let receipt_digest = receipt_digests
            .get(bundle_id)
            .copied()
            .ok_or(PrivateSettlementGlobalStateErrorV1::Receipt)?;
        for (index, leg) in receipt.legs.iter().enumerate() {
            let leg_ordinal =
                u8::try_from(index).map_err(|_| PrivateSettlementGlobalStateErrorV1::Receipt)?;
            let pool = PrivateSettlementPoolKeyV1::new(leg.delta.route, leg.delta.pool_id)?;
            let governed_pool = governance
                .get(&pool)
                .ok_or(PrivateSettlementGlobalStateErrorV1::Governance)?;
            if !receipt_leg_matches_governance_lineage_v1(governed_pool, receipt, &leg.delta)
                || leg.delta.old_epoch.checked_add(1) != Some(leg.delta.new_epoch)
            {
                return Err(PrivateSettlementGlobalStateErrorV1::Governance);
            }
            let reference = PrivateSettlementFinalizationReferenceV1 {
                bundle_id: *bundle_id,
                receipt_digest,
                leg_ordinal,
                finalized_height: receipt.finalized_height,
            };
            let old_root_key = PrivateSettlementRootKeyV1 {
                pool,
                epoch: leg.delta.old_epoch,
                root: leg.delta.old_root,
            };
            let new_root_key = PrivateSettlementRootKeyV1 {
                pool,
                epoch: leg.delta.new_epoch,
                root: leg.delta.new_root,
            };
            if roots.get(&old_root_key).is_none()
                || roots.get(&new_root_key)
                    != Some(&PrivateSettlementRootProvenanceV1::Settlement(reference))
                || transitions.insert(old_root_key, new_root_key).is_some()
                || !transition_targets.insert(new_root_key)
            {
                return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
            }
            for nullifier in &leg.delta.nullifiers {
                if nullifiers.get(&PrivateSettlementNullifierKeyV1 {
                    pool,
                    nullifier: *nullifier,
                }) != Some(&reference)
                {
                    return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
                }
            }
            for (commitment, encrypted_output) in leg
                .delta
                .output_commitments
                .iter()
                .zip(&leg.delta.encrypted_outputs)
            {
                let record = outputs
                    .get(&PrivateSettlementOutputKeyV1 {
                        pool,
                        commitment: *commitment,
                    })
                    .ok_or(PrivateSettlementGlobalStateErrorV1::Conflict)?;
                if record.reference != reference || &record.encrypted_output != encrypted_output {
                    return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
                }
            }
        }
    }
    for (key, reference) in nullifiers.iter() {
        validate_reference(*reference)?;
        let receipt = receipts
            .get(&reference.bundle_id)
            .ok_or(PrivateSettlementGlobalStateErrorV1::Receipt)?;
        let delta = &receipt.legs[usize::from(reference.leg_ordinal)].delta;
        if key.pool.route != delta.route
            || key.pool.pool_id != delta.pool_id
            || !delta.nullifiers.contains(&key.nullifier)
        {
            return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
        }
    }
    for (key, record) in outputs.iter() {
        validate_reference(record.reference)?;
        if record.encrypted_output.commitment != key.commitment {
            return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
        }
        let receipt = receipts
            .get(&record.reference.bundle_id)
            .ok_or(PrivateSettlementGlobalStateErrorV1::Receipt)?;
        let delta = &receipt.legs[usize::from(record.reference.leg_ordinal)].delta;
        if key.pool.route != delta.route
            || key.pool.pool_id != delta.pool_id
            || !delta.encrypted_outputs.contains(&record.encrypted_output)
        {
            return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
        }
    }
    for (key, provenance) in roots.iter() {
        match provenance {
            PrivateSettlementRootProvenanceV1::Governance {
                governance_digest,
                admitted_at_height,
            } => {
                if key.epoch != 1
                    || *admitted_at_height == 0
                    || governance.get(&key.pool).is_none_or(|record| {
                        !governance_origin_matches_current_v1(
                            record,
                            *governance_digest,
                            *admitted_at_height,
                        )
                    })
                {
                    return Err(PrivateSettlementGlobalStateErrorV1::Governance);
                }
            }
            PrivateSettlementRootProvenanceV1::Settlement(reference) => {
                validate_reference(*reference)?;
                let receipt = receipts
                    .get(&reference.bundle_id)
                    .ok_or(PrivateSettlementGlobalStateErrorV1::Receipt)?;
                let delta = &receipt.legs[usize::from(reference.leg_ordinal)].delta;
                if key.pool.route != delta.route
                    || key.pool.pool_id != delta.pool_id
                    || key.epoch != delta.new_epoch
                    || key.root != delta.new_root
                {
                    return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
                }
            }
        }
    }
    for (key, pool) in pools.iter() {
        let origin_root = roots_by_pool
            .get(key)
            .and_then(|pool_roots| pool_roots.get(&1))
            .copied()
            .ok_or(PrivateSettlementGlobalStateErrorV1::Conflict)?;
        let mut cursor = PrivateSettlementRootKeyV1 {
            pool: *key,
            epoch: 1,
            root: origin_root,
        };
        let transition_count = transitions
            .keys()
            .filter(|root_key| root_key.pool == *key)
            .count();
        let mut traversed = 0_usize;
        while let Some(next) = transitions.get(&cursor).copied() {
            if next.pool != *key || cursor.epoch.checked_add(1) != Some(next.epoch) {
                return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
            }
            cursor = next;
            traversed = traversed
                .checked_add(1)
                .ok_or(PrivateSettlementGlobalStateErrorV1::Conflict)?;
            if traversed > transition_count {
                return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
            }
        }
        if cursor.epoch != pool.epoch()
            || cursor.root != pool.root()
            || traversed != transition_count
            || roots_by_pool.get(key).map(BTreeMap::len) != Some(transition_count + 1)
        {
            return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
        }
    }
    for (bundle_id, abort) in aborts.iter() {
        abort
            .validate()
            .map_err(|_| PrivateSettlementGlobalStateErrorV1::Receipt)?;
        if bundle_id != &abort.bundle_id || receipts.get(bundle_id).is_some() {
            return Err(PrivateSettlementGlobalStateErrorV1::Receipt);
        }
    }
    Ok(())
}

#[cfg(test)]
impl PrivateSettlementGlobalStateV1 {
    /// Explicitly bootstrap one restricted pool through governance.
    pub(crate) fn bootstrap_pool(
        &mut self,
        governance: PrivateSettlementPoolGovernanceV1,
        initial_commitments: &[PrivacyCommitmentV1],
        admitted_at_height: u64,
    ) -> Result<PrivateSettlementGlobalStateOutcomeV1, PrivateSettlementGlobalStateErrorV1> {
        governance
            .validate()
            .map_err(|_| PrivateSettlementGlobalStateErrorV1::Governance)?;
        if admitted_at_height == 0 || !governance.body.lifecycle.is_active_at(admitted_at_height) {
            return Err(PrivateSettlementGlobalStateErrorV1::Governance);
        }
        let key = PrivateSettlementPoolKeyV1::new(governance.body.route, governance.body.pool_id)?;
        let governance_projection =
            PrivateSettlementPoolGovernanceProjectionV1::from_restricted(&governance)
                .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
        let pool = PrivateSettlementPoolStateV1::bootstrap(
            governance.body.route,
            governance.body.pool_id,
            governance.governance_digest,
            initial_commitments,
        )
        .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
        if let Some(existing) = self.governance.get(&key) {
            return if existing == &governance_projection && self.pools.get(&key) == Some(&pool) {
                Ok(PrivateSettlementGlobalStateOutcomeV1::Idempotent)
            } else {
                Err(PrivateSettlementGlobalStateErrorV1::Substitution)
            };
        }
        let root_key = PrivateSettlementRootKeyV1 {
            pool: key,
            epoch: pool.epoch(),
            root: pool.root(),
        };
        if self.pools.contains_key(&key) || self.roots.contains_key(&root_key) {
            return Err(PrivateSettlementGlobalStateErrorV1::Substitution);
        }
        self.governance.insert(key, governance_projection);
        self.pools.insert(key, pool);
        self.roots.insert(
            root_key,
            PrivateSettlementRootProvenanceV1::Governance {
                governance_digest: governance.governance_digest,
                admitted_at_height,
            },
        );
        Ok(PrivateSettlementGlobalStateOutcomeV1::Applied)
    }

    /// Rotate one pool governance projection while preserving its exact frontier.
    pub(crate) fn rotate_pool_policy(
        &mut self,
        expected_governance_digest: Hash,
        replacement: PrivateSettlementPoolGovernanceProjectionV1,
        admitted_at_height: u64,
    ) -> Result<PrivateSettlementGlobalStateOutcomeV1, PrivateSettlementGlobalStateErrorV1> {
        let governance = mv::storage::Storage::from_iter(self.governance.clone());
        let pools = mv::storage::Storage::from_iter(self.pools.clone());
        let receipts = mv::storage::Storage::from_iter(self.receipts.clone());
        let plan = plan_private_settlement_pool_rotation_v1(
            &governance.view(),
            &pools.view(),
            &receipts.view(),
            expected_governance_digest,
            replacement,
            admitted_at_height,
        )?;
        let Some(plan) = plan else {
            return Ok(PrivateSettlementGlobalStateOutcomeV1::Idempotent);
        };
        self.governance.insert(plan.key, plan.governance);
        self.pools.insert(plan.key, plan.pool);
        Ok(PrivateSettlementGlobalStateOutcomeV1::Applied)
    }

    /// Apply all certified legs and their receipt together or leave state unchanged.
    pub(crate) fn apply_receipt(
        &mut self,
        receipt: PrivateSettlementReceiptV1,
        current_height: u64,
    ) -> Result<PrivateSettlementGlobalStateOutcomeV1, PrivateSettlementGlobalStateErrorV1> {
        verify_private_settlement_receipt_v1(&receipt)
            .map_err(|_| PrivateSettlementGlobalStateErrorV1::Receipt)?;
        if current_height == 0 || receipt.finalized_height != current_height {
            return Err(PrivateSettlementGlobalStateErrorV1::Height);
        }
        let bundle_id = receipt.manifest.bundle_id;
        if let Some(existing) = self.receipts.get(&bundle_id) {
            return if existing == &receipt {
                Ok(PrivateSettlementGlobalStateOutcomeV1::Idempotent)
            } else {
                Err(PrivateSettlementGlobalStateErrorV1::Substitution)
            };
        }
        if self.aborts.contains_key(&bundle_id) {
            return Err(PrivateSettlementGlobalStateErrorV1::Terminal);
        }
        let receipt_digest = canonical_receipt_digest_v1(&receipt)?;
        let mut seen_pools = BTreeSet::new();
        let mut seen_nullifiers = BTreeSet::new();
        let mut seen_outputs = BTreeSet::new();
        let mut plan = Vec::with_capacity(receipt.legs.len());
        for (index, leg) in receipt.legs.iter().enumerate() {
            let ordinal =
                u8::try_from(index).map_err(|_| PrivateSettlementGlobalStateErrorV1::Receipt)?;
            let key = PrivateSettlementPoolKeyV1::new(leg.delta.route, leg.delta.pool_id)?;
            if !seen_pools.insert(key) {
                return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
            }
            let governance = self
                .governance
                .get(&key)
                .ok_or(PrivateSettlementGlobalStateErrorV1::Governance)?;
            let pool = self
                .pools
                .get(&key)
                .ok_or(PrivateSettlementGlobalStateErrorV1::Pool)?;
            let next_pool = pool
                .apply_certified_delta(
                    &leg.delta,
                    governance,
                    receipt.manifest.authority_context_height,
                    receipt.manifest.expiry_height,
                    receipt.finalized_height,
                )
                .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
            let root_key = PrivateSettlementRootKeyV1 {
                pool: key,
                epoch: next_pool.epoch(),
                root: next_pool.root(),
            };
            if self.roots.contains_key(&root_key) {
                return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
            }
            let reference = PrivateSettlementFinalizationReferenceV1 {
                bundle_id,
                receipt_digest,
                leg_ordinal: ordinal,
                finalized_height: current_height,
            };
            let mut nullifiers = Vec::with_capacity(leg.delta.nullifiers.len());
            for nullifier in &leg.delta.nullifiers {
                let nullifier_key = PrivateSettlementNullifierKeyV1 {
                    pool: key,
                    nullifier: *nullifier,
                };
                if !seen_nullifiers.insert(nullifier_key)
                    || self.nullifiers.contains_key(&nullifier_key)
                {
                    return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
                }
                nullifiers.push(nullifier_key);
            }
            let mut outputs = Vec::with_capacity(leg.delta.output_commitments.len());
            for (commitment, encrypted_output) in leg
                .delta
                .output_commitments
                .iter()
                .zip(&leg.delta.encrypted_outputs)
            {
                let output_key = PrivateSettlementOutputKeyV1 {
                    pool: key,
                    commitment: *commitment,
                };
                if !seen_outputs.insert(output_key) || self.outputs.contains_key(&output_key) {
                    return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
                }
                outputs.push((
                    output_key,
                    PrivateSettlementOutputRecordV1 {
                        reference,
                        encrypted_output: encrypted_output.clone(),
                    },
                ));
            }
            plan.push(PrivateSettlementPlannedLegV1 {
                key,
                next_pool,
                root_key,
                reference,
                nullifiers,
                outputs,
            });
        }

        // No fallible validation or encoding occurs below this atomic mutation barrier.
        for leg in plan {
            self.pools.insert(leg.key, leg.next_pool);
            self.roots.insert(
                leg.root_key,
                PrivateSettlementRootProvenanceV1::Settlement(leg.reference),
            );
            for key in leg.nullifiers {
                self.nullifiers.insert(key, leg.reference);
            }
            for (key, record) in leg.outputs {
                self.outputs.insert(key, record);
            }
        }
        self.receipts.insert(bundle_id, receipt);
        Ok(PrivateSettlementGlobalStateOutcomeV1::Applied)
    }

    /// Persist an optional public abort/expiry marker without private contents.
    pub(crate) fn apply_abort(
        &mut self,
        receipt: PrivateSettlementAbortReceiptV1,
        current_height: u64,
    ) -> Result<PrivateSettlementGlobalStateOutcomeV1, PrivateSettlementGlobalStateErrorV1> {
        receipt
            .validate()
            .map_err(|_| PrivateSettlementGlobalStateErrorV1::Receipt)?;
        if current_height == 0 || receipt.finalized_height != current_height {
            return Err(PrivateSettlementGlobalStateErrorV1::Height);
        }
        if self.receipts.contains_key(&receipt.bundle_id) {
            return Err(PrivateSettlementGlobalStateErrorV1::Terminal);
        }
        if let Some(existing) = self.aborts.get(&receipt.bundle_id) {
            return if existing == &receipt {
                Ok(PrivateSettlementGlobalStateOutcomeV1::Idempotent)
            } else {
                Err(PrivateSettlementGlobalStateErrorV1::Substitution)
            };
        }
        self.aborts.insert(receipt.bundle_id, receipt);
        Ok(PrivateSettlementGlobalStateOutcomeV1::Applied)
    }

    /// Return the current root head for a restricted pool without exposing its asset mapping.
    pub(crate) fn pool_head(
        &self,
        route: PrivateSettlementRouteV1,
        pool_id: PrivacyPoolIdV1,
    ) -> Option<(u64, PrivacyRootV1)> {
        let key = PrivateSettlementPoolKeyV1::new(route, pool_id).ok()?;
        self.pools.get(&key).map(|pool| (pool.epoch(), pool.root()))
    }

    /// Query one public finalized receipt by opaque bundle id.
    pub(crate) fn receipt(&self, bundle_id: &Hash) -> Option<&PrivateSettlementReceiptV1> {
        self.receipts.get(bundle_id)
    }

    /// Validate all restored maps and cross-references before accepting a snapshot.
    pub(crate) fn validate(&self) -> Result<(), PrivateSettlementGlobalStateErrorV1> {
        for (key, governance) in &self.governance {
            governance
                .validate()
                .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
            if key.route != governance.route || key.pool_id != governance.pool_id {
                return Err(PrivateSettlementGlobalStateErrorV1::Governance);
            }
            let pool = self
                .pools
                .get(key)
                .ok_or(PrivateSettlementGlobalStateErrorV1::Pool)?;
            pool.validate()
                .map_err(PrivateSettlementGlobalStateErrorV1::from_pool)?;
            if pool.route() != key.route
                || pool.pool_id() != key.pool_id
                || pool.restricted_pool_policy_digest() != governance.governance_digest
                || !self.roots.contains_key(&PrivateSettlementRootKeyV1 {
                    pool: *key,
                    epoch: pool.epoch(),
                    root: pool.root(),
                })
            {
                return Err(PrivateSettlementGlobalStateErrorV1::Pool);
            }
        }
        if self.pools.len() != self.governance.len() {
            return Err(PrivateSettlementGlobalStateErrorV1::Pool);
        }
        let mut receipt_digests = BTreeMap::new();
        for (bundle_id, receipt) in &self.receipts {
            verify_private_settlement_receipt_v1(receipt)
                .map_err(|_| PrivateSettlementGlobalStateErrorV1::Receipt)?;
            if bundle_id != &receipt.manifest.bundle_id || self.aborts.contains_key(bundle_id) {
                return Err(PrivateSettlementGlobalStateErrorV1::Receipt);
            }
            for leg in &receipt.legs {
                let key = PrivateSettlementPoolKeyV1::new(leg.delta.route, leg.delta.pool_id)?;
                let governance = self
                    .governance
                    .get(&key)
                    .ok_or(PrivateSettlementGlobalStateErrorV1::Governance)?;
                if !receipt_leg_matches_governance_lineage_v1(governance, receipt, &leg.delta) {
                    return Err(PrivateSettlementGlobalStateErrorV1::Governance);
                }
            }
            receipt_digests.insert(*bundle_id, canonical_receipt_digest_v1(receipt)?);
        }
        let validate_reference = |reference: PrivateSettlementFinalizationReferenceV1| {
            let receipt = self
                .receipts
                .get(&reference.bundle_id)
                .ok_or(PrivateSettlementGlobalStateErrorV1::Receipt)?;
            if receipt.finalized_height != reference.finalized_height
                || receipt_digests.get(&reference.bundle_id) != Some(&reference.receipt_digest)
                || usize::from(reference.leg_ordinal) >= receipt.legs.len()
            {
                return Err(PrivateSettlementGlobalStateErrorV1::Receipt);
            }
            Ok(())
        };
        for (key, reference) in &self.nullifiers {
            validate_reference(*reference)?;
            let receipt = &self.receipts[&reference.bundle_id];
            let delta = &receipt.legs[usize::from(reference.leg_ordinal)].delta;
            if key.pool.route != delta.route
                || key.pool.pool_id != delta.pool_id
                || !delta.nullifiers.contains(&key.nullifier)
            {
                return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
            }
        }
        for (key, record) in &self.outputs {
            validate_reference(record.reference)?;
            if record.encrypted_output.commitment != key.commitment {
                return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
            }
            let receipt = &self.receipts[&record.reference.bundle_id];
            let delta = &receipt.legs[usize::from(record.reference.leg_ordinal)].delta;
            if key.pool.route != delta.route
                || key.pool.pool_id != delta.pool_id
                || !delta.encrypted_outputs.contains(&record.encrypted_output)
            {
                return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
            }
        }
        for (key, provenance) in &self.roots {
            match provenance {
                PrivateSettlementRootProvenanceV1::Governance {
                    governance_digest,
                    admitted_at_height,
                } => {
                    if *admitted_at_height == 0
                        || self.governance.get(&key.pool).is_none_or(|record| {
                            !governance_origin_matches_current_v1(
                                record,
                                *governance_digest,
                                *admitted_at_height,
                            )
                        })
                    {
                        return Err(PrivateSettlementGlobalStateErrorV1::Governance);
                    }
                }
                PrivateSettlementRootProvenanceV1::Settlement(reference) => {
                    validate_reference(*reference)?;
                    let delta = &self.receipts[&reference.bundle_id].legs
                        [usize::from(reference.leg_ordinal)]
                    .delta;
                    if key.pool.route != delta.route
                        || key.pool.pool_id != delta.pool_id
                        || key.epoch != delta.new_epoch
                        || key.root != delta.new_root
                    {
                        return Err(PrivateSettlementGlobalStateErrorV1::Conflict);
                    }
                }
            }
        }
        for (bundle_id, abort) in &self.aborts {
            abort
                .validate()
                .map_err(|_| PrivateSettlementGlobalStateErrorV1::Receipt)?;
            if bundle_id != &abort.bundle_id || self.receipts.contains_key(bundle_id) {
                return Err(PrivateSettlementGlobalStateErrorV1::Receipt);
            }
        }
        Ok(())
    }
}

/// Redacted global-state failure safe for consensus diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum PrivateSettlementGlobalStateErrorV1 {
    /// The governed feature is disabled in the consensus configuration.
    #[error("private-settlement is disabled")]
    Disabled,
    /// The carrier or governance operation precedes governed activation.
    #[error("private-settlement is not active at this height")]
    Activation,
    /// The configured node capability cannot enforce the V1 proof/policy profile.
    #[error("private-settlement capability is unavailable")]
    Capability,
    /// Network identity differs from the local consensus security domain.
    #[error("private-settlement network binding is invalid")]
    Network,
    /// Participant, expiry, or encoded carrier bounds exceed consensus configuration.
    #[error("private-settlement consensus bounds are invalid")]
    Bounds,
    /// Restricted pool governance is missing, malformed, or inactive.
    #[error("private-settlement pool governance is invalid")]
    Governance,
    /// The pool state or caller-selected successor is invalid.
    #[error("private-settlement pool transition is invalid")]
    Pool,
    /// The finalized receipt or its phase certificates are invalid.
    #[error("private-settlement receipt is invalid")]
    Receipt,
    /// State application height differs from the receipt.
    #[error("private-settlement finalization height is invalid")]
    Height,
    /// A nullifier, output, root, or pool conflicts with durable state.
    #[error("private-settlement state conflict")]
    Conflict,
    /// The bundle already has the opposite terminal marker.
    #[error("private-settlement bundle is terminal")]
    Terminal,
    /// A replay attempted to replace existing evidence.
    #[error("private-settlement substitution was rejected")]
    Substitution,
    /// Canonical receipt encoding failed.
    #[error("private-settlement canonical encoding failed")]
    Encoding,
}

impl PrivateSettlementGlobalStateErrorV1 {
    fn from_pool(_: PrivateSettlementStateErrorV1) -> Self {
        Self::Pool
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::private_settlement::{
        protocol::{
            aggregate_private_settlement_phase_votes_v1, private_settlement_phase_body_v1,
            private_settlement_prepared_bundle_digest_v1,
            private_settlement_reserved_prepared_bundle_digest_v1,
            sign_private_settlement_phase_vote_v1,
        },
        sidecar_store::tests::{SidecarFixtureV1, sidecar_fixture},
    };
    use iroha_crypto::{HashOf, KeyPair};
    use iroha_data_model::{
        nexus::{
            ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, PrivateSettlementAuditPolicyV1,
            PrivateSettlementCommitteeAuthorityV1, PrivateSettlementDeltaV1,
            PrivateSettlementLegReceiptV1, PrivateSettlementPhaseCertificateV1,
            PrivateSettlementPhaseV1, PrivateSettlementPoolGovernanceLifecycleV1,
        },
        peer::PeerId,
    };

    fn certificate(
        manifest: &iroha_data_model::nexus::AtomicPrivateSettlementV1,
        delta: &PrivateSettlementDeltaV1,
        authority: &PrivateSettlementCommitteeAuthorityV1,
        keys: &[KeyPair],
        phase: PrivateSettlementPhaseV1,
        prepared_bundle_digest: Hash,
    ) -> PrivateSettlementPhaseCertificateV1 {
        let body = private_settlement_phase_body_v1(
            manifest,
            delta,
            authority,
            phase,
            prepared_bundle_digest,
        )
        .expect("phase body");
        let votes = keys[..3]
            .iter()
            .map(|key| sign_private_settlement_phase_vote_v1(body, key).expect("phase vote"))
            .collect::<Vec<_>>();
        aggregate_private_settlement_phase_votes_v1(body, delta.leg_ordinal, authority, &votes)
            .expect("phase certificate")
    }

    fn recertify_receipt_with_validator_keys(
        mut receipt: PrivateSettlementReceiptV1,
        validator_keys: &[KeyPair],
    ) -> PrivateSettlementReceiptV1 {
        let mut ordered_keys = validator_keys.to_vec();
        ordered_keys.sort_by(|left, right| {
            let left_peer = PeerId::from(left.public_key().clone());
            let right_peer = PeerId::from(right.public_key().clone());
            iroha_data_model::account::AccountId::new(left.public_key().clone())
                .cmp(&iroha_data_model::account::AccountId::new(
                    right.public_key().clone(),
                ))
                .then_with(|| left_peer.cmp(&right_peer))
        });
        let validators = ordered_keys
            .iter()
            .map(|key| PeerId::from(key.public_key().clone()))
            .collect::<Vec<_>>();
        let validator_pops = ordered_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("validator PoP")
            })
            .collect::<Vec<_>>();
        let authorities = receipt
            .manifest
            .legs
            .iter()
            .map(|leg| PrivateSettlementCommitteeAuthorityV1 {
                route: leg.route,
                validator_set_hash: HashOf::new(&validators),
                validators: validators.clone(),
                validator_pops: validator_pops.clone(),
            })
            .collect::<Vec<_>>();
        let deltas = receipt
            .legs
            .iter()
            .map(|leg| leg.delta.clone())
            .collect::<Vec<_>>();
        let prepares = deltas
            .iter()
            .zip(&authorities)
            .map(|(delta, authority)| {
                certificate(
                    &receipt.manifest,
                    delta,
                    authority,
                    &ordered_keys,
                    PrivateSettlementPhaseV1::Prepare,
                    private_settlement_reserved_prepared_bundle_digest_v1(),
                )
            })
            .collect::<Vec<_>>();
        let prepared_bundle_digest = private_settlement_prepared_bundle_digest_v1(
            &receipt.manifest,
            &authorities,
            &deltas,
            &prepares,
        )
        .expect("prepared bundle digest");
        receipt.legs = deltas
            .iter()
            .zip(&authorities)
            .zip(prepares)
            .map(
                |((delta, authority), prepare)| PrivateSettlementLegReceiptV1 {
                    delta: delta.clone(),
                    prepare,
                    commit: certificate(
                        &receipt.manifest,
                        delta,
                        authority,
                        &ordered_keys,
                        PrivateSettlementPhaseV1::Commit,
                        prepared_bundle_digest,
                    ),
                },
            )
            .collect();
        receipt.authority_catalog = authorities;
        verify_private_settlement_receipt_v1(&receipt).expect("recertified receipt");
        receipt
    }

    fn private_settlement_transaction_bytes(
        transaction: &crate::state::StateTransaction<'_, '_>,
    ) -> Vec<u8> {
        let state = PrivateSettlementGlobalStateV1 {
            governance: transaction
                .world
                .private_settlement_governance
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            pools: transaction
                .world
                .private_settlement_pools
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            roots: transaction
                .world
                .private_settlement_roots
                .iter()
                .map(|(key, value)| (*key, *value))
                .collect(),
            nullifiers: transaction
                .world
                .private_settlement_nullifiers
                .iter()
                .map(|(key, value)| (*key, *value))
                .collect(),
            outputs: transaction
                .world
                .private_settlement_outputs
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            receipts: transaction
                .world
                .private_settlement_receipts
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            aborts: transaction
                .world
                .private_settlement_aborts
                .iter()
                .map(|(key, value)| (*key, *value))
                .collect(),
        };
        norito::encode_canonical(&state).expect("private-settlement transaction state encodes")
    }

    fn install_private_settlement_authority_fixture(
        state: &crate::state::State,
        receipt: &PrivateSettlementReceiptV1,
        validator_keys: &[KeyPair],
    ) {
        use crate::{
            governance::manifest::{
                GovernanceRules, LaneManifestRegistry, LaneManifestStatus, ManifestValidatorBinding,
            },
            state::derive_validator_key_id,
        };
        use iroha_data_model::{
            account::AccountId,
            consensus::{ConsensusKeyRecord, ConsensusKeyStatus},
            nexus::{LaneStorageProfile, LaneVisibility},
        };
        use std::{collections::BTreeMap, sync::Arc};

        let mut world = state.world.block();
        {
            let mut peers = world.peers_mut_for_testing().transaction();
            for key in validator_keys {
                let peer = PeerId::from(key.public_key().clone());
                if !peers.iter().any(|existing| existing == &peer) {
                    peers.push(peer);
                }
            }
            peers.apply();
        }
        for key in validator_keys {
            let id = derive_validator_key_id(key.public_key());
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: key.public_key().clone(),
                pop: Some(
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("authority fixture PoP"),
                ),
                activation_height: 0,
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            world.consensus_keys.insert(id, record.clone());
            world
                .consensus_keys_by_pk
                .insert(record.public_key.to_string(), vec![record.id]);
        }
        world.commit();

        let validators = validator_keys
            .iter()
            .map(|key| AccountId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let mut statuses = BTreeMap::new();
        for leg in &receipt.manifest.legs {
            let validator_bindings = validators
                .iter()
                .map(|validator| ManifestValidatorBinding {
                    validator: validator.clone(),
                    peer_id: PeerId::from(
                        validator
                            .try_signatory()
                            .expect("fixture validators are single-signatory")
                            .clone(),
                    ),
                    torii_url: None,
                })
                .collect();
            statuses.insert(
                leg.route.lane_id,
                LaneManifestStatus {
                    lane: leg.route.lane_id,
                    alias: format!("private-settlement-{}", leg.route.lane_id.as_u32()),
                    dataspace: leg.route.dataspace_id,
                    visibility: LaneVisibility::Public,
                    storage: LaneStorageProfile::FullReplica,
                    governance: Some("parliament".to_owned()),
                    manifest_path: Some(std::path::PathBuf::from(
                        "/tmp/private-settlement-authority.json",
                    )),
                    governance_rules: Some(GovernanceRules {
                        validators: validators.clone(),
                        validator_bindings,
                        ..GovernanceRules::default()
                    }),
                    privacy_commitments: Vec::new(),
                },
            );
        }
        state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));
    }

    pub(crate) fn fixture() -> (
        PrivateSettlementGlobalStateV1,
        PrivateSettlementReceiptV1,
        SidecarFixtureV1,
    ) {
        let fixture = sidecar_fixture();
        let mut manifest = fixture.sidecar.manifest.clone();
        let first_governance = fixture.pool_governance.clone();
        let mut second_policy_body = fixture.sidecar.policy.body.clone();
        second_policy_body.dataspace_id = manifest.legs[1].route.dataspace_id;
        second_policy_body.policy_id = Hash::new(b"second-dataspace-audit-policy");
        let second_policy =
            PrivateSettlementAuditPolicyV1::new(second_policy_body).expect("second audit policy");
        let second_governance = PrivateSettlementPoolGovernanceV1::from_restricted_mapping(
            manifest.legs[1].route,
            manifest.legs[1].pool_id,
            first_governance.body.asset_definition_id.clone(),
            [0x4B; 32],
            &second_policy,
            PrivateSettlementPoolGovernanceLifecycleV1 {
                governance_revision: 1,
                activation_height: 5,
                retirement_height: Some(500),
            },
        )
        .expect("second pool governance");
        manifest.legs[1].asset_binding_commitment = second_governance.body.asset_binding_commitment;
        manifest.legs[1].audit_policy_digest = second_policy.policy_digest;
        manifest.bundle_id = manifest.computed_bundle_id().expect("bundle id");

        let initial_commitments = [PrivacyCommitmentV1::new([0xE1; 32])];
        let first_pool = PrivateSettlementPoolStateV1::bootstrap(
            first_governance.body.route,
            first_governance.body.pool_id,
            first_governance.governance_digest,
            &initial_commitments,
        )
        .expect("first pool");
        let second_pool = PrivateSettlementPoolStateV1::bootstrap(
            second_governance.body.route,
            second_governance.body.pool_id,
            second_governance.governance_digest,
            &initial_commitments,
        )
        .expect("second pool");
        let mut deltas = vec![
            fixture.sidecar.payload.delta.clone(),
            fixture.sidecar.payload.delta.clone(),
        ];
        for (index, pool) in [&first_pool, &second_pool].into_iter().enumerate() {
            let manifest_leg = manifest.legs[index].clone();
            let delta = &mut deltas[index];
            delta.bundle_id = manifest.bundle_id;
            delta.leg_ordinal = u8::try_from(index).expect("fixture ordinal");
            delta.route = manifest_leg.route;
            delta.pool_id = manifest_leg.pool_id;
            delta.asset_binding_commitment = manifest_leg.asset_binding_commitment;
            delta.audit_policy_digest = manifest_leg.audit_policy_digest;
            delta.audit_key_epoch = if index == 0 {
                fixture.sidecar.policy.body.key_epoch
            } else {
                second_policy.body.key_epoch
            };
            delta.old_epoch = pool.epoch();
            delta.old_root = pool.root();
            let successor = pool
                .successor(&delta.output_commitments)
                .expect("successor");
            delta.new_epoch = successor.epoch;
            delta.new_root = successor.root;
            manifest.legs[index].delta_digest = delta.digest().expect("delta digest");
        }
        manifest.validate().expect("manifest");

        let validators = fixture
            .validator_keys
            .iter()
            .map(|key| PeerId::from(key.public_key().clone()))
            .collect::<Vec<_>>();
        let first_authority = fixture.sidecar.authority.clone();
        let second_authority = PrivateSettlementCommitteeAuthorityV1 {
            route: manifest.legs[1].route,
            validator_set_hash: HashOf::new(&validators),
            validators,
            validator_pops: fixture
                .validator_keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("validator PoP")
                })
                .collect(),
        };
        let authorities = vec![first_authority, second_authority];
        let prepares = deltas
            .iter()
            .zip(&authorities)
            .map(|(delta, authority)| {
                certificate(
                    &manifest,
                    delta,
                    authority,
                    &fixture.validator_keys,
                    PrivateSettlementPhaseV1::Prepare,
                    private_settlement_reserved_prepared_bundle_digest_v1(),
                )
            })
            .collect::<Vec<_>>();
        let prepared_bundle_digest = private_settlement_prepared_bundle_digest_v1(
            &manifest,
            &authorities,
            &deltas,
            &prepares,
        )
        .expect("prepared bundle digest");
        let legs = deltas
            .iter()
            .zip(&authorities)
            .zip(prepares)
            .map(
                |((delta, authority), prepare)| PrivateSettlementLegReceiptV1 {
                    delta: delta.clone(),
                    prepare,
                    commit: certificate(
                        &manifest,
                        delta,
                        authority,
                        &fixture.validator_keys,
                        PrivateSettlementPhaseV1::Commit,
                        prepared_bundle_digest,
                    ),
                },
            )
            .collect();
        let receipt = PrivateSettlementReceiptV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            manifest,
            authority_catalog: authorities,
            legs,
            finalized_height: 20,
        };
        verify_private_settlement_receipt_v1(&receipt).expect("receipt");
        let mut state = PrivateSettlementGlobalStateV1::default();
        state
            .bootstrap_pool(first_governance, &initial_commitments, 5)
            .expect("first bootstrap");
        state
            .bootstrap_pool(second_governance, &initial_commitments, 5)
            .expect("second bootstrap");
        (state, receipt, fixture)
    }

    #[test]
    fn valid_receipt_advances_every_leg_exactly_once() {
        let (mut state, receipt, _) = fixture();
        let old_heads = receipt
            .legs
            .iter()
            .map(|leg| {
                state
                    .pool_head(leg.delta.route, leg.delta.pool_id)
                    .expect("old head")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            state.apply_receipt(receipt.clone(), 20),
            Ok(PrivateSettlementGlobalStateOutcomeV1::Applied)
        );
        for (index, leg) in receipt.legs.iter().enumerate() {
            assert_eq!(
                state.pool_head(leg.delta.route, leg.delta.pool_id),
                Some((leg.delta.new_epoch, leg.delta.new_root))
            );
            assert_ne!(old_heads[index], (leg.delta.new_epoch, leg.delta.new_root));
        }
        assert_eq!(state.nullifiers.len(), 4);
        assert_eq!(state.outputs.len(), 6);
        assert_eq!(state.receipt(&receipt.manifest.bundle_id), Some(&receipt));
        state.validate().expect("restored state validates");
        assert_eq!(
            state.apply_receipt(receipt, 20),
            Ok(PrivateSettlementGlobalStateOutcomeV1::Idempotent)
        );
    }

    #[test]
    fn policy_rotation_preserves_frontier_and_rejects_old_policy_bundles() {
        let (mut state, old_policy_receipt, _) = fixture();
        let key = PrivateSettlementPoolKeyV1::new(
            old_policy_receipt.legs[0].delta.route,
            old_policy_receipt.legs[0].delta.pool_id,
        )
        .expect("pool key");
        let current = state.governance.get(&key).expect("governance").clone();
        let old_head = state.pool_head(key.route, key.pool_id).expect("head");
        let old_roots = state.roots.clone();
        let old_nullifiers = state.nullifiers.clone();
        let old_outputs = state.outputs.clone();
        let old_receipts = state.receipts.clone();
        let mut replacement = current.clone();
        replacement.audit_policy_digest = Hash::new(b"rotated audit policy");
        replacement.audit_key_epoch += 1;
        replacement.lifecycle.governance_revision += 1;
        replacement.lifecycle.activation_height = 20;
        replacement.governance_digest = Hash::new(b"rotated restricted governance");

        assert_eq!(
            state.rotate_pool_policy(current.governance_digest, replacement.clone(), 20),
            Ok(PrivateSettlementGlobalStateOutcomeV1::Applied)
        );
        assert_eq!(state.pool_head(key.route, key.pool_id), Some(old_head));
        assert_eq!(state.roots, old_roots);
        assert_eq!(state.nullifiers, old_nullifiers);
        assert_eq!(state.outputs, old_outputs);
        assert_eq!(state.receipts, old_receipts);
        state
            .validate()
            .expect("rotated state validates after restart");
        assert_eq!(
            state.rotate_pool_policy(current.governance_digest, replacement, 20),
            Ok(PrivateSettlementGlobalStateOutcomeV1::Idempotent)
        );

        let before_old_bundle = norito::encode_canonical(&state).expect("state bytes");
        assert!(state.apply_receipt(old_policy_receipt, 20).is_err());
        assert_eq!(
            norito::encode_canonical(&state).expect("state bytes"),
            before_old_bundle,
            "a bundle spanning the policy boundary must not mutate any map"
        );
    }

    #[test]
    fn policy_rotation_retains_finalized_history_across_restart_and_replay() {
        let (mut state, receipt, _) = fixture();
        assert_eq!(
            state.apply_receipt(receipt.clone(), receipt.finalized_height),
            Ok(PrivateSettlementGlobalStateOutcomeV1::Applied)
        );
        let key = PrivateSettlementPoolKeyV1::new(
            receipt.legs[0].delta.route,
            receipt.legs[0].delta.pool_id,
        )
        .expect("pool key");
        let current = state.governance.get(&key).expect("governance").clone();
        let old_head = state.pool_head(key.route, key.pool_id).expect("head");
        let mut replacement = current.clone();
        replacement.audit_policy_digest = Hash::new(b"post-finality audit policy");
        replacement.audit_key_epoch += 1;
        replacement.lifecycle.governance_revision += 1;
        replacement.lifecycle.activation_height = receipt.finalized_height + 1;
        replacement.governance_digest = Hash::new(b"post-finality governance");

        assert_eq!(
            state.rotate_pool_policy(
                current.governance_digest,
                replacement,
                receipt.finalized_height + 1,
            ),
            Ok(PrivateSettlementGlobalStateOutcomeV1::Applied)
        );
        assert_eq!(state.pool_head(key.route, key.pool_id), Some(old_head));
        let persisted = norito::encode_canonical(&state).expect("state encodes");
        let mut restored: PrivateSettlementGlobalStateV1 =
            norito::decode_from_bytes(&persisted).expect("state decodes");
        restored
            .validate()
            .expect("pre-rotation receipt remains valid after restart");
        let world = world_from_private_settlement_state(restored.clone());
        validate_private_settlement_persisted_state_v1(
            &world.private_settlement_governance.view(),
            &world.private_settlement_pools.view(),
            &world.private_settlement_roots.view(),
            &world.private_settlement_nullifiers.view(),
            &world.private_settlement_outputs.view(),
            &world.private_settlement_receipts.view(),
            &world.private_settlement_aborts.view(),
        )
        .expect("WSV recovery accepts the anchored historical policy revision");
        let rotated = restored.governance.get(&key).expect("rotated governance");
        assert_eq!(rotated.prior_revisions.len(), 1);
        assert_eq!(
            rotated.prior_revisions[0].governance_digest,
            current.governance_digest
        );
        assert_eq!(
            restored.apply_receipt(receipt.clone(), receipt.finalized_height),
            Ok(PrivateSettlementGlobalStateOutcomeV1::Idempotent)
        );
        assert_eq!(
            norito::encode_canonical(&restored).expect("state re-encodes"),
            persisted,
            "exact replay after rotation must leave every state byte unchanged"
        );
    }

    #[test]
    fn policy_rotation_rejects_a_same_height_pool_finalization() {
        let (mut state, receipt, _) = fixture();
        assert_eq!(
            state.apply_receipt(receipt.clone(), receipt.finalized_height),
            Ok(PrivateSettlementGlobalStateOutcomeV1::Applied)
        );
        let key = PrivateSettlementPoolKeyV1::new(
            receipt.legs[0].delta.route,
            receipt.legs[0].delta.pool_id,
        )
        .expect("pool key");
        let current = state.governance.get(&key).expect("governance").clone();
        let mut replacement = current.clone();
        replacement.audit_policy_digest = Hash::new(b"same-height audit policy");
        replacement.audit_key_epoch += 1;
        replacement.lifecycle.governance_revision += 1;
        replacement.lifecycle.activation_height = receipt.finalized_height;
        replacement.governance_digest = Hash::new(b"same-height governance");
        let before = norito::encode_canonical(&state).expect("state bytes");

        assert_eq!(
            state.rotate_pool_policy(
                current.governance_digest,
                replacement,
                receipt.finalized_height,
            ),
            Err(PrivateSettlementGlobalStateErrorV1::Governance)
        );
        assert_eq!(
            norito::encode_canonical(&state).expect("state bytes"),
            before
        );
    }

    #[test]
    fn policy_rotation_rejects_stale_rollback_and_asset_substitution_without_mutation() {
        let (mut state, receipt, _) = fixture();
        let key = PrivateSettlementPoolKeyV1::new(
            receipt.legs[0].delta.route,
            receipt.legs[0].delta.pool_id,
        )
        .expect("pool key");
        let current = state.governance.get(&key).expect("governance").clone();
        let mut replacement = current.clone();
        replacement.audit_policy_digest = Hash::new(b"new policy");
        replacement.audit_key_epoch += 1;
        replacement.lifecycle.governance_revision += 1;
        replacement.lifecycle.activation_height = 20;
        replacement.governance_digest = Hash::new(b"new governance");

        for invalid in [
            PrivateSettlementPoolGovernanceProjectionV1 {
                audit_key_epoch: current.audit_key_epoch,
                ..replacement.clone()
            },
            PrivateSettlementPoolGovernanceProjectionV1 {
                lifecycle: current.lifecycle,
                ..replacement.clone()
            },
            PrivateSettlementPoolGovernanceProjectionV1 {
                asset_binding_commitment: Hash::new(b"substituted asset binding"),
                ..replacement.clone()
            },
        ] {
            let before = norito::encode_canonical(&state).expect("state bytes");
            assert!(
                state
                    .rotate_pool_policy(current.governance_digest, invalid, 20)
                    .is_err()
            );
            assert_eq!(
                norito::encode_canonical(&state).expect("state bytes"),
                before
            );
        }
        let before = norito::encode_canonical(&state).expect("state bytes");
        assert_eq!(
            state.rotate_pool_policy(Hash::new(b"stale expected digest"), replacement.clone(), 20,),
            Err(PrivateSettlementGlobalStateErrorV1::Substitution)
        );
        assert_eq!(
            norito::encode_canonical(&state).expect("state bytes"),
            before
        );
    }

    #[test]
    fn invalid_later_leg_leaves_all_state_bytes_identical() {
        let (mut state, mut receipt, _) = fixture();
        receipt.legs[1].delta.new_root = PrivacyRootV1::new([0xFF; 32]);
        let before = norito::encode_canonical(&state).expect("state bytes");
        assert!(state.apply_receipt(receipt, 20).is_err());
        let after = norito::encode_canonical(&state).expect("state bytes");
        assert_eq!(before, after);
    }

    #[test]
    fn persisted_state_rejects_duplicate_pool_epoch_roots() {
        let mut world = finalized_world_fixture();
        let (pool, governance_digest, admitted_at_height) = {
            let governance = world.private_settlement_governance.view();
            let (pool, record) = governance.iter().next().expect("governed fixture pool");
            let admitted_at_height = world
                .private_settlement_roots
                .view()
                .iter()
                .find_map(|(root, provenance)| {
                    (root.pool == *pool)
                        .then_some(provenance)
                        .and_then(|provenance| match provenance {
                            PrivateSettlementRootProvenanceV1::Governance {
                                admitted_at_height,
                                ..
                            } => Some(*admitted_at_height),
                            PrivateSettlementRootProvenanceV1::Settlement(_) => None,
                        })
                })
                .expect("governance origin root");
            (*pool, record.governance_digest, admitted_at_height)
        };
        world.private_settlement_roots.insert(
            PrivateSettlementRootKeyV1 {
                pool,
                epoch: 1,
                root: PrivacyRootV1::new([0xD7; 32]),
            },
            PrivateSettlementRootProvenanceV1::Governance {
                governance_digest,
                admitted_at_height,
            },
        );

        let error = validate_private_settlement_persisted_state_v1(
            &world.private_settlement_governance.view(),
            &world.private_settlement_pools.view(),
            &world.private_settlement_roots.view(),
            &world.private_settlement_nullifiers.view(),
            &world.private_settlement_outputs.view(),
            &world.private_settlement_receipts.view(),
            &world.private_settlement_aborts.view(),
        )
        .expect_err("two roots at one pool epoch must fail closed");
        assert_eq!(error, PrivateSettlementGlobalStateErrorV1::Conflict);
    }

    #[test]
    fn world_view_pool_head_requires_retained_root_provenance() {
        let world = finalized_world_fixture();
        let (pool_key, epoch, root) = {
            let pools = world.private_settlement_pools.view();
            let (pool_key, pool) = pools.iter().next().expect("governed fixture pool");
            (*pool_key, pool.epoch(), pool.root())
        };
        assert_eq!(
            world
                .view()
                .private_settlement_pool_head_v1(pool_key.route, pool_key.pool_id),
            Some((epoch, root))
        );

        {
            let mut roots = world.private_settlement_roots.block();
            roots.remove(PrivateSettlementRootKeyV1 {
                pool: pool_key,
                epoch,
                root,
            });
            roots.commit();
        }

        assert_eq!(
            world
                .view()
                .private_settlement_pool_head_v1(pool_key.route, pool_key.pool_id),
            None,
            "an unproven pool frontier must not be reported"
        );
    }

    #[test]
    fn persisted_state_rejects_receipt_disconnected_from_governance_origin() {
        let mut world = finalized_world_fixture();
        let (origin_key, origin_provenance) = {
            let roots = world.private_settlement_roots.view();
            roots
                .iter()
                .find_map(|(key, provenance)| {
                    matches!(
                        provenance,
                        PrivateSettlementRootProvenanceV1::Governance { .. }
                    )
                    .then_some((*key, *provenance))
                })
                .expect("governance origin root")
        };
        {
            let mut roots = world.private_settlement_roots.block();
            roots.remove(origin_key);
            roots.commit();
        }
        world.private_settlement_roots.insert(
            PrivateSettlementRootKeyV1 {
                root: PrivacyRootV1::new([0xD8; 32]),
                ..origin_key
            },
            origin_provenance,
        );

        let error = validate_private_settlement_persisted_state_v1(
            &world.private_settlement_governance.view(),
            &world.private_settlement_pools.view(),
            &world.private_settlement_roots.view(),
            &world.private_settlement_nullifiers.view(),
            &world.private_settlement_outputs.view(),
            &world.private_settlement_receipts.view(),
            &world.private_settlement_aborts.view(),
        )
        .expect_err("a certified target without its exact old root must fail closed");
        assert_eq!(error, PrivateSettlementGlobalStateErrorV1::Conflict);
    }

    #[test]
    fn abort_marker_excludes_later_finalization() {
        let (mut state, receipt, _) = fixture();
        let abort = PrivateSettlementAbortReceiptV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: receipt.manifest.network_id.clone(),
            bundle_id: receipt.manifest.bundle_id,
            manifest_digest: receipt.manifest.manifest_digest().expect("manifest digest"),
            finalized_height: 19,
            reason: iroha_data_model::nexus::PrivateSettlementAbortReasonV1::ParticipantRejected,
        };
        assert_eq!(
            state.apply_abort(abort, 19),
            Ok(PrivateSettlementGlobalStateOutcomeV1::Applied)
        );
        let before = norito::encode_canonical(&state).expect("state bytes");
        assert_eq!(
            state.apply_receipt(receipt, 20),
            Err(PrivateSettlementGlobalStateErrorV1::Terminal)
        );
        assert_eq!(
            before,
            norito::encode_canonical(&state).expect("state bytes")
        );
    }

    pub(crate) fn world_from_private_settlement_state(
        state: PrivateSettlementGlobalStateV1,
    ) -> crate::state::World {
        let PrivateSettlementGlobalStateV1 {
            governance,
            pools,
            roots,
            nullifiers,
            outputs,
            receipts,
            aborts,
        } = state;
        let mut world = crate::state::World::default();
        world.private_settlement_governance = mv::storage::Storage::from_iter(governance);
        world.private_settlement_pools = mv::storage::Storage::from_iter(pools);
        world.private_settlement_roots = mv::storage::Storage::from_iter(roots);
        world.private_settlement_nullifiers = mv::storage::Storage::from_iter(nullifiers);
        world.private_settlement_outputs = mv::storage::Storage::from_iter(outputs);
        world.private_settlement_receipts = mv::storage::Storage::from_iter(receipts);
        world.private_settlement_aborts = mv::storage::Storage::from_iter(aborts);
        world
    }

    pub(crate) fn finalized_world_fixture() -> crate::state::World {
        let (mut private_state, receipt, _) = fixture();
        private_state
            .apply_receipt(receipt.clone(), receipt.finalized_height)
            .expect("finalize private-settlement fixture");
        let abort = PrivateSettlementAbortReceiptV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: receipt.manifest.network_id,
            bundle_id: Hash::new(b"private-settlement tiered abort fixture"),
            manifest_digest: Hash::new(b"private-settlement tiered abort manifest"),
            finalized_height: receipt.finalized_height + 1,
            reason: iroha_data_model::nexus::PrivateSettlementAbortReasonV1::Expired,
        };
        private_state
            .apply_abort(abort, receipt.finalized_height + 1)
            .expect("persist private-settlement abort fixture");
        world_from_private_settlement_state(private_state)
    }

    #[test]
    fn state_transaction_applies_all_legs_only_at_atomic_commit_boundaries() {
        use crate::{kura::Kura, query::store::LiveQueryStore, state::State};
        use iroha_data_model::{
            ChainId,
            block::BlockHeader,
            nexus::{DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig},
        };
        use std::num::{NonZeroU32, NonZeroU64};

        let (private_state, receipt, sidecar_fixture) = fixture();
        let receipt =
            recertify_receipt_with_validator_keys(receipt, &sidecar_fixture.validator_keys);
        let world = world_from_private_settlement_state(private_state);
        let state = State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("private-settlement-wsv-test"),
            receipt.manifest.network_id.clone(),
        );
        install_private_settlement_authority_fixture(
            &state,
            &receipt,
            &sidecar_fixture.validator_keys,
        );
        let header = BlockHeader::new(
            NonZeroU64::new(receipt.finalized_height).expect("non-zero finalization height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        block.nexus.atomic_private_settlement.enabled = true;
        block.nexus.atomic_private_settlement.activation_height = Some(2);
        block
            .nexus
            .atomic_private_settlement
            .minimum_activation_notice_blocks =
            NonZeroU64::new(2).expect("non-zero settlement activation notice");
        let lane_bound = receipt
            .manifest
            .legs
            .iter()
            .map(|leg| leg.route.lane_id.as_u32())
            .max()
            .expect("receipt has participants")
            .checked_add(1)
            .and_then(NonZeroU32::new)
            .expect("fixture lane bound");
        let lane_catalog = LaneCatalog::new(
            lane_bound,
            receipt
                .manifest
                .legs
                .iter()
                .map(|leg| LaneConfig {
                    id: leg.route.lane_id,
                    dataspace_id: leg.route.dataspace_id,
                    alias: format!("private-settlement-{}", leg.route.lane_id.as_u32()),
                    ..LaneConfig::default()
                })
                .collect(),
        )
        .expect("fixture settlement lane catalog");
        block.nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog);
        block.nexus.lane_catalog = lane_catalog;
        block.nexus.dataspace_catalog = DataSpaceCatalog::new(
            receipt
                .manifest
                .legs
                .iter()
                .map(|leg| DataSpaceMetadata {
                    id: leg.route.dataspace_id,
                    alias: format!("private-settlement-{}", leg.route.dataspace_id.as_u64()),
                    description: None,
                    fault_tolerance: 1,
                })
                .collect(),
        )
        .expect("fixture settlement dataspace catalog");
        block.lane_incarnations = receipt
            .manifest
            .legs
            .iter()
            .map(|leg| (leg.route.lane_id, leg.route.lane_incarnation))
            .collect();
        block.lane_incarnation_activation_heights = receipt
            .manifest
            .legs
            .iter()
            .map(|leg| (leg.route.lane_id, 0))
            .collect();

        let old_heads = receipt
            .legs
            .iter()
            .map(|leg| {
                let key = PrivateSettlementPoolKeyV1::new(leg.delta.route, leg.delta.pool_id)
                    .expect("seeded pool key");
                block
                    .world
                    .private_settlement_pools
                    .get(&key)
                    .map(|pool| (pool.epoch(), pool.root()))
                    .expect("seeded pool head")
            })
            .collect::<Vec<_>>();

        {
            let mut transaction = block.transaction();
            assert_eq!(
                transaction.apply_private_settlement_receipt_v1(receipt.clone()),
                Err(PrivateSettlementGlobalStateErrorV1::Capability),
                "WSV admission must not trust the Torii capability gate"
            );
            assert!(
                transaction
                    .private_settlement_receipt_v1(&receipt.manifest.bundle_id)
                    .is_none()
            );
            for (index, leg) in receipt.legs.iter().enumerate() {
                assert_eq!(
                    transaction.private_settlement_pool_head_v1(leg.delta.route, leg.delta.pool_id),
                    Some(old_heads[index])
                );
            }
        }
        assert!(
            block
                .world
                .private_settlement_receipts
                .get(&receipt.manifest.bundle_id)
                .is_none(),
            "missing compiled capability must leave the parent block byte-clean"
        );

        let protocol_id =
            iroha_data_model::privacy::PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1;
        let profile = crate::privacy_profiles::compiled_privacy_profile_v1(protocol_id)
            .expect("compiled IVM private-note profile");
        let proposed = profile.activation_record(
            iroha_data_model::privacy::PrivacyProtocolLifecycleV1::Proposed(
                iroha_data_model::privacy::PrivacyProposedLifecycleV1 {
                    proposed_at_height: 1,
                    activate_at_height: 2,
                },
            ),
        );
        block.world.privacy_activations.insert(
            crate::privacy_state::PrivacyActivationKeyV1::new(protocol_id),
            proposed,
        );
        {
            let mut transaction = block.transaction();
            assert_eq!(
                transaction.apply_private_settlement_receipt_v1(receipt.clone()),
                Err(PrivateSettlementGlobalStateErrorV1::Activation),
                "a proposed generic capability must not activate the settlement profile"
            );
        }

        let active = profile.activation_record(
            iroha_data_model::privacy::PrivacyProtocolLifecycleV1::Active(
                iroha_data_model::privacy::PrivacyActiveLifecycleV1 {
                    proposed_at_height: 1,
                    activated_at_height: 2,
                    state_since_height: 2,
                },
            ),
        );
        block.world.privacy_activations.insert(
            crate::privacy_state::PrivacyActivationKeyV1::new(protocol_id),
            active,
        );
        {
            let mut transaction = block.transaction();
            assert_eq!(
                transaction.apply_private_settlement_receipt_v1(receipt.clone()),
                Err(PrivateSettlementGlobalStateErrorV1::Activation),
                "configuration activation earlier than the notice window must fail closed"
            );
        }
        block
            .nexus
            .atomic_private_settlement
            .minimum_activation_notice_blocks =
            NonZeroU64::new(1).expect("non-zero settlement activation notice");

        let forged_validator_keys = (0xB1_u8..=0xB4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        let forged_receipt =
            recertify_receipt_with_validator_keys(receipt.clone(), &forged_validator_keys);
        {
            let mut transaction = block.transaction();
            let before = private_settlement_transaction_bytes(&transaction);
            assert_eq!(
                transaction.apply_private_settlement_receipt_v1(forged_receipt),
                Err(PrivateSettlementGlobalStateErrorV1::Receipt),
                "self-certified attacker keys must not authorize global state mutation"
            );
            let after = private_settlement_transaction_bytes(&transaction);
            assert_eq!(
                before, after,
                "forged-authority rejection must leave every private-state map byte-identical"
            );
        }

        let mut invalid = receipt.clone();
        invalid.legs[1].delta.new_root = PrivacyRootV1::new([0xFF; 32]);
        {
            let mut transaction = block.transaction();
            assert!(
                transaction
                    .apply_private_settlement_receipt_v1(invalid)
                    .is_err()
            );
            assert!(
                transaction
                    .private_settlement_receipt_v1(&receipt.manifest.bundle_id)
                    .is_none()
            );
            for (index, leg) in receipt.legs.iter().enumerate() {
                assert_eq!(
                    transaction.private_settlement_pool_head_v1(leg.delta.route, leg.delta.pool_id),
                    Some(old_heads[index])
                );
            }
        }
        assert!(
            block
                .world
                .private_settlement_receipts
                .get(&receipt.manifest.bundle_id)
                .is_none(),
            "dropping a failed StateTransaction must leave its parent block byte-clean"
        );

        {
            let mut transaction = block.transaction();
            assert_eq!(
                transaction.apply_private_settlement_receipt_v1(receipt.clone()),
                Ok(PrivateSettlementGlobalStateOutcomeV1::Applied)
            );
            for leg in &receipt.legs {
                assert_eq!(
                    transaction.private_settlement_pool_head_v1(leg.delta.route, leg.delta.pool_id),
                    Some((leg.delta.new_epoch, leg.delta.new_root))
                );
            }
            transaction.apply();
        }
        assert_eq!(
            block
                .world
                .private_settlement_receipts
                .get(&receipt.manifest.bundle_id),
            Some(&receipt)
        );
        block
            .commit_world_overlay_for_testing()
            .expect("commit private-settlement WorldBlock overlay");
        let committed = state.world.view();
        assert_eq!(
            committed.private_settlement_receipt_v1(&receipt.manifest.bundle_id),
            Some(&receipt)
        );
        for leg in &receipt.legs {
            assert_eq!(
                committed.private_settlement_pool_head_v1(leg.delta.route, leg.delta.pool_id),
                Some((leg.delta.new_epoch, leg.delta.new_root))
            );
        }
    }
}
