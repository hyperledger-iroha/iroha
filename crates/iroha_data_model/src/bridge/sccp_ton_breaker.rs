//! Consensus state for authenticated TON disable-only breaker observations.

use super::{
    SCCP_TON_BASECHAIN_WORKCHAIN_V1, SCCP_TON_MAINNET_GLOBAL_ID_V1, SCCP_TON_MASTERCHAIN_SHARD_V1,
    SCCP_TON_MASTERCHAIN_WORKCHAIN_V1, SCCP_V1_TON_MAX_COINS, SccpLaneIdV1, SccpNetworkV1,
    SccpRouteKeyV1, SccpTonAddressV1, SccpTonMintBreakerGuardianKeysV1,
    canonical_sccp_lane_id_bytes_v1, sccp_lane_id_hash_v1,
};
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Maximum accepted future skew for an authenticated TON block time.
pub const SCCP_TON_BREAKER_MAX_FUTURE_SKEW_MS_V1: u64 = 120_000;
/// Maximum age of an authenticated TON breaker observation at an outbound gate.
pub const SCCP_TON_BREAKER_MAX_AGE_MS_V1: u64 = 900_000;
/// Maximum live entries in each governed TON pending-operation map.
pub const SCCP_TON_PENDING_OPERATION_CAP_V1: u16 = 1_024;

const OBSERVATION_RECORD_DOMAIN_V1: &[u8] = b"iroha:sccp:ton-breaker-observation-record:final-v1";

/// Exact TON block identifier authenticated by native finality or a shard descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonBlockIdExtV1 {
    /// Signed workchain identifier.
    pub workchain: i32,
    /// Full unsigned TON shard identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub shard: u64,
    /// Monotonic block sequence number.
    pub seqno: u32,
    /// TON representation hash of the root cell.
    pub root_hash: [u8; 32],
    /// SHA-256 hash of the canonical block file.
    pub file_hash: [u8; 32],
}

impl SccpTonBlockIdExtV1 {
    /// Return whether this is a nonzero ordinary TON block coordinate.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        self.seqno != 0 && nonzero(&self.root_hash) && nonzero(&self.file_hash)
    }

    /// Return whether this is an exact masterchain coordinate.
    #[must_use]
    pub fn is_masterchain(self) -> bool {
        self.is_well_formed()
            && self.workchain == SCCP_TON_MASTERCHAIN_WORKCHAIN_V1
            && self.shard == SCCP_TON_MASTERCHAIN_SHARD_V1
    }
}

/// Finalized masterchain coordinate and its authenticated UNIX time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonFinalizedMasterchainBlockV1 {
    /// Exact finalized masterchain block.
    pub block_id: SccpTonBlockIdExtV1,
    /// Authenticated `BlockInfo.gen_utime`, in whole UNIX seconds.
    pub gen_utime: u32,
}

impl SccpTonFinalizedMasterchainBlockV1 {
    /// Return the authenticated TON time in milliseconds.
    #[must_use]
    pub const fn gen_utime_ms(self) -> u64 {
        (self.gen_utime as u64) * 1_000
    }

    /// Return whether the coordinate and timestamp are canonical.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        self.block_id.is_masterchain() && self.gen_utime != 0
    }
}

/// Authenticated account-state opening at one finalized shard block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonAccountStateReadbackV1 {
    /// Exact basechain account opened by the proof.
    pub address: SccpTonAddressV1,
    /// Shard block selected from the finalized masterchain shard tree.
    pub shard_block: SccpTonBlockIdExtV1,
    /// Masterchain sequence number recorded by the authenticated shard descriptor.
    pub registered_masterchain_seqno: u32,
    /// Representation hash of the authenticated shard state root.
    pub shard_state_hash: [u8; 32],
    /// Representation hash of the complete account cell.
    pub account_state_hash: [u8; 32],
    /// Representation hash of the active account code cell.
    pub code_hash: [u8; 32],
    /// Representation hash of the complete persistent data cell.
    pub data_hash: [u8; 32],
}

impl SccpTonAccountStateReadbackV1 {
    /// Return whether this readback names one nonzero basechain account state.
    #[must_use]
    pub fn is_well_formed(self, finalized_masterchain_seqno: u32) -> bool {
        self.address.is_sccp_basechain_contract()
            && self.shard_block.is_well_formed()
            && self.shard_block.workchain == SCCP_TON_BASECHAIN_WORKCHAIN_V1
            && self.registered_masterchain_seqno != 0
            && self.registered_masterchain_seqno <= finalized_masterchain_seqno
            && nonzero(&self.shard_state_hash)
            && nonzero(&self.account_state_hash)
            && nonzero(&self.code_hash)
            && nonzero(&self.data_hash)
    }
}

