//! `World`-related ISI implementations.

use iroha_data_model::smart_contract::manifest::{ContractManifest, ManifestProvenance};
use iroha_telemetry::metrics;

use super::prelude::*;
use crate::{prelude::*, state::WorldTransaction};

/// Iroha Special Instructions that have `World` as their target.
#[allow(clippy::used_underscore_binding)]
pub mod isi {
    use core::{
        convert::{TryFrom, TryInto},
        time::Duration,
    };
    use std::{
        collections::{BTreeMap, BTreeSet},
        str::FromStr,
    };

    use base64::engine::Engine as _;
    use eyre::Result;
    use iroha_crypto::{Hash, Hash as CryptoHash, PublicKey, blake2::Blake2b512};
    // Governance ISIs
    use iroha_data_model::isi::confidential;
    // Bring runtime upgrade ISIs into scope
    use iroha_data_model::isi::runtime_upgrade;
    use iroha_data_model::{
        Level,
        account::AccountController,
        asset::definition::{
            AssetConfidentialPolicy, ConfidentialPolicyMode, ConfidentialPolicyTransition,
        },
        confidential::ConfidentialStatus,
        consensus::{ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus},
        da::pin_intent::DaPinIntentWithLocation,
        events::data::{
            confidential::{
                ConfidentialEvent, ConfidentialShielded, ConfidentialTransferred,
                ConfidentialUnshielded,
            },
            governance::{
                GovernanceEvent, GovernanceParliamentApprovalRecorded, GovernanceSlashReason,
            },
            smart_contract::{
                ContractCodeRegistered, ContractCodeRemoved, ContractInstanceActivated,
                ContractInstanceDeactivated, SmartContractEvent,
            },
        },
        governance::types::{
            AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ParliamentBody,
            ProposalKind,
        },
        isi::{
            bridge, consensus_keys, endorsement,
            error::{InstructionExecutionError, InvalidParameterError, MathError, RepetitionError},
            governance as gov, nexus, smart_contract_code as scode, verifying_keys,
        },
        name::{self, Name},
        nexus::{
            DomainCommittee, DomainEndorsement, DomainEndorsementPolicy, DomainEndorsementRecord,
            LaneRelayEmergencyValidatorSet,
        },
        parameter::Parameter,
        prelude::*,
        proof::{ProofId, VerifyingKeyId, VerifyingKeyRecord},
        query::error::FindError,
        zk::{BackendTag, OpenVerifyEnvelope as ZkOpenVerifyEnvelope},
    };
    use iroha_primitives::{
        numeric::{Numeric, NumericSpec},
        unique_vec::PushResult,
    };
    #[cfg(feature = "telemetry")]
    use iroha_telemetry::metrics::GovernanceManifestActivation;
    use mv::storage::StorageReadOnly;
    use sha2::{Digest as _, Sha256};

    use super::*;
    use crate::{
        governance::selector::derive_parliament_bodies, state::derive_validator_key_id,
        sumeragi::status::PeerKeyPolicyRejectReason, zk::hash_vk,
    };

    fn has_permission(world: &WorldTransaction<'_, '_>, who: &AccountId, name: &str) -> bool {
        world
            .account_permissions
            .get(who)
            .is_some_and(|perms| perms.iter().any(|p| p.name() == name))
    }

    fn validate_consensus_key_record(
        record: &ConsensusKeyRecord,
        sumeragi: &iroha_data_model::parameter::system::SumeragiParameters,
        expected_replaces: Option<&ConsensusKeyId>,
        block_height: u64,
        allow_activation_fast_path: bool,
    ) -> Result<(), Error> {
        if record.activation_height < block_height {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "consensus key activation height cannot be in the past".into(),
                ),
            ));
        }
        let activation_guard = block_height.saturating_add(sumeragi.key_activation_lead_blocks);
        if record.activation_height < activation_guard
            && !(allow_activation_fast_path && record.activation_height == block_height)
        {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "consensus key activation height violates lead-time policy".into(),
                ),
            ));
        }
        if let Some(expiry) = record.expiry_height {
            if expiry <= record.activation_height {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "consensus key expiry must exceed activation height".into(),
                    ),
                ));
            }
        }
        if let Some(expected) = expected_replaces {
            if record.replaces.as_ref() != Some(expected) {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "rotation must reference the key being replaced".into(),
                    ),
                ));
            }
        } else if record.replaces.is_some() {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "new consensus key must not declare a replacement".into(),
                ),
            ));
        }

        let render_list = |values: &[String]| -> String {
            if values.is_empty() {
                "[]".to_owned()
            } else {
                format!("[{}]", values.join(", "))
            }
        };

        let mut allowed_algorithms: Vec<String> = sumeragi
            .key_allowed_algorithms
            .iter()
            .map(ToString::to_string)
            .collect();
        allowed_algorithms.sort();
        allowed_algorithms.dedup();

        let algo = record.public_key.algorithm();
        if !sumeragi.key_allowed_algorithms.contains(&algo) {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "consensus key algorithm {algo} is not allowed; allowed: {}",
                    render_list(&allowed_algorithms)
                )),
            ));
        }

        if sumeragi.key_require_hsm && record.hsm.is_none() {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "HSM binding required for consensus key".into(),
                ),
            ));
        }

        let mut allowed_hsm_providers: Vec<String> = sumeragi.key_allowed_hsm_providers.clone();
        allowed_hsm_providers.sort();
        allowed_hsm_providers.dedup();
        if let Some(hsm) = &record.hsm {
            if !sumeragi
                .key_allowed_hsm_providers
                .iter()
                .any(|provider| provider == &hsm.provider)
            {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "HSM provider {} is not allowed; allowed providers: {}",
                        hsm.provider,
                        render_list(&allowed_hsm_providers)
                    )),
                ));
            }
        }

        if matches!(record.status, ConsensusKeyStatus::Disabled) {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "cannot register a consensus key with Disabled status".into(),
                ),
            ));
        }

        Ok(())
    }

    fn upsert_consensus_key(
        world: &mut WorldTransaction<'_, '_>,
        id: &ConsensusKeyId,
        record: ConsensusKeyRecord,
    ) {
        let pk = record.public_key.to_string();
        world.consensus_keys.insert(id.clone(), record);

        let mut by_pk = world
            .consensus_keys_by_pk
            .get(&pk)
            .cloned()
            .unwrap_or_default();
        if !by_pk.contains(id) {
            by_pk.push(id.clone());
            world.consensus_keys_by_pk.insert(pk, by_pk);
        }
    }

    fn decode_open_verify_envelope(
        proof: &iroha_data_model::proof::ProofBox,
    ) -> Option<ZkOpenVerifyEnvelope> {
        let backend = proof.backend.as_str();
        if !backend.starts_with("halo2/") {
            return None;
        }
        norito::decode_from_bytes::<ZkOpenVerifyEnvelope>(&proof.bytes).ok()
    }

    fn voting_asset_ids(
        gov: &iroha_config::parameters::actual::Governance,
        owner: &AccountId,
    ) -> (AssetId, AssetId) {
        let def_id = gov.voting_asset_id.clone();
        (
            AssetId::new(def_id.clone(), owner.clone()),
            AssetId::new(def_id, gov.bond_escrow_account.clone()),
        )
    }

    fn citizenship_asset_ids(
        gov: &iroha_config::parameters::actual::Governance,
        owner: &AccountId,
    ) -> (AssetId, AssetId) {
        let def_id = gov.citizenship_asset_id.clone();
        (
            AssetId::new(def_id.clone(), owner.clone()),
            AssetId::new(def_id, gov.citizenship_escrow_account.clone()),
        )
    }

    const DEFAULT_NULLIFIER_DOMAIN_TAG: &str = "iroha:gov:nullifier:v1";

    fn citizen_registry_root(world: &WorldTransaction<'_, '_>) -> [u8; 32] {
        // Deterministic Merkle root over bonded citizens (sorted by account id)
        let mut entries: Vec<_> = world
            .citizens
            .iter()
            .map(|(id, record)| (id.clone(), record.clone()))
            .collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));
        if entries.is_empty() {
            return [0u8; 32];
        }
        let mut leaves = Vec::with_capacity(entries.len());
        for (id, record) in entries {
            let mut leaf = Vec::new();
            leaf.extend_from_slice(b"iroha:gov:citizen:v1|");
            leaf.extend_from_slice(id.to_string().as_bytes());
            leaf.push(b'|');
            leaf.extend_from_slice(&record.amount.to_be_bytes());
            leaf.push(b'|');
            leaf.extend_from_slice(&record.bonded_height.to_be_bytes());
            let digest = Sha256::digest(&leaf);
            leaves.push(digest.into());
        }
        let tree = iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves);
        tree.root().map_or([0u8; 32], |h| *h.as_ref())
    }

    fn numeric_with_spec(amount: u128, spec: NumericSpec) -> Result<Numeric, Error> {
        let scale = spec.scale().unwrap_or(0);
        Numeric::try_new(amount, scale).map_err(|_| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                "bond amount exceeds numeric scale".into(),
            ))
        })
    }

    fn reset_citizen_epoch(record: &mut crate::state::CitizenshipRecord, epoch: u64) {
        if record.last_epoch_seen != epoch {
            record.last_epoch_seen = epoch;
            record.seats_in_epoch = 0;
            record.declines_used = 0;
        }
    }

    fn ensure_citizen_available(
        record: &mut crate::state::CitizenshipRecord,
        epoch: u64,
        current_height: u64,
    ) -> Result<(), Error> {
        reset_citizen_epoch(record, epoch);
        if record.cooldown_until > current_height {
            return Err(InstructionExecutionError::InvariantViolation(
                "citizen is in cooldown for the requested epoch".into(),
            ));
        }
        Ok(())
    }

    fn assign_citizen_seat(
        record: &mut crate::state::CitizenshipRecord,
        epoch: u64,
        current_height: u64,
        cfg: &iroha_config::parameters::actual::CitizenServiceDiscipline,
    ) -> Result<(), Error> {
        reset_citizen_epoch(record, epoch);
        if cfg.max_seats_per_epoch > 0 && record.seats_in_epoch >= cfg.max_seats_per_epoch {
            return Err(InstructionExecutionError::InvariantViolation(
                "citizen seat limit reached for epoch".into(),
            ));
        }
        if record.cooldown_until > current_height {
            return Err(InstructionExecutionError::InvariantViolation(
                "citizen is in cooldown for the requested epoch".into(),
            ));
        }
        record.seats_in_epoch = record.seats_in_epoch.saturating_add(1);
        let cooldown = current_height.saturating_add(cfg.seat_cooldown_blocks);
        record.cooldown_until = record.cooldown_until.max(cooldown);
        Ok(())
    }

    fn slash_citizenship_bond(
        record: &mut crate::state::CitizenshipRecord,
        slash_bps: u16,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<u128, Error> {
        if slash_bps == 0 || record.amount == 0 {
            return Ok(0);
        }
        let slash_amount = record.amount.saturating_mul(u128::from(slash_bps)) / 10_000;
        if slash_amount == 0 {
            return Ok(0);
        }
        let def_id = state_transaction.gov.citizenship_asset_id.clone();
        let escrow_asset_id = iroha_data_model::asset::AssetId::new(
            def_id.clone(),
            state_transaction.gov.citizenship_escrow_account.clone(),
        );
        let receiver_asset_id = iroha_data_model::asset::AssetId::new(
            def_id,
            state_transaction.gov.slash_receiver_account.clone(),
        );
        let spec = state_transaction.numeric_spec_for(escrow_asset_id.definition())?;
        let slash_numeric = numeric_with_spec(slash_amount, spec)?;
        crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with(&slash_numeric, spec)?;
        state_transaction
            .world
            .withdraw_numeric_asset(&escrow_asset_id, &slash_numeric)?;
        state_transaction
            .world
            .deposit_numeric_asset(&receiver_asset_id, &slash_numeric)?;
        record.amount = record.amount.saturating_sub(slash_amount);
        Ok(slash_amount)
    }

    fn required_citizenship_bond_for_role(
        gov: &iroha_config::parameters::actual::Governance,
        role: &str,
    ) -> u128 {
        let multiplier = gov.citizen_service.bond_multiplier_for_role(role).max(1);
        gov.citizenship_bond_amount
            .saturating_mul(u128::from(multiplier))
    }

    fn lock_voting_bond(
        ballot_amount: u128,
        previous_amount: Option<u128>,
        authority: &AccountId,
        referendum_id: &str,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let min_bond = state_transaction.gov.min_bond_amount;
        if min_bond == 0 {
            return Ok(());
        }
        if ballot_amount < min_bond {
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                        referendum_id: referendum_id.to_owned(),
                        reason: "bond amount below minimum".into(),
                    },
                ),
            ));
            return Err(InstructionExecutionError::InvariantViolation(
                "bond amount below minimum".into(),
            ));
        }
        let delta = ballot_amount.saturating_sub(previous_amount.unwrap_or(0));
        if delta == 0 {
            return Ok(());
        }
        let (owner_asset_id, escrow_asset_id) = voting_asset_ids(&state_transaction.gov, authority);
        let spec = state_transaction.numeric_spec_for(owner_asset_id.definition())?;
        let delta_numeric = numeric_with_spec(delta, spec)?;
        crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with(&delta_numeric, spec)?;
        state_transaction
            .world
            .withdraw_numeric_asset(&owner_asset_id, &delta_numeric)?;
        state_transaction
            .world
            .deposit_numeric_asset(&escrow_asset_id, &delta_numeric)?;
        Ok(())
    }

    fn ensure_citizen_for_ballot(
        authority: &AccountId,
        referendum_id: &str,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let required = state_transaction.gov.citizenship_bond_amount;
        if required == 0 {
            return Ok(());
        }
        let is_citizen = state_transaction
            .world
            .citizens
            .get(authority)
            .is_some_and(|rec| rec.amount >= required);
        if is_citizen {
            return Ok(());
        }
        state_transaction.world.emit_events(Some(
            iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                iroha_data_model::events::data::governance::GovernanceBallotRejected {
                    referendum_id: referendum_id.to_owned(),
                    reason: "citizenship bond required".into(),
                },
            ),
        ));
        Err(InstructionExecutionError::InvariantViolation(
            "citizenship bond required".into(),
        ))
    }

    struct GovernanceSlashRequest<'a> {
        referendum_id: &'a str,
        owner: &'a AccountId,
        amount: u128,
        reason: GovernanceSlashReason,
        note: &'a str,
    }

    fn governance_slash_percent(
        referendum_id: &str,
        owner: &AccountId,
        bps: u16,
        reason: GovernanceSlashReason,
        note: &str,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<Option<u128>, Error> {
        let bps = bps.min(10_000);
        if bps == 0 {
            return Ok(None);
        }
        let Some(mut locks) = state_transaction
            .world
            .governance_locks
            .get(referendum_id)
            .cloned()
        else {
            return Ok(None);
        };
        let Some(mut rec) = locks.locks.get(owner).cloned() else {
            return Ok(None);
        };
        let slash_amount = rec.amount.saturating_mul(u128::from(bps)) / 10_000;
        if slash_amount == 0 {
            return Ok(None);
        }
        let request = GovernanceSlashRequest {
            referendum_id,
            owner,
            amount: slash_amount,
            reason,
            note,
        };
        apply_governance_slash(&request, &mut locks, &mut rec, state_transaction)?;
        Ok(Some(slash_amount))
    }

    fn governance_slash_absolute(
        referendum_id: &str,
        owner: &AccountId,
        amount: u128,
        reason: GovernanceSlashReason,
        note: &str,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<u128, Error> {
        if amount == 0 {
            return Ok(0);
        }
        let Some(mut locks) = state_transaction
            .world
            .governance_locks
            .get(referendum_id)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                "referendum not found for governance lock slash".into(),
            ));
        };
        let Some(mut rec) = locks.locks.get(owner).cloned() else {
            return Err(InstructionExecutionError::InvariantViolation(
                "governance lock not found for slash".into(),
            ));
        };
        if amount > rec.amount {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract("slash amount exceeds locked balance".into()),
            ));
        }
        if amount == 0 {
            return Ok(0);
        }
        let request = GovernanceSlashRequest {
            referendum_id,
            owner,
            amount,
            reason,
            note,
        };
        apply_governance_slash(&request, &mut locks, &mut rec, state_transaction)?;
        Ok(amount)
    }

    fn apply_governance_slash(
        request: &GovernanceSlashRequest<'_>,
        locks: &mut crate::state::GovernanceLocksForReferendum,
        rec: &mut crate::state::GovernanceLockRecord,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let def_id = state_transaction.gov.voting_asset_id.clone();
        let escrow_asset_id = iroha_data_model::asset::AssetId::new(
            def_id.clone(),
            state_transaction.gov.bond_escrow_account.clone(),
        );
        let receiver_account = state_transaction.gov.slash_receiver_account.clone();
        let receiver_asset_id =
            iroha_data_model::asset::AssetId::new(def_id, receiver_account.clone());
        let spec = state_transaction.numeric_spec_for(escrow_asset_id.definition())?;
        let slash_numeric = numeric_with_spec(request.amount, spec)?;
        crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with(&slash_numeric, spec)?;
        state_transaction
            .world
            .withdraw_numeric_asset(&escrow_asset_id, &slash_numeric)?;
        state_transaction
            .world
            .deposit_numeric_asset(&receiver_asset_id, &slash_numeric)?;
        rec.amount = rec.amount.saturating_sub(request.amount);
        rec.slashed = rec.slashed.saturating_add(request.amount);
        locks.locks.insert(request.owner.clone(), rec.clone());
        state_transaction
            .world
            .governance_locks
            .insert(request.referendum_id.to_owned(), locks.clone());

        let mut ledger = state_transaction
            .world
            .governance_slashes
            .get(request.referendum_id)
            .cloned()
            .unwrap_or_default();
        let entry = ledger
            .slashes
            .entry(request.owner.clone())
            .or_insert_with(crate::state::GovernanceSlashEntry::default);
        entry.total_slashed = entry.total_slashed.saturating_add(request.amount);
        entry.last_reason = request.reason;
        entry.last_height = state_transaction._curr_block.height().get();
        state_transaction
            .world
            .governance_slashes
            .insert(request.referendum_id.to_owned(), ledger);

        state_transaction
            .world
            .emit_events(Some(GovernanceEvent::LockSlashed(
                iroha_data_model::events::data::governance::GovernanceLockSlashed {
                    referendum_id: request.referendum_id.to_owned(),
                    owner: request.owner.clone(),
                    amount: request.amount,
                    reason: request.reason,
                    destination: receiver_account,
                    note: request.note.to_owned(),
                },
            )));
        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_governance_bond_event("lock_slashed");
        Ok(())
    }

    fn governance_restitute_lock(
        referendum_id: &str,
        owner: &AccountId,
        amount: u128,
        reason: GovernanceSlashReason,
        note: &str,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<u128, Error> {
        if amount == 0 {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract("restitution amount must be > 0".into()),
            ));
        }
        let Some(mut locks) = state_transaction
            .world
            .governance_locks
            .get(referendum_id)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                "referendum not found for governance lock restitution".into(),
            ));
        };
        let Some(mut rec) = locks.locks.get(owner).cloned() else {
            return Err(InstructionExecutionError::InvariantViolation(
                "governance lock not found for restitution".into(),
            ));
        };
        if rec.slashed == 0 {
            return Err(InstructionExecutionError::InvariantViolation(
                "no slashed balance available for restitution".into(),
            ));
        }
        if amount > rec.slashed {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "requested restitution exceeds slashed balance".into(),
                ),
            ));
        }
        rec.amount = rec
            .amount
            .checked_add(amount)
            .ok_or_else(|| Error::from(MathError::Overflow))?;
        rec.slashed -= amount;

        let def_id = state_transaction.gov.voting_asset_id.clone();
        let escrow_asset_id = iroha_data_model::asset::AssetId::new(
            def_id.clone(),
            state_transaction.gov.bond_escrow_account.clone(),
        );
        let receiver_asset_id = iroha_data_model::asset::AssetId::new(
            def_id,
            state_transaction.gov.slash_receiver_account.clone(),
        );
        let spec = state_transaction.numeric_spec_for(escrow_asset_id.definition())?;
        let restore_numeric = numeric_with_spec(amount, spec)?;
        crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with(&restore_numeric, spec)?;
        state_transaction
            .world
            .withdraw_numeric_asset(&receiver_asset_id, &restore_numeric)?;
        state_transaction
            .world
            .deposit_numeric_asset(&escrow_asset_id, &restore_numeric)?;
        let mut ledger = state_transaction
            .world
            .governance_slashes
            .get(referendum_id)
            .cloned()
            .unwrap_or_default();
        let Some(entry) = ledger.slashes.get_mut(owner) else {
            return Err(InstructionExecutionError::InvariantViolation(
                "slash ledger missing for restitution".into(),
            ));
        };
        let available = entry.total_slashed.saturating_sub(entry.total_restituted);
        if amount > available {
            return Err(InstructionExecutionError::InvariantViolation(
                "requested restitution exceeds recorded slashes".into(),
            ));
        }
        entry.total_restituted = entry.total_restituted.saturating_add(amount);
        entry.last_reason = reason;
        entry.last_height = state_transaction._curr_block.height().get();
        locks.locks.insert(owner.clone(), rec);
        state_transaction
            .world
            .governance_locks
            .insert(referendum_id.to_owned(), locks);
        state_transaction
            .world
            .governance_slashes
            .insert(referendum_id.to_owned(), ledger);
        state_transaction.world.emit_events(Some(
            iroha_data_model::events::data::governance::GovernanceEvent::LockRestituted(
                iroha_data_model::events::data::governance::GovernanceLockRestituted {
                    referendum_id: referendum_id.to_owned(),
                    owner: owner.clone(),
                    amount,
                    reason,
                    note: note.to_owned(),
                },
            ),
        ));
        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_governance_bond_event("lock_restituted");
        Ok(amount)
    }

    fn ensure_manifest_signature(
        manifest: &ContractManifest,
    ) -> Result<&ManifestProvenance, InstructionExecutionError> {
        let provenance = manifest.provenance.as_ref().ok_or_else(|| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                "manifest.provenance missing".into(),
            ))
        })?;
        let payload = manifest.signature_payload_bytes();
        provenance
            .signature
            .verify(&provenance.signer, &payload)
            .map_err(|_| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    "manifest signature verification failed".into(),
                ))
            })?;
        Ok(provenance)
    }

    #[cfg(feature = "telemetry")]
    fn root_evictions_since(before_len: usize, appended: usize, after_len: usize) -> u64 {
        let expected = before_len.saturating_add(appended);
        let evicted = expected.saturating_sub(after_len);
        u64::try_from(evicted).unwrap_or(u64::MAX)
    }

    impl Execute for verifying_keys::RegisterVerifyingKey {
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Permission check (simple token)
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageVerifyingKeys",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageVerifyingKeys".into(),
                ));
            }

            let id = self.id().clone();
            let record = self.record().clone();
            if matches!(record.status, ConfidentialStatus::Withdrawn) {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "cannot register verifying key with Withdrawn status".into(),
                    ),
                ));
            }
            if record.backend != BackendTag::Halo2IpaPasta {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verifying key backend must be Halo2IpaPasta".into(),
                    ),
                ));
            }
            if !record.curve.eq_ignore_ascii_case("pallas") {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verifying key curve must be \"pallas\"".into(),
                    ),
                ));
            }
            let id_backend = id.backend.as_str();
            if !id_backend.starts_with("halo2/") || id_backend.contains("bn254") {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verifying key id backend must target halo2 IPA (no bn254)".into(),
                    ),
                ));
            }
            // Commitment sanity if inline key present
            if let Some(vk) = &record.key {
                if record.commitment != hash_vk(vk) {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "verifying key commitment mismatch".into(),
                    ));
                }
                // backend must match id.backend
                if vk.backend != id.backend {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "vk backend does not match id backend".into(),
                    ));
                }
            }
            // Ensure entry not present
            if state_transaction.world.verifying_keys.get(&id).is_some() {
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::Permission(Permission::new(
                        "VerifyingKey".into(),
                        iroha_primitives::json::Json::from(
                            format!("{}::{}", id.backend, id.name).as_str(),
                        ),
                    )),
                }
                .into());
            }
            let circuit_key = (record.circuit_id.clone(), record.version);
            if let Some(existing) = state_transaction
                .world
                .verifying_keys_by_circuit
                .get(&circuit_key)
            {
                if existing != &id {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "verifying key circuit/version already registered".into(),
                    ));
                }
            }
            if record.circuit_id.trim().is_empty() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verifying key circuit_id must not be empty".into(),
                    ),
                ));
            }
            if record.public_inputs_schema_hash == [0u8; 32] {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verifying key public_inputs_schema_hash must be set".into(),
                    ),
                ));
            }
            if record.gas_schedule_id.is_none() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verifying key gas_schedule_id must be set".into(),
                    ),
                ));
            }
            state_transaction
                .world
                .verifying_keys
                .insert(id.clone(), record.clone());
            state_transaction
                .world
                .verifying_keys_by_circuit
                .insert(circuit_key, id.clone());
            // Emit verifying key registered event
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::verifying_keys::VerifyingKeyEvent::Registered(
                    iroha_data_model::events::data::verifying_keys::VerifyingKeyRegistered {
                        id: id.clone(),
                        record: record.clone(),
                    },
                ),
            ));
            Ok(())
        }
    }

    // ---------------- Governance (stubs) ----------------
    impl Execute for gov::ProposeDeployContract {
        #[allow(clippy::too_many_lines)]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let canonical_hex32 = |value: &str, field: &str| -> Result<(String, [u8; 32]), Error> {
                let trimmed = value.trim();
                let without_scheme = if let Some((scheme, rest)) = trimmed.split_once(':') {
                    if scheme.is_empty() || scheme.eq_ignore_ascii_case("blake2b32") {
                        rest
                    } else {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(format!(
                                "unsupported {field} scheme"
                            )),
                        ));
                    }
                } else {
                    trimmed
                };
                let body = without_scheme
                    .trim()
                    .strip_prefix("0x")
                    .unwrap_or_else(|| without_scheme.trim());
                if body.len() != 64 || !body.as_bytes().iter().all(u8::is_ascii_hexdigit) {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(format!(
                            "{field} must be 32-byte hex"
                        )),
                    ));
                }
                let canonical = body.to_ascii_lowercase();
                let mut out = [0u8; 32];
                hex::decode_to_slice(&canonical, &mut out).map_err(|_| {
                    InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(format!("invalid {field} hex")),
                    )
                })?;
                Ok((canonical, out))
            };

            if !has_permission(
                &state_transaction.world,
                authority,
                "CanProposeContractDeployment",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanProposeContractDeployment".into(),
                ));
            }
            let namespace_trimmed = self.namespace.trim();
            if namespace_trimmed.is_empty() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract("namespace must not be empty".into()),
                ));
            }
            let contract_trimmed = self.contract_id.trim();
            if contract_trimmed.is_empty() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract("contract_id must not be empty".into()),
                ));
            }
            let namespace = namespace_trimmed.to_string();
            let contract_id = contract_trimmed.to_string();

            let (code_hash_hex_str, code_hash_bytes) =
                canonical_hex32(&self.code_hash_hex, "code_hash")?;
            let (abi_hash_hex_str, abi_hash_bytes) =
                canonical_hex32(&self.abi_hash_hex, "abi_hash")?;
            let code_hash_hex = ContractCodeHash::from_hex_str(&code_hash_hex_str)
                .expect("canonical_hex32 guarantees valid hex");
            let abi_hash_hex = ContractAbiHash::from_hex_str(&abi_hash_hex_str)
                .expect("canonical_hex32 guarantees valid hex");

            let abi_version_trimmed = self.abi_version.trim();
            if abi_version_trimmed != "1" {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "unsupported abi_version: {abi_version_trimmed}"
                    )),
                ));
            }
            let expected_abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
            if abi_hash_bytes != expected_abi_hash {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "abi_hash does not match canonical hash for abi_version {abi_version_trimmed}"
                    )),
                ));
            }
            let abi_version = AbiVersion::new(1);

            let namespace_len: u32 = namespace.len().try_into().map_err(|_| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    "namespace length exceeds 2^32 bytes".into(),
                ))
            })?;
            let contract_len: u32 = contract_id.len().try_into().map_err(|_| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    "contract_id length exceeds 2^32 bytes".into(),
                ))
            })?;

            let mut id_input = Vec::with_capacity(
                b"iroha:gov:proposal:v1|".len()
                    + core::mem::size_of::<u32>() * 2
                    + namespace.len()
                    + contract_id.len()
                    + code_hash_bytes.len()
                    + abi_hash_bytes.len(),
            );
            id_input.extend_from_slice(b"iroha:gov:proposal:v1|");
            id_input.extend_from_slice(&namespace_len.to_le_bytes());
            id_input.extend_from_slice(namespace.as_bytes());
            id_input.extend_from_slice(&contract_len.to_le_bytes());
            id_input.extend_from_slice(contract_id.as_bytes());
            id_input.extend_from_slice(&code_hash_bytes);
            id_input.extend_from_slice(&abi_hash_bytes);
            let id_bytes = Blake2b512::digest(&id_input);
            let mut id = [0u8; 32];
            id.copy_from_slice(&id_bytes[..32]);
            let rid = hex::encode(id);

            let desired_mode = match self.mode {
                Some(iroha_data_model::isi::governance::VotingMode::Plain) => {
                    crate::state::GovernanceReferendumMode::Plain
                }
                _ => crate::state::GovernanceReferendumMode::Zk,
            };

            let h_now = state_transaction._curr_block.height().get();
            let min_start = h_now.saturating_add(state_transaction.gov.min_enactment_delay);
            let (start, end) = if let Some(win) = self.window {
                if win.upper < win.lower {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "window.upper must be >= window.lower".into(),
                        ),
                    ));
                }
                if win.lower < min_start {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "window.lower below minimum enactment delay".into(),
                        ),
                    ));
                }
                (win.lower, win.upper)
            } else {
                let span = state_transaction.gov.window_span.max(1);
                let end = min_start.saturating_add(span.saturating_sub(1));
                (min_start, end)
            };

            if end < start {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "enactment window upper precedes lower".into(),
                    ),
                ));
            }

            let payload = DeployContractProposal {
                namespace: namespace.clone(),
                contract_id: contract_id.clone(),
                code_hash_hex,
                abi_hash_hex,
                abi_version,
                manifest_provenance: self.manifest_provenance.clone(),
            };
            let kind = ProposalKind::DeployContract(payload.clone());

            if let Some(existing) = state_transaction.world.governance_proposals.get(&id) {
                let Some(existing_payload) = existing.as_deploy_contract() else {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "governance proposal id collision".into(),
                    ));
                };
                if existing_payload.namespace != payload.namespace
                    || existing_payload.contract_id != payload.contract_id
                    || existing_payload.code_hash_hex != payload.code_hash_hex
                    || existing_payload.abi_hash_hex != payload.abi_hash_hex
                    || existing_payload.abi_version != payload.abi_version
                    || existing_payload.manifest_provenance != payload.manifest_provenance
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "governance proposal id collision".into(),
                    ));
                }
                if let Some(ref_rec) = state_transaction.world.governance_referenda.get(&rid) {
                    if ref_rec.h_start != start
                        || ref_rec.h_end != end
                        || ref_rec.mode != desired_mode
                    {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "existing referendum parameters mismatch".into(),
                        ));
                    }
                }
                return Ok(());
            }

            if let Some(ref_rec) = state_transaction.world.governance_referenda.get(&rid) {
                if ref_rec.h_start != start || ref_rec.h_end != end || ref_rec.mode != desired_mode
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "referendum already exists with different parameters".into(),
                    ));
                }
            } else {
                state_transaction.world.governance_referenda.insert(
                    rid.clone(),
                    crate::state::GovernanceReferendumRecord {
                        h_start: start,
                        h_end: end,
                        status: crate::state::GovernanceReferendumStatus::Proposed,
                        mode: desired_mode,
                    },
                );
            }

            // Record basic proposal info (WSV schema stub)
            let created_height = h_now;
            let referendum_snapshot = state_transaction
                .world
                .governance_referenda
                .get(&rid)
                .copied();
            let pipeline = crate::state::GovernancePipeline::seeded(
                created_height,
                referendum_snapshot.as_ref(),
                &state_transaction.gov,
            );
            let rec = crate::state::GovernanceProposalRecord {
                proposer: authority.clone(),
                kind: kind.clone(),
                created_height,
                status: crate::state::GovernanceProposalStatus::Proposed,
                pipeline,
            };
            state_transaction.world.governance_proposals.insert(id, rec);

            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::ProposalSubmitted(
                    iroha_data_model::events::data::governance::GovernanceProposalSubmitted {
                        id,
                        proposer: authority.clone(),
                        namespace: payload.namespace,
                        contract_id: payload.contract_id,
                    },
                ),
            ));
            Ok(())
        }
    }

    impl Execute for gov::CastZkBallot {
        #[allow(clippy::too_many_lines)]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let parse_hex32_field = |raw: &str| -> Option<[u8; 32]> {
                let mut body = raw.trim();
                if let Some(rest) = body.strip_prefix("blake2b32:") {
                    body = rest;
                }
                if let Some(rest) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
                    body = rest;
                }
                if body.len() != 64 || !body.as_bytes().iter().all(u8::is_ascii_hexdigit) {
                    return None;
                }
                let mut out = [0u8; 32];
                hex::decode_to_slice(body, &mut out).ok()?;
                Some(out)
            };

            let parse_ballot_amount = |value: &norito::json::Value| -> Option<u128> {
                if let Some(n) = value.as_u64() {
                    return Some(u128::from(n));
                }
                value.as_str().and_then(|s| s.trim().parse::<u128>().ok())
            };

            let parse_duration_blocks = |value: &norito::json::Value| -> Option<u64> {
                value
                    .as_u64()
                    .or_else(|| value.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
            };

            if !has_permission(
                &state_transaction.world,
                authority,
                "CanSubmitGovernanceBallot",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanSubmitGovernanceBallot".into(),
                ));
            }
            ensure_citizen_for_ballot(authority, &self.election_id, state_transaction)?;
            let id = self.election_id.clone();
            // Validate ballot proof inputs and enforce governance policy before recording state.

            // 1) Decode proof bytes from base64 (reject empty/invalid)
            let proof_bytes =
                match base64::engine::general_purpose::STANDARD.decode(self.proof_b64.as_bytes()) {
                    Ok(v) if !v.is_empty() => v,
                    _ => {
                        // Non-consensus visibility
                        state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "invalid or empty proof".into(),
                            },
                        ),
                    ));
                        return Err(InstructionExecutionError::InvariantViolation(
                            "invalid or empty proof".into(),
                        ));
                    }
                };

            state_transaction.register_confidential_proof(proof_bytes.len())?;
            let max_proof = state_transaction.zk.preverify_max_bytes;
            if max_proof > 0 && proof_bytes.len() > max_proof {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "proof exceeds configured max bytes".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "proof exceeds configured max bytes".into(),
                    ),
                ));
            }

            let public_inputs = if self.public_inputs_json.trim().is_empty() {
                None
            } else if let Ok(value) =
                norito::json::from_str::<norito::json::Value>(self.public_inputs_json.as_str())
            {
                Some(value)
            } else {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "public inputs must be valid JSON".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "public inputs must be valid JSON".into(),
                ));
            };
            let mut lock_owner: Option<iroha_data_model::account::AccountId> = None;
            let mut lock_amount: Option<u128> = None;
            let mut lock_duration: Option<u64> = None;
            let mut pending_owner_for_nullifier: Option<iroha_data_model::account::AccountId> =
                None;
            let mut pending_salt: Option<[u8; 32]> = None;

            // 2) Compute a candidate ballot nullifier
            // Priority order:
            //   a) Explicit `nullifier_hex` (or `nullifier`) in `public_inputs_json` when valid 32-byte hex
            //   b) Deterministic derivation from (chain_id, election_id, owner, salt)
            //      using a dedicated domain tag (preferred per BallotProof sketch)
            // Missing both inputs is rejected; proofs must include either explicit
            // nullifiers or owner+salt hints provided by the circuit.
            let mut nullifier = [0u8; 32];
            let mut used_explicit = false;
            // Optional eligibility root hint provided via public_inputs_json (hex32)
            let mut root_hint_opt: Option<[u8; 32]> = None;
            if let Some(val) = public_inputs.as_ref() {
                if let Some(rh_str) = val.get("root_hint").and_then(|v| v.as_str()) {
                    if let Some(parsed) = parse_hex32_field(rh_str) {
                        root_hint_opt = Some(parsed);
                    }
                }
                if let Some(hex_str) = val
                    .get("nullifier_hex")
                    .and_then(|v| v.as_str())
                    .or_else(|| val.get("nullifier").and_then(|v| v.as_str()))
                {
                    if let Some(parsed) = parse_hex32_field(hex_str) {
                        nullifier = parsed;
                        used_explicit = true;
                    }
                }
                if !used_explicit {
                    let salt_candidate = val
                        .get("salt")
                        .and_then(|v| v.as_str())
                        .or_else(|| val.get("salt_hex").and_then(|v| v.as_str()));
                    if let (Some(owner_str), Some(salt_str)) =
                        (val.get("owner").and_then(|v| v.as_str()), salt_candidate)
                    {
                        if let Some(salt) = parse_hex32_field(salt_str) {
                            if salt.iter().all(|b| *b == 0) {
                                state_transaction.world.emit_events(Some(
                                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                            referendum_id: self.election_id.clone(),
                                            reason: "salt must be non-zero".into(),
                                        },
                                    ),
                                ));
                                return Err(InstructionExecutionError::InvariantViolation(
                                    "salt must be non-zero".into(),
                                ));
                            }
                            let owner_id: iroha_data_model::account::AccountId = if let Ok(id) =
                                owner_str.parse()
                            {
                                id
                            } else {
                                state_transaction.world.emit_events(Some(
                                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                            referendum_id: self.election_id.clone(),
                                            reason: "owner must be a canonical account id".into(),
                                        },
                                    ),
                                ));
                                return Err(InstructionExecutionError::InvariantViolation(
                                    "owner must be a canonical account id".into(),
                                ));
                            };
                            let owner_canonical = owner_id.to_string();
                            if owner_canonical.as_str() != owner_str {
                                state_transaction.world.emit_events(Some(
                                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                            referendum_id: self.election_id.clone(),
                                            reason: "owner must use canonical account id form".into(),
                                        },
                                    ),
                            ));
                                return Err(InstructionExecutionError::InvariantViolation(
                                    "owner must use canonical account id form".into(),
                                ));
                            }
                            pending_owner_for_nullifier = Some(owner_id);
                            pending_salt = Some(salt);
                        }
                    }
                }
            }
            if !used_explicit {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "nullifier inputs missing".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "nullifier inputs missing".into(),
                ));
            }

            if let Some(val) = public_inputs.as_ref() {
                if let Some(owner_str) = val.get("owner").and_then(|v| v.as_str()) {
                    let owner_parsed: iroha_data_model::account::AccountId = if let Ok(id) =
                        owner_str.parse()
                    {
                        id
                    } else {
                        state_transaction.world.emit_events(Some(
                            iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                    referendum_id: self.election_id.clone(),
                                    reason: "owner must be a canonical account id".into(),
                                },
                            ),
                        ));
                        return Err(InstructionExecutionError::InvariantViolation(
                            "owner must be a canonical account id".into(),
                        ));
                    };
                    let owner_canonical = owner_parsed.to_string();
                    if owner_canonical != owner_str {
                        state_transaction.world.emit_events(Some(
                            iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                    referendum_id: self.election_id.clone(),
                                    reason: "owner must use canonical account id form".into(),
                                },
                            ),
                        ));
                        return Err(InstructionExecutionError::InvariantViolation(
                            "owner must use canonical account id form".into(),
                        ));
                    }
                    if lock_owner.is_none() {
                        lock_owner = Some(owner_parsed);
                    }
                }
                if let Some(amount_val) = val.get("amount") {
                    if let Some(parsed) = parse_ballot_amount(amount_val) {
                        lock_amount = Some(parsed);
                    } else {
                        state_transaction.world.emit_events(Some(
                            iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                    referendum_id: self.election_id.clone(),
                                    reason: "amount must be u128".into(),
                                },
                            ),
                        ));
                        return Err(InstructionExecutionError::InvariantViolation(
                            "amount must be u128".into(),
                        ));
                    }
                }
                if let Some(duration_val) = val.get("duration_blocks") {
                    if let Some(parsed) = parse_duration_blocks(duration_val) {
                        lock_duration = Some(parsed);
                    } else {
                        state_transaction.world.emit_events(Some(
                            iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                    referendum_id: self.election_id.clone(),
                                    reason: "duration_blocks must be u64".into(),
                                },
                            ),
                        ));
                        return Err(InstructionExecutionError::InvariantViolation(
                            "duration_blocks must be u64".into(),
                        ));
                    }
                }
            }

            let mut st = state_transaction
                .world
                .elections
                .get(&self.election_id)
                .cloned()
                .ok_or_else(|| {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "unknown election id".into(),
                            },
                        ),
                    ));
                    InstructionExecutionError::InvariantViolation("unknown election id".into())
                })?;
            if st.finalized {
                return Err(InstructionExecutionError::InvariantViolation(
                    "election already finalized".into(),
                ));
            }
            let citizen_root = citizen_registry_root(&state_transaction.world);
            if st.eligible_root != citizen_root {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "citizen registry root mismatch".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "citizen registry root mismatch".into(),
                ));
            }
            let domain_tag = if st.domain_tag.is_empty() {
                DEFAULT_NULLIFIER_DOMAIN_TAG.to_string()
            } else {
                st.domain_tag.clone()
            };

            if !used_explicit {
                if let (Some(owner_id), Some(salt)) =
                    (pending_owner_for_nullifier.clone(), pending_salt)
                {
                    let owner_canonical = owner_id.to_string();
                    let mut input = Vec::with_capacity(
                        domain_tag.len()
                            + state_transaction.chain_id.as_str().len()
                            + self.election_id.len()
                            + owner_canonical.len()
                            + salt.len()
                            + 3,
                    );
                    input.extend_from_slice(domain_tag.as_bytes());
                    if !domain_tag.as_bytes().ends_with(b"|") {
                        input.push(b'|');
                    }
                    input.extend_from_slice(state_transaction.chain_id.as_str().as_bytes());
                    input.push(b'|');
                    input.extend_from_slice(self.election_id.as_bytes());
                    input.push(b'|');
                    input.extend_from_slice(owner_canonical.as_bytes());
                    input.push(b'|');
                    input.extend_from_slice(&salt);
                    let digest = Blake2b512::digest(&input);
                    nullifier.copy_from_slice(&digest[..32]);
                    used_explicit = true;
                    if lock_owner.is_none() {
                        lock_owner = Some(owner_id);
                    }
                }
            }
            if !used_explicit {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "nullifier inputs missing".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "nullifier inputs missing".into(),
                ));
            }

            // Early referendum existence/window checks (Zk)
            {
                let rid = self.election_id.clone();
                let now_h = state_transaction._curr_block.height().get();
                let Some(rr) = state_transaction
                    .world
                    .governance_referenda
                    .get(&rid)
                    .copied()
                else {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: rid,
                                reason: "referendum not found".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "referendum not found".into(),
                    ));
                };
                if rr.mode != crate::state::GovernanceReferendumMode::Zk {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: rid,
                                reason: "mode mismatch (expected Zk)".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "referendum mode mismatch: expected Zk".into(),
                    ));
                }
                if now_h < rr.h_start || now_h > rr.h_end {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: rid.clone(),
                                reason: "referendum not active (outside window)".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "referendum not active".into(),
                    ));
                }
                if matches!(rr.status, crate::state::GovernanceReferendumStatus::Closed) {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: rid,
                                reason: "referendum already closed".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "referendum closed".into(),
                    ));
                }
            }

            // 3) Verify the proof against the resolved VK (ZK1/H2* envelope dispatch)
            let vk_id = st
                .vk_ballot
                .clone()
                .or_else(|| {
                    state_transaction.gov.vk_ballot.as_ref().map(|v| {
                        iroha_data_model::proof::VerifyingKeyId::new(
                            v.backend.clone(),
                            v.name.clone(),
                        )
                    })
                })
                .ok_or_else(|| {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "verifying key not configured".into(),
                            },
                        ),
                    ));
                    InstructionExecutionError::InvariantViolation(
                        "verifying key not configured".into(),
                    )
                })?;
            let Some(vk_rec) = state_transaction.world.verifying_keys.get(&vk_id).cloned() else {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "verifying key not found".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key not found".into(),
                ));
            };
            if vk_rec.status != iroha_data_model::confidential::ConfidentialStatus::Active {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "verifying key is not Active".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key is not Active".into(),
                ));
            }
            if let Some(expected) = st.vk_ballot_commitment {
                if vk_rec.commitment != expected {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "verifying key commitment mismatch".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "verifying key commitment mismatch".into(),
                    ));
                }
            }
            let Some(vk_box) = vk_rec.key.clone() else {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key bytes not available inline".into(),
                ));
            };
            let computed_commitment = hash_vk(&vk_box);
            if computed_commitment != vk_rec.commitment {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "verifying key commitment mismatch".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key commitment mismatch".into(),
                ));
            }
            let backend = vk_id.backend.as_str();
            if vk_box.backend.as_str() != backend {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "verifying key backend mismatch".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key backend mismatch".into(),
                ));
            }
            let proof_box =
                iroha_data_model::proof::ProofBox::new(vk_id.backend.clone(), proof_bytes.clone());
            let verify_report =
                crate::zk::verify_backend_with_timing(backend, &proof_box, Some(&vk_box));
            let timeout_budget = state_transaction.zk.verify_timeout;
            if timeout_budget > Duration::ZERO && verify_report.elapsed > timeout_budget {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "proof verification exceeded timeout".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "proof verification exceeded timeout".into(),
                    ),
                ));
            }
            if !verify_report.ok {
                let slash_target = lock_owner.as_ref().unwrap_or(authority);
                let _ = governance_slash_percent(
                    &self.election_id,
                    slash_target,
                    state_transaction.gov.slash_invalid_proof_bps,
                    GovernanceSlashReason::Misconduct,
                    "invalid_proof",
                    state_transaction,
                )?;
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "invalid proof".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "invalid proof".into(),
                ));
            }
            let proof_verified = true;

            // 4) Ensure a referendum record exists and open it if needed; enforce Zk mode
            {
                let rid = self.election_id.clone();
                let Some(mut rr) = state_transaction
                    .world
                    .governance_referenda
                    .get(&rid)
                    .copied()
                else {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "referendum not found".into(),
                    ));
                };
                if !matches!(rr.status, crate::state::GovernanceReferendumStatus::Open) {
                    rr.status = crate::state::GovernanceReferendumStatus::Open;
                    state_transaction
                        .world
                        .governance_referenda
                        .insert(rid.clone(), rr);
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::ReferendumOpened(
                            iroha_data_model::events::data::governance::GovernanceReferendumOpened {
                                id: rid.clone(),
                                h_start: 0,
                                h_end: 0,
                            },
                        ),
                    ));
                }
            }
            // 5) Enforce eligibility and nullifier uniqueness on the existing election state
            if st.finalized {
                return Err(InstructionExecutionError::InvariantViolation(
                    "election already finalized".into(),
                ));
            }
            if let Some(root_hint) = root_hint_opt {
                if st.eligible_root != root_hint {
                    let slash_target = lock_owner.as_ref().unwrap_or(authority);
                    let _ = governance_slash_percent(
                        &self.election_id,
                        slash_target,
                        state_transaction.gov.slash_ineligible_proof_bps,
                        GovernanceSlashReason::IneligibleProof,
                        "ineligible_proof",
                        state_transaction,
                    )?;
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "stale or unknown eligibility root".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "stale or unknown eligibility root".into(),
                    ));
                }
            }
            // Resolve verifying key id: prefer election's vk_ballot, else config default
            if let Some(vk_id) = st.vk_ballot.clone().or_else(|| {
                state_transaction.gov.vk_ballot.as_ref().map(|v| {
                    iroha_data_model::proof::VerifyingKeyId::new(v.backend.clone(), v.name.clone())
                })
            }) {
                if state_transaction.world.verifying_keys.get(&vk_id).is_none() {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "verifying key not found".into(),
                    ));
                }
            }
            if !st.ballot_nullifiers.insert(nullifier) {
                let slash_target = lock_owner.as_ref().unwrap_or(authority);
                let _ = governance_slash_percent(
                    &self.election_id,
                    slash_target,
                    state_transaction.gov.slash_double_vote_bps,
                    GovernanceSlashReason::DoubleVote,
                    "double_vote",
                    state_transaction,
                )?;
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "duplicate ballot nullifier".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "duplicate ballot nullifier".into(),
                ));
            }

            // Record a ciphertext placeholder (proof bytes here) and enforce cap
            st.ciphertexts.push(proof_bytes);
            let cap = state_transaction.zk.ballot_history_cap.max(1);
            if st.ciphertexts.len() > cap {
                let surplus = st.ciphertexts.len() - cap;
                st.ciphertexts.drain(0..surplus);
            }
            state_transaction.world.elections.remove(id.clone());
            state_transaction.world.elections.insert(id, st);
            // Emit a governance ballot accepted event (mode = Zk, weight not revealed)
            let rid_ev = self.election_id.clone();
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::BallotAccepted(
                    iroha_data_model::events::data::governance::GovernanceBallotAccepted {
                        referendum_id: rid_ev,
                        mode: iroha_data_model::events::data::governance::GovernanceBallotMode::Zk,
                        weight: None,
                    },
                ),
            ));
            if proof_verified {
                if let (Some(owner), Some(amount), Some(duration_blocks)) =
                    (lock_owner.clone(), lock_amount, lock_duration)
                {
                    if owner != *authority {
                        state_transaction.world.emit_events(Some(
                            iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                    referendum_id: self.election_id.clone(),
                                    reason: "owner must equal authority".into(),
                                },
                            ),
                        ));
                        return Err(InstructionExecutionError::InvariantViolation(
                            "owner must equal authority".into(),
                        ));
                    }
                    if state_transaction.gov.min_bond_amount > 0
                        && amount < state_transaction.gov.min_bond_amount
                    {
                        state_transaction.world.emit_events(Some(
                            iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                    referendum_id: self.election_id.clone(),
                                    reason: "bond amount below minimum".into(),
                                },
                            ),
                        ));
                        return Err(InstructionExecutionError::InvariantViolation(
                            "bond amount below minimum".into(),
                        ));
                    }
                    if duration_blocks < state_transaction.gov.conviction_step_blocks {
                        state_transaction.world.emit_events(Some(
                            iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                    referendum_id: self.election_id.clone(),
                                    reason: "lock shorter than minimum".into(),
                                },
                            ),
                        ));
                        return Err(InstructionExecutionError::InvariantViolation(
                            "lock duration shorter than minimum".into(),
                        ));
                    }
                    let rid = self.election_id.clone();
                    let now_h = state_transaction._curr_block.height().get();
                    let new_expiry = now_h.saturating_add(duration_blocks);
                    let mut locks = state_transaction
                        .world
                        .governance_locks
                        .get(&rid)
                        .cloned()
                        .unwrap_or_default();
                    if let Some(prev) = locks.locks.get(&owner) {
                        if amount < prev.amount || new_expiry < prev.expiry_height {
                            state_transaction.world.emit_events(Some(
                                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                        referendum_id: rid,
                                        reason:
                                            "re-vote cannot reduce existing lock (amount/expiry)".into(),
                                    },
                                ),
                            ));
                            return Err(InstructionExecutionError::InvariantViolation(
                                "re-vote cannot reduce existing lock".into(),
                            ));
                        }
                    }
                    lock_voting_bond(
                        amount,
                        locks.locks.get(&owner).map(|rec| rec.amount),
                        &owner,
                        &self.election_id,
                        state_transaction,
                    )?;
                    let existed = locks.locks.contains_key(&owner);
                    let rec = crate::state::GovernanceLockRecord {
                        owner: owner.clone(),
                        amount,
                        slashed: 0,
                        expiry_height: new_expiry,
                        direction: 2,
                        duration_blocks,
                    };
                    locks.locks.insert(owner.clone(), rec.clone());
                    state_transaction
                        .world
                        .governance_locks
                        .insert(rid.clone(), locks);
                    if existed {
                        state_transaction.world.emit_events(Some(
                            iroha_data_model::events::data::governance::GovernanceEvent::LockExtended(
                                iroha_data_model::events::data::governance::GovernanceLockExtended {
                                    referendum_id: rid,
                                    owner: rec.owner,
                                    amount: rec.amount,
                                    expiry_height: rec.expiry_height,
                                },
                            ),
                        ));
                    } else {
                        state_transaction.world.emit_events(Some(
                            iroha_data_model::events::data::governance::GovernanceEvent::LockCreated(
                                iroha_data_model::events::data::governance::GovernanceLockCreated {
                                    referendum_id: rid,
                                    owner: rec.owner,
                                    amount: rec.amount,
                                    expiry_height: rec.expiry_height,
                                },
                            ),
                        ));
                    }
                }
            }
            Ok(())
        }
    }

    fn ensure_plain_ballot_preconditions(
        ballot: &gov::CastPlainBallot,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if ballot.owner != *authority {
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                        referendum_id: ballot.referendum_id.clone(),
                        reason: "owner must equal authority".into(),
                    },
                ),
            ));
            return Err(InstructionExecutionError::InvariantViolation(
                "owner must equal authority".into(),
            ));
        }
        ensure_citizen_for_ballot(authority, &ballot.referendum_id, state_transaction)?;
        if state_transaction.gov.min_bond_amount > 0
            && ballot.amount < state_transaction.gov.min_bond_amount
        {
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                        referendum_id: ballot.referendum_id.clone(),
                        reason: "bond amount below minimum".into(),
                    },
                ),
            ));
            return Err(InstructionExecutionError::InvariantViolation(
                "bond amount below minimum".into(),
            ));
        }
        if !state_transaction.gov.plain_voting_enabled {
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                        referendum_id: ballot.referendum_id.clone(),
                        reason: "mode disabled".into(),
                    },
                ),
            ));
            return Err(InstructionExecutionError::InvariantViolation(
                "plain voting mode disabled by policy".into(),
            ));
        }
        if ballot.duration_blocks < state_transaction.gov.conviction_step_blocks {
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                        referendum_id: ballot.referendum_id.clone(),
                        reason: "lock shorter than minimum".into(),
                    },
                ),
            ));
            return Err(InstructionExecutionError::InvariantViolation(
                "lock duration shorter than minimum".into(),
            ));
        }
        if !has_permission(
            &state_transaction.world,
            authority,
            "CanSubmitGovernanceBallot",
        ) {
            return Err(InstructionExecutionError::InvariantViolation(
                "not permitted: CanSubmitGovernanceBallot".into(),
            ));
        }
        Ok(())
    }

    fn sweep_expired_plain_locks(
        ballot: &gov::CastPlainBallot,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) {
        let rid = ballot.referendum_id.clone();
        let current_h = state_transaction._curr_block.height().get();
        if let Some(mut existing) = state_transaction.world.governance_locks.get(&rid).cloned() {
            existing.locks.retain(|_, v| v.expiry_height >= current_h);
            state_transaction
                .world
                .governance_locks
                .insert(rid, existing);
        }
    }

    fn ensure_plain_referendum_open(
        ballot: &gov::CastPlainBallot,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let rid = ballot.referendum_id.clone();
        let now_h = state_transaction._curr_block.height().get();
        let Some(mut rr) = state_transaction
            .world
            .governance_referenda
            .get(&rid)
            .copied()
        else {
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                        referendum_id: rid,
                        reason: "referendum not found".into(),
                    },
                ),
            ));
            return Err(InstructionExecutionError::InvariantViolation(
                "referendum not found".into(),
            ));
        };

        if rr.mode != crate::state::GovernanceReferendumMode::Plain {
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                        referendum_id: rid,
                        reason: "mode mismatch (expected Plain)".into(),
                    },
                ),
            ));
            return Err(InstructionExecutionError::InvariantViolation(
                "referendum mode mismatch: expected Plain".into(),
            ));
        }

        if now_h < rr.h_start || now_h > rr.h_end {
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                        referendum_id: rid.clone(),
                        reason: "referendum not active (outside window)".into(),
                    },
                ),
            ));
            return Err(InstructionExecutionError::InvariantViolation(
                "referendum not active".into(),
            ));
        }

        if matches!(rr.status, crate::state::GovernanceReferendumStatus::Closed) {
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                        referendum_id: rid,
                        reason: "referendum already closed".into(),
                    },
                ),
            ));
            return Err(InstructionExecutionError::InvariantViolation(
                "referendum closed".into(),
            ));
        }

        if !matches!(rr.status, crate::state::GovernanceReferendumStatus::Open) {
            rr.status = crate::state::GovernanceReferendumStatus::Open;
            state_transaction
                .world
                .governance_referenda
                .insert(ballot.referendum_id.clone(), rr);
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::ReferendumOpened(
                    iroha_data_model::events::data::governance::GovernanceReferendumOpened {
                        id: ballot.referendum_id.clone(),
                        h_start: 0,
                        h_end: 0,
                    },
                ),
            ));
        }
        Ok(())
    }

    fn apply_plain_ballot_lock(
        ballot: &gov::CastPlainBallot,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let rid = ballot.referendum_id.clone();
        let mut locks = state_transaction
            .world
            .governance_locks
            .get(&rid)
            .cloned()
            .unwrap_or_default();
        let now_h = state_transaction._curr_block.height().get();
        let new_expiry = now_h.saturating_add(ballot.duration_blocks);

        if let Some(prev) = locks.locks.get(authority) {
            if prev.direction != ballot.direction {
                let _ = governance_slash_percent(
                    &rid,
                    authority,
                    state_transaction.gov.slash_double_vote_bps,
                    GovernanceSlashReason::DoubleVote,
                    "double_vote",
                    state_transaction,
                )?;
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: rid.clone(),
                            reason: "re-vote cannot change direction".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "re-vote cannot change direction".into(),
                ));
            }
            if ballot.amount < prev.amount || new_expiry < prev.expiry_height {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: rid.clone(),
                            reason: "re-vote cannot reduce existing lock (amount/expiry)".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "re-vote cannot reduce existing lock".into(),
                ));
            }
        }
        lock_voting_bond(
            ballot.amount,
            locks.locks.get(authority).map(|rec| rec.amount),
            authority,
            &ballot.referendum_id,
            state_transaction,
        )?;

        let existed = locks.locks.contains_key(authority);
        let expiry_height = locks.locks.get(authority).map_or(new_expiry, |prev| {
            core::cmp::max(prev.expiry_height, new_expiry)
        });
        let rec = crate::state::GovernanceLockRecord {
            owner: ballot.owner.clone(),
            amount: ballot.amount,
            slashed: 0,
            expiry_height,
            direction: ballot.direction,
            duration_blocks: ballot.duration_blocks,
        };
        locks.locks.insert(authority.clone(), rec.clone());
        state_transaction
            .world
            .governance_locks
            .insert(rid.clone(), locks);

        record_plain_ballot_events(ballot, &rec, existed, &rid, state_transaction)?;
        Ok(())
    }

    fn record_plain_ballot_events(
        ballot: &gov::CastPlainBallot,
        rec: &crate::state::GovernanceLockRecord,
        existed: bool,
        referendum_id: &str,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let event = if existed {
            iroha_data_model::events::data::governance::GovernanceEvent::LockExtended(
                iroha_data_model::events::data::governance::GovernanceLockExtended {
                    referendum_id: referendum_id.to_owned(),
                    owner: rec.owner.clone(),
                    amount: rec.amount,
                    expiry_height: rec.expiry_height,
                },
            )
        } else {
            iroha_data_model::events::data::governance::GovernanceEvent::LockCreated(
                iroha_data_model::events::data::governance::GovernanceLockCreated {
                    referendum_id: referendum_id.to_owned(),
                    owner: rec.owner.clone(),
                    amount: rec.amount,
                    expiry_height: rec.expiry_height,
                },
            )
        };
        state_transaction.world.emit_events(Some(event));

        let mut weight = integer_sqrt_u128(ballot.amount);
        let step = state_transaction.gov.conviction_step_blocks.max(1);
        let mut factor = 1u64 + (ballot.duration_blocks / step);
        if factor > state_transaction.gov.max_conviction {
            factor = state_transaction.gov.max_conviction;
        }
        weight = weight
            .checked_mul(u128::from(factor))
            .ok_or_else(|| Error::from(MathError::Overflow))?;
        state_transaction.world.emit_events(Some(
            iroha_data_model::events::data::governance::GovernanceEvent::BallotAccepted(
                iroha_data_model::events::data::governance::GovernanceBallotAccepted {
                    referendum_id: referendum_id.to_owned(),
                    mode: iroha_data_model::events::data::governance::GovernanceBallotMode::Plain,
                    weight: Some(weight),
                },
            ),
        ));
        Ok(())
    }

    impl Execute for gov::CastPlainBallot {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            ensure_plain_ballot_preconditions(&self, authority, state_transaction)?;
            sweep_expired_plain_locks(&self, state_transaction);
            ensure_plain_referendum_open(&self, state_transaction)?;
            apply_plain_ballot_lock(&self, authority, state_transaction)?;
            Ok(())
        }
    }

    impl Execute for gov::SlashGovernanceLock {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if self.amount == 0 {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract("slash amount must be > 0".into()),
                ));
            }
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanSlashGovernanceLock",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanSlashGovernanceLock".into(),
                ));
            }
            governance_slash_absolute(
                &self.referendum_id,
                &self.owner,
                self.amount,
                GovernanceSlashReason::Manual,
                &self.reason,
                state_transaction,
            )?;
            Ok(())
        }
    }

    impl Execute for gov::RestituteGovernanceLock {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanRestituteGovernanceLock",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanRestituteGovernanceLock".into(),
                ));
            }
            governance_restitute_lock(
                &self.referendum_id,
                &self.owner,
                self.amount,
                GovernanceSlashReason::Restitution,
                &self.reason,
                state_transaction,
            )?;
            Ok(())
        }
    }

    fn load_proposal(
        state_transaction: &StateTransaction<'_, '_>,
        pid: [u8; 32],
        pid_hex: &str,
    ) -> Result<(DeployContractProposal, bool), Error> {
        let Some(prop) = state_transaction
            .world
            .governance_proposals
            .get(&pid)
            .cloned()
        else {
            return Err(
                iroha_data_model::query::error::FindError::Permission(Box::new(Permission::new(
                    "GovernanceProposalMissing".into(),
                    iroha_primitives::json::Json::from(pid_hex),
                )))
                .into(),
            );
        };

        let payload = prop.as_deploy_contract().cloned().ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "unsupported governance proposal kind".into(),
            )
        })?;

        let already_enacted =
            matches!(prop.status, crate::state::GovernanceProposalStatus::Enacted);
        if !already_enacted
            && !matches!(
                prop.status,
                crate::state::GovernanceProposalStatus::Approved
            )
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "proposal must be approved before enactment".into(),
            ));
        }

        Ok((payload, already_enacted))
    }

    fn parse_hex32(input: &str, what: &str) -> Result<[u8; 32], Error> {
        let bytes = hex::decode(input.trim_start_matches("0x")).map_err(|_| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                format!("invalid {what} hex"),
            ))
        })?;
        if bytes.len() != 32 {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!("{what} must be 32 bytes")),
            ));
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes);
        Ok(out)
    }

    fn extract_hashes(payload: &DeployContractProposal) -> Result<([u8; 32], [u8; 32]), Error> {
        let code_hash = parse_hex32(&payload.code_hash_hex.to_hex(), "code_hash")?;
        let abi_hash = parse_hex32(&payload.abi_hash_hex.to_hex(), "abi_hash")?;
        Ok((code_hash, abi_hash))
    }

    fn upsert_manifest(
        state_transaction: &mut StateTransaction<'_, '_>,
        key: iroha_crypto::Hash,
        abi_hash: [u8; 32],
        provenance: Option<&ManifestProvenance>,
    ) -> Result<bool, Error> {
        if let Some(existing) = state_transaction.world.contract_manifests.get(&key) {
            if let Some(ex_abi) = &existing.abi_hash {
                let ex_arr: [u8; 32] = (*ex_abi).into();
                if ex_arr != abi_hash {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "existing manifest abi_hash mismatch".into(),
                    ));
                }
            }
            Ok(false)
        } else {
            let provenance = provenance.ok_or_else(|| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    "manifest_provenance missing for governance contract deployment".into(),
                ))
            })?;
            let manifest = iroha_data_model::smart_contract::manifest::ContractManifest {
                code_hash: Some(key),
                abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                provenance: Some(provenance.clone()),
            };
            ensure_manifest_signature(&manifest)?;
            state_transaction
                .world
                .contract_manifests
                .insert(key, manifest);
            Ok(true)
        }
    }

    fn mark_proposal_enacted(state_transaction: &mut StateTransaction<'_, '_>, pid: [u8; 32]) {
        if let Some(mut rec) = state_transaction
            .world
            .governance_proposals
            .get(&pid)
            .cloned()
        {
            rec.status = crate::state::GovernanceProposalStatus::Enacted;
            state_transaction
                .world
                .governance_proposals
                .insert(pid, rec);
        }
    }

    fn bind_contract_instance(
        state_transaction: &mut StateTransaction<'_, '_>,
        payload: &DeployContractProposal,
        key: iroha_crypto::Hash,
    ) -> Result<bool, Error> {
        let ns_key = (payload.namespace.clone(), payload.contract_id.clone());
        if let Some(existing) = state_transaction.world.contract_instances.get(&ns_key) {
            if *existing != key {
                return Err(InstructionExecutionError::InvariantViolation(
                    "contract instance already bound to a different code hash".into(),
                ));
            }
            return Ok(false);
        }
        state_transaction
            .world
            .contract_instances
            .insert(ns_key, key);
        Ok(true)
    }

    #[cfg(feature = "telemetry")]
    fn record_enactment_telemetry(
        state_transaction: &mut StateTransaction<'_, '_>,
        payload: &DeployContractProposal,
        code_hash: [u8; 32],
        abi_hash: [u8; 32],
        manifest_inserted: bool,
        instance_bound_new: bool,
    ) {
        if manifest_inserted {
            state_transaction
                .telemetry
                .record_manifest_activation(None, "manifest_inserted");
        }
        if instance_bound_new {
            let activated_at_ms_u128 = state_transaction._curr_block.creation_time().as_millis();
            let activated_at_ms = u64::try_from(activated_at_ms_u128).unwrap_or(u64::MAX);
            let activation = GovernanceManifestActivation {
                namespace: payload.namespace.clone(),
                contract_id: payload.contract_id.clone(),
                code_hash_hex: hex::encode(code_hash),
                abi_hash_hex: Some(hex::encode(abi_hash)),
                height: state_transaction._curr_block.height().get(),
                activated_at_ms,
            };
            state_transaction
                .telemetry
                .record_manifest_activation(Some(activation), "instance_bound");
        }
    }

    fn close_referendum_if_open(state_transaction: &mut StateTransaction<'_, '_>, pid_hex: &str) {
        if let Some(mut ref_rec) = state_transaction
            .world
            .governance_referenda
            .get(pid_hex)
            .copied()
        {
            if ref_rec.status != crate::state::GovernanceReferendumStatus::Closed {
                ref_rec.status = crate::state::GovernanceReferendumStatus::Closed;
                state_transaction
                    .world
                    .governance_referenda
                    .insert(pid_hex.to_owned(), ref_rec);
            }
        }
    }

    fn process_council_members(
        members: &[AccountId],
        epoch: u64,
        required_bond: u128,
        citizen_cfg: &iroha_config::parameters::actual::CitizenServiceDiscipline,
        current_height: u64,
        world: &mut WorldTransaction<'_, '_>,
        updated_citizens: &mut BTreeMap<AccountId, crate::state::CitizenshipRecord>,
    ) -> Result<(), Error> {
        for account_id in members {
            let Some(mut record) = world.citizens.get(account_id).cloned() else {
                return Err(InstructionExecutionError::InvariantViolation(
                    "council members must be registered citizens".into(),
                ));
            };
            if record.amount < required_bond {
                return Err(InstructionExecutionError::InvariantViolation(
                    "council members must meet the citizenship bond floor for the role".into(),
                ));
            }
            assign_citizen_seat(&mut record, epoch, current_height, citizen_cfg)?;
            updated_citizens.insert(account_id.clone(), record);
        }
        Ok(())
    }

    fn process_council_alternates(
        alternates: &[AccountId],
        epoch: u64,
        required_bond: u128,
        current_height: u64,
        world: &mut WorldTransaction<'_, '_>,
        updated_citizens: &mut BTreeMap<AccountId, crate::state::CitizenshipRecord>,
    ) -> Result<(), Error> {
        for account_id in alternates {
            let Some(mut record) = world.citizens.get(account_id).cloned() else {
                return Err(InstructionExecutionError::InvariantViolation(
                    "council alternates must be registered citizens".into(),
                ));
            };
            if record.amount < required_bond {
                return Err(InstructionExecutionError::InvariantViolation(
                    "council alternates must meet the citizenship bond floor for the role".into(),
                ));
            }
            ensure_citizen_available(&mut record, epoch, current_height)?;
            updated_citizens.entry(account_id.clone()).or_insert(record);
        }
        Ok(())
    }

    impl Execute for gov::EnactReferendum {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(&state_transaction.world, authority, "CanEnactGovernance") {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanEnactGovernance".into(),
                ));
            }

            let pid = self.referendum_id;
            let pid_hex = hex::encode(pid);
            let (payload, already_enacted) = load_proposal(state_transaction, pid, &pid_hex)?;
            let (code_hash_b, abi_hash_b) = extract_hashes(&payload)?;
            let key = iroha_crypto::Hash::prehashed(code_hash_b);

            let manifest_inserted = upsert_manifest(
                state_transaction,
                key,
                abi_hash_b,
                payload.manifest_provenance.as_ref(),
            )?;
            #[cfg(not(feature = "telemetry"))]
            let _ = manifest_inserted;

            if !already_enacted {
                mark_proposal_enacted(state_transaction, pid);
            }

            let instance_bound_new = bind_contract_instance(state_transaction, &payload, key)?;
            #[cfg(not(feature = "telemetry"))]
            let _ = instance_bound_new;

            #[cfg(feature = "telemetry")]
            record_enactment_telemetry(
                state_transaction,
                &payload,
                code_hash_b,
                abi_hash_b,
                manifest_inserted,
                instance_bound_new,
            );

            close_referendum_if_open(state_transaction, &pid_hex);

            if !already_enacted {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::ProposalEnacted(
                        iroha_data_model::events::data::governance::GovernanceProposalEnacted {
                            id: pid,
                        },
                    ),
                ));
            }
            Ok(())
        }
    }

    impl Execute for scode::ActivateContractInstance {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Require governance authority to activate instances
            if !has_permission(&state_transaction.world, authority, "CanEnactGovernance") {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanEnactGovernance".into(),
                ));
            }
            let key = *self.code_hash();
            let ns = self.namespace().clone();
            let cid = self.contract_id().clone();
            let ns_key = (ns, cid);
            // Enforce namespace uniqueness when governance protects namespaces to prevent cross-namespace rebinding.
            let mut protected = Vec::new();
            if let Ok(name) = core::str::FromStr::from_str("gov_protected_namespaces") {
                let id = iroha_data_model::parameter::CustomParameterId(name);
                let params = state_transaction.world.parameters.get();
                if let Some(custom) = params.custom().get(&id)
                    && let Ok(v) = custom.payload().try_into_any_norito::<Vec<String>>()
                {
                    protected = v;
                }
            }
            if !protected.is_empty() {
                if let Some(conflict) = state_transaction
                    .world
                    .contract_instances
                    .iter()
                    .find(|((ns, contract_id), _)| {
                        contract_id == self.contract_id() && ns != self.namespace()
                    })
                    .map(|(k, _)| k.clone())
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "contract_id `{}` already bound under protected namespace `{}`",
                            self.contract_id(),
                            conflict.0
                        )
                        .into(),
                    ));
                }
            }
            if let Some(existing) = state_transaction.world.contract_instances.get(&ns_key) {
                if *existing != key {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "contract instance already bound to a different code hash".into(),
                    ));
                }
                // idempotent when same
                return Ok(());
            }
            // Optional: ensure manifest exists for code_hash
            if state_transaction
                .world
                .contract_manifests
                .get(&key)
                .is_none()
            {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract("manifest for code_hash not found".into()),
                ));
            }
            state_transaction
                .world
                .contract_instances
                .insert(ns_key, key);
            state_transaction
                .world
                .emit_events(Some(SmartContractEvent::InstanceActivated(
                    ContractInstanceActivated {
                        namespace: self.namespace().clone(),
                        contract_id: self.contract_id().clone(),
                        code_hash: *self.code_hash(),
                        activated_by: authority.clone(),
                    },
                )));
            Ok(())
        }
    }

    impl Execute for scode::DeactivateContractInstance {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(&state_transaction.world, authority, "CanEnactGovernance") {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanEnactGovernance".into(),
                ));
            }
            let namespace = self.namespace().trim();
            if namespace.is_empty() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract("namespace must not be empty".into()),
                ));
            }
            let contract_id = self.contract_id().trim();
            if contract_id.is_empty() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract("contract_id must not be empty".into()),
                ));
            }
            let key = (namespace.to_owned(), contract_id.to_owned());
            let Some(prev_hash) = state_transaction
                .world
                .contract_instances
                .remove(key.clone())
            else {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract("contract instance is not active".into()),
                ));
            };
            let reason = self.reason().clone().and_then(|r| {
                let trimmed = r.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                }
            });
            state_transaction
                .world
                .emit_events(Some(SmartContractEvent::InstanceDeactivated(
                    ContractInstanceDeactivated {
                        namespace: key.0,
                        contract_id: key.1,
                        previous_code_hash: prev_hash,
                        deactivated_by: authority.clone(),
                        reason,
                    },
                )));
            Ok(())
        }
    }

    impl Execute for scode::RegisterSmartContractBytes {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Permission gate: same as manifest registration
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanRegisterSmartContractCode",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanRegisterSmartContractCode".into(),
                ));
            }
            let code = self.code().clone();
            // Parse IVM header and verify code_hash over program body
            let parsed = ivm::ProgramMetadata::parse(&code).map_err(|e| {
                InstructionExecutionError::InvariantViolation(
                    format!("invalid IVM program: {e}").into(),
                )
            })?;
            if parsed.metadata.version_major != 1 {
                return Err(InstructionExecutionError::InvariantViolation(
                    "unsupported IVM program version".into(),
                ));
            }
            let body_hash = iroha_crypto::Hash::new(&code[parsed.header_len..]);
            if body_hash != *self.code_hash() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "code_hash does not match program body".into(),
                ));
            }
            // Optional size cap via custom parameter `max_contract_code_bytes` (JSON u64)
            let mut cap_bytes: u64 = 16 * 1024 * 1024; // default 16 MiB
            if let Ok(name) = core::str::FromStr::from_str("max_contract_code_bytes") {
                let id = iroha_data_model::parameter::CustomParameterId(name);
                let params = state_transaction.world.parameters.get();
                if let Some(custom) = params.custom().get(&id) {
                    if let Ok(v) = custom.payload().try_into_any_norito::<u64>() {
                        cap_bytes = v;
                    }
                }
            }
            if (code.len() as u64) > cap_bytes {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("code bytes exceed cap: {} > {}", code.len(), cap_bytes).into(),
                ));
            }
            // Idempotent insert; if exists and differs, reject
            if let Some(existing) = state_transaction.world.contract_code.get(self.code_hash()) {
                if existing.as_slice() != code.as_slice() {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "different code bytes already stored for this code_hash".into(),
                    ));
                }
                return Ok(());
            }
            state_transaction
                .world
                .contract_code
                .insert(*self.code_hash(), code);
            state_transaction
                .world
                .emit_events(Some(SmartContractEvent::CodeRegistered(
                    ContractCodeRegistered {
                        code_hash: *self.code_hash(),
                        registrar: authority.clone(),
                    },
                )));
            Ok(())
        }
    }

    impl Execute for scode::RemoveSmartContractBytes {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanRegisterSmartContractCode",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanRegisterSmartContractCode".into(),
                ));
            }
            if state_transaction
                .world
                .contract_manifests
                .get(self.code_hash())
                .is_some()
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    "contract manifest referencing code_hash still exists".into(),
                ));
            }
            let code_in_use = state_transaction
                .world
                .contract_instances
                .iter()
                .any(|(_, hash)| hash == self.code_hash());
            if code_in_use {
                return Err(InstructionExecutionError::InvariantViolation(
                    "active contract instance references this code_hash".into(),
                ));
            }
            let removed = state_transaction
                .world
                .contract_code
                .remove(*self.code_hash());
            if removed.is_none() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "contract code for code_hash not found".into(),
                    ),
                ));
            }
            let reason = self.reason().clone().and_then(|r| {
                let trimmed = r.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                }
            });
            state_transaction
                .world
                .emit_events(Some(SmartContractEvent::CodeRemoved(ContractCodeRemoved {
                    code_hash: *self.code_hash(),
                    removed_by: authority.clone(),
                    reason,
                })));
            Ok(())
        }
    }

    impl Execute for gov::FinalizeReferendum {
        #[allow(clippy::too_many_lines)]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Compute plain tally from active locks (ignore expired)
            use iroha_data_model::events::data::governance as gev;
            let mut approve: u128 = 0;
            let mut reject: u128 = 0;
            let now_h = state_transaction._curr_block.height().get();
            if let Some(locks) = state_transaction
                .world
                .governance_locks
                .get(&self.referendum_id)
            {
                let step = state_transaction.gov.conviction_step_blocks.max(1);
                let max_c = state_transaction.gov.max_conviction;
                for rec in locks.locks.values() {
                    if rec.expiry_height < now_h {
                        continue;
                    }
                    // integer sqrt and conviction factor
                    let base = integer_sqrt_u128(rec.amount);
                    let mut f = 1u64 + (rec.duration_blocks / step);
                    if f > max_c {
                        f = max_c;
                    }
                    let w = base.saturating_mul(u128::from(f));
                    match rec.direction {
                        0 => approve = approve.saturating_add(w),
                        1 => reject = reject.saturating_add(w),
                        _ => {}
                    }
                }
            }
            // ZK tally fallback: if an election exists, prefer its tally if shape matches 2 options
            if let Some(e) = state_transaction.world.elections.get(&self.referendum_id) {
                if e.tally.len() >= 2 {
                    approve = u128::from(e.tally[0]);
                    reject = u128::from(e.tally[1]);
                }
            }
            // Note: closing by height is automatic in State::block; no need to change status here.
            // Decide and emit Approved/Rejected with thresholds
            let turnout = approve.saturating_add(reject);
            let turnout_all = turnout.saturating_add(0); // abstain ignored here; auto close uses abstain
            let num = state_transaction.gov.approval_threshold_q_num;
            let den = state_transaction.gov.approval_threshold_q_den.max(1);
            let decision_approve = if turnout_all >= state_transaction.gov.min_turnout {
                let lhs = approve.saturating_mul(u128::from(den));
                let rhs = turnout.saturating_mul(u128::from(num));
                lhs >= rhs
            } else {
                false
            };
            if decision_approve {
                state_transaction
                    .world
                    .emit_events(Some(gev::GovernanceEvent::ProposalApproved(
                        gev::GovernanceProposalApproved {
                            id: self.proposal_id,
                        },
                    )));
                if let Some(rec) = state_transaction
                    .world
                    .governance_proposals
                    .get_mut(&self.proposal_id)
                {
                    rec.status = crate::state::GovernanceProposalStatus::Approved;
                }
            } else {
                state_transaction
                    .world
                    .emit_events(Some(gev::GovernanceEvent::ProposalRejected(
                        gev::GovernanceProposalRejected {
                            id: self.proposal_id,
                        },
                    )));
                if let Some(rec) = state_transaction
                    .world
                    .governance_proposals
                    .get_mut(&self.proposal_id)
                {
                    rec.status = crate::state::GovernanceProposalStatus::Rejected;
                }
            }
            Ok(())
        }
    }

    impl Execute for gov::ApproveGovernanceProposal {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let ctx = load_approval_context(&self, state_transaction)?;
            ensure_parliament_member(authority, &ctx.roster)?;
            persist_parliament_bodies_if_missing(&ctx, state_transaction);

            let Some((approvals_count, required, approvals)) =
                record_parliament_approval(&self, authority, &ctx, state_transaction)
            else {
                return Ok(());
            };

            state_transaction
                .world
                .emit_events(Some(GovernanceEvent::ParliamentApprovalRecorded(
                    GovernanceParliamentApprovalRecorded {
                        proposal_id: self.proposal_id,
                        epoch: ctx.epoch,
                        body: self.body,
                        approvals: approvals_count,
                        required,
                    },
                )));
            maybe_open_referendum(&ctx, &approvals, state_transaction);
            Ok(())
        }
    }

    struct ApprovalContext {
        rid: String,
        referendum: crate::state::GovernanceReferendumRecord,
        bodies: iroha_data_model::governance::types::ParliamentBodies,
        roster: iroha_data_model::governance::types::ParliamentRoster,
        epoch: u64,
        now_h: u64,
        quorum_bps: u16,
    }

    fn load_approval_context(
        req: &gov::ApproveGovernanceProposal,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<ApprovalContext, Error> {
        let _proposal = state_transaction
            .world
            .governance_proposals
            .get(&req.proposal_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "governance proposal not found".into(),
                )
            })?;
        let rid = hex::encode(req.proposal_id);
        let referendum = state_transaction
            .world
            .governance_referenda
            .get(&rid)
            .copied()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "referendum not found for governance proposal".into(),
                )
            })?;
        if referendum.status == crate::state::GovernanceReferendumStatus::Closed {
            return Err(InstructionExecutionError::InvariantViolation(
                "referendum already closed".into(),
            ));
        }
        let term_blocks = state_transaction.gov.parliament_term_blocks.max(1);
        let now_h = state_transaction._curr_block.height().get();
        let epoch = now_h.saturating_sub(1).saturating_div(term_blocks);
        let council = state_transaction
            .world
            .council
            .get(&epoch)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "council roster missing for current epoch".into(),
                )
            })?;
        let bodies = state_transaction
            .world
            .parliament_bodies
            .get(&epoch)
            .cloned()
            .unwrap_or_else(|| derive_parliament_bodies(&council));
        if bodies.selection_epoch != epoch {
            return Err(InstructionExecutionError::InvariantViolation(
                "parliament roster epoch mismatch".into(),
            ));
        }
        let Some(roster) = bodies.rosters.get(&req.body).cloned() else {
            return Err(InstructionExecutionError::InvariantViolation(
                "parliament roster missing for requested body".into(),
            ));
        };
        if roster.members.is_empty() {
            return Err(InstructionExecutionError::InvariantViolation(
                "parliament roster empty for requested body".into(),
            ));
        }

        Ok(ApprovalContext {
            rid,
            referendum,
            bodies,
            roster,
            epoch,
            now_h,
            quorum_bps: state_transaction.gov.parliament_quorum_bps,
        })
    }

    fn ensure_parliament_member(
        authority: &AccountId,
        roster: &iroha_data_model::governance::types::ParliamentRoster,
    ) -> Result<(), Error> {
        let eligible = roster
            .members
            .iter()
            .chain(roster.alternates.iter())
            .any(|member| member == authority);
        if eligible {
            Ok(())
        } else {
            Err(InstructionExecutionError::InvariantViolation(
                "only seated members or alternates may approve proposals for this body".into(),
            ))
        }
    }

    fn persist_parliament_bodies_if_missing(
        ctx: &ApprovalContext,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) {
        if state_transaction
            .world
            .parliament_bodies
            .get(&ctx.epoch)
            .is_none()
        {
            state_transaction
                .world
                .parliament_bodies
                .insert(ctx.epoch, ctx.bodies.clone());
        }
    }

    fn record_parliament_approval(
        req: &gov::ApproveGovernanceProposal,
        authority: &AccountId,
        ctx: &ApprovalContext,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Option<(u32, u32, crate::state::GovernanceStageApprovals)> {
        let committee_size = ctx.roster.members.len();
        let required = crate::state::council_quorum_threshold(committee_size, ctx.quorum_bps);
        let mut approvals = state_transaction
            .world
            .governance_stage_approvals
            .get(&ctx.rid)
            .cloned()
            .unwrap_or_default();
        let (inserted, approvals_count, required) = {
            let stage_record =
                approvals.ensure_stage(req.body, ctx.epoch, required, ctx.quorum_bps);
            let inserted = stage_record.record(authority.clone());
            let approvals_count = u32::try_from(stage_record.approvers.len()).unwrap_or(u32::MAX);
            let required = stage_record.required;
            (inserted, approvals_count, required)
        };
        if !inserted {
            return None;
        }
        let approvals_snapshot = approvals.clone();
        state_transaction
            .world
            .governance_stage_approvals
            .insert(ctx.rid.clone(), approvals_snapshot);
        Some((approvals_count, required, approvals))
    }

    fn maybe_open_referendum(
        ctx: &ApprovalContext,
        approvals: &crate::state::GovernanceStageApprovals,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) {
        let rules_ready = approvals.quorum_met(ParliamentBody::RulesCommittee, ctx.epoch);
        let agenda_ready = approvals.quorum_met(ParliamentBody::AgendaCouncil, ctx.epoch);
        if rules_ready
            && agenda_ready
            && ctx.referendum.status == crate::state::GovernanceReferendumStatus::Proposed
            && ctx.now_h >= ctx.referendum.h_start
            && ctx.now_h <= ctx.referendum.h_end
        {
            let mut rec = ctx.referendum;
            rec.status = crate::state::GovernanceReferendumStatus::Open;
            state_transaction
                .world
                .governance_referenda
                .insert(ctx.rid.clone(), rec);
            state_transaction
                .world
                .emit_events(Some(GovernanceEvent::ReferendumOpened(
                    iroha_data_model::events::data::governance::GovernanceReferendumOpened {
                        id: ctx.rid.clone(),
                        h_start: rec.h_start,
                        h_end: rec.h_end,
                    },
                )));
        }
    }

    // Persist council membership for an epoch.
    impl Execute for gov::PersistCouncilForEpoch {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let required_bond =
                required_citizenship_bond_for_role(&state_transaction.gov, "council");
            let mut updated_citizens: BTreeMap<AccountId, crate::state::CitizenshipRecord> =
                BTreeMap::new();
            let citizen_cfg = &state_transaction.gov.citizen_service;
            let current_height = state_transaction._curr_block.height().get();

            if state_transaction.gov.citizenship_bond_amount > 0 {
                process_council_members(
                    &self.members,
                    self.epoch,
                    required_bond,
                    citizen_cfg,
                    current_height,
                    &mut state_transaction.world,
                    &mut updated_citizens,
                )?;
                process_council_alternates(
                    &self.alternates,
                    self.epoch,
                    required_bond,
                    current_height,
                    &mut state_transaction.world,
                    &mut updated_citizens,
                )?;
            }
            for (account, record) in updated_citizens {
                state_transaction.world.citizens.insert(account, record);
            }
            // Idempotent insert: if entry exists and draw metadata match, accept; if different, overwrite.
            let mut rec = crate::state::CouncilState {
                epoch: self.epoch,
                members: self.members.clone(),
                alternates: self.alternates.clone(),
                verified: self.verified,
                candidate_count: self.candidates_count,
                derived_by: self.derived_by,
            };
            if let Some(existing) = state_transaction.world.council.get(&self.epoch) {
                let same_members = existing.members == self.members;
                let same_alternates = existing.alternates == self.alternates;
                let same_verification = existing.verified == self.verified;
                let same_candidate_count = existing.candidate_count == self.candidates_count;
                let same_derivation = existing.derived_by == self.derived_by;
                if same_members
                    && same_alternates
                    && same_verification
                    && same_candidate_count
                    && same_derivation
                {
                    rec = existing.clone();
                    let already_recorded = state_transaction
                        .world
                        .parliament_bodies
                        .get(&self.epoch)
                        .is_some();
                    if already_recorded {
                        return Ok(());
                    }
                } else {
                    rec.epoch = existing.epoch; // ensure consistent
                }
            }
            state_transaction
                .world
                .council
                .insert(self.epoch, rec.clone());
            // Emit event for auditability
            let members_count = u32::try_from(self.members.len()).unwrap_or(u32::MAX);
            let alternates_count = u32::try_from(self.alternates.len()).unwrap_or(u32::MAX);
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::CouncilPersisted(
                    iroha_data_model::events::data::governance::GovernanceCouncilPersisted {
                        epoch: self.epoch,
                        members_count,
                        alternates_count,
                        verified: self.verified,
                        candidates_count: self.candidates_count,
                        derived_by: self.derived_by,
                    },
                ),
            ));
            let bodies = derive_parliament_bodies(&rec);
            state_transaction
                .world
                .parliament_bodies
                .insert(rec.epoch, bodies.clone());
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::ParliamentSelected(
                    iroha_data_model::events::data::governance::GovernanceParliamentSelected {
                        selection_epoch: rec.epoch,
                        bodies,
                    },
                ),
            ));
            Ok(())
        }
    }

    impl Execute for gov::RegisterCitizen {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if self.owner != *authority {
                return Err(InstructionExecutionError::InvariantViolation(
                    "owner must equal authority".into(),
                ));
            }
            if self.amount < state_transaction.gov.citizenship_bond_amount {
                return Err(InstructionExecutionError::InvariantViolation(
                    "citizenship bond below minimum".into(),
                ));
            }
            let existing = state_transaction.world.citizens.get(&self.owner).cloned();
            if let Some(ref rec) = existing {
                if self.amount < rec.amount {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "citizenship bond cannot decrease".into(),
                    ));
                }
            }
            let delta = self
                .amount
                .saturating_sub(existing.as_ref().map_or(0, |rec| rec.amount));
            if delta > 0 {
                let (owner_asset_id, escrow_asset_id) =
                    citizenship_asset_ids(&state_transaction.gov, &self.owner);
                let spec = state_transaction.numeric_spec_for(owner_asset_id.definition())?;
                let delta_numeric = numeric_with_spec(delta, spec)?;
                crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with(
                    &delta_numeric,
                    spec,
                )?;
                state_transaction
                    .world
                    .withdraw_numeric_asset(&owner_asset_id, &delta_numeric)?;
                state_transaction
                    .world
                    .deposit_numeric_asset(&escrow_asset_id, &delta_numeric)?;
            }
            let bonded_height = state_transaction._curr_block.height().get();
            let record = crate::state::CitizenshipRecord::new(
                self.owner.clone(),
                self.amount,
                bonded_height,
            );
            state_transaction
                .world
                .citizens
                .insert(self.owner.clone(), record.clone());
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::CitizenRegistered(
                    iroha_data_model::events::data::governance::GovernanceCitizenRegistered {
                        owner: record.owner,
                        amount: record.amount,
                    },
                ),
            ));
            #[cfg(feature = "telemetry")]
            {
                let citizens_total = u64::try_from(state_transaction.world.citizens.iter().count())
                    .unwrap_or(u64::MAX);
                state_transaction
                    .telemetry
                    .record_citizens_total(citizens_total);
            }
            Ok(())
        }
    }

    impl Execute for gov::UnregisterCitizen {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if self.owner != *authority {
                return Err(InstructionExecutionError::InvariantViolation(
                    "owner must equal authority".into(),
                ));
            }
            let Some(record) = state_transaction.world.citizens.get(&self.owner).cloned() else {
                return Err(InstructionExecutionError::InvariantViolation(
                    "citizen not found".into(),
                ));
            };
            let (owner_asset_id, escrow_asset_id) =
                citizenship_asset_ids(&state_transaction.gov, &self.owner);
            let spec = state_transaction.numeric_spec_for(owner_asset_id.definition())?;
            let amount_numeric = numeric_with_spec(record.amount, spec)?;
            crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with(
                &amount_numeric,
                spec,
            )?;
            state_transaction
                .world
                .withdraw_numeric_asset(&escrow_asset_id, &amount_numeric)?;
            state_transaction
                .world
                .deposit_numeric_asset(&owner_asset_id, &amount_numeric)?;
            state_transaction.world.citizens.remove(self.owner.clone());
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::CitizenRevoked(
                    iroha_data_model::events::data::governance::GovernanceCitizenRevoked {
                        owner: record.owner,
                        amount: record.amount,
                    },
                ),
            ));
            #[cfg(feature = "telemetry")]
            {
                let citizens_total = u64::try_from(state_transaction.world.citizens.iter().count())
                    .unwrap_or(u64::MAX);
                state_transaction
                    .telemetry
                    .record_citizens_total(citizens_total);
            }
            Ok(())
        }
    }

    impl Execute for gov::RecordCitizenServiceOutcome {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if self.role.trim().is_empty() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract("role must not be blank".into()),
                ));
            }
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanRecordCitizenService",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanRecordCitizenService".into(),
                ));
            }
            let Some(mut record) = state_transaction.world.citizens.get(&self.owner).cloned()
            else {
                return Err(InstructionExecutionError::InvariantViolation(
                    "citizen not found for service record".into(),
                ));
            };
            let required_bond =
                required_citizenship_bond_for_role(&state_transaction.gov, &self.role);
            if record.amount < required_bond {
                return Err(InstructionExecutionError::InvariantViolation(
                    "citizenship bond below role requirement".into(),
                ));
            }
            let citizen_cfg = state_transaction.gov.citizen_service.clone();
            let current_height = state_transaction._curr_block.height().get();
            reset_citizen_epoch(&mut record, self.epoch);
            let slashed = match self.event {
                gov::CitizenServiceEvent::Decline => {
                    let penalty = if record.declines_used >= citizen_cfg.free_declines_per_epoch {
                        slash_citizenship_bond(
                            &mut record,
                            citizen_cfg.decline_slash_bps,
                            state_transaction,
                        )?
                    } else {
                        0
                    };
                    record.declines_used = record.declines_used.saturating_add(1);
                    let cooldown = current_height.saturating_add(citizen_cfg.seat_cooldown_blocks);
                    record.cooldown_until = record.cooldown_until.max(cooldown);
                    penalty
                }
                gov::CitizenServiceEvent::NoShow => {
                    record.no_show_strikes = record.no_show_strikes.saturating_add(1);
                    let cooldown = current_height.saturating_add(citizen_cfg.seat_cooldown_blocks);
                    record.cooldown_until = record.cooldown_until.max(cooldown);
                    slash_citizenship_bond(
                        &mut record,
                        citizen_cfg.no_show_slash_bps,
                        state_transaction,
                    )?
                }
                gov::CitizenServiceEvent::Misconduct => {
                    record.misconduct_strikes = record.misconduct_strikes.saturating_add(1);
                    let cooldown = current_height.saturating_add(citizen_cfg.seat_cooldown_blocks);
                    record.cooldown_until = record.cooldown_until.max(cooldown);
                    slash_citizenship_bond(
                        &mut record,
                        citizen_cfg.misconduct_slash_bps,
                        state_transaction,
                    )?
                }
            };
            state_transaction
                .world
                .citizens
                .insert(self.owner.clone(), record.clone());
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::CitizenServiceRecorded(
                    iroha_data_model::events::data::governance::GovernanceCitizenServiceRecorded {
                        owner: self.owner.clone(),
                        epoch: self.epoch,
                        role: self.role.clone(),
                        event: self.event,
                        slashed,
                        cooldown_until: record.cooldown_until,
                    },
                ),
            ));
            #[cfg(feature = "telemetry")]
            state_transaction
                .telemetry
                .record_citizen_service_event(self.event, slashed);
            Ok(())
        }
    }

    fn integer_sqrt_u128(n: u128) -> u128 {
        if n == 0 {
            return 0;
        }
        // Newton's method
        let mut x0 = n;
        let mut x1 = u128::midpoint(x0, n / x0);
        while x1 < x0 {
            x0 = x1;
            x1 = u128::midpoint(x0, n / x0);
        }
        x0
    }

    fn require_runtime_upgrade_permission(
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !has_permission(
            &state_transaction.world,
            authority,
            "CanManageRuntimeUpgrades",
        ) {
            return Err(InstructionExecutionError::InvariantViolation(
                "not permitted: CanManageRuntimeUpgrades".into(),
            ));
        }
        Ok(())
    }

    fn decode_runtime_upgrade_manifest(
        bytes: &[u8],
    ) -> Result<(iroha_data_model::runtime::RuntimeUpgradeManifest, Vec<u8>), Error> {
        let manifest: iroha_data_model::runtime::RuntimeUpgradeManifest =
            norito::decode_from_bytes(bytes).map_err(|e| {
                InstructionExecutionError::InvariantViolation(
                    format!("invalid manifest: {e}").into(),
                )
            })?;
        let canonical_bytes = manifest.canonical_bytes();
        if canonical_bytes != *bytes {
            return Err(InstructionExecutionError::InvariantViolation(
                "manifest bytes must use canonical Norito framing".into(),
            ));
        }
        if manifest.end_height <= manifest.start_height {
            return Err(InstructionExecutionError::InvariantViolation(
                "invalid window: end_height must be > start_height".into(),
            ));
        }
        Ok((manifest, canonical_bytes))
    }

    fn runtime_upgrade_provenance_error(
        reason: iroha_data_model::runtime::RuntimeUpgradeProvenanceError,
        _state_transaction: &StateTransaction<'_, '_>,
    ) -> Error {
        #[cfg(feature = "telemetry")]
        _state_transaction
            .telemetry
            .record_runtime_upgrade_provenance_rejection(reason);
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(format!(
            "runtime_upgrade_provenance:{}",
            reason.as_label()
        )))
    }

    fn validate_runtime_upgrade_provenance(
        manifest: &iroha_data_model::runtime::RuntimeUpgradeManifest,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        use iroha_data_model::runtime::RuntimeUpgradeProvenanceError;

        let policy = &state_transaction.gov.runtime_upgrade_provenance;
        let has_provenance = !(manifest.sbom_digests.is_empty()
            && manifest.slsa_attestation.is_empty()
            && manifest.provenance.is_empty());
        if policy.mode.is_required() && !has_provenance {
            return Err(runtime_upgrade_provenance_error(
                RuntimeUpgradeProvenanceError::MissingProvenance,
                state_transaction,
            ));
        }
        if !has_provenance {
            return Ok(());
        }

        if policy.require_sbom && manifest.sbom_digests.is_empty() {
            return Err(runtime_upgrade_provenance_error(
                RuntimeUpgradeProvenanceError::MissingSbom,
                state_transaction,
            ));
        }
        for entry in &manifest.sbom_digests {
            if entry.algorithm.trim().is_empty() || entry.digest.is_empty() {
                return Err(runtime_upgrade_provenance_error(
                    RuntimeUpgradeProvenanceError::InvalidSbomDigest,
                    state_transaction,
                ));
            }
        }
        if policy.require_slsa && manifest.slsa_attestation.is_empty() {
            return Err(runtime_upgrade_provenance_error(
                RuntimeUpgradeProvenanceError::MissingSlsaAttestation,
                state_transaction,
            ));
        }

        if policy.signature_threshold > 0 && manifest.provenance.is_empty() {
            return Err(runtime_upgrade_provenance_error(
                RuntimeUpgradeProvenanceError::MissingSignatures,
                state_transaction,
            ));
        }
        if !manifest.provenance.is_empty() {
            let payload = manifest.signature_payload_bytes();
            let mut trusted_signers = BTreeSet::new();
            for provenance in &manifest.provenance {
                provenance
                    .signature
                    .verify(&provenance.signer, &payload)
                    .map_err(|_| {
                        runtime_upgrade_provenance_error(
                            RuntimeUpgradeProvenanceError::InvalidSignature,
                            state_transaction,
                        )
                    })?;
                if !policy.trusted_signers.is_empty()
                    && !policy.trusted_signers.contains(&provenance.signer)
                {
                    return Err(runtime_upgrade_provenance_error(
                        RuntimeUpgradeProvenanceError::UntrustedSigner,
                        state_transaction,
                    ));
                }
                if policy.trusted_signers.contains(&provenance.signer) {
                    trusted_signers.insert(provenance.signer.clone());
                }
            }
            if policy.signature_threshold > 0 && trusted_signers.len() < policy.signature_threshold
            {
                return Err(runtime_upgrade_provenance_error(
                    RuntimeUpgradeProvenanceError::SignatureThresholdNotMet,
                    state_transaction,
                ));
            }
        }

        Ok(())
    }

    fn policy_for_abi_version(
        abi_version: u16,
    ) -> Result<(ivm::SyscallPolicy, u8), InstructionExecutionError> {
        if abi_version != 1 {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("unsupported abi_version {abi_version}; expected 1").into(),
            ));
        }
        Ok((ivm::SyscallPolicy::AbiV1, 1))
    }

    fn allowed_syscalls_for_policy(policy: ivm::SyscallPolicy) -> BTreeSet<u32> {
        ivm::syscalls::syscalls_for_policy(policy)
            .iter()
            .copied()
            .collect()
    }

    fn allowed_pointer_types_for_policy(policy: ivm::SyscallPolicy) -> BTreeSet<ivm::PointerType> {
        ivm::pointer_abi::policy_pointer_types(policy)
            .iter()
            .copied()
            .collect()
    }

    fn latest_active_abi_version(state_transaction: &StateTransaction<'_, '_>) -> u16 {
        state_transaction
            .world
            .runtime_upgrades
            .iter()
            .filter_map(|(_, rec)| match rec.status {
                iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(_) => {
                    Some(rec.manifest.abi_version)
                }
                _ => None,
            })
            .max()
            .unwrap_or(1)
    }

    fn validate_runtime_upgrade_surface(
        manifest: &iroha_data_model::runtime::RuntimeUpgradeManifest,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let (policy, _) = policy_for_abi_version(manifest.abi_version)?;
        let computed = ivm::syscalls::compute_abi_hash(policy);
        if manifest.abi_hash != computed {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("abi_hash mismatch for abi_version {}", manifest.abi_version).into(),
            ));
        }

        let previous = latest_active_abi_version(state_transaction);
        let (prev_policy, _) = policy_for_abi_version(previous)?;
        let prev_syscalls = allowed_syscalls_for_policy(prev_policy);
        let new_syscalls = allowed_syscalls_for_policy(policy);
        if !new_syscalls.is_superset(&prev_syscalls) {
            return Err(InstructionExecutionError::InvariantViolation(
                "syscall surface must be additive".into(),
            ));
        }
        let expected_syscall_delta: BTreeSet<u32> =
            new_syscalls.difference(&prev_syscalls).copied().collect();
        let manifest_syscalls: BTreeSet<u32> = manifest
            .added_syscalls
            .iter()
            .copied()
            .map(u32::from)
            .collect();
        if expected_syscall_delta != manifest_syscalls {
            return Err(InstructionExecutionError::InvariantViolation(
                "added_syscalls must list exactly the new syscall numbers".into(),
            ));
        }

        let prev_ptrs = allowed_pointer_types_for_policy(prev_policy);
        let new_ptrs = allowed_pointer_types_for_policy(policy);
        if !new_ptrs.is_superset(&prev_ptrs) {
            return Err(InstructionExecutionError::InvariantViolation(
                "pointer-type surface must be additive".into(),
            ));
        }
        let expected_ptr_delta: BTreeSet<u16> = new_ptrs
            .difference(&prev_ptrs)
            .map(|ty| *ty as u16)
            .collect();
        let manifest_ptrs: BTreeSet<u16> = manifest.added_pointer_types.iter().copied().collect();
        if expected_ptr_delta != manifest_ptrs {
            return Err(InstructionExecutionError::InvariantViolation(
                "added_pointer_types must list exactly the new pointer-type identifiers".into(),
            ));
        }

        Ok(())
    }

    fn ensure_runtime_upgrade_abi_version(
        manifest: &iroha_data_model::runtime::RuntimeUpgradeManifest,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        policy_for_abi_version(manifest.abi_version)?;
        let max_active = state_transaction
            .world
            .runtime_upgrades
            .iter()
            .filter_map(|(_, rec)| match rec.status {
                iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(_) => {
                    Some(rec.manifest.abi_version)
                }
                _ => None,
            })
            .max()
            .unwrap_or(1);
        if manifest.abi_version <= max_active {
            return Err(InstructionExecutionError::InvariantViolation(
                "abi_version must be strictly greater than any active version".into(),
            ));
        }
        Ok(())
    }

    fn ensure_runtime_upgrade_no_overlap(
        manifest: &iroha_data_model::runtime::RuntimeUpgradeManifest,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let new_start = manifest.start_height;
        let new_end = manifest.end_height;
        for (_id, rec) in state_transaction.world.runtime_upgrades.iter() {
            if matches!(
                rec.status,
                iroha_data_model::runtime::RuntimeUpgradeStatus::Proposed
                    | iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(_)
            ) {
                let s = rec.manifest.start_height;
                let e = rec.manifest.end_height;
                if new_start < e && s < new_end {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "runtime upgrade window overlaps existing proposal/activation".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    fn handle_existing_runtime_upgrade(
        id: iroha_data_model::runtime::RuntimeUpgradeId,
        manifest: &iroha_data_model::runtime::RuntimeUpgradeManifest,
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<bool, Error> {
        if let Some(existing) = state_transaction.world.runtime_upgrades.get(&id) {
            if existing.manifest == *manifest
                && matches!(
                    existing.status,
                    iroha_data_model::runtime::RuntimeUpgradeStatus::Proposed
                )
                && existing.proposer == *authority
            {
                return Ok(true);
            }
            return Err(InstructionExecutionError::InvariantViolation(
                "runtime upgrade already proposed".into(),
            ));
        }
        Ok(false)
    }

    fn validate_verifying_key_update(
        id: &VerifyingKeyId,
        new: &VerifyingKeyRecord,
        old: &VerifyingKeyRecord,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if matches!(old.status, ConfidentialStatus::Withdrawn) {
            return Err(InstructionExecutionError::InvariantViolation(
                "cannot update withdrawn verifying key".into(),
            ));
        }
        if new.version <= old.version {
            return Err(InstructionExecutionError::InvariantViolation(
                "verifying key version must increase".into(),
            ));
        }
        if new.backend != BackendTag::Halo2IpaPasta {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "verifying key backend must be Halo2IpaPasta".into(),
                ),
            ));
        }
        if !new.curve.eq_ignore_ascii_case("pallas") {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "verifying key curve must be \"pallas\"".into(),
                ),
            ));
        }
        let id_backend = id.backend.as_str();
        if !id_backend.starts_with("halo2/") || id_backend.contains("bn254") {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "verifying key id backend must target halo2 IPA (no bn254)".into(),
                ),
            ));
        }
        if let Some(vk) = &new.key {
            if new.commitment != hash_vk(vk) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key commitment mismatch".into(),
                ));
            }
            if vk.backend != id.backend {
                return Err(InstructionExecutionError::InvariantViolation(
                    "vk backend does not match id backend".into(),
                ));
            }
        }
        let new_key = (new.circuit_id.clone(), new.version);
        if let Some(existing) = state_transaction
            .world
            .verifying_keys_by_circuit
            .get(&new_key)
        {
            if existing != id {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key circuit/version already registered".into(),
                ));
            }
        }
        if new.circuit_id.trim().is_empty() {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "verifying key circuit_id must not be empty".into(),
                ),
            ));
        }
        if new.public_inputs_schema_hash == [0u8; 32] {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "verifying key public_inputs_schema_hash must be set".into(),
                ),
            ));
        }
        if new.gas_schedule_id.is_none() {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "verifying key gas_schedule_id must be set".into(),
                ),
            ));
        }
        Ok(())
    }

    // Runtime Upgrade Governance — minimal implementation (see docs/source/runtime_upgrades.md)
    impl Execute for runtime_upgrade::ProposeRuntimeUpgrade {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            require_runtime_upgrade_permission(authority, state_transaction)?;
            let (manifest, canonical_bytes) =
                decode_runtime_upgrade_manifest(self.manifest_bytes())?;
            let id =
                iroha_data_model::runtime::RuntimeUpgradeId::from_manifest_bytes(&canonical_bytes);
            if handle_existing_runtime_upgrade(id, &manifest, authority, state_transaction)? {
                return Ok(());
            }
            validate_runtime_upgrade_provenance(&manifest, state_transaction)?;
            ensure_runtime_upgrade_abi_version(&manifest, state_transaction)?;
            validate_runtime_upgrade_surface(&manifest, state_transaction)?;
            ensure_runtime_upgrade_no_overlap(&manifest, state_transaction)?;
            let created_height = state_transaction._curr_block.height().get();
            let record = iroha_data_model::runtime::RuntimeUpgradeRecord {
                manifest: manifest.clone(),
                status: iroha_data_model::runtime::RuntimeUpgradeStatus::Proposed,
                proposer: authority.clone(),
                created_height,
            };
            state_transaction.world.runtime_upgrades.insert(id, record);
            // Emit Proposed event
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::runtime_upgrade::RuntimeUpgradeEvent::Proposed(
                    iroha_data_model::events::data::runtime_upgrade::RuntimeUpgradeProposed {
                        id,
                        abi_version: manifest.abi_version,
                        start_height: manifest.start_height,
                        end_height: manifest.end_height,
                    },
                ),
            ));
            #[cfg(feature = "telemetry")]
            {
                state_transaction
                    .telemetry
                    .inc_runtime_upgrade_event("proposed");
            }
            Ok(())
        }
    }

    impl Execute for runtime_upgrade::ActivateRuntimeUpgrade {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageRuntimeUpgrades",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageRuntimeUpgrades".into(),
                ));
            }
            let id = *self.id();
            let Some(rec) = state_transaction.world.runtime_upgrades.get(&id).cloned() else {
                return Err(FindError::Permission(Box::new(Permission::new(
                    "RuntimeUpgradeNotFound".into(),
                    iroha_primitives::json::Json::from("id"),
                )))
                .into());
            };
            validate_runtime_upgrade_provenance(&rec.manifest, state_transaction)?;
            validate_runtime_upgrade_surface(&rec.manifest, state_transaction)?;
            let h = state_transaction._curr_block.height().get();
            match rec.status {
                iroha_data_model::runtime::RuntimeUpgradeStatus::Proposed => {
                    if h != rec.manifest.start_height || h >= rec.manifest.end_height {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "activation height is outside of the scheduled window".into(),
                        ));
                    }
                    // Flip to ActivatedAt(h)
                    let mut new_rec = rec.clone();
                    new_rec.status =
                        iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(h);
                    state_transaction.world.runtime_upgrades.insert(id, new_rec);
                    // Emit Activated event
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::runtime_upgrade::RuntimeUpgradeEvent::Activated(
                            iroha_data_model::events::data::runtime_upgrade::RuntimeUpgradeActivated {
                                id,
                                abi_version: rec.manifest.abi_version,
                                at_height: h,
                            },
                        ),
                    ));
                    #[cfg(feature = "telemetry")]
                    {
                        state_transaction
                            .telemetry
                            .inc_runtime_upgrade_event("activated");
                    }
                    Ok(())
                }
                iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(prev) if prev == h => {
                    // Idempotent replay: already activated at this height.
                    Ok(())
                }
                iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(prev) => {
                    Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot activate: already activated at height {prev}, current block {h}"
                        )
                        .into(),
                    ))
                }
                _ => Err(InstructionExecutionError::InvariantViolation(
                    "cannot activate: status is not Proposed".into(),
                )),
            }
        }
    }

    impl Execute for runtime_upgrade::CancelRuntimeUpgrade {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageRuntimeUpgrades",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageRuntimeUpgrades".into(),
                ));
            }
            let id = *self.id();
            let Some(rec) = state_transaction.world.runtime_upgrades.get(&id).cloned() else {
                return Err(FindError::Permission(Box::new(Permission::new(
                    "RuntimeUpgradeNotFound".into(),
                    iroha_primitives::json::Json::from("id"),
                )))
                .into());
            };
            if !matches!(
                rec.status,
                iroha_data_model::runtime::RuntimeUpgradeStatus::Proposed
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "cannot cancel: status is not Proposed".into(),
                ));
            }
            let h = state_transaction._curr_block.height().get();
            if h >= rec.manifest.start_height {
                return Err(InstructionExecutionError::InvariantViolation(
                    "cannot cancel at/after start_height".into(),
                ));
            }
            // Flip to Canceled
            let mut new_rec = rec.clone();
            new_rec.status = iroha_data_model::runtime::RuntimeUpgradeStatus::Canceled;
            state_transaction.world.runtime_upgrades.insert(id, new_rec);
            // Emit Canceled event
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::runtime_upgrade::RuntimeUpgradeEvent::Canceled(
                    iroha_data_model::events::data::runtime_upgrade::RuntimeUpgradeCanceled { id },
                ),
            ));
            #[cfg(feature = "telemetry")]
            {
                state_transaction
                    .telemetry
                    .inc_runtime_upgrade_event("canceled");
            }
            Ok(())
        }
    }

    impl Execute for verifying_keys::UpdateVerifyingKey {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageVerifyingKeys",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageVerifyingKeys".into(),
                ));
            }
            let id = self.id().clone();
            let new = self.record().clone();
            let Some(old) = state_transaction.world.verifying_keys.get(&id).cloned() else {
                return Err(FindError::Permission(Box::new(Permission::new(
                    "VerifyingKeyMissing".into(),
                    iroha_primitives::json::Json::from(
                        format!("{}::{}", id.backend, id.name).as_str(),
                    ),
                )))
                .into());
            };
            validate_verifying_key_update(&id, &new, &old, state_transaction)?;
            let new_key = (new.circuit_id.clone(), new.version);
            let old_key = (old.circuit_id.clone(), old.version);
            state_transaction.world.verifying_keys.remove(id.clone());
            state_transaction
                .world
                .verifying_keys
                .insert(id.clone(), new.clone());
            state_transaction
                .world
                .verifying_keys_by_circuit
                .remove(old_key);
            state_transaction
                .world
                .verifying_keys_by_circuit
                .insert(new_key, id.clone());
            // Emit verifying key updated event
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::verifying_keys::VerifyingKeyEvent::Updated(
                    iroha_data_model::events::data::verifying_keys::VerifyingKeyUpdated {
                        id: id.clone(),
                        record: new.clone(),
                    },
                ),
            ));
            Ok(())
        }
    }

    impl Execute for consensus_keys::RegisterConsensusKey {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            ensure_can_manage_consensus_keys(authority, state_transaction)?;

            let id = self.id().clone();
            let params = state_transaction.world.parameters.get();
            let block_height = state_transaction.block_height();
            let record = prepare_consensus_key_registration(
                &id,
                self.record().clone(),
                block_height,
                state_transaction,
            )?;
            validate_consensus_key_record(
                &record,
                &params.sumeragi,
                None,
                block_height,
                state_transaction._curr_block.is_genesis(),
            )?;
            commit_consensus_key_registration(state_transaction, &id, record);
            Ok(())
        }
    }

    fn ensure_can_manage_consensus_keys(
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if has_permission(
            &state_transaction.world,
            authority,
            "CanManageConsensusKeys",
        ) {
            Ok(())
        } else {
            Err(InstructionExecutionError::InvariantViolation(
                "not permitted: CanManageConsensusKeys".into(),
            ))
        }
    }

    fn prepare_consensus_key_registration(
        id: &ConsensusKeyId,
        mut record: ConsensusKeyRecord,
        block_height: u64,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<ConsensusKeyRecord, Error> {
        if &record.id != id {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "consensus key id mismatch between instruction and record".into(),
                ),
            ));
        }
        if state_transaction.world.consensus_keys.get(id).is_some() {
            return Err(RepetitionError {
                instruction: InstructionType::Register,
                id: IdBox::Permission(Permission::new(
                    "ConsensusKey".into(),
                    iroha_primitives::json::Json::from(id.to_string().as_str()),
                )),
            }
            .into());
        }
        record.status = if record.activation_height > block_height {
            ConsensusKeyStatus::Pending
        } else {
            ConsensusKeyStatus::Active
        };
        Ok(record)
    }

    fn commit_consensus_key_registration(
        state_transaction: &mut StateTransaction<'_, '_>,
        id: &ConsensusKeyId,
        record: ConsensusKeyRecord,
    ) {
        let lifecycle_record = record.clone();
        let log_id = id.clone();
        let log_record = lifecycle_record.clone();
        upsert_consensus_key(&mut state_transaction.world, id, record);
        crate::sumeragi::status::record_consensus_key(lifecycle_record);
        iroha_logger::info!(
            key = %log_id,
            activation = log_record.activation_height,
            expiry = ?log_record.expiry_height,
            "registered consensus key"
        );
    }

    fn load_prev_record_for_rotation(
        new_record: &ConsensusKeyRecord,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(ConsensusKeyId, ConsensusKeyRecord), Error> {
        let Some(prev_id) = new_record.replaces.clone() else {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "rotation must reference the key being replaced".into(),
                ),
            ));
        };
        let Some(prev_record) = state_transaction
            .world
            .consensus_keys
            .get(&prev_id)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "referenced key not found for rotation".into(),
                ),
            ));
        };
        Ok((prev_id, prev_record))
    }

    fn validate_rotation_pair(
        new_record: &ConsensusKeyRecord,
        prev_record: &ConsensusKeyRecord,
    ) -> Result<(), Error> {
        if new_record.id.role != prev_record.id.role {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "replacement consensus key must preserve the role".into(),
                ),
            ));
        }
        if matches!(prev_record.status, ConsensusKeyStatus::Disabled) {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract("cannot rotate from a disabled key".into()),
            ));
        }
        Ok(())
    }

    fn enforce_rotation_overlap(
        prev_record: &mut ConsensusKeyRecord,
        new_record: &ConsensusKeyRecord,
        overlap: u64,
    ) -> Result<(), Error> {
        if let Some(expiry) = prev_record.expiry_height {
            if new_record.activation_height > expiry.saturating_add(overlap) {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "replacement consensus key activation exceeds overlap window".into(),
                    ),
                ));
            }
        } else {
            let retire_at = new_record.activation_height.saturating_add(overlap);
            prev_record.expiry_height = Some(retire_at);
        }
        Ok(())
    }

    fn ensure_rotation_target_unique(
        state_transaction: &StateTransaction<'_, '_>,
        new_id: &ConsensusKeyId,
    ) -> Result<(), Error> {
        if state_transaction.world.consensus_keys.get(new_id).is_some() {
            return Err(RepetitionError {
                instruction: InstructionType::Register,
                id: IdBox::Permission(Permission::new(
                    "ConsensusKey".into(),
                    iroha_primitives::json::Json::from(new_id.to_string().as_str()),
                )),
            }
            .into());
        }
        Ok(())
    }

    fn apply_consensus_key_rotation(
        state_transaction: &mut StateTransaction<'_, '_>,
        new_id: &ConsensusKeyId,
        mut new_record: ConsensusKeyRecord,
        prev_id: ConsensusKeyId,
        mut prev_record: ConsensusKeyRecord,
        block_height: u64,
    ) {
        prev_record.status = ConsensusKeyStatus::Retiring;
        let log_prev = prev_id.clone();
        crate::sumeragi::status::record_consensus_key(prev_record.clone());
        state_transaction
            .world
            .consensus_keys
            .insert(prev_id, prev_record);

        new_record.status = if new_record.activation_height > block_height {
            ConsensusKeyStatus::Pending
        } else {
            ConsensusKeyStatus::Active
        };
        let lifecycle_record = new_record.clone();
        let log_new = new_id.clone();
        let log_record = lifecycle_record.clone();
        upsert_consensus_key(&mut state_transaction.world, new_id, new_record);
        crate::sumeragi::status::record_consensus_key(lifecycle_record);
        iroha_logger::info!(
            replaced = %log_prev,
            key = %log_new,
            activation = log_record.activation_height,
            expiry = ?log_record.expiry_height,
            "rotated consensus key"
        );
    }

    impl Execute for consensus_keys::RotateConsensusKey {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            ensure_can_manage_consensus_keys(authority, state_transaction)?;
            let new_id = self.id().clone();
            let new_record = self.record().clone();
            let params = state_transaction.world.parameters.get();
            let block_height = state_transaction.block_height();
            if new_record.id != new_id {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "consensus key id mismatch between instruction and record".into(),
                    ),
                ));
            }
            let (prev_id, mut prev_record) =
                load_prev_record_for_rotation(&new_record, state_transaction)?;

            validate_consensus_key_record(
                &new_record,
                &params.sumeragi,
                Some(&prev_id),
                block_height,
                false,
            )?;
            validate_rotation_pair(&new_record, &prev_record)?;
            enforce_rotation_overlap(
                &mut prev_record,
                &new_record,
                params.sumeragi.key_overlap_grace_blocks,
            )?;
            ensure_rotation_target_unique(state_transaction, &new_id)?;
            apply_consensus_key_rotation(
                state_transaction,
                &new_id,
                new_record,
                prev_id,
                prev_record,
                block_height,
            );
            Ok(())
        }
    }

    impl Execute for consensus_keys::DisableConsensusKey {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageConsensusKeys",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageConsensusKeys".into(),
                ));
            }

            let id = self.id().clone();
            let Some(mut record) = state_transaction.world.consensus_keys.get(&id).cloned() else {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract("consensus key not found".into()),
                ));
            };

            record.status = ConsensusKeyStatus::Disabled;
            record
                .expiry_height
                .get_or_insert_with(|| state_transaction.block_height());
            let pk = record.public_key.to_string();
            state_transaction
                .world
                .consensus_keys
                .insert(id.clone(), record);
            if let Some(updated) = state_transaction.world.consensus_keys.get(&id).cloned() {
                crate::sumeragi::status::record_consensus_key(updated);
            }
            let mut by_pk = state_transaction
                .world
                .consensus_keys_by_pk
                .get(&pk)
                .cloned()
                .unwrap_or_default();
            if !by_pk.contains(&id) {
                by_pk.push(id.clone());
                state_transaction
                    .world
                    .consensus_keys_by_pk
                    .insert(pk, by_pk);
            }
            iroha_logger::info!(
                key = %id,
                expiry = ?state_transaction
                    .world
                    .consensus_keys
                    .get(&id)
                    .and_then(|rec| rec.expiry_height),
                "disabled consensus key"
            );
            Ok(())
        }
    }

    impl Execute for confidential::PublishPedersenParams {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageConfidentialParams",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageConfidentialParams".into(),
                ));
            }

            let params = self.params().clone();
            let id = params.params_id;
            if state_transaction.world.pedersen_params.get(&id).is_some() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "pedersen parameter set already exists".into(),
                ));
            }
            if matches!(params.status, ConfidentialStatus::Withdrawn) {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "cannot publish pedersen params with Withdrawn status".into(),
                    ),
                ));
            }
            state_transaction.world.pedersen_params.insert(id, params);
            Ok(())
        }
    }

    impl Execute for confidential::SetPedersenParamsLifecycle {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageConfidentialParams",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageConfidentialParams".into(),
                ));
            }

            let id = self.params_id();
            let Some(mut current) = state_transaction.world.pedersen_params.get(id).cloned() else {
                return Err(FindError::Permission(Box::new(Permission::new(
                    "PedersenParamsMissing".into(),
                    iroha_primitives::json::Json::from(format!("{id}").as_str()),
                )))
                .into());
            };
            if matches!(current.status, ConfidentialStatus::Withdrawn) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "cannot update withdrawn pedersen params".into(),
                ));
            }
            current.status = *self.status();
            current.activation_height = *self.activation_height();
            current.withdraw_height = *self.withdraw_height();

            state_transaction.world.pedersen_params.remove(*id);
            state_transaction.world.pedersen_params.insert(*id, current);
            Ok(())
        }
    }

    impl Execute for confidential::PublishPoseidonParams {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageConfidentialParams",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageConfidentialParams".into(),
                ));
            }

            let params = self.params().clone();
            let id = params.params_id;
            if state_transaction.world.poseidon_params.get(&id).is_some() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "poseidon parameter set already exists".into(),
                ));
            }
            if matches!(params.status, ConfidentialStatus::Withdrawn) {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "cannot publish poseidon params with Withdrawn status".into(),
                    ),
                ));
            }
            state_transaction.world.poseidon_params.insert(id, params);
            Ok(())
        }
    }

    impl Execute for confidential::SetPoseidonParamsLifecycle {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageConfidentialParams",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageConfidentialParams".into(),
                ));
            }

            let id = self.params_id();
            let Some(mut current) = state_transaction.world.poseidon_params.get(id).cloned() else {
                return Err(FindError::Permission(Box::new(Permission::new(
                    "PoseidonParamsMissing".into(),
                    iroha_primitives::json::Json::from(format!("{id}").as_str()),
                )))
                .into());
            };
            if matches!(current.status, ConfidentialStatus::Withdrawn) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "cannot update withdrawn poseidon params".into(),
                ));
            }
            current.status = *self.status();
            current.activation_height = *self.activation_height();
            current.withdraw_height = *self.withdraw_height();

            state_transaction.world.poseidon_params.remove(*id);
            state_transaction.world.poseidon_params.insert(*id, current);
            Ok(())
        }
    }

    fn validate_proof_attachment<'a>(
        attachment: &'a iroha_data_model::proof::ProofAttachment,
        proof: &'a iroha_data_model::proof::ProofBox,
        expects_envelope: bool,
        envelope_meta: Option<&'a ZkOpenVerifyEnvelope>,
    ) -> Result<Option<&'a ZkOpenVerifyEnvelope>, Error> {
        if attachment.backend != proof.backend {
            return Err(InstructionExecutionError::InvariantViolation(
                "proof backend mismatch".into(),
            ));
        }
        if expects_envelope && envelope_meta.is_none() {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "halo2 proofs must use OpenVerifyEnvelope payload".into(),
                ),
            ));
        }
        Ok(envelope_meta)
    }

    struct ProofEventArgs<'a> {
        pid: &'a iroha_data_model::proof::ProofId,
        attachment: &'a iroha_data_model::proof::ProofAttachment,
        vk_commitment: Option<[u8; 32]>,
        call_hash: Option<[u8; 32]>,
        prune_outcome: Option<ProofPruneOutcome>,
        cap: usize,
        grace_blocks: u64,
        prune_batch: usize,
        height: u64,
        authority: &'a AccountId,
    }

    fn proof_events_for_result(
        ok: bool,
        args: ProofEventArgs<'_>,
    ) -> Vec<iroha_data_model::events::data::proof::ProofEvent> {
        use iroha_data_model::events::data::proof::{
            ProofEvent, ProofPruneOrigin, ProofPruned, ProofRejected, ProofVerified,
        };

        let mut events = Vec::new();
        if ok {
            events.push(ProofEvent::Verified(ProofVerified {
                id: args.pid.clone(),
                vk_ref: args.attachment.vk_ref.clone(),
                vk_commitment: args.vk_commitment,
                call_hash: args.call_hash,
                envelope_hash: args.attachment.envelope_hash,
            }));
        } else {
            events.push(ProofEvent::Rejected(ProofRejected {
                id: args.pid.clone(),
                vk_ref: args.attachment.vk_ref.clone(),
                vk_commitment: args.vk_commitment,
                call_hash: args.call_hash,
                envelope_hash: args.attachment.envelope_hash,
            }));
        }
        if let Some(outcome) = args.prune_outcome {
            events.push(ProofEvent::Pruned(ProofPruned {
                backend: outcome.backend,
                removed: outcome.removed,
                remaining: outcome.remaining,
                cap: args.cap as u64,
                grace_blocks: args.grace_blocks,
                prune_batch: args.prune_batch as u64,
                pruned_at_height: args.height,
                pruned_by: args.authority.clone(),
                origin: ProofPruneOrigin::Insert,
            }));
        }
        events
    }

    fn encode_and_validate_bridge_proof(
        proof: &iroha_data_model::bridge::BridgeProof,
        zk_cfg: &iroha_config::parameters::actual::Zk,
        current_height: u64,
    ) -> Result<Vec<u8>, Error> {
        if !proof.range.is_valid() {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "bridge proof range must satisfy start_height <= end_height".into(),
                ),
            ));
        }
        if proof.manifest_hash.iter().all(|b| *b == 0) {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "bridge proof manifest hash must not be all zeros".into(),
                ),
            ));
        }

        let range_len = proof.range.len();
        let max_range = zk_cfg.bridge_proof_max_range_len;
        if max_range > 0 && range_len > max_range {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "bridge proof range too large ({range_len} > {max_range})"
                )),
            ));
        }

        let past_window = zk_cfg.bridge_proof_max_past_age_blocks;
        if past_window > 0 {
            let floor = current_height.saturating_sub(past_window);
            if proof.range.end_height < floor {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "bridge proof range ends before the allowed past window (end_height={} < floor={floor})",
                        proof.range.end_height
                    )),
                ));
            }
        }

        let future_window = zk_cfg.bridge_proof_max_future_drift_blocks;
        if future_window > 0 {
            let ceiling = current_height.saturating_add(future_window);
            if proof.range.end_height > ceiling {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "bridge proof range ends after the allowed future window (end_height={} > ceiling={ceiling})",
                        proof.range.end_height
                    )),
                ));
            }
        }

        let encoded = norito::to_bytes(proof).map_err(|err| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                format!("failed to encode bridge proof: {err}"),
            ))
        })?;
        let proof_size = encoded.len();
        let max_bytes = zk_cfg.max_proof_size_bytes as usize;
        if max_bytes > 0 && proof_size > max_bytes {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "bridge proof exceeds max_proof_size_bytes cap ({proof_size} > {max_bytes})"
                )),
            ));
        }

        match &proof.payload {
            iroha_data_model::bridge::BridgeProofPayload::Ics(ics) => {
                validate_bridge_ics_proof(ics, zk_cfg.merkle_depth)?;
            }
            iroha_data_model::bridge::BridgeProofPayload::TransparentZk(tp) => {
                if tp.proof.backend.trim().is_empty() {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "transparent bridge proofs must declare a backend".into(),
                        ),
                    ));
                }
            }
        }

        Ok(encoded)
    }

    struct BridgeProofEventArgs<'a> {
        pid: &'a iroha_data_model::proof::ProofId,
        call_hash: Option<[u8; 32]>,
        envelope_hash: Option<[u8; 32]>,
        prune_outcome: Option<ProofPruneOutcome>,
        cap: usize,
        grace_blocks: u64,
        prune_batch: usize,
        height: u64,
        authority: &'a AccountId,
    }

    fn bridge_proof_events(
        args: BridgeProofEventArgs<'_>,
    ) -> Vec<iroha_data_model::events::data::proof::ProofEvent> {
        use iroha_data_model::events::data::proof::{
            ProofEvent, ProofPruneOrigin, ProofPruned, ProofVerified,
        };

        let mut events = Vec::new();
        events.push(ProofEvent::Verified(ProofVerified {
            id: args.pid.clone(),
            vk_ref: None,
            vk_commitment: None,
            call_hash: args.call_hash,
            envelope_hash: args.envelope_hash,
        }));
        if let Some(outcome) = args.prune_outcome {
            events.push(ProofEvent::Pruned(ProofPruned {
                backend: outcome.backend,
                removed: outcome.removed,
                remaining: outcome.remaining,
                cap: args.cap as u64,
                grace_blocks: args.grace_blocks,
                prune_batch: args.prune_batch as u64,
                pruned_at_height: args.height,
                pruned_by: args.authority.clone(),
                origin: ProofPruneOrigin::Insert,
            }));
        }
        events
    }

    impl Execute for zk::VerifyProof {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let attachment = self.attachment().clone();
            let proof = attachment.proof.clone();
            let envelope_meta = decode_open_verify_envelope(&proof);
            let backend_label = attachment.backend.as_str();
            let expects_envelope = backend_label.starts_with("halo2/");

            let envelope_ref = validate_proof_attachment(
                &attachment,
                &proof,
                expects_envelope,
                envelope_meta.as_ref(),
            )?;

            state_transaction.register_confidential_proof(proof.bytes.len())?;

            let vk_commitment =
                resolve_vk_commitment(&attachment, envelope_ref, state_transaction)?;

            let pid = iroha_data_model::proof::ProofId {
                backend: attachment.backend.clone(),
                proof_hash: crate::zk::hash_proof(&proof),
            };

            ensure_unique_proof(state_transaction, &pid)?;

            let (ok, verify_elapsed) =
                verify_proof_backend(&attachment, &proof, state_transaction)?;
            let status = if ok {
                iroha_data_model::proof::ProofStatus::Verified
            } else {
                iroha_data_model::proof::ProofStatus::Rejected
            };

            #[cfg(feature = "telemetry")]
            {
                let latency_ms = u64::try_from(verify_elapsed.as_millis()).unwrap_or(u64::MAX);
                let proof_bytes = proof.bytes.len();
                state_transaction.telemetry.record_zk_verify(
                    attachment.backend.as_str(),
                    status,
                    proof_bytes,
                    latency_ms,
                );
            }
            #[cfg(not(feature = "telemetry"))]
            let _ = &verify_elapsed;

            index_proof_tags(&mut state_transaction.world, &pid, &proof.bytes);

            let height = state_transaction._curr_block.height.get();
            let call_hash_opt: Option<[u8; 32]> = state_transaction
                .tx_call_hash
                .as_ref()
                .map(|h| <[u8; 32]>::from(*h));
            let record = iroha_data_model::proof::ProofRecord {
                id: pid.clone(),
                vk_ref: attachment.vk_ref.clone(),
                vk_commitment,
                status,
                verified_at_height: Some(height),
                bridge: None,
            };
            state_transaction.world.proofs.insert(pid.clone(), record);

            let cap = state_transaction.zk.proof_history_cap;
            let prune_outcome = enforce_proof_history_cap(
                state_transaction,
                backend_label,
                cap,
                state_transaction.zk.proof_retention_grace_blocks,
                state_transaction.zk.proof_prune_batch,
                height,
            );
            let events = proof_events_for_result(
                ok,
                ProofEventArgs {
                    pid: &pid,
                    attachment: &attachment,
                    vk_commitment,
                    call_hash: call_hash_opt,
                    prune_outcome,
                    cap,
                    grace_blocks: state_transaction.zk.proof_retention_grace_blocks,
                    prune_batch: state_transaction.zk.proof_prune_batch,
                    height,
                    authority,
                },
            );
            state_transaction.world.emit_events(events);
            Ok(())
        }
    }

    impl Execute for zk::PruneProofs {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            use std::collections::BTreeSet;

            use iroha_data_model::events::data::proof::{
                ProofEvent, ProofPruneOrigin, ProofPruned,
            };

            let cap = state_transaction.zk.proof_history_cap;
            let grace = state_transaction.zk.proof_retention_grace_blocks;
            let prune_batch = state_transaction.zk.proof_prune_batch;
            let height = state_transaction._curr_block.height.get();

            let mut backends: BTreeSet<String> = BTreeSet::new();
            if let Some(spec) = &self.backend {
                backends.insert(spec.clone());
            } else {
                for (id, _) in state_transaction.world.proofs.iter() {
                    backends.insert(id.backend.clone());
                }
            }

            let mut events = Vec::new();
            for backend in backends {
                if let Some(outcome) = enforce_proof_history_cap(
                    state_transaction,
                    &backend,
                    cap,
                    grace,
                    prune_batch,
                    height,
                ) {
                    events.push(ProofEvent::Pruned(ProofPruned {
                        backend: outcome.backend,
                        removed: outcome.removed,
                        remaining: outcome.remaining,
                        cap: cap as u64,
                        grace_blocks: grace,
                        prune_batch: prune_batch as u64,
                        pruned_at_height: height,
                        pruned_by: authority.clone(),
                        origin: ProofPruneOrigin::Manual,
                    }));
                }
            }

            if !events.is_empty() {
                state_transaction.world.emit_events(events);
            }
            Ok(())
        }
    }

    impl Execute for bridge::SubmitBridgeProof {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let current_height = state_transaction._curr_block.height.get();
            let encoded = encode_and_validate_bridge_proof(
                &self.proof,
                &state_transaction.zk,
                current_height,
            )?;
            let proof_size = encoded.len();
            let backend_label = self.proof.backend_label();
            let commitment = hash_bridge_proof(&backend_label, &encoded);

            if let Some(conflict) =
                find_overlapping_bridge_range(state_transaction, &backend_label, &self.proof.range)
            {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "bridge proof range overlaps existing proof {conflict}"
                    )),
                ));
            }

            let pid = iroha_data_model::proof::ProofId {
                backend: backend_label.clone(),
                proof_hash: commitment,
            };
            ensure_unique_proof(state_transaction, &pid)?;

            let height = current_height;
            let call_hash_opt: Option<[u8; 32]> = state_transaction
                .tx_call_hash
                .as_ref()
                .map(|h| <[u8; 32]>::from(*h));
            let envelope_hash: Option<[u8; 32]> =
                Some(<[u8; 32]>::from(iroha_crypto::Hash::new(&encoded)));

            let record = iroha_data_model::proof::ProofRecord {
                id: pid.clone(),
                vk_ref: None,
                vk_commitment: None,
                status: iroha_data_model::proof::ProofStatus::Verified,
                verified_at_height: Some(height),
                bridge: Some(iroha_data_model::bridge::BridgeProofRecord {
                    proof: self.proof,
                    commitment,
                    size_bytes: u32::try_from(proof_size).unwrap_or(u32::MAX),
                }),
            };
            state_transaction.world.proofs.insert(pid.clone(), record);

            let cap = state_transaction.zk.proof_history_cap;
            let prune_outcome = enforce_bridge_history_cap(
                state_transaction,
                &backend_label,
                cap,
                state_transaction.zk.proof_retention_grace_blocks,
                state_transaction.zk.proof_prune_batch,
                height,
            );

            let events = bridge_proof_events(BridgeProofEventArgs {
                pid: &pid,
                call_hash: call_hash_opt,
                envelope_hash,
                prune_outcome,
                cap,
                grace_blocks: state_transaction.zk.proof_retention_grace_blocks,
                prune_batch: state_transaction.zk.proof_prune_batch,
                height,
                authority,
            });
            state_transaction.world.emit_events(events);
            Ok(())
        }
    }

    impl Execute for bridge::RecordBridgeReceipt {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            use iroha_data_model::events::data::prelude::BridgeEvent;

            state_transaction
                .world
                .emit_events(Some(BridgeEvent::Emitted(self.receipt)));
            Ok(())
        }
    }

    fn resolve_vk_commitment(
        attachment: &iroha_data_model::proof::ProofAttachment,
        envelope: Option<&ZkOpenVerifyEnvelope>,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<Option<[u8; 32]>, Error> {
        match (&attachment.vk_ref, &attachment.vk_inline) {
            (None, None) => {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "proof attachments must include exactly one verifying key (inline or reference)"
                            .into(),
                    ),
                ));
            }
            (Some(_), Some(_)) => {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "proof attachments must not mix inline and referenced verifying keys"
                            .into(),
                    ),
                ));
            }
            _ => {}
        }
        let inline_commitment = if let Some(vk) = &attachment.vk_inline {
            if vk.backend != attachment.backend {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key backend mismatch".into(),
                ));
            }
            let inline_commit = hash_vk(vk);
            if let Some(env) = envelope {
                if env.vk_hash != [0u8; 32] && env.vk_hash != inline_commit {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "inline verifying key commitment mismatch".into(),
                    ));
                }
            }
            Some(inline_commit)
        } else {
            None
        };
        let vk_commitment = if let Some(id @ VerifyingKeyId { .. }) = &attachment.vk_ref {
            let Some(rec) = state_transaction.world.verifying_keys.get(id) else {
                return Err(FindError::Permission(Box::new(Permission::new(
                    "VerifyingKeyMissing".into(),
                    iroha_primitives::json::Json::from(
                        format!("{}::{}", id.backend, id.name).as_str(),
                    ),
                )))
                .into());
            };
            if rec.status != ConfidentialStatus::Active {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key is not active".into(),
                ));
            }
            if rec.gas_schedule_id.is_none() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key missing gas_schedule_id".into(),
                ));
            }
            let circuit_key = (rec.circuit_id.clone(), rec.version);
            match state_transaction
                .world
                .verifying_keys_by_circuit
                .get(&circuit_key)
            {
                Some(mapped) if mapped == id => {}
                _ => {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "verifying key circuit/version not active".into(),
                    ));
                }
            }
            if let Some(env) = envelope {
                if rec.circuit_id != env.circuit_id {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "verifying key circuit mismatch".into(),
                    ));
                }
                if rec.public_inputs_schema_hash != [0u8; 32] {
                    let observed_hash: [u8; 32] = CryptoHash::new(&env.public_inputs).into();
                    if rec.public_inputs_schema_hash != observed_hash {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "public inputs schema hash mismatch".into(),
                        ));
                    }
                }
                if env.vk_hash != [0u8; 32] && env.vk_hash != rec.commitment {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "verifying key commitment mismatch".into(),
                    ));
                }
            }
            Some(rec.commitment)
        } else {
            inline_commitment
        };

        Ok(vk_commitment)
    }

    fn ensure_unique_proof(
        state_transaction: &StateTransaction<'_, '_>,
        pid: &ProofId,
    ) -> Result<(), Error> {
        if state_transaction.world.proofs.get(pid).is_some() {
            return Err(RepetitionError {
                instruction: InstructionType::Register,
                id: IdBox::Permission(Permission::new(
                    "Proof".into(),
                    iroha_primitives::json::Json::from(
                        format!("{}:{:x?}", &pid.backend, &pid.proof_hash[..4]).as_str(),
                    ),
                )),
            }
            .into());
        }
        Ok(())
    }

    fn verify_proof_backend(
        attachment: &iroha_data_model::proof::ProofAttachment,
        proof: &iroha_data_model::proof::ProofBox,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(bool, Duration), Error> {
        let timeout_budget = state_transaction.zk.verify_timeout;
        if let Some(preverified) = state_transaction.lookup_preverified_proof(proof) {
            return Ok((preverified, Duration::ZERO));
        }

        let vk_box = match (&attachment.vk_inline, &attachment.vk_ref) {
            (Some(vk_inline), None) => Some(vk_inline.clone()),
            (None, Some(id)) => {
                let record = state_transaction
                    .world
                    .verifying_keys
                    .get(id)
                    .cloned()
                    .ok_or_else(|| {
                        InstructionExecutionError::InvariantViolation(
                            "verifying key not found".into(),
                        )
                    })?;
                let vk_box = record.key.clone().ok_or_else(|| {
                    InstructionExecutionError::InvariantViolation(
                        "verifying key bytes missing".into(),
                    )
                })?;
                let commitment = hash_vk(&vk_box);
                if record.commitment != commitment {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "verifying key commitment mismatch".into(),
                    ));
                }
                if vk_box.backend != attachment.backend {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "verifying key backend mismatch".into(),
                    ));
                }
                Some(vk_box)
            }
            _ => None,
        };

        let report = crate::zk::verify_backend_with_timing(
            attachment.backend.as_str(),
            proof,
            vk_box.as_ref(),
        );
        if timeout_budget > Duration::ZERO && report.elapsed > timeout_budget {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract("proof verification exceeded timeout".into()),
            ));
        }
        Ok((report.ok, report.elapsed))
    }

    fn index_proof_tags(world: &mut WorldTransaction<'_, '_>, pid: &ProofId, proof_bytes: &[u8]) {
        let mut tags = zk1_list_tags(proof_bytes);
        if tags.is_empty() {
            return;
        }
        tags.sort_unstable();
        tags.dedup();
        world.proof_tags.insert(pid.clone(), tags.clone());
        for tag in tags {
            let tag_slice: &[u8] = &tag;
            let mut ids = world
                .proofs_by_tag
                .get(tag_slice)
                .cloned()
                .unwrap_or_default();
            if !ids.iter().any(|existing| existing == pid) {
                ids.push(pid.clone());
                ids.sort();
            }
            world.proofs_by_tag.insert(tag, ids);
        }
    }

    fn proof_retention_removals(
        mut items: Vec<(ProofId, u64)>,
        cap: usize,
        grace_blocks: u64,
        prune_batch: usize,
        current_height: u64,
    ) -> Vec<ProofId> {
        if items.is_empty() {
            return Vec::new();
        }
        items.sort_by_key(|(_, height)| *height);
        let retention_floor =
            (grace_blocks > 0).then(|| current_height.saturating_sub(grace_blocks));
        // Always keep at least this many newest entries even if they fall outside the grace window
        // so we do not prune the entire registry when all proofs are old.
        let min_keep = if cap == 0 {
            usize::try_from(grace_blocks).unwrap_or(usize::MAX)
        } else {
            let cap_u64 = u64::try_from(cap).unwrap_or(u64::MAX);
            let bounded = grace_blocks.min(cap_u64);
            usize::try_from(bounded).unwrap_or(usize::MAX)
        };

        let mut retained: Vec<(ProofId, u64)> = Vec::new();
        let mut stale: Vec<(ProofId, u64)> = Vec::new();

        for (id, height) in items {
            if let Some(floor) = retention_floor {
                if height < floor {
                    stale.push((id, height));
                } else {
                    retained.push((id, height));
                }
            } else {
                retained.push((id, height));
            }
        }

        if retained.len() < min_keep {
            let deficit = min_keep - retained.len();
            let keep_from_stale = stale.split_off(stale.len().saturating_sub(deficit));
            retained.extend(keep_from_stale);
        }
        retained.sort_by_key(|(_, height)| *height);

        let mut removals: Vec<ProofId> = stale.into_iter().map(|(id, _)| id).collect();

        if cap > 0 && retained.len() > cap {
            let overflow = retained.len() - cap;
            removals.extend(retained.into_iter().take(overflow).map(|(id, _)| id));
        }

        if prune_batch > 0 && removals.len() > prune_batch {
            removals.truncate(prune_batch);
        }

        removals
    }

    #[cfg(test)]
    mod retention_tests {
        use super::*;

        #[test]
        fn retention_prunes_entries_older_than_grace_window() {
            let backend = "halo2/ipa";
            let items: Vec<_> = (1u64..=4)
                .map(|height| {
                    let proof_height = u8::try_from(height).unwrap_or(u8::MAX);
                    (
                        ProofId {
                            backend: backend.into(),
                            proof_hash: [proof_height; 32],
                        },
                        height,
                    )
                })
                .collect();

            let removals = proof_retention_removals(items, 10, 2, 10, 9);
            assert_eq!(removals.len(), 2);
            let hashes: Vec<u8> = removals.iter().map(|id| id.proof_hash[0]).collect();
            assert!(hashes.contains(&1));
            assert!(hashes.contains(&2));
        }

        #[test]
        fn retention_respects_prune_batch_limit() {
            let backend = "halo2/ipa";
            let items: Vec<_> = (0u64..5)
                .map(|height| {
                    let proof_height = u8::try_from(height).unwrap_or(u8::MAX);
                    (
                        ProofId {
                            backend: backend.into(),
                            proof_hash: [proof_height; 32],
                        },
                        height,
                    )
                })
                .collect();

            let removals = proof_retention_removals(items, 1, 0, 2, 5);
            assert_eq!(removals.len(), 2);
            let hashes: Vec<u8> = removals.iter().map(|id| id.proof_hash[0]).collect();
            assert!(hashes.contains(&0));
            assert!(hashes.contains(&1));
        }

        #[test]
        fn retention_keeps_minimum_entries_when_all_are_stale() {
            let backend = "halo2/ipa";
            let items: Vec<_> = (1u64..=6)
                .map(|height| {
                    let proof_height = u8::try_from(height).unwrap_or(u8::MAX);
                    (
                        ProofId {
                            backend: backend.into(),
                            proof_hash: [proof_height; 32],
                        },
                        height,
                    )
                })
                .collect();

            // Everything is older than the grace window; we still keep the newest three
            // entries because grace_blocks acts as a floor when cap=0.
            let removals = proof_retention_removals(items, 0, 3, 10, 20);
            let hashes: Vec<u8> = removals.iter().map(|id| id.proof_hash[0]).collect();
            assert_eq!(hashes.len(), 3);
            assert!(hashes.contains(&1));
            assert!(hashes.contains(&2));
            assert!(hashes.contains(&3));
            assert!(!hashes.contains(&4));
            assert!(!hashes.contains(&5));
            assert!(!hashes.contains(&6));
        }
    }

    struct ProofPruneOutcome {
        backend: String,
        removed: Vec<ProofId>,
        remaining: u64,
    }

    fn prune_proof_entries(state_transaction: &mut StateTransaction<'_, '_>, removals: &[ProofId]) {
        for old_id in removals {
            if let Some(existing_tags) = state_transaction.world.proof_tags.get(old_id) {
                for tag in existing_tags {
                    let tag_slice: &[u8] = &tag[..];
                    let mut ids = state_transaction
                        .world
                        .proofs_by_tag
                        .get(tag_slice)
                        .cloned()
                        .unwrap_or_default();
                    ids.retain(|entry| entry != old_id);
                    if ids.is_empty() {
                        state_transaction.world.proofs_by_tag.remove(*tag);
                    } else {
                        state_transaction.world.proofs_by_tag.insert(*tag, ids);
                    }
                }
            }
            state_transaction.world.proof_tags.remove(old_id.clone());
            state_transaction.world.proofs.remove(old_id.clone());
        }
    }

    fn enforce_proof_history_cap(
        state_transaction: &mut StateTransaction<'_, '_>,
        backend: &str,
        cap: usize,
        grace_blocks: u64,
        prune_batch: usize,
        current_height: u64,
    ) -> Option<ProofPruneOutcome> {
        let items: Vec<(ProofId, u64)> = state_transaction
            .world
            .proofs
            .iter()
            .filter(|(id, _)| id.backend == backend)
            .map(|(id, rec)| {
                let height = rec.verified_at_height.unwrap_or(0);
                (id.clone(), height)
            })
            .collect();
        let removals =
            proof_retention_removals(items, cap, grace_blocks, prune_batch, current_height);

        if removals.is_empty() {
            return None;
        }

        iroha_logger::info!(
            backend,
            removed = removals.len(),
            cap,
            grace_blocks,
            prune_batch,
            "pruned proof registry entries for retention"
        );

        let removed = removals;
        prune_proof_entries(state_transaction, &removed);

        let remaining = state_transaction
            .world
            .proofs
            .iter()
            .filter(|(id, _)| id.backend == backend)
            .count() as u64;
        Some(ProofPruneOutcome {
            backend: backend.to_owned(),
            removed,
            remaining,
        })
    }

    fn enforce_bridge_history_cap(
        state_transaction: &mut StateTransaction<'_, '_>,
        backend: &str,
        cap: usize,
        grace_blocks: u64,
        prune_batch: usize,
        current_height: u64,
    ) -> Option<ProofPruneOutcome> {
        let items: Vec<(ProofId, u64)> = state_transaction
            .world
            .proofs
            .iter()
            .filter(|(id, rec)| id.backend == backend && rec.bridge.is_some())
            .filter_map(|(id, rec)| {
                let bridge = rec.bridge.as_ref()?;
                if bridge.proof.pinned {
                    return None;
                }
                Some((id.clone(), rec.verified_at_height.unwrap_or(0)))
            })
            .collect();
        let removals =
            proof_retention_removals(items, cap, grace_blocks, prune_batch, current_height);
        if removals.is_empty() {
            return None;
        }

        iroha_logger::info!(
            backend,
            removed = removals.len(),
            cap,
            grace_blocks,
            prune_batch,
            "pruned bridge proof registry entries for retention"
        );

        let removed = removals;
        prune_proof_entries(state_transaction, &removed);

        let remaining = state_transaction
            .world
            .proofs
            .iter()
            .filter(|(id, rec)| id.backend == backend && rec.bridge.is_some())
            .count() as u64;
        Some(ProofPruneOutcome {
            backend: backend.to_owned(),
            removed,
            remaining,
        })
    }

    fn hash_bridge_proof(backend: &str, payload: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(backend.as_bytes());
        h.update(payload);
        h.finalize().into()
    }

    fn find_overlapping_bridge_range(
        state_transaction: &StateTransaction<'_, '_>,
        backend: &str,
        new_range: &iroha_data_model::bridge::BridgeProofRange,
    ) -> Option<ProofId> {
        state_transaction.world.proofs.iter().find_map(|(id, rec)| {
            let bridge = rec.bridge.as_ref()?;
            if id.backend != backend {
                return None;
            }
            if ranges_overlap(&bridge.proof.range, new_range) {
                Some(id.clone())
            } else {
                None
            }
        })
    }

    fn ranges_overlap(
        a: &iroha_data_model::bridge::BridgeProofRange,
        b: &iroha_data_model::bridge::BridgeProofRange,
    ) -> bool {
        a.start_height <= b.end_height && b.start_height <= a.end_height
    }

    fn validate_bridge_ics_proof(
        proof: &iroha_data_model::bridge::BridgeIcsProof,
        max_depth: u8,
    ) -> Result<(), Error> {
        let depth = u8::try_from(proof.proof.audit_path().len()).unwrap_or(u8::MAX);
        if max_depth > 0 && depth > max_depth {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "bridge proof path depth {depth} exceeds cap {max_depth}"
                )),
            ));
        }
        if proof.state_root.iter().all(|b| *b == 0) {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "bridge proof missing state root commitment".into(),
                ),
            ));
        }

        let leaf = iroha_crypto::HashOf::<[u8; 32]>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed(proof.leaf_hash),
        );
        let root =
            iroha_crypto::HashOf::<iroha_crypto::MerkleTree<[u8; 32]>>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed(proof.state_root),
            );
        let ok =
            match proof.hash_function {
                iroha_data_model::bridge::BridgeHashFunction::Sha256 => proof
                    .proof
                    .clone()
                    .verify_sha256(&leaf, &root, usize::from(max_depth.max(depth))),
                iroha_data_model::bridge::BridgeHashFunction::Blake2b => proof
                    .proof
                    .clone()
                    .verify(&leaf, &root, usize::from(max_depth.max(depth))),
            };
        if !ok {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "bridge proof Merkle verification failed".into(),
                ),
            ));
        }
        Ok(())
    }

    fn zk1_list_tags(bytes: &[u8]) -> Vec<[u8; 4]> {
        if bytes.len() < 8 {
            return Vec::new();
        }
        if &bytes[..4] != b"ZK1\0" {
            return Vec::new();
        }
        let mut pos = 4usize;
        let mut out = Vec::new();
        while pos + 8 <= bytes.len() {
            let mut tag = [0u8; 4];
            tag.copy_from_slice(&bytes[pos..pos + 4]);
            let len = u32::from_le_bytes([
                bytes[pos + 4],
                bytes[pos + 5],
                bytes[pos + 6],
                bytes[pos + 7],
            ]) as usize;
            pos += 8;
            if pos + len > bytes.len() {
                break;
            }
            out.push(tag);
            pos += len;
        }
        out
    }

    // --- ZK Asset policy & ledger ---

    fn enforce_vk_binding(
        binding: &crate::state::ZkAssetVerifierBinding,
        attachment: &iroha_data_model::proof::ProofAttachment,
    ) -> Result<(), Error> {
        if attachment.backend.as_str() != binding.id.backend {
            return Err(InstructionExecutionError::InvariantViolation(
                "proof backend does not match asset verifying key".into(),
            ));
        }
        let saw_binding_ref = if let Some(vk_ref) = &attachment.vk_ref {
            if vk_ref != &binding.id {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key reference mismatch".into(),
                ));
            }
            true
        } else {
            false
        };
        let saw_binding = if let Some(vk_inline) = &attachment.vk_inline {
            if vk_inline.backend != binding.id.backend {
                return Err(InstructionExecutionError::InvariantViolation(
                    "inline verifying key backend mismatch".into(),
                ));
            }
            let digest = crate::zk::hash_vk(vk_inline);
            if digest != binding.commitment {
                return Err(InstructionExecutionError::InvariantViolation(
                    "inline verifying key commitment mismatch".into(),
                ));
            }
            true
        } else {
            saw_binding_ref
        };
        if let Some(commitment) = attachment.vk_commitment {
            if commitment != binding.commitment {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key commitment mismatch".into(),
                ));
            }
        }
        if !saw_binding {
            return Err(InstructionExecutionError::InvariantViolation(
                "proof missing verifying key reference or inline key".into(),
            ));
        }
        Ok(())
    }

    #[derive(Clone)]
    struct PolicyMetadataContext {
        allow_shield: bool,
        allow_unshield: bool,
        vk_transfer: Option<iroha_data_model::proof::VerifyingKeyId>,
        vk_unshield: Option<iroha_data_model::proof::VerifyingKeyId>,
        vk_shield: Option<iroha_data_model::proof::VerifyingKeyId>,
    }

    fn metadata_context_from_state(
        state: Option<&crate::state::ZkAssetState>,
    ) -> PolicyMetadataContext {
        state.map_or(
            PolicyMetadataContext {
                allow_shield: false,
                allow_unshield: false,
                vk_transfer: None,
                vk_unshield: None,
                vk_shield: None,
            },
            |st| PolicyMetadataContext {
                allow_shield: st.allow_shield,
                allow_unshield: st.allow_unshield,
                vk_transfer: st.vk_transfer.as_ref().map(|binding| binding.id.clone()),
                vk_unshield: st.vk_unshield.as_ref().map(|binding| binding.id.clone()),
                vk_shield: st.vk_shield.as_ref().map(|binding| binding.id.clone()),
            },
        )
    }

    fn persist_policy_metadata(
        state_transaction: &mut StateTransaction<'_, '_>,
        asset_def_id: &AssetDefinitionId,
        policy: &AssetConfidentialPolicy,
        context: &PolicyMetadataContext,
    ) -> Result<(), Error> {
        let mut policy_map = BTreeMap::new();
        policy_map.insert(
            "mode".into(),
            norito::json::native::Value::from(format!("{:?}", policy.mode())),
        );
        policy_map.insert(
            "allow_shield".into(),
            norito::json::native::Value::from(context.allow_shield),
        );
        policy_map.insert(
            "allow_unshield".into(),
            norito::json::native::Value::from(context.allow_unshield),
        );
        let vk_to_value =
            |vk: Option<iroha_data_model::proof::VerifyingKeyId>| -> norito::json::native::Value {
                vk.map_or(norito::json::native::Value::Null, |id| {
                    norito::json::native::Value::from(format!("{}::{}", id.backend, id.name))
                })
            };
        policy_map.insert(
            "vk_transfer".into(),
            vk_to_value(context.vk_transfer.clone()),
        );
        policy_map.insert(
            "vk_unshield".into(),
            vk_to_value(context.vk_unshield.clone()),
        );
        policy_map.insert("vk_shield".into(), vk_to_value(context.vk_shield.clone()));

        let pending_value =
            policy
                .pending_transition()
                .map_or(norito::json::native::Value::Null, |pending| {
                    let mut pending_map = BTreeMap::new();
                    pending_map.insert(
                        "new_mode".into(),
                        norito::json::native::Value::from(format!("{:?}", pending.new_mode())),
                    );
                    pending_map.insert(
                        "effective_height".into(),
                        norito::json::native::Value::from(pending.effective_height()),
                    );
                    pending_map.insert(
                        "previous_mode".into(),
                        norito::json::native::Value::from(format!("{:?}", pending.previous_mode())),
                    );
                    pending_map.insert(
                        "transition_id".into(),
                        norito::json::native::Value::from(hex::encode(
                            pending.transition_id().as_ref(),
                        )),
                    );
                    if let Some(window) = pending.conversion_window() {
                        pending_map.insert(
                            "conversion_window".into(),
                            norito::json::native::Value::from(*window),
                        );
                    }
                    norito::json::native::Value::Object(pending_map)
                });
        policy_map.insert("pending_transition".into(), pending_value);

        let features_digest_hex = hex::encode(policy.features_digest().as_ref());
        policy_map.insert(
            "features_digest".into(),
            norito::json::native::Value::from(features_digest_hex),
        );

        let policy_json =
            iroha_primitives::json::Json::from(norito::json::native::Value::Object(policy_map));
        let key: Name = "zk.policy".parse().unwrap();
        {
            let asset_definition = state_transaction
                .world
                .asset_definition_mut(asset_def_id)
                .map_err(Error::from)?;
            asset_definition
                .metadata_mut()
                .insert(key.clone(), policy_json.clone());
        }
        state_transaction.world.emit_events(Some(
            iroha_data_model::prelude::AssetDefinitionEvent::MetadataInserted(
                iroha_data_model::prelude::MetadataChanged {
                    target: asset_def_id.clone(),
                    key,
                    value: policy_json,
                },
            ),
        ));
        Ok(())
    }

    pub(crate) fn apply_policy_if_due(
        state_transaction: &mut StateTransaction<'_, '_>,
        asset_def_id: &AssetDefinitionId,
    ) -> Result<AssetConfidentialPolicy, Error> {
        let block_height = state_transaction.block_height();
        let mut changed = false;
        let current_policy = {
            let asset_definition = state_transaction
                .world
                .asset_definition(asset_def_id)
                .map_err(Error::from)?;
            *asset_definition.confidential_policy()
        };
        let mut working_policy = current_policy;
        let mut aborted = false;
        let pending_transition = working_policy.pending_transition;
        if let Some(transition) = pending_transition {
            if block_height >= transition.effective_height()
                && transition.new_mode() == ConfidentialPolicyMode::ShieldedOnly
            {
                let transparent_total = state_transaction
                    .world
                    .asset_total_amount(asset_def_id)
                    .map_err(Error::from)?;
                if transparent_total > iroha_primitives::numeric::Numeric::zero() {
                    aborted = true;
                    working_policy.pending_transition = None;
                    working_policy.mode = transition.previous_mode();
                    iroha_logger::warn!(
                        asset = %asset_def_id,
                        total = %transparent_total,
                        "aborting confidential policy transition: non-zero transparent supply"
                    );
                }
            }
        }
        let (candidate_policy, did_change) = if aborted {
            let changed_flag = working_policy != current_policy;
            (working_policy, changed_flag)
        } else {
            working_policy.apply_if_due(block_height)
        };

        let applied_policy = if did_change {
            {
                let asset_definition = state_transaction
                    .world
                    .asset_definition_mut(asset_def_id)
                    .map_err(Error::from)?;
                asset_definition.set_confidential_policy(candidate_policy);
            }
            changed = true;
            candidate_policy
        } else {
            current_policy
        };

        if changed {
            let context =
                metadata_context_from_state(state_transaction.world.zk_assets.get(asset_def_id));
            persist_policy_metadata(state_transaction, asset_def_id, &applied_policy, &context)?;
        }
        Ok(applied_policy)
    }

    struct PolicyTransitionValidation {
        current_mode: ConfidentialPolicyMode,
        new_mode: ConfidentialPolicyMode,
        commitments_empty: bool,
        block_height: u64,
        effective_height: u64,
        delay_blocks: u64,
        window_blocks: u64,
        requested_window: Option<u64>,
    }

    fn validate_policy_transition(args: &PolicyTransitionValidation) -> Result<(), Error> {
        ensure_mode_change(args)?;
        let lead_time = ensure_effective_height(args)?;
        let window_for_shield = ensure_notice_window(args, lead_time)?;
        ensure_transition_allowed(args, window_for_shield)
    }

    fn ensure_mode_change(args: &PolicyTransitionValidation) -> Result<(), Error> {
        if args.new_mode == args.current_mode {
            Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "confidential policy transition keeps the same mode".into(),
                ),
            ))
        } else {
            Ok(())
        }
    }

    fn ensure_effective_height(args: &PolicyTransitionValidation) -> Result<u64, Error> {
        if args.effective_height <= args.block_height {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "effective height must be in the future".into(),
                ),
            ));
        }
        let lead_time = args.effective_height.saturating_sub(args.block_height);
        if lead_time < args.delay_blocks {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "effective height must be at least {delay_blocks} blocks ahead",
                    delay_blocks = args.delay_blocks,
                )),
            ));
        }
        Ok(lead_time)
    }

    fn ensure_notice_window(
        args: &PolicyTransitionValidation,
        lead_time: u64,
    ) -> Result<Option<u64>, Error> {
        if matches!(
            args.new_mode,
            ConfidentialPolicyMode::ShieldedOnly | ConfidentialPolicyMode::TransparentOnly
        ) && args.window_blocks > 0
            && lead_time < args.window_blocks
        {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "confidential policy transition must provide at least {window_blocks} blocks of notice",
                    window_blocks = args.window_blocks,
                )),
            ));
        }

        if args.new_mode == ConfidentialPolicyMode::ShieldedOnly {
            let window = args.requested_window.ok_or_else(|| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    "conversion window is required when transitioning to ShieldedOnly".into(),
                ))
            })?;
            if window == 0 {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "conversion window must be greater than zero".into(),
                    ),
                ));
            }
            if window < args.window_blocks {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "conversion window must be at least {window_blocks} blocks",
                        window_blocks = args.window_blocks,
                    )),
                ));
            }
            if window > lead_time {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "conversion window cannot exceed lead time before effective height".into(),
                    ),
                ));
            }
            Ok(Some(window))
        } else {
            if args.requested_window.is_some() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "conversion window is only supported for ShieldedOnly transitions".into(),
                    ),
                ));
            }
            Ok(None)
        }
    }

    fn ensure_transition_allowed(
        args: &PolicyTransitionValidation,
        window_for_shield: Option<u64>,
    ) -> Result<(), Error> {
        match (args.current_mode, args.new_mode) {
            (ConfidentialPolicyMode::TransparentOnly, ConfidentialPolicyMode::Convertible) => {
                Ok(())
            }
            (ConfidentialPolicyMode::TransparentOnly, ConfidentialPolicyMode::ShieldedOnly) => {
                let _ = window_for_shield;
                Ok(())
            }
            (ConfidentialPolicyMode::Convertible, ConfidentialPolicyMode::ShieldedOnly)
            | (ConfidentialPolicyMode::ShieldedOnly, ConfidentialPolicyMode::Convertible) => Ok(()),
            (
                ConfidentialPolicyMode::Convertible | ConfidentialPolicyMode::ShieldedOnly,
                ConfidentialPolicyMode::TransparentOnly,
            ) => {
                if args.commitments_empty {
                    Ok(())
                } else {
                    Err(InstructionExecutionError::InvariantViolation(
                        "cannot transition to TransparentOnly while shielded commitments exist"
                            .into(),
                    ))
                }
            }
            _ => Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "unsupported confidential policy transition".into(),
                ),
            )),
        }
    }

    impl Execute for zk::RegisterZkAsset {
        #[allow(clippy::too_many_lines)]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Ensure the asset definition exists
            let asset_def_id = self.asset().clone();
            // Derive canonical confidential policy for asset definition.
            let policy_mode = match self.mode() {
                iroha_data_model::isi::zk::ZkAssetMode::ZkNative => {
                    ConfidentialPolicyMode::ShieldedOnly
                }
                iroha_data_model::isi::zk::ZkAssetMode::Hybrid => {
                    if !*self.allow_shield() && !*self.allow_unshield() {
                        ConfidentialPolicyMode::TransparentOnly
                    } else {
                        ConfidentialPolicyMode::Convertible
                    }
                }
            };
            let resolve_binding = |vk: &Option<iroha_data_model::proof::VerifyingKeyId>| -> Result<
                Option<crate::state::ZkAssetVerifierBinding>,
                Error,
            > {
                let Some(id) = vk.clone() else {
                    return Ok(None);
                };
                let Some(rec) = state_transaction.world.verifying_keys.get(&id) else {
                    return Err(FindError::Permission(Box::new(Permission::new(
                        "VerifyingKeyMissing".into(),
                        iroha_primitives::json::Json::from(
                            format!("{}::{}", id.backend, id.name).as_str(),
                        ),
                    )))
                    .into());
                };
                if rec.status != iroha_data_model::confidential::ConfidentialStatus::Active {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "verifying key is not active".into(),
                    ));
                }
                Ok(Some(crate::state::ZkAssetVerifierBinding {
                    id,
                    commitment: rec.commitment,
                }))
            };

            let vk_transfer_binding = resolve_binding(self.vk_transfer())?;
            let vk_unshield_binding = resolve_binding(self.vk_unshield())?;
            let vk_shield_binding = resolve_binding(self.vk_shield())?;

            let mut vk_fingerprints: Vec<String> = [
                self.vk_transfer().clone(),
                self.vk_unshield().clone(),
                self.vk_shield().clone(),
            ]
            .into_iter()
            .flatten()
            .map(|vk| format!("{}::{}", vk.backend, vk.name))
            .collect();
            vk_fingerprints.sort();
            let vk_set_hash = if vk_fingerprints.is_empty() {
                None
            } else {
                let joined = vk_fingerprints.join("|");
                Some(iroha_crypto::Hash::new(joined.as_bytes()))
            };
            let policy_struct = AssetConfidentialPolicy {
                mode: policy_mode,
                vk_set_hash,
                poseidon_params_id: state_transaction.zk.poseidon_params_id,
                pedersen_params_id: state_transaction.zk.pedersen_params_id,
                pending_transition: None,
            };
            {
                let asset_definition = state_transaction
                    .world
                    .asset_definition_mut(&asset_def_id)
                    .map_err(Error::from)?;
                asset_definition.set_confidential_policy(policy_struct);
            }
            let metadata_context = PolicyMetadataContext {
                allow_shield: *self.allow_shield(),
                allow_unshield: *self.allow_unshield(),
                vk_transfer: self.vk_transfer().clone(),
                vk_unshield: self.vk_unshield().clone(),
                vk_shield: self.vk_shield().clone(),
            };
            persist_policy_metadata(
                state_transaction,
                &asset_def_id,
                &policy_struct,
                &metadata_context,
            )?;
            // Persist/Update internal ZK policy state
            let mut st = state_transaction
                .world
                .zk_assets
                .get(&asset_def_id)
                .cloned()
                .unwrap_or_default();
            st.mode = *self.mode();
            st.allow_shield = *self.allow_shield();
            st.allow_unshield = *self.allow_unshield();
            st.vk_transfer = vk_transfer_binding;
            st.vk_unshield = vk_unshield_binding;
            st.vk_shield = vk_shield_binding;
            state_transaction
                .world
                .zk_assets
                .remove(asset_def_id.clone());
            state_transaction
                .world
                .zk_assets
                .insert(asset_def_id.clone(), st);
            Ok(())
        }
    }

    impl Execute for zk::ScheduleConfidentialPolicyTransition {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_def_id = self.asset().clone();
            let mut policy = apply_policy_if_due(state_transaction, &asset_def_id)?;
            if policy.pending_transition().is_some() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "confidential policy transition already scheduled".into(),
                ));
            }
            let st = state_transaction
                .world
                .zk_assets
                .get(&asset_def_id)
                .ok_or_else(|| {
                    InstructionExecutionError::InvariantViolation(
                        "asset has no confidential state".into(),
                    )
                })?;
            let commitments_empty = st.commitments.is_empty();
            let context = metadata_context_from_state(Some(st));
            let validation = PolicyTransitionValidation {
                current_mode: policy.mode(),
                new_mode: *self.new_mode(),
                commitments_empty,
                block_height: state_transaction.block_height(),
                effective_height: *self.effective_height(),
                delay_blocks: state_transaction.zk.policy_transition_delay_blocks,
                window_blocks: state_transaction.zk.policy_transition_window_blocks,
                requested_window: *self.conversion_window(),
            };
            validate_policy_transition(&validation)?;
            let transition = ConfidentialPolicyTransition {
                new_mode: *self.new_mode(),
                effective_height: *self.effective_height(),
                previous_mode: policy.mode(),
                transition_id: *self.transition_id(),
                conversion_window: *self.conversion_window(),
            };
            policy.pending_transition = Some(transition);
            {
                let asset_definition = state_transaction
                    .world
                    .asset_definition_mut(&asset_def_id)
                    .map_err(Error::from)?;
                asset_definition.set_confidential_policy(policy);
            }
            persist_policy_metadata(state_transaction, &asset_def_id, &policy, &context)?;
            Ok(())
        }
    }

    impl Execute for zk::CancelConfidentialPolicyTransition {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_def_id = self.asset().clone();
            let mut policy = apply_policy_if_due(state_transaction, &asset_def_id)?;
            let context =
                metadata_context_from_state(state_transaction.world.zk_assets.get(&asset_def_id));
            let pending = policy.pending_transition().ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "no confidential policy transition scheduled".into(),
                )
            })?;
            if pending.transition_id() != self.transition_id() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "transition id does not match pending transition".into(),
                    ),
                ));
            }
            policy.pending_transition = None;
            {
                let asset_definition = state_transaction
                    .world
                    .asset_definition_mut(&asset_def_id)
                    .map_err(Error::from)?;
                asset_definition.set_confidential_policy(policy);
            }
            persist_policy_metadata(state_transaction, &asset_def_id, &policy, &context)?;
            Ok(())
        }
    }

    impl Execute for zk::Shield {
        #[allow(clippy::too_many_lines)]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Debit public balance by burning, then append a note commitment to shielded ledger.
            // Policy: ZkNative always ok; Hybrid requires allow_shield.
            let asset_id = AssetId::of(self.asset().clone(), self.from().clone());
            let def_id = self.asset().clone();
            let policy_mode = apply_policy_if_due(state_transaction, &def_id)?.mode();
            match policy_mode {
                ConfidentialPolicyMode::TransparentOnly => {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "shield not permitted by policy".into(),
                    ));
                }
                ConfidentialPolicyMode::Convertible => {
                    if let Some(st) = state_transaction.world.zk_assets.get(&def_id) {
                        if !st.allow_shield {
                            return Err(InstructionExecutionError::InvariantViolation(
                                "shield not permitted by policy".into(),
                            ));
                        }
                    } else {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "shield not permitted by policy".into(),
                        ));
                    }
                }
                ConfidentialPolicyMode::ShieldedOnly => {}
            }
            if !self.enc_payload().is_supported() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "unsupported confidential payload version: {}",
                        self.enc_payload().version()
                    )),
                ));
            }
            let burn = Burn::asset_numeric(
                iroha_primitives::numeric::Numeric::new(*self.amount(), 0),
                asset_id,
            );
            burn.execute(authority, state_transaction)?;
            state_transaction.register_commitments(1)?;
            // Append commitment and update root; emit audit metadata with roots and commitment.
            let mut st = state_transaction
                .world
                .zk_assets
                .get(&def_id)
                .cloned()
                .unwrap_or_default();
            let root_before = st.root_history.last().copied();
            let root_before_hex = root_before.map_or_else(|| hex::encode([0u8; 32]), hex::encode);
            #[cfg(feature = "telemetry")]
            let root_history_before = st.root_history.len();
            let new_root = st.push_commitment(
                *self.note_commitment(),
                state_transaction.zk.root_history_cap,
            );
            let frontier_update = st.record_frontier_checkpoint(
                state_transaction.block_height(),
                state_transaction.zk.tree_frontier_checkpoint_interval,
                state_transaction.zk.reorg_depth_bound,
            );
            #[cfg(feature = "telemetry")]
            let frontier_evictions = frontier_update.evicted;
            #[cfg(not(feature = "telemetry"))]
            let _ = frontier_update;
            let root_after_hex = hex::encode(new_root);
            #[cfg(feature = "telemetry")]
            let telemetry_stats = {
                let root_evictions =
                    root_evictions_since(root_history_before, 1, st.root_history.len());
                st.telemetry_stats(root_evictions, frontier_evictions)
            };
            state_transaction.world.zk_assets.remove(def_id.clone());
            state_transaction.world.zk_assets.insert(def_id.clone(), st);
            #[cfg(feature = "telemetry")]
            state_transaction
                .telemetry
                .record_confidential_tree_stats(&def_id, telemetry_stats);
            // Emit audit metadata pulse under `zk.shield.last`
            let key: Name = "zk.shield.last".parse().unwrap();
            let mut last_map = BTreeMap::new();
            last_map.insert(
                "commitment".into(),
                norito::json::native::Value::from(hex::encode(self.note_commitment())),
            );
            last_map.insert(
                "root_before".into(),
                norito::json::native::Value::from(root_before_hex),
            );
            last_map.insert(
                "root_after".into(),
                norito::json::native::Value::from(root_after_hex),
            );
            let summary =
                iroha_primitives::json::Json::from(norito::json::native::Value::Object(last_map));
            state_transaction
                .world
                .asset_definition_mut(&def_id)
                .map_err(Error::from)
                .map(|def| def.metadata_mut().insert(key.clone(), summary.clone()))?;
            state_transaction.world.emit_events(Some(
                iroha_data_model::prelude::AssetDefinitionEvent::MetadataInserted(
                    iroha_data_model::prelude::MetadataChanged {
                        target: def_id.clone(),
                        key,
                        value: summary,
                    },
                ),
            ));
            let call_hash = state_transaction.tx_call_hash.as_ref().map(|h| {
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(h.as_ref());
                bytes
            });
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::DataEvent::Confidential(
                    ConfidentialEvent::Shielded(ConfidentialShielded {
                        asset_definition: def_id,
                        account: self.from().clone(),
                        commitment: *self.note_commitment(),
                        root_before,
                        root_after: new_root,
                        call_hash,
                    }),
                ),
            ));
            Ok(())
        }
    }

    impl Execute for zk::ZkTransfer {
        #[allow(clippy::too_many_lines)]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Consume nullifiers and append outputs in shielded ledger; no public balance change here.
            // Emit a metadata pulse for observability under a reserved transient key.
            let asset_def_id = self.asset().clone();
            let mut st = state_transaction
                .world
                .zk_assets
                .get(&asset_def_id)
                .cloned()
                .unwrap_or_default();
            state_transaction.register_nullifiers(self.inputs().len())?;
            state_transaction.register_commitments(self.outputs().len())?;
            if let Some(binding) = st.vk_transfer.as_ref() {
                enforce_vk_binding(binding, self.proof())?;
            }
            let policy_mode = apply_policy_if_due(state_transaction, &asset_def_id)?.mode();
            if matches!(policy_mode, ConfidentialPolicyMode::TransparentOnly) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "transfer not permitted by policy".into(),
                ));
            }
            // If a root_hint is provided, enforce it is within the bounded recent root window.
            if let Some(root_hint) = self.root_hint() {
                if !st.root_history.iter().any(|r| r == root_hint) {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "stale or unknown Merkle root".into(),
                    ));
                }
            }
            // Reject duplicated nullifiers
            for &nullifier in self.inputs() {
                if st.nullifiers.contains(&nullifier) {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "duplicate nullifier".into(),
                    ));
                }
            }
            for &nullifier in self.inputs() {
                st.nullifiers.insert(nullifier);
            }
            let root_before = st.root_history.last().copied();
            let root_before_hex = root_before.map_or_else(|| hex::encode([0u8; 32]), hex::encode);
            let mut outputs_sorted = self.outputs().clone();
            // Enforce deterministic output ordering to keep the shielded tree consistent
            // across peers regardless of transaction construction order.
            outputs_sorted.sort_unstable();
            #[cfg(feature = "telemetry")]
            let root_history_before = st.root_history.len();
            #[cfg(feature = "telemetry")]
            let appended_outputs = outputs_sorted.len();
            for &commitment in &outputs_sorted {
                let _ = st.push_commitment(commitment, state_transaction.zk.root_history_cap);
            }
            let frontier_update = st.record_frontier_checkpoint(
                state_transaction.block_height(),
                state_transaction.zk.tree_frontier_checkpoint_interval,
                state_transaction.zk.reorg_depth_bound,
            );
            #[cfg(feature = "telemetry")]
            let frontier_evictions = frontier_update.evicted;
            #[cfg(not(feature = "telemetry"))]
            let _ = frontier_update;
            let root_after = st.root_history.last().copied().unwrap_or([0u8; 32]);
            let root_after_hex = hex::encode(root_after);
            #[cfg(feature = "telemetry")]
            let telemetry_stats = {
                let root_evictions = root_evictions_since(
                    root_history_before,
                    appended_outputs,
                    st.root_history.len(),
                );
                st.telemetry_stats(root_evictions, frontier_evictions)
            };
            let key: Name = "zk.transfer.last".parse().unwrap();
            // Include envelope/proof hash for auditability
            let proof_hash = crate::zk::hash_proof(&self.proof().proof);
            let proof_hash_hex = hex::encode(proof_hash);
            let call_hash_hex = state_transaction
                .tx_call_hash
                .as_ref()
                .map(|h| hex::encode(h.as_ref()))
                .unwrap_or_default();
            let env_hash_hex = self
                .proof()
                .envelope_hash
                .as_ref()
                .map(hex::encode)
                .unwrap_or_default();
            let mut summary_map = norito::json::native::Map::new();
            summary_map.insert(
                "inputs".into(),
                norito::json::native::Value::from(self.inputs().len() as u64),
            );
            summary_map.insert(
                "outputs".into(),
                norito::json::native::Value::from(outputs_sorted.len() as u64),
            );
            summary_map.insert(
                "proof_hash".into(),
                norito::json::native::Value::from(proof_hash_hex),
            );
            summary_map.insert(
                "envelope_hash".into(),
                norito::json::native::Value::from(env_hash_hex),
            );
            summary_map.insert(
                "call_hash".into(),
                norito::json::native::Value::from(call_hash_hex),
            );
            summary_map.insert(
                "root_before".into(),
                norito::json::native::Value::from(root_before_hex),
            );
            summary_map.insert(
                "root_after".into(),
                norito::json::native::Value::from(root_after_hex),
            );
            let outputs_value = outputs_sorted
                .iter()
                .map(|commitment| norito::json::native::Value::from(hex::encode(commitment)))
                .collect();
            summary_map.insert(
                "outputs_commitments".into(),
                norito::json::native::Value::Array(outputs_value),
            );
            let summary = iroha_primitives::json::Json::from(norito::json::native::Value::Object(
                summary_map,
            ));
            state_transaction
                .world
                .asset_definition_mut(&asset_def_id)
                .map_err(Error::from)
                .map(|def| def.metadata_mut().insert(key.clone(), summary.clone()))?;
            state_transaction.world.emit_events(Some(
                iroha_data_model::prelude::AssetDefinitionEvent::MetadataInserted(
                    iroha_data_model::prelude::MetadataChanged {
                        target: asset_def_id.clone(),
                        key,
                        value: summary,
                    },
                ),
            ));
            // Write back updated ZK state
            state_transaction
                .world
                .zk_assets
                .remove(asset_def_id.clone());
            state_transaction
                .world
                .zk_assets
                .insert(asset_def_id.clone(), st);
            #[cfg(feature = "telemetry")]
            state_transaction
                .telemetry
                .record_confidential_tree_stats(&asset_def_id, telemetry_stats);
            let call_hash = state_transaction.tx_call_hash.as_ref().map(|h| {
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(h.as_ref());
                bytes
            });
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::DataEvent::Confidential(
                    ConfidentialEvent::Transferred(ConfidentialTransferred {
                        asset_definition: asset_def_id,
                        nullifiers: self.inputs().clone(),
                        outputs: outputs_sorted,
                        root_before,
                        root_after,
                        proof_hash,
                        envelope_hash: self.proof().envelope_hash,
                        call_hash,
                    }),
                ),
            ));
            Ok(())
        }
    }

    impl Execute for zk::Unshield {
        #[allow(clippy::too_many_lines)]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Consume nullifiers and credit public balance by minting.
            let asset_id = AssetId::of(self.asset().clone(), self.to().clone());
            let def_id = self.asset().clone();
            let policy_mode = apply_policy_if_due(state_transaction, &def_id)?.mode();
            match policy_mode {
                ConfidentialPolicyMode::TransparentOnly | ConfidentialPolicyMode::ShieldedOnly => {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "unshield not permitted by policy".into(),
                    ));
                }
                ConfidentialPolicyMode::Convertible => {
                    if let Some(st) = state_transaction.world.zk_assets.get(&def_id) {
                        if !st.allow_unshield {
                            return Err(InstructionExecutionError::InvariantViolation(
                                "unshield not permitted by policy".into(),
                            ));
                        }
                    } else {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "unshield not permitted by policy".into(),
                        ));
                    }
                }
            }
            let mut st = state_transaction
                .world
                .zk_assets
                .get(&def_id)
                .cloned()
                .unwrap_or_default();
            state_transaction.register_nullifiers(self.inputs().len())?;
            if let Some(binding) = st.vk_unshield.as_ref() {
                enforce_vk_binding(binding, self.proof())?;
            }
            // If a root_hint is provided, enforce it is within the bounded recent root window.
            if let Some(root_hint) = self.root_hint() {
                if !st.root_history.iter().any(|r| r == root_hint) {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "stale or unknown Merkle root".into(),
                    ));
                }
            }
            for &nullifier in self.inputs() {
                if !st.nullifiers.insert(nullifier) {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "duplicate nullifier".into(),
                    ));
                }
            }
            let mint = Mint::asset_numeric(
                iroha_primitives::numeric::Numeric::new(*self.public_amount(), 0),
                asset_id,
            );
            mint.execute(authority, state_transaction)?;
            // Emit an audit pulse with latest unshield info, including proof hash
            let key: Name = "zk.unshield.last".parse().unwrap();
            let proof_hash = crate::zk::hash_proof(&self.proof().proof);
            let proof_hash_hex = hex::encode(proof_hash);
            let pub_amt_u64 = u64::try_from(*self.public_amount()).unwrap_or(u64::MAX);
            let call_hash_hex = state_transaction
                .tx_call_hash
                .as_ref()
                .map(|h| hex::encode(h.as_ref()))
                .unwrap_or_default();
            let env_hash_hex = self
                .proof()
                .envelope_hash
                .as_ref()
                .map(hex::encode)
                .unwrap_or_default();
            let mut summary_map = norito::json::native::Map::new();
            summary_map.insert(
                "inputs".into(),
                norito::json::native::Value::from(self.inputs().len() as u64),
            );
            summary_map.insert(
                "public_amount".into(),
                norito::json::native::Value::from(pub_amt_u64),
            );
            summary_map.insert(
                "proof_hash".into(),
                norito::json::native::Value::from(proof_hash_hex),
            );
            summary_map.insert(
                "envelope_hash".into(),
                norito::json::native::Value::from(env_hash_hex),
            );
            summary_map.insert(
                "call_hash".into(),
                norito::json::native::Value::from(call_hash_hex),
            );
            let summary = iroha_primitives::json::Json::from(norito::json::native::Value::Object(
                summary_map,
            ));
            state_transaction
                .world
                .asset_definition_mut(&def_id)
                .map_err(Error::from)
                .map(|def| def.metadata_mut().insert(key.clone(), summary.clone()))?;
            state_transaction.world.emit_events(Some(
                iroha_data_model::prelude::AssetDefinitionEvent::MetadataInserted(
                    iroha_data_model::prelude::MetadataChanged {
                        target: def_id.clone(),
                        key,
                        value: summary,
                    },
                ),
            ));
            state_transaction.world.zk_assets.remove(def_id.clone());
            state_transaction.world.zk_assets.insert(def_id.clone(), st);
            let call_hash = state_transaction.tx_call_hash.as_ref().map(|h| {
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(h.as_ref());
                bytes
            });
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::DataEvent::Confidential(
                    ConfidentialEvent::Unshielded(ConfidentialUnshielded {
                        asset_definition: def_id,
                        account: self.to().clone(),
                        public_amount: *self.public_amount(),
                        nullifiers: self.inputs().clone(),
                        root_hint: *self.root_hint(),
                        proof_hash,
                        envelope_hash: self.proof().envelope_hash,
                        call_hash,
                    }),
                ),
            ));
            // No new root is produced by unshield (public credit)
            Ok(())
        }
    }

    // --- ZK Voting ---

    fn election_vk_commitment(
        vk_id: &iroha_data_model::proof::VerifyingKeyId,
        label: &str,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<[u8; 32], Error> {
        let record = state_transaction
            .world
            .verifying_keys
            .get(vk_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("{label} verifying key not found").into(),
                )
            })?;
        if record.status != ConfidentialStatus::Active {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("{label} verifying key is not Active").into(),
            ));
        }
        let vk_box = record.key.clone().ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!("{label} verifying key bytes missing").into(),
            )
        })?;
        let commitment = hash_vk(&vk_box);
        if record.commitment != commitment {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("{label} verifying key commitment mismatch").into(),
            ));
        }
        Ok(commitment)
    }

    fn resolve_ballot_vk(
        st: &crate::state::ElectionState,
        attachment: &iroha_data_model::proof::ProofAttachment,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<
        (
            iroha_data_model::proof::VerifyingKeyId,
            iroha_data_model::proof::VerifyingKeyBox,
        ),
        Error,
    > {
        let vk_id = st
            .vk_ballot
            .clone()
            .or_else(|| {
                state_transaction.gov.vk_ballot.as_ref().map(|v| {
                    iroha_data_model::proof::VerifyingKeyId::new(v.backend.clone(), v.name.clone())
                })
            })
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation("verifying key not configured".into())
            })?;
        if let Some(vk_ref) = &attachment.vk_ref {
            if vk_ref != &vk_id {
                return Err(InstructionExecutionError::InvariantViolation(
                    "ballot verifying key ref mismatch".into(),
                ));
            }
        }
        let record = state_transaction
            .world
            .verifying_keys
            .get(&vk_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation("verifying key not found".into())
            })?;
        if record.status != ConfidentialStatus::Active {
            return Err(InstructionExecutionError::InvariantViolation(
                "verifying key is not Active".into(),
            ));
        }
        if let Some(expected) = st.vk_ballot_commitment {
            if record.commitment != expected {
                return Err(InstructionExecutionError::InvariantViolation(
                    "ballot verifying key commitment mismatch".into(),
                ));
            }
        }
        if let Some(expected) = attachment.vk_commitment {
            if record.commitment != expected {
                return Err(InstructionExecutionError::InvariantViolation(
                    "ballot verifying key commitment mismatch".into(),
                ));
            }
        }
        let vk_box = if let Some(inline) = attachment.vk_inline.clone() {
            let commit = hash_vk(&inline);
            if record.commitment != commit {
                return Err(InstructionExecutionError::InvariantViolation(
                    "ballot verifying key commitment mismatch".into(),
                ));
            }
            inline
        } else if let Some(vk) = record.key.clone() {
            let commit = hash_vk(&vk);
            if record.commitment != commit {
                return Err(InstructionExecutionError::InvariantViolation(
                    "ballot verifying key commitment mismatch".into(),
                ));
            }
            vk
        } else {
            return Err(InstructionExecutionError::InvariantViolation(
                "verifying key bytes missing".into(),
            ));
        };
        let backend = vk_id.backend.as_str();
        if backend != attachment.backend.as_str() || vk_box.backend.as_str() != backend {
            return Err(InstructionExecutionError::InvariantViolation(
                "ballot verifying key backend mismatch".into(),
            ));
        }
        Ok((vk_id, vk_box))
    }

    fn resolve_tally_vk(
        st: &crate::state::ElectionState,
        attachment: &iroha_data_model::proof::ProofAttachment,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<
        (
            iroha_data_model::proof::VerifyingKeyId,
            iroha_data_model::proof::VerifyingKeyBox,
        ),
        Error,
    > {
        let vk_id = st
            .vk_tally
            .clone()
            .or_else(|| {
                state_transaction.gov.vk_tally.as_ref().map(|v| {
                    iroha_data_model::proof::VerifyingKeyId::new(v.backend.clone(), v.name.clone())
                })
            })
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation("verifying key not configured".into())
            })?;
        if let Some(vk_ref) = &attachment.vk_ref {
            if vk_ref != &vk_id {
                return Err(InstructionExecutionError::InvariantViolation(
                    "tally verifying key ref mismatch".into(),
                ));
            }
        }
        let record = state_transaction
            .world
            .verifying_keys
            .get(&vk_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "verifying key for tally not found".into(),
                )
            })?;
        if record.status != ConfidentialStatus::Active {
            return Err(InstructionExecutionError::InvariantViolation(
                "verifying key is not Active".into(),
            ));
        }
        if let Some(expected) = st.vk_tally_commitment {
            if record.commitment != expected {
                return Err(InstructionExecutionError::InvariantViolation(
                    "tally verifying key commitment mismatch".into(),
                ));
            }
        }
        if let Some(expected) = attachment.vk_commitment {
            if record.commitment != expected {
                return Err(InstructionExecutionError::InvariantViolation(
                    "tally verifying key commitment mismatch".into(),
                ));
            }
        }
        let vk_box = if let Some(inline) = attachment.vk_inline.clone() {
            let commit = hash_vk(&inline);
            if record.commitment != commit {
                return Err(InstructionExecutionError::InvariantViolation(
                    "tally verifying key commitment mismatch".into(),
                ));
            }
            inline
        } else if let Some(vk) = record.key.clone() {
            let commit = hash_vk(&vk);
            if record.commitment != commit {
                return Err(InstructionExecutionError::InvariantViolation(
                    "tally verifying key commitment mismatch".into(),
                ));
            }
            vk
        } else {
            return Err(InstructionExecutionError::InvariantViolation(
                "verifying key bytes missing".into(),
            ));
        };
        let backend = vk_id.backend.as_str();
        if backend != attachment.backend.as_str() || vk_box.backend.as_str() != backend {
            return Err(InstructionExecutionError::InvariantViolation(
                "tally vk backend mismatch".into(),
            ));
        }
        Ok((vk_id, vk_box))
    }

    impl Execute for zk::CreateElection {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(&state_transaction.world, authority, "CanManageParliament") {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageParliament".into(),
                ));
            }
            // Insert a new election if it doesn't exist.
            let id = self.election_id().clone();
            if state_transaction.world.elections.get(&id).is_some() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "election id already exists".into(),
                ));
            }
            let options = *self.options();
            let tally_slots = usize::try_from(options).map_err(|_| {
                InstructionExecutionError::InvariantViolation(
                    "election options exceed platform limits".into(),
                )
            })?;
            // If a matching referendum is present, require Zk mode; otherwise seed one in Zk mode using config windows.
            if let Some(existing) = state_transaction.world.governance_referenda.get(&id) {
                if existing.mode != crate::state::GovernanceReferendumMode::Zk {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "referendum mode mismatch for election (expected Zk)".into(),
                    ));
                }
            } else {
                let start = state_transaction
                    ._curr_block
                    .height()
                    .get()
                    .saturating_add(state_transaction.gov.min_enactment_delay);
                let span = state_transaction.gov.window_span.max(1);
                let end = start.saturating_add(span.saturating_sub(1));
                state_transaction.world.governance_referenda.insert(
                    id.clone(),
                    crate::state::GovernanceReferendumRecord {
                        h_start: start,
                        h_end: end,
                        status: crate::state::GovernanceReferendumStatus::Proposed,
                        mode: crate::state::GovernanceReferendumMode::Zk,
                    },
                );
            }
            // Bind eligibility to the current citizen registry root
            let citizen_root = citizen_registry_root(&state_transaction.world);
            if citizen_root != *self.eligible_root() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "eligible_root must match citizen registry".into(),
                ));
            }
            // Require ballot/tally verifying keys to be attested and active.
            let ballot_vk_id = self.vk_ballot().clone();
            let ballot_commitment =
                election_vk_commitment(&ballot_vk_id, "ballot", state_transaction)?;

            let tally_vk_id = self.vk_tally().clone();
            let tally_rec = state_transaction
                .world
                .verifying_keys
                .get(&tally_vk_id)
                .cloned()
                .ok_or_else(|| {
                    InstructionExecutionError::InvariantViolation(
                        "tally verifying key not found".into(),
                    )
                })?;
            if tally_rec.status != ConfidentialStatus::Active {
                return Err(InstructionExecutionError::InvariantViolation(
                    "tally verifying key is not Active".into(),
                ));
            }
            let tally_commitment =
                election_vk_commitment(&tally_vk_id, "tally", state_transaction)?;
            let st = crate::state::ElectionState {
                options,
                eligible_root: *self.eligible_root(),
                start_ts: *self.start_ts(),
                end_ts: *self.end_ts(),
                finalized: false,
                tally: vec![0; tally_slots],
                ballot_nullifiers: std::collections::BTreeSet::default(),
                ciphertexts: Vec::new(),
                vk_ballot: Some(self.vk_ballot().clone()),
                vk_ballot_commitment: Some(ballot_commitment),
                vk_tally: Some(self.vk_tally().clone()),
                vk_tally_commitment: Some(tally_commitment),
                domain_tag: self.domain_tag().clone(),
            };
            state_transaction.world.elections.insert(id, st);
            Ok(())
        }
    }

    impl Execute for zk::SubmitBallot {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanSubmitGovernanceBallot",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanSubmitGovernanceBallot".into(),
                ));
            }
            ensure_citizen_for_ballot(authority, &self.election_id, state_transaction)?;

            let id = self.election_id().clone();
            let mut st = state_transaction
                .world
                .elections
                .get(&id)
                .cloned()
                .ok_or_else(|| {
                    InstructionExecutionError::InvariantViolation("unknown election id".into())
                })?;
            if st.finalized {
                return Err(InstructionExecutionError::InvariantViolation(
                    "election already finalized".into(),
                ));
            }
            let citizen_root = citizen_registry_root(&state_transaction.world);
            if st.eligible_root != citizen_root {
                return Err(InstructionExecutionError::InvariantViolation(
                    "citizen registry root mismatch".into(),
                ));
            }
            let (vk_id, vk_box) = resolve_ballot_vk(&st, &self.ballot_proof, state_transaction)?;
            let backend = vk_id.backend.as_str();

            state_transaction.register_confidential_proof(self.ballot_proof.proof.bytes.len())?;
            let report = crate::zk::verify_backend_with_timing(
                backend,
                &self.ballot_proof.proof,
                Some(&vk_box),
            );
            let timeout_budget = state_transaction.zk.verify_timeout;
            if timeout_budget > Duration::ZERO && report.elapsed > timeout_budget {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "ballot proof verification exceeded timeout".into(),
                    ),
                ));
            }
            if !report.ok {
                return Err(InstructionExecutionError::InvariantViolation(
                    "invalid ballot proof".into(),
                ));
            }
            if !st.ballot_nullifiers.insert(*self.nullifier()) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "duplicate ballot nullifier".into(),
                ));
            }
            st.ciphertexts.push(self.ciphertext().clone());
            // Enforce ballot history cap from config
            let cap = state_transaction.zk.ballot_history_cap.max(1);
            if st.ciphertexts.len() > cap {
                let surplus = st.ciphertexts.len() - cap;
                st.ciphertexts.drain(0..surplus);
            }
            state_transaction.world.elections.remove(id.clone());
            state_transaction.world.elections.insert(id, st);
            Ok(())
        }
    }

    impl Execute for zk::FinalizeElection {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(&state_transaction.world, authority, "CanEnactGovernance") {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanEnactGovernance".into(),
                ));
            }
            let id = self.election_id().clone();
            let mut st = state_transaction
                .world
                .elections
                .get(&id)
                .cloned()
                .ok_or_else(|| {
                    InstructionExecutionError::InvariantViolation("unknown election id".into())
                })?;
            if st.finalized {
                return Err(InstructionExecutionError::InvariantViolation(
                    "election already finalized".into(),
                ));
            }
            let citizen_root = citizen_registry_root(&state_transaction.world);
            if st.eligible_root != citizen_root {
                return Err(InstructionExecutionError::InvariantViolation(
                    "citizen registry root mismatch".into(),
                ));
            }
            // Tally shape must match options
            let expected_tally_len = usize::try_from(st.options).map_err(|_| {
                InstructionExecutionError::InvariantViolation(
                    "election options exceed platform limits".into(),
                )
            })?;
            if self.tally().len() != expected_tally_len {
                return Err(InstructionExecutionError::InvariantViolation(
                    "tally length does not match options".into(),
                ));
            }

            let att = self.tally_proof();
            let (vk_id, vk_box) = resolve_tally_vk(&st, att, state_transaction)?;
            let backend = vk_id.backend.as_str();
            state_transaction.register_confidential_proof(att.proof.bytes.len())?;
            let report = crate::zk::verify_backend_with_timing(backend, &att.proof, Some(&vk_box));
            let timeout_budget = state_transaction.zk.verify_timeout;
            if timeout_budget > Duration::ZERO && report.elapsed > timeout_budget {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "tally proof verification exceeded timeout".into(),
                    ),
                ));
            }
            if !report.ok {
                return Err(InstructionExecutionError::InvariantViolation(
                    "invalid tally proof".into(),
                ));
            }
            st.tally.clone_from(self.tally());
            st.finalized = true;
            state_transaction.world.elections.remove(id.clone());
            state_transaction.world.elections.insert(id, st);
            Ok(())
        }
    }

    impl Execute for smart_contract_code::RegisterSmartContractCode {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Enforce permission to register smart contract code/manifests
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanRegisterSmartContractCode",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanRegisterSmartContractCode".into(),
                ));
            }
            let manifest = self.manifest().clone();
            let Some(key @ Hash { .. }) = manifest.code_hash else {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract("manifest.code_hash missing".into()),
                ));
            };
            let provenance = ensure_manifest_signature(&manifest)?;

            let signer_allowed = match authority.controller() {
                AccountController::Single(signatory) => signatory == &provenance.signer,
                AccountController::Multisig(policy) => policy
                    .members()
                    .iter()
                    .any(|member| member.public_key() == &provenance.signer),
            };
            if !signer_allowed {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "manifest signer is not authorised for the submitting account".into(),
                    ),
                ));
            }
            if state_transaction
                .world
                .contract_manifests
                .get(&key)
                .is_some_and(|existing| existing == &manifest)
            {
                return Ok(());
            }
            // Insert/overwrite manifest for code_hash
            state_transaction
                .world
                .contract_manifests
                .insert(key, manifest);
            Ok(())
        }
    }

    impl ValidSingularQuery
        for iroha_data_model::query::smart_contract::prelude::FindContractManifestByCodeHash
    {
        #[metrics(+"find_contract_manifest_by_code_hash")]
        fn execute(
            &self,
            state_ro: &impl StateReadOnly,
        ) -> Result<
            iroha_data_model::smart_contract::manifest::ContractManifest,
            iroha_data_model::query::error::QueryExecutionFail,
        > {
            state_ro
                .world()
                .contract_manifests()
                .get(&self.code_hash)
                .cloned()
                .ok_or(iroha_data_model::query::error::QueryExecutionFail::NotFound)
        }
    }

    /// Collect consensus key identifiers bound to a public key.
    fn consensus_key_ids_for_public_key(
        world: &WorldTransaction<'_, '_>,
        public_key_label: &str,
    ) -> Vec<ConsensusKeyId> {
        world
            .consensus_keys_by_pk
            .get(public_key_label)
            .cloned()
            .unwrap_or_default()
    }

    /// Register a peer (BLS-normal with `PoP`)
    impl Execute for iroha_data_model::isi::register::RegisterPeerWithPop {
        #[metrics(+"register_peer")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            fn peer_key_policy_reason(
                err: &InstructionExecutionError,
            ) -> Option<PeerKeyPolicyRejectReason> {
                let InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(msg),
                ) = err
                else {
                    return None;
                };
                if msg.contains("lead-time policy") {
                    Some(PeerKeyPolicyRejectReason::LeadTimeViolation)
                } else if msg.contains("activation height cannot be in the past") {
                    Some(PeerKeyPolicyRejectReason::ActivationInPast)
                } else if msg.contains("expiry must exceed activation height") {
                    Some(PeerKeyPolicyRejectReason::ExpiryBeforeActivation)
                } else if msg.contains("algorithm") && msg.contains("not allowed") {
                    Some(PeerKeyPolicyRejectReason::DisallowedAlgorithm)
                } else if msg.contains("HSM binding required") {
                    Some(PeerKeyPolicyRejectReason::MissingHsm)
                } else if msg.contains("HSM provider") {
                    Some(PeerKeyPolicyRejectReason::DisallowedProvider)
                } else if msg.contains("identifier collision") {
                    Some(PeerKeyPolicyRejectReason::IdentifierCollision)
                } else {
                    None
                }
            }

            // Validators must support BLS batching: require non-zero cap in pipeline config.
            if state_transaction.pipeline.signature_batch_max_bls == 0 {
                iroha_logger::error!(
                    peer = %self.peer,
                    cap = state_transaction.pipeline.signature_batch_max_bls,
                    "RegisterPeerWithPop rejected: signature_batch_max_bls is zero"
                );
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "signature_batch_max_bls must be > 0 to register a validator peer".into(),
                    ),
                ));
            }
            let peer_id = self.peer.clone();
            // Enforce BLS-normal only for consensus peers.
            if peer_id.public_key().algorithm() != iroha_crypto::Algorithm::BlsNormal {
                crate::sumeragi::status::record_peer_key_policy_reject(
                    PeerKeyPolicyRejectReason::DisallowedAlgorithm,
                );
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "peer public_key must use BLS-Normal (BLS-Small unsupported for peers)"
                            .into(),
                    ),
                ));
            }
            // Verify PoP
            if let Err(err) = iroha_crypto::bls_normal_pop_verify(peer_id.public_key(), &self.pop) {
                iroha_logger::error!(
                    %peer_id,
                    ?err,
                    "RegisterPeerWithPop rejected: invalid BLS PoP"
                );
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "invalid BLS proof-of-possession: {err}"
                    )),
                ));
            }
            let (activation_lead_blocks, sumeragi_params) = {
                let params = state_transaction.world.parameters.get();
                (
                    params.sumeragi.key_activation_lead_blocks,
                    params.sumeragi.clone(),
                )
            };
            let world = &mut state_transaction.world;
            if world.peers.iter().any(|id| id == &peer_id) {
                if state_transaction._curr_block.is_genesis() {
                    iroha_logger::debug!(
                        %peer_id,
                        "Duplicate RegisterPeerWithPop during genesis; treating as no-op"
                    );
                    return Ok(());
                }
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::PeerId(peer_id),
                }
                .into());
            }
            let block_height = state_transaction._curr_block.height().get();
            let activation_expected = if state_transaction._curr_block.is_genesis() {
                block_height
            } else {
                block_height.saturating_add(activation_lead_blocks)
            };
            let activation_height = self.activation_at.unwrap_or(activation_expected);
            if activation_height < block_height {
                crate::sumeragi::status::record_peer_key_policy_reject(
                    PeerKeyPolicyRejectReason::ActivationInPast,
                );
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "consensus key activation height cannot be in the past".into(),
                    ),
                ));
            }
            if activation_height != activation_expected
                && !(state_transaction._curr_block.is_genesis()
                    && activation_height == block_height)
            {
                crate::sumeragi::status::record_peer_key_policy_reject(
                    PeerKeyPolicyRejectReason::LeadTimeViolation,
                );
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "activation height {activation_height} violates lead-time policy; expected {activation_expected}"
                    )),
                ));
            }
            let status = if activation_height > block_height {
                ConsensusKeyStatus::Pending
            } else {
                ConsensusKeyStatus::Active
            };
            let hsm_binding = match (self.hsm.clone(), sumeragi_params.key_require_hsm) {
                (Some(binding), _) => Some(binding),
                (None, true) => {
                    crate::sumeragi::status::record_peer_key_policy_reject(
                        PeerKeyPolicyRejectReason::MissingHsm,
                    );
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "HSM binding required for consensus key".into(),
                        ),
                    ));
                }
                (None, false) => None,
            };
            let key_label = peer_id.public_key().to_string();
            let candidate_id = derive_validator_key_id(peer_id.public_key());
            if let Some(conflict) = consensus_key_ids_for_public_key(world, &key_label)
                .into_iter()
                .find(|id| id != &candidate_id)
            {
                crate::sumeragi::status::record_peer_key_policy_reject(
                    PeerKeyPolicyRejectReason::IdentifierCollision,
                );
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "consensus key identifier collision for peer public key; existing id: {conflict}"
                    )),
                ));
            }
            if let Some(existing) = world.consensus_keys.get(&candidate_id) {
                if existing.public_key != *peer_id.public_key() {
                    crate::sumeragi::status::record_peer_key_policy_reject(
                        PeerKeyPolicyRejectReason::IdentifierCollision,
                    );
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "consensus key identifier collision for peer public key".into(),
                        ),
                    ));
                }
            }
            let lifecycle_record = ConsensusKeyRecord {
                id: candidate_id,
                public_key: peer_id.public_key().clone(),
                activation_height,
                expiry_height: self.expiry_at,
                hsm: hsm_binding,
                replaces: None,
                status,
            };
            if let Err(err) = validate_consensus_key_record(
                &lifecycle_record,
                &sumeragi_params,
                None,
                block_height,
                state_transaction._curr_block.is_genesis(),
            ) {
                if let Some(reason) = peer_key_policy_reason(&err) {
                    crate::sumeragi::status::record_peer_key_policy_reject(reason);
                }
                return Err(err);
            }
            if let PushResult::Duplicate(duplicate) = world.peers.push(peer_id.clone()) {
                if state_transaction._curr_block.is_genesis() {
                    iroha_logger::debug!(
                        %duplicate,
                        "Duplicate RegisterPeerWithPop during genesis; treating as no-op"
                    );
                    return Ok(());
                }
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::PeerId(duplicate),
                }
                .into());
            }
            upsert_consensus_key(world, &lifecycle_record.id, lifecycle_record.clone());
            crate::sumeragi::status::record_consensus_key(lifecycle_record);
            world.emit_events(Some(PeerEvent::Added(peer_id)));
            Ok(())
        }
    }

    impl Execute for Unregister<Peer> {
        #[metrics(+"unregister_peer")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let peer_id = self.object().clone();
            let world = &mut state_transaction.world;
            let Some(index) = world.peers.iter().position(|id| id == &peer_id) else {
                return Err(FindError::Peer(peer_id).into());
            };

            world.peers.remove(index);
            // Mark any validators tied to this peer as exited to avoid dangling roster entries.
            let exited_keys: Vec<_> = world
                .public_lane_validators
                .iter()
                .filter(|(_, record)| {
                    record
                        .validator
                        .try_signatory()
                        .is_some_and(|pk| pk == peer_id.public_key())
                })
                .map(|(key, _)| key.clone())
                .collect();
            for key in exited_keys {
                if let Some(record) = world.public_lane_validators.get_mut(&key) {
                    record.status = iroha_data_model::nexus::PublicLaneValidatorStatus::Exited;
                    // Prune stake shares so roster/state snapshots do not retain exited validators.
                    let share_keys: Vec<_> = world
                        .public_lane_stake_shares
                        .iter()
                        .filter(|((lane, validator_id, _), _)| {
                            *lane == key.0 && validator_id == &record.validator
                        })
                        .map(|(share_key, _)| share_key.clone())
                        .collect();
                    for share_key in share_keys {
                        world.public_lane_stake_shares.remove(share_key);
                    }
                }
            }

            let key_label = peer_id.public_key().to_string();
            let block_height = state_transaction._curr_block.height().get();
            let lifecycle_record = ConsensusKeyRecord {
                id: derive_validator_key_id(peer_id.public_key()),
                public_key: peer_id.public_key().clone(),
                activation_height: block_height,
                expiry_height: Some(block_height),
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Disabled,
            };
            upsert_consensus_key(world, &lifecycle_record.id, lifecycle_record.clone());
            crate::sumeragi::status::record_consensus_key(lifecycle_record.clone());
            let mut ids = consensus_key_ids_for_public_key(world, &key_label);
            if !ids.contains(&lifecycle_record.id) {
                ids.push(lifecycle_record.id.clone());
            }
            ids.sort();
            ids.dedup();
            world.consensus_keys_by_pk.insert(key_label, ids.clone());
            for id in ids {
                if id == lifecycle_record.id {
                    continue;
                }
                if let Some(mut record) = world.consensus_keys.get(&id).cloned() {
                    if !matches!(record.status, ConsensusKeyStatus::Disabled) {
                        record.status = ConsensusKeyStatus::Disabled;
                        record.expiry_height.get_or_insert(block_height);
                        world.consensus_keys.insert(id.clone(), record.clone());
                        crate::sumeragi::status::record_consensus_key(record);
                    }
                }
            }
            world.emit_events(Some(PeerEvent::Removed(peer_id)));

            Ok(())
        }
    }

    impl Execute for Register<Domain> {
        #[metrics("register_domain")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let Register { object: new_domain } = self;
            let raw_domain_id = new_domain.id.clone();

            let canonical_label = name::canonicalize_domain_label(raw_domain_id.name().as_ref())
                .map_err(|err| {
                    InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(err.reason().into()),
                    )
                })?;
            let canonical_name = Name::from_str(&canonical_label).map_err(|err| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    err.reason().into(),
                ))
            })?;
            let canonical_id = DomainId::new(canonical_name);

            if canonical_id == *iroha_genesis::GENESIS_DOMAIN_ID {
                return Err(InstructionExecutionError::InvariantViolation(
                    "Not allowed to register genesis domain".to_owned().into(),
                ));
            }

            let mut domain = new_domain.build(authority);
            domain.id = canonical_id.clone();
            let requires_endorsement = state_transaction
                .world
                .domain_endorsement_policies
                .get(&canonical_id)
                .map_or(
                    state_transaction.nexus.enabled
                        && state_transaction.nexus.endorsement.quorum > 0,
                    |policy| policy.required,
                );
            if requires_endorsement {
                validate_domain_endorsement(&canonical_id, &domain.metadata, state_transaction)?;
            }
            let event_domain = domain.clone();
            let world = &mut state_transaction.world;

            if let Some(existing) = world.domains.insert(canonical_id.clone(), domain) {
                let _ = world.domains.insert(canonical_id.clone(), existing);
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::DomainId(canonical_id.clone()),
                }
                .into());
            }

            world.emit_events(Some(DomainEvent::Created(event_domain)));

            Ok(())
        }
    }

    fn endorsement_error(message: impl Into<String>) -> InstructionExecutionError {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message.into(),
        ))
    }

    fn validate_domain_endorsement(
        canonical_id: &DomainId,
        metadata: &Metadata,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        const ENDORSEMENT_KEY: &str = "endorsement";
        let key = Name::from_str(ENDORSEMENT_KEY).map_err(|err| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                format!("invalid endorsement metadata key: {err}"),
            ))
        })?;
        let Some(raw) = metadata.get(&key) else {
            return Err(endorsement_error(
                "domain endorsement required (nexus.endorsement.quorum > 0)",
            ));
        };
        let endorsement: DomainEndorsement = raw.clone().try_into_any_norito().map_err(|err| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                format!("invalid domain endorsement payload: {err}"),
            ))
        })?;

        verify_domain_endorsement_payload(canonical_id, endorsement, state_transaction, None)
    }

    fn ensure_endorsement_identity(
        domain_id: &DomainId,
        endorsement: &DomainEndorsement,
    ) -> Result<(), InstructionExecutionError> {
        if endorsement.domain_id != *domain_id {
            return Err(endorsement_error("domain endorsement domain_id mismatch"));
        }
        let expected_hash = Hash::new(domain_id.to_string().as_bytes());
        if endorsement.statement_hash != expected_hash {
            return Err(endorsement_error(
                "domain endorsement statement_hash mismatch",
            ));
        }
        if endorsement.version != iroha_data_model::nexus::DOMAIN_ENDORSEMENT_VERSION_V1 {
            return Err(endorsement_error("domain endorsement version unsupported"));
        }
        Ok(())
    }

    fn resolve_endorsement_policy(
        domain_id: &DomainId,
        policy_override: Option<DomainEndorsementPolicy>,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<DomainEndorsementPolicy, InstructionExecutionError> {
        policy_override
            .or_else(|| {
                state_transaction
                    .world
                    .domain_endorsement_policies
                    .get(domain_id)
                    .cloned()
            })
            .or_else(|| {
                if state_transaction.nexus.enabled && state_transaction.nexus.endorsement.quorum > 0
                {
                    Some(DomainEndorsementPolicy {
                        committee_id: "default".to_owned(),
                        max_endorsement_age: u64::MAX,
                        required: true,
                    })
                } else {
                    None
                }
            })
            .ok_or_else(|| endorsement_error("domain endorsement policy missing"))
    }

    fn validate_endorsement_scope(
        endorsement: &DomainEndorsement,
        policy: &DomainEndorsementPolicy,
        height: u64,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if endorsement.committee_id != policy.committee_id {
            return Err(endorsement_error(
                "domain endorsement committee_id mismatch",
            ));
        }
        if endorsement.expires_at_height <= height {
            return Err(endorsement_error("domain endorsement expired"));
        }
        if height < endorsement.issued_at_height {
            return Err(endorsement_error(
                "domain endorsement issued_at_height in future",
            ));
        }
        if height.saturating_sub(endorsement.issued_at_height) > policy.max_endorsement_age {
            return Err(endorsement_error("domain endorsement is stale"));
        }
        if let (Some(start), Some(end)) =
            (endorsement.scope.block_start, endorsement.scope.block_end)
        {
            if start > end {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "domain endorsement scope start exceeds end".into(),
                    ),
                ));
            }
        }
        if !endorsement.scope.contains_height(height) {
            return Err(endorsement_error(
                "domain endorsement outside declared block window",
            ));
        }
        if let Some(dataspace) = endorsement.scope.dataspace {
            let known = state_transaction
                .nexus
                .dataspace_catalog
                .entries()
                .iter()
                .any(|entry| entry.id == dataspace);
            if !known {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "domain endorsement dataspace unknown".into(),
                    ),
                ));
            }
        }
        Ok(())
    }

    fn resolve_endorsement_committee(
        policy: &DomainEndorsementPolicy,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<DomainCommittee, InstructionExecutionError> {
        state_transaction
            .world
            .domain_committees
            .get(&policy.committee_id)
            .cloned()
            .or_else(|| {
                let keys = &state_transaction.nexus.endorsement.committee_keys;
                if keys.is_empty() {
                    None
                } else {
                    Some(DomainCommittee {
                        committee_id: policy.committee_id.clone(),
                        members: keys
                            .iter()
                            .filter_map(|key| PublicKey::from_str(key).ok())
                            .collect(),
                        quorum: state_transaction.nexus.endorsement.quorum,
                        metadata: Metadata::default(),
                    })
                }
            })
            .ok_or_else(|| endorsement_error("domain endorsement committee not found"))
    }

    fn ensure_unique_endorsement(
        msg_hash: &iroha_crypto::HashOf<DomainEndorsement>,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if state_transaction
            .world
            .domain_endorsements
            .get(msg_hash)
            .is_some()
        {
            return Err(endorsement_error("domain endorsement already recorded"));
        }
        Ok(())
    }

    fn count_endorsement_approvals(
        endorsement: &DomainEndorsement,
        committee: &DomainCommittee,
        height: u64,
        overlap: u64,
        expiry_grace: u64,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<u16, InstructionExecutionError> {
        let committee_keys: BTreeSet<_> = committee.members.iter().cloned().collect();
        if committee_keys.is_empty() {
            return Err(endorsement_error(
                "domain endorsement committee has no members",
            ));
        }
        let msg_hash = endorsement.body_hash();
        let mut seen = BTreeSet::new();
        let mut approvals: u16 = 0;
        for sig in &endorsement.signatures {
            if !committee_keys.contains(&sig.signer) {
                continue;
            }
            if !seen.insert(sig.signer.clone()) {
                continue;
            }
            let key_ids = state_transaction
                .world
                .consensus_keys_by_pk
                .get(&sig.signer.to_string())
                .cloned()
                .unwrap_or_default();
            let mut signature_valid = false;
            for key_id in key_ids {
                let Some(rec) = state_transaction.world.consensus_keys.get(&key_id) else {
                    continue;
                };
                if rec.id.role != ConsensusKeyRole::Endorsement {
                    continue;
                }
                if !rec.is_live_at(height, overlap, expiry_grace) {
                    continue;
                }
                if sig
                    .signature
                    .verify(&rec.public_key, msg_hash.as_ref())
                    .is_ok()
                {
                    signature_valid = true;
                    break;
                }
            }
            if signature_valid {
                approvals = approvals.saturating_add(1);
            }
        }
        Ok(approvals)
    }

    fn persist_endorsement_record(
        domain_id: &DomainId,
        endorsement: DomainEndorsement,
        height: u64,
        msg_hash: iroha_crypto::HashOf<DomainEndorsement>,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) {
        state_transaction.world.domain_endorsements.insert(
            msg_hash,
            DomainEndorsementRecord {
                endorsement,
                accepted_at_height: height,
            },
        );
        let mut domain_index = state_transaction
            .world
            .domain_endorsements_by_domain
            .get(domain_id)
            .cloned()
            .unwrap_or_default();
        if !domain_index.contains(&msg_hash) {
            domain_index.push(msg_hash);
            state_transaction
                .world
                .domain_endorsements_by_domain
                .insert(domain_id.clone(), domain_index);
        }
    }

    fn verify_domain_endorsement_payload(
        domain_id: &DomainId,
        endorsement: DomainEndorsement,
        state_transaction: &mut StateTransaction<'_, '_>,
        policy_override: Option<DomainEndorsementPolicy>,
    ) -> Result<(), InstructionExecutionError> {
        let height = state_transaction.block_height();
        ensure_endorsement_identity(domain_id, &endorsement)?;
        let policy = resolve_endorsement_policy(domain_id, policy_override, state_transaction)?;
        if !policy.required {
            return Ok(());
        }
        validate_endorsement_scope(&endorsement, &policy, height, state_transaction)?;
        let committee = resolve_endorsement_committee(&policy, state_transaction)?;
        if !committee.is_valid() {
            return Err(InstructionExecutionError::InvariantViolation(
                "domain endorsement committee invalid (quorum/members)".into(),
            ));
        }
        let params = state_transaction.world.parameters.get();
        let overlap = params.sumeragi.key_overlap_grace_blocks;
        let expiry_grace = params.sumeragi.key_expiry_grace_blocks;
        let msg_hash = endorsement.body_hash();
        ensure_unique_endorsement(&msg_hash, state_transaction)?;
        let approvals = count_endorsement_approvals(
            &endorsement,
            &committee,
            height,
            overlap,
            expiry_grace,
            state_transaction,
        )?;
        if approvals < committee.quorum {
            return Err(InstructionExecutionError::InvariantViolation(
                "domain endorsement does not meet quorum".into(),
            ));
        }
        persist_endorsement_record(domain_id, endorsement, height, msg_hash, state_transaction);
        Ok(())
    }

    impl Execute for endorsement::RegisterDomainCommittee {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageConsensusKeys",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageConsensusKeys".into(),
                ));
            }

            let committee = self.committee().clone();
            if !committee.is_valid() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "invalid domain committee shape (members/quorum)".into(),
                    ),
                ));
            }
            let mut seen = BTreeSet::new();
            let params = state_transaction.world.parameters.get();
            let overlap = params.sumeragi.key_overlap_grace_blocks;
            let expiry_grace = params.sumeragi.key_expiry_grace_blocks;
            for member in &committee.members {
                if !seen.insert(member.clone()) {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "duplicate domain committee member".into(),
                        ),
                    ));
                }
                let key_ids = state_transaction
                    .world
                    .consensus_keys_by_pk
                    .get(&member.to_string())
                    .cloned()
                    .unwrap_or_default();
                let mut has_live_endorsement_key = false;
                for key_id in key_ids {
                    let Some(rec) = state_transaction.world.consensus_keys.get(&key_id) else {
                        continue;
                    };
                    if rec.id.role != ConsensusKeyRole::Endorsement {
                        continue;
                    }
                    if rec.is_live_at(state_transaction.block_height(), overlap, expiry_grace) {
                        has_live_endorsement_key = true;
                        break;
                    }
                }
                if !has_live_endorsement_key {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "committee member lacks a live endorsement key".into(),
                        ),
                    ));
                }
            }

            if state_transaction
                .world
                .domain_committees
                .get(&committee.committee_id)
                .is_some()
            {
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::Permission(Permission::new(
                        "DomainCommittee".into(),
                        iroha_primitives::json::Json::from(committee.committee_id.as_str()),
                    )),
                }
                .into());
            }

            state_transaction
                .world
                .domain_committees
                .insert(committee.committee_id.clone(), committee);
            Ok(())
        }
    }

    impl Execute for endorsement::SetDomainEndorsementPolicy {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageConsensusKeys",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageConsensusKeys".into(),
                ));
            }

            let domain = self.domain().clone();
            if state_transaction.world.domains.get(&domain).is_none() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "domain not found for endorsement policy".into(),
                    ),
                ));
            }

            let mut policy = self.policy().clone();
            if policy.max_endorsement_age == 0 {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "max_endorsement_age must be positive".into(),
                    ),
                ));
            }

            let Some(committee) = state_transaction
                .world
                .domain_committees
                .get(&policy.committee_id)
                .cloned()
            else {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "committee not registered for endorsement policy".into(),
                    ),
                ));
            };

            if !committee.is_valid() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "committee invalid for endorsement policy".into(),
                    ),
                ));
            }

            // Normalize committee id casing to avoid duplicates.
            policy.committee_id.clone_from(&committee.committee_id);
            state_transaction
                .world
                .domain_endorsement_policies
                .insert(domain, policy);
            Ok(())
        }
    }

    impl Execute for endorsement::SubmitDomainEndorsement {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let endorsement = self.endorsement().clone();
            let domain_id = endorsement.domain_id.clone();
            verify_domain_endorsement_payload(&domain_id, endorsement, state_transaction, None)?;
            Ok(())
        }
    }

    impl Execute for nexus::SetLaneRelayEmergencyValidators {
        #[metrics(+"set_lane_relay_emergency_validators")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(&state_transaction.world, authority, "CanManagePeers") {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManagePeers".into(),
                ));
            }
            if !state_transaction.nexus.enabled {
                return Err(InstructionExecutionError::InvariantViolation(
                    "lane relay emergency override requires nexus.enabled=true".into(),
                ));
            }
            if !state_transaction.nexus.lane_relay_emergency.enabled {
                return Err(InstructionExecutionError::InvariantViolation(
                    "lane relay emergency override requires nexus.lane_relay_emergency.enabled=true"
                        .into(),
                ));
            }

            let required_threshold = state_transaction
                .nexus
                .lane_relay_emergency
                .multisig_threshold;
            let required_members = state_transaction
                .nexus
                .lane_relay_emergency
                .multisig_members;
            let Some(policy) = authority.controller().multisig_policy() else {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "lane relay emergency override requires multisig authority (threshold >= {required_threshold}, members >= {required_members})"
                    )
                    .into(),
                ));
            };
            if policy.threshold() < required_threshold.get()
                || policy.members().len() < usize::from(required_members.get())
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "lane relay emergency override requires multisig authority (threshold >= {required_threshold}, members >= {required_members})"
                    )
                    .into(),
                ));
            }

            let dataspace_id = *self.dataspace_id();
            if !state_transaction
                .nexus
                .dataspace_catalog
                .entries()
                .iter()
                .any(|entry| entry.id == dataspace_id)
            {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "unknown dataspace id {}",
                        dataspace_id.as_u64()
                    )),
                ));
            }

            let mut validators = self.validators().clone();
            validators.sort();
            validators.dedup();
            for validator in &validators {
                state_transaction.world.account(validator)?;
            }

            if validators.is_empty() {
                state_transaction
                    .world
                    .lane_relay_emergency_validators
                    .remove(dataspace_id);
                return Ok(());
            }

            state_transaction
                .world
                .lane_relay_emergency_validators
                .insert(
                    dataspace_id,
                    LaneRelayEmergencyValidatorSet {
                        validators,
                        expires_at_height: *self.expires_at_height(),
                        metadata: self.metadata().clone(),
                    },
                );
            Ok(())
        }
    }

    impl Execute for Unregister<Domain> {
        #[metrics("unregister_domain")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let domain_id = self.object().clone();

            state_transaction
                .world()
                .triggers()
                .inspect_by_action(
                    |action| action.authority().domain() == &domain_id,
                    |trigger_id, _| trigger_id.clone(),
                )
                .collect::<Vec<_>>()
                .into_iter()
                .for_each(|trigger_id| {
                    state_transaction
                        .world
                        .triggers
                        .remove(trigger_id)
                        .then_some(())
                        .expect("should succeed")
                });

            let remove_accounts: Vec<AccountId> = state_transaction
                .world
                .accounts_in_domain_iter(&domain_id)
                .map(|account| account.id().clone())
                .collect();
            for account in remove_accounts {
                state_transaction
                    .world
                    .account_permissions
                    .remove(account.clone());

                state_transaction.world.remove_account_roles(&account);

                let remove_assets: Vec<AssetId> = state_transaction
                    .world
                    .assets_in_account_iter(&account)
                    .map(|ad| ad.id().clone())
                    .collect();
                for asset_id in remove_assets {
                    state_transaction.world.assets.remove(asset_id);
                }

                state_transaction.world.accounts.remove(account);
            }

            let remove_asset_definitions: Vec<AssetDefinitionId> = state_transaction
                .world
                .asset_definitions_in_domain_iter(&domain_id)
                .map(|ad| ad.id().clone())
                .collect();
            for asset_definition_id in remove_asset_definitions {
                state_transaction
                    .world
                    .asset_definitions
                    .remove(asset_definition_id.clone());
            }

            let remove_nfts: Vec<NftId> = state_transaction
                .world
                .nfts_in_domain_iter(&domain_id)
                .map(|nft| nft.id().clone())
                .collect();
            for nft_id in remove_nfts {
                state_transaction.world.nfts.remove(nft_id.clone());
            }

            if state_transaction
                .world
                .domains
                .remove(domain_id.clone())
                .is_none()
            {
                return Err(FindError::Domain(domain_id).into());
            }

            state_transaction
                .world
                .emit_events(Some(DomainEvent::Deleted(domain_id)));

            Ok(())
        }
    }

    impl Execute for Register<Role> {
        #[metrics(+"register_role")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Preserve the requested initial owner before building the role.
            let new_role = self.object().clone();
            let initial_owner = new_role.grant_to().clone();
            let role = new_role.build(authority);

            if state_transaction.world.roles.get(role.id()).is_some() {
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::RoleId(role.id().clone()),
                }
                .into());
            }

            let world = &mut state_transaction.world;
            let role_id = role.id().clone();
            world.roles.insert(role_id.clone(), role.clone());

            // Emit the RoleCreated event first so the observable order matches
            // how clients expect to see events related to this role.
            world.emit_events(Some(RoleEvent::Created(role)));

            // Auto‑grant the newly created role to the designated initial owner
            // to reflect the semantic of `NewRole { grant_to, .. }`.
            // Reuse the existing Grant<RoleId, Account> execution to keep
            // validation and event emission consistent.
            // If the `initial_owner` account does not exist, surface a precise
            // "find account" error to the caller via the Grant path.
            Grant::account_role(role_id, initial_owner).execute(authority, state_transaction)?;

            Ok(())
        }
    }

    impl Execute for Unregister<Role> {
        #[metrics("unregister_role")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let role_id = self.object().clone();

            let accounts_with_role = state_transaction
                .world
                .account_roles
                .iter()
                .map(|(role, ())| role)
                .filter(|role| role.id.eq(&role_id))
                .map(|role| &role.account)
                .cloned()
                .collect::<Vec<_>>();

            for account_id in accounts_with_role {
                let revoke = Revoke::account_role(role_id.clone(), account_id);
                revoke.execute(authority, state_transaction)?
            }

            let world = &mut state_transaction.world;
            if world.roles.remove(role_id.clone()).is_none() {
                return Err(FindError::Role(role_id).into());
            }

            world.emit_events(Some(RoleEvent::Deleted(role_id)));

            Ok(())
        }
    }

    impl Execute for Grant<Permission, Role> {
        #[metrics(+"grant_role_permission")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let role_id = self.destination().clone();
            let permission = self.object().clone();

            let Some(existing_role) = state_transaction.world.roles.get(&role_id) else {
                return Err(FindError::Role(role_id).into());
            };

            if existing_role.permissions().any(|p| p == &permission) {
                return Err(RepetitionError {
                    instruction: InstructionType::Grant,
                    id: permission.clone().into(),
                }
                .into());
            }

            let impacted_accounts = state_transaction.accounts_with_role(&role_id);

            // Rebuild role with added permission
            let mut new_role = Role::new(existing_role.id().clone(), authority.clone());
            for p in existing_role.permissions() {
                new_role = new_role.add_permission(p.clone());
            }
            new_role = new_role.add_permission(permission.clone());
            let new_role = new_role.build(authority);
            state_transaction
                .world
                .roles
                .insert(role_id.clone(), new_role);

            state_transaction
                .world
                .emit_events(Some(RoleEvent::PermissionAdded(RolePermissionChanged {
                    role: role_id,
                    permission,
                })));

            state_transaction.invalidate_permission_cache_for(impacted_accounts.iter());

            Ok(())
        }
    }

    impl Execute for Revoke<Permission, Role> {
        #[metrics(+"grant_role_permission")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let role_id = self.destination().clone();
            let permission = self.object().clone();

            let Some(existing_role) = state_transaction.world.roles.get(&role_id) else {
                return Err(FindError::Role(role_id).into());
            };

            if existing_role.permissions().all(|p| p != &permission) {
                return Err(FindError::Permission(permission.clone().into()).into());
            }

            let impacted_accounts = state_transaction.accounts_with_role(&role_id);

            // Rebuild role with removed permission
            let mut new_role = Role::new(existing_role.id().clone(), authority.clone());
            for p in existing_role.permissions() {
                if p != &permission {
                    new_role = new_role.add_permission(p.clone());
                }
            }
            let new_role = new_role.build(authority);
            state_transaction
                .world
                .roles
                .insert(role_id.clone(), new_role);

            state_transaction
                .world
                .emit_events(Some(RoleEvent::PermissionRemoved(RolePermissionChanged {
                    role: role_id,
                    permission,
                })));

            state_transaction.invalidate_permission_cache_for(impacted_accounts.iter());

            Ok(())
        }
    }

    impl Execute for SetParameter {
        #[metrics(+"set_parameter")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Handle scheduling parameters specially since they are optional in WSV
            // and are not listed by `Parameters::parameters()` iterator.
            match self.inner().clone() {
                Parameter::Sumeragi(iroha_data_model::parameter::SumeragiParameter::NextMode(
                    next_mode,
                )) => {
                    let params_view = state_transaction.world.parameters.get();
                    // Avoid redundant updates when the requested mode is already staged.
                    if params_view.sumeragi().next_mode() == Some(next_mode) {
                        return Ok(());
                    }

                    let params = state_transaction.world.parameters.get_mut();
                    params.set_parameter(Parameter::Sumeragi(
                        iroha_data_model::parameter::SumeragiParameter::NextMode(next_mode),
                    ));
                    state_transaction.mark_mode_cutover_next_set();
                    state_transaction
                        .world
                        .emit_events(Some(ConfigurationEvent::Changed(ParameterChanged {
                            old_value: Parameter::Sumeragi(
                                iroha_data_model::parameter::SumeragiParameter::NextMode(next_mode),
                            ),
                            new_value: Parameter::Sumeragi(
                                iroha_data_model::parameter::SumeragiParameter::NextMode(next_mode),
                            ),
                        })));
                    return Ok(());
                }
                Parameter::Sumeragi(
                    iroha_data_model::parameter::SumeragiParameter::ModeActivationHeight(height),
                ) => {
                    let params_view = state_transaction.world.parameters.get();
                    if params_view.sumeragi().next_mode().is_none() {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "mode_activation_height requires next_mode to be set in the same block"
                                    .to_owned(),
                            ),
                        ));
                    }

                    let current_height = state_transaction._curr_block.height().get();
                    if height <= current_height {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(format!(
                                "mode_activation_height {height} must be greater than current block height {current_height} to enforce joint-consensus activation"
                            )),
                        ));
                    }

                    let params = state_transaction.world.parameters.get_mut();
                    params.set_parameter(Parameter::Sumeragi(
                        iroha_data_model::parameter::SumeragiParameter::ModeActivationHeight(
                            height,
                        ),
                    ));
                    state_transaction.mark_mode_cutover_activation_set();
                    state_transaction
                        .world
                        .emit_events(Some(ConfigurationEvent::Changed(ParameterChanged {
                        old_value: Parameter::Sumeragi(
                            iroha_data_model::parameter::SumeragiParameter::ModeActivationHeight(
                                height,
                            ),
                        ),
                        new_value: Parameter::Sumeragi(
                            iroha_data_model::parameter::SumeragiParameter::ModeActivationHeight(
                                height,
                            ),
                        ),
                    })));
                    return Ok(());
                }
                _ => {}
            }
            match self.inner().clone() {
                Parameter::Sumeragi(
                    iroha_data_model::parameter::SumeragiParameter::CollectorsK(0),
                ) => {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "sumeragi.collectors_k must be greater than zero".to_owned(),
                        ),
                    ));
                }
                Parameter::Sumeragi(
                    iroha_data_model::parameter::SumeragiParameter::RedundantSendR(0),
                ) => {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "sumeragi.collectors_redundant_send_r must be greater than zero"
                                .to_owned(),
                        ),
                    ));
                }
                _ => {}
            }
            macro_rules! set_parameter {
                ($($container:ident($param:ident.$field:ident) => $single:ident::$variant:ident),* $(,)?) => {
                    match self.inner().clone() { $(
                        Parameter::$container(iroha_data_model::parameter::$single::$variant(next)) => {
                            let params = state_transaction.world.parameters.get_mut();
                            let prev_value = params.$param.$field;
                            let prev_param = Parameter::$container(
                                iroha_data_model::parameter::$single::$variant(prev_value),
                            );
                            let new_param = Parameter::$container(
                                iroha_data_model::parameter::$single::$variant(next),
                            );

                            params.set_parameter(new_param.clone());

                            state_transaction.world.emit_events(
                                Some(ConfigurationEvent::Changed(ParameterChanged {
                                    old_value: prev_param,
                                    new_value: new_param,
                                }))
                            );
                        })*
                        Parameter::Custom(next) => {
                            let params = state_transaction.world.parameters.get_mut();
                            // Set new value via public setter; previous value is unknown via public API
                            params.set_parameter(Parameter::Custom(next.clone()));

                            state_transaction
                                .world
                                .emit_events(Some(ConfigurationEvent::Changed(ParameterChanged {
                                    old_value: Parameter::Custom(next.clone()),
                                    new_value: Parameter::Custom(next),
                                })));
                        }
                        _ => {}
                    }
                };
            }

            set_parameter!(
                Sumeragi(sumeragi.max_clock_drift_ms) => SumeragiParameter::MaxClockDriftMs,
                Sumeragi(sumeragi.block_time_ms) => SumeragiParameter::BlockTimeMs,
                Sumeragi(sumeragi.commit_time_ms) => SumeragiParameter::CommitTimeMs,
                Sumeragi(sumeragi.collectors_k) => SumeragiParameter::CollectorsK,
                Sumeragi(sumeragi.collectors_redundant_send_r) => SumeragiParameter::RedundantSendR,
                Sumeragi(sumeragi.da_enabled) => SumeragiParameter::DaEnabled,

                Block(block.max_transactions) => BlockParameter::MaxTransactions,

                Transaction(transaction.max_instructions) => TransactionParameter::MaxInstructions,
                Transaction(transaction.ivm_bytecode_size) => TransactionParameter::IvmBytecodeSize,
                Transaction(transaction.max_tx_bytes) => TransactionParameter::MaxTxBytes,
                Transaction(transaction.max_decompressed_bytes) => TransactionParameter::MaxDecompressedBytes,
                Transaction(transaction.max_metadata_depth) => TransactionParameter::MaxMetadataDepth,

                SmartContract(smart_contract.fuel) => SmartContractParameter::Fuel,
                SmartContract(smart_contract.memory) => SmartContractParameter::Memory,
                SmartContract(smart_contract.execution_depth) => SmartContractParameter::ExecutionDepth,

                Executor(executor.fuel) => SmartContractParameter::Fuel,
                Executor(executor.memory) => SmartContractParameter::Memory,
                Executor(executor.execution_depth) => SmartContractParameter::ExecutionDepth,
            );

            Ok(())
        }
    }

    impl Execute for Upgrade {
        #[metrics(+"upgrade_executor")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let raw_executor = self.executor();

            // Cloning executor to avoid multiple mutable borrows of `state_transaction`.
            let mut upgraded_executor = state_transaction.world.executor.clone();
            upgraded_executor
                .migrate(raw_executor.clone(), state_transaction, authority)
                .map_err(|migration_error| {
                    InvalidParameterError::SmartContract(format!(
                        "{:?}",
                        eyre::eyre!(migration_error).wrap_err("Migration failed"),
                    ))
                })?;

            *state_transaction.world.executor.get_mut() = upgraded_executor;

            state_transaction
                .world
                .emit_events(Some(ExecutorEvent::Upgraded(ExecutorUpgrade {
                    new_data_model: state_transaction.world.executor_data_model.clone(),
                })));

            Ok(())
        }
    }

    impl Execute for Log {
        fn execute(
            self,
            _authority: &AccountId,
            _state_transaction: &mut StateTransaction<'_, '_>,
        ) -> std::result::Result<(), Error> {
            const TARGET: &str = "log_isi";
            let Self { level, msg } = self;

            match level {
                Level::TRACE => iroha_logger::trace!(target: TARGET, "{}", msg),
                Level::DEBUG => iroha_logger::debug!(target: TARGET, "{}", msg),
                Level::INFO => iroha_logger::info!(target: TARGET, "{}", msg),
                Level::WARN => iroha_logger::warn!(target: TARGET, "{}", msg),
                Level::ERROR => iroha_logger::error!(target: TARGET, "{}", msg),
            }

            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use core::num::NonZeroU64;
        use std::{
            collections::{BTreeMap, BTreeSet},
            str::FromStr,
            sync::Arc,
        };

        use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
        #[allow(unused_imports)]
        use iroha_data_model::{
            account::{AccountId, MultisigMember, MultisigPolicy},
            bridge::BridgeReceipt,
            confidential::ConfidentialStatus,
            consensus::{
                ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus,
                HsmBinding,
            },
            events::data::{DataEvent, prelude::BridgeEvent},
            isi::{Grant, consensus_keys, nexus::SetLaneRelayEmergencyValidators, verifying_keys},
            metadata::Metadata,
            nexus::{
                DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, DomainEndorsement,
                DomainEndorsementScope, DomainEndorsementSignature, LaneId,
            },
            permission::Permission,
            proof::{
                ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord,
            },
            query::error::FindError,
            zk::BackendTag,
        };
        use iroha_data_model::{
            isi::{SetParameter, bridge::RecordBridgeReceipt},
            parameter::system::{SumeragiConsensusMode, SumeragiParameter},
            prelude::Parameter,
            zk::OpenVerifyEnvelope,
        };
        use iroha_primitives::json::Json;
        #[allow(unused_imports)]
        use iroha_schema::Ident;
        use iroha_test_samples::{ALICE_ID, gen_account_in};

        use super::*;
        use crate::{
            block::ValidBlock,
            executor::Executor,
            kura::Kura,
            query::store::LiveQueryStore,
            state::{
                State, StateTransaction, SumeragiPolicyConfig, SumeragiPolicyFlags, World,
                storage_transactions::TransactionsBlockError,
            },
            zk::{hash_proof, hash_vk},
        };

        fn new_dummy_block() -> crate::block::CommittedBlock {
            let (leader_public_key, leader_private_key) =
                iroha_crypto::KeyPair::random().into_parts();
            let peer_id = crate::PeerId::new(leader_public_key);
            let topology = crate::sumeragi::network_topology::Topology::new(vec![peer_id]);
            ValidBlock::new_dummy_and_modify_header(&leader_private_key, |h| {
                h.set_height(NonZeroU64::new(1).unwrap());
            })
            .commit(&topology)
            .unpack(|_| {})
            .unwrap()
        }

        fn bootstrap_alice_account(stx: &mut StateTransaction<'_, '_>) {
            let domain_id: DomainId = "wonderland".parse().expect("domain id parses");
            Register::domain(Domain::new(domain_id))
                .execute(&ALICE_ID, stx)
                .expect("register wonderland domain");
            Register::account(Account::new(ALICE_ID.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register ALICE account");
        }

        fn configure_global_dataspace(stx: &mut StateTransaction<'_, '_>) {
            stx.nexus.dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
                id: DataSpaceId::GLOBAL,
                alias: "global".to_string(),
                description: None,
                fault_tolerance: 1,
            }])
            .expect("dataspace catalog");
        }

        fn grant_manage_peers_permission(stx: &mut StateTransaction<'_, '_>, account: &AccountId) {
            let permission = Permission::new("CanManagePeers".into(), Json::new(()));
            stx.world
                .account_permissions
                .insert(account.clone(), BTreeSet::from([permission]));
        }

        fn register_multisig_authority(
            stx: &mut StateTransaction<'_, '_>,
            threshold: u16,
            member_count: usize,
        ) -> AccountId {
            let mut members = Vec::with_capacity(member_count);
            for _ in 0..member_count {
                let kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
                let member = MultisigMember::new(kp.public_key().clone(), 1).expect("member");
                members.push(member);
            }
            let policy = MultisigPolicy::new(threshold, members).expect("multisig policy");
            let domain_id: DomainId = "wonderland".parse().expect("domain id parses");
            let multisig_id = AccountId::new_multisig(domain_id, policy);
            Register::account(Account::new(multisig_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register multisig authority");
            multisig_id
        }

        #[test]
        fn register_role_auto_grants_owner() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            // Bootstrap a domain and the Alice account
            bootstrap_alice_account(&mut stx);

            // Register a role with Alice as the initial owner
            let role_id: RoleId = "TEST_ROLE".parse().unwrap();
            Register::role(Role::new(role_id.clone(), ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            // Verify Alice has been granted the role
            let has_role = stx
                .world
                .account_roles_iter(&ALICE_ID)
                .any(|rid| rid == &role_id);
            assert!(has_role, "owner must be auto‑granted the new role");
        }

        #[test]
        fn register_peer_rejects_non_bls() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            // Enable BLS batching requirement so we exercise the key-type validation.
            let mut pipeline = state.view().pipeline().clone();
            pipeline.signature_batch_max_bls = 1;
            state.set_pipeline(pipeline);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            // Construct a peer with a non-BLS-normal key (Ed25519)
            let kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let peer_id = crate::PeerId::new(kp.public_key().clone());
            let isi =
                iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), vec![]);
            let res = isi.execute(&ALICE_ID, &mut stx);
            assert!(res.is_err(), "non-BLS peer must be rejected");
            // World should not contain the peer
            assert!(stx.world.peers().iter().all(|p| p != &peer_id));
        }

        #[test]
        fn record_bridge_receipt_emits_event() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let receipt = BridgeReceipt {
                lane: LaneId::new(1),
                direction: b"mint".to_vec(),
                source_tx: [0x11; 32],
                dest_tx: None,
                proof_hash: [0x22; 32],
                amount: 1,
                asset_id: b"wBTC#btc".to_vec(),
                recipient: b"alice@main".to_vec(),
            };

            RecordBridgeReceipt::new(receipt.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("record bridge receipt");

            let events = &stx.world.internal_event_buf;
            assert_eq!(events.len(), 1, "expected one emitted event");
            match events[0].as_ref() {
                DataEvent::Bridge(BridgeEvent::Emitted(emitted)) => {
                    assert_eq!(emitted, &receipt);
                }
                other => panic!("unexpected event: {other:?}"),
            }
        }

        #[test]
        fn set_lane_relay_emergency_validators_requires_permission() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            bootstrap_alice_account(&mut stx);
            stx.nexus.enabled = true;
            stx.nexus.lane_relay_emergency.enabled = true;
            configure_global_dataspace(&mut stx);
            let authority = register_multisig_authority(&mut stx, 3, 5);

            let (validator, _) = gen_account_in("wonderland");
            Register::account(Account::new(validator.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register validator");

            let err = SetLaneRelayEmergencyValidators {
                dataspace_id: DataSpaceId::GLOBAL,
                validators: vec![validator],
                expires_at_height: None,
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect_err("permission should be required");
            assert!(matches!(
                err,
                InstructionExecutionError::InvariantViolation(msg)
                    if msg.as_ref() == "not permitted: CanManagePeers"
            ));
            assert!(
                stx.world
                    .lane_relay_emergency_validators
                    .get(&DataSpaceId::GLOBAL)
                    .is_none(),
                "override must not be stored without permission"
            );
        }

        #[test]
        fn set_lane_relay_emergency_validators_rejects_when_nexus_disabled() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            bootstrap_alice_account(&mut stx);
            configure_global_dataspace(&mut stx);
            stx.nexus.lane_relay_emergency.enabled = true;
            let authority = register_multisig_authority(&mut stx, 3, 5);
            grant_manage_peers_permission(&mut stx, &authority);
            stx.nexus.enabled = false;

            let (validator, _) = gen_account_in("wonderland");
            Register::account(Account::new(validator.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register validator");

            let err = SetLaneRelayEmergencyValidators {
                dataspace_id: DataSpaceId::GLOBAL,
                validators: vec![validator],
                expires_at_height: None,
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect_err("nexus disabled should be rejected");
            assert!(matches!(
                err,
                InstructionExecutionError::InvariantViolation(msg)
                    if msg.as_ref() == "lane relay emergency override requires nexus.enabled=true"
            ));
        }

        #[test]
        fn set_lane_relay_emergency_validators_rejects_when_disabled() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            bootstrap_alice_account(&mut stx);
            stx.nexus.enabled = true;
            configure_global_dataspace(&mut stx);
            let authority = register_multisig_authority(&mut stx, 3, 5);
            grant_manage_peers_permission(&mut stx, &authority);

            let (validator, _) = gen_account_in("wonderland");
            Register::account(Account::new(validator.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register validator");

            let err = SetLaneRelayEmergencyValidators {
                dataspace_id: DataSpaceId::GLOBAL,
                validators: vec![validator],
                expires_at_height: None,
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect_err("disabled override should be rejected");
            assert!(matches!(
                err,
                InstructionExecutionError::InvariantViolation(msg)
                    if msg.as_ref()
                        == "lane relay emergency override requires nexus.lane_relay_emergency.enabled=true"
            ));
        }

        #[test]
        fn set_lane_relay_emergency_validators_requires_multisig_authority() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            bootstrap_alice_account(&mut stx);
            stx.nexus.enabled = true;
            stx.nexus.lane_relay_emergency.enabled = true;
            configure_global_dataspace(&mut stx);
            grant_manage_peers_permission(&mut stx, &ALICE_ID);

            let (validator, _) = gen_account_in("wonderland");
            Register::account(Account::new(validator.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register validator");

            let err = SetLaneRelayEmergencyValidators {
                dataspace_id: DataSpaceId::GLOBAL,
                validators: vec![validator],
                expires_at_height: None,
                metadata: Metadata::default(),
            }
            .execute(&ALICE_ID, &mut stx)
            .expect_err("single-signature authority should be rejected");
            assert!(matches!(
                err,
                InstructionExecutionError::InvariantViolation(msg)
                    if msg
                        .as_ref()
                        .starts_with("lane relay emergency override requires multisig authority")
            ));
        }

        #[test]
        fn set_lane_relay_emergency_validators_rejects_unknown_dataspace() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            bootstrap_alice_account(&mut stx);
            stx.nexus.enabled = true;
            stx.nexus.lane_relay_emergency.enabled = true;
            configure_global_dataspace(&mut stx);
            let authority = register_multisig_authority(&mut stx, 3, 5);
            grant_manage_peers_permission(&mut stx, &authority);

            let (validator, _) = gen_account_in("wonderland");
            Register::account(Account::new(validator.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register validator");

            let unknown = DataSpaceId::new(42);
            let err = SetLaneRelayEmergencyValidators {
                dataspace_id: unknown,
                validators: vec![validator],
                expires_at_height: None,
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect_err("unknown dataspace should be rejected");
            let msg = smart_contract_instruction_error_message(err);
            assert!(
                msg.contains("unknown dataspace id"),
                "unexpected error message: {msg}"
            );
        }

        #[test]
        fn set_lane_relay_emergency_validators_rejects_unknown_validator() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            bootstrap_alice_account(&mut stx);
            stx.nexus.enabled = true;
            stx.nexus.lane_relay_emergency.enabled = true;
            configure_global_dataspace(&mut stx);
            let authority = register_multisig_authority(&mut stx, 3, 5);
            grant_manage_peers_permission(&mut stx, &authority);

            let (missing, _) = gen_account_in("wonderland");
            let err = SetLaneRelayEmergencyValidators {
                dataspace_id: DataSpaceId::GLOBAL,
                validators: vec![missing.clone()],
                expires_at_height: None,
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect_err("unknown validator should be rejected");
            assert!(matches!(
                err,
                InstructionExecutionError::Find(FindError::Account(id)) if id == missing
            ));
        }

        #[test]
        fn set_lane_relay_emergency_validators_inserts_and_deduplicates() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            bootstrap_alice_account(&mut stx);
            stx.nexus.enabled = true;
            stx.nexus.lane_relay_emergency.enabled = true;
            configure_global_dataspace(&mut stx);
            let authority = register_multisig_authority(&mut stx, 3, 5);
            grant_manage_peers_permission(&mut stx, &authority);

            let (validator_a, _) = gen_account_in("wonderland");
            let (validator_b, _) = gen_account_in("wonderland");
            Register::account(Account::new(validator_a.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register validator_a");
            Register::account(Account::new(validator_b.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register validator_b");

            SetLaneRelayEmergencyValidators {
                dataspace_id: DataSpaceId::GLOBAL,
                validators: vec![
                    validator_b.clone(),
                    validator_a.clone(),
                    validator_b.clone(),
                ],
                expires_at_height: Some(12),
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect("set emergency validators");

            let record = stx
                .world
                .lane_relay_emergency_validators
                .get(&DataSpaceId::GLOBAL)
                .expect("override stored");
            let mut expected = vec![validator_a, validator_b];
            expected.sort();
            expected.dedup();
            assert_eq!(record.validators, expected);
            assert_eq!(record.expires_at_height, Some(12));
            assert!(record.metadata.is_empty());
        }

        #[test]
        fn set_lane_relay_emergency_validators_clears_on_empty_list() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            bootstrap_alice_account(&mut stx);
            stx.nexus.enabled = true;
            stx.nexus.lane_relay_emergency.enabled = true;
            configure_global_dataspace(&mut stx);
            let authority = register_multisig_authority(&mut stx, 3, 5);
            grant_manage_peers_permission(&mut stx, &authority);

            let (validator, _) = gen_account_in("wonderland");
            Register::account(Account::new(validator.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register validator");

            SetLaneRelayEmergencyValidators {
                dataspace_id: DataSpaceId::GLOBAL,
                validators: vec![validator.clone()],
                expires_at_height: None,
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect("set emergency validators");
            assert!(
                stx.world
                    .lane_relay_emergency_validators
                    .get(&DataSpaceId::GLOBAL)
                    .is_some(),
                "override should be stored before clearing"
            );

            SetLaneRelayEmergencyValidators {
                dataspace_id: DataSpaceId::GLOBAL,
                validators: Vec::new(),
                expires_at_height: None,
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect("clear emergency validators");
            assert!(
                stx.world
                    .lane_relay_emergency_validators
                    .get(&DataSpaceId::GLOBAL)
                    .is_none(),
                "override should be removed when validators list is empty"
            );
        }

        fn smart_contract_instruction_error_message(err: InstructionExecutionError) -> String {
            match err {
                InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(msg),
                ) => msg,
                other => panic!("unexpected error: {other:?}"),
            }
        }

        fn smart_contract_error_message(err: ValidationFail) -> String {
            match err {
                ValidationFail::InstructionFailed(inner) => {
                    smart_contract_instruction_error_message(inner)
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }

        #[test]
        fn register_domain_rejects_labels_failing_norm_v1() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let domain_id: DomainId = "wÍḷd-card".parse().expect("domain id parses");
            let err = Register::domain(Domain::new(domain_id))
                .execute(&ALICE_ID, &mut stx)
                .expect_err("label violating STD3 must be rejected");
            let msg = smart_contract_instruction_error_message(err);
            assert!(
                msg.contains("normalization"),
                "unexpected error message: {msg}"
            );
        }

        #[test]
        fn register_domain_accepts_idn_labels() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let domain_id: DomainId = "例え.テスト".parse().expect("IDN label parses");
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("IDN domain must be accepted");
            let canonical_label =
                name::canonicalize_domain_label("例え.テスト").expect("canonical label");
            let canonical_id: DomainId =
                canonical_label.parse().expect("canonical domain id parses");
            assert!(stx.world.domain(&canonical_id).is_ok());
        }

        #[test]
        fn register_domain_requires_endorsement_when_configured() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            stx.nexus.enabled = true;

            let kp = KeyPair::random();
            stx.nexus.endorsement.quorum = 1;
            stx.nexus
                .endorsement
                .committee_keys
                .push(kp.public_key().to_string());
            let endorsement_key_id =
                ConsensusKeyId::new(ConsensusKeyRole::Endorsement, "committee-key");
            let endorsement_record = ConsensusKeyRecord {
                id: endorsement_key_id.clone(),
                public_key: kp.public_key().clone(),
                activation_height: stx.block_height(),
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            stx.world
                .consensus_keys
                .insert(endorsement_key_id.clone(), endorsement_record);
            stx.world
                .consensus_keys_by_pk
                .insert(kp.public_key().to_string(), vec![endorsement_key_id]);

            let domain_id: DomainId = "endorsed".parse().expect("domain id parses");
            let mut new_domain = Domain::new(domain_id.clone());

            let canonical_label =
                name::canonicalize_domain_label(domain_id.name.as_ref()).expect("canonical");
            let canonical_id: DomainId = canonical_label.parse().expect("canonical domain");
            let statement_hash = Hash::new(canonical_id.to_string().as_bytes());
            let mut endorsement = DomainEndorsement {
                version: iroha_data_model::nexus::DOMAIN_ENDORSEMENT_VERSION_V1,
                domain_id: canonical_id.clone(),
                committee_id: "default".into(),
                statement_hash,
                issued_at_height: stx.block_height(),
                expires_at_height: stx.block_height() + 10,
                scope: DomainEndorsementScope::default(),
                signatures: Vec::new(),
                metadata: Metadata::default(),
            };
            let msg_hash = endorsement.body_hash();
            endorsement.signatures.push(DomainEndorsementSignature {
                signer: kp.public_key().clone(),
                signature: Signature::new(kp.private_key(), msg_hash.as_ref()),
            });
            new_domain.metadata.insert(
                Name::from_str("endorsement").expect("name"),
                Json::new(endorsement),
            );

            Register::domain(new_domain)
                .execute(&ALICE_ID, &mut stx)
                .expect("endorsed domain registers");
            assert!(stx.world.domain(&canonical_id).is_ok());
            let record = stx
                .world
                .domain_endorsements
                .get(&msg_hash)
                .expect("recorded endorsement");
            assert_eq!(record.accepted_at_height, stx.block_height());
        }

        #[test]
        fn register_domain_rejects_missing_endorsement_when_quorum_set() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            stx.nexus.enabled = true;
            stx.nexus.endorsement.quorum = 1;
            let kp = KeyPair::random();
            stx.nexus
                .endorsement
                .committee_keys
                .push(kp.public_key().to_string());
            let endorsement_key_id =
                ConsensusKeyId::new(ConsensusKeyRole::Endorsement, "committee-key");
            let endorsement_record = ConsensusKeyRecord {
                id: endorsement_key_id.clone(),
                public_key: kp.public_key().clone(),
                activation_height: stx.block_height(),
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            stx.world
                .consensus_keys
                .insert(endorsement_key_id.clone(), endorsement_record);
            stx.world
                .consensus_keys_by_pk
                .insert(kp.public_key().to_string(), vec![endorsement_key_id]);

            let domain_id: DomainId = "endorse-missing".parse().expect("domain id parses");
            let res = Register::domain(Domain::new(domain_id)).execute(&ALICE_ID, &mut stx);
            assert!(res.is_err(), "missing endorsement must be rejected");
        }

        #[test]
        fn register_domain_rejects_expired_endorsement() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            stx.nexus.enabled = true;

            let kp = KeyPair::random();
            stx.nexus.endorsement.quorum = 1;
            stx.nexus
                .endorsement
                .committee_keys
                .push(kp.public_key().to_string());
            let endorsement_key_id =
                ConsensusKeyId::new(ConsensusKeyRole::Endorsement, "committee-key");
            let endorsement_record = ConsensusKeyRecord {
                id: endorsement_key_id.clone(),
                public_key: kp.public_key().clone(),
                activation_height: stx.block_height(),
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            stx.world
                .consensus_keys
                .insert(endorsement_key_id.clone(), endorsement_record);
            stx.world
                .consensus_keys_by_pk
                .insert(kp.public_key().to_string(), vec![endorsement_key_id]);

            let domain_id: DomainId = "endorse-expired".parse().expect("domain id parses");
            let mut new_domain = Domain::new(domain_id.clone());
            let canonical_label =
                name::canonicalize_domain_label(domain_id.name.as_ref()).expect("canonical");
            let canonical_id: DomainId = canonical_label.parse().expect("canonical domain");
            let statement_hash = Hash::new(canonical_id.to_string().as_bytes());
            let mut endorsement = DomainEndorsement {
                version: iroha_data_model::nexus::DOMAIN_ENDORSEMENT_VERSION_V1,
                domain_id: canonical_id.clone(),
                committee_id: "default".into(),
                statement_hash,
                issued_at_height: stx.block_height().saturating_sub(5),
                expires_at_height: stx.block_height().saturating_sub(1),
                scope: DomainEndorsementScope::default(),
                signatures: Vec::new(),
                metadata: Metadata::default(),
            };
            let msg_hash = endorsement.body_hash();
            endorsement.signatures.push(DomainEndorsementSignature {
                signer: kp.public_key().clone(),
                signature: Signature::new(kp.private_key(), msg_hash.as_ref()),
            });
            new_domain.metadata.insert(
                Name::from_str("endorsement").expect("name"),
                Json::new(endorsement),
            );

            let res = Register::domain(new_domain).execute(&ALICE_ID, &mut stx);
            assert!(res.is_err(), "expired endorsement must be rejected");
        }

        #[test]
        fn register_peer_validates() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.signature_batch_max_bls = 4;
            state.set_pipeline(pipeline);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            // BLS-normal key with valid PoP
            let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer_id = crate::PeerId::new(bls.public_key().clone());
            let pop = iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("pop");
            let isi =
                iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), pop);
            isi.execute(&ALICE_ID, &mut stx)
                .expect("register with valid pop");
            assert!(stx.world.peers().iter().any(|p| p == &peer_id));

            // Mismatched PoP for another key must fail
            let other = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let bad_pop = iroha_crypto::bls_normal_pop_prove(other.private_key()).expect("pop");
            let isi_bad =
                iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), bad_pop);
            let res = isi_bad.execute(&ALICE_ID, &mut stx);
            assert!(res.is_err(), "invalid PoP must be rejected");
        }

        #[test]
        fn register_peer_applies_key_policy_defaults() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.signature_batch_max_bls = 4;
            state.set_pipeline(pipeline);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer_id = crate::PeerId::new(bls.public_key().clone());
            let pop = iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("pop");
            let isi =
                iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), pop);
            isi.execute(&ALICE_ID, &mut stx)
                .expect("register peer with policy enforcement");

            let pk_label = peer_id.public_key().to_string();
            let ids = stx
                .world
                .consensus_keys_by_pk
                .get(&pk_label)
                .cloned()
                .expect("consensus key index");
            assert_eq!(ids.len(), 1, "validator key id must be indexed by pk");
            let record = stx
                .world
                .consensus_keys
                .get(&ids[0])
                .expect("consensus key record");
            let params = stx.world.parameters.get();
            let expected_activation = stx
                .block_height()
                .saturating_add(params.sumeragi.key_activation_lead_blocks);
            assert_eq!(
                record.activation_height, expected_activation,
                "activation height must obey lead-time policy"
            );
            assert_eq!(
                record.status,
                ConsensusKeyStatus::Pending,
                "lead-time activation should mark the key pending"
            );
            assert!(
                record.hsm.is_some(),
                "HSM binding must be populated when key_require_hsm is enabled"
            );
        }

        #[test]
        fn register_peer_rejects_id_collision() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.signature_batch_max_bls = 4;
            state.set_pipeline(pipeline);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer_id = crate::PeerId::new(bls.public_key().clone());
            let pop = iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("pop");

            // Seed a conflicting consensus key record with the same derived id but a different pk.
            let collision_id = crate::state::derive_validator_key_id(peer_id.public_key());
            let other = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let bogus = ConsensusKeyRecord {
                id: collision_id.clone(),
                public_key: other.public_key().clone(),
                activation_height: stx.block_height(),
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            stx.world
                .consensus_keys
                .insert(collision_id.clone(), bogus.clone());
            stx.world
                .consensus_keys_by_pk
                .insert(other.public_key().to_string(), vec![collision_id.clone()]);

            let isi =
                iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), pop);
            let err = isi
                .execute(&ALICE_ID, &mut stx)
                .expect_err("id collision must reject peer registration");
            let msg = smart_contract_instruction_error_message(err);
            assert!(
                msg.contains("collision"),
                "unexpected error message for id collision: {msg}"
            );
            // Verify we did not add the peer
            assert!(stx.world.peers().iter().all(|p| p != &peer_id));
            // Original bogus record remains untouched
            let stored = stx
                .world
                .consensus_keys
                .get(&collision_id)
                .expect("existing record");
            assert_eq!(stored.public_key, bogus.public_key);
        }

        #[test]
        fn register_peer_small_validates() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.signature_batch_max_bls = 4;
            state.set_pipeline(pipeline);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let bls_small = KeyPair::random_with_algorithm(Algorithm::BlsSmall);
            let peer_id = crate::PeerId::new(bls_small.public_key().clone());
            let pop = iroha_crypto::bls_small_pop_prove(bls_small.private_key()).expect("pop");
            let isi =
                iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), pop);
            let res = isi.execute(&ALICE_ID, &mut stx);
            assert!(
                res.is_err(),
                "BLS-small peer registration should be rejected for consensus"
            );
            assert!(stx.world.peers().iter().all(|p| p != &peer_id));
        }

        #[test]
        fn register_peer_small_rejected_even_with_batching() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.signature_batch_max_bls = 16; // ensure batching enabled
            state.set_pipeline(pipeline);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let bls_small = KeyPair::random_with_algorithm(Algorithm::BlsSmall);
            let peer_id = crate::PeerId::new(bls_small.public_key().clone());
            let pop = iroha_crypto::bls_small_pop_prove(bls_small.private_key()).expect("pop");
            let isi =
                iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), pop);
            let res = isi.execute(&ALICE_ID, &mut stx);
            assert!(
                res.is_err(),
                "BLS-small peer registration must be rejected even when batching is enabled"
            );
            assert!(stx.world.peers().iter().all(|p| p != &peer_id));
        }

        #[test]
        fn register_peer_fails_when_bls_batch_disabled() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.signature_batch_max_bls = 0;
            state.set_pipeline(pipeline);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer_id = crate::PeerId::new(bls.public_key().clone());
            let pop = iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("pop");
            let isi =
                iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), pop);
            let err = isi
                .execute(&ALICE_ID, &mut stx)
                .expect_err("batching disabled must reject peer registration");
            let msg = smart_contract_instruction_error_message(err);
            assert!(
                msg.contains("signature_batch_max_bls"),
                "unexpected error message: {msg}"
            );
        }

        #[test]
        fn register_peer_requires_hsm_binding_when_policy_enabled() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.signature_batch_max_bls = 4;
            state.set_pipeline(pipeline);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            {
                let mut stx = state_block.transaction();
                let params = stx.world.parameters.get_mut();
                params.sumeragi.key_require_hsm = true;
                params.sumeragi.key_allowed_hsm_providers = vec!["softkey".into()];
                stx.apply();
            }

            crate::sumeragi::status::reset_peer_key_policy_counters_for_tests();

            let bls_missing = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer_id_missing = crate::PeerId::new(bls_missing.public_key().clone());
            {
                let mut stx = state_block.transaction();
                let pop_missing =
                    iroha_crypto::bls_normal_pop_prove(bls_missing.private_key()).expect("pop");
                let isi_missing = iroha_data_model::isi::register::RegisterPeerWithPop::new(
                    peer_id_missing.clone(),
                    pop_missing,
                );
                let err = isi_missing
                    .execute(&ALICE_ID, &mut stx)
                    .expect_err("missing HSM binding must be rejected");
                let msg = smart_contract_instruction_error_message(err);
                assert!(
                    msg.contains("HSM binding required"),
                    "unexpected error: {msg}"
                );
                assert!(stx.world.peers().iter().all(|p| p != &peer_id_missing));
                let snapshot = crate::sumeragi::status::snapshot().peer_key_policy;
                assert_eq!(snapshot.missing_hsm_total, 1);
            }

            let bls_bound = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer_id_bound = crate::PeerId::new(bls_bound.public_key().clone());
            let mut stx = state_block.transaction();
            let pop_bound =
                iroha_crypto::bls_normal_pop_prove(bls_bound.private_key()).expect("pop");
            let binding = iroha_data_model::consensus::HsmBinding {
                provider: "softkey".into(),
                key_label: peer_id_bound.public_key().to_string(),
                slot: Some(1),
            };
            let isi = iroha_data_model::isi::register::RegisterPeerWithPop::new(
                peer_id_bound.clone(),
                pop_bound,
            )
            .with_hsm(binding);
            isi.execute(&ALICE_ID, &mut stx)
                .expect("HSM-bound peer registration should succeed");
            assert!(stx.world.peers().iter().any(|p| p == &peer_id_bound));
        }

        #[test]
        fn register_peer_rejects_activation_before_lead_time() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.signature_batch_max_bls = 4;
            state.set_pipeline(pipeline);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            {
                let mut stx = state_block.transaction();
                let params = stx.world.parameters.get_mut();
                params.sumeragi.key_activation_lead_blocks = 2;
                stx.apply();
            }

            crate::sumeragi::status::reset_peer_key_policy_counters_for_tests();

            let mut stx = state_block.transaction();
            let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer_id = crate::PeerId::new(bls.public_key().clone());
            let pop = iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("pop");
            let isi =
                iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), pop)
                    .with_activation_at(stx.block_height());
            let err = isi
                .execute(&ALICE_ID, &mut stx)
                .expect_err("lead-time violation must be rejected");
            let msg = smart_contract_instruction_error_message(err);
            assert!(msg.contains("lead-time policy"), "unexpected error: {msg}");
            assert!(stx.world.peers().iter().all(|p| p != &peer_id));
            let snapshot = crate::sumeragi::status::snapshot().peer_key_policy;
            assert!(snapshot.lead_time_violation_total > 0);
        }

        #[test]
        fn register_peer_rejects_identifier_collisions() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.signature_batch_max_bls = 4;
            state.set_pipeline(pipeline);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer_id = crate::PeerId::new(bls.public_key().clone());
            let pop = iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("pop");

            {
                let mut stx = state_block.transaction();
                let existing_id =
                    ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-existing");
                let existing_record = ConsensusKeyRecord {
                    id: existing_id.clone(),
                    public_key: peer_id.public_key().clone(),
                    activation_height: stx.block_height(),
                    expiry_height: None,
                    hsm: None,
                    replaces: None,
                    status: ConsensusKeyStatus::Active,
                };
                stx.world
                    .consensus_keys
                    .insert(existing_id.clone(), existing_record);
                stx.world
                    .consensus_keys_by_pk
                    .insert(peer_id.public_key().to_string(), vec![existing_id]);
                stx.apply();
            }

            crate::sumeragi::status::reset_peer_key_policy_counters_for_tests();

            let mut stx = state_block.transaction();
            let isi =
                iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), pop);
            let err = isi
                .execute(&ALICE_ID, &mut stx)
                .expect_err("identifier collisions must be rejected");
            let msg = smart_contract_instruction_error_message(err);
            assert!(msg.contains("collision"), "unexpected error: {msg}");
            let snapshot = crate::sumeragi::status::snapshot().peer_key_policy;
            assert_eq!(snapshot.identifier_collision_total, 1);
        }

        #[test]
        fn register_vk_requires_gas_schedule() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            bootstrap_alice_account(&mut stx);
            let perm = Permission::new(
                "CanManageVerifyingKeys".parse().unwrap(),
                iroha_primitives::json::Json::new(()),
            );
            Grant::account_permission(perm, ALICE_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant manage vk");
            stx.apply();

            let mut stx = state_block.transaction();
            let exec = Executor::default();
            let id = VerifyingKeyId::new("halo2/ipa", "vk_missing_gas");
            let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2, 3]);
            let mut rec = VerifyingKeyRecord::new_with_owner(
                1,
                "vk_missing_gas",
                None,
                "test",
                BackendTag::Halo2IpaPasta,
                "pallas",
                [0x61; 32],
                hash_vk(&vk_box),
            );
            rec.vk_len = 3;
            rec.status = ConfidentialStatus::Active;
            rec.key = Some(vk_box);
            let instr: InstructionBox =
                verifying_keys::RegisterVerifyingKey { id, record: rec }.into();
            let err = exec
                .execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
                .expect_err("missing gas_schedule_id must fail");
            let msg = smart_contract_error_message(err);
            assert!(msg.contains("gas_schedule_id"), "unexpected msg: {msg}");
        }

        #[test]
        fn register_vk_rejects_non_ipa_backend() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            bootstrap_alice_account(&mut stx);
            let perm = Permission::new(
                "CanManageVerifyingKeys".parse().unwrap(),
                iroha_primitives::json::Json::new(()),
            );
            Grant::account_permission(perm, ALICE_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant manage vk");
            stx.apply();

            let mut stx = state_block.transaction();
            let exec = Executor::default();
            let id = VerifyingKeyId::new("halo2/bn254", "vk_bad_backend");
            let vk_box = VerifyingKeyBox::new("halo2/bn254".into(), vec![1, 2, 3]);
            let mut rec = VerifyingKeyRecord::new_with_owner(
                1,
                "vk_bad_backend",
                None,
                "test",
                BackendTag::Halo2Bn254,
                "bn254",
                [0x41; 32],
                hash_vk(&vk_box),
            );
            rec.vk_len = 3;
            rec.status = ConfidentialStatus::Active;
            rec.key = Some(vk_box);
            rec.gas_schedule_id = Some("halo2_default".into());
            let instr: InstructionBox =
                verifying_keys::RegisterVerifyingKey { id, record: rec }.into();
            let err = exec
                .execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
                .expect_err("non-IPA backend must be rejected");
            let msg = smart_contract_error_message(err);
            assert!(
                msg.contains("backend must be Halo2IpaPasta"),
                "unexpected msg: {msg}"
            );
        }

        #[test]
        fn register_consensus_key_enforces_policy() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let params = {
                let mut stx = state_block.transaction();
                bootstrap_alice_account(&mut stx);
                let perm = Permission::new(
                    "CanManageConsensusKeys".parse().unwrap(),
                    iroha_primitives::json::Json::new(()),
                );
                Grant::account_permission(perm, ALICE_ID.clone())
                    .execute(&ALICE_ID, &mut stx)
                    .expect("grant manage consensus keys");
                let params = stx.world.parameters.get().sumeragi.clone();
                assert!(
                    params
                        .key_allowed_algorithms
                        .contains(&Algorithm::BlsNormal),
                    "default consensus key allow-list should include BLS-Normal"
                );
                stx.apply();
                params
            };

            let kp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-primary");
            let pk = kp.public_key().clone();
            let hsm = HsmBinding {
                provider: "pkcs11".to_string(),
                key_label: "validator/0".to_string(),
                slot: Some(1),
            };
            let make_record = |activation_height: u64| ConsensusKeyRecord {
                id: id.clone(),
                public_key: pk.clone(),
                activation_height,
                expiry_height: None,
                hsm: Some(hsm.clone()),
                replaces: None,
                status: ConsensusKeyStatus::Pending,
            };
            let exec = Executor::default();

            // Lead-time violation should be rejected.
            {
                let mut stx = state_block.transaction();
                let bad_record = make_record(stx.block_height());
                let bad_instr: InstructionBox = consensus_keys::RegisterConsensusKey {
                    id: id.clone(),
                    record: bad_record,
                }
                .into();
                let err = exec
                    .execute_instruction(&mut stx, &ALICE_ID.clone(), bad_instr)
                    .expect_err("lead time guard must reject");
                let msg = smart_contract_error_message(err);
                assert!(
                    msg.contains("lead-time"),
                    "expected lead-time rejection, got {msg}"
                );
            }

            // Valid registration succeeds.
            {
                let mut stx = state_block.transaction();
                let ok_record = make_record(
                    stx.block_height()
                        .saturating_add(params.key_activation_lead_blocks),
                );
                let ok_instr: InstructionBox = consensus_keys::RegisterConsensusKey {
                    id: id.clone(),
                    record: ok_record.clone(),
                }
                .into();
                exec.execute_instruction(&mut stx, &ALICE_ID.clone(), ok_instr)
                    .expect("register consensus key");
                let stored = stx
                    .world
                    .consensus_keys
                    .get(&id)
                    .expect("consensus key stored");
                assert_eq!(stored.status, ok_record.status);
                assert!(
                    stx.world
                        .consensus_keys_by_pk
                        .get(&ok_record.public_key.to_string())
                        .is_some()
                );
            }
        }

        #[test]
        fn register_consensus_key_requires_hsm_when_configured() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let params = {
                let mut stx = state_block.transaction();
                bootstrap_alice_account(&mut stx);
                let perm = Permission::new(
                    "CanManageConsensusKeys".parse().unwrap(),
                    iroha_primitives::json::Json::new(()),
                );
                Grant::account_permission(perm, ALICE_ID.clone())
                    .execute(&ALICE_ID, &mut stx)
                    .expect("grant manage consensus keys");
                let params = stx.world.parameters.get().sumeragi.clone();
                stx.apply();
                params
            };

            let mut stx = state_block.transaction();
            let kp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-hsm");
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: kp.public_key().clone(),
                activation_height: stx
                    .block_height()
                    .saturating_add(params.key_activation_lead_blocks),
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Pending,
            };
            let exec = Executor::default();
            let instr: InstructionBox = consensus_keys::RegisterConsensusKey {
                id: id.clone(),
                record,
            }
            .into();
            let err = exec
                .execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
                .expect_err("HSM-required policy must reject missing binding");
            let msg = smart_contract_error_message(err);
            assert!(
                msg.contains("HSM binding required"),
                "unexpected msg: {msg}"
            );
        }

        #[test]
        fn register_consensus_key_rejects_disallowed_algorithm() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let params = {
                let mut stx = state_block.transaction();
                bootstrap_alice_account(&mut stx);
                let perm = Permission::new(
                    "CanManageConsensusKeys".parse().unwrap(),
                    iroha_primitives::json::Json::new(()),
                );
                Grant::account_permission(perm, ALICE_ID.clone())
                    .execute(&ALICE_ID, &mut stx)
                    .expect("grant manage consensus keys");
                let params = stx.world.parameters.get().sumeragi.clone();
                stx.apply();
                params
            };

            let mut stx = state_block.transaction();
            let kp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-ed25519");
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: kp.public_key().clone(),
                activation_height: stx
                    .block_height()
                    .saturating_add(params.key_activation_lead_blocks),
                expiry_height: None,
                hsm: Some(HsmBinding {
                    provider: "pkcs11".into(),
                    key_label: "validator/ed25519".into(),
                    slot: None,
                }),
                replaces: None,
                status: ConsensusKeyStatus::Pending,
            };
            let exec = Executor::default();
            let instr: InstructionBox = consensus_keys::RegisterConsensusKey {
                id: id.clone(),
                record,
            }
            .into();
            let err = exec
                .execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
                .expect_err("disallowed algorithm must be rejected");
            let msg = smart_contract_error_message(err);
            assert_eq!(
                msg,
                "consensus key algorithm ed25519 is not allowed; allowed: [bls_normal]"
            );
        }

        #[test]
        fn register_consensus_key_records_lifecycle_history() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            crate::sumeragi::status::reset_consensus_keys_for_tests();
            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let params = {
                let mut stx = state_block.transaction();
                bootstrap_alice_account(&mut stx);
                let perm = Permission::new(
                    "CanManageConsensusKeys".parse().unwrap(),
                    iroha_primitives::json::Json::new(()),
                );
                Grant::account_permission(perm, ALICE_ID.clone())
                    .execute(&ALICE_ID, &mut stx)
                    .expect("grant manage consensus keys");
                let params = stx.world.parameters.get().sumeragi.clone();
                stx.apply();
                params
            };

            let mut stx = state_block.transaction();
            let kp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-history");
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: kp.public_key().clone(),
                activation_height: stx
                    .block_height()
                    .saturating_add(params.key_activation_lead_blocks),
                expiry_height: None,
                hsm: Some(HsmBinding {
                    provider: "pkcs11".into(),
                    key_label: "validator/history".into(),
                    slot: None,
                }),
                replaces: None,
                status: ConsensusKeyStatus::Pending,
            };
            let exec = Executor::default();
            let instr: InstructionBox = consensus_keys::RegisterConsensusKey {
                id: id.clone(),
                record,
            }
            .into();
            exec.execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
                .expect("register consensus key");
            let history = crate::sumeragi::status::consensus_key_history();
            assert!(
                history.iter().any(|rec| rec.id == id),
                "expected lifecycle history to include registered key"
            );
        }

        #[test]
        #[allow(clippy::too_many_lines)]
        fn register_consensus_key_respects_config_allowlist_and_hsm_flag() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);

            let sumeragi_cfg = SumeragiPolicyConfig {
                collectors_k: iroha_config::parameters::defaults::sumeragi::COLLECTORS_K,
                collectors_redundant_send_r:
                    iroha_config::parameters::defaults::sumeragi::COLLECTORS_REDUNDANT_SEND_R,
                policy_flags: SumeragiPolicyFlags::new(
                    iroha_config::parameters::defaults::sumeragi::DA_ENABLED,
                    false,
                ),
                key_activation_lead_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_ACTIVATION_LEAD_BLOCKS,
                key_overlap_grace_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_OVERLAP_GRACE_BLOCKS,
                key_expiry_grace_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_EXPIRY_GRACE_BLOCKS,
                key_allowed_algorithms: [Algorithm::BlsNormal].into_iter().collect(),
                key_allowed_hsm_providers: ["softkey".to_owned()].into_iter().collect(),
            };
            state.set_sumeragi_parameters(sumeragi_cfg.clone());

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let params = {
                let mut stx = state_block.transaction();
                bootstrap_alice_account(&mut stx);
                let perm = Permission::new(
                    "CanManageConsensusKeys".parse().unwrap(),
                    iroha_primitives::json::Json::new(()),
                );
                Grant::account_permission(perm, ALICE_ID.clone())
                    .execute(&ALICE_ID, &mut stx)
                    .expect("grant manage consensus keys");
                let params = stx.world.parameters.get().sumeragi.clone();
                stx.apply();
                params
            };

            let exec = Executor::default();

            // BLS is allowed and does not require an HSM binding once the config is applied.
            {
                let mut stx = state_block.transaction();
                let kp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
                let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-bls-ok");
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: kp.public_key().clone(),
                    activation_height: stx
                        .block_height()
                        .saturating_add(params.key_activation_lead_blocks),
                    expiry_height: None,
                    hsm: None,
                    replaces: None,
                    status: ConsensusKeyStatus::Pending,
                };
                let instr: InstructionBox = consensus_keys::RegisterConsensusKey {
                    id: id.clone(),
                    record,
                }
                .into();
                exec.execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
                    .expect("bls consensus key should be accepted without HSM");
            }

            // Ed25519 is filtered out by the config allowlist.
            {
                let mut stx = state_block.transaction();
                let kp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::Ed25519);
                let id =
                    ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-ed25519-reject");
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: kp.public_key().clone(),
                    activation_height: stx
                        .block_height()
                        .saturating_add(params.key_activation_lead_blocks),
                    expiry_height: None,
                    hsm: Some(HsmBinding {
                        provider: "softkey".into(),
                        key_label: "validator/bls".into(),
                        slot: None,
                    }),
                    replaces: None,
                    status: ConsensusKeyStatus::Pending,
                };
                let instr: InstructionBox = consensus_keys::RegisterConsensusKey {
                    id: id.clone(),
                    record,
                }
                .into();
                let err = exec
                    .execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
                    .expect_err("disallowed algorithm must be rejected");
                let msg = smart_contract_error_message(err);
                assert_eq!(
                    msg,
                    "consensus key algorithm ed25519 is not allowed; allowed: [bls_normal]"
                );
            }

            // Provider outside the allowlist is rejected even when the algorithm is permitted.
            {
                let mut stx = state_block.transaction();
                let kp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
                let id =
                    ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-provider-reject");
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: kp.public_key().clone(),
                    activation_height: stx
                        .block_height()
                        .saturating_add(params.key_activation_lead_blocks),
                    expiry_height: None,
                    hsm: Some(HsmBinding {
                        provider: "pkcs11".into(),
                        key_label: "validator/provider".into(),
                        slot: None,
                    }),
                    replaces: None,
                    status: ConsensusKeyStatus::Pending,
                };
                let instr: InstructionBox = consensus_keys::RegisterConsensusKey {
                    id: id.clone(),
                    record,
                }
                .into();
                let err = exec
                    .execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
                    .expect_err("provider not on allowlist must be rejected");
                let msg = smart_contract_error_message(err);
                assert_eq!(
                    msg,
                    "HSM provider pkcs11 is not allowed; allowed providers: [softkey]"
                );
            }
        }

        #[test]
        fn rotate_consensus_key_requires_hsm_when_configured() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            crate::sumeragi::status::reset_consensus_keys_for_tests();
            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let params = {
                let mut stx = state_block.transaction();
                bootstrap_alice_account(&mut stx);
                let perm = Permission::new(
                    "CanManageConsensusKeys".parse().unwrap(),
                    iroha_primitives::json::Json::new(()),
                );
                Grant::account_permission(perm, ALICE_ID.clone())
                    .execute(&ALICE_ID, &mut stx)
                    .expect("grant manage consensus keys");
                let params = stx.world.parameters.get().sumeragi.clone();
                stx.apply();
                params
            };

            let mut stx = state_block.transaction();
            let kp_a = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let id_a = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-hsm-required");
            let record_a = ConsensusKeyRecord {
                id: id_a.clone(),
                public_key: kp_a.public_key().clone(),
                activation_height: stx
                    .block_height()
                    .saturating_add(params.key_activation_lead_blocks),
                expiry_height: None,
                hsm: Some(HsmBinding {
                    provider: "pkcs11".into(),
                    key_label: "validator/a".into(),
                    slot: Some(0),
                }),
                replaces: None,
                status: ConsensusKeyStatus::Pending,
            };
            let exec = Executor::default();
            let instr_a: InstructionBox = consensus_keys::RegisterConsensusKey {
                id: id_a.clone(),
                record: record_a,
            }
            .into();
            exec.execute_instruction(&mut stx, &ALICE_ID.clone(), instr_a)
                .expect("register initial consensus key");
            stx.apply();

            let mut stx = state_block.transaction();
            let kp_b = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let id_b = ConsensusKeyId::new(
                ConsensusKeyRole::Validator,
                "validator-hsm-missing-rotation",
            );
            let record_b = ConsensusKeyRecord {
                id: id_b.clone(),
                public_key: kp_b.public_key().clone(),
                activation_height: stx
                    .block_height()
                    .saturating_add(params.key_activation_lead_blocks + 1),
                expiry_height: None,
                hsm: None,
                replaces: Some(id_a.clone()),
                status: ConsensusKeyStatus::Pending,
            };
            let instr_b: InstructionBox = consensus_keys::RotateConsensusKey {
                id: id_b.clone(),
                record: record_b,
            }
            .into();
            let err = exec
                .execute_instruction(&mut stx, &ALICE_ID.clone(), instr_b)
                .expect_err("rotation without HSM must be rejected when required");
            let msg = smart_contract_error_message(err);
            assert!(
                msg.contains("HSM binding required"),
                "unexpected error: {msg}"
            );
        }

        #[test]
        #[allow(clippy::too_many_lines)]
        fn rotate_consensus_key_allows_missing_hsm_when_optional() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);

            let sumeragi_cfg = SumeragiPolicyConfig {
                collectors_k: iroha_config::parameters::defaults::sumeragi::COLLECTORS_K,
                collectors_redundant_send_r:
                    iroha_config::parameters::defaults::sumeragi::COLLECTORS_REDUNDANT_SEND_R,
                policy_flags: SumeragiPolicyFlags::new(
                    iroha_config::parameters::defaults::sumeragi::DA_ENABLED,
                    false,
                ),
                key_activation_lead_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_ACTIVATION_LEAD_BLOCKS,
                key_overlap_grace_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_OVERLAP_GRACE_BLOCKS,
                key_expiry_grace_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_EXPIRY_GRACE_BLOCKS,
                key_allowed_algorithms: [Algorithm::BlsNormal].into_iter().collect(),
                key_allowed_hsm_providers: ["pkcs11".to_owned()].into_iter().collect(),
            };
            state.set_sumeragi_parameters(sumeragi_cfg.clone());

            crate::sumeragi::status::reset_consensus_keys_for_tests();
            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let params = {
                let mut stx = state_block.transaction();
                bootstrap_alice_account(&mut stx);
                let perm = Permission::new(
                    "CanManageConsensusKeys".parse().unwrap(),
                    iroha_primitives::json::Json::new(()),
                );
                Grant::account_permission(perm, ALICE_ID.clone())
                    .execute(&ALICE_ID, &mut stx)
                    .expect("grant manage consensus keys");
                let params = stx.world.parameters.get().sumeragi.clone();
                stx.apply();
                params
            };

            let mut stx = state_block.transaction();
            let kp_a = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let id_a = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-hsm-optional-a");
            let record_a = ConsensusKeyRecord {
                id: id_a.clone(),
                public_key: kp_a.public_key().clone(),
                activation_height: stx
                    .block_height()
                    .saturating_add(params.key_activation_lead_blocks),
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Pending,
            };
            let exec = Executor::default();
            let instr_a: InstructionBox = consensus_keys::RegisterConsensusKey {
                id: id_a.clone(),
                record: record_a.clone(),
            }
            .into();
            exec.execute_instruction(&mut stx, &ALICE_ID.clone(), instr_a)
                .expect("register initial key without HSM when optional");
            stx.apply();

            let mut stx = state_block.transaction();
            let kp_b = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let id_b = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-hsm-optional-b");
            let record_b = ConsensusKeyRecord {
                id: id_b.clone(),
                public_key: kp_b.public_key().clone(),
                activation_height: stx
                    .block_height()
                    .saturating_add(params.key_activation_lead_blocks + 2),
                expiry_height: None,
                hsm: None,
                replaces: Some(id_a.clone()),
                status: ConsensusKeyStatus::Pending,
            };
            let instr_b: InstructionBox = consensus_keys::RotateConsensusKey {
                id: id_b.clone(),
                record: record_b.clone(),
            }
            .into();
            exec.execute_instruction(&mut stx, &ALICE_ID.clone(), instr_b)
                .expect("rotation without HSM should succeed when optional");

            let prev = stx
                .world
                .consensus_keys
                .get(&id_a)
                .expect("previous key stored");
            let next = stx
                .world
                .consensus_keys
                .get(&id_b)
                .expect("rotated key stored");
            assert_eq!(prev.status, ConsensusKeyStatus::Retiring);
            assert_eq!(next.status, ConsensusKeyStatus::Pending);
            assert!(
                crate::sumeragi::status::consensus_key_history()
                    .iter()
                    .any(|entry| entry.id == id_b && entry.hsm.is_none()),
                "lifecycle history should record rotation without HSM when optional"
            );
        }

        #[test]
        #[allow(clippy::too_many_lines)]
        fn register_consensus_key_rejects_empty_allowlists() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);

            let mut sumeragi_cfg = SumeragiPolicyConfig {
                collectors_k: iroha_config::parameters::defaults::sumeragi::COLLECTORS_K,
                collectors_redundant_send_r:
                    iroha_config::parameters::defaults::sumeragi::COLLECTORS_REDUNDANT_SEND_R,
                policy_flags: SumeragiPolicyFlags::new(
                    iroha_config::parameters::defaults::sumeragi::DA_ENABLED,
                    true,
                ),
                key_activation_lead_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_ACTIVATION_LEAD_BLOCKS,
                key_overlap_grace_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_OVERLAP_GRACE_BLOCKS,
                key_expiry_grace_blocks:
                    iroha_config::parameters::defaults::sumeragi::KEY_EXPIRY_GRACE_BLOCKS,
                key_allowed_algorithms: BTreeSet::new(),
                key_allowed_hsm_providers: BTreeSet::new(),
            };
            state.set_sumeragi_parameters(sumeragi_cfg.clone());

            let exec = Executor::default();

            // Empty algorithm allowlist rejects any registration.
            {
                let block = new_dummy_block();
                let mut state_block = state.block(block.as_ref().header());
                let params = {
                    let mut stx = state_block.transaction();
                    bootstrap_alice_account(&mut stx);
                    let perm = Permission::new(
                        "CanManageConsensusKeys".parse().unwrap(),
                        iroha_primitives::json::Json::new(()),
                    );
                    Grant::account_permission(perm, ALICE_ID.clone())
                        .execute(&ALICE_ID, &mut stx)
                        .expect("grant manage consensus keys");
                    let params = stx.world.parameters.get().sumeragi.clone();
                    stx.apply();
                    params
                };
                let mut stx = state_block.transaction();
                let kp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
                let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-empty-algos");
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: kp.public_key().clone(),
                    activation_height: stx
                        .block_height()
                        .saturating_add(params.key_activation_lead_blocks),
                    expiry_height: None,
                    hsm: Some(HsmBinding {
                        provider: "softkey".into(),
                        key_label: "validator/empty".into(),
                        slot: None,
                    }),
                    replaces: None,
                    status: ConsensusKeyStatus::Pending,
                };
                let instr: InstructionBox = consensus_keys::RegisterConsensusKey {
                    id: id.clone(),
                    record,
                }
                .into();
                let err = exec
                    .execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
                    .expect_err("empty algorithm allowlist must reject");
                let msg = smart_contract_error_message(err);
                assert_eq!(
                    msg,
                    "consensus key algorithm bls_normal is not allowed; allowed: []"
                );
            }

            // Empty provider allowlist rejects bindings even when the algorithm is permitted.
            {
                sumeragi_cfg.key_allowed_algorithms =
                    [Algorithm::Ed25519].into_iter().collect::<BTreeSet<_>>();
                sumeragi_cfg.key_allowed_hsm_providers.clear();
                state.set_sumeragi_parameters(sumeragi_cfg.clone());

                let block = new_dummy_block();
                let mut state_block = state.block(block.as_ref().header());
                let params = {
                    let mut stx = state_block.transaction();
                    bootstrap_alice_account(&mut stx);
                    let perm = Permission::new(
                        "CanManageConsensusKeys".parse().unwrap(),
                        iroha_primitives::json::Json::new(()),
                    );
                    Grant::account_permission(perm, ALICE_ID.clone())
                        .execute(&ALICE_ID, &mut stx)
                        .expect("grant manage consensus keys");
                    let params = stx.world.parameters.get().sumeragi.clone();
                    stx.apply();
                    params
                };
                let mut stx = state_block.transaction();
                let kp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::Ed25519);
                let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-empty-hsm");
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: kp.public_key().clone(),
                    activation_height: stx
                        .block_height()
                        .saturating_add(params.key_activation_lead_blocks),
                    expiry_height: None,
                    hsm: Some(HsmBinding {
                        provider: "softkey".into(),
                        key_label: "validator/empty-hsm".into(),
                        slot: None,
                    }),
                    replaces: None,
                    status: ConsensusKeyStatus::Pending,
                };
                let instr: InstructionBox = consensus_keys::RegisterConsensusKey {
                    id: id.clone(),
                    record,
                }
                .into();
                let err = exec
                    .execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
                    .expect_err("empty provider allowlist must reject");
                let msg = smart_contract_error_message(err);
                assert_eq!(
                    msg,
                    "HSM provider softkey is not allowed; allowed providers: []"
                );
            }

            // Optional HSM policy still enforces the provider allowlist when a binding is supplied.
            {
                sumeragi_cfg.policy_flags.set_key_require_hsm(false);
                sumeragi_cfg.key_allowed_algorithms =
                    [Algorithm::Ed25519].into_iter().collect::<BTreeSet<_>>();
                sumeragi_cfg.key_allowed_hsm_providers.clear();
                state.set_sumeragi_parameters(sumeragi_cfg.clone());

                let block = new_dummy_block();
                let mut state_block = state.block(block.as_ref().header());
                let params = {
                    let mut stx = state_block.transaction();
                    bootstrap_alice_account(&mut stx);
                    let perm = Permission::new(
                        "CanManageConsensusKeys".parse().unwrap(),
                        iroha_primitives::json::Json::new(()),
                    );
                    Grant::account_permission(perm, ALICE_ID.clone())
                        .execute(&ALICE_ID, &mut stx)
                        .expect("grant manage consensus keys");
                    let params = stx.world.parameters.get().sumeragi.clone();
                    stx.apply();
                    params
                };
                let mut stx = state_block.transaction();
                let kp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::Ed25519);
                let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-optional-hsm");
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: kp.public_key().clone(),
                    activation_height: stx
                        .block_height()
                        .saturating_add(params.key_activation_lead_blocks),
                    expiry_height: None,
                    hsm: Some(HsmBinding {
                        provider: "softkey".into(),
                        key_label: "validator/optional".into(),
                        slot: None,
                    }),
                    replaces: None,
                    status: ConsensusKeyStatus::Pending,
                };
                let instr: InstructionBox = consensus_keys::RegisterConsensusKey {
                    id: id.clone(),
                    record,
                }
                .into();
                let err = exec
                    .execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
                    .expect_err("provided binding must honor allowlist even when optional");
                let msg = smart_contract_error_message(err);
                assert_eq!(
                    msg,
                    "HSM provider softkey is not allowed; allowed providers: []"
                );
            }
        }

        #[test]
        fn rotate_consensus_key_marks_previous_retiring() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let params = {
                let mut stx = state_block.transaction();
                bootstrap_alice_account(&mut stx);
                let perm = Permission::new(
                    "CanManageConsensusKeys".parse().unwrap(),
                    iroha_primitives::json::Json::new(()),
                );
                Grant::account_permission(perm, ALICE_ID.clone())
                    .execute(&ALICE_ID, &mut stx)
                    .expect("grant manage consensus keys");
                let params = stx.world.parameters.get().sumeragi.clone();
                stx.apply();
                params
            };

            let mut stx = state_block.transaction();
            let kp_a = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let id_a = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-primary");
            let record_a = ConsensusKeyRecord {
                id: id_a.clone(),
                public_key: kp_a.public_key().clone(),
                activation_height: stx
                    .block_height()
                    .saturating_add(params.key_activation_lead_blocks),
                expiry_height: None,
                hsm: Some(HsmBinding {
                    provider: "pkcs11".into(),
                    key_label: "validator/0".into(),
                    slot: None,
                }),
                replaces: None,
                status: ConsensusKeyStatus::Pending,
            };
            let exec = Executor::default();
            let instr_a: InstructionBox = consensus_keys::RegisterConsensusKey {
                id: id_a.clone(),
                record: record_a,
            }
            .into();
            exec.execute_instruction(&mut stx, &ALICE_ID.clone(), instr_a)
                .expect("register initial key");
            stx.apply();

            let mut stx = state_block.transaction();
            let kp_b = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let id_b = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-next");
            let record_b = ConsensusKeyRecord {
                id: id_b.clone(),
                public_key: kp_b.public_key().clone(),
                activation_height: stx
                    .block_height()
                    .saturating_add(params.key_activation_lead_blocks + 1),
                expiry_height: None,
                hsm: Some(HsmBinding {
                    provider: "pkcs11".into(),
                    key_label: "validator/1".into(),
                    slot: Some(2),
                }),
                replaces: Some(id_a.clone()),
                status: ConsensusKeyStatus::Pending,
            };
            let rotate_instr: InstructionBox = consensus_keys::RotateConsensusKey {
                id: id_b.clone(),
                record: record_b.clone(),
            }
            .into();
            exec.execute_instruction(&mut stx, &ALICE_ID.clone(), rotate_instr)
                .expect("rotate consensus key");

            let prev = stx
                .world
                .consensus_keys
                .get(&id_a)
                .expect("prev key stored");
            let next = stx
                .world
                .consensus_keys
                .get(&id_b)
                .expect("next key stored");
            assert_eq!(prev.status, ConsensusKeyStatus::Retiring);
            assert_eq!(next.public_key, record_b.public_key);
        }

        #[test]
        fn mode_activation_height_requires_next_mode_in_same_block() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let activation = SetParameter(Parameter::Sumeragi(
                SumeragiParameter::ModeActivationHeight(5),
            ));
            let err = activation
                .execute(&ALICE_ID, &mut stx)
                .expect_err("mode_activation_height without next_mode must fail");
            drop(stx);

            match err {
                Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => assert!(
                    msg.contains("mode_activation_height requires next_mode"),
                    "unexpected error message: {msg}"
                ),
                other => panic!("unexpected error type: {other:?}"),
            }
        }

        #[test]
        fn set_parameter_rejects_zero_collectors_k() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let update = SetParameter(Parameter::Sumeragi(SumeragiParameter::CollectorsK(0)));
            let err = update
                .execute(&ALICE_ID, &mut stx)
                .expect_err("collectors_k=0 must be rejected");
            match err {
                Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                    assert_eq!(msg, "sumeragi.collectors_k must be greater than zero")
                }
                other => panic!("unexpected error type: {other:?}"),
            }
        }

        #[test]
        fn set_parameter_rejects_zero_redundant_send_r() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let update = SetParameter(Parameter::Sumeragi(SumeragiParameter::RedundantSendR(0)));
            let err = update
                .execute(&ALICE_ID, &mut stx)
                .expect_err("redundant_send_r=0 must be rejected");
            match err {
                Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => assert_eq!(
                    msg,
                    "sumeragi.collectors_redundant_send_r must be greater than zero"
                ),
                other => panic!("unexpected error type: {other:?}"),
            }
        }

        #[test]
        fn next_mode_without_activation_height_rejected_at_commit() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let next_mode = SetParameter(Parameter::Sumeragi(SumeragiParameter::NextMode(
                SumeragiConsensusMode::Npos,
            )));
            next_mode
                .execute(&ALICE_ID, &mut stx)
                .expect("set next_mode should succeed");
            stx.apply();

            let err = state_block
                .commit()
                .expect_err("commit must fail when activation height is missing");
            assert!(
                matches!(err, TransactionsBlockError::ModeStagingInvariant),
                "unexpected commit error: {err:?}"
            );
        }

        #[test]
        fn next_mode_and_activation_height_commit_in_same_block() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());

            {
                let mut stx = state_block.transaction();
                let next_mode = SetParameter(Parameter::Sumeragi(SumeragiParameter::NextMode(
                    SumeragiConsensusMode::Npos,
                )));
                next_mode
                    .execute(&ALICE_ID, &mut stx)
                    .expect("set next_mode should succeed");
                stx.apply();
            }

            {
                let mut stx = state_block.transaction();
                let activation = SetParameter(Parameter::Sumeragi(
                    SumeragiParameter::ModeActivationHeight(5),
                ));
                activation
                    .execute(&ALICE_ID, &mut stx)
                    .expect("activation height should be accepted");
                stx.apply();
            }

            state_block
                .commit()
                .expect("commit should succeed when both staging parameters are present");
        }

        #[test]
        fn mode_activation_height_requires_future_block() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());

            // Schedule a mode transition to NPoS.
            let mut stx = state_block.transaction();
            let next_mode = SetParameter(Parameter::Sumeragi(SumeragiParameter::NextMode(
                SumeragiConsensusMode::Npos,
            )));
            next_mode
                .execute(&ALICE_ID, &mut stx)
                .expect("set next_mode should succeed");
            stx.apply();

            {
                // Activation height equal to the current block height must be rejected.
                let mut stx = state_block.transaction();
                let invalid_activation = SetParameter(Parameter::Sumeragi(
                    SumeragiParameter::ModeActivationHeight(1),
                ));
                let err = invalid_activation
                    .execute(&ALICE_ID, &mut stx)
                    .expect_err("mode_activation_height equal to current height must fail");
                drop(stx);
                match err {
                    Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => assert!(
                        msg.contains("mode_activation_height")
                            && msg.contains("must be greater than current block height"),
                        "unexpected error message: {msg}"
                    ),
                    other => panic!("unexpected error type: {other:?}"),
                }
            }

            {
                // Activation height behind the current block must also be rejected.
                let mut stx = state_block.transaction();
                let invalid_activation = SetParameter(Parameter::Sumeragi(
                    SumeragiParameter::ModeActivationHeight(0),
                ));
                let err = invalid_activation
                    .execute(&ALICE_ID, &mut stx)
                    .expect_err("mode_activation_height below current height must fail");
                drop(stx);
                match err {
                    Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => assert!(
                        msg.contains("mode_activation_height")
                            && msg.contains("must be greater than current block height"),
                        "unexpected error message: {msg}"
                    ),
                    other => panic!("unexpected error type: {other:?}"),
                }
            }

            // Activation height one block ahead is accepted.
            let mut stx = state_block.transaction();
            let valid_activation = SetParameter(Parameter::Sumeragi(
                SumeragiParameter::ModeActivationHeight(2),
            ));
            valid_activation
                .execute(&ALICE_ID, &mut stx)
                .expect("mode_activation_height greater than current height must succeed");
        }

        #[test]
        fn update_vk_rejects_non_ipa_backend() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            bootstrap_alice_account(&mut stx);
            let perm = Permission::new(
                "CanManageVerifyingKeys".parse().unwrap(),
                iroha_primitives::json::Json::new(()),
            );
            Grant::account_permission(perm, ALICE_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant manage vk");
            stx.apply();

            // Seed registry with a valid record
            let mut stx = state_block.transaction();
            let exec = Executor::default();
            let id = VerifyingKeyId::new("halo2/ipa", "vk_update");
            let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2, 3]);
            let mut rec = VerifyingKeyRecord::new_with_owner(
                1,
                "vk_update",
                None,
                "test",
                BackendTag::Halo2IpaPasta,
                "pallas",
                [0x51; 32],
                hash_vk(&vk_box),
            );
            rec.vk_len = 3;
            rec.status = ConfidentialStatus::Active;
            rec.key = Some(vk_box.clone());
            rec.gas_schedule_id = Some("halo2_default".into());
            let register_vk_instruction: InstructionBox = verifying_keys::RegisterVerifyingKey {
                id: id.clone(),
                record: rec,
            }
            .into();
            exec.execute_instruction(&mut stx, &ALICE_ID.clone(), register_vk_instruction)
                .expect("register vk");
            stx.apply();

            // Attempt to update with an unsupported backend tag
            let mut stx = state_block.transaction();
            let mut new_rec = VerifyingKeyRecord::new_with_owner(
                2,
                "vk_update",
                None,
                "test",
                BackendTag::Halo2Bn254,
                "bn254",
                [0x52; 32],
                hash_vk(&vk_box),
            );
            new_rec.vk_len = 3;
            new_rec.status = ConfidentialStatus::Active;
            new_rec.key = Some(VerifyingKeyBox::new("halo2/bn254".into(), vec![4, 5, 6]));
            new_rec.gas_schedule_id = Some("halo2_default".into());
            let upd: InstructionBox = verifying_keys::UpdateVerifyingKey {
                id: id.clone(),
                record: new_rec,
            }
            .into();
            let err = exec
                .execute_instruction(&mut stx, &ALICE_ID.clone(), upd)
                .expect_err("update with non-IPA backend must fail");
            let msg = smart_contract_error_message(err);
            assert!(
                msg.contains("backend must be Halo2IpaPasta"),
                "unexpected msg: {msg}"
            );
        }

        #[test]
        fn verify_proof_rejects_missing_circuit_index() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let header = iroha_data_model::block::BlockHeader::new(
                NonZeroU64::new(1).unwrap(),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let exec = Executor::default();

            // Grant permission to manage verifying keys
            let mut stx = block.transaction();
            bootstrap_alice_account(&mut stx);
            let perm = Permission::new(
                "CanManageVerifyingKeys".parse().unwrap(),
                iroha_primitives::json::Json::new(()),
            );
            Grant::account_permission(perm, ALICE_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant manage vk");
            stx.apply();

            // Register a verifying key
            let mut stx = block.transaction();
            let circuit = "circuit_live";
            let vk_id = VerifyingKeyId::new("halo2/ipa", "vk_live");
            let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![7, 7, 7]);
            let mut rec = VerifyingKeyRecord::new_with_owner(
                1,
                circuit.to_string(),
                None,
                "test",
                BackendTag::Halo2IpaPasta,
                "pallas",
                [0x62; 32],
                hash_vk(&vk_box),
            );
            rec.vk_len = 3;
            rec.status = ConfidentialStatus::Active;
            rec.key = Some(vk_box.clone());
            rec.gas_schedule_id = Some("halo2_default".into());
            let register_vk_instruction: InstructionBox = verifying_keys::RegisterVerifyingKey {
                id: vk_id.clone(),
                record: rec,
            }
            .into();
            exec.execute_instruction(&mut stx, &ALICE_ID.clone(), register_vk_instruction)
                .expect("register vk");
            stx.apply();

            // Remove circuit index entry to simulate missing mapping
            {
                let mut stx_remove = block.transaction();
                stx_remove
                    .world
                    .verifying_keys_by_circuit
                    .remove((circuit.to_string(), 1));
                stx_remove.apply();
            }

            // Prepare proof attachment and mark as preverified
            let envelope = OpenVerifyEnvelope {
                backend: BackendTag::Halo2IpaPasta,
                circuit_id: circuit.to_string(),
                vk_hash: hash_vk(&vk_box),
                public_inputs: vec![1, 2, 3],
                proof_bytes: vec![4, 5, 6],
                aux: Vec::new(),
            };
            let proof_bytes = norito::to_bytes(&envelope).expect("encode envelope");
            let proof_box = ProofBox::new("halo2/ipa".into(), proof_bytes);
            let attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof_box.clone(), vk_id);
            let proof_hash = hash_proof(&attachment.proof);
            let mut map = BTreeMap::new();
            map.insert(proof_hash, true);
            block.set_preverified_batch(Arc::new(map));

            let mut stx_verify = block.transaction();
            let verify: InstructionBox =
                iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
            let err = exec
                .execute_instruction(&mut stx_verify, &ALICE_ID.clone(), verify)
                .expect_err("missing circuit index should reject proof");
            match err {
                ValidationFail::InstructionFailed(
                    InstructionExecutionError::InvariantViolation(msg),
                ) => assert!(msg.contains("circuit/version"), "unexpected msg: {msg}"),
                other => panic!("unexpected error: {other:?}"),
            }
        }

        #[test]
        fn verify_proof_requires_open_verify_envelope() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let header = iroha_data_model::block::BlockHeader::new(
                NonZeroU64::new(1).unwrap(),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let exec = Executor::default();

            let mut stx = block.transaction();
            bootstrap_alice_account(&mut stx);
            let perm = Permission::new(
                "CanManageVerifyingKeys".parse().unwrap(),
                iroha_primitives::json::Json::new(()),
            );
            Grant::account_permission(perm, ALICE_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant manage vk");
            stx.apply();

            let mut stx = block.transaction();
            let vk_id = VerifyingKeyId::new("halo2/ipa", "vk_env");
            let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![5, 4, 3]);
            let mut rec = VerifyingKeyRecord::new_with_owner(
                1,
                "circuit_env".to_string(),
                None,
                "test",
                BackendTag::Halo2IpaPasta,
                "pallas",
                [0x63; 32],
                hash_vk(&vk_box),
            );
            rec.vk_len = 3;
            rec.status = ConfidentialStatus::Active;
            rec.key = Some(vk_box);
            rec.gas_schedule_id = Some("halo2_default".into());
            let register_vk_instruction: InstructionBox = verifying_keys::RegisterVerifyingKey {
                id: vk_id.clone(),
                record: rec,
            }
            .into();
            exec.execute_instruction(&mut stx, &ALICE_ID.clone(), register_vk_instruction)
                .expect("register vk");
            stx.apply();

            let proof_box = ProofBox::new("halo2/ipa".into(), vec![0xAA]);
            let attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof_box.clone(), vk_id);
            let proof_hash = hash_proof(&attachment.proof);
            let mut map = BTreeMap::new();
            map.insert(proof_hash, true);
            block.set_preverified_batch(Arc::new(map));

            let mut stx_verify = block.transaction();
            let verify: InstructionBox =
                iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
            let err = exec
                .execute_instruction(&mut stx_verify, &ALICE_ID.clone(), verify)
                .expect_err("missing envelope should reject proof");
            let msg = smart_contract_error_message(err);
            assert!(msg.contains("OpenVerifyEnvelope"), "unexpected msg: {msg}");
        }

        #[test]
        fn verify_proof_rejects_missing_gas_schedule() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let header = iroha_data_model::block::BlockHeader::new(
                NonZeroU64::new(1).unwrap(),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let exec = Executor::default();

            let mut stx = block.transaction();
            bootstrap_alice_account(&mut stx);
            let perm = Permission::new(
                "CanManageVerifyingKeys".parse().unwrap(),
                iroha_primitives::json::Json::new(()),
            );
            Grant::account_permission(perm, ALICE_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant manage vk");
            stx.apply();

            // Register verifying key
            let mut stx = block.transaction();
            let vk_id = VerifyingKeyId::new("halo2/ipa", "vk_gas");
            let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![8, 8, 8]);
            let mut rec = VerifyingKeyRecord::new_with_owner(
                1,
                "circuit_gas".to_string(),
                None,
                "test",
                BackendTag::Halo2IpaPasta,
                "pallas",
                [0x64; 32],
                hash_vk(&vk_box),
            );
            rec.vk_len = 3;
            rec.status = ConfidentialStatus::Active;
            rec.key = Some(vk_box.clone());
            rec.gas_schedule_id = Some("halo2_default".into());
            let register_vk_instruction: InstructionBox = verifying_keys::RegisterVerifyingKey {
                id: vk_id.clone(),
                record: rec,
            }
            .into();
            exec.execute_instruction(&mut stx, &ALICE_ID.clone(), register_vk_instruction)
                .expect("register vk");
            stx.apply();

            // Corrupt stored record by removing gas schedule
            {
                let mut stx_mut = block.transaction();
                if let Some(stored) = stx_mut.world.verifying_keys.get_mut(&vk_id) {
                    stored.gas_schedule_id = None;
                }
                stx_mut.apply();
            }

            let envelope = OpenVerifyEnvelope {
                backend: BackendTag::Halo2IpaPasta,
                circuit_id: "circuit_gas".to_string(),
                vk_hash: hash_vk(&vk_box),
                public_inputs: vec![1, 2, 3],
                proof_bytes: vec![4, 5, 6],
                aux: Vec::new(),
            };
            let proof_bytes = norito::to_bytes(&envelope).expect("encode envelope");
            let proof_box = ProofBox::new("halo2/ipa".into(), proof_bytes);
            let attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof_box.clone(), vk_id);
            let proof_hash = hash_proof(&attachment.proof);
            let mut map = BTreeMap::new();
            map.insert(proof_hash, true);
            block.set_preverified_batch(Arc::new(map));

            let mut stx_verify = block.transaction();
            let verify: InstructionBox =
                iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
            let err = exec
                .execute_instruction(&mut stx_verify, &ALICE_ID.clone(), verify)
                .expect_err("missing gas schedule should reject proof");
            match err {
                ValidationFail::InstructionFailed(
                    InstructionExecutionError::InvariantViolation(msg),
                ) => assert!(msg.contains("gas_schedule_id"), "unexpected msg: {msg}"),
                other => panic!("unexpected error: {other:?}"),
            }
        }

        fn install_endorsement_keys(stx: &mut StateTransaction<'_, '_>, keys: &[KeyPair]) {
            for (idx, kp) in keys.iter().enumerate() {
                let id = ConsensusKeyId::new(
                    ConsensusKeyRole::Endorsement,
                    Ident::from_str(&format!("endorser-{idx}")).expect("valid ident"),
                );
                let rec = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: kp.public_key().clone(),
                    activation_height: 0,
                    expiry_height: None,
                    hsm: None,
                    replaces: None,
                    status: ConsensusKeyStatus::Active,
                };
                stx.world.consensus_keys.insert(id.clone(), rec);
                stx.world
                    .consensus_keys_by_pk
                    .insert(kp.public_key().to_string(), vec![id]);
            }
        }

        fn make_endorsement(
            domain_id: &DomainId,
            committee_id: &str,
            issued_at_height: u64,
            expires_at_height: u64,
            scope: DomainEndorsementScope,
            signers: &[KeyPair],
        ) -> DomainEndorsement {
            let mut endorsement = DomainEndorsement {
                version: iroha_data_model::nexus::DOMAIN_ENDORSEMENT_VERSION_V1,
                domain_id: domain_id.clone(),
                committee_id: committee_id.to_owned(),
                statement_hash: Hash::new(domain_id.to_string().as_bytes()),
                issued_at_height,
                expires_at_height,
                scope,
                signatures: Vec::new(),
                metadata: Metadata::default(),
            };
            let body_hash = endorsement.body_hash();
            for kp in signers {
                let sig = iroha_crypto::Signature::new(kp.private_key(), body_hash.as_ref());
                endorsement.signatures.push(DomainEndorsementSignature {
                    signer: kp.public_key().clone(),
                    signature: sig,
                });
            }
            endorsement
        }

        #[test]
        fn register_domain_rejects_missing_endorsement_when_required() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::default(), kura, query_handle);
            state.nexus.get_mut().enabled = true;
            state.nexus.get_mut().endorsement.quorum = 1;
            state.nexus.get_mut().endorsement.committee_keys = vec!["noop".to_string()];

            let header = iroha_data_model::block::BlockHeader::new(
                NonZeroU64::new(5).unwrap(),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let mut stx = block.transaction();
            let domain_id: DomainId = "wonderland".parse().expect("domain id parses");
            let err = Register::domain(Domain::new(domain_id))
                .execute(&ALICE_ID, &mut stx)
                .expect_err("endorsement must be required");
            let msg = smart_contract_error_message(
                iroha_data_model::ValidationFail::InstructionFailed(err),
            );
            assert!(
                msg.contains("domain endorsement required"),
                "unexpected error: {msg}"
            );
        }

        #[test]
        fn domain_endorsement_with_valid_scope_is_recorded() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::default(), kura, query_handle);
            let kp_a = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_b = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            state.nexus.get_mut().enabled = true;
            state.nexus.get_mut().endorsement.quorum = 2;
            state.nexus.get_mut().endorsement.committee_keys =
                vec![kp_a.public_key().to_string(), kp_b.public_key().to_string()];

            let header = iroha_data_model::block::BlockHeader::new(
                NonZeroU64::new(7).unwrap(),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let mut stx = block.transaction();
            install_endorsement_keys(&mut stx, &[kp_a.clone(), kp_b.clone()]);

            let domain_id: DomainId = "endorse-me".parse().expect("domain id parses");
            let scope = DomainEndorsementScope {
                dataspace: None,
                block_start: Some(stx.block_height()),
                block_end: Some(stx.block_height().saturating_add(3)),
            };
            let endorsement = make_endorsement(
                &domain_id,
                "default",
                stx.block_height(),
                stx.block_height().saturating_add(5),
                scope,
                &[kp_a, kp_b],
            );
            let mut metadata = Metadata::default();
            let key: Name = "endorsement".parse().expect("name parses");
            metadata.insert(key, Json::new(endorsement.clone()));

            Register::domain(Domain::new(domain_id.clone()).with_metadata(metadata))
                .execute(&ALICE_ID, &mut stx)
                .expect("domain registration should accept valid endorsement");

            let recorded = stx
                .world
                .domain_endorsements
                .get(&endorsement.body_hash())
                .expect("endorsement recorded");
            assert_eq!(recorded.accepted_at_height, stx.block_height());
            let index = stx
                .world
                .domain_endorsements_by_domain
                .get(&domain_id)
                .cloned()
                .unwrap_or_default();
            assert!(
                index.contains(&endorsement.body_hash()),
                "endorsement hash must be indexed by domain"
            );
        }

        #[test]
        fn domain_endorsement_rejects_window_and_dataspace_mismatch() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::default(), kura, query_handle);
            let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            state.nexus.get_mut().enabled = true;
            state.nexus.get_mut().endorsement.quorum = 1;
            state.nexus.get_mut().endorsement.committee_keys = vec![kp.public_key().to_string()];

            let header = iroha_data_model::block::BlockHeader::new(
                NonZeroU64::new(9).unwrap(),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let mut stx = block.transaction();
            install_endorsement_keys(&mut stx, std::slice::from_ref(&kp));

            let domain_id: DomainId = "scope-check".parse().expect("domain id parses");
            let late_scope = DomainEndorsementScope {
                dataspace: None,
                block_start: Some(stx.block_height().saturating_add(2)),
                block_end: Some(stx.block_height().saturating_add(4)),
            };
            let late_endorsement = make_endorsement(
                &domain_id,
                "default",
                stx.block_height(),
                stx.block_height().saturating_add(10),
                late_scope,
                std::slice::from_ref(&kp),
            );
            let mut metadata = Metadata::default();
            let key: Name = "endorsement".parse().expect("name parses");
            metadata.insert(key.clone(), Json::new(late_endorsement.clone()));
            let err = Register::domain(Domain::new(domain_id.clone()).with_metadata(metadata))
                .execute(&ALICE_ID, &mut stx)
                .expect_err("endorsement outside window must be rejected");
            let msg = smart_contract_error_message(
                iroha_data_model::ValidationFail::InstructionFailed(err),
            );
            assert!(
                msg.contains("outside declared block window"),
                "unexpected message: {msg}"
            );

            let mut metadata = Metadata::default();
            let scope = DomainEndorsementScope {
                dataspace: Some(DataSpaceId::new(99)),
                block_start: Some(stx.block_height()),
                block_end: Some(stx.block_height().saturating_add(5)),
            };
            let ds_endorsement = make_endorsement(
                &domain_id,
                "default",
                stx.block_height(),
                stx.block_height().saturating_add(10),
                scope,
                &[kp],
            );
            metadata.insert(key, Json::new(ds_endorsement));
            let err = Register::domain(Domain::new(domain_id).with_metadata(metadata))
                .execute(&ALICE_ID, &mut stx)
                .expect_err("unknown dataspace must be rejected");
            let msg = smart_contract_error_message(
                iroha_data_model::ValidationFail::InstructionFailed(err),
            );
            assert!(
                msg.contains("dataspace"),
                "unexpected message for dataspace rejection: {msg}"
            );
        }
    }
    /// Query module provides `IrohaQuery` Peer related implementations.
    pub mod query {
        use eyre::Result;
        use iroha_data_model::{
            parameter::Parameters,
            prelude::*,
            query::{
                dsl::{CompoundPredicate, EvaluatePredicate},
                error::QueryExecutionFail as Error,
            },
            role::Role,
        };

        use super::*;
        use crate::{smartcontracts::ValidQuery, state::StateReadOnly};

        impl ValidQuery for FindRoles {
            #[metrics(+"find_roles")]
            fn execute(
                self,
                filter: CompoundPredicate<Role>,
                state_ro: &impl StateReadOnly,
            ) -> Result<impl Iterator<Item = Self::Item>, Error> {
                Ok(state_ro
                    .world()
                    .roles()
                    .iter()
                    .map(|(_, role)| role)
                    .filter(move |&role| filter.applies(role))
                    .cloned())
            }
        }

        impl ValidQuery for FindRoleIds {
            #[metrics(+"find_role_ids")]
            fn execute(
                self,
                filter: CompoundPredicate<RoleId>,
                state_ro: &impl StateReadOnly,
            ) -> Result<impl Iterator<Item = Self::Item>, Error> {
                Ok(state_ro
                    .world()
                    .roles()
                    .iter()
                    .map(|(_, role)| role)
                    .map(Role::id)
                    .filter(move |&role| filter.applies(role))
                    .cloned())
            }
        }

        impl ValidQuery for FindPeers {
            #[metrics(+"find_peers")]
            fn execute(
                self,
                filter: CompoundPredicate<PeerId>,
                state_ro: &impl StateReadOnly,
            ) -> Result<impl Iterator<Item = Self::Item>, Error> {
                Ok(state_ro
                    .world()
                    .peers()
                    .into_iter()
                    .filter(move |peer| filter.applies(peer))
                    .cloned())
            }
        }

        impl ValidSingularQuery for FindExecutorDataModel {
            #[metrics(+"find_executor_data_model")]
            fn execute(&self, state_ro: &impl StateReadOnly) -> Result<ExecutorDataModel, Error> {
                Ok(state_ro.world().executor_data_model().clone())
            }
        }

        impl ValidSingularQuery for iroha_data_model::query::runtime::prelude::FindActiveAbiVersions {
            #[metrics(+"find_active_abi_versions")]
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<iroha_data_model::query::runtime::ActiveAbiVersions, Error> {
                let set = state_ro.world().active_abi_versions();
                let mut v: Vec<u16> = set.into_iter().collect();
                v.sort_unstable();
                let default = *v.last().unwrap_or(&1);
                Ok(iroha_data_model::query::runtime::ActiveAbiVersions {
                    active_versions: v,
                    default_compile_target: default,
                })
            }
        }

        impl ValidSingularQuery for FindParameters {
            #[metrics(+"find_parameters")]
            fn execute(&self, state_ro: &impl StateReadOnly) -> Result<Parameters, Error> {
                let params = state_ro.world().parameters().clone();
                iroha_logger::debug!(
                    max_tx = params.block().max_transactions().get(),
                    sc_depth = params.smart_contract().execution_depth(),
                    exec_depth = params.executor().execution_depth(),
                    "serving FindParameters"
                );
                Ok(params)
            }
        }

        impl ValidSingularQuery for iroha_data_model::query::proof::prelude::FindProofRecordById {
            #[metrics(+"find_proof_record_by_id")]
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<iroha_data_model::proof::ProofRecord, Error> {
                state_ro
                    .world()
                    .proofs()
                    .get(&self.id)
                    .cloned()
                    .ok_or_else(|| Error::Conversion("ProofRecord not found".to_string()))
            }
        }

        impl ValidSingularQuery
            for iroha_data_model::query::endorsement::prelude::FindDomainEndorsementPolicy
        {
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<DomainEndorsementPolicy, Error> {
                state_ro
                    .world()
                    .domain_endorsement_policies()
                    .get(&self.domain_id)
                    .cloned()
                    .ok_or_else(|| {
                        Error::Conversion("Domain endorsement policy not found".to_string())
                    })
            }
        }

        impl ValidSingularQuery for iroha_data_model::query::endorsement::prelude::FindDomainCommittee {
            fn execute(&self, state_ro: &impl StateReadOnly) -> Result<DomainCommittee, Error> {
                state_ro
                    .world()
                    .domain_committees()
                    .get(&self.committee_id)
                    .cloned()
                    .ok_or_else(|| Error::Conversion("Domain committee not found".to_string()))
            }
        }

        impl ValidSingularQuery for iroha_data_model::query::sorafs::prelude::FindSorafsProviderOwner {
            #[metrics(+"find_sorafs_provider_owner")]
            fn execute(&self, state_ro: &impl StateReadOnly) -> Result<AccountId, Error> {
                state_ro
                    .world()
                    .provider_owners()
                    .get(&self.provider_id)
                    .cloned()
                    .ok_or_else(|| Error::Conversion("SoraFS provider owner not found".to_string()))
            }
        }

        impl ValidSingularQuery for iroha_data_model::query::da::prelude::FindDaPinIntentByTicket {
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<DaPinIntentWithLocation, Error> {
                state_ro
                    .world()
                    .da_pin_intents_by_ticket()
                    .get(&self.storage_ticket)
                    .cloned()
                    .ok_or_else(|| Error::Conversion("DA pin intent not found".to_string()))
            }
        }

        impl ValidSingularQuery for iroha_data_model::query::da::prelude::FindDaPinIntentByManifest {
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<DaPinIntentWithLocation, Error> {
                let ticket = state_ro
                    .world()
                    .da_pin_intents_by_manifest()
                    .get(&self.manifest_hash)
                    .copied()
                    .ok_or_else(|| Error::Conversion("DA pin intent not found".to_string()))?;
                state_ro
                    .world()
                    .da_pin_intents_by_ticket()
                    .get(&ticket)
                    .cloned()
                    .ok_or_else(|| Error::Conversion("DA pin intent not found".to_string()))
            }
        }

        impl ValidSingularQuery for iroha_data_model::query::da::prelude::FindDaPinIntentByAlias {
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<DaPinIntentWithLocation, Error> {
                let ticket = state_ro
                    .world()
                    .da_pin_intents_by_alias()
                    .get(&self.alias)
                    .copied()
                    .ok_or_else(|| Error::Conversion("DA pin intent not found".to_string()))?;
                state_ro
                    .world()
                    .da_pin_intents_by_ticket()
                    .get(&ticket)
                    .cloned()
                    .ok_or_else(|| Error::Conversion("DA pin intent not found".to_string()))
            }
        }

        impl ValidSingularQuery
            for iroha_data_model::query::da::prelude::FindDaPinIntentByLaneEpochSequence
        {
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<DaPinIntentWithLocation, Error> {
                let ticket = state_ro
                    .world()
                    .da_pin_intents_by_lane_epoch()
                    .get(&(self.lane_id, self.epoch, self.sequence))
                    .copied()
                    .ok_or_else(|| Error::Conversion("DA pin intent not found".to_string()))?;
                state_ro
                    .world()
                    .da_pin_intents_by_ticket()
                    .get(&ticket)
                    .cloned()
                    .ok_or_else(|| Error::Conversion("DA pin intent not found".to_string()))
            }
        }

        impl ValidSingularQuery for iroha_data_model::query::endorsement::prelude::FindDomainEndorsements {
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<Vec<iroha_data_model::nexus::DomainEndorsementRecord>, Error> {
                let hashes = state_ro
                    .world()
                    .domain_endorsements_by_domain()
                    .get(&self.domain_id)
                    .cloned()
                    .unwrap_or_default();
                let records = hashes
                    .into_iter()
                    .filter_map(|h| state_ro.world().domain_endorsements().get(&h).cloned())
                    .collect();
                Ok(records)
            }
        }

        impl ValidQuery for iroha_data_model::query::proof::prelude::FindProofRecords {
            #[metrics(+"find_proof_records")]
            fn execute(
                self,
                filter: CompoundPredicate<iroha_data_model::proof::ProofRecord>,
                state_ro: &impl StateReadOnly,
            ) -> Result<impl Iterator<Item = iroha_data_model::proof::ProofRecord>, Error>
            {
                Ok(state_ro
                    .world()
                    .proofs()
                    .iter()
                    .map(|(_, rec)| rec)
                    .filter(move |rec| filter.applies(rec))
                    .cloned())
            }
        }

        impl ValidQuery for iroha_data_model::query::proof::prelude::FindProofRecordsByBackend {
            #[metrics(+"find_proof_records_by_backend")]
            fn execute(
                self,
                filter: CompoundPredicate<iroha_data_model::proof::ProofRecord>,
                state_ro: &impl StateReadOnly,
            ) -> Result<impl Iterator<Item = iroha_data_model::proof::ProofRecord>, Error>
            {
                let backend = self.backend;
                Ok(state_ro
                    .world()
                    .proofs()
                    .iter()
                    .map(|(_, rec)| rec)
                    .filter(move |rec| rec.id.backend == backend && filter.applies(rec))
                    .cloned())
            }
        }

        impl ValidQuery for iroha_data_model::query::proof::prelude::FindProofRecordsByStatus {
            #[metrics(+"find_proof_records_by_status")]
            fn execute(
                self,
                filter: CompoundPredicate<iroha_data_model::proof::ProofRecord>,
                state_ro: &impl StateReadOnly,
            ) -> Result<impl Iterator<Item = iroha_data_model::proof::ProofRecord>, Error>
            {
                let status = self.status;
                Ok(state_ro
                    .world()
                    .proofs()
                    .iter()
                    .map(|(_, rec)| rec)
                    .filter(move |rec| rec.status == status && filter.applies(rec))
                    .cloned())
            }
        }
    }
}
