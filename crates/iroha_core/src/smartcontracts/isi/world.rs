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
    use iroha_crypto::{Algorithm, Hash, Hash as CryptoHash, PublicKey, blake2::Blake2b512};
    use iroha_executor_data_model::permission::{
        account::CanRegisterAccount,
        asset::{
            CanBurnAsset, CanBurnAssetWithDefinition, CanMintAsset, CanMintAssetWithDefinition,
            CanModifyAssetMetadata, CanModifyAssetMetadataWithDefinition, CanTransferAsset,
            CanTransferAssetWithDefinition,
        },
        asset_definition::{CanModifyAssetDefinitionMetadata, CanUnregisterAssetDefinition},
        domain::{CanModifyDomainMetadata, CanUnregisterDomain},
        nft::{CanModifyNftMetadata, CanRegisterNft, CanTransferNft, CanUnregisterNft},
    };
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
            prelude::TriggerEvent,
            smart_contract::{
                ContractCodeRegistered, ContractCodeRemoved, ContractInstanceActivated,
                ContractInstanceDeactivated, SmartContractEvent,
            },
        },
        governance::types::{
            AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ParliamentBody,
            ProposalKind, RuntimeUpgradeProposal,
        },
        isi::{
            bridge, consensus_keys, endorsement,
            error::{InstructionExecutionError, InvalidParameterError, MathError, RepetitionError},
            governance as gov, nexus, smart_contract_code as scode, verifying_keys,
        },
        name::{self, Name},
        nexus::{
            AxtProofEnvelope, DomainCommittee, DomainEndorsement, DomainEndorsementPolicy,
            DomainEndorsementRecord, LaneRelayEmergencyValidatorSet, LaneRelayEnvelopeRef,
            VerifiedLaneRelayRecord, proof_matches_manifest,
        },
        parameter::{Parameter, SumeragiParameter},
        prelude::*,
        proof::{ProofId, VerifyingKeyId, VerifyingKeyRecord},
        query::error::FindError,
        zk::{BackendTag, OpenVerifyEnvelope as ZkOpenVerifyEnvelope, StarkFriOpenProofV1},
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
        governance::draw::{self, derive_parliament_bodies},
        smartcontracts::triggers::isi::register_trigger_internal,
        state::derive_validator_key_id,
        sumeragi::status::PeerKeyPolicyRejectReason,
        zk::hash_vk,
    };

    fn ensure_metadata_value(
        metadata: &mut Metadata,
        key: &Name,
        value: iroha_primitives::json::Json,
    ) -> Result<(), Error> {
        if let Some(existing) = metadata.get(key) {
            if existing != &value {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "trigger metadata `{key}` does not match contract deployment context"
                    )),
                ));
            }
            return Ok(());
        }
        metadata.insert(key.clone(), value);
        Ok(())
    }

    fn register_manifest_triggers(
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
        namespace: &str,
        contract_id: &str,
        code_bytes: &[u8],
        manifest: &ContractManifest,
    ) -> Result<(), Error> {
        let Some(entrypoints) = manifest.entrypoints.as_ref() else {
            return Ok(());
        };
        let has_triggers = entrypoints
            .iter()
            .any(|entrypoint| !entrypoint.triggers.is_empty());
        if !has_triggers {
            return Ok(());
        }
        let code_hash = manifest.code_hash.ok_or_else(|| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                "contract manifest missing code_hash for trigger registration".into(),
            ))
        })?;

        for entrypoint in entrypoints {
            for descriptor in &entrypoint.triggers {
                let code_hash_string = code_hash.to_string();
                let trigger_id_string = descriptor.id.to_string();
                if let Some(ref target_ns) = descriptor.callback.namespace
                    && target_ns != namespace
                {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "cross-contract trigger callbacks are not supported yet".into(),
                        ),
                    ));
                }
                let mut metadata = descriptor.metadata.clone();
                let ns_key = Name::from_str("contract_namespace").expect("static metadata key");
                let cid_key = Name::from_str("contract_id").expect("static metadata key");
                let ep_key = Name::from_str("contract_entrypoint").expect("static metadata key");
                let code_key = Name::from_str("contract_code_hash").expect("static metadata key");
                let trigger_key = Name::from_str("contract_trigger_id").expect("static metadata");
                ensure_metadata_value(
                    &mut metadata,
                    &ns_key,
                    iroha_primitives::json::Json::from(namespace),
                )?;
                ensure_metadata_value(
                    &mut metadata,
                    &cid_key,
                    iroha_primitives::json::Json::from(contract_id),
                )?;
                ensure_metadata_value(
                    &mut metadata,
                    &ep_key,
                    iroha_primitives::json::Json::from(descriptor.callback.entrypoint.as_str()),
                )?;
                ensure_metadata_value(
                    &mut metadata,
                    &code_key,
                    iroha_primitives::json::Json::from(code_hash_string.as_str()),
                )?;
                ensure_metadata_value(
                    &mut metadata,
                    &trigger_key,
                    iroha_primitives::json::Json::from(trigger_id_string.as_str()),
                )?;

                let trigger_authority = descriptor
                    .authority
                    .clone()
                    .unwrap_or_else(|| authority.clone());
                let action = iroha_data_model::trigger::action::Action::new(
                    Executable::Ivm(IvmBytecode::from_compiled(code_bytes.to_vec())),
                    descriptor.repeats,
                    trigger_authority,
                    descriptor.filter.clone(),
                )
                .with_metadata(metadata);
                let trigger = Trigger::new(descriptor.id.clone(), action);
                register_trigger_internal(authority, state_transaction, trigger, true)?;
            }
        }
        Ok(())
    }

    fn has_permission(world: &WorldTransaction<'_, '_>, who: &AccountId, name: &str) -> bool {
        world
            .account_permissions
            .get(who)
            .is_some_and(|perms| perms.iter().any(|p| p.name() == name))
    }

    const VERIFIED_LANE_RELAY_STATE_ROOT: &str = "pkdeploy/verified-lane-relays";

    fn build_state_path_key(base: &Name, key: i64) -> Result<Name, Error> {
        Name::from_str(&format!("{base}/{key}")).map_err(|_| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                "invalid smart-contract state path".into(),
            ))
            .into()
        })
    }

    fn build_state_path_key_norito(base: &Name, key_bytes: &[u8]) -> Result<Name, Error> {
        let digest: [u8; 32] = CryptoHash::new(key_bytes).into();
        let suffix = hex::encode(digest);
        Name::from_str(&format!("{base}/{suffix}")).map_err(|_| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                "invalid smart-contract state path".into(),
            ))
            .into()
        })
    }

    fn verified_lane_relay_state_key(relay_ref: &LaneRelayEnvelopeRef) -> Result<Name, Error> {
        let root = Name::from_str(VERIFIED_LANE_RELAY_STATE_ROOT).expect("static state root");
        let ds_path = build_state_path_key(
            &root,
            i64::try_from(relay_ref.dataspace_id.as_u64()).map_err(|_| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    "dataspace id does not fit contract-state path".into(),
                ))
            })?,
        )?;
        let lane_path = build_state_path_key(&ds_path, i64::from(relay_ref.lane_id.as_u32()))?;
        let block_path = build_state_path_key(
            &lane_path,
            i64::try_from(relay_ref.block_height).map_err(|_| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    "block height does not fit contract-state path".into(),
                ))
            })?,
        )?;
        build_state_path_key_norito(&block_path, relay_ref.settlement_hash.as_ref())
    }

    fn load_verified_lane_relay_record(
        state_ro: &impl StateReadOnly,
        relay_ref: &LaneRelayEnvelopeRef,
    ) -> std::result::Result<
        VerifiedLaneRelayRecord,
        iroha_data_model::query::error::QueryExecutionFail,
    > {
        let key = verified_lane_relay_state_key(relay_ref).map_err(|err| {
            iroha_data_model::query::error::QueryExecutionFail::Conversion(err.to_string())
        })?;
        let payload = state_ro
            .world()
            .smart_contract_state()
            .get(&key)
            .ok_or(iroha_data_model::query::error::QueryExecutionFail::NotFound)?;
        norito::decode_from_bytes::<VerifiedLaneRelayRecord>(payload).map_err(|err| {
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "verified lane relay decode failed: {err}"
            ))
        })
    }
    fn protected_contract_namespaces(
        state_transaction: &StateTransaction<'_, '_>,
    ) -> BTreeSet<String> {
        let Ok(name) = core::str::FromStr::from_str("gov_protected_namespaces") else {
            return BTreeSet::new();
        };
        let id = iroha_data_model::parameter::CustomParameterId(name);
        let params = state_transaction.world.parameters.get();
        params
            .custom()
            .get(&id)
            .and_then(|custom| custom.payload().try_into_any_norito::<Vec<String>>().ok())
            .map(|namespaces| {
                namespaces
                    .into_iter()
                    .map(|namespace| namespace.trim().to_owned())
                    .filter(|namespace| !namespace.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn contract_namespace_requires_governance(
        state_transaction: &StateTransaction<'_, '_>,
        namespace: &str,
    ) -> bool {
        let namespace = namespace.trim();
        !namespace.is_empty()
            && protected_contract_namespaces(state_transaction).contains(namespace)
    }

    fn ensure_contract_namespace_governance(
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
        namespace: &str,
    ) -> Result<(), Error> {
        if contract_namespace_requires_governance(state_transaction, namespace)
            && !has_permission(&state_transaction.world, authority, "CanEnactGovernance")
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "not permitted: CanEnactGovernance".into(),
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
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

        match algo {
            Algorithm::BlsNormal => {
                let Some(pop) = record.pop.as_deref() else {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "BLS proof-of-possession required for consensus key".into(),
                        ),
                    ));
                };
                if let Err(err) = iroha_crypto::bls_normal_pop_verify(&record.public_key, pop) {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(format!(
                            "invalid BLS proof-of-possession: {err}"
                        )),
                    ));
                }
            }
            Algorithm::BlsSmall => {
                let Some(pop) = record.pop.as_deref() else {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "BLS proof-of-possession required for consensus key".into(),
                        ),
                    ));
                };
                if let Err(err) = iroha_crypto::bls_small_pop_verify(&record.public_key, pop) {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(format!(
                            "invalid BLS proof-of-possession: {err}"
                        )),
                    ));
                }
            }
            _ => {
                if record.pop.is_some() {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "proof-of-possession is only valid for BLS consensus keys".into(),
                        ),
                    ));
                }
            }
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
        if !(backend.starts_with("halo2/") || crate::zk::is_stark_fri_v1_backend(backend)) {
            return None;
        }
        norito::decode_from_bytes::<ZkOpenVerifyEnvelope>(&proof.bytes).ok()
    }

    struct VotePublicInputs {
        columns: Vec<Vec<[u8; 32]>>,
        envelope: Option<ZkOpenVerifyEnvelope>,
    }

    fn normalize_halo2_circuit_id(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(rest) = trimmed.strip_prefix("halo2/pasta/ipa/") {
            return (!rest.is_empty()).then(|| trimmed.to_string());
        }
        if let Some(rest) = trimmed.strip_prefix("halo2/pasta/") {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
        }
        if let Some(rest) = trimmed.strip_prefix(crate::zk::ZK_BACKEND_HALO2_IPA) {
            if let Some(rest) = rest.strip_prefix("::") {
                return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
            }
            if let Some(rest) = rest.strip_prefix(':') {
                return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
            }
            if let Some(rest) = rest.strip_prefix('/') {
                return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
            }
        }
        Some(format!("halo2/pasta/ipa/{trimmed}"))
    }

    fn circuit_id_matches(backend: &str, record_id: &str, env_id: &str) -> bool {
        if backend == crate::zk::ZK_BACKEND_HALO2_IPA {
            match (
                normalize_halo2_circuit_id(record_id),
                normalize_halo2_circuit_id(env_id),
            ) {
                (Some(rec), Some(env)) => rec == env,
                _ => record_id == env_id,
            }
        } else if crate::zk::is_stark_fri_v1_backend(backend) {
            match (
                normalize_stark_fri_circuit_id(backend, record_id),
                normalize_stark_fri_circuit_id(backend, env_id),
            ) {
                (Some(rec), Some(env)) => rec == env,
                _ => record_id == env_id,
            }
        } else {
            record_id == env_id
        }
    }

    const VOTING_BALLOT_CIRCUIT_ID: &str = "vote-ballot";
    const VOTING_TALLY_CIRCUIT_ID: &str = "vote-tally";

    fn voting_circuit_matches(backend: &str, record_circuit_id: &str, expected_id: &str) -> bool {
        if crate::zk::is_stark_fri_v1_backend(backend) {
            // Enforce canonical STARK vote circuit roles to avoid swapping ballot/tally VKs.
            circuit_id_matches(backend, record_circuit_id, expected_id)
        } else {
            // Preserve existing Halo2 and historical backend behavior for backwards compatibility.
            true
        }
    }

    fn ensure_voting_circuit_role(
        label: &str,
        backend: &str,
        record_circuit_id: &str,
        expected_id: &str,
    ) -> Result<(), Error> {
        if !voting_circuit_matches(backend, record_circuit_id, expected_id) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("{label} verifying key circuit mismatch").into(),
            ));
        }
        Ok(())
    }

    fn normalize_stark_fri_circuit_id(backend: &str, raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == backend {
            return None;
        }
        if let Some(rest) = trimmed.strip_prefix(backend) {
            if let Some(rest) = rest.strip_prefix(':') {
                return (!rest.is_empty()).then(|| trimmed.to_string());
            }
            if let Some(rest) = rest.strip_prefix('/') {
                return (!rest.is_empty()).then(|| format!("{backend}:{rest}"));
            }
        }
        Some(format!("{backend}:{trimmed}"))
    }

    fn validate_vote_envelope_metadata(
        label: &str,
        backend: &str,
        envelope: &ZkOpenVerifyEnvelope,
        vk_record: &VerifyingKeyRecord,
    ) -> Result<(), Error> {
        if !circuit_id_matches(backend, &vk_record.circuit_id, &envelope.circuit_id) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("{label} verifying key circuit mismatch").into(),
            ));
        }
        if vk_record.public_inputs_schema_hash != [0u8; 32] {
            let observed_hash: [u8; 32] = CryptoHash::new(&envelope.public_inputs).into();
            if observed_hash != vk_record.public_inputs_schema_hash {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("{label} public inputs schema hash mismatch").into(),
                ));
            }
        }
        if envelope.vk_hash != [0u8; 32] && envelope.vk_hash != vk_record.commitment {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("{label} verifying key commitment mismatch").into(),
            ));
        }
        Ok(())
    }

    fn enforce_vk_max_proof_bytes(
        label: &str,
        vk_record: &VerifyingKeyRecord,
        proof_len: usize,
    ) -> Result<(), Error> {
        let max = vk_record.max_proof_bytes;
        if max > 0 && proof_len > usize::try_from(max).unwrap_or(usize::MAX) {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(format!(
                    "{label} proof exceeds verifying key max_proof_bytes"
                )),
            ));
        }
        Ok(())
    }

    fn extract_vote_public_inputs(
        backend: &str,
        proof_bytes: &[u8],
    ) -> Result<VotePublicInputs, Error> {
        if backend == crate::zk::ZK_BACKEND_HALO2_IPA {
            let env: ZkOpenVerifyEnvelope =
                norito::decode_from_bytes(proof_bytes).map_err(|_| {
                    InstructionExecutionError::InvariantViolation(
                        "invalid OpenVerifyEnvelope payload".into(),
                    )
                })?;
            if env.backend != BackendTag::Halo2IpaPasta {
                return Err(InstructionExecutionError::InvariantViolation(
                    "unexpected OpenVerifyEnvelope backend tag".into(),
                ));
            }
            let columns = crate::zk::extract_pasta_instance_columns_bytes(&env.proof_bytes)
                .ok_or_else(|| {
                    InstructionExecutionError::InvariantViolation(
                        "failed to extract vote public inputs".into(),
                    )
                })?;
            return Ok(VotePublicInputs {
                columns,
                envelope: Some(env),
            });
        }

        if crate::zk::is_stark_fri_v1_backend(backend) {
            let env: ZkOpenVerifyEnvelope =
                norito::decode_from_bytes(proof_bytes).map_err(|_| {
                    InstructionExecutionError::InvariantViolation(
                        "invalid OpenVerifyEnvelope payload".into(),
                    )
                })?;
            if env.backend != BackendTag::Stark {
                return Err(InstructionExecutionError::InvariantViolation(
                    "unexpected OpenVerifyEnvelope backend tag".into(),
                ));
            }
            let open: StarkFriOpenProofV1 =
                norito::decode_from_bytes(&env.proof_bytes).map_err(|_| {
                    InstructionExecutionError::InvariantViolation(
                        "invalid STARK open proof payload".into(),
                    )
                })?;
            if open.version != 1 {
                return Err(InstructionExecutionError::InvariantViolation(
                    "unsupported STARK open proof version".into(),
                ));
            }
            let columns = open.public_inputs;
            return Ok(VotePublicInputs {
                columns,
                envelope: Some(env),
            });
        }

        let columns =
            crate::zk::extract_pasta_instance_columns_bytes(proof_bytes).ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "failed to extract vote public inputs".into(),
                )
            })?;
        Ok(VotePublicInputs {
            columns,
            envelope: None,
        })
    }

    fn ballot_inputs_from_columns(
        columns: &[Vec<[u8; 32]>],
    ) -> Result<([u8; 32], [u8; 32]), Error> {
        if columns.len() != 2 || columns.iter().any(|col| col.len() != 1) {
            return Err(InstructionExecutionError::InvariantViolation(
                "ballot proof must expose commit/root public inputs".into(),
            ));
        }
        Ok((columns[0][0], columns[1][0]))
    }

    fn bytes_to_u64(value: &[u8; 32]) -> Option<u64> {
        if value[8..].iter().any(|b| *b != 0) {
            return None;
        }
        Some(u64::from_le_bytes(
            value[..8].try_into().expect("slice length"),
        ))
    }

    fn tally_from_columns(
        columns: &[Vec<[u8; 32]>],
        expected_len: usize,
    ) -> Result<Vec<u64>, Error> {
        if columns.len() != expected_len || columns.iter().any(|col| col.len() != 1) {
            return Err(InstructionExecutionError::InvariantViolation(
                "tally proof must expose one public input per option".into(),
            ));
        }
        let mut tally = Vec::with_capacity(expected_len);
        for column in columns {
            let value = bytes_to_u64(&column[0]).ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "tally proof public input out of range".into(),
                )
            })?;
            tally.push(value);
        }
        Ok(tally)
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

    fn derive_ballot_nullifier(
        domain_tag: &str,
        chain_id: &iroha_data_model::ChainId,
        election_id: &str,
        commit: &[u8; 32],
    ) -> [u8; 32] {
        let mut input = Vec::with_capacity(
            domain_tag.len() + chain_id.as_str().len() + election_id.len() + commit.len() + 24,
        );
        // Length-prefix fields to avoid delimiter collisions in concatenated inputs.
        let push_len = |input: &mut Vec<u8>, len: usize| {
            let len_u64 = len as u64;
            input.extend_from_slice(&len_u64.to_le_bytes());
        };
        push_len(&mut input, domain_tag.len());
        input.extend_from_slice(domain_tag.as_bytes());
        push_len(&mut input, chain_id.as_str().len());
        input.extend_from_slice(chain_id.as_str().as_bytes());
        push_len(&mut input, election_id.len());
        input.extend_from_slice(election_id.as_bytes());
        input.extend_from_slice(commit);
        let digest = Blake2b512::digest(&input);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest[..32]);
        out
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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum GovernanceApprovalMode {
        ParliamentSortitionJit,
        LegacyCouncilEpoch,
    }

    fn resolve_governance_approval_mode(
        state_transaction: &StateTransaction<'_, '_>,
    ) -> GovernanceApprovalMode {
        let catalog = &state_transaction.nexus.governance;
        let Some(module_name) = catalog.default_module.as_deref() else {
            return GovernanceApprovalMode::LegacyCouncilEpoch;
        };
        let module_type = catalog
            .modules
            .get(module_name)
            .and_then(|module| module.module_type.as_deref())
            .unwrap_or(module_name);
        let normalized = module_type.trim().to_ascii_lowercase().replace('-', "_");
        if normalized.contains("parliament") || normalized.contains("sortition") {
            GovernanceApprovalMode::ParliamentSortitionJit
        } else {
            GovernanceApprovalMode::LegacyCouncilEpoch
        }
    }

    fn latest_governance_entropy_seed(state_transaction: &StateTransaction<'_, '_>) -> [u8; 32] {
        if let Some((_epoch, record)) = state_transaction.world.vrf_epochs.iter().last() {
            return record.seed;
        }
        let mut input = Vec::with_capacity(
            b"iroha:gov:jit:entropy:fallback:v1|".len()
                + state_transaction.chain_id.as_str().len()
                + core::mem::size_of::<u64>(),
        );
        input.extend_from_slice(b"iroha:gov:jit:entropy:fallback:v1|");
        input.extend_from_slice(state_transaction.chain_id.as_str().as_bytes());
        input.extend_from_slice(&state_transaction._curr_block.height().get().to_le_bytes());
        let digest = Blake2b512::digest(input);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest[..32]);
        out
    }

    fn derive_epoch_parliament_beacon(
        epoch: u64,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> [u8; 32] {
        let entropy = latest_governance_entropy_seed(state_transaction);
        let mut input = Vec::with_capacity(
            b"iroha:gov:epoch-beacon:v1|".len()
                + state_transaction.chain_id.as_str().len()
                + core::mem::size_of::<u64>()
                + entropy.len(),
        );
        input.extend_from_slice(b"iroha:gov:epoch-beacon:v1|");
        input.extend_from_slice(state_transaction.chain_id.as_str().as_bytes());
        input.extend_from_slice(&epoch.to_le_bytes());
        input.extend_from_slice(&entropy);
        let digest = Blake2b512::digest(input);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest[..32]);
        out
    }

    fn derive_proposal_parliament_beacon(
        proposal_id: [u8; 32],
        created_height: u64,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> [u8; 32] {
        let entropy = latest_governance_entropy_seed(state_transaction);
        let mut input = Vec::with_capacity(
            b"iroha:gov:proposal-beacon:v1|".len()
                + state_transaction.chain_id.as_str().len()
                + core::mem::size_of::<u64>()
                + proposal_id.len()
                + entropy.len(),
        );
        input.extend_from_slice(b"iroha:gov:proposal-beacon:v1|");
        input.extend_from_slice(state_transaction.chain_id.as_str().as_bytes());
        input.extend_from_slice(&created_height.to_le_bytes());
        input.extend_from_slice(&proposal_id);
        input.extend_from_slice(&entropy);
        let digest = Blake2b512::digest(input);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest[..32]);
        out
    }

    fn compute_parliament_roster_root(
        bodies: &iroha_data_model::governance::types::ParliamentBodies,
    ) -> Result<[u8; 32], Error> {
        let encoded = norito::to_bytes(bodies).map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                "failed to encode parliament roster commitment".into(),
            )
        })?;
        let digest = Blake2b512::digest(encoded);
        let mut root = [0u8; 32];
        root.copy_from_slice(&digest[..32]);
        Ok(root)
    }

    fn derive_jit_parliament_snapshot(
        proposal_id: [u8; 32],
        created_height: u64,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<crate::state::GovernanceParliamentSnapshot, Error> {
        let selection_epoch = created_height;
        let beacon =
            derive_proposal_parliament_beacon(proposal_id, created_height, state_transaction);
        let current_height = state_transaction._curr_block.height().get();
        let required_bond =
            required_citizenship_bond_for_role(&state_transaction.gov, "parliament").max(
                required_citizenship_bond_for_role(&state_transaction.gov, "council"),
            );
        let candidates: Vec<(AccountId, u128)> = state_transaction
            .world
            .citizens
            .iter()
            .filter_map(|(account_id, record)| {
                if record.amount < required_bond || record.cooldown_until > current_height {
                    return None;
                }
                Some((account_id.clone(), record.amount))
            })
            .collect();
        if candidates.is_empty() {
            return Err(InstructionExecutionError::InvariantViolation(
                "no eligible citizens available for proposal-time parliament sortition".into(),
            ));
        }
        let bodies = draw::derive_parliament_bodies_from_bonded_citizens(
            &state_transaction.gov,
            &state_transaction.chain_id,
            selection_epoch,
            &beacon,
            candidates
                .iter()
                .map(|(account_id, bond)| (account_id, *bond)),
            iroha_data_model::isi::governance::CouncilDerivationKind::Vrf,
        );
        let roster_root = compute_parliament_roster_root(&bodies)?;
        Ok(crate::state::GovernanceParliamentSnapshot {
            selection_epoch,
            beacon,
            roster_root,
            bodies,
        })
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
            let id_backend = id.backend.as_str();
            match record.backend {
                BackendTag::Halo2IpaPasta => {
                    if !record.curve.eq_ignore_ascii_case("pallas") {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "verifying key curve must be \"pallas\"".into(),
                            ),
                        ));
                    }
                    if !id_backend.starts_with("halo2/") || id_backend.contains("bn254") {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "verifying key id backend must target halo2 IPA (no bn254)".into(),
                            ),
                        ));
                    }
                }
                BackendTag::Stark => {
                    if !record.curve.eq_ignore_ascii_case("goldilocks") {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "verifying key curve must be \"goldilocks\"".into(),
                            ),
                        ));
                    }
                    if !crate::zk::is_stark_fri_v1_backend(id_backend) {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "verifying key id backend must target stark/fri".into(),
                            ),
                        ));
                    }
                }
                _ => {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "verifying key backend must be Halo2IpaPasta or Stark".into(),
                        ),
                    ));
                }
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
                if record.backend == BackendTag::Stark {
                    #[cfg(not(feature = "zk-stark"))]
                    {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "verifying key backend Stark is not enabled".into(),
                            ),
                        ));
                    }

                    #[cfg(feature = "zk-stark")]
                    {
                        use crate::zk_stark::{
                            STARK_HASH_POSEIDON2_V1, STARK_HASH_SHA256_V1, StarkFriVerifyingKeyV1,
                        };
                        let expected_hash_fn = if id_backend == crate::zk::ZK_BACKEND_STARK_FRI_V1 {
                            None
                        } else if id_backend.contains("/sha256-") {
                            Some(STARK_HASH_SHA256_V1)
                        } else if id_backend.contains("/poseidon2-") {
                            Some(STARK_HASH_POSEIDON2_V1)
                        } else {
                            return Err(InstructionExecutionError::InvalidParameter(
                                InvalidParameterError::SmartContract(
                                    "unsupported stark/fri backend variant".into(),
                                ),
                            ));
                        };
                        let payload: StarkFriVerifyingKeyV1 = norito::decode_from_bytes(&vk.bytes)
                            .map_err(|_| {
                                InstructionExecutionError::InvalidParameter(
                                    InvalidParameterError::SmartContract(
                                        "invalid STARK verifying key payload".into(),
                                    ),
                                )
                            })?;
                        if payload.version != 1 {
                            return Err(InstructionExecutionError::InvalidParameter(
                                InvalidParameterError::SmartContract(
                                    "unsupported STARK verifying key payload version".into(),
                                ),
                            ));
                        }
                        if payload.hash_fn != STARK_HASH_SHA256_V1
                            && payload.hash_fn != STARK_HASH_POSEIDON2_V1
                        {
                            return Err(InstructionExecutionError::InvalidParameter(
                                InvalidParameterError::SmartContract(
                                    "unsupported STARK verifying key hash_fn".into(),
                                ),
                            ));
                        }
                        if let Some(expected_hash_fn) = expected_hash_fn {
                            if payload.hash_fn != expected_hash_fn {
                                return Err(InstructionExecutionError::InvalidParameter(
                                    InvalidParameterError::SmartContract(
                                        "STARK verifying key hash_fn mismatch".into(),
                                    ),
                                ));
                            }
                        }
                        if payload.circuit_id.trim().is_empty() {
                            return Err(InstructionExecutionError::InvalidParameter(
                                InvalidParameterError::SmartContract(
                                    "STARK verifying key circuit_id must not be empty".into(),
                                ),
                            ));
                        }
                        let payload_circuit_id =
                            normalize_stark_fri_circuit_id(id_backend, &payload.circuit_id)
                                .ok_or_else(|| {
                                    InstructionExecutionError::InvalidParameter(
                                        InvalidParameterError::SmartContract(
                                            "invalid STARK verifying key circuit_id".into(),
                                        ),
                                    )
                                })?;
                        let record_circuit_id =
                            normalize_stark_fri_circuit_id(id_backend, &record.circuit_id)
                                .ok_or_else(|| {
                                    InstructionExecutionError::InvalidParameter(
                                        InvalidParameterError::SmartContract(
                                            "invalid verifying key record circuit_id".into(),
                                        ),
                                    )
                                })?;
                        if payload_circuit_id != record_circuit_id {
                            return Err(InstructionExecutionError::InvalidParameter(
                                InvalidParameterError::SmartContract(
                                    "STARK verifying key circuit_id does not match record".into(),
                                ),
                            ));
                        }
                    }
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
            state_transaction.mark_confidential_registry_dirty();
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
            let parliament_snapshot = match resolve_governance_approval_mode(state_transaction) {
                GovernanceApprovalMode::ParliamentSortitionJit => Some(
                    derive_jit_parliament_snapshot(id, created_height, state_transaction)?,
                ),
                GovernanceApprovalMode::LegacyCouncilEpoch => None,
            };
            let rec = crate::state::GovernanceProposalRecord {
                proposer: authority.clone(),
                kind: kind.clone(),
                created_height,
                status: crate::state::GovernanceProposalStatus::Proposed,
                pipeline,
                parliament_snapshot,
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

    fn compute_runtime_upgrade_proposal_id(
        manifest: &iroha_data_model::runtime::RuntimeUpgradeManifest,
    ) -> Result<[u8; 32], Error> {
        let canonical = manifest.canonical_bytes();
        let manifest_len: u32 = canonical.len().try_into().map_err(|_| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                "runtime upgrade manifest length exceeds 2^32 bytes".into(),
            ))
        })?;
        let mut input = Vec::with_capacity(
            b"iroha:gov:runtime-upgrade:proposal:v1|".len()
                + core::mem::size_of::<u32>()
                + canonical.len(),
        );
        input.extend_from_slice(b"iroha:gov:runtime-upgrade:proposal:v1|");
        input.extend_from_slice(&manifest_len.to_le_bytes());
        input.extend_from_slice(&canonical);
        let digest = Blake2b512::digest(&input);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest[..32]);
        Ok(out)
    }

    impl Execute for gov::ProposeRuntimeUpgradeProposal {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanProposeContractDeployment",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanProposeContractDeployment".into(),
                ));
            }

            if self.manifest.end_height <= self.manifest.start_height {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "runtime upgrade window must satisfy end_height > start_height".into(),
                    ),
                ));
            }
            validate_runtime_upgrade_provenance(&self.manifest, state_transaction)?;
            ensure_runtime_upgrade_abi_version(&self.manifest, state_transaction)?;
            validate_runtime_upgrade_surface(&self.manifest, state_transaction)?;
            ensure_runtime_upgrade_no_overlap(&self.manifest, state_transaction)?;

            let id = compute_runtime_upgrade_proposal_id(&self.manifest)?;
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

            let payload = RuntimeUpgradeProposal {
                manifest: self.manifest.clone(),
            };
            let kind = ProposalKind::RuntimeUpgrade(payload.clone());
            if let Some(existing) = state_transaction.world.governance_proposals.get(&id) {
                let Some(existing_payload) = existing.as_runtime_upgrade() else {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "governance proposal id collision".into(),
                    ));
                };
                if existing_payload.manifest != payload.manifest {
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
            let parliament_snapshot = match resolve_governance_approval_mode(state_transaction) {
                GovernanceApprovalMode::ParliamentSortitionJit => Some(
                    derive_jit_parliament_snapshot(id, created_height, state_transaction)?,
                ),
                GovernanceApprovalMode::LegacyCouncilEpoch => None,
            };
            state_transaction.world.governance_proposals.insert(
                id,
                crate::state::GovernanceProposalRecord {
                    proposer: authority.clone(),
                    kind,
                    created_height,
                    status: crate::state::GovernanceProposalStatus::Proposed,
                    pipeline,
                    parliament_snapshot,
                },
            );

            let runtime_target = self.manifest.id();
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::governance::GovernanceEvent::ProposalSubmitted(
                    iroha_data_model::events::data::governance::GovernanceProposalSubmitted {
                        id,
                        proposer: authority.clone(),
                        namespace: "runtime-upgrade".to_string(),
                        contract_id: hex::encode(runtime_target.0),
                    },
                ),
            ));
            Ok(())
        }
    }

    fn parse_hex32_hint(raw: &str) -> Option<[u8; 32]> {
        let trimmed = raw.trim();
        let without_scheme = if let Some((scheme, rest)) = trimmed.split_once(':') {
            if scheme.is_empty() || scheme.eq_ignore_ascii_case("blake2b32") {
                rest
            } else {
                return None;
            }
        } else {
            trimmed
        };
        let body = without_scheme.trim();
        let body = body
            .strip_prefix("0x")
            .or_else(|| body.strip_prefix("0X"))
            .unwrap_or(body)
            .trim();
        if body.len() != 64 || !body.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        let mut out = [0u8; 32];
        hex::decode_to_slice(body, &mut out).ok()?;
        Some(out)
    }

    impl Execute for gov::CastZkBallot {
        #[allow(clippy::too_many_lines)]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
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
            let parse_ballot_direction = |value: &norito::json::Value| -> Option<u8> {
                if let Some(n) = value.as_u64() {
                    return u8::try_from(n).ok().filter(|v| *v <= 2);
                }
                let raw = value.as_str()?.trim();
                if raw.eq_ignore_ascii_case("aye") {
                    return Some(0);
                }
                if raw.eq_ignore_ascii_case("nay") {
                    return Some(1);
                }
                if raw.eq_ignore_ascii_case("abstain") {
                    return Some(2);
                }
                raw.parse::<u8>().ok().filter(|v| *v <= 2)
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
            } else {
                let value = if let Ok(value) =
                    norito::json::from_str::<norito::json::Value>(self.public_inputs_json.as_str())
                {
                    value
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
                if !value.is_object() {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "public inputs must be a JSON object".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "public inputs must be a JSON object".into(),
                    ));
                }
                Some(value)
            };
            let mut lock_owner: Option<iroha_data_model::account::AccountId> = None;
            let mut lock_amount: Option<u128> = None;
            let mut lock_duration: Option<u64> = None;
            let mut lock_direction: Option<u8> = None;
            let mut root_hint_opt: Option<[u8; 32]> = None;
            let mut nullifier_hint: Option<[u8; 32]> = None;
            if let Some(val) = public_inputs.as_ref() {
                if val.get("durationBlocks").is_some() {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "public inputs must use duration_blocks".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "public inputs must use duration_blocks".into(),
                    ));
                }
                if val.get("root_hint_hex").is_some()
                    || val.get("rootHintHex").is_some()
                    || val.get("rootHint").is_some()
                {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "public inputs must use root_hint".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "public inputs must use root_hint".into(),
                    ));
                }
                if let Some(root_val) = val.get("root_hint") {
                    if !matches!(root_val, norito::json::Value::Null) {
                        let rh_str = root_val.as_str().ok_or_else(|| {
                            state_transaction.world.emit_events(Some(
                                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                        referendum_id: self.election_id.clone(),
                                        reason: "root_hint must be 32-byte hex".into(),
                                    },
                                ),
                            ));
                            InstructionExecutionError::InvariantViolation(
                                "root_hint must be 32-byte hex".into(),
                            )
                        })?;
                        if let Some(parsed) = parse_hex32_hint(rh_str) {
                            root_hint_opt = Some(parsed);
                        } else {
                            state_transaction.world.emit_events(Some(
                                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                        referendum_id: self.election_id.clone(),
                                        reason: "root_hint must be 32-byte hex".into(),
                                    },
                                ),
                            ));
                            return Err(InstructionExecutionError::InvariantViolation(
                                "root_hint must be 32-byte hex".into(),
                            ));
                        }
                    }
                }
                if val.get("nullifier_hex").is_some() || val.get("nullifierHex").is_some() {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "public inputs must use nullifier".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "public inputs must use nullifier".into(),
                    ));
                }
                if let Some(null_val) = val.get("nullifier") {
                    if !matches!(null_val, norito::json::Value::Null) {
                        let hex_str = null_val.as_str().ok_or_else(|| {
                            state_transaction.world.emit_events(Some(
                                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                        referendum_id: self.election_id.clone(),
                                        reason: "nullifier must be 32-byte hex".into(),
                                    },
                                ),
                            ));
                            InstructionExecutionError::InvariantViolation(
                                "nullifier must be 32-byte hex".into(),
                            )
                        })?;
                        if let Some(parsed) = parse_hex32_hint(hex_str) {
                            nullifier_hint = Some(parsed);
                        } else {
                            state_transaction.world.emit_events(Some(
                                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                        referendum_id: self.election_id.clone(),
                                        reason: "nullifier must be 32-byte hex".into(),
                                    },
                                ),
                            ));
                            return Err(InstructionExecutionError::InvariantViolation(
                                "nullifier must be 32-byte hex".into(),
                            ));
                        }
                    }
                }
            }

            if let Some(val) = public_inputs.as_ref() {
                if let Some(owner_val) = val.get("owner") {
                    if !matches!(owner_val, norito::json::Value::Null) {
                        let owner_str_raw = owner_val.as_str().ok_or_else(|| {
                            state_transaction.world.emit_events(Some(
                                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                        referendum_id: self.election_id.clone(),
                                        reason: "owner must be a canonical I105 account id".into(),
                                },
                            ),
                        ));
                        InstructionExecutionError::InvariantViolation(
                            "owner must be a canonical I105 account id".into(),
                        )
                    })?;
                        let owner_str = owner_str_raw.trim();
                        if owner_str != owner_str_raw || owner_str.is_empty() {
                            state_transaction.world.emit_events(Some(
                                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                        referendum_id: self.election_id.clone(),
                                        reason: "owner must use canonical I105 account id form".into(),
                                    },
                                ),
                            ));
                            return Err(InstructionExecutionError::InvariantViolation(
                                "owner must use canonical I105 account id form".into(),
                            ));
                        }

                        let owner_parsed = iroha_data_model::account::AccountId::parse_encoded(
                            owner_str,
                        )
                        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                        .map_err(|_| {
                            state_transaction.world.emit_events(Some(
                                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                        referendum_id: self.election_id.clone(),
                                        reason: "owner must be a canonical I105 account id".into(),
                                    },
                                ),
                            ));
                            InstructionExecutionError::InvariantViolation(
                                "owner must be a canonical I105 account id".into(),
                            )
                        })?;
                        if owner_parsed.to_string() != owner_str {
                            state_transaction.world.emit_events(Some(
                                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                        referendum_id: self.election_id.clone(),
                                        reason: "owner must use canonical I105 account id form".into(),
                                    },
                                ),
                            ));
                            return Err(InstructionExecutionError::InvariantViolation(
                                "owner must use canonical I105 account id form".into(),
                            ));
                        }
                        if lock_owner.is_none() {
                            lock_owner = Some(owner_parsed);
                        }
                    }
                }
                if let Some(amount_val) = val.get("amount") {
                    if !matches!(amount_val, norito::json::Value::Null) {
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
                }
                if let Some(duration_val) = val.get("duration_blocks") {
                    if !matches!(duration_val, norito::json::Value::Null) {
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
                if let Some(direction_val) = val.get("direction") {
                    if !matches!(direction_val, norito::json::Value::Null) {
                        if let Some(parsed) = parse_ballot_direction(direction_val) {
                            lock_direction = Some(parsed);
                        } else {
                            state_transaction.world.emit_events(Some(
                                iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                                    iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                        referendum_id: self.election_id.clone(),
                                        reason: "direction must be Aye, Nay, or Abstain".into(),
                                    },
                                ),
                            ));
                            return Err(InstructionExecutionError::InvariantViolation(
                                "direction must be Aye, Nay, or Abstain".into(),
                            ));
                        }
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
            let domain_tag = if st.domain_tag.is_empty() {
                DEFAULT_NULLIFIER_DOMAIN_TAG.to_string()
            } else {
                st.domain_tag.clone()
            };

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
            let lock_hint_present =
                lock_owner.is_some() || lock_amount.is_some() || lock_duration.is_some();
            if lock_hint_present {
                if lock_owner.is_none() || lock_amount.is_none() || lock_duration.is_none() {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "lock hints must include owner, amount, duration_blocks"
                                    .into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "lock hints must include owner, amount, duration_blocks".into(),
                    ));
                }
            } else if state_transaction.gov.min_bond_amount > 0 {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "lock hints required for governance bond".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "lock hints required for governance bond".into(),
                ));
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
            if crate::zk::is_stark_fri_v1_backend(backend) && !state_transaction.zk.stark.enabled {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "stark verification is disabled in node configuration".into(),
                        },
                    ),
                ));
                return Err(InstructionExecutionError::InvariantViolation(
                    "stark verification is disabled in node configuration".into(),
                ));
            }
            if let Err(err) = enforce_vk_max_proof_bytes("ballot", &vk_rec, proof_bytes.len()) {
                state_transaction.world.emit_events(Some(
                    iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                        iroha_data_model::events::data::governance::GovernanceBallotRejected {
                            referendum_id: self.election_id.clone(),
                            reason: "proof exceeds verifying key max_proof_bytes".into(),
                        },
                    ),
                ));
                return Err(err);
            }
            let proof_box =
                iroha_data_model::proof::ProofBox::new(vk_id.backend.clone(), proof_bytes.clone());
            let verify_report = crate::zk::verify_backend_with_timing_checked(
                backend,
                &proof_box,
                Some(&vk_box),
                &state_transaction.zk,
            );
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
                let _ = governance_slash_percent(
                    &self.election_id,
                    authority,
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
            let inputs = match extract_vote_public_inputs(backend, &proof_bytes) {
                Ok(inputs) => inputs,
                Err(err) => {
                    let _ = governance_slash_percent(
                        &self.election_id,
                        authority,
                        state_transaction.gov.slash_invalid_proof_bps,
                        GovernanceSlashReason::Misconduct,
                        "invalid_proof_inputs",
                        state_transaction,
                    )?;
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "invalid proof inputs".into(),
                            },
                        ),
                    ));
                    return Err(err);
                }
            };
            if let Some(envelope) = inputs.envelope.as_ref() {
                if let Err(err) =
                    validate_vote_envelope_metadata("ballot", backend, envelope, &vk_rec)
                {
                    let _ = governance_slash_percent(
                        &self.election_id,
                        authority,
                        state_transaction.gov.slash_invalid_proof_bps,
                        GovernanceSlashReason::Misconduct,
                        "invalid_proof_inputs",
                        state_transaction,
                    )?;
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "invalid proof inputs".into(),
                            },
                        ),
                    ));
                    return Err(err);
                }
            }
            let (commit_bytes, root_bytes) = match ballot_inputs_from_columns(&inputs.columns) {
                Ok(v) => v,
                Err(err) => {
                    let _ = governance_slash_percent(
                        &self.election_id,
                        authority,
                        state_transaction.gov.slash_invalid_proof_bps,
                        GovernanceSlashReason::Misconduct,
                        "invalid_proof_inputs",
                        state_transaction,
                    )?;
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "invalid proof inputs".into(),
                            },
                        ),
                    ));
                    return Err(err);
                }
            };
            if let Some(root_hint) = root_hint_opt {
                if root_hint != root_bytes {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "root_hint does not match proof".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "root_hint does not match proof".into(),
                    ));
                }
            }
            if root_bytes != st.eligible_root {
                let _ = governance_slash_percent(
                    &self.election_id,
                    authority,
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
            let nullifier = derive_ballot_nullifier(
                &domain_tag,
                &state_transaction.chain_id,
                &self.election_id,
                &commit_bytes,
            );
            if let Some(hint) = nullifier_hint {
                if hint != nullifier {
                    state_transaction.world.emit_events(Some(
                        iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(
                            iroha_data_model::events::data::governance::GovernanceBallotRejected {
                                referendum_id: self.election_id.clone(),
                                reason: "nullifier does not match proof".into(),
                            },
                        ),
                    ));
                    return Err(InstructionExecutionError::InvariantViolation(
                        "nullifier does not match proof".into(),
                    ));
                }
            }

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
                                h_start: rr.h_start,
                                h_end: rr.h_end,
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
                let _ = governance_slash_percent(
                    &self.election_id,
                    authority,
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

            if proof_verified {
                if let (Some(owner), Some(amount), Some(duration_blocks)) =
                    (lock_owner.clone(), lock_amount, lock_duration)
                {
                    let direction = lock_direction.unwrap_or(2);
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
                        direction,
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
            // Record commitment bytes and enforce cap
            st.ciphertexts.push(commit_bytes.to_vec());
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
                        h_start: rr.h_start,
                        h_end: rr.h_end,
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
    ) -> Result<(crate::state::GovernanceProposalRecord, bool), Error> {
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

        Ok((prop, already_enacted))
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
                kotoba: None,
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

    fn enact_runtime_upgrade_proposal(
        state_transaction: &mut StateTransaction<'_, '_>,
        payload: &RuntimeUpgradeProposal,
        proposer: &AccountId,
    ) -> Result<(), Error> {
        let now_h = state_transaction._curr_block.height().get();
        if now_h > payload.manifest.start_height {
            return Err(InstructionExecutionError::InvariantViolation(
                "runtime upgrade start_height already passed before enactment".into(),
            ));
        }

        let id = payload.manifest.id();
        if let Some(existing) = state_transaction.world.runtime_upgrades.get(&id) {
            if existing.manifest != payload.manifest {
                return Err(InstructionExecutionError::InvariantViolation(
                    "runtime upgrade id collision".into(),
                ));
            }
            if matches!(
                existing.status,
                iroha_data_model::runtime::RuntimeUpgradeStatus::Canceled
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "runtime upgrade already canceled".into(),
                ));
            }
            return Ok(());
        }

        validate_runtime_upgrade_provenance(&payload.manifest, state_transaction)?;
        ensure_runtime_upgrade_abi_version(&payload.manifest, state_transaction)?;
        validate_runtime_upgrade_surface(&payload.manifest, state_transaction)?;
        ensure_runtime_upgrade_no_overlap(&payload.manifest, state_transaction)?;

        let mut status = iroha_data_model::runtime::RuntimeUpgradeStatus::Proposed;
        if now_h == payload.manifest.start_height {
            status = iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(now_h);
        }
        state_transaction.world.runtime_upgrades.insert(
            id,
            iroha_data_model::runtime::RuntimeUpgradeRecord {
                manifest: payload.manifest.clone(),
                status,
                proposer: proposer.clone(),
                created_height: now_h,
            },
        );
        state_transaction.world.emit_events(Some(
            iroha_data_model::events::data::runtime_upgrade::RuntimeUpgradeEvent::Proposed(
                iroha_data_model::events::data::runtime_upgrade::RuntimeUpgradeProposed {
                    id,
                    abi_version: payload.manifest.abi_version,
                    start_height: payload.manifest.start_height,
                    end_height: payload.manifest.end_height,
                },
            ),
        ));
        #[cfg(feature = "telemetry")]
        {
            state_transaction
                .telemetry
                .inc_runtime_upgrade_event("proposed");
        }
        if matches!(
            status,
            iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(_)
        ) {
            state_transaction.world.emit_events(Some(
                iroha_data_model::events::data::runtime_upgrade::RuntimeUpgradeEvent::Activated(
                    iroha_data_model::events::data::runtime_upgrade::RuntimeUpgradeActivated {
                        id,
                        abi_version: payload.manifest.abi_version,
                        at_height: now_h,
                    },
                ),
            ));
            #[cfg(feature = "telemetry")]
            {
                state_transaction
                    .telemetry
                    .inc_runtime_upgrade_event("activated");
            }
        }
        Ok(())
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
            let (proposal, already_enacted) = load_proposal(state_transaction, pid, &pid_hex)?;
            match &proposal.kind {
                ProposalKind::DeployContract(payload) => {
                    let payload = payload.clone();
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

                    let instance_bound_new =
                        bind_contract_instance(state_transaction, &payload, key)?;
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
                }
                ProposalKind::RuntimeUpgrade(payload) => {
                    enact_runtime_upgrade_proposal(state_transaction, payload, &proposal.proposer)?;
                }
            }

            if !already_enacted {
                mark_proposal_enacted(state_transaction, pid);
            }

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
            ensure_contract_namespace_governance(authority, state_transaction, self.namespace())?;
            let key = *self.code_hash();
            let ns = self.namespace().clone();
            let cid = self.contract_id().clone();
            let ns_key = (ns, cid);
            // Enforce namespace uniqueness when governance protects namespaces to prevent cross-namespace rebinding.
            let protected = protected_contract_namespaces(state_transaction);
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
            let Some(manifest) = state_transaction
                .world
                .contract_manifests
                .get(&key)
                .cloned()
            else {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract("manifest for code_hash not found".into()),
                ));
            };
            let needs_trigger_registration =
                manifest.entrypoints.as_ref().is_some_and(|entrypoints| {
                    entrypoints
                        .iter()
                        .any(|entrypoint| !entrypoint.triggers.is_empty())
                });
            if needs_trigger_registration {
                let code_bytes = state_transaction
                    .world
                    .contract_code
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| {
                        InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "contract bytecode is required to register manifest triggers"
                                    .into(),
                            ),
                        )
                    })?;
                register_manifest_triggers(
                    authority,
                    state_transaction,
                    self.namespace(),
                    self.contract_id(),
                    &code_bytes,
                    &manifest,
                )?;
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
            ensure_contract_namespace_governance(authority, state_transaction, self.namespace())?;
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
            let trigger_ids: Vec<TriggerId> = state_transaction
                .world
                .contract_manifests
                .get(&prev_hash)
                .and_then(|manifest| manifest.entrypoints.as_ref())
                .map(|entrypoints| {
                    entrypoints
                        .iter()
                        .flat_map(|entrypoint| {
                            entrypoint
                                .triggers
                                .iter()
                                .map(|descriptor| descriptor.id.clone())
                        })
                        .collect()
                })
                .unwrap_or_default();
            for trigger_id in trigger_ids {
                if state_transaction.world.triggers.remove(&trigger_id) {
                    crate::smartcontracts::isi::triggers::isi::remove_trigger_associated_permissions(
                        state_transaction,
                        &trigger_id,
                    );
                    state_transaction
                        .world
                        .emit_events(Some(TriggerEvent::Deleted(trigger_id)));
                }
            }
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
            // ZK tally: require finalized election and use its tally for approve/reject.
            if let Some(e) = state_transaction.world.elections.get(&self.referendum_id) {
                if !e.finalized {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "election tally not finalized".into(),
                    ));
                }
                if e.tally.len() < 2 {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "election tally missing options".into(),
                    ));
                }
                approve = u128::from(e.tally[0]);
                reject = u128::from(e.tally[1]);
            }
            // Note: closing by height is automatic in State::block; no need to change status here.
            // Decide and emit Approved/Rejected with thresholds
            let turnout = approve.saturating_add(reject);
            let num = state_transaction.gov.approval_threshold_q_num;
            let den = state_transaction.gov.approval_threshold_q_den.max(1);
            let decision_approve = if turnout >= state_transaction.gov.min_turnout {
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
            if ctx.persist_epoch_bodies {
                persist_parliament_bodies_if_missing(&ctx, state_transaction);
            }

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
        proposal_kind: ProposalKind,
        bodies: iroha_data_model::governance::types::ParliamentBodies,
        roster: iroha_data_model::governance::types::ParliamentRoster,
        epoch: u64,
        persist_epoch_bodies: bool,
        now_h: u64,
        quorum_bps: u16,
    }

    fn load_approval_context(
        req: &gov::ApproveGovernanceProposal,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<ApprovalContext, Error> {
        let proposal = state_transaction
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
        let now_h = state_transaction._curr_block.height().get();
        let (bodies, epoch, persist_epoch_bodies) =
            if resolve_governance_approval_mode(state_transaction)
                == GovernanceApprovalMode::ParliamentSortitionJit
            {
                if let Some(snapshot) = proposal.parliament_snapshot.as_ref() {
                    if snapshot.bodies.selection_epoch != snapshot.selection_epoch {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "proposal parliament snapshot epoch mismatch".into(),
                        ));
                    }
                    let roster_root = compute_parliament_roster_root(&snapshot.bodies)?;
                    if roster_root != snapshot.roster_root {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "proposal parliament snapshot commitment mismatch".into(),
                        ));
                    }
                    (snapshot.bodies.clone(), snapshot.selection_epoch, false)
                } else {
                    let term_blocks = state_transaction.gov.parliament_term_blocks.max(1);
                    let fallback_epoch = now_h.saturating_sub(1).saturating_div(term_blocks);
                    let council = state_transaction
                        .world
                        .council
                        .get(&fallback_epoch)
                        .cloned()
                        .ok_or_else(|| {
                            InstructionExecutionError::InvariantViolation(
                            "proposal parliament snapshot missing and council roster unavailable"
                                .into(),
                        )
                        })?;
                    let beacon = derive_epoch_parliament_beacon(fallback_epoch, state_transaction);
                    let bodies = state_transaction
                        .world
                        .parliament_bodies
                        .get(&fallback_epoch)
                        .cloned()
                        .unwrap_or_else(|| {
                            derive_parliament_bodies(
                                &state_transaction.gov,
                                &state_transaction.chain_id,
                                fallback_epoch,
                                &beacon,
                                &council,
                            )
                        });
                    (bodies, fallback_epoch, true)
                }
            } else {
                let term_blocks = state_transaction.gov.parliament_term_blocks.max(1);
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
                let beacon = derive_epoch_parliament_beacon(epoch, state_transaction);
                let bodies = state_transaction
                    .world
                    .parliament_bodies
                    .get(&epoch)
                    .cloned()
                    .unwrap_or_else(|| {
                        derive_parliament_bodies(
                            &state_transaction.gov,
                            &state_transaction.chain_id,
                            epoch,
                            &beacon,
                            &council,
                        )
                    });
                (bodies, epoch, true)
            };
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
            proposal_kind: proposal.kind,
            bodies,
            roster,
            epoch,
            persist_epoch_bodies,
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

    fn runtime_upgrade_open_gate_met(
        approvals: &crate::state::GovernanceStageApprovals,
        epoch: u64,
    ) -> bool {
        [
            ParliamentBody::RulesCommittee,
            ParliamentBody::AgendaCouncil,
            ParliamentBody::InterestPanel,
            ParliamentBody::ReviewPanel,
            ParliamentBody::PolicyJury,
            ParliamentBody::OversightCommittee,
            ParliamentBody::FmaCommittee,
        ]
        .into_iter()
        .all(|body| approvals.quorum_met(body, epoch))
    }

    fn maybe_open_referendum(
        ctx: &ApprovalContext,
        approvals: &crate::state::GovernanceStageApprovals,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) {
        let open_gate_met = match &ctx.proposal_kind {
            ProposalKind::DeployContract(_) => {
                approvals.quorum_met(ParliamentBody::RulesCommittee, ctx.epoch)
                    && approvals.quorum_met(ParliamentBody::AgendaCouncil, ctx.epoch)
            }
            ProposalKind::RuntimeUpgrade(_) => runtime_upgrade_open_gate_met(approvals, ctx.epoch),
        };
        if open_gate_met
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
            let beacon = derive_epoch_parliament_beacon(rec.epoch, state_transaction);
            let bodies = derive_parliament_bodies(
                &state_transaction.gov,
                &state_transaction.chain_id,
                rec.epoch,
                &beacon,
                &rec,
            );
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
        _state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        // Runtime upgrades in the first release keep ABI v1 fixed; no version bumping is allowed.
        policy_for_abi_version(manifest.abi_version)?;
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
        if new.backend != old.backend {
            return Err(InstructionExecutionError::InvariantViolation(
                "verifying key backend cannot change".into(),
            ));
        }
        let id_backend = id.backend.as_str();
        match new.backend {
            BackendTag::Halo2IpaPasta => {
                if !new.curve.eq_ignore_ascii_case("pallas") {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "verifying key curve must be \"pallas\"".into(),
                        ),
                    ));
                }
                if !id_backend.starts_with("halo2/") || id_backend.contains("bn254") {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "verifying key id backend must target halo2 IPA (no bn254)".into(),
                        ),
                    ));
                }
            }
            BackendTag::Stark => {
                if !new.curve.eq_ignore_ascii_case("goldilocks") {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "verifying key curve must be \"goldilocks\"".into(),
                        ),
                    ));
                }
                if !crate::zk::is_stark_fri_v1_backend(id_backend) {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "verifying key id backend must target stark/fri".into(),
                        ),
                    ));
                }
            }
            _ => {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verifying key backend must be Halo2IpaPasta or Stark".into(),
                    ),
                ));
            }
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
            if new.backend == BackendTag::Stark {
                #[cfg(not(feature = "zk-stark"))]
                {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "verifying key backend Stark is not enabled".into(),
                        ),
                    ));
                }

                #[cfg(feature = "zk-stark")]
                {
                    use crate::zk_stark::{
                        STARK_HASH_POSEIDON2_V1, STARK_HASH_SHA256_V1, StarkFriVerifyingKeyV1,
                    };
                    let expected_hash_fn = if id_backend == crate::zk::ZK_BACKEND_STARK_FRI_V1 {
                        None
                    } else if id_backend.contains("/sha256-") {
                        Some(STARK_HASH_SHA256_V1)
                    } else if id_backend.contains("/poseidon2-") {
                        Some(STARK_HASH_POSEIDON2_V1)
                    } else {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "unsupported stark/fri backend variant".into(),
                            ),
                        ));
                    };
                    let payload: StarkFriVerifyingKeyV1 = norito::decode_from_bytes(&vk.bytes)
                        .map_err(|_| {
                            InstructionExecutionError::InvalidParameter(
                                InvalidParameterError::SmartContract(
                                    "invalid STARK verifying key payload".into(),
                                ),
                            )
                        })?;
                    if payload.version != 1 {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "unsupported STARK verifying key payload version".into(),
                            ),
                        ));
                    }
                    if payload.hash_fn != STARK_HASH_SHA256_V1
                        && payload.hash_fn != STARK_HASH_POSEIDON2_V1
                    {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "unsupported STARK verifying key hash_fn".into(),
                            ),
                        ));
                    }
                    if let Some(expected_hash_fn) = expected_hash_fn {
                        if payload.hash_fn != expected_hash_fn {
                            return Err(InstructionExecutionError::InvalidParameter(
                                InvalidParameterError::SmartContract(
                                    "STARK verifying key hash_fn mismatch".into(),
                                ),
                            ));
                        }
                    }
                    let payload_circuit_id =
                        normalize_stark_fri_circuit_id(id_backend, &payload.circuit_id)
                            .ok_or_else(|| {
                                InstructionExecutionError::InvalidParameter(
                                    InvalidParameterError::SmartContract(
                                        "invalid STARK verifying key circuit_id".into(),
                                    ),
                                )
                            })?;
                    let record_circuit_id =
                        normalize_stark_fri_circuit_id(id_backend, &new.circuit_id).ok_or_else(
                            || {
                                InstructionExecutionError::InvalidParameter(
                                    InvalidParameterError::SmartContract(
                                        "invalid verifying key record circuit_id".into(),
                                    ),
                                )
                            },
                        )?;
                    if payload_circuit_id != record_circuit_id {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "STARK verifying key circuit_id does not match record".into(),
                            ),
                        ));
                    }
                }
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
            state_transaction.mark_confidential_registry_dirty();
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
            state_transaction.mark_confidential_registry_dirty();
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
            state_transaction.mark_confidential_registry_dirty();
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
            state_transaction.mark_confidential_registry_dirty();
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
            state_transaction.mark_confidential_registry_dirty();
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
                    "proofs for this backend must use OpenVerifyEnvelope payload".into(),
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
            let expects_envelope = backend_label.starts_with("halo2/")
                || crate::zk::is_stark_fri_v1_backend(backend_label);

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
                if !circuit_id_matches(
                    attachment.backend.as_str(),
                    &rec.circuit_id,
                    &env.circuit_id,
                ) {
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
        if crate::zk::is_stark_fri_v1_backend(attachment.backend.as_str())
            && !state_transaction.zk.stark.enabled
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "stark verification is disabled in node configuration".into(),
            ));
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

        let report = crate::zk::verify_backend_with_timing_checked(
            attachment.backend.as_str(),
            proof,
            vk_box.as_ref(),
            &state_transaction.zk,
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

    fn resolve_asset_vk(
        state_transaction: &StateTransaction<'_, '_>,
        binding: Option<&crate::state::ZkAssetVerifierBinding>,
        attachment: &iroha_data_model::proof::ProofAttachment,
    ) -> Result<
        (
            iroha_data_model::proof::VerifyingKeyBox,
            Option<VerifyingKeyRecord>,
        ),
        Error,
    > {
        if attachment.vk_ref.is_none() && attachment.vk_inline.is_none() {
            return Err(InstructionExecutionError::InvariantViolation(
                "proof missing verifying key reference or inline key".into(),
            ));
        }
        let record = if let Some(binding) = binding {
            let record = state_transaction
                .world
                .verifying_keys
                .get(&binding.id)
                .cloned()
                .ok_or_else(|| {
                    FindError::Permission(Box::new(Permission::new(
                        "VerifyingKeyMissing".into(),
                        iroha_primitives::json::Json::from(
                            format!("{}::{}", binding.id.backend, binding.id.name).as_str(),
                        ),
                    )))
                })?;
            if record.status != ConfidentialStatus::Active {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key is not active".into(),
                ));
            }
            if record.commitment != binding.commitment {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key commitment mismatch".into(),
                ));
            }
            Some(record)
        } else if let Some(vk_ref) = &attachment.vk_ref {
            let record = state_transaction
                .world
                .verifying_keys
                .get(vk_ref)
                .cloned()
                .ok_or_else(|| {
                    FindError::Permission(Box::new(Permission::new(
                        "VerifyingKeyMissing".into(),
                        iroha_primitives::json::Json::from(
                            format!("{}::{}", vk_ref.backend, vk_ref.name).as_str(),
                        ),
                    )))
                })?;
            if record.status != ConfidentialStatus::Active {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verifying key is not active".into(),
                ));
            }
            Some(record)
        } else {
            None
        };
        if let (Some(record), Some(inline)) = (record.as_ref(), attachment.vk_inline.as_ref()) {
            let inline_commitment = hash_vk(inline);
            if record.commitment != inline_commitment {
                return Err(InstructionExecutionError::InvariantViolation(
                    "inline verifying key commitment mismatch".into(),
                ));
            }
        }
        let vk_box = if let Some(inline) = attachment.vk_inline.clone() {
            inline
        } else if let Some(record) = record.as_ref() {
            record.key.clone().ok_or_else(|| {
                InstructionExecutionError::InvariantViolation("verifying key bytes missing".into())
            })?
        } else {
            return Err(InstructionExecutionError::InvariantViolation(
                "verifying key bytes missing".into(),
            ));
        };
        if vk_box.backend != attachment.backend {
            return Err(InstructionExecutionError::InvariantViolation(
                "verifying key backend mismatch".into(),
            ));
        }
        Ok((vk_box, record))
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
            let attachment = self.proof();
            if attachment.backend != attachment.proof.backend {
                return Err(InstructionExecutionError::InvariantViolation(
                    "proof backend mismatch".into(),
                ));
            }
            if let Some(binding) = st.vk_transfer.as_ref() {
                enforce_vk_binding(binding, attachment)?;
            }
            if crate::zk::is_stark_fri_v1_backend(attachment.backend.as_str())
                && !state_transaction.zk.stark.enabled
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    "stark verification is disabled in node configuration".into(),
                ));
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
            let (vk_box, vk_record) =
                resolve_asset_vk(state_transaction, st.vk_transfer.as_ref(), attachment)?;
            if crate::zk::is_stark_fri_v1_backend(attachment.backend.as_str()) {
                // For `stark/fri` we require the canonical OpenVerifyEnvelope wrapper so the
                // proof bytes are bound to `(backend, circuit_id, vk_hash, public_inputs)` and we
                // can validate metadata against the configured verifying key.
                let env: ZkOpenVerifyEnvelope = norito::decode_from_bytes(&attachment.proof.bytes)
                    .map_err(|_| {
                        InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "invalid OpenVerifyEnvelope payload".into(),
                            ),
                        )
                    })?;
                if env.backend != BackendTag::Stark {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "unexpected OpenVerifyEnvelope backend tag".into(),
                    ));
                }
                if let Some(record) = vk_record.as_ref() {
                    if !circuit_id_matches(
                        attachment.backend.as_str(),
                        &record.circuit_id,
                        &env.circuit_id,
                    ) {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "transfer verifying key circuit mismatch".into(),
                        ));
                    }
                    if record.public_inputs_schema_hash != [0u8; 32] {
                        let observed_hash: [u8; 32] = CryptoHash::new(&env.public_inputs).into();
                        if record.public_inputs_schema_hash != observed_hash {
                            return Err(InstructionExecutionError::InvariantViolation(
                                "public inputs schema hash mismatch".into(),
                            ));
                        }
                    }
                    if env.vk_hash != [0u8; 32] && env.vk_hash != record.commitment {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "verifying key commitment mismatch".into(),
                        ));
                    }
                }
            }
            let proof_len = attachment.proof.bytes.len();
            if let Some(record) = vk_record.as_ref() {
                enforce_vk_max_proof_bytes("transfer", record, proof_len)?;
            }
            state_transaction.register_confidential_proof(proof_len)?;
            let report = crate::zk::verify_backend_with_timing_checked(
                attachment.backend.as_str(),
                &attachment.proof,
                Some(&vk_box),
                &state_transaction.zk,
            );
            let timeout_budget = state_transaction.zk.verify_timeout;
            if timeout_budget > Duration::ZERO && report.elapsed > timeout_budget {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "transfer proof verification exceeded timeout".into(),
                    ),
                ));
            }
            if !report.ok {
                return Err(InstructionExecutionError::InvariantViolation(
                    "invalid transfer proof".into(),
                ));
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
            let attachment = self.proof();
            if attachment.backend != attachment.proof.backend {
                return Err(InstructionExecutionError::InvariantViolation(
                    "proof backend mismatch".into(),
                ));
            }
            if let Some(binding) = st.vk_unshield.as_ref() {
                enforce_vk_binding(binding, attachment)?;
            }
            if crate::zk::is_stark_fri_v1_backend(attachment.backend.as_str())
                && !state_transaction.zk.stark.enabled
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    "stark verification is disabled in node configuration".into(),
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
            let (vk_box, vk_record) =
                resolve_asset_vk(state_transaction, st.vk_unshield.as_ref(), attachment)?;
            if crate::zk::is_stark_fri_v1_backend(attachment.backend.as_str()) {
                // For `stark/fri` we require the canonical OpenVerifyEnvelope wrapper so the
                // proof bytes are bound to `(backend, circuit_id, vk_hash, public_inputs)` and we
                // can validate metadata against the configured verifying key.
                let env: ZkOpenVerifyEnvelope = norito::decode_from_bytes(&attachment.proof.bytes)
                    .map_err(|_| {
                        InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "invalid OpenVerifyEnvelope payload".into(),
                            ),
                        )
                    })?;
                if env.backend != BackendTag::Stark {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "unexpected OpenVerifyEnvelope backend tag".into(),
                    ));
                }
                if let Some(record) = vk_record.as_ref() {
                    if !circuit_id_matches(
                        attachment.backend.as_str(),
                        &record.circuit_id,
                        &env.circuit_id,
                    ) {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "unshield verifying key circuit mismatch".into(),
                        ));
                    }
                    if record.public_inputs_schema_hash != [0u8; 32] {
                        let observed_hash: [u8; 32] = CryptoHash::new(&env.public_inputs).into();
                        if record.public_inputs_schema_hash != observed_hash {
                            return Err(InstructionExecutionError::InvariantViolation(
                                "public inputs schema hash mismatch".into(),
                            ));
                        }
                    }
                    if env.vk_hash != [0u8; 32] && env.vk_hash != record.commitment {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "verifying key commitment mismatch".into(),
                        ));
                    }
                }
            }
            let proof_len = attachment.proof.bytes.len();
            if let Some(record) = vk_record.as_ref() {
                enforce_vk_max_proof_bytes("unshield", record, proof_len)?;
            }
            state_transaction.register_confidential_proof(proof_len)?;
            let report = crate::zk::verify_backend_with_timing_checked(
                attachment.backend.as_str(),
                &attachment.proof,
                Some(&vk_box),
                &state_transaction.zk,
            );
            let timeout_budget = state_transaction.zk.verify_timeout;
            if timeout_budget > Duration::ZERO && report.elapsed > timeout_budget {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "unshield proof verification exceeded timeout".into(),
                    ),
                ));
            }
            if !report.ok {
                return Err(InstructionExecutionError::InvariantViolation(
                    "invalid unshield proof".into(),
                ));
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
            VerifyingKeyRecord,
        ),
        Error,
    > {
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
        ensure_voting_circuit_role(
            "ballot",
            backend,
            &record.circuit_id,
            VOTING_BALLOT_CIRCUIT_ID,
        )?;
        Ok((vk_id, vk_box, record))
    }

    fn resolve_tally_vk(
        st: &crate::state::ElectionState,
        attachment: &iroha_data_model::proof::ProofAttachment,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<
        (
            iroha_data_model::proof::VerifyingKeyId,
            iroha_data_model::proof::VerifyingKeyBox,
            VerifyingKeyRecord,
        ),
        Error,
    > {
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
        ensure_voting_circuit_role(
            "tally",
            backend,
            &record.circuit_id,
            VOTING_TALLY_CIRCUIT_ID,
        )?;
        Ok((vk_id, vk_box, record))
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
            if self.end_ts() < self.start_ts() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "end_ts must be >= start_ts".into(),
                ));
            }
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
            // Require ballot/tally verifying keys to be attested and active.
            let ballot_vk_id = self.vk_ballot().clone();
            let ballot_rec = state_transaction
                .world
                .verifying_keys
                .get(&ballot_vk_id)
                .cloned()
                .ok_or_else(|| {
                    InstructionExecutionError::InvariantViolation(
                        "ballot verifying key not found".into(),
                    )
                })?;
            ensure_voting_circuit_role(
                "ballot",
                ballot_vk_id.backend.as_str(),
                &ballot_rec.circuit_id,
                VOTING_BALLOT_CIRCUIT_ID,
            )?;
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
            ensure_voting_circuit_role(
                "tally",
                tally_vk_id.backend.as_str(),
                &tally_rec.circuit_id,
                VOTING_TALLY_CIRCUIT_ID,
            )?;
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
        #[allow(clippy::too_many_lines)]
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
            let now_ms = u64::try_from(state_transaction._curr_block.creation_time().as_millis())
                .unwrap_or(u64::MAX);
            if now_ms < st.start_ts || now_ms > st.end_ts {
                return Err(InstructionExecutionError::InvariantViolation(
                    "election not active".into(),
                ));
            }
            if self.ballot_proof.backend != self.ballot_proof.proof.backend {
                return Err(InstructionExecutionError::InvariantViolation(
                    "proof backend mismatch".into(),
                ));
            }
            let (vk_id, vk_box, vk_rec) =
                resolve_ballot_vk(&st, &self.ballot_proof, state_transaction)?;
            let backend = vk_id.backend.as_str();
            if crate::zk::is_stark_fri_v1_backend(backend) && !state_transaction.zk.stark.enabled {
                return Err(InstructionExecutionError::InvariantViolation(
                    "stark verification is disabled in node configuration".into(),
                ));
            }

            let proof_len = self.ballot_proof.proof.bytes.len();
            enforce_vk_max_proof_bytes("ballot", &vk_rec, proof_len)?;
            state_transaction.register_confidential_proof(proof_len)?;
            let report = crate::zk::verify_backend_with_timing_checked(
                backend,
                &self.ballot_proof.proof,
                Some(&vk_box),
                &state_transaction.zk,
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
            let inputs = extract_vote_public_inputs(backend, &self.ballot_proof.proof.bytes)?;
            if let Some(envelope) = inputs.envelope.as_ref() {
                validate_vote_envelope_metadata("ballot", backend, envelope, &vk_rec)?;
            }
            let (commit_bytes, root_bytes) = ballot_inputs_from_columns(&inputs.columns)?;
            if root_bytes != st.eligible_root {
                return Err(InstructionExecutionError::InvariantViolation(
                    "stale or unknown eligibility root".into(),
                ));
            }
            let domain_tag = if st.domain_tag.is_empty() {
                DEFAULT_NULLIFIER_DOMAIN_TAG.to_string()
            } else {
                st.domain_tag.clone()
            };
            let expected_nullifier = derive_ballot_nullifier(
                &domain_tag,
                &state_transaction.chain_id,
                &id,
                &commit_bytes,
            );
            if *self.nullifier() != expected_nullifier {
                return Err(InstructionExecutionError::InvariantViolation(
                    "ballot nullifier does not match proof".into(),
                ));
            }
            if self.ciphertext().len() != commit_bytes.len()
                || self.ciphertext().as_slice() != commit_bytes.as_slice()
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    "ciphertext does not match proof".into(),
                ));
            }
            if !st.ballot_nullifiers.insert(expected_nullifier) {
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
            let now_ms = u64::try_from(state_transaction._curr_block.creation_time().as_millis())
                .unwrap_or(u64::MAX);
            if now_ms < st.end_ts {
                return Err(InstructionExecutionError::InvariantViolation(
                    "election still active".into(),
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
            if att.backend != att.proof.backend {
                return Err(InstructionExecutionError::InvariantViolation(
                    "proof backend mismatch".into(),
                ));
            }
            let (vk_id, vk_box, vk_rec) = resolve_tally_vk(&st, att, state_transaction)?;
            let backend = vk_id.backend.as_str();
            if crate::zk::is_stark_fri_v1_backend(backend) && !state_transaction.zk.stark.enabled {
                return Err(InstructionExecutionError::InvariantViolation(
                    "stark verification is disabled in node configuration".into(),
                ));
            }
            let proof_len = att.proof.bytes.len();
            enforce_vk_max_proof_bytes("tally", &vk_rec, proof_len)?;
            state_transaction.register_confidential_proof(proof_len)?;
            let report = crate::zk::verify_backend_with_timing_checked(
                backend,
                &att.proof,
                Some(&vk_box),
                &state_transaction.zk,
            );
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
            let inputs = extract_vote_public_inputs(backend, &att.proof.bytes)?;
            if let Some(envelope) = inputs.envelope.as_ref() {
                validate_vote_envelope_metadata("tally", backend, envelope, &vk_rec)?;
            }
            let expected_tally = tally_from_columns(&inputs.columns, expected_tally_len)?;
            if expected_tally != *self.tally() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "tally does not match proof".into(),
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
                pop: Some(self.pop.clone()),
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
            let candidate_id = derive_validator_key_id(peer_id.public_key());
            let existing_pop = world
                .consensus_keys
                .get(&candidate_id)
                .and_then(|record| record.pop.clone());
            let lifecycle_record = ConsensusKeyRecord {
                id: candidate_id,
                public_key: peer_id.public_key().clone(),
                pop: existing_pop,
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

            if state_transaction.world.domains.get(&canonical_id).is_some() {
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::DomainId(canonical_id.clone()),
                }
                .into());
            }
            let now_ms = state_transaction.block_unix_timestamp_ms();
            let bootstrap_domain_name_lease = state_transaction._curr_block.is_genesis()
                && state_transaction.block_hashes.is_empty()
                && state_transaction
                    .world
                    .domain(&iroha_genesis::GENESIS_DOMAIN_ID)
                    .map(|domain| domain.owned_by() == authority)
                    .unwrap_or(false);
            let should_seed_domain_name_lease = match crate::sns::active_domain_owner(
                state_transaction.world(),
                &canonical_id,
                now_ms,
            ) {
                Some(owner) if owner == *authority => false,
                Some(owner) => {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "active SNS domain-name lease for `{canonical_id}` is owned by `{owner}`, not `{authority}`"
                        )
                        .into(),
                    ));
                }
                None if bootstrap_domain_name_lease => true,
                None => {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "active SNS domain-name lease is required before registering `{canonical_id}`"
                        )
                        .into(),
                    ));
                }
            };
            let selector =
                iroha_data_model::account::AccountDomainSelector::from_domain(&canonical_id)
                    .map_err(|err| {
                        InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(err.code_str().into()),
                        )
                    })?;
            if let Some(existing) = state_transaction.world.domain_selectors.get(&selector) {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("Domain selector {selector:?} already bound to domain {existing}")
                        .into(),
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
            if world
                .domain_selectors
                .insert(selector, canonical_id.clone())
                .is_some()
            {
                world.domains.remove(canonical_id.clone());
                return Err(InstructionExecutionError::InvariantViolation(
                    "Domain selector already registered".to_owned().into(),
                ));
            }
            if should_seed_domain_name_lease {
                let lease_selector =
                    crate::sns::selector_for_domain(&canonical_id).map_err(|err| {
                        InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(err.to_string()),
                        )
                    })?;
                let address = iroha_data_model::account::AccountAddress::from_account_id(authority)
                    .map_err(|err| {
                        InstructionExecutionError::InvariantViolation(
                            format!(
                                "failed to derive genesis account address for SNS bootstrap: {err}"
                            )
                            .into(),
                        )
                    })?;
                let record = iroha_data_model::sns::NameRecordV1::new(
                    lease_selector.clone(),
                    authority.clone(),
                    vec![iroha_data_model::sns::NameControllerV1::account(&address)],
                    0,
                    0,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    Metadata::default(),
                );
                world.smart_contract_state.insert(
                    crate::sns::record_storage_key(&lease_selector),
                    norito::codec::Encode::encode(&record),
                );
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
            if !has_permission(
                &state_transaction.world,
                authority,
                "CanManageLaneRelayEmergency",
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "not permitted: CanManageLaneRelayEmergency".into(),
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

            let lane_id = *self.lane_id();
            if state_transaction.nexus.lane_config.entry(lane_id).is_none() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "unknown lane id {}",
                        lane_id.as_u32()
                    )),
                ));
            }

            let mut peers = self.peers().clone();
            peers.sort();
            peers.dedup();
            if peers.is_empty() {
                iroha_logger::info!(
                    %authority,
                    lane_id = lane_id.as_u32(),
                    "clearing lane relay emergency override"
                );
                state_transaction
                    .world
                    .lane_relay_emergency_validators
                    .remove(lane_id);
                return Ok(());
            }

            let current_height = state_transaction.block_height();
            let expires_at_height = self.expires_at_height().ok_or_else(|| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    "lane relay emergency override requires expires_at_height for non-empty peer rosters"
                        .into(),
                ))
            })?;
            if expires_at_height < current_height {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "lane relay emergency override expiry {} is already in the past at current height {}",
                        expires_at_height, current_height
                    )),
                ));
            }
            let max_ttl_blocks = u64::from(
                state_transaction
                    .nexus
                    .lane_relay_emergency
                    .max_ttl_blocks
                    .get(),
            );
            if expires_at_height.saturating_sub(current_height) > max_ttl_blocks {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "lane relay emergency override expiry {} exceeds max_ttl_blocks {} at current height {}",
                        expires_at_height, max_ttl_blocks, current_height
                    )),
                ));
            }

            let present_peers: BTreeSet<_> =
                state_transaction.world.peers().iter().cloned().collect();
            let topology_peers: BTreeSet<_> = state_transaction
                .commit_topology()
                .get()
                .iter()
                .cloned()
                .collect();
            let enforce_topology_membership = !topology_peers.is_empty();
            for peer in &peers {
                if !present_peers.contains(peer) {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(format!(
                            "lane relay emergency peer {} is not registered",
                            peer.public_key()
                        )),
                    ));
                }
                if enforce_topology_membership && !topology_peers.contains(peer) {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(format!(
                            "lane relay emergency peer {} is not in the current commit topology",
                            peer.public_key()
                        )),
                    ));
                }
                if crate::state::live_consensus_key_pop_for_peer(
                    &state_transaction.world,
                    peer,
                    current_height,
                )
                .is_none()
                {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(format!(
                            "lane relay emergency peer {} does not have a live consensus key",
                            peer.public_key()
                        )),
                    ));
                }
            }

            iroha_logger::warn!(
                %authority,
                lane_id = lane_id.as_u32(),
                peer_count = peers.len(),
                expires_at_height,
                "setting lane relay emergency override"
            );
            state_transaction
                .world
                .lane_relay_emergency_validators
                .insert(
                    lane_id,
                    LaneRelayEmergencyValidatorSet {
                        peers,
                        expires_at_height,
                        metadata: self.metadata().clone(),
                    },
                );
            Ok(())
        }
    }

    impl Execute for nexus::RegisterVerifiedLaneRelay {
        #[metrics(+"register_verified_lane_relay")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !state_transaction.nexus.enabled {
                return Err(InstructionExecutionError::InvariantViolation(
                    "verified lane relay registration requires nexus.enabled=true".into(),
                ));
            }

            let envelope = self.envelope().clone();
            envelope.verify().map_err(|err| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    format!("lane relay envelope failed verification: {err}"),
                ))
            })?;
            envelope.verify_fastpq_proof_material().map_err(|err| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    format!("lane relay envelope FASTPQ binding failed verification: {err}"),
                ))
            })?;

            let Some(lane) = state_transaction
                .nexus
                .lane_catalog
                .lanes()
                .iter()
                .find(|entry| entry.id == envelope.lane_id)
            else {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "unknown lane id {}",
                        envelope.lane_id.as_u32()
                    )),
                )
                .into());
            };
            if lane.dataspace_id != envelope.dataspace_id {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "lane {} belongs to dataspace {}, not {}",
                        envelope.lane_id.as_u32(),
                        lane.dataspace_id.as_u64(),
                        envelope.dataspace_id.as_u64()
                    )),
                )
                .into());
            }
            if !state_transaction
                .nexus
                .dataspace_catalog
                .entries()
                .iter()
                .any(|entry| entry.id == envelope.dataspace_id)
            {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "unknown dataspace id {}",
                        envelope.dataspace_id.as_u64()
                    )),
                )
                .into());
            }

            let manifest_root = envelope.manifest_root.ok_or_else(|| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    "lane relay envelope is missing manifest_root".into(),
                ))
            })?;
            if manifest_root.iter().all(|byte| *byte == 0) {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "lane relay manifest_root cannot be zeroed".into(),
                    ),
                )
                .into());
            }

            let proof_blob = self.proof_blob().clone();
            if proof_blob.payload.is_empty() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verified lane relay proof payload is empty".into(),
                    ),
                )
                .into());
            }

            let verified_at_height = state_transaction.block_height();
            if let Some(expiry_slot) = proof_blob.expiry_slot
                && verified_at_height > expiry_slot
            {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(format!(
                        "verified lane relay proof expired at slot {expiry_slot}"
                    )),
                )
                .into());
            }
            if !proof_matches_manifest(&proof_blob, envelope.dataspace_id, manifest_root) {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verified lane relay proof does not match the declared manifest_root"
                            .into(),
                    ),
                )
                .into());
            }

            let proof_envelope = norito::decode_from_bytes::<AxtProofEnvelope>(&proof_blob.payload)
                .map_err(|err| {
                    InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(format!(
                            "verified lane relay proof envelope decode failed: {err}"
                        )),
                    )
                })?;
            if proof_envelope.manifest_root != manifest_root {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verified lane relay proof manifest_root mismatch".into(),
                    ),
                )
                .into());
            }
            let Some(binding) = proof_envelope.fastpq_binding.clone() else {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verified lane relay proof is missing fastpq_binding".into(),
                    ),
                )
                .into());
            };
            if binding.source_dsid != envelope.dataspace_id.as_u64() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "verified lane relay proof source_dsid mismatch".into(),
                    ),
                )
                .into());
            }
            let batch = fastpq_prover::build_batch_from_binding(&binding).map_err(|err| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    format!("verified lane relay binding is invalid: {err}"),
                ))
            })?;
            let proof = norito::decode_from_bytes::<fastpq_prover::Proof>(&proof_envelope.proof)
                .map_err(|err| {
                    InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(format!(
                            "verified lane relay FASTPQ proof decode failed: {err}"
                        )),
                    )
                })?;
            fastpq_prover::verify(&batch, &proof).map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!("verified lane relay FASTPQ verification failed: {err}").into(),
                )
            })?;

            let record = VerifiedLaneRelayRecord::new(
                envelope,
                CryptoHash::new(&proof_blob.payload),
                verified_at_height,
                manifest_root,
                binding,
            );
            let key = verified_lane_relay_state_key(&record.relay_ref)?;
            let encoded = norito::to_bytes(&record).map_err(|err| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    format!("verified lane relay encode failed: {err}"),
                ))
            })?;
            if let Some(existing) = state_transaction.world.smart_contract_state.get(&key) {
                let decoded = norito::decode_from_bytes::<VerifiedLaneRelayRecord>(existing)
                    .map_err(|err| {
                        InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(format!(
                                "stored verified lane relay decode failed: {err}"
                            )),
                        )
                    })?;
                if decoded != record {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "conflicting verified lane relay already exists".into(),
                    )
                    .into());
                }
                return Ok(());
            }

            state_transaction
                .world
                .smart_contract_state
                .insert(key, encoded);
            Ok(())
        }
    }

    fn parse_config_asset_definition_id(
        world: &impl crate::state::WorldReadOnly,
        raw: &str,
        now_ms: u64,
    ) -> Option<AssetDefinitionId> {
        crate::block::parse_asset_definition_literal_with_world(world, raw, now_ms)
    }

    fn is_permission_domain_associated(permission: &Permission, domain_id: &DomainId) -> bool {
        let asset_definition_matches_domain = |asset_definition: &AssetDefinitionId| -> bool {
            asset_definition.try_domain() == Some(domain_id)
        };
        if let Ok(permission) = CanUnregisterDomain::try_from(permission) {
            return &permission.domain == domain_id;
        }
        if let Ok(permission) = CanModifyDomainMetadata::try_from(permission) {
            return &permission.domain == domain_id;
        }
        if let Ok(permission) = CanRegisterAccount::try_from(permission) {
            return &permission.domain == domain_id;
        }
        if let Ok(permission) = CanUnregisterAssetDefinition::try_from(permission) {
            return asset_definition_matches_domain(&permission.asset_definition);
        }
        if let Ok(permission) = CanModifyAssetDefinitionMetadata::try_from(permission) {
            return asset_definition_matches_domain(&permission.asset_definition);
        }
        if let Ok(permission) = CanMintAssetWithDefinition::try_from(permission) {
            return asset_definition_matches_domain(&permission.asset_definition);
        }
        if let Ok(permission) = CanBurnAssetWithDefinition::try_from(permission) {
            return asset_definition_matches_domain(&permission.asset_definition);
        }
        if let Ok(permission) = CanTransferAssetWithDefinition::try_from(permission) {
            return asset_definition_matches_domain(&permission.asset_definition);
        }
        if let Ok(permission) = CanModifyAssetMetadataWithDefinition::try_from(permission) {
            return asset_definition_matches_domain(&permission.asset_definition);
        }
        if let Ok(permission) = CanMintAsset::try_from(permission) {
            return asset_definition_matches_domain(permission.asset.definition());
        }
        if let Ok(permission) = CanBurnAsset::try_from(permission) {
            return asset_definition_matches_domain(permission.asset.definition());
        }
        if let Ok(permission) = CanTransferAsset::try_from(permission) {
            return asset_definition_matches_domain(permission.asset.definition());
        }
        if let Ok(permission) = CanModifyAssetMetadata::try_from(permission) {
            return asset_definition_matches_domain(permission.asset.definition());
        }
        if let Ok(permission) = CanRegisterNft::try_from(permission) {
            return &permission.domain == domain_id;
        }
        if let Ok(permission) = CanUnregisterNft::try_from(permission) {
            return permission.nft.domain() == domain_id;
        }
        if let Ok(permission) = CanTransferNft::try_from(permission) {
            return permission.nft.domain() == domain_id;
        }
        if let Ok(permission) = CanModifyNftMetadata::try_from(permission) {
            return permission.nft.domain() == domain_id;
        }

        false
    }

    fn remove_domain_associated_permissions(
        state_transaction: &mut StateTransaction<'_, '_>,
        domain_id: &DomainId,
    ) {
        let account_ids: Vec<AccountId> = state_transaction
            .world
            .account_permissions
            .iter()
            .map(|(account_id, _)| account_id.clone())
            .collect();

        for account_id in account_ids {
            let should_remove = state_transaction
                .world
                .account_permissions
                .get(&account_id)
                .is_some_and(|permissions| {
                    permissions
                        .iter()
                        .any(|permission| is_permission_domain_associated(permission, domain_id))
                });
            if !should_remove {
                continue;
            }

            let remove_entry = if let Some(permissions) = state_transaction
                .world
                .account_permissions
                .get_mut(&account_id)
            {
                permissions
                    .retain(|permission| !is_permission_domain_associated(permission, domain_id));
                permissions.is_empty()
            } else {
                false
            };

            if remove_entry {
                state_transaction
                    .world
                    .account_permissions
                    .remove(account_id.clone());
            }

            state_transaction.invalidate_permission_cache_for_account(&account_id);
        }

        let role_ids: Vec<RoleId> = state_transaction
            .world
            .roles
            .iter()
            .map(|(role_id, _)| role_id.clone())
            .collect();

        for role_id in role_ids {
            let should_remove = state_transaction
                .world
                .roles
                .get(&role_id)
                .is_some_and(|role| {
                    role.permissions()
                        .any(|permission| is_permission_domain_associated(permission, domain_id))
                });
            if !should_remove {
                continue;
            }

            let impacted_accounts = state_transaction.accounts_with_role(&role_id);

            if let Some(role) = state_transaction.world.roles.get_mut(&role_id) {
                role.permissions
                    .retain(|permission| !is_permission_domain_associated(permission, domain_id));
                role.permission_epochs
                    .retain(|permission, _| role.permissions.contains(permission));
            }

            if !impacted_accounts.is_empty() {
                state_transaction.invalidate_permission_cache_for(impacted_accounts.iter());
            }
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

            let relabeled_accounts: Vec<AccountId> = state_transaction
                .world
                .accounts_in_domain_iter(&domain_id)
                .map(|account| account.id().clone())
                .collect();
            let remove_asset_definitions: Vec<AssetDefinitionId> = state_transaction
                .world
                .asset_definitions_in_domain_iter(&domain_id)
                .map(|ad| ad.id().clone())
                .collect();

            remove_domain_associated_permissions(state_transaction, &domain_id);

            state_transaction
                .world
                .domain_endorsement_policies
                .remove(domain_id.clone());

            let mut endorsement_hashes = BTreeSet::new();
            if let Some(hashes) = state_transaction
                .world
                .domain_endorsements_by_domain
                .remove(domain_id.clone())
            {
                endorsement_hashes.extend(hashes);
            }
            for (hash, record) in state_transaction.world.domain_endorsements.iter() {
                if record.endorsement.domain_id == domain_id {
                    endorsement_hashes.insert(*hash);
                }
            }
            for hash in endorsement_hashes {
                state_transaction.world.domain_endorsements.remove(hash);
            }

            let remove_nfts: BTreeSet<NftId> = state_transaction
                .world
                .nfts_in_domain_iter(&domain_id)
                .map(|nft| nft.id().clone())
                .collect();

            if let Some(rwa_id) = state_transaction
                .world
                .rwas_in_domain_iter(&domain_id)
                .map(|rwa| rwa.id().clone())
                .next()
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister domain {domain_id}: RWA {rwa_id} still exists in the domain; transfer or redeem all RWAs first"
                    )
                    .into(),
                )
                .into());
            }

            for nft_id in remove_nfts {
                crate::smartcontracts::isi::nft::isi::remove_nft_associated_permissions(
                    state_transaction,
                    &nft_id,
                );
                state_transaction.world.nfts.remove(nft_id.clone());
                state_transaction
                    .world
                    .emit_events(Some(DomainEvent::Nft(NftEvent::Deleted(nft_id))));
            }

            for asset_definition_id in &remove_asset_definitions {
                if asset_definition_id == &state_transaction.gov.voting_asset_id {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is configured as governance voting asset definition (`gov.voting_asset_id`); update governance config first"
                        )
                        .into(),
                    )
                    .into());
                }
                if asset_definition_id == &state_transaction.gov.citizenship_asset_id {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is configured as governance citizenship asset definition (`gov.citizenship_asset_id`); update governance config first"
                        )
                        .into(),
                    )
                    .into());
                }
                if asset_definition_id == &state_transaction.gov.parliament_eligibility_asset_id {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is configured as governance parliament eligibility asset definition (`gov.parliament_eligibility_asset_id`); update governance config first"
                        )
                        .into(),
                    )
                    .into());
                }
                if asset_definition_id
                    == &state_transaction
                        .gov
                        .viral_incentives
                        .reward_asset_definition_id
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is configured as governance viral reward asset definition (`gov.viral_incentives.reward_asset_definition_id`); update governance config first"
                        )
                        .into(),
                    )
                    .into());
                }
                if asset_definition_id == &state_transaction.oracle.economics.reward_asset {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is configured as oracle reward asset definition (`oracle.economics.reward_asset`); update oracle config first"
                        )
                        .into(),
                    )
                    .into());
                }
                if asset_definition_id == &state_transaction.oracle.economics.slash_asset {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is configured as oracle slash asset definition (`oracle.economics.slash_asset`); update oracle config first"
                        )
                        .into(),
                    )
                    .into());
                }
                if asset_definition_id == &state_transaction.oracle.economics.dispute_bond_asset {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is configured as oracle dispute bond asset definition (`oracle.economics.dispute_bond_asset`); update oracle config first"
                        )
                        .into(),
                    )
                    .into());
                }
                if parse_config_asset_definition_id(
                    &state_transaction.world,
                    &state_transaction.nexus.fees.fee_asset_id,
                    state_transaction.block_unix_timestamp_ms(),
                )
                .is_some_and(|configured| &configured == asset_definition_id)
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is configured as nexus fee asset definition (`nexus.fees.fee_asset_id`); update nexus config first"
                        )
                        .into(),
                    )
                    .into());
                }
                if parse_config_asset_definition_id(
                    &state_transaction.world,
                    &state_transaction.nexus.staking.stake_asset_id,
                    state_transaction.block_unix_timestamp_ms(),
                )
                .is_some_and(|configured| &configured == asset_definition_id)
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is configured as nexus staking asset definition (`nexus.staking.stake_asset_id`); update nexus config first"
                        )
                        .into(),
                    )
                    .into());
                }
                if state_transaction
                    .settlement
                    .repo
                    .eligible_collateral
                    .iter()
                    .any(|definition_id| definition_id == asset_definition_id)
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is configured as settlement repo eligible collateral (`settlement.repo.eligible_collateral`); update settlement config first"
                        )
                        .into(),
                    )
                    .into());
                }
                if let Some((base_definition_id, _)) = state_transaction
                    .settlement
                    .repo
                    .collateral_substitution_matrix
                    .iter()
                    .find(|(base_definition_id, substitutes)| {
                        *base_definition_id == asset_definition_id
                            || substitutes
                                .iter()
                                .any(|definition_id| definition_id == asset_definition_id)
                    })
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is configured in settlement repo collateral substitution matrix (`settlement.repo.collateral_substitution_matrix`, base {base_definition_id}); update settlement config first"
                        )
                        .into(),
                    )
                    .into());
                }
                if let Some((agreement_id, _)) = state_transaction
                    .world
                    .repo_agreements
                    .iter()
                    .find(|(_, agreement)| {
                        agreement.cash_leg().asset_definition_id() == asset_definition_id
                            || agreement.collateral_leg().asset_definition_id()
                                == asset_definition_id
                    })
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is referenced by repo agreement state ({agreement_id}); retain asset definition for settlement audit references"
                        )
                        .into(),
                    )
                    .into());
                }
                if let Some((settlement_id, _)) = state_transaction
                    .world
                    .settlement_ledgers
                    .iter()
                    .find(|(_, ledger)| {
                        ledger.entries.iter().any(|entry| {
                            entry
                                .legs
                                .iter()
                                .any(|leg| leg.leg.asset_definition_id() == asset_definition_id)
                        })
                    })
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} is referenced by settlement ledger state ({settlement_id}); retain asset definition for settlement audit references"
                        )
                        .into(),
                    )
                    .into());
                }
                if let Some(((lane_id, epoch), _)) = state_transaction
                    .world
                    .public_lane_rewards
                    .iter()
                    .find(|(_, record)| record.asset.definition() == asset_definition_id)
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} has active public-lane reward ledger state (lane {lane_id}, epoch {epoch}); settle or prune rewards first"
                        )
                        .into(),
                    )
                    .into());
                }
                if let Some(((lane_id, claimant, asset_id), _)) = state_transaction
                    .world
                    .public_lane_reward_claims
                    .iter()
                    .find(|((_, _, asset_id), _)| asset_id.definition() == asset_definition_id)
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} has pending public-lane reward claim state (lane {lane_id}, account {claimant}, asset {asset_id}); claim or clear rewards first"
                        )
                        .into(),
                    )
                    .into());
                }
                if let Some((certificate_id, _)) = state_transaction
                    .world
                    .offline_allowances
                    .iter()
                    .find(|(_, record)| {
                        record.certificate.allowance.asset.definition() == asset_definition_id
                    })
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} has active offline allowance state (certificate {certificate_id}); revoke or rotate allowance first"
                        )
                        .into(),
                    )
                    .into());
                }
                if let Some((bundle_id, _)) = state_transaction
                    .world
                    .offline_to_online_transfers
                    .iter()
                    .find(|(_, record)| {
                        record
                            .transfer
                            .receipts
                            .iter()
                            .any(|receipt| receipt.asset.definition() == asset_definition_id)
                    })
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister domain {domain_id}: asset definition {asset_definition_id} has active offline transfer state (bundle {bundle_id}); settle or prune transfer history first"
                        )
                        .into(),
                    )
                    .into());
                }
            }

            let remove_assets: Vec<AssetId> = state_transaction
                .world
                .assets
                .iter()
                .filter(|(asset_id, _)| asset_id.definition().try_domain() == Some(&domain_id))
                .map(|(asset_id, _)| asset_id.clone())
                .collect();
            for asset_id in remove_assets {
                state_transaction
                    .world
                    .remove_asset_and_metadata_with_total(&asset_id)?;
            }

            for asset_definition_id in remove_asset_definitions {
                state_transaction
                    .settlement
                    .offline
                    .escrow_accounts
                    .remove(&asset_definition_id);
                state_transaction
                    .world
                    .zk_assets
                    .remove(asset_definition_id.clone());
                state_transaction
                    .world
                    .asset_definitions
                    .remove(asset_definition_id.clone());
            }

            for account_id in relabeled_accounts {
                let labels: Vec<_> = state_transaction
                    .world
                    .account_aliases_by_account
                    .get(&account_id)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|label| {
                        label
                            .domain
                            .as_ref()
                            .is_some_and(|label_domain| label_domain.name() == domain_id.name())
                    })
                    .collect();
                if let Some(account) = state_transaction.world.accounts.get_mut(&account_id)
                    && account.label().is_some_and(|label| {
                        label
                            .domain
                            .as_ref()
                            .is_some_and(|label_domain| label_domain.name() == domain_id.name())
                    })
                {
                    account.set_label(None);
                }
                for label in labels {
                    state_transaction
                        .world
                        .account_rekey_records
                        .remove(label.clone());
                    state_transaction.world.remove_account_alias_binding(&label);
                }
            }
            let selector = iroha_data_model::account::AccountDomainSelector::from_domain(
                &domain_id,
            )
            .map_err(|err| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    err.code_str().into(),
                ))
            })?;
            if state_transaction
                .world
                .domains
                .remove(domain_id.clone())
                .is_none()
            {
                return Err(FindError::Domain(domain_id).into());
            }
            state_transaction.world.domain_selectors.remove(selector);

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
            let mut role = new_role.build(authority);
            role.ensure_permission_epochs(state_transaction.block_height());

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
            let current_epoch = state_transaction.block_height();

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
                let epoch = existing_role.permission_epoch(p).unwrap_or_default();
                new_role = new_role.add_permission_with_epoch(p.clone(), epoch);
            }
            new_role = new_role.add_permission_with_epoch(permission.clone(), current_epoch);
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
                    let epoch = existing_role.permission_epoch(p).unwrap_or_default();
                    new_role = new_role.add_permission_with_epoch(p.clone(), epoch);
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
                    if state_transaction.nexus.enabled {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "Nexus networks do not support staged consensus cutovers; remove sumeragi.next_mode/mode_activation_height"
                                    .to_owned(),
                            ),
                        ));
                    }
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
                    if state_transaction.nexus.enabled {
                        return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "Nexus networks do not support staged consensus cutovers; remove sumeragi.next_mode/mode_activation_height"
                                    .to_owned(),
                            ),
                        ));
                    }
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
            let validate_timing = |min_finality_ms: u64,
                                   block_time_ms: u64,
                                   commit_time_ms: u64|
             -> Result<(), Error> {
                if min_finality_ms == 0 {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "sumeragi.min_finality_ms must be greater than zero".to_owned(),
                        ),
                    )
                    .into());
                }
                if block_time_ms < min_finality_ms {
                    return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "sumeragi.block_time_ms must be greater than or equal to sumeragi.min_finality_ms"
                                    .to_owned(),
                            ),
                        )
                        .into());
                }
                if commit_time_ms < block_time_ms {
                    return Err(InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(
                                "sumeragi.commit_time_ms must be greater than or equal to sumeragi.block_time_ms"
                                    .to_owned(),
                            ),
                        )
                        .into());
                }
                Ok(())
            };
            if let Parameter::Sumeragi(param) = self.inner().clone() {
                let params_view = state_transaction.world.parameters.get();
                let sumeragi = params_view.sumeragi();
                let mut min_finality_ms = sumeragi.min_finality_ms();
                let mut block_time_ms = sumeragi.block_time_ms();
                let mut commit_time_ms = sumeragi.commit_time_ms();
                let mut should_validate = false;
                match param {
                    SumeragiParameter::MinFinalityMs(value) => {
                        min_finality_ms = value;
                        should_validate = true;
                    }
                    SumeragiParameter::BlockTimeMs(value) => {
                        block_time_ms = value;
                        should_validate = true;
                    }
                    SumeragiParameter::CommitTimeMs(value) => {
                        commit_time_ms = value;
                        should_validate = true;
                    }
                    SumeragiParameter::PacingFactorBps(value) => {
                        if value < 10_000 {
                            return Err(InstructionExecutionError::InvalidParameter(
                                InvalidParameterError::SmartContract(
                                    "sumeragi.pacing_factor_bps must be greater than or equal to 10_000"
                                        .to_owned(),
                                ),
                            ));
                        }
                    }
                    _ => {}
                }
                if should_validate {
                    validate_timing(min_finality_ms, block_time_ms, commit_time_ms)?;
                }
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
                            if let Some(npos) = iroha_data_model::parameter::system::SumeragiNposParameters::from_custom_parameter(&next) {
                                if npos.evidence_horizon_blocks() == 0 {
                                    return Err(InstructionExecutionError::InvalidParameter(
                                        InvalidParameterError::SmartContract(
                                            "sumeragi.npos.reconfig.evidence_horizon_blocks must be greater than zero"
                                                .to_owned(),
                                        ),
                                    ));
                                }
                                if npos.activation_lag_blocks() == 0 {
                                    return Err(InstructionExecutionError::InvalidParameter(
                                        InvalidParameterError::SmartContract(
                                            "sumeragi.npos.reconfig.activation_lag_blocks must be greater than zero"
                                                .to_owned(),
                                        ),
                                    ));
                                }
                                if npos.slashing_delay_blocks() == 0 {
                                    return Err(InstructionExecutionError::InvalidParameter(
                                        InvalidParameterError::SmartContract(
                                            "sumeragi.npos.reconfig.slashing_delay_blocks must be greater than zero"
                                                .to_owned(),
                                        ),
                                    ));
                                }
                            }
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
                Sumeragi(sumeragi.min_finality_ms) => SumeragiParameter::MinFinalityMs,
                Sumeragi(sumeragi.pacing_factor_bps) => SumeragiParameter::PacingFactorBps,
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
            IntoKeyValue,
            account::{AccountId, MultisigMember, MultisigPolicy},
            bridge::BridgeReceipt,
            confidential::ConfidentialStatus,
            consensus::{
                ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus,
                HsmBinding,
            },
            events::data::{DataEvent, prelude::BridgeEvent},
            isi::{
                Grant, Revoke, consensus_keys, nexus::SetLaneRelayEmergencyValidators,
                verifying_keys,
            },
            metadata::Metadata,
            nexus::{
                DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, DomainEndorsement,
                DomainEndorsementPolicy, DomainEndorsementScope, DomainEndorsementSignature,
                LaneId,
            },
            permission::Permission,
            proof::{
                ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord,
            },
            query::error::FindError,
            role::{Role, RoleId},
            zk::BackendTag,
        };
        use iroha_data_model::{
            account::{NewAccount, rekey::AccountAlias},
            asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
            nexus::{AssetPermissionManifest, ManifestVersion, UniversalAccountId},
            nft::{Nft, NftId},
            offline::{
                OfflineAllowanceCommitment, OfflineBalanceProof, OfflineToOnlineTransfer,
                OfflineTransferRecord, OfflineTransferStatus,
            },
        };
        use iroha_data_model::{
            isi::{SetParameter, bridge::RecordBridgeReceipt},
            parameter::system::{SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter},
            prelude::Parameter,
            zk::OpenVerifyEnvelope,
        };

        #[test]
        fn derive_ballot_nullifier_is_unambiguous_for_delimiters() {
            let commit = [0x42; 32];
            let chain_left: iroha_data_model::ChainId = "c".parse().expect("valid chain id");
            let chain_right: iroha_data_model::ChainId = "b|c".parse().expect("valid chain id");
            let first = derive_ballot_nullifier("a|b", &chain_left, "d", &commit);
            let second = derive_ballot_nullifier("a", &chain_right, "d", &commit);
            assert_ne!(first, second);
        }

        #[test]
        fn extract_vote_public_inputs_rejects_invalid_payload() {
            let result = super::extract_vote_public_inputs("halo2/kzg", &[]);
            assert!(result.is_err());
        }

        #[test]
        fn parse_hex32_hint_accepts_scheme_and_prefix() {
            let raw = format!("BlAkE2B32:0x{}", "Aa".repeat(32));
            let parsed = super::parse_hex32_hint(&raw).expect("parse hint");
            assert_eq!(parsed, [0xaa; 32]);
        }

        #[test]
        fn parse_hex32_hint_rejects_unknown_scheme() {
            let raw = format!("sha256:{}", "aa".repeat(32));
            assert!(super::parse_hex32_hint(&raw).is_none());
        }
        use iroha_executor_data_model::permission::domain::CanModifyDomainMetadata;
        use iroha_primitives::{json::Json, numeric::Numeric};
        #[allow(unused_imports)]
        use iroha_schema::Ident;
        use iroha_test_samples::{ALICE_ID, gen_account_in};

        use super::*;
        use crate::{
            block::ValidBlock,
            executor::Executor,
            kura::Kura,
            nexus::space_directory::{SpaceDirectoryManifestRecord, SpaceDirectoryManifestSet},
            query::store::LiveQueryStore,
            state::{
                State, StateTransaction, SumeragiPolicyConfig, SumeragiPolicyFlags, World,
                storage_transactions::TransactionsBlockError,
            },
            zk::{hash_proof, hash_vk},
        };

        fn new_dummy_block_at_height(height: NonZeroU64) -> crate::block::CommittedBlock {
            let (leader_public_key, leader_private_key) =
                iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal).into_parts();
            let peer_id = crate::PeerId::new(leader_public_key);
            let topology = crate::sumeragi::network_topology::Topology::new(vec![peer_id]);
            ValidBlock::new_dummy_and_modify_header(&leader_private_key, |h| {
                h.set_height(height);
            })
            .commit(&topology)
            .unpack(|_| {})
            .unwrap()
        }

        fn new_dummy_block() -> crate::block::CommittedBlock {
            new_dummy_block_at_height(NonZeroU64::new(1).unwrap())
        }

        fn new_dummy_block_non_genesis() -> crate::block::CommittedBlock {
            new_dummy_block_at_height(NonZeroU64::new(2).unwrap())
        }

        fn new_account_in_domain(account_id: &AccountId) -> NewAccount {
            NewAccount::new(account_id.clone())
        }

        fn seed_domain_name_lease(world: &mut World, owner: &AccountId, domain_id: &DomainId) {
            let selector = crate::sns::selector_for_domain(domain_id).expect("selector");
            let address =
                iroha_data_model::account::AccountAddress::from_account_id(owner).expect("address");
            let record = iroha_data_model::sns::NameRecordV1::new(
                selector.clone(),
                owner.clone(),
                vec![iroha_data_model::sns::NameControllerV1::account(&address)],
                0,
                0,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                Metadata::default(),
            );
            world.smart_contract_state_mut_for_testing().insert(
                crate::sns::record_storage_key(&selector),
                norito::codec::Encode::encode(&record),
            );
        }

        fn seed_domain_name_lease_tx(
            world: &mut WorldTransaction<'_, '_>,
            owner: &AccountId,
            domain_id: &DomainId,
        ) {
            let selector = crate::sns::selector_for_domain(domain_id).expect("selector");
            let address =
                iroha_data_model::account::AccountAddress::from_account_id(owner).expect("address");
            let record = iroha_data_model::sns::NameRecordV1::new(
                selector.clone(),
                owner.clone(),
                vec![iroha_data_model::sns::NameControllerV1::account(&address)],
                0,
                0,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                Metadata::default(),
            );
            world.smart_contract_state_mut_for_testing().insert(
                crate::sns::record_storage_key(&selector),
                norito::codec::Encode::encode(&record),
            );
        }

        #[test]
        fn grant_role_permission_records_epoch_and_revoke_clears() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut world = World::default();

            let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let authority = AccountId::new(keypair.public_key().clone());

            let role_id: RoleId = "auditor".parse().expect("role id");
            let role = Role::new(role_id.clone(), authority.clone()).build(&authority);
            world.roles.insert(role_id.clone(), role);

            let state = State::new(world, kura, query_handle);
            let header = BlockHeader::new(NonZeroU64::new(5).unwrap(), None, None, None, 0, 0);
            let mut state_block = state.block(header);
            let mut stx = state_block.transaction();

            let perm = Permission::new("can_read_all_accounts".to_string(), Json::new(()));
            Grant::role_permission(perm.clone(), role_id.clone())
                .execute(&authority, &mut stx)
                .expect("grant role permission");
            let role_after_grant = stx.world.roles.get(&role_id).expect("role exists");
            assert_eq!(
                role_after_grant.permission_epoch(&perm),
                Some(stx.block_height())
            );

            Revoke::role_permission(perm.clone(), role_id.clone())
                .execute(&authority, &mut stx)
                .expect("revoke role permission");
            let role_after_revoke = stx.world.roles.get(&role_id).expect("role exists");
            assert_eq!(role_after_revoke.permission_epoch(&perm), None);
        }

        #[test]
        fn normalize_halo2_circuit_id_and_match_variants() {
            assert_eq!(
                normalize_halo2_circuit_id(" halo2/pasta/ipa/foo "),
                Some("halo2/pasta/ipa/foo".to_string())
            );
            assert_eq!(
                normalize_halo2_circuit_id("halo2/pasta/foo"),
                Some("halo2/pasta/ipa/foo".to_string())
            );
            assert_eq!(
                normalize_halo2_circuit_id("halo2/ipa:foo"),
                Some("halo2/pasta/ipa/foo".to_string())
            );
            assert_eq!(normalize_halo2_circuit_id(""), None);

            assert!(circuit_id_matches(
                "halo2/ipa",
                "halo2/pasta/ipa/zk-vote",
                "halo2/ipa:zk-vote"
            ));
            assert!(!circuit_id_matches(
                "halo2/ipa",
                "halo2/pasta/ipa/zk-vote",
                "halo2/ipa:other"
            ));
            assert!(circuit_id_matches("groth16", "plain", "plain"));
            assert!(!circuit_id_matches("groth16", "plain", "plain "));
            assert!(voting_circuit_matches(
                "halo2/ipa",
                "halo2/pasta/ipa/any-circuit",
                "vote-ballot"
            ));
            assert!(voting_circuit_matches(
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks:vote-ballot",
                "vote-ballot"
            ));
            assert!(!voting_circuit_matches(
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks:vote-tally",
                "vote-ballot"
            ));
        }

        #[test]
        fn register_domain_requires_active_sns_lease_for_non_genesis_owner() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);
            let block = new_dummy_block_non_genesis();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            let (authority, _) = gen_account_in("tenants");
            let domain_id: DomainId = "leased.world".parse().expect("domain");

            let err = Register::domain(Domain::new(domain_id))
                .execute(&authority, &mut stx)
                .expect_err("missing lease must fail");

            assert!(
                err.to_string().contains("active SNS domain-name lease"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn register_domain_in_genesis_does_not_require_sns_lease() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);
            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            let (authority, _) = gen_account_in("tenants");
            let domain_id: DomainId = "leased-genesis.world".parse().expect("domain");

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&authority, &mut stx)
                .expect("genesis registration should bypass SNS lease gating");

            assert!(
                stx.world.domains.get(&domain_id).is_some(),
                "genesis registration should materialize the domain"
            );
        }

        #[test]
        fn register_domain_accepts_matching_active_sns_lease() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let (authority, _) = gen_account_in("tenants");
            let domain_id: DomainId = "leased-ok.world".parse().expect("domain");
            let mut world = World::default();
            seed_domain_name_lease(&mut world, &authority, &domain_id);
            let state = State::new(world, kura, query_handle);
            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&authority, &mut stx)
                .expect("lease-backed registration should succeed");

            assert!(
                stx.world.domains.get(&domain_id).is_some(),
                "domain should be stored after registration"
            );
        }

        #[test]
        fn validate_vote_envelope_metadata_checks_circuit_and_commitment() {
            let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2, 3, 4]);
            let commitment = hash_vk(&vk_box);
            let mut vk_rec = VerifyingKeyRecord::new_with_owner(
                1,
                "halo2/pasta/ipa/test-circuit",
                None,
                "test",
                BackendTag::Halo2IpaPasta,
                "pallas",
                [0u8; 32],
                commitment,
            );
            vk_rec.status = ConfidentialStatus::Active;

            let bad_circuit = OpenVerifyEnvelope::new(
                BackendTag::Halo2IpaPasta,
                "halo2/ipa:other-circuit",
                commitment,
                Vec::new(),
                Vec::new(),
            );
            assert!(
                validate_vote_envelope_metadata("ballot", "halo2/ipa", &bad_circuit, &vk_rec)
                    .is_err()
            );

            let mut bad_hash = commitment;
            bad_hash[0] ^= 0x01;
            let bad_commitment = OpenVerifyEnvelope::new(
                BackendTag::Halo2IpaPasta,
                "halo2/ipa:test-circuit",
                bad_hash,
                Vec::new(),
                Vec::new(),
            );
            assert!(
                validate_vote_envelope_metadata("ballot", "halo2/ipa", &bad_commitment, &vk_rec)
                    .is_err()
            );

            let ok = OpenVerifyEnvelope::new(
                BackendTag::Halo2IpaPasta,
                "halo2/ipa:test-circuit",
                commitment,
                Vec::new(),
                Vec::new(),
            );
            assert!(validate_vote_envelope_metadata("ballot", "halo2/ipa", &ok, &vk_rec).is_ok());
        }

        #[test]
        fn validate_vote_envelope_metadata_checks_schema_hash() {
            let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![9, 8, 7, 6]);
            let commitment = hash_vk(&vk_box);
            let schema = b"schema:voting:v1".to_vec();
            let schema_hash: [u8; 32] = iroha_crypto::Hash::new(&schema).into();
            let mut vk_rec = VerifyingKeyRecord::new_with_owner(
                1,
                "halo2/pasta/ipa/test-circuit",
                None,
                "test",
                BackendTag::Halo2IpaPasta,
                "pallas",
                schema_hash,
                commitment,
            );
            vk_rec.status = ConfidentialStatus::Active;

            let ok = OpenVerifyEnvelope::new(
                BackendTag::Halo2IpaPasta,
                "halo2/ipa:test-circuit",
                commitment,
                schema.clone(),
                Vec::new(),
            );
            assert!(validate_vote_envelope_metadata("ballot", "halo2/ipa", &ok, &vk_rec).is_ok());

            let bad = OpenVerifyEnvelope::new(
                BackendTag::Halo2IpaPasta,
                "halo2/ipa:test-circuit",
                commitment,
                b"schema:voting:v2".to_vec(),
                Vec::new(),
            );
            assert!(validate_vote_envelope_metadata("ballot", "halo2/ipa", &bad, &vk_rec).is_err());
        }

        #[test]
        fn enforce_vk_max_proof_bytes_rejects_too_large() {
            let mut rec = VerifyingKeyRecord::new_with_owner(
                1,
                "halo2/pasta/ipa/max-proof",
                None,
                "test",
                BackendTag::Halo2IpaPasta,
                "pallas",
                [0u8; 32],
                [0u8; 32],
            );
            rec.max_proof_bytes = 8;
            assert!(enforce_vk_max_proof_bytes("ballot", &rec, 8).is_ok());
            assert!(enforce_vk_max_proof_bytes("ballot", &rec, 9).is_err());
            rec.max_proof_bytes = 0;
            assert!(enforce_vk_max_proof_bytes("ballot", &rec, 64).is_ok());
        }

        #[test]
        fn extract_vote_public_inputs_handles_halo2_envelope() {
            use iroha_zkp_halo2::Halo2ProofEnvelope;

            let inputs = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32]];
            let halo_env = Halo2ProofEnvelope::new(18, 1, 1, 0, inputs.clone(), vec![0xaa])
                .expect("halo2 envelope");
            let proof_bytes = halo_env.to_bytes();
            let columns: Vec<Vec<[u8; 32]>> = inputs.iter().copied().map(|v| vec![v]).collect();
            let envelope = OpenVerifyEnvelope::new(
                BackendTag::Halo2IpaPasta,
                "halo2/ipa:vote-circuit",
                [0u8; 32],
                b"schema:voting:halo2:v1".to_vec(),
                proof_bytes,
            );
            let payload = norito::to_bytes(&envelope).expect("encode envelope");

            let parsed = extract_vote_public_inputs("halo2/ipa", &payload).expect("extract inputs");
            assert_eq!(parsed.columns, columns);
            assert!(parsed.envelope.is_some());
        }

        #[test]
        fn extract_vote_public_inputs_handles_stark_envelope() {
            use iroha_data_model::zk::StarkFriOpenProofV1;

            let columns: Vec<Vec<[u8; 32]>> = vec![vec![[1u8; 32]], vec![[2u8; 32]]];
            let open = StarkFriOpenProofV1 {
                version: 1,
                public_inputs: columns.clone(),
                envelope_bytes: vec![0xAA, 0xBB],
            };
            let proof_bytes = norito::to_bytes(&open).expect("encode stark open proof");
            let envelope = OpenVerifyEnvelope::new(
                BackendTag::Stark,
                "vote-circuit",
                [0u8; 32],
                b"schema:voting:stark:v1".to_vec(),
                proof_bytes,
            );
            let payload = norito::to_bytes(&envelope).expect("encode envelope");

            let parsed = extract_vote_public_inputs("stark/fri/sha256-goldilocks", &payload)
                .expect("extract inputs");
            assert_eq!(parsed.columns, columns);
            assert!(parsed.envelope.is_some());
        }

        #[test]
        fn decode_open_verify_envelope_accepts_stark_backend() {
            let envelope = OpenVerifyEnvelope::new(
                BackendTag::Stark,
                "stark/fri/sha256-goldilocks:dummy-circuit",
                [0u8; 32],
                vec![1, 2, 3],
                vec![4, 5, 6],
            );
            let bytes = norito::to_bytes(&envelope).expect("encode OpenVerifyEnvelope");
            let proof_box = ProofBox::new("stark/fri/sha256-goldilocks".into(), bytes.clone());
            let decoded = decode_open_verify_envelope(&proof_box).expect("decode");
            assert_eq!(decoded.backend, BackendTag::Stark);
            assert_eq!(decoded.circuit_id, envelope.circuit_id);
            assert_eq!(decoded.public_inputs, envelope.public_inputs);
            assert_eq!(decoded.proof_bytes, envelope.proof_bytes);
        }

        #[test]
        fn resolve_vk_commitment_accepts_normalized_circuit_id() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let vk_id = VerifyingKeyId::new("halo2/ipa", "vk_test");
            let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2, 3, 4]);
            let commitment = hash_vk(&vk_box);
            let mut rec = VerifyingKeyRecord::new_with_owner(
                1,
                "halo2/pasta/ipa/tiny-add2inst-public",
                None,
                "test",
                BackendTag::Halo2IpaPasta,
                "pallas",
                [0u8; 32],
                commitment,
            );
            rec.status = ConfidentialStatus::Active;
            rec.gas_schedule_id = Some("halo2_default".to_string());
            rec.key = Some(vk_box);
            stx.world.verifying_keys.insert(vk_id.clone(), rec.clone());
            stx.world
                .verifying_keys_by_circuit
                .insert((rec.circuit_id.clone(), rec.version), vk_id.clone());

            let proof = ProofBox::new("halo2/ipa".into(), Vec::new());
            let attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof, vk_id);
            let envelope = OpenVerifyEnvelope::new(
                BackendTag::Halo2IpaPasta,
                "halo2/ipa:tiny-add2inst-public",
                commitment,
                Vec::new(),
                Vec::new(),
            );
            let resolved = resolve_vk_commitment(&attachment, Some(&envelope), &stx)
                .expect("resolve vk commitment");
            assert_eq!(resolved, Some(commitment));
        }

        #[test]
        fn resolve_vk_requires_single_attachment() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let stx = state_block.transaction();
            let st = crate::state::ElectionState::default();
            let proof = ProofBox::new("halo2/ipa".into(), Vec::new());

            let missing = ProofAttachment {
                backend: "halo2/ipa".into(),
                proof: proof.clone(),
                vk_ref: None,
                vk_inline: None,
                vk_commitment: None,
                envelope_hash: None,
                lane_privacy: None,
            };
            assert!(resolve_ballot_vk(&st, &missing, &stx).is_err());
            assert!(resolve_tally_vk(&st, &missing, &stx).is_err());

            let mut mixed = ProofAttachment::new_ref(
                "halo2/ipa".into(),
                proof,
                VerifyingKeyId::new("halo2/ipa", "vk_mixed"),
            );
            mixed.vk_inline = Some(VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2]));
            assert!(resolve_ballot_vk(&st, &mixed, &stx).is_err());
            assert!(resolve_tally_vk(&st, &mixed, &stx).is_err());
        }

        #[test]
        fn resolve_ballot_and_tally_vk_accept_valid_attachments() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let vk_id = VerifyingKeyId::new("halo2/ipa", "vk_ok");
            let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2, 3, 4, 5]);
            let commitment = hash_vk(&vk_box);
            let mut rec = VerifyingKeyRecord::new_with_owner(
                1,
                "halo2/pasta/ipa/vk-ok",
                None,
                "test",
                BackendTag::Halo2IpaPasta,
                "pallas",
                [0u8; 32],
                commitment,
            );
            rec.status = ConfidentialStatus::Active;
            rec.key = Some(vk_box.clone());
            rec.vk_len =
                u32::try_from(vk_box.bytes.len()).expect("verifying key length fits into u32");
            stx.world.verifying_keys.insert(vk_id.clone(), rec);

            let st = crate::state::ElectionState {
                vk_ballot: Some(vk_id.clone()),
                vk_tally: Some(vk_id.clone()),
                ..Default::default()
            };

            let proof = ProofBox::new("halo2/ipa".into(), vec![0xaa]);
            let ballot_att =
                ProofAttachment::new_inline("halo2/ipa".into(), proof.clone(), vk_box.clone());
            let (ballot_id, ballot_vk, ballot_rec) =
                resolve_ballot_vk(&st, &ballot_att, &stx).expect("resolve ballot vk");
            assert_eq!(ballot_id, vk_id);
            assert_eq!(ballot_vk, vk_box);
            assert_eq!(ballot_rec.commitment, commitment);

            let tally_att = ProofAttachment::new_ref("halo2/ipa".into(), proof, vk_id.clone());
            let (tally_id, tally_vk, tally_rec) =
                resolve_tally_vk(&st, &tally_att, &stx).expect("resolve tally vk");
            assert_eq!(tally_id, vk_id);
            assert_eq!(tally_vk, ballot_vk);
            assert_eq!(tally_rec.commitment, commitment);
        }

        #[test]
        fn resolve_ballot_and_tally_vk_accept_valid_stark_attachments() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let backend = "stark/fri/sha256-goldilocks";
            let ballot_vk_id = VerifyingKeyId::new(backend, "vk_stark_ballot_ok");
            let ballot_vk_box = VerifyingKeyBox::new(backend.into(), vec![9, 8, 7, 6, 5]);
            let ballot_commitment = hash_vk(&ballot_vk_box);
            let mut ballot_rec = VerifyingKeyRecord::new_with_owner(
                1,
                "stark/fri/sha256-goldilocks:vote-ballot",
                None,
                "test",
                BackendTag::Stark,
                "goldilocks",
                [0u8; 32],
                ballot_commitment,
            );
            ballot_rec.status = ConfidentialStatus::Active;
            ballot_rec.key = Some(ballot_vk_box.clone());
            ballot_rec.vk_len = u32::try_from(ballot_vk_box.bytes.len())
                .expect("verifying key length fits into u32");
            stx.world
                .verifying_keys
                .insert(ballot_vk_id.clone(), ballot_rec);

            let tally_vk_id = VerifyingKeyId::new(backend, "vk_stark_tally_ok");
            let tally_vk_box = VerifyingKeyBox::new(backend.into(), vec![5, 6, 7, 8, 9]);
            let tally_commitment = hash_vk(&tally_vk_box);
            let mut tally_rec = VerifyingKeyRecord::new_with_owner(
                1,
                "stark/fri/sha256-goldilocks:vote-tally",
                None,
                "test",
                BackendTag::Stark,
                "goldilocks",
                [0u8; 32],
                tally_commitment,
            );
            tally_rec.status = ConfidentialStatus::Active;
            tally_rec.key = Some(tally_vk_box.clone());
            tally_rec.vk_len = u32::try_from(tally_vk_box.bytes.len())
                .expect("verifying key length fits into u32");
            stx.world
                .verifying_keys
                .insert(tally_vk_id.clone(), tally_rec);

            let st = crate::state::ElectionState {
                vk_ballot: Some(ballot_vk_id.clone()),
                vk_tally: Some(tally_vk_id.clone()),
                ..Default::default()
            };

            let proof = ProofBox::new(backend.into(), vec![0xbb]);
            let ballot_att =
                ProofAttachment::new_inline(backend.into(), proof.clone(), ballot_vk_box.clone());
            let (ballot_id, ballot_vk, ballot_rec) =
                resolve_ballot_vk(&st, &ballot_att, &stx).expect("resolve ballot vk");
            assert_eq!(ballot_id, ballot_vk_id);
            assert_eq!(ballot_vk, ballot_vk_box);
            assert_eq!(ballot_rec.commitment, ballot_commitment);

            let tally_att = ProofAttachment::new_ref(backend.into(), proof, tally_vk_id.clone());
            let (tally_id, tally_vk, tally_rec) =
                resolve_tally_vk(&st, &tally_att, &stx).expect("resolve tally vk");
            assert_eq!(tally_id, tally_vk_id);
            assert_eq!(tally_vk, tally_vk_box);
            assert_eq!(tally_rec.commitment, tally_commitment);
        }

        #[test]
        fn resolve_ballot_and_tally_vk_reject_stark_role_mismatch() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let backend = "stark/fri/sha256-goldilocks";
            let ballot_vk_id = VerifyingKeyId::new(backend, "vk_stark_ballot_bad");
            let ballot_vk_box = VerifyingKeyBox::new(backend.into(), vec![1, 2, 3, 4, 5]);
            let ballot_commitment = hash_vk(&ballot_vk_box);
            let mut ballot_rec = VerifyingKeyRecord::new_with_owner(
                1,
                "stark/fri/sha256-goldilocks:not-a-ballot-circuit",
                None,
                "test",
                BackendTag::Stark,
                "goldilocks",
                [0u8; 32],
                ballot_commitment,
            );
            ballot_rec.status = ConfidentialStatus::Active;
            ballot_rec.key = Some(ballot_vk_box);
            stx.world
                .verifying_keys
                .insert(ballot_vk_id.clone(), ballot_rec);

            let tally_vk_id = VerifyingKeyId::new(backend, "vk_stark_tally_bad");
            let tally_vk_box = VerifyingKeyBox::new(backend.into(), vec![5, 4, 3, 2, 1]);
            let tally_commitment = hash_vk(&tally_vk_box);
            let mut tally_rec = VerifyingKeyRecord::new_with_owner(
                1,
                "stark/fri/sha256-goldilocks:not-a-tally-circuit",
                None,
                "test",
                BackendTag::Stark,
                "goldilocks",
                [0u8; 32],
                tally_commitment,
            );
            tally_rec.status = ConfidentialStatus::Active;
            tally_rec.key = Some(tally_vk_box);
            stx.world
                .verifying_keys
                .insert(tally_vk_id.clone(), tally_rec);

            let st = crate::state::ElectionState {
                vk_ballot: Some(ballot_vk_id.clone()),
                vk_tally: Some(tally_vk_id.clone()),
                ..Default::default()
            };

            let ballot_proof = ProofBox::new(backend.into(), vec![0x11]);
            let ballot_att = ProofAttachment::new_ref(backend.into(), ballot_proof, ballot_vk_id);
            assert!(resolve_ballot_vk(&st, &ballot_att, &stx).is_err());

            let tally_proof = ProofBox::new(backend.into(), vec![0x22]);
            let tally_att = ProofAttachment::new_ref(backend.into(), tally_proof, tally_vk_id);
            assert!(resolve_tally_vk(&st, &tally_att, &stx).is_err());
        }

        fn bootstrap_alice_account(stx: &mut StateTransaction<'_, '_>) {
            let domain_id: DomainId = "wonderland".parse().expect("domain id parses");
            seed_domain_name_lease_tx(&mut stx.world, &ALICE_ID, &domain_id);
            Register::domain(Domain::new(domain_id))
                .execute(&ALICE_ID, stx)
                .expect("register wonderland domain");
            Register::account(new_account_in_domain(&ALICE_ID))
                .execute(&ALICE_ID, stx)
                .expect("register ALICE account");
        }

        fn configure_global_dataspace(stx: &mut StateTransaction<'_, '_>) {
            stx.nexus.dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
                id: DataSpaceId::GLOBAL,
                alias: "universal".to_string(),
                description: None,
                fault_tolerance: 1,
            }])
            .expect("dataspace catalog");
        }

        fn seed_manifest_record(
            stx: &mut StateTransaction<'_, '_>,
            uaid: UniversalAccountId,
            dataspace: DataSpaceId,
        ) {
            let manifest = AssetPermissionManifest {
                version: ManifestVersion::default(),
                uaid,
                dataspace,
                issued_ms: 0,
                activation_epoch: 0,
                expiry_epoch: None,
                entries: Vec::new(),
            };
            let mut record = SpaceDirectoryManifestRecord::new(manifest);
            record.lifecycle.mark_activated(1);
            let mut set = SpaceDirectoryManifestSet::default();
            set.upsert(record);
            stx.world.space_directory_manifests.insert(uaid, set);
        }

        fn grant_manage_lane_relay_emergency_permission(
            stx: &mut StateTransaction<'_, '_>,
            account: &AccountId,
        ) {
            let permission = Permission::new("CanManageLaneRelayEmergency".into(), Json::new(()));
            stx.world
                .account_permissions
                .insert(account.clone(), BTreeSet::from([permission]));
        }

        fn seed_live_peer(stx: &mut StateTransaction<'_, '_>, keypair: &KeyPair) -> PeerId {
            let peer = PeerId::new(keypair.public_key().clone());
            if stx.world.peers.iter().all(|existing| existing != &peer) {
                let _ = stx.world.peers.push(peer.clone());
            }

            let record = ConsensusKeyRecord {
                id: crate::state::derive_validator_key_id(keypair.public_key()),
                public_key: keypair.public_key().clone(),
                pop: Some(
                    iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                        .expect("generate pop for test peer"),
                ),
                activation_height: 0,
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            let record_id = record.id.clone();
            upsert_consensus_key(&mut stx.world, &record_id, record);
            peer
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
            let multisig_id = AccountId::new_multisig(policy);
            Register::account(Account::new(multisig_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register multisig authority");
            multisig_id
        }

        #[test]
        fn unregister_domain_preserves_global_account_records_and_owned_foreign_nfts() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");
            let other_domain_id: DomainId = "other.world".parse().expect("domain id parses");
            let account_label = AccountAlias::new(
                "primary".parse().unwrap(),
                Some(AccountAliasDomain::new(domain_id.name().clone())),
                DataSpaceId::GLOBAL,
            );
            let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::domain_unregister"));
            let dataspace = DataSpaceId::new(17);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");
            Register::domain(Domain::new(other_domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register other domain");

            seed_manifest_record(&mut stx, uaid, dataspace);

            let keypair = KeyPair::random();
            let account_id = AccountId::new(keypair.public_key().clone());
            Register::account(
                new_account_in_domain(&account_id)
                    .with_label(Some(account_label.clone()))
                    .with_uaid(Some(uaid)),
            )
            .execute(&ALICE_ID, &mut stx)
            .expect("register account");

            assert!(
                stx.world.uaid_dataspaces.get(&uaid).is_some(),
                "bindings should exist before unregister"
            );

            stx.world.tx_sequences.insert(account_id.clone(), 7);

            let asset_def_id: AssetDefinitionId =
                AssetDefinitionId::new(domain_id.clone(), "rose".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register asset definition");
            let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
            let asset = Asset::new(asset_id.clone(), Numeric::new(1, 0));
            let (asset_id, asset_value) = asset.into_key_value();
            stx.world.assets.insert(asset_id.clone(), asset_value);
            stx.world.track_asset_holder(&asset_id);

            let mut metadata = Metadata::default();
            let key: iroha_data_model::name::Name = "tag".parse().unwrap();
            let value = Json::from(norito::json!("cleanup"));
            metadata.insert(key, value);
            stx.world.asset_metadata.insert(asset_id.clone(), metadata);

            let nft_id = NftId::new(other_domain_id.clone(), "phoenix".parse().unwrap());
            let nft = Nft {
                id: nft_id.clone(),
                content: Metadata::default(),
                owned_by: account_id.clone(),
            };
            let (nft_id, nft_value) = nft.into_key_value();
            stx.world.nfts.insert(nft_id.clone(), nft_value);

            Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("unregister domain");

            assert!(
                stx.world.accounts.get(&account_id).is_some(),
                "account should remain materialized"
            );
            assert!(
                stx.world.tx_sequences.get(&account_id) == Some(&7),
                "tx sequence should remain"
            );
            assert!(
                stx.world
                    .account_rekey_records
                    .get(&account_label)
                    .is_none(),
                "account label record should be removed"
            );
            assert!(
                stx.world.uaid_dataspaces.get(&uaid).is_some(),
                "UAID bindings should remain with the surviving account"
            );
            assert!(
                stx.world.nfts.get(&nft_id).is_some(),
                "foreign-domain NFT ownership should remain"
            );
            assert!(
                stx.world.asset_metadata.get(&asset_id).is_none(),
                "asset metadata should be removed with assets"
            );
            assert!(
                stx.world
                    .accounts
                    .get(&account_id)
                    .is_some_and(|account| account.label().is_none()),
                "domain-scoped label should be cleared from the surviving account"
            );
        }

        #[test]
        fn unregister_domain_removes_assets_with_definitions_in_other_domains() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "defs.world".parse().expect("domain id parses");
            let holder_domain: DomainId = "holder.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register defs domain");
            Register::domain(Domain::new(holder_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register holder domain");

            let (holder_id, _) = gen_account_in(&holder_domain);
            Register::account(new_account_in_domain(&holder_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register holder account");

            let asset_def_id: AssetDefinitionId =
                AssetDefinitionId::new(domain_id.clone(), "rose".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register asset definition");

            let asset_id = AssetId::new(asset_def_id.clone(), holder_id.clone());
            let asset = Asset::new(asset_id.clone(), Numeric::new(1, 0));
            let (asset_id, asset_value) = asset.into_key_value();
            stx.world.assets.insert(asset_id.clone(), asset_value);
            stx.world.track_asset_holder(&asset_id);
            stx.world
                .asset_metadata
                .insert(asset_id.clone(), Metadata::default());

            Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("unregister defs domain");

            assert!(
                stx.world.accounts.get(&holder_id).is_some(),
                "holder account should remain"
            );
            assert!(
                stx.world.asset_definitions.get(&asset_def_id).is_none(),
                "asset definition should be removed"
            );
            assert!(
                stx.world.assets.get(&asset_id).is_none(),
                "assets with definitions in the removed domain should be removed"
            );
            assert!(
                stx.world.asset_metadata.get(&asset_id).is_none(),
                "asset metadata should be removed with assets"
            );
        }

        #[test]
        fn unregister_domain_preserves_surviving_account_foreign_ownerships() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");
            let foreign_domain: DomainId = "foreign.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");
            Register::domain(Domain::new(foreign_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register foreign domain");

            let (account_id, _) = gen_account_in(&domain_id);
            Register::account(new_account_in_domain(&account_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register account in cleanup domain");
            stx.world
                .domain_mut(&foreign_domain)
                .expect("foreign domain exists")
                .set_owned_by(account_id.clone());

            let asset_def_id: AssetDefinitionId =
                AssetDefinitionId::new(foreign_domain.clone(), "bond".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register foreign asset definition");
            stx.world
                .asset_definition_mut(&asset_def_id)
                .expect("foreign asset definition exists")
                .set_owned_by(account_id.clone());

            Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("domain unlink should preserve foreign ownership references");

            assert!(
                stx.world.domains.get(&domain_id).is_none(),
                "cleanup domain should be deleted"
            );
            assert!(
                stx.world.accounts.get(&account_id).is_some(),
                "account should remain materialized"
            );
            assert_eq!(
                stx.world
                    .domain(&foreign_domain)
                    .expect("foreign domain exists")
                    .owned_by(),
                &account_id,
                "foreign domain ownership should remain"
            );
            assert_eq!(
                stx.world
                    .asset_definition(&asset_def_id)
                    .expect("foreign asset definition exists")
                    .owned_by(),
                &account_id,
                "foreign asset-definition ownership should remain"
            );
        }

        #[test]
        fn unregister_domain_allows_surviving_accounts_used_by_global_config_and_invalid_literals()
        {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");

            let (account_id, _) = gen_account_in(&domain_id);
            Register::account(new_account_in_domain(&account_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register account in cleanup domain");
            stx.gov.bond_escrow_account = account_id.clone();
            stx.gov.viral_incentives.incentive_pool_account = account_id.clone();
            stx.oracle.economics.reward_pool = account_id.clone();
            stx.nexus.fees.fee_sink_account_id = "not-an-account-literal".to_owned();
            stx.nexus.staking.stake_escrow_account_id = account_id.to_string();
            stx.content.publish_allow_accounts = vec![account_id.clone()];

            let provider_id = iroha_data_model::sorafs::capacity::ProviderId::new([0xC3; 32]);
            stx.gov
                .sorafs_telemetry
                .per_provider_submitters
                .insert(provider_id, vec![account_id.clone()]);
            stx.gov
                .sorafs_provider_owners
                .insert(provider_id, account_id.clone());

            Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("global config/account references must not block domain unlink");

            assert!(
                stx.world.domains.get(&domain_id).is_none(),
                "cleanup domain should be deleted"
            );
            assert!(
                stx.world.accounts.get(&account_id).is_some(),
                "configured account should remain materialized"
            );
        }

        #[test]
        fn unregister_domain_preserves_configured_canonical_account() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let remove_domain: DomainId = "cleanup.world".parse().expect("domain id parses");
            let retained_domain: DomainId = "retained.world".parse().expect("domain id parses");
            let holder_domain: DomainId = "holder.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            bootstrap_alice_account(&mut stx);
            Register::domain(Domain::new(remove_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");
            Register::domain(Domain::new(retained_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register retained domain");
            Register::domain(Domain::new(holder_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register holder domain");

            let account_id = AccountId::new(KeyPair::random().public_key().clone());
            Register::account(new_account_in_domain(&account_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup-domain account");
            let (holder_id, _) = gen_account_in(&holder_domain);
            Register::account(new_account_in_domain(&holder_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register holder account");

            let permission: Permission =
                iroha_executor_data_model::permission::account::CanModifyAccountMetadata {
                    account: account_id.clone(),
                }
                .into();
            Grant::account_permission(permission.clone(), holder_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant account-target permission");
            stx.nexus.fees.fee_sink_account_id = account_id.to_string();

            Unregister::domain(remove_domain.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("removing an unrelated domain should preserve canonical account state");

            assert!(
                stx.world.domains.get(&remove_domain).is_none(),
                "removed domain should be deleted"
            );
            assert!(
                stx.world.accounts.get(&account_id).is_some(),
                "configured account should remain"
            );
            let account = stx
                .world
                .account(&account_id)
                .expect("configured account should exist");
            assert!(account.label().is_none());
            assert!(
                stx.world
                    .account_permissions
                    .get(&holder_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "global account-target permissions should remain"
            );
        }

        #[test]
        fn unregister_domain_rejects_when_domain_asset_definition_has_foreign_repo_agreement_state()
        {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");
            let foreign_domain: DomainId = "external.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");
            Register::domain(Domain::new(foreign_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register foreign domain");

            let (initiator, _) = gen_account_in(&foreign_domain);
            let (counterparty, _) = gen_account_in(&foreign_domain);
            Register::account(new_account_in_domain(&initiator))
                .execute(&ALICE_ID, &mut stx)
                .expect("register foreign initiator");
            Register::account(new_account_in_domain(&counterparty))
                .execute(&ALICE_ID, &mut stx)
                .expect("register foreign counterparty");

            let cash_def = AssetDefinitionId::new(domain_id.clone(), "usd".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = cash_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register cleanup-domain asset definition");

            let collateral_def =
                AssetDefinitionId::new(foreign_domain.clone(), "bond".parse().unwrap());
            let repo_id: iroha_data_model::repo::RepoAgreementId =
                "foreign_ref_guard".parse().expect("repo agreement id");
            stx.world.repo_agreements.insert(
                repo_id.clone(),
                iroha_data_model::repo::RepoAgreement::new(
                    repo_id,
                    initiator,
                    counterparty,
                    iroha_data_model::repo::RepoCashLeg {
                        asset_definition_id: cash_def.clone(),
                        quantity: Numeric::new(10, 0),
                    },
                    iroha_data_model::repo::RepoCollateralLeg::new(
                        collateral_def,
                        Numeric::new(12, 0),
                    ),
                    250,
                    1_000,
                    1,
                    iroha_data_model::repo::RepoGovernance::with_defaults(1_000, 60),
                    None,
                ),
            );

            let err = Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("domain unregister must reject foreign repo references to its assets");
            let err_string = err.to_string();
            assert!(
                err_string.contains("asset definition")
                    && err_string.contains("repo agreement state"),
                "error should explain foreign repo asset-definition conflict: {err_string}"
            );
            assert!(
                stx.world.domains.get(&domain_id).is_some(),
                "cleanup domain should remain after rejected unregister"
            );
            assert!(
                stx.world.asset_definitions.get(&cash_def).is_some(),
                "asset definition should remain after rejected unregister"
            );
        }

        #[test]
        fn unregister_domain_rejects_when_domain_asset_definition_is_governance_voting_asset() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");

            let voting_def = AssetDefinitionId::new(domain_id.clone(), "vote".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = voting_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register cleanup-domain asset definition");
            stx.gov.voting_asset_id = voting_def.clone();

            let err = Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err(
                    "domain unregister must reject governance voting asset-definition removal",
                );
            let err_string = err.to_string();
            assert!(
                err_string.contains("governance voting asset definition"),
                "error should explain governance voting-asset conflict: {err_string}"
            );
            assert!(
                stx.world.domains.get(&domain_id).is_some(),
                "cleanup domain should remain after rejected unregister"
            );
            assert!(
                stx.world.asset_definitions.get(&voting_def).is_some(),
                "asset definition should remain after rejected unregister"
            );
        }

        #[test]
        fn unregister_domain_rejects_when_domain_asset_definition_is_governance_viral_reward_asset()
        {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");

            let reward_def = AssetDefinitionId::new(domain_id.clone(), "viral".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = reward_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register cleanup-domain asset definition");
            stx.gov.viral_incentives.reward_asset_definition_id = reward_def.clone();

            let err = Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err(
                    "domain unregister must reject governance viral reward asset-definition removal",
                );
            let err_string = err.to_string();
            assert!(
                err_string.contains("governance viral reward asset definition"),
                "error should explain governance viral reward-asset conflict: {err_string}"
            );
            assert!(
                stx.world.domains.get(&domain_id).is_some(),
                "cleanup domain should remain after rejected unregister"
            );
            assert!(
                stx.world.asset_definitions.get(&reward_def).is_some(),
                "asset definition should remain after rejected unregister"
            );
        }

        #[test]
        fn unregister_domain_rejects_when_domain_asset_definition_is_oracle_reward_asset() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");

            let reward_def = AssetDefinitionId::new(domain_id.clone(), "oracle".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = reward_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register cleanup-domain asset definition");
            stx.oracle.economics.reward_asset = reward_def.clone();

            let err = Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("domain unregister must reject oracle reward asset-definition removal");
            let err_string = err.to_string();
            assert!(
                err_string.contains("oracle reward asset definition"),
                "error should explain oracle reward-asset conflict: {err_string}"
            );
            assert!(
                stx.world.domains.get(&domain_id).is_some(),
                "cleanup domain should remain after rejected unregister"
            );
            assert!(
                stx.world.asset_definitions.get(&reward_def).is_some(),
                "asset definition should remain after rejected unregister"
            );
        }

        #[test]
        fn unregister_domain_rejects_when_domain_asset_definition_is_nexus_fee_asset() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");

            let fee_def = AssetDefinitionId::new(domain_id.clone(), "nexusfee".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = fee_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register cleanup-domain asset definition");
            stx.nexus.fees.fee_asset_id = fee_def.to_string();

            let err = Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("domain unregister must reject nexus fee asset-definition removal");
            let err_string = err.to_string();
            assert!(
                err_string.contains("nexus fee asset definition"),
                "error should explain nexus fee-asset conflict: {err_string}"
            );
            assert!(
                stx.world.domains.get(&domain_id).is_some(),
                "cleanup domain should remain after rejected unregister"
            );
            assert!(
                stx.world.asset_definitions.get(&fee_def).is_some(),
                "asset definition should remain after rejected unregister"
            );
        }

        #[test]
        fn unregister_domain_rejects_when_domain_asset_definition_is_nexus_staking_asset() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");

            let stake_def = AssetDefinitionId::new(domain_id.clone(), "stake".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = stake_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register cleanup-domain asset definition");
            stx.nexus.staking.stake_asset_id = stake_def.to_string();

            let err = Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("domain unregister must reject nexus staking asset-definition removal");
            let err_string = err.to_string();
            assert!(
                err_string.contains("nexus staking asset definition"),
                "error should explain nexus staking-asset conflict: {err_string}"
            );
            assert!(
                stx.world.domains.get(&domain_id).is_some(),
                "cleanup domain should remain after rejected unregister"
            );
            assert!(
                stx.world.asset_definitions.get(&stake_def).is_some(),
                "asset definition should remain after rejected unregister"
            );
        }

        #[test]
        fn unregister_domain_rejects_when_domain_asset_definition_is_settlement_repo_eligible_collateral()
         {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");

            let collateral_def =
                AssetDefinitionId::new(domain_id.clone(), "collateral".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = collateral_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register cleanup-domain asset definition");
            stx.settlement.repo.eligible_collateral = vec![collateral_def.clone()];

            let err = Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err(
                    "domain unregister must reject settlement repo eligible collateral removal",
                );
            let err_string = err.to_string();
            assert!(
                err_string.contains("settlement repo eligible collateral"),
                "error should explain settlement repo eligible-collateral conflict: {err_string}"
            );
            assert!(
                stx.world.domains.get(&domain_id).is_some(),
                "cleanup domain should remain after rejected unregister"
            );
            assert!(
                stx.world.asset_definitions.get(&collateral_def).is_some(),
                "asset definition should remain after rejected unregister"
            );
        }

        #[test]
        fn unregister_domain_rejects_when_domain_asset_definition_is_settlement_repo_substitution_entry()
         {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");
            let foreign_domain: DomainId = "foreign.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");
            Register::domain(Domain::new(foreign_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register foreign domain");

            let base_def = AssetDefinitionId::new(foreign_domain, "base".parse().unwrap());
            let substitute_def = AssetDefinitionId::new(domain_id.clone(), "sub".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = base_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register foreign base asset definition");
            Register::asset_definition({
                let __asset_definition_id = substitute_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register cleanup substitute asset definition");
            stx.settlement
                .repo
                .collateral_substitution_matrix
                .insert(base_def, vec![substitute_def.clone()]);

            let err = Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("domain unregister must reject settlement repo substitution removal");
            let err_string = err.to_string();
            assert!(
                err_string.contains("collateral substitution matrix"),
                "error should explain settlement repo substitution conflict: {err_string}"
            );
            assert!(
                stx.world.domains.get(&domain_id).is_some(),
                "cleanup domain should remain after rejected unregister"
            );
            assert!(
                stx.world.asset_definitions.get(&substitute_def).is_some(),
                "asset definition should remain after rejected unregister"
            );
        }

        #[test]
        fn unregister_domain_removes_offline_escrow_mappings_for_domain_asset_definitions() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");

            let reward_def = AssetDefinitionId::new(domain_id.clone(), "offline".parse().unwrap());
            let mut metadata = Metadata::default();
            metadata.insert(
                iroha_data_model::offline::OFFLINE_ASSET_ENABLED_METADATA_KEY
                    .parse()
                    .expect("metadata key"),
                Json::new(true),
            );
            Register::asset_definition(NewAssetDefinition {
                id: reward_def.clone(),
                name: "offline".to_owned(),
                description: None,
                alias: None,
                spec: NumericSpec::integer(),
                mintable: Mintable::Infinitely,
                logo: None,
                metadata,
                balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
                confidential_policy: AssetConfidentialPolicy::transparent(),
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register cleanup-domain offline asset definition");

            assert!(
                stx.settlement
                    .offline
                    .escrow_accounts
                    .get(&reward_def)
                    .is_some(),
                "offline escrow mapping should exist before domain unregister"
            );

            Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("domain unregister should remove domain-local offline escrow mapping");

            assert!(
                stx.settlement
                    .offline
                    .escrow_accounts
                    .get(&reward_def)
                    .is_none(),
                "offline escrow mapping should be removed with domain asset definitions"
            );
            assert!(
                stx.world.domains.get(&domain_id).is_none(),
                "domain should be removed"
            );
        }

        #[test]
        fn unregister_domain_preserves_accounts_with_active_settlement_oracle_and_offline_state() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");
            let external_domain: DomainId = "external.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");
            Register::domain(Domain::new(external_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register external domain");

            let (account_id, _) = gen_account_in(&domain_id);
            Register::account(new_account_in_domain(&account_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register account in cleanup domain");

            let cash_def = AssetDefinitionId::new(external_domain.clone(), "usd".parse().unwrap());
            let collateral_def =
                AssetDefinitionId::new(external_domain.clone(), "bond".parse().unwrap());
            let reward_def =
                AssetDefinitionId::new(external_domain.clone(), "fee".parse().unwrap());
            let offline_def =
                AssetDefinitionId::new(external_domain.clone(), "coin".parse().unwrap());
            Register::asset_definition({
                let __asset_definition_id = cash_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register external cash definition");
            Register::asset_definition({
                let __asset_definition_id = collateral_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register external collateral definition");
            Register::asset_definition({
                let __asset_definition_id = reward_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register external reward definition");
            Register::asset_definition({
                let __asset_definition_id = offline_def.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .expect("register external offline definition");

            let repo_id: iroha_data_model::repo::RepoAgreementId =
                "repoguard".parse().expect("repo agreement id");
            let agreement = iroha_data_model::repo::RepoAgreement::new(
                repo_id.clone(),
                account_id.clone(),
                ALICE_ID.clone(),
                iroha_data_model::repo::RepoCashLeg {
                    asset_definition_id: cash_def,
                    quantity: Numeric::new(10, 0),
                },
                iroha_data_model::repo::RepoCollateralLeg::new(collateral_def, Numeric::new(12, 0)),
                250,
                1_000,
                1,
                iroha_data_model::repo::RepoGovernance::with_defaults(1_000, 60),
                None,
            );
            stx.world.repo_agreements.insert(repo_id.clone(), agreement);

            let settlement_id: iroha_data_model::isi::SettlementId =
                "settleguard".parse().expect("settlement id");
            let mut ledger = iroha_data_model::isi::SettlementLedger::default();
            ledger.push(iroha_data_model::isi::SettlementLedgerEntry {
                settlement_id: settlement_id.clone(),
                kind: iroha_data_model::isi::SettlementKind::Dvp,
                authority: account_id.clone(),
                plan: iroha_data_model::isi::SettlementPlan::default(),
                metadata: Metadata::default(),
                block_height: 1,
                block_hash: iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                    Hash::prehashed([0; Hash::LENGTH]),
                ),
                executed_at_ms: 1,
                legs: vec![iroha_data_model::isi::SettlementLegSnapshot {
                    role: iroha_data_model::isi::SettlementLegRole::Delivery,
                    leg: iroha_data_model::isi::SettlementLeg::new(
                        reward_def.clone(),
                        Numeric::new(1, 0),
                        account_id.clone(),
                        ALICE_ID.clone(),
                    ),
                    committed: true,
                }],
                outcome: iroha_data_model::isi::SettlementOutcomeRecord::Success(
                    iroha_data_model::isi::SettlementSuccessRecord {
                        first_committed: true,
                        second_committed: true,
                        fx_window_ms: None,
                    },
                ),
            });
            stx.world.settlement_ledgers.insert(settlement_id, ledger);

            let mut feed = iroha_data_model::oracle::kits::price_xor_usd().feed_config;
            feed.providers = vec![account_id.clone()];
            let feed_id = feed.feed_id.clone();
            let feed_config_version = feed.feed_config_version;
            stx.world.oracle_feeds.insert(feed_id.clone(), feed);
            stx.world.oracle_history.insert(
                feed_id.clone(),
                vec![iroha_data_model::events::data::oracle::FeedEventRecord {
                    event: iroha_data_model::oracle::FeedEvent {
                        feed_id,
                        feed_config_version,
                        slot: 1,
                        outcome: iroha_data_model::oracle::FeedEventOutcome::Success(
                            iroha_data_model::oracle::FeedSuccess {
                                value: iroha_data_model::oracle::ObservationValue::new(1_000, 2),
                                entries: vec![iroha_data_model::oracle::ReportEntry {
                                    oracle_id: account_id.clone(),
                                    observation_hash: iroha_crypto::HashOf::from_untyped_unchecked(
                                        Hash::new(b"oracle-history-domain-guard"),
                                    ),
                                    value: iroha_data_model::oracle::ObservationValue::new(
                                        1_000, 2,
                                    ),
                                    outlier: false,
                                }],
                            },
                        ),
                    },
                    evidence_hashes: Vec::new(),
                }],
            );

            let allowance = OfflineAllowanceCommitment::new(
                AssetId::new(offline_def, account_id.clone()),
                Numeric::new(10, 0),
                vec![0xCD],
            );
            let bundle_id = Hash::new(b"offline-transfer-domain-guard");
            let transfer = OfflineToOnlineTransfer::new(
                bundle_id,
                account_id.clone(),
                account_id.clone(),
                Vec::new(),
                OfflineBalanceProof::new(allowance, vec![0xCD], Numeric::new(1, 0), None),
                None,
                None,
                None,
            );
            stx.world.offline_to_online_transfers.insert(
                bundle_id,
                OfflineTransferRecord {
                    transfer,
                    controller: account_id.clone(),
                    status: OfflineTransferStatus::Settled,
                    rejection_reason: None,
                    recorded_at_ms: 1,
                    recorded_at_height: 1,
                    archived_at_height: None,
                    history: Vec::new(),
                    pos_verdict_snapshots: Vec::new(),
                    verdict_snapshot: None,
                    platform_snapshot: None,
                },
            );
            let verdict_id = Hash::new(b"offline-verdict-domain-guard");
            stx.world.offline_verdict_revocations.insert(
                verdict_id,
                iroha_data_model::offline::OfflineVerdictRevocation {
                    verdict_id,
                    issuer: account_id.clone(),
                    revoked_at_ms: 1,
                    reason:
                        iroha_data_model::offline::OfflineVerdictRevocationReason::IssuerRequest,
                    note: None,
                    metadata: Metadata::default(),
                },
            );

            stx.world.public_lane_validators.insert(
                (LaneId::SINGLE, account_id.clone()),
                iroha_data_model::nexus::PublicLaneValidatorRecord {
                    lane_id: LaneId::SINGLE,
                    validator: account_id.clone(),
                    peer_id: PeerId::from(account_id.signatory().clone()),
                    stake_account: account_id.clone(),
                    total_stake: Numeric::new(1, 0),
                    self_stake: Numeric::new(1, 0),
                    metadata: Metadata::default(),
                    status: iroha_data_model::nexus::PublicLaneValidatorStatus::Active,
                    activation_epoch: Some(1),
                    activation_height: Some(1),
                    last_reward_epoch: None,
                },
            );
            stx.world.public_lane_rewards.insert(
                (LaneId::SINGLE, 1),
                iroha_data_model::nexus::PublicLaneRewardRecord {
                    lane_id: LaneId::SINGLE,
                    epoch: 1,
                    asset: AssetId::new(reward_def.clone(), account_id.clone()),
                    total_reward: Numeric::new(1, 0),
                    shares: vec![iroha_data_model::nexus::PublicLaneRewardShare {
                        account: account_id.clone(),
                        role: iroha_data_model::nexus::PublicLaneRewardRole::Validator,
                        amount: Numeric::new(1, 0),
                    }],
                    metadata: Metadata::default(),
                },
            );
            stx.world.public_lane_reward_claims.insert(
                (
                    LaneId::SINGLE,
                    ALICE_ID.clone(),
                    AssetId::new(reward_def, account_id.clone()),
                ),
                1,
            );

            Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("domain unlink should preserve surviving account audit state");

            assert!(
                stx.world.domains.get(&domain_id).is_none(),
                "cleanup domain should be deleted"
            );
            assert!(
                stx.world.accounts.get(&account_id).is_some(),
                "account should remain materialized"
            );
            assert!(
                stx.world.repo_agreements.get(&repo_id).is_some(),
                "repo agreement state should remain"
            );
            assert!(
                stx.world
                    .offline_to_online_transfers
                    .get(&bundle_id)
                    .is_some(),
                "offline transfer state should remain"
            );
        }

        #[test]
        fn unregister_domain_preserves_accounts_with_active_governance_and_storage_audit_state() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");

            let (account_id, _) = gen_account_in(&domain_id);
            Register::account(new_account_in_domain(&account_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register account in cleanup domain");

            let proposal_id = [0xB7; 32];
            let kind = ProposalKind::DeployContract(DeployContractProposal {
                namespace: "gov".to_string(),
                contract_id: "proposal-guard".to_string(),
                code_hash_hex: ContractCodeHash::new([0x31; 32]),
                abi_hash_hex: ContractAbiHash::new([0x41; 32]),
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            });
            stx.world.governance_proposals.insert(
                proposal_id,
                crate::state::GovernanceProposalRecord {
                    proposer: account_id.clone(),
                    kind,
                    created_height: 1,
                    status: crate::state::GovernanceProposalStatus::Proposed,
                    pipeline: crate::state::GovernancePipeline::default(),
                    parliament_snapshot: Some(crate::state::GovernanceParliamentSnapshot {
                        selection_epoch: 1,
                        beacon: [0x91; 32],
                        roster_root: [0x92; 32],
                        bodies: iroha_data_model::governance::types::ParliamentBodies {
                            selection_epoch: 1,
                            rosters: std::collections::BTreeMap::from([(
                                iroha_data_model::governance::types::ParliamentBody::AgendaCouncil,
                                iroha_data_model::governance::types::ParliamentRoster {
                                    body: iroha_data_model::governance::types::ParliamentBody::AgendaCouncil,
                                    epoch: 1,
                                    members: vec![account_id.clone()],
                                    alternates: Vec::new(),
                                    verified: 0,
                                    candidate_count: 0,
                                    derived_by: Default::default(),
                                },
                            )]),
                        },
                    }),
                },
            );

            let bundle_id = Hash::new(b"content-bundle-domain-guard");
            let stripe_layout = iroha_data_model::da::prelude::DaStripeLayout::default();
            let manifest = iroha_data_model::content::ContentBundleManifest {
                bundle_id,
                index_hash: [0x55; 32],
                dataspace: DataSpaceId::GLOBAL,
                lane: LaneId::SINGLE,
                blob_class: iroha_data_model::da::types::BlobClass::GovernanceArtifact,
                retention: iroha_data_model::da::types::RetentionPolicy::default(),
                cache: iroha_data_model::content::ContentCachePolicy {
                    max_age_seconds: 60,
                    immutable: false,
                },
                auth: iroha_data_model::content::ContentAuthMode::Public,
                stripe_layout,
                mime_overrides: std::collections::BTreeMap::new(),
            };
            stx.world.content_bundles.insert(
                bundle_id,
                iroha_data_model::content::ContentBundleRecord {
                    bundle_id,
                    manifest,
                    total_bytes: 0,
                    chunk_size: 1,
                    chunk_hashes: Vec::new(),
                    chunk_root: [0; 32],
                    stripe_layout,
                    pdp_commitment: None,
                    files: Vec::new(),
                    created_by: account_id.clone(),
                    created_height: 1,
                    expires_at_height: None,
                },
            );

            let runtime_manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
                name: "runtime-guard".to_string(),
                description: "guard".to_string(),
                abi_version: 1,
                abi_hash: [0x61; 32],
                added_syscalls: Vec::new(),
                added_pointer_types: Vec::new(),
                start_height: 1,
                end_height: 2,
                sbom_digests: Vec::new(),
                slsa_attestation: Vec::new(),
                provenance: Vec::new(),
            };
            let upgrade_id = runtime_manifest.id();
            stx.world.runtime_upgrades.insert(
                upgrade_id,
                iroha_data_model::runtime::RuntimeUpgradeRecord {
                    manifest: runtime_manifest,
                    status: iroha_data_model::runtime::RuntimeUpgradeStatus::Proposed,
                    proposer: account_id.clone(),
                    created_height: 1,
                },
            );

            let binding_digest = Hash::new(b"viral-escrow-domain-guard");
            stx.world.viral_escrows.insert(
                binding_digest,
                iroha_data_model::social::ViralEscrowRecord {
                    binding_hash: iroha_data_model::oracle::KeyedHash {
                        pepper_id: "pepper".to_string(),
                        digest: binding_digest,
                    },
                    sender: account_id.clone(),
                    amount: Numeric::new(1, 0),
                    created_at_ms: 1,
                },
            );

            let digest = iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0xAC; 32]);
            stx.world.pin_manifests.insert(
                digest,
                iroha_data_model::sorafs::pin_registry::PinManifestRecord::new(
                    digest,
                    iroha_data_model::sorafs::pin_registry::ChunkerProfileHandle {
                        profile_id: 1,
                        namespace: "sorafs".to_string(),
                        name: "sf1".to_string(),
                        semver: "1.0.0".to_string(),
                        multihash_code: 0x1E,
                    },
                    [0xDC; 32],
                    iroha_data_model::sorafs::pin_registry::PinPolicy::default(),
                    account_id.clone(),
                    1,
                    None,
                    None,
                    Metadata::default(),
                ),
            );

            let ticket_id = iroha_data_model::da::types::StorageTicketId::new([0xD2; 32]);
            stx.world.da_pin_intents_by_ticket.insert(
                ticket_id,
                iroha_data_model::da::pin_intent::DaPinIntentWithLocation {
                    intent: iroha_data_model::da::pin_intent::DaPinIntent {
                        lane_id: LaneId::new(1),
                        epoch: 1,
                        sequence: 1,
                        storage_ticket: ticket_id,
                        manifest_hash: iroha_data_model::sorafs::pin_registry::ManifestDigest::new(
                            [0xE3; 32],
                        ),
                        alias: None,
                        owner: Some(account_id.clone()),
                    },
                    location: iroha_data_model::da::commitment::DaCommitmentLocation {
                        block_height: 1,
                        index_in_bundle: 0,
                    },
                },
            );
            stx.world.lane_relay_emergency_validators.insert(
                LaneId::new(0),
                iroha_data_model::nexus::LaneRelayEmergencyValidatorSet {
                    peers: vec![PeerId::from(account_id.signatory().clone())],
                    expires_at_height: 10,
                    metadata: Metadata::default(),
                },
            );

            Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("domain unlink should preserve governance and storage audit state");

            assert!(
                stx.world.domains.get(&domain_id).is_none(),
                "cleanup domain should be deleted"
            );
            assert!(
                stx.world.accounts.get(&account_id).is_some(),
                "account should remain materialized"
            );
            assert!(
                stx.world.governance_proposals.get(&proposal_id).is_some(),
                "governance proposal state should remain"
            );
            assert!(
                stx.world.pin_manifests.get(&digest).is_some(),
                "SoraFS pin manifest state should remain"
            );
        }

        #[test]
        fn unregister_domain_removes_associated_permissions_from_accounts_and_roles() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "kingdom".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            bootstrap_alice_account(&mut stx);

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register kingdom domain");

            let owner_domain: DomainId = "wonderland".parse().expect("domain id parses");
            let (bob_id, _) = gen_account_in(&owner_domain);
            Register::account(new_account_in_domain(&bob_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register bob account");

            let permission: Permission = CanModifyDomainMetadata {
                domain: domain_id.clone(),
            }
            .into();
            Grant::account_permission(permission.clone(), bob_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant permission to bob");

            let role_id: RoleId = "KINGDOM_ADMIN".parse().expect("role id parses");
            Register::role(Role::new(role_id.clone(), ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register role");
            Grant::role_permission(permission.clone(), role_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant permission to role");
            Grant::account_role(role_id.clone(), bob_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant role to bob");

            assert!(
                stx.world
                    .account_permissions
                    .get(&bob_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "bob should have permission before unregister"
            );
            let role = stx.world.roles.get(&role_id).expect("role should exist");
            assert!(
                role.permissions().any(|perm| perm == &permission),
                "role should have permission before unregister"
            );

            Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("unregister domain");

            assert!(
                !stx.world
                    .account_permissions
                    .get(&bob_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "bob permission should be removed"
            );
            let role = stx.world.roles.get(&role_id).expect("role should exist");
            assert!(
                !role.permissions().any(|perm| perm == &permission),
                "role permission should be removed"
            );
            assert!(
                !role.permission_epochs().contains_key(&permission),
                "permission epochs should be pruned"
            );
        }

        #[test]
        fn unregister_domain_preserves_account_target_permissions_for_surviving_accounts_and_roles()
        {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            bootstrap_alice_account(&mut stx);

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");

            let (target_id, _) = gen_account_in(&domain_id);
            Register::account(new_account_in_domain(&target_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register account in cleanup domain");

            let owner_domain: DomainId = "wonderland".parse().expect("domain id parses");
            let (holder_id, _) = gen_account_in(&owner_domain);
            Register::account(new_account_in_domain(&holder_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register holder account");

            let permission: Permission =
                iroha_executor_data_model::permission::account::CanModifyAccountMetadata {
                    account: target_id.clone(),
                }
                .into();
            Grant::account_permission(permission.clone(), holder_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant account-target permission to holder");

            let role_id: RoleId = "ACCOUNT_SCOPE_ADMIN".parse().expect("role id parses");
            Register::role(Role::new(role_id.clone(), ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register role");
            Grant::role_permission(permission.clone(), role_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant account-target permission to role");
            Grant::account_role(role_id.clone(), holder_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant role to holder");

            assert!(
                stx.world
                    .account_permissions
                    .get(&holder_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "holder should have permission before unregister"
            );
            let role = stx.world.roles.get(&role_id).expect("role should exist");
            assert!(
                role.permissions().any(|perm| perm == &permission),
                "role should include permission before unregister"
            );

            Unregister::domain(domain_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("unregister domain");

            assert!(
                stx.world
                    .account_permissions
                    .get(&holder_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "holder permission should remain"
            );
            let role = stx.world.roles.get(&role_id).expect("role should exist");
            assert!(
                role.permissions().any(|perm| perm == &permission),
                "role permission should remain"
            );
            assert!(
                role.permission_epochs().contains_key(&permission),
                "permission epochs should remain"
            );
        }

        #[test]
        fn unregister_domain_preserves_citizen_service_permissions_for_surviving_accounts_and_roles()
         {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            bootstrap_alice_account(&mut stx);

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");

            let (target_id, _) = gen_account_in(&domain_id);
            Register::account(new_account_in_domain(&target_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register account in cleanup domain");

            let owner_domain: DomainId = "wonderland".parse().expect("domain id parses");
            let (holder_id, _) = gen_account_in(&owner_domain);
            Register::account(new_account_in_domain(&holder_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register holder account");

            let permission: Permission =
                iroha_executor_data_model::permission::governance::CanRecordCitizenService {
                    owner: target_id.clone(),
                }
                .into();
            Grant::account_permission(permission.clone(), holder_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant account-target permission to holder");

            let role_id: RoleId = "CITIZEN_SERVICE_ADMIN".parse().expect("role id parses");
            Register::role(Role::new(role_id.clone(), ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register role");
            Grant::role_permission(permission.clone(), role_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant account-target permission to role");
            Grant::account_role(role_id.clone(), holder_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant role to holder");

            assert!(
                stx.world
                    .account_permissions
                    .get(&holder_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "holder should have permission before unregister"
            );
            let role = stx.world.roles.get(&role_id).expect("role should exist");
            assert!(
                role.permissions().any(|perm| perm == &permission),
                "role should include permission before unregister"
            );

            Unregister::domain(domain_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("unregister domain");

            assert!(
                stx.world
                    .account_permissions
                    .get(&holder_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "holder permission should remain"
            );
            let role = stx.world.roles.get(&role_id).expect("role should exist");
            assert!(
                role.permissions().any(|perm| perm == &permission),
                "role permission should remain"
            );
            assert!(
                role.permission_epochs().contains_key(&permission),
                "permission epochs should remain"
            );
        }

        #[test]
        fn unregister_domain_preserves_other_domain_permissions_for_same_subject() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");
            let retained_domain_id: DomainId = "retained.world".parse().expect("domain id parses");
            let holder_domain_id: DomainId = "holder.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            bootstrap_alice_account(&mut stx);

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");
            Register::domain(Domain::new(retained_domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register retained domain");
            Register::domain(Domain::new(holder_domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register holder domain");

            let keypair = KeyPair::random();
            let target_id = AccountId::new(keypair.public_key().clone());
            Register::account(new_account_in_domain(&target_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register account in cleanup domain");

            let (holder_id, _) = gen_account_in(&holder_domain_id);
            Register::account(new_account_in_domain(&holder_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register holder account");

            let permission: Permission =
                iroha_executor_data_model::permission::account::CanModifyAccountMetadata {
                    account: target_id.clone(),
                }
                .into();
            Grant::account_permission(permission.clone(), holder_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant account-target permission to holder");

            let role_id: RoleId = "CROSS_DOMAIN_KEEP".parse().expect("role id parses");
            Register::role(Role::new(role_id.clone(), ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register role");
            Grant::role_permission(permission.clone(), role_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant account-target permission to role");
            Grant::account_role(role_id.clone(), holder_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant role to holder");

            Unregister::domain(domain_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("unregister cleanup domain");

            assert!(
                stx.world.domains.get(&retained_domain_id).is_some(),
                "retained domain should remain"
            );
            assert!(
                stx.world.accounts.get(&target_id).is_some(),
                "canonical account should remain materialized"
            );
            assert!(
                stx.world.account(&target_id).is_ok(),
                "canonical account should remain addressable"
            );
            assert!(
                stx.world
                    .account_permissions
                    .get(&holder_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "holder permission for retained account should stay"
            );
            let role = stx.world.roles.get(&role_id).expect("role should exist");
            assert!(
                role.permissions().any(|perm| perm == &permission),
                "role permission for retained account should stay"
            );
            assert!(
                role.permission_epochs().contains_key(&permission),
                "permission epoch should stay for retained permission"
            );
        }

        #[test]
        fn unregister_domain_preserves_foreign_nft_permissions_for_surviving_accounts_and_roles() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "cleanup.world".parse().expect("domain id parses");
            let foreign_domain_id: DomainId = "foreign.world".parse().expect("domain id parses");
            let holder_domain_id: DomainId = "holder.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            bootstrap_alice_account(&mut stx);

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register cleanup domain");
            Register::domain(Domain::new(foreign_domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register foreign domain");
            Register::domain(Domain::new(holder_domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register holder domain");

            let (target_id, _) = gen_account_in(&domain_id);
            Register::account(new_account_in_domain(&target_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register account in cleanup domain");

            let (holder_id, _) = gen_account_in(&holder_domain_id);
            Register::account(new_account_in_domain(&holder_id))
                .execute(&ALICE_ID, &mut stx)
                .expect("register holder account");

            let nft_id = NftId::new(foreign_domain_id, "phoenix".parse().unwrap());
            let nft = Nft {
                id: nft_id.clone(),
                content: Metadata::default(),
                owned_by: target_id.clone(),
            };
            let (nft_id, nft_value) = nft.into_key_value();
            stx.world.nfts.insert(nft_id.clone(), nft_value);

            let permission: Permission =
                iroha_executor_data_model::permission::nft::CanTransferNft {
                    nft: nft_id.clone(),
                }
                .into();
            Grant::account_permission(permission.clone(), holder_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant nft-target permission to holder");

            let role_id: RoleId = "NFT_SCOPE_ADMIN".parse().expect("role id parses");
            Register::role(Role::new(role_id.clone(), ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register role");
            Grant::role_permission(permission.clone(), role_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant nft-target permission to role");
            Grant::account_role(role_id.clone(), holder_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant role to holder");

            assert!(
                stx.world
                    .account_permissions
                    .get(&holder_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "holder should have permission before unregister"
            );
            let role = stx.world.roles.get(&role_id).expect("role should exist");
            assert!(
                role.permissions().any(|perm| perm == &permission),
                "role should include permission before unregister"
            );

            Unregister::domain(domain_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("unregister domain");

            assert!(
                stx.world.nfts.get(&nft_id).is_some(),
                "foreign-domain NFT owned by surviving account should remain"
            );
            assert!(
                stx.world
                    .account_permissions
                    .get(&holder_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "holder permission should remain"
            );
            let role = stx.world.roles.get(&role_id).expect("role should exist");
            assert!(
                role.permissions().any(|perm| perm == &permission),
                "role permission should remain"
            );
            assert!(
                role.permission_epochs().contains_key(&permission),
                "permission epochs should remain"
            );
        }

        #[test]
        fn unregister_domain_clears_endorsements_and_policy() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let domain_id: DomainId = "endorsed.world".parse().expect("domain id parses");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register domain");

            let policy = DomainEndorsementPolicy {
                committee_id: "default".to_owned(),
                max_endorsement_age: 10,
                required: false,
            };
            stx.world
                .domain_endorsement_policies
                .insert(domain_id.clone(), policy);

            let statement_hash = Hash::new(domain_id.to_string().as_bytes());
            let endorsement = DomainEndorsement {
                version: iroha_data_model::nexus::DOMAIN_ENDORSEMENT_VERSION_V1,
                domain_id: domain_id.clone(),
                committee_id: "default".to_owned(),
                statement_hash,
                issued_at_height: stx.block_height(),
                expires_at_height: stx.block_height() + 5,
                scope: DomainEndorsementScope::default(),
                signatures: Vec::new(),
                metadata: Metadata::default(),
            };
            let msg_hash = endorsement.body_hash();
            persist_endorsement_record(
                &domain_id,
                endorsement,
                stx.block_height(),
                msg_hash,
                &mut stx,
            );

            assert!(
                stx.world
                    .domain_endorsement_policies
                    .get(&domain_id)
                    .is_some(),
                "policy should exist before unregister"
            );
            assert!(
                stx.world
                    .domain_endorsements_by_domain
                    .get(&domain_id)
                    .is_some(),
                "endorsement index should exist before unregister"
            );
            assert!(
                stx.world.domain_endorsements.get(&msg_hash).is_some(),
                "endorsement record should exist before unregister"
            );

            Unregister::domain(domain_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("unregister domain");

            assert!(
                stx.world
                    .domain_endorsement_policies
                    .get(&domain_id)
                    .is_none(),
                "endorsement policy should be removed"
            );
            assert!(
                stx.world
                    .domain_endorsements_by_domain
                    .get(&domain_id)
                    .is_none(),
                "endorsement index should be removed"
            );
            assert!(
                stx.world.domain_endorsements.get(&msg_hash).is_none(),
                "endorsement record should be removed"
            );
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
            let peer_keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer = seed_live_peer(&mut stx, &peer_keypair);

            let err = SetLaneRelayEmergencyValidators {
                lane_id: LaneId::new(0),
                peers: vec![peer],
                expires_at_height: Some(12),
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect_err("permission should be required");
            assert!(matches!(
                err,
                InstructionExecutionError::InvariantViolation(msg)
                    if msg.as_ref() == "not permitted: CanManageLaneRelayEmergency"
            ));
            assert!(
                stx.world
                    .lane_relay_emergency_validators
                    .get(&LaneId::new(0))
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
            grant_manage_lane_relay_emergency_permission(&mut stx, &authority);
            stx.nexus.enabled = false;
            let peer_keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer = seed_live_peer(&mut stx, &peer_keypair);

            let err = SetLaneRelayEmergencyValidators {
                lane_id: LaneId::new(0),
                peers: vec![peer],
                expires_at_height: Some(12),
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
            grant_manage_lane_relay_emergency_permission(&mut stx, &authority);
            let peer_keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer = seed_live_peer(&mut stx, &peer_keypair);

            let err = SetLaneRelayEmergencyValidators {
                lane_id: LaneId::new(0),
                peers: vec![peer],
                expires_at_height: Some(12),
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
            grant_manage_lane_relay_emergency_permission(&mut stx, &ALICE_ID);
            let peer_keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer = seed_live_peer(&mut stx, &peer_keypair);

            let err = SetLaneRelayEmergencyValidators {
                lane_id: LaneId::new(0),
                peers: vec![peer],
                expires_at_height: Some(12),
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
        fn set_lane_relay_emergency_validators_rejects_unknown_lane() {
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
            grant_manage_lane_relay_emergency_permission(&mut stx, &authority);
            let peer_keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer = seed_live_peer(&mut stx, &peer_keypair);
            let unknown = LaneId::new(42);
            let err = SetLaneRelayEmergencyValidators {
                lane_id: unknown,
                peers: vec![peer],
                expires_at_height: Some(12),
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect_err("unknown lane should be rejected");
            let msg = smart_contract_instruction_error_message(err);
            assert!(
                msg.contains("unknown lane id"),
                "unexpected error message: {msg}"
            );
        }

        #[test]
        fn set_lane_relay_emergency_validators_rejects_unregistered_peer() {
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
            grant_manage_lane_relay_emergency_permission(&mut stx, &authority);
            let missing = PeerId::new(
                KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                    .public_key()
                    .clone(),
            );
            let err = SetLaneRelayEmergencyValidators {
                lane_id: LaneId::new(0),
                peers: vec![missing],
                expires_at_height: Some(12),
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect_err("unregistered peer should be rejected");
            let msg = smart_contract_instruction_error_message(err);
            assert!(
                msg.contains("is not registered"),
                "unexpected error message: {msg}"
            );
        }

        #[test]
        fn set_lane_relay_emergency_validators_rejects_peer_without_live_consensus_key() {
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
            grant_manage_lane_relay_emergency_permission(&mut stx, &authority);

            let peer = PeerId::new(
                KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                    .public_key()
                    .clone(),
            );
            let _ = stx.world.peers.push(peer.clone());
            let err = SetLaneRelayEmergencyValidators {
                lane_id: LaneId::new(0),
                peers: vec![peer],
                expires_at_height: Some(12),
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect_err("peer without live consensus key should be rejected");
            let msg = smart_contract_instruction_error_message(err);
            assert!(
                msg.contains("does not have a live consensus key"),
                "unexpected error message: {msg}"
            );
        }

        #[test]
        fn set_lane_relay_emergency_validators_requires_expiry_for_non_empty_roster() {
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
            grant_manage_lane_relay_emergency_permission(&mut stx, &authority);

            let peer_keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer = seed_live_peer(&mut stx, &peer_keypair);
            let err = SetLaneRelayEmergencyValidators {
                lane_id: LaneId::new(0),
                peers: vec![peer],
                expires_at_height: None,
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect_err("missing expiry should be rejected");
            let msg = smart_contract_instruction_error_message(err);
            assert!(
                msg.contains("requires expires_at_height"),
                "unexpected error message: {msg}"
            );
        }

        #[test]
        fn set_lane_relay_emergency_validators_rejects_expiry_beyond_max_ttl() {
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
            grant_manage_lane_relay_emergency_permission(&mut stx, &authority);

            let peer_keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer = seed_live_peer(&mut stx, &peer_keypair);
            let err = SetLaneRelayEmergencyValidators {
                lane_id: LaneId::new(0),
                peers: vec![peer],
                expires_at_height: Some(40),
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect_err("oversized ttl should be rejected");
            let msg = smart_contract_instruction_error_message(err);
            assert!(
                msg.contains("exceeds max_ttl_blocks"),
                "unexpected error message: {msg}"
            );
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
            grant_manage_lane_relay_emergency_permission(&mut stx, &authority);

            let validator_a = seed_live_peer(
                &mut stx,
                &KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            );
            let validator_b = seed_live_peer(
                &mut stx,
                &KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            );

            SetLaneRelayEmergencyValidators {
                lane_id: LaneId::new(0),
                peers: vec![
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
                .get(&LaneId::new(0))
                .expect("override stored");
            let mut expected = vec![validator_a, validator_b];
            expected.sort();
            expected.dedup();
            assert_eq!(record.peers, expected);
            assert_eq!(record.expires_at_height, 12);
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
            grant_manage_lane_relay_emergency_permission(&mut stx, &authority);
            let validator = seed_live_peer(
                &mut stx,
                &KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            );

            SetLaneRelayEmergencyValidators {
                lane_id: LaneId::new(0),
                peers: vec![validator.clone()],
                expires_at_height: Some(12),
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect("set emergency validators");
            assert!(
                stx.world
                    .lane_relay_emergency_validators
                    .get(&LaneId::new(0))
                    .is_some(),
                "override should be stored before clearing"
            );

            SetLaneRelayEmergencyValidators {
                lane_id: LaneId::new(0),
                peers: Vec::new(),
                expires_at_height: None,
                metadata: Metadata::default(),
            }
            .execute(&authority, &mut stx)
            .expect("clear emergency validators");
            assert!(
                stx.world
                    .lane_relay_emergency_validators
                    .get(&LaneId::new(0))
                    .is_none(),
                "override should be removed when peer list is empty"
            );
        }

        fn smart_contract_instruction_error_message(err: InstructionExecutionError) -> String {
            match err {
                InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(msg),
                ) => msg,
                InstructionExecutionError::InvariantViolation(msg) => msg.to_string(),
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
        fn register_domain_rejects_labels_failing_norm_current() {
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
                pop: None,
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
                pop: None,
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
        fn register_domain_duplicate_does_not_persist_endorsement() {
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
                pop: None,
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
            let key: iroha_data_model::name::Name = "endorsement".parse().expect("name");
            let mut domain = Domain::new(domain_id.clone());
            domain.metadata.insert(key.clone(), Json::new(endorsement));

            Register::domain(domain)
                .execute(&ALICE_ID, &mut stx)
                .expect("initial register should succeed");

            let mut endorsement_dupe = DomainEndorsement {
                version: iroha_data_model::nexus::DOMAIN_ENDORSEMENT_VERSION_V1,
                domain_id: canonical_id.clone(),
                committee_id: "default".into(),
                statement_hash,
                issued_at_height: stx.block_height(),
                expires_at_height: stx.block_height() + 20,
                scope: DomainEndorsementScope::default(),
                signatures: Vec::new(),
                metadata: Metadata::default(),
            };
            let msg_hash_dupe = endorsement_dupe.body_hash();
            endorsement_dupe
                .signatures
                .push(DomainEndorsementSignature {
                    signer: kp.public_key().clone(),
                    signature: Signature::new(kp.private_key(), msg_hash_dupe.as_ref()),
                });
            let mut dup_domain = Domain::new(domain_id.clone());
            dup_domain.metadata.insert(key, Json::new(endorsement_dupe));

            let err = Register::domain(dup_domain)
                .execute(&ALICE_ID, &mut stx)
                .expect_err("duplicate domain should be rejected");
            assert!(matches!(err, InstructionExecutionError::Repetition(_)));

            assert!(
                stx.world.domain_endorsements.get(&msg_hash).is_some(),
                "initial endorsement should remain recorded"
            );
            assert!(
                stx.world.domain_endorsements.get(&msg_hash_dupe).is_none(),
                "duplicate attempt must not persist endorsement"
            );
            let hashes = stx
                .world
                .domain_endorsements_by_domain
                .get(&canonical_id)
                .cloned()
                .unwrap_or_default();
            assert!(
                !hashes.contains(&msg_hash_dupe),
                "duplicate endorsement hash must not be indexed"
            );
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
                pop: None,
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

            let block = new_dummy_block_non_genesis();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let params = stx.world.parameters.get().clone();
            let require_hsm = params.sumeragi.key_require_hsm;
            let allowed_hsm_providers = params.sumeragi.key_allowed_hsm_providers.clone();
            let activation_lead_blocks = params.sumeragi.key_activation_lead_blocks;
            let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let peer_id = crate::PeerId::new(bls.public_key().clone());
            let pop = iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("pop");
            let mut isi =
                iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), pop);
            if require_hsm {
                let provider = allowed_hsm_providers
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "softkey".to_string());
                let binding = iroha_data_model::consensus::HsmBinding {
                    provider,
                    key_label: peer_id.public_key().to_string(),
                    slot: Some(1),
                };
                isi = isi.with_hsm(binding);
            }
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
            let expected_activation = stx.block_height().saturating_add(activation_lead_blocks);
            assert_eq!(
                record.activation_height, expected_activation,
                "activation height must obey lead-time policy"
            );
            assert_eq!(
                record.status,
                ConsensusKeyStatus::Pending,
                "lead-time activation should mark the key pending"
            );
            if require_hsm {
                assert!(
                    record.hsm.is_some(),
                    "HSM binding must be populated when key_require_hsm is enabled"
                );
            } else {
                assert!(
                    record.hsm.is_none(),
                    "HSM binding must be empty when key_require_hsm is disabled"
                );
            }
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
            let other_pop = iroha_crypto::bls_normal_pop_prove(other.private_key())
                .expect("pop for conflicting key");
            let bogus = ConsensusKeyRecord {
                id: collision_id.clone(),
                public_key: other.public_key().clone(),
                pop: Some(other_pop),
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

            let block = new_dummy_block_non_genesis();
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
            let _guard = crate::sumeragi::status::peer_key_policy_test_guard();
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
                    pop: Some(pop.clone()),
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

            let block = new_dummy_block_non_genesis();
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
            let pop =
                iroha_crypto::bls_normal_pop_prove(kp.private_key()).expect("pop for validator");
            let hsm = HsmBinding {
                provider: "pkcs11".to_string(),
                key_label: "validator/0".to_string(),
                slot: Some(1),
            };
            let make_record = |activation_height: u64| ConsensusKeyRecord {
                id: id.clone(),
                public_key: pk.clone(),
                pop: Some(pop.clone()),
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
                let params = stx.world.parameters.get_mut();
                params.sumeragi.key_require_hsm = true;
                let params = stx.world.parameters.get().sumeragi.clone();
                stx.apply();
                params
            };

            let mut stx = state_block.transaction();
            let kp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let pop =
                iroha_crypto::bls_normal_pop_prove(kp.private_key()).expect("pop for validator");
            let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-hsm");
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: kp.public_key().clone(),
                pop: Some(pop),
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
                pop: None,
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
            let pop =
                iroha_crypto::bls_normal_pop_prove(kp.private_key()).expect("pop for validator");
            let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-history");
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: kp.public_key().clone(),
                pop: Some(pop),
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
                let pop = iroha_crypto::bls_normal_pop_prove(kp.private_key())
                    .expect("pop for validator");
                let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-bls-ok");
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: kp.public_key().clone(),
                    pop: Some(pop),
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
                    pop: None,
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
                let pop = iroha_crypto::bls_normal_pop_prove(kp.private_key())
                    .expect("pop for validator");
                let id =
                    ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-provider-reject");
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: kp.public_key().clone(),
                    pop: Some(pop),
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
                let params = stx.world.parameters.get_mut();
                params.sumeragi.key_require_hsm = true;
                let params = stx.world.parameters.get().sumeragi.clone();
                stx.apply();
                params
            };

            let mut stx = state_block.transaction();
            let kp_a = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let pop_a =
                iroha_crypto::bls_normal_pop_prove(kp_a.private_key()).expect("pop for validator");
            let id_a = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-hsm-required");
            let record_a = ConsensusKeyRecord {
                id: id_a.clone(),
                public_key: kp_a.public_key().clone(),
                pop: Some(pop_a),
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
            let pop_b =
                iroha_crypto::bls_normal_pop_prove(kp_b.private_key()).expect("pop for validator");
            let id_b = ConsensusKeyId::new(
                ConsensusKeyRole::Validator,
                "validator-hsm-missing-rotation",
            );
            let record_b = ConsensusKeyRecord {
                id: id_b.clone(),
                public_key: kp_b.public_key().clone(),
                pop: Some(pop_b),
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
            let pop_a =
                iroha_crypto::bls_normal_pop_prove(kp_a.private_key()).expect("pop for validator");
            let id_a = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-hsm-optional-a");
            let record_a = ConsensusKeyRecord {
                id: id_a.clone(),
                public_key: kp_a.public_key().clone(),
                pop: Some(pop_a),
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
            let pop_b =
                iroha_crypto::bls_normal_pop_prove(kp_b.private_key()).expect("pop for validator");
            let id_b = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-hsm-optional-b");
            let record_b = ConsensusKeyRecord {
                id: id_b.clone(),
                public_key: kp_b.public_key().clone(),
                pop: Some(pop_b),
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
                let pop = iroha_crypto::bls_normal_pop_prove(kp.private_key())
                    .expect("pop for validator");
                let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-empty-algos");
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: kp.public_key().clone(),
                    pop: Some(pop),
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
                    pop: None,
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
                    pop: None,
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
            let pop_a =
                iroha_crypto::bls_normal_pop_prove(kp_a.private_key()).expect("pop for validator");
            let id_a = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-primary");
            let record_a = ConsensusKeyRecord {
                id: id_a.clone(),
                public_key: kp_a.public_key().clone(),
                pop: Some(pop_a),
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
            let pop_b =
                iroha_crypto::bls_normal_pop_prove(kp_b.private_key()).expect("pop for validator");
            let id_b = ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator-next");
            let record_b = ConsensusKeyRecord {
                id: id_b.clone(),
                public_key: kp_b.public_key().clone(),
                pop: Some(pop_b),
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
            let mut state = State::new(World::default(), kura, query_handle);
            state.nexus.get_mut().enabled = false;

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
        fn next_mode_rejected_when_nexus_enabled() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            state.nexus.get_mut().enabled = true;

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let next_mode = SetParameter(Parameter::Sumeragi(SumeragiParameter::NextMode(
                SumeragiConsensusMode::Npos,
            )));
            let err = next_mode
                .execute(&ALICE_ID, &mut stx)
                .expect_err("nexus should reject staged consensus cutovers");
            match err {
                Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => assert!(
                    msg.contains("Nexus networks do not support staged consensus cutovers"),
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
        fn set_parameter_rejects_zero_min_finality() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let update = SetParameter(Parameter::Sumeragi(SumeragiParameter::MinFinalityMs(0)));
            let err = update
                .execute(&ALICE_ID, &mut stx)
                .expect_err("min_finality_ms=0 must be rejected");
            match err {
                Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                    assert_eq!(msg, "sumeragi.min_finality_ms must be greater than zero")
                }
                other => panic!("unexpected error type: {other:?}"),
            }
        }

        #[test]
        fn set_parameter_rejects_block_time_below_min_finality() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let update = SetParameter(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(50)));
            let err = update
                .execute(&ALICE_ID, &mut stx)
                .expect_err("block_time_ms below min_finality must be rejected");
            match err {
                Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                    assert_eq!(
                        msg,
                        "sumeragi.block_time_ms must be greater than or equal to sumeragi.min_finality_ms"
                    )
                }
                other => panic!("unexpected error type: {other:?}"),
            }
        }

        #[test]
        fn set_parameter_rejects_commit_time_below_block_time() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let update = SetParameter(Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(50)));
            let err = update
                .execute(&ALICE_ID, &mut stx)
                .expect_err("commit_time_ms below block_time_ms must be rejected");
            match err {
                Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                    assert_eq!(
                        msg,
                        "sumeragi.commit_time_ms must be greater than or equal to sumeragi.block_time_ms"
                    )
                }
                other => panic!("unexpected error type: {other:?}"),
            }
        }

        #[test]
        fn set_parameter_allows_sequential_sumeragi_timing_updates() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            SetParameter(Parameter::Sumeragi(SumeragiParameter::MinFinalityMs(100)))
                .execute(&ALICE_ID, &mut stx)
                .expect("min_finality should accept 100");
            SetParameter(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(100)))
                .execute(&ALICE_ID, &mut stx)
                .expect("block_time should accept 100 after min_finality update");
            SetParameter(Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(100)))
                .execute(&ALICE_ID, &mut stx)
                .expect("commit_time should accept 100 after block_time update");
            SetParameter(Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(667)))
                .execute(&ALICE_ID, &mut stx)
                .expect("commit_time should accept 667 before block_time increase");
            SetParameter(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(333)))
                .execute(&ALICE_ID, &mut stx)
                .expect("block_time should accept 333 after commit_time increase");

            let params = stx.world.parameters.get().sumeragi().clone();
            assert_eq!(params.min_finality_ms(), 100);
            assert_eq!(params.block_time_ms(), 333);
            assert_eq!(params.commit_time_ms(), 667);
        }

        #[test]
        fn set_parameter_rejects_zero_npos_reconfig_fields() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());

            {
                let mut stx = state_block.transaction();
                let params = SumeragiNposParameters {
                    evidence_horizon_blocks: 0,
                    ..Default::default()
                };
                let update = SetParameter(Parameter::Custom(params.into_custom_parameter()));
                let err = update
                    .execute(&ALICE_ID, &mut stx)
                    .expect_err("evidence_horizon_blocks=0 must be rejected");
                match err {
                    Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                        assert_eq!(
                            msg,
                            "sumeragi.npos.reconfig.evidence_horizon_blocks must be greater than zero"
                        )
                    }
                    other => panic!("unexpected error type: {other:?}"),
                }
            }

            {
                let mut stx = state_block.transaction();
                let params = SumeragiNposParameters {
                    activation_lag_blocks: 0,
                    ..Default::default()
                };
                let update = SetParameter(Parameter::Custom(params.into_custom_parameter()));
                let err = update
                    .execute(&ALICE_ID, &mut stx)
                    .expect_err("activation_lag_blocks=0 must be rejected");
                match err {
                    Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                        assert_eq!(
                            msg,
                            "sumeragi.npos.reconfig.activation_lag_blocks must be greater than zero"
                        )
                    }
                    other => panic!("unexpected error type: {other:?}"),
                }
            }

            {
                let mut stx = state_block.transaction();
                let params = SumeragiNposParameters {
                    slashing_delay_blocks: 0,
                    ..Default::default()
                };
                let update = SetParameter(Parameter::Custom(params.into_custom_parameter()));
                let err = update
                    .execute(&ALICE_ID, &mut stx)
                    .expect_err("slashing_delay_blocks=0 must be rejected");
                match err {
                    Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                        assert_eq!(
                            msg,
                            "sumeragi.npos.reconfig.slashing_delay_blocks must be greater than zero"
                        )
                    }
                    other => panic!("unexpected error type: {other:?}"),
                }
            }
        }

        #[test]
        fn next_mode_without_activation_height_rejected_at_commit() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), kura, query_handle);
            state.nexus.get_mut().enabled = false;

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
            let mut state = State::new(World::default(), kura, query_handle);
            state.nexus.get_mut().enabled = false;

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
            let mut state = State::new(World::default(), kura, query_handle);
            state.nexus.get_mut().enabled = false;

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
                msg.contains("backend cannot change"),
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
                    pop: None,
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
        fn activate_contract_instance_is_public_for_unprotected_namespace() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new_for_testing(World::default(), kura, query_handle);
            let header = iroha_data_model::block::BlockHeader::new(
                NonZeroU64::new(1).unwrap(),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let mut stx = block.transaction();
            Register::account(Account::new(ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("seed authority");

            let code_hash = Hash::new(b"public-contract");
            let manifest = ContractManifest {
                code_hash: Some(code_hash),
                abi_hash: None,
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                kotoba: None,
                provenance: None,
            };
            stx.world.contract_manifests.insert(code_hash, manifest);

            scode::ActivateContractInstance {
                namespace: "public".to_owned(),
                contract_id: "demo".to_owned(),
                code_hash,
            }
            .execute(&ALICE_ID, &mut stx)
            .expect("unprotected namespace should not require governance");

            let binding = ("public".to_owned(), "demo".to_owned());
            assert_eq!(stx.world.contract_instances.get(&binding), Some(&code_hash));
        }

        #[test]
        fn activate_contract_instance_requires_governance_for_protected_namespace() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new_for_testing(World::default(), kura, query_handle);
            let header = iroha_data_model::block::BlockHeader::new(
                NonZeroU64::new(1).unwrap(),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let mut stx = block.transaction();
            Register::account(Account::new(ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("seed authority");

            let protected = iroha_data_model::parameter::custom::CustomParameter::new(
                iroha_data_model::parameter::custom::CustomParameterId(
                    "gov_protected_namespaces".parse().expect("parameter id"),
                ),
                Json::new(vec!["protected".to_owned()]),
            );
            stx.world
                .parameters
                .get_mut()
                .set_parameter(Parameter::Custom(protected));

            let code_hash = Hash::new(b"protected-contract");
            let manifest = ContractManifest {
                code_hash: Some(code_hash),
                abi_hash: None,
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                kotoba: None,
                provenance: None,
            };
            stx.world.contract_manifests.insert(code_hash, manifest);

            let err = scode::ActivateContractInstance {
                namespace: "protected".to_owned(),
                contract_id: "demo".to_owned(),
                code_hash,
            }
            .execute(&ALICE_ID, &mut stx)
            .expect_err("protected namespace must remain governance-gated");

            let msg = smart_contract_error_message(
                iroha_data_model::ValidationFail::InstructionFailed(err),
            );
            assert!(
                msg.contains("CanEnactGovernance"),
                "unexpected protected-namespace error: {msg}"
            );
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
                let model = state_ro.world().executor_data_model().clone();
                if model.permissions().is_empty() {
                    return Ok(crate::executor::initial_executor_data_model_fallback());
                }
                Ok(model)
            }
        }

        impl ValidSingularQuery for iroha_data_model::query::runtime::prelude::FindAbiVersion {
            #[metrics(+"find_abi_version")]
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<iroha_data_model::query::runtime::AbiVersion, Error> {
                Ok(iroha_data_model::query::runtime::AbiVersion {
                    abi_version: state_ro.world().abi_version(),
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

        impl ValidSingularQuery for iroha_data_model::query::nexus::prelude::FindLaneRelayEnvelopeByRef {
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<VerifiedLaneRelayRecord, Error> {
                load_verified_lane_relay_record(state_ro, &self.relay_ref)
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