/// Authenticated sparse replay-forest descriptor stored by a TON contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonReplayForestReadbackV1 {
    /// Absent only for the canonical empty forest.
    pub root_hash: Option<[u8; 32]>,
    /// Number of occupied replay leaves.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub leaf_count: u64,
    /// Monotonic update sequence; final V1 requires it to equal the leaf count.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub update_sequence: u64,
}

impl SccpTonReplayForestReadbackV1 {
    /// Return whether empty/root/count invariants are canonical.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        self.update_sequence == self.leaf_count
            && match self.root_hash {
                None => self.leaf_count == 0,
                Some(root) => self.leaf_count != 0 && nonzero(&root),
            }
    }
}

/// Authenticated bridge pending-map roots and bounded live counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonBridgePendingReadbackV1 {
    /// Pending mint dictionary root, absent exactly when its count is zero.
    pub mint_root_hash: Option<[u8; 32]>,
    /// Pending burn dictionary root, absent exactly when its count is zero.
    pub burn_root_hash: Option<[u8; 32]>,
    /// Number of pending mint operations.
    pub mint_count: u16,
    /// Number of pending burn operations.
    pub burn_count: u16,
}

impl SccpTonBridgePendingReadbackV1 {
    /// Return whether both map roots match their bounded counts.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        pending_root_is_well_formed(self.mint_root_hash, self.mint_count)
            && pending_root_is_well_formed(self.burn_root_hash, self.burn_count)
    }
}

/// Complete immutable TON deployment/configuration decoded from both accounts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonDeploymentReadbackV1 {
    /// Exact governed Jetton-master account.
    pub jetton_master_address: SccpTonAddressV1,
    /// Exact governed SCCP route account.
    pub route_address: SccpTonAddressV1,
    /// Exact TON mainnet global id.
    pub expected_global_id: i32,
    /// Immutable SCCP route revision.
    pub route_revision: u32,
    /// Exact Taira-to-Jetton base-unit multiplier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub taira_to_ton_multiplier: u64,
    /// Positive `coins`-domain wrapped supply cap.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u128_string"))]
    pub max_wrapped_supply: u128,
    /// Canonical TON-to-SORA lane bytes stored by both contracts.
    pub source_lane_bytes: Vec<u8>,
    /// Canonical SORA-to-TON lane bytes stored by both contracts.
    pub destination_lane_bytes: Vec<u8>,
    /// Hash of `source_lane_bytes`.
    pub source_lane_hash: [u8; 32],
    /// Hash of `destination_lane_bytes`.
    pub destination_lane_hash: [u8; 32],
    /// Immutable route configuration commitment.
    pub route_configuration_hash: [u8; 32],
    /// Immutable destination deployment binding.
    pub destination_binding_hash: [u8; 32],
    /// TON representation hash of the shared config cell.
    pub bridge_config_cell_hash: [u8; 32],
    /// Jetton master code identity.
    pub jetton_master_code_hash: [u8; 32],
    /// TON representation hash of canonical initial Jetton-master data.
    pub jetton_master_initial_data_hash: [u8; 32],
    /// Jetton wallet code identity.
    pub jetton_wallet_code_hash: [u8; 32],
    /// SCCP route code identity.
    pub route_code_hash: [u8; 32],
    /// TON representation hash of canonical initial SCCP-route data.
    pub route_initial_data_hash: [u8; 32],
    /// Embedded BLS12-381 verifier code identity.
    pub embedded_verifier_code_hash: [u8; 32],
    /// Governed verifier circuit commitment.
    pub verifier_circuit_hash: [u8; 32],
    /// Governed verification-key commitment.
    pub verifying_key_hash: [u8; 32],
    /// TON representation hash of the exact verification-key cell.
    pub verifying_key_cell_hash: [u8; 32],
    /// Governed proof-profile commitment.
    pub proof_profile_commitment: [u8; 32],
    /// Governed semantic proof profile commitment.
    pub semantic_proof_profile_hash: [u8; 32],
    /// Governed SORA finality-anchor commitment.
    pub sora_finality_anchor_hash: [u8; 32],
    /// Exact fixed disable-only guardian set.
    pub mint_breaker_guardian_keys: SccpTonMintBreakerGuardianKeysV1,
    /// TON representation hash of the exact master metadata cell.
    pub master_metadata_hash: [u8; 32],
}

impl SccpTonDeploymentReadbackV1 {
    /// Validate final-V1 network, lane, cap, guardian, and hash-role invariants.
    #[must_use]
    pub fn is_well_formed(&self, route_key: &SccpRouteKeyV1) -> bool {
        let source_lane = SccpLaneIdV1 {
            source: SccpNetworkV1::TonMainnet,
            target: SccpNetworkV1::SoraTaira,
        };
        let destination_lane = SccpLaneIdV1 {
            source: SccpNetworkV1::SoraTaira,
            target: SccpNetworkV1::TonMainnet,
        };
        self.expected_global_id == SCCP_TON_MAINNET_GLOBAL_ID_V1
            && self.jetton_master_address.is_sccp_basechain_contract()
            && self.route_address.is_sccp_basechain_contract()
            && self.jetton_master_address != self.route_address
            && route_key.lane_id == source_lane
            && self.route_revision == route_key.revision
            && self.route_revision != 0
            && self.taira_to_ton_multiplier != 0
            && self.max_wrapped_supply != 0
            && self.max_wrapped_supply <= SCCP_V1_TON_MAX_COINS
            && canonical_sccp_lane_id_bytes_v1(source_lane).as_deref()
                == Some(self.source_lane_bytes.as_slice())
            && canonical_sccp_lane_id_bytes_v1(destination_lane).as_deref()
                == Some(self.destination_lane_bytes.as_slice())
            && sccp_lane_id_hash_v1(source_lane) == Some(self.source_lane_hash)
            && sccp_lane_id_hash_v1(destination_lane) == Some(self.destination_lane_hash)
            && guardians_are_canonical(self.mint_breaker_guardian_keys)
            && all_nonzero(&[
                self.source_lane_hash,
                self.destination_lane_hash,
                self.route_configuration_hash,
                self.destination_binding_hash,
                self.bridge_config_cell_hash,
                self.jetton_master_code_hash,
                self.jetton_master_initial_data_hash,
                self.jetton_wallet_code_hash,
                self.route_code_hash,
                self.route_initial_data_hash,
                self.embedded_verifier_code_hash,
                self.verifier_circuit_hash,
                self.verifying_key_hash,
                self.verifying_key_cell_hash,
                self.proof_profile_commitment,
                self.semantic_proof_profile_hash,
                self.sora_finality_anchor_hash,
                self.master_metadata_hash,
            ])
            && all_distinct(&[
                self.source_lane_hash,
                self.destination_lane_hash,
                self.route_configuration_hash,
                self.destination_binding_hash,
                self.bridge_config_cell_hash,
                self.jetton_master_code_hash,
                self.jetton_master_initial_data_hash,
                self.jetton_wallet_code_hash,
                self.route_code_hash,
                self.route_initial_data_hash,
                self.embedded_verifier_code_hash,
                self.verifier_circuit_hash,
                self.verifying_key_hash,
                self.verifying_key_cell_hash,
                self.proof_profile_commitment,
                self.semantic_proof_profile_hash,
                self.sora_finality_anchor_hash,
                self.master_metadata_hash,
            ])
    }
}

/// Mutable route-account state decoded at the authenticated block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonRouteStorageReadbackV1 {
    /// Exact storage schema version.
    pub storage_version: u8,
    /// Route configuration commitment repeated in the route storage root.
    pub route_configuration_hash: [u8; 32],
    /// Exact shared config-cell hash repeated in the route storage root.
    pub bridge_config_cell_hash: [u8; 32],
    /// Inbound-mint replay state.
    pub inbound_mint_replay: SccpTonReplayForestReadbackV1,
    /// Outbound-burn replay state.
    pub outbound_burn_replay: SccpTonReplayForestReadbackV1,
    /// Pending-operation roots and counts.
    pub pending: SccpTonBridgePendingReadbackV1,
    /// Irreversible route-side mint breaker flag.
    pub minting_disabled: bool,
}

/// Mutable Jetton-master state decoded at the authenticated block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonMasterStorageReadbackV1 {
    /// Exact storage schema version.
    pub storage_version: u8,
    /// Route configuration commitment repeated in the master storage root.
    pub route_configuration_hash: [u8; 32],
    /// Exact shared config-cell hash repeated in the master storage root.
    pub bridge_config_cell_hash: [u8; 32],
    /// Current wrapped Jetton supply.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u128_string"))]
    pub total_supply: u128,
    /// TON representation hash of the metadata cell stored by the master.
    pub metadata_hash: [u8; 32],
    /// Reciprocal route address stored by the master.
    pub route_address: SccpTonAddressV1,
    /// Master-side mint replay state.
    pub mint_replay: SccpTonReplayForestReadbackV1,
    /// Master-side burn replay state.
    pub burn_replay: SccpTonReplayForestReadbackV1,
    /// Pending-mint dictionary root, absent exactly when its count is zero.
    pub pending_mint_root_hash: Option<[u8; 32]>,
    /// Number of pending master-to-wallet mints.
    pub pending_mint_count: u16,
    /// Irreversible master-side mint breaker flag.
    pub minting_disabled: bool,
}

/// Consensus record produced by one canonical, proof-authenticated TON observation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonBreakerObservationRecordV1 {
    /// Exact governed route revision observed.
    pub route_key: SccpRouteKeyV1,
    /// Exact retained native trust anchor that authenticated this observation.
    ///
    /// This nonzero hash is encoded immediately after `route_key` and before
    /// the authenticated masterchain coordinate. Consequently the whole-record
    /// compare-and-swap digest cannot be replayed under a different checkpoint.
    pub authenticated_native_anchor_hash: [u8; 32],
    /// Finalized masterchain block authenticating both account openings.
    pub masterchain: SccpTonFinalizedMasterchainBlockV1,
    /// Route-account state opening.
    pub route_account: SccpTonAccountStateReadbackV1,
    /// Jetton-master account-state opening.
    pub jetton_master_account: SccpTonAccountStateReadbackV1,
    /// Complete immutable configuration decoded identically from both accounts.
    pub deployment: SccpTonDeploymentReadbackV1,
    /// Complete mutable route storage needed by the breaker policy.
    pub route_storage: SccpTonRouteStorageReadbackV1,
    /// Complete mutable master storage needed by the breaker policy.
    pub master_storage: SccpTonMasterStorageReadbackV1,
    /// `route_storage.minting_disabled || master_storage.minting_disabled`.
    pub effective_disabled: bool,
    /// One-way consensus latch for this exact route revision.
    pub disabled_latched: bool,
    /// SHA-256 of the canonical submitted proof bytes.
    pub proof_sha256: [u8; 32],
    /// Canonical submitted proof size charged to SCCP proof limits.
    pub proof_size_bytes: u32,
    /// SORA block height that accepted the observation.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub accepted_at_height: u64,
    /// Consensus SORA block creation time that accepted the observation.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub accepted_at_unix_ms: u64,
    /// Domain-separated digest of the entire record with this field zeroed.
    pub observation_digest: [u8; 32],
}

impl SccpTonBreakerObservationRecordV1 {
    /// Compute the canonical compare-and-swap digest for this complete record.
    #[must_use]
    pub fn computed_digest(&self) -> [u8; 32] {
        let mut canonical = self.clone();
        canonical.observation_digest = [0; 32];
        let mut hasher = Sha256::new();
        hasher.update(OBSERVATION_RECORD_DOMAIN_V1);
        hasher.update(canonical.encode());
        hasher.finalize().into()
    }

    /// Validate every state-local invariant, including the self digest.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        let effective = self.route_storage.minting_disabled || self.master_storage.minting_disabled;
        self.route_key.is_well_formed()
            && nonzero(&self.authenticated_native_anchor_hash)
            && self.masterchain.is_well_formed()
            && self
                .route_account
                .is_well_formed(self.masterchain.block_id.seqno)
            && self
                .jetton_master_account
                .is_well_formed(self.masterchain.block_id.seqno)
            && self.route_account.address != self.jetton_master_account.address
            && self.route_account.address == self.deployment.route_address
            && self.jetton_master_account.address == self.deployment.jetton_master_address
            && self.route_account.code_hash == self.deployment.route_code_hash
            && self.jetton_master_account.code_hash == self.deployment.jetton_master_code_hash
            && self.deployment.is_well_formed(&self.route_key)
            && self.route_storage.storage_version == super::SCCP_V1_TON_STORAGE_VERSION
            && self.master_storage.storage_version == super::SCCP_V1_TON_STORAGE_VERSION
            && self.route_storage.route_configuration_hash
                == self.deployment.route_configuration_hash
            && self.master_storage.route_configuration_hash
                == self.deployment.route_configuration_hash
            && self.route_storage.bridge_config_cell_hash == self.deployment.bridge_config_cell_hash
            && self.master_storage.bridge_config_cell_hash
                == self.deployment.bridge_config_cell_hash
            && self.route_storage.inbound_mint_replay.is_well_formed()
            && self.route_storage.outbound_burn_replay.is_well_formed()
            && self.route_storage.pending.is_well_formed()
            && self.master_storage.mint_replay.is_well_formed()
            && self.master_storage.burn_replay.is_well_formed()
            && pending_root_is_well_formed(
                self.master_storage.pending_mint_root_hash,
                self.master_storage.pending_mint_count,
            )
            && self.master_storage.route_address == self.route_account.address
            && self.master_storage.metadata_hash == self.deployment.master_metadata_hash
            && self.master_storage.total_supply <= self.deployment.max_wrapped_supply
            && self.effective_disabled == effective
            && (self.disabled_latched || !self.effective_disabled)
            && nonzero(&self.proof_sha256)
            && self.proof_size_bytes != 0
            && self.accepted_at_height != 0
            && observation_is_fresh_at(self.masterchain.gen_utime_ms(), self.accepted_at_unix_ms)
            && nonzero(&self.observation_digest)
            && self.observation_digest == self.computed_digest()
    }

    /// Return whether the authenticated TON time is fresh at consensus time `now_ms`.
    #[must_use]
    pub fn is_fresh_at(&self, now_ms: u64) -> bool {
        observation_is_fresh_at(self.masterchain.gen_utime_ms(), now_ms)
    }
}

/// Check the fixed, checked-arithmetic final-V1 time window.
#[must_use]
pub fn observation_is_fresh_at(ton_gen_utime_ms: u64, sora_time_ms: u64) -> bool {
    let Some(max_ton_time) = sora_time_ms.checked_add(SCCP_TON_BREAKER_MAX_FUTURE_SKEW_MS_V1)
    else {
        return false;
    };
    let Some(max_sora_time) = ton_gen_utime_ms.checked_add(SCCP_TON_BREAKER_MAX_AGE_MS_V1) else {
        return false;
    };
    ton_gen_utime_ms <= max_ton_time && sora_time_ms <= max_sora_time
}

fn pending_root_is_well_formed(root: Option<[u8; 32]>, count: u16) -> bool {
    count <= SCCP_TON_PENDING_OPERATION_CAP_V1
        && match root {
            None => count == 0,
            Some(root) => count != 0 && nonzero(&root),
        }
}

fn guardians_are_canonical(keys: SccpTonMintBreakerGuardianKeysV1) -> bool {
    let keys = keys.into_array();
    keys.iter().all(nonzero) && keys.windows(2).all(|pair| pair[0] < pair[1])
}

fn all_nonzero(values: &[[u8; 32]]) -> bool {
    values.iter().all(nonzero)
}

fn all_distinct(values: &[[u8; 32]]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[..index].contains(value))
}

fn nonzero<const N: usize>(value: &[u8; N]) -> bool {
    value.iter().any(|byte| *byte != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_uses_checked_fixed_time_window() {
        let ton = 1_000_000;
        assert!(observation_is_fresh_at(ton, ton));
        assert!(observation_is_fresh_at(ton, ton - 120_000));
        assert!(observation_is_fresh_at(ton, ton + 900_000));
        assert!(!observation_is_fresh_at(ton, ton - 120_001));
        assert!(!observation_is_fresh_at(ton, ton + 900_001));
    }

    #[test]
    fn replay_and_pending_descriptors_are_canonical() {
        assert!(
            SccpTonReplayForestReadbackV1 {
                root_hash: None,
                leaf_count: 0,
                update_sequence: 0,
            }
            .is_well_formed()
        );
        assert!(
            !SccpTonReplayForestReadbackV1 {
                root_hash: Some([1; 32]),
                leaf_count: 0,
                update_sequence: 0,
            }
            .is_well_formed()
        );
        assert!(pending_root_is_well_formed(Some([1; 32]), 1));
        assert!(!pending_root_is_well_formed(
            Some([1; 32]),
            SCCP_TON_PENDING_OPERATION_CAP_V1 + 1,
        ));
    }
}
