use core::convert::TryFrom;
use std::{collections::BTreeSet, str::FromStr, sync::OnceLock};

use blake3::hash as blake3_hash;
use iroha_crypto::{Algorithm, PublicKey, ed25519_parse_signature};
use iroha_data_model::{
    asset::AssetId,
    events::data::sorafs::{SorafsGatewayEvent, SorafsProofHealthAlert},
    isi::error::{InstructionExecutionError, InvalidParameterError},
    metadata::Metadata,
    name::Name,
    permission::{Permission, Permissions},
    sorafs::{
        capacity::{
            CapacityAccrual, CapacityDeclarationRecord, CapacityDisputeEvidence, CapacityDisputeId,
            CapacityDisputeRecord, CapacityDisputeStatus, CapacityFeeLedgerEntry,
            CapacityTelemetryRecord, ProviderId,
        },
        pin_registry::{
            ChunkerProfileHandle, ManifestAliasBinding, ManifestAliasId, ManifestAliasRecord,
            ManifestDigest, ManifestRootCid, PinFeePayment, PinManifestRecord, PinPolicy,
            PinStatus, ReplicationOrderId, ReplicationOrderRecord, ReplicationOrderStatus,
            StorageClass,
        },
        pricing::{
            PricingComputationError, PricingScheduleRecord, ProviderCreditRecord,
            XOR_QUANTITY_SCALE, checked_mul_div_round_u128,
        },
    },
};
use iroha_executor_data_model::permission::sorafs::CanOperateSorafsRepair;
use iroha_primitives::{
    json::Json,
    numeric::{NumericOperationError, Quantity, RoundingMode},
};
use mv::storage::{StorageReadOnly, Transaction as StorageTransaction};
use norito::{
    DecodeLimits, decode_from_bytes_with_limits,
    json::{self, Value},
};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, ManifestValidationError, PinPolicy as ManifestPinPolicy,
    PinPolicyConstraints as ManifestPinPolicyConstraints, ProfileId,
    StorageClass as ManifestStorageClass,
    alias_cache::decode_alias_proof,
    capacity::{
        CAPACITY_DISPUTE_VERSION_V1, CapacityDeclarationV1, CapacityDisputeEvidenceV1,
        CapacityDisputeKind, CapacityDisputeV1, CapacityMetadataEntry,
        REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1,
        ReplicationOrderV1,
    },
    validate_chunker_handle, validate_manifest, validate_manifest_root_cid, validate_pin_policy,
};

use super::*;
use crate::state::StateTransaction;

/// Convert governance configuration into manifest validation constraints.
pub fn manifest_pin_policy_constraints_from_config(
    config: &iroha_config::parameters::actual::SorafsPinPolicyConstraints,
) -> ManifestPinPolicyConstraints {
    let allowed_storage_classes = config.allowed_storage_classes.as_ref().map(|set| {
        set.iter()
            .copied()
            .map(convert_storage_class)
            .collect::<BTreeSet<_>>()
    });

    ManifestPinPolicyConstraints {
        min_replicas_floor: config.min_replicas_floor,
        max_replicas_ceiling: config.max_replicas_ceiling,
        max_retention_epoch: config.max_retention_epoch,
        allowed_storage_classes,
        require_council_signatures: config.require_council_signatures,
    }
}

fn manifest_hex(digest: &ManifestDigest) -> String {
    hex::encode(digest.as_bytes())
}

fn round_xor_quantity_ratio(
    value: &Quantity,
    multiplier: u128,
    divisor: u128,
) -> Result<Quantity, NumericOperationError> {
    let multiplier = Quantity::from(multiplier);
    let divisor = Quantity::from(divisor);
    value.try_mul_div_decimal_round(
        multiplier.as_numeric(),
        divisor.as_numeric(),
        XOR_QUANTITY_SCALE,
        RoundingMode::NearestAway,
    )
}

fn quantity_arithmetic_error(
    context: &str,
    error: NumericOperationError,
) -> InstructionExecutionError {
    InstructionExecutionError::InvariantViolation(
        format!("SoraFS {context} calculation failed: {error}").into(),
    )
}

fn pricing_computation_error(
    context: &str,
    error: PricingComputationError,
) -> InstructionExecutionError {
    InstructionExecutionError::InvariantViolation(
        format!("SoraFS {context} calculation failed: {error}").into(),
    )
}

const STORAGE_CLASS_METADATA_KEY: &str = "sorafs.storage_class";
const PROVIDER_OWNER_METADATA_KEY: &str = "sorafs.owner_account_id";
const MAX_COUNCIL_ENVELOPE_BYTES: usize = 1024 * 1024;
const MAX_COUNCIL_ENVELOPE_SIGNATURES: usize = 64;
const MAX_COUNCIL_ENVELOPE_PROFILE_ALIASES: usize = 16;
const MAX_COUNCIL_ENVELOPE_MANIFEST_NAME_BYTES: usize = 255;
const MAX_RETIREMENT_REASON_BYTES: usize = 1024;
const MAX_ALIAS_PROOF_BYTES: usize = 1024 * 1024;
const MAX_REPLICATION_ORDER_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_CAPACITY_DECLARATION_PAYLOAD_BYTES: usize = 256 * 1024;
const MAX_CAPACITY_DISPUTE_PAYLOAD_BYTES: usize = 64 * 1024;
const REPLICATION_ORDER_DECODE_LIMITS: DecodeLimits = DecodeLimits::new(
    sorafs_manifest::capacity::MAX_CAPACITY_METADATA_VALUE_BYTES,
    MAX_REPLICATION_ORDER_PAYLOAD_BYTES,
    65_536,
    MAX_REPLICATION_ORDER_PAYLOAD_BYTES * 4,
    32,
);
const CAPACITY_DECLARATION_DECODE_LIMITS: DecodeLimits = DecodeLimits::new(
    sorafs_manifest::capacity::MAX_CAPACITY_METADATA_VALUE_BYTES,
    MAX_CAPACITY_DECLARATION_PAYLOAD_BYTES,
    131_072,
    MAX_CAPACITY_DECLARATION_PAYLOAD_BYTES * 4,
    32,
);
const CAPACITY_DISPUTE_DECODE_LIMITS: DecodeLimits = DecodeLimits::new(
    2_048,
    MAX_CAPACITY_DISPUTE_PAYLOAD_BYTES,
    8_192,
    MAX_CAPACITY_DISPUTE_PAYLOAD_BYTES * 4,
    24,
);

fn decode_capacity_declaration_payload(bytes: &[u8]) -> Result<CapacityDeclarationV1, String> {
    if bytes.is_empty() || bytes.len() > MAX_CAPACITY_DECLARATION_PAYLOAD_BYTES {
        return Err(format!(
            "payload has {} bytes; expected 1..={MAX_CAPACITY_DECLARATION_PAYLOAD_BYTES}",
            bytes.len()
        ));
    }
    let declaration = decode_from_bytes_with_limits::<CapacityDeclarationV1>(
        bytes,
        CAPACITY_DECLARATION_DECODE_LIMITS,
    )
    .map_err(|error| error.to_string())?;
    let canonical = norito::to_bytes(&declaration).map_err(|error| error.to_string())?;
    if canonical != bytes {
        return Err("payload must use canonical first-release Norito".to_owned());
    }
    Ok(declaration)
}

fn decode_capacity_dispute_payload(bytes: &[u8]) -> Result<CapacityDisputeV1, String> {
    if bytes.is_empty() || bytes.len() > MAX_CAPACITY_DISPUTE_PAYLOAD_BYTES {
        return Err(format!(
            "payload has {} bytes; expected 1..={MAX_CAPACITY_DISPUTE_PAYLOAD_BYTES}",
            bytes.len()
        ));
    }
    let dispute =
        decode_from_bytes_with_limits::<CapacityDisputeV1>(bytes, CAPACITY_DISPUTE_DECODE_LIMITS)
            .map_err(|error| error.to_string())?;
    let canonical = norito::to_bytes(&dispute).map_err(|error| error.to_string())?;
    if canonical != bytes {
        return Err("payload must use canonical first-release Norito".to_owned());
    }
    Ok(dispute)
}

fn storage_class_metadata_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| {
        STORAGE_CLASS_METADATA_KEY
            .parse()
            .expect("static storage class metadata key must parse")
    })
}

fn parse_storage_class_label(
    provider_id: ProviderId,
    value: &str,
) -> Result<StorageClass, InstructionExecutionError> {
    let provider_hex = hex::encode(provider_id.as_bytes());
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(invalid_parameter(format!(
            "capacity declaration metadata `{STORAGE_CLASS_METADATA_KEY}` for provider {provider_hex} must not be empty"
        )));
    }

    let normalized = trimmed.to_ascii_lowercase();
    let class = match normalized.as_str() {
        "hot" => StorageClass::Hot,
        "warm" => StorageClass::Warm,
        "cold" => StorageClass::Cold,
        _ => {
            return Err(invalid_parameter(format!(
                "capacity declaration metadata `{STORAGE_CLASS_METADATA_KEY}` for provider {provider_hex} must be one of hot, warm, or cold (found `{trimmed}`)"
            )));
        }
    };

    Ok(class)
}

fn storage_class_from_declaration_metadata(
    provider_id: ProviderId,
    metadata: &Metadata,
    default: StorageClass,
) -> Result<StorageClass, InstructionExecutionError> {
    let Some(json_value) = metadata.get(storage_class_metadata_key()) else {
        return Ok(default);
    };

    let value: String = json_value.try_into_any().map_err(|err| {
        invalid_parameter(format!(
            "capacity declaration metadata `{STORAGE_CLASS_METADATA_KEY}` for provider {} must be a string: {err}",
            hex::encode(provider_id.as_bytes())
        ))
    })?;
    parse_storage_class_label(provider_id, &value)
}

fn storage_class_from_declaration_record(
    record: &CapacityDeclarationRecord,
    default: StorageClass,
) -> Result<StorageClass, InstructionExecutionError> {
    if record.metadata.get(storage_class_metadata_key()).is_some() {
        return storage_class_from_declaration_metadata(
            record.provider_id,
            &record.metadata,
            default,
        );
    }

    let provider_id = record.provider_id;
    let provider_hex = hex::encode(provider_id.as_bytes());
    let declaration = decode_capacity_declaration_payload(&record.declaration).map_err(|err| {
        invalid_parameter(format!(
            "invalid capacity declaration payload for provider {provider_hex}: {err}"
        ))
    })?;

    for entry in &declaration.metadata {
        if entry.key.trim() == STORAGE_CLASS_METADATA_KEY {
            return parse_storage_class_label(provider_id, &entry.value);
        }
    }

    Ok(default)
}

fn merge_declaration_metadata_into_record(
    provider_id: ProviderId,
    record_metadata: &mut Metadata,
    declaration_metadata: &[CapacityMetadataEntry],
) -> Result<(), InstructionExecutionError> {
    if declaration_metadata.is_empty() {
        return Ok(());
    }

    let provider_hex = hex::encode(provider_id.as_bytes());
    for entry in declaration_metadata {
        let key: Name = entry.key.parse().map_err(|err| {
            invalid_parameter(format!(
                "capacity declaration metadata key `{}` for provider {} is invalid: {err}",
                entry.key, provider_hex
            ))
        })?;

        let payload_value_trimmed = entry.value.trim();
        if let Some(existing) = record_metadata.get(&key) {
            let existing_str: String = existing.try_into_any().map_err(|err| {
                invalid_parameter(format!(
                    "capacity declaration metadata `{}` for provider {} must be a string to match payload: {err}",
                    entry.key, provider_hex
                ))
            })?;
            if existing_str.trim() != payload_value_trimmed {
                return Err(invalid_parameter(format!(
                    "capacity declaration metadata conflict for provider {} on key `{}`: record value `{}`, payload value `{}`",
                    provider_hex, entry.key, existing_str, entry.value
                )));
            }
            continue;
        }

        record_metadata.insert(key, Json::new(payload_value_trimmed));
    }

    Ok(())
}

fn owner_literal_matches_authority(authority: &AccountId, literal: &str) -> bool {
    literal == authority.to_string()
}

fn same_account_subject(left: &AccountId, right: &AccountId) -> bool {
    left.subject_id() == right.subject_id()
}

fn enforce_provider_owner(
    world: &impl crate::state::WorldReadOnly,
    dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
    authority: &AccountId,
    metadata: &Metadata,
    provider_hex: &str,
) -> Result<(), InstructionExecutionError> {
    let key = Name::from_str(PROVIDER_OWNER_METADATA_KEY).expect("static metadata key");
    let Some(value) = metadata.get(&key) else {
        return Err(invalid_parameter(format!(
            "capacity declaration metadata `{PROVIDER_OWNER_METADATA_KEY}` for provider {provider_hex} must be present"
        )));
    };

    let owner_str: String = value.try_into_any().map_err(|err| {
        invalid_parameter(format!(
            "capacity declaration metadata `{PROVIDER_OWNER_METADATA_KEY}` for provider {provider_hex} must be a string: {err}"
        ))
    })?;

    let owner_literal = owner_str.trim();
    if let Some(owner) =
        crate::block::parse_account_literal_with_world(world, dataspace_catalog, owner_literal)
    {
        if same_account_subject(&owner, authority) {
            return Ok(());
        }
        return Err(invalid_parameter(format!(
            "capacity declaration metadata `{PROVIDER_OWNER_METADATA_KEY}` for provider {provider_hex} must match the submitting authority"
        )));
    }

    if owner_literal_matches_authority(authority, owner_literal) {
        return Ok(());
    }

    Err(invalid_parameter(format!(
        "capacity declaration metadata `{PROVIDER_OWNER_METADATA_KEY}` for provider {provider_hex} must be a canonical I105 account id or on-chain alias matching the submitting authority"
    )))
}

fn ensure_provider_owner_matches_authority(
    authority: &AccountId,
    record: &CapacityDeclarationRecord,
    world: &impl crate::state::WorldReadOnly,
    dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
) -> Result<(), InstructionExecutionError> {
    let provider_hex = hex::encode(record.provider_id.as_bytes());
    enforce_provider_owner(
        world,
        dataspace_catalog,
        authority,
        &record.metadata,
        &provider_hex,
    )
}

fn ensure_provider_owner_registered(
    state_transaction: &StateTransaction<'_, '_>,
    provider: &ProviderId,
    authority: &AccountId,
) -> Result<(), InstructionExecutionError> {
    if let Some(owner) = state_transaction.world.provider_owners.get(provider) {
        if !same_account_subject(owner, authority) {
            return Err(invalid_parameter(format!(
                "provider {provider:?} owned by {owner}, but {authority} attempted a SoraFS operation"
            )));
        }
        return Ok(());
    }

    Err(invalid_parameter(format!(
        "provider {provider:?} has no registered owner"
    )))
}

impl Execute for iroha_data_model::isi::sorafs::RegisterProviderOwner {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(
            state_transaction,
            authority,
            "CanRegisterSorafsProviderOwner",
        )?;

        let provider_hex = hex::encode(self.provider_id.as_bytes());
        if let Some(existing) = state_transaction
            .world
            .provider_owners
            .get(&self.provider_id)
        {
            if same_account_subject(existing, &self.owner) {
                return Ok(());
            }
            return Err(invalid_parameter(format!(
                "provider {provider_hex} already owned by {existing} and cannot be rebound to {}",
                self.owner
            )));
        }

        state_transaction.world.account(&self.owner)?;

        state_transaction
            .world
            .provider_owners
            .insert(self.provider_id, self.owner.clone());
        grant_repair_worker_permission(state_transaction, &self.owner, self.provider_id);

        Ok(())
    }
}

impl Execute for iroha_data_model::isi::sorafs::UnregisterProviderOwner {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(
            state_transaction,
            authority,
            "CanUnregisterSorafsProviderOwner",
        )?;

        let removed = state_transaction
            .world
            .provider_owners
            .remove(self.provider_id);
        let Some(owner) = removed else {
            return Err(invalid_parameter(format!(
                "provider {} has no registered owner",
                hex::encode(self.provider_id.as_bytes())
            )));
        };
        revoke_repair_worker_permission(state_transaction, &owner, self.provider_id);

        Ok(())
    }
}

impl Execute for iroha_data_model::isi::sorafs::RegisterPinManifest {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Self {
            manifest_payload,
            submitted_epoch,
            mut alias,
            successor_of,
        } = self;

        if manifest_payload.is_empty()
            || manifest_payload.len() > sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES
        {
            return Err(invalid_parameter(format!(
                "manifest payload has {} bytes; expected 1..={}",
                manifest_payload.len(),
                sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES,
            )));
        }
        let manifest =
            sorafs_manifest::decode_manifest_v1_canonical(&manifest_payload).map_err(|error| {
                invalid_parameter(format!("invalid canonical ManifestV1 payload: {error}"))
            })?;
        let mut manifest_constraints =
            manifest_pin_policy_constraints_from_config(&state_transaction.gov.sorafs_pin_policy);
        // The separately submitted approval envelope remains the authoritative
        // council gate. Embedded manifest signatures, when present, are still
        // verified by `validate_manifest`.
        manifest_constraints.require_council_signatures = false;
        validate_manifest(&manifest, &manifest_constraints).map_err(|error| {
            invalid_parameter(format!("manifest payload failed validation: {error}"))
        })?;

        let chunk_digest_sha3_256 = manifest.chunk_digest_sha3_256;
        let digest = ManifestDigest::from_manifest(&manifest).map_err(|error| {
            invalid_parameter(format!("failed to derive manifest digest: {error}"))
        })?;
        let root_cid = ManifestRootCid::try_from_slice(&manifest.root_cid).map_err(|error| {
            invalid_parameter(format!("invalid manifest root CID width: {error}"))
        })?;
        let chunker = ChunkerProfileHandle {
            profile_id: manifest.chunking.profile_id.0,
            namespace: manifest.chunking.namespace.clone(),
            name: manifest.chunking.name.clone(),
            semver: manifest.chunking.semver.clone(),
            multihash_code: manifest.chunking.multihash_code,
        };
        let content_length = manifest.content_length;
        let policy = PinPolicy {
            min_replicas: manifest.pin_policy.min_replicas,
            storage_class: match manifest.pin_policy.storage_class {
                ManifestStorageClass::Hot => StorageClass::Hot,
                ManifestStorageClass::Warm => StorageClass::Warm,
                ManifestStorageClass::Cold => StorageClass::Cold,
            },
            retention_epoch: manifest.pin_policy.retention_epoch,
        };

        if policy.retention_epoch <= submitted_epoch {
            return Err(invalid_parameter(format!(
                "manifest retention epoch {} must be greater than submission epoch {submitted_epoch}",
                policy.retention_epoch,
            )));
        }
        ensure_chunker_handle(&chunker)?;
        ensure_pin_policy(&policy, &state_transaction.gov.sorafs_pin_policy)?;

        if let Some(binding) = alias.as_mut() {
            require_permission(state_transaction, authority, "CanBindSorafsAlias")?;
            let canonical_proof = validate_manifest_alias_binding(
                binding,
                &digest,
                &root_cid,
                Some((submitted_epoch, policy.retention_epoch)),
            )?;
            binding.proof = canonical_proof;
            ensure_alias_unique(
                binding,
                &state_transaction.world.pin_manifests,
                &state_transaction.world.manifest_aliases,
                None,
            )?;
        }

        if let Some(successor_of) = &successor_of {
            if successor_of.as_bytes().iter().all(|byte| *byte == 0) {
                return Err(invalid_parameter(
                    "successor manifest digest must not be zero",
                ));
            }
            ensure_successor_chain(
                &state_transaction.world.pin_manifests,
                &digest,
                successor_of,
            )?;
        }

        if state_transaction.world.pin_manifests.get(&digest).is_some() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("manifest {} already registered", manifest_hex(&digest)).into(),
            ));
        }

        let mut record = PinManifestRecord::new(
            digest,
            root_cid,
            chunker,
            chunk_digest_sha3_256,
            policy,
            authority.clone(),
            submitted_epoch,
            alias.clone(),
            successor_of,
            Metadata::default(),
        )
        .with_content_length(content_length);

        let requires_council_approval = state_transaction
            .gov
            .sorafs_pin_policy
            .require_council_signatures;
        let auto_order = if requires_council_approval {
            // Consensus must retain governed submissions as pending. Torii-side
            // manifest checks are only an early rejection layer and can be
            // bypassed by clients submitting the instruction directly.
            None
        } else {
            let auto_providers = select_auto_replication_providers(
                state_transaction,
                &record.chunker,
                &record.policy,
                submitted_epoch,
            )?;
            build_auto_replication_order(&record, authority, submitted_epoch, &auto_providers)?
        };

        // Keep every fallible validation/allocation step ahead of fee movement.
        let pin_fee_payment = collect_public_pin_fee(
            state_transaction,
            authority,
            &policy,
            content_length,
            submitted_epoch,
        )?;
        record.record_pin_fee_payment(pin_fee_payment);

        if !requires_council_approval {
            record.approve(submitted_epoch, None);

            if let Some(alias) = &record.alias {
                ensure_alias_unique(
                    alias,
                    &state_transaction.world.pin_manifests,
                    &state_transaction.world.manifest_aliases,
                    Some(&digest),
                )?;
                bind_alias_record(
                    state_transaction,
                    alias,
                    &digest,
                    authority,
                    submitted_epoch,
                    record.policy.retention_epoch,
                );
            }
        }

        state_transaction.world.pin_manifests.insert(digest, record);
        if let Some(order) = auto_order {
            state_transaction
                .world
                .replication_orders
                .insert(order.order_id, order);
        }

        Ok(())
    }
}

#[allow(clippy::too_many_lines)]
impl Execute for iroha_data_model::isi::sorafs::ApprovePinManifest {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(state_transaction, authority, "CanApproveSorafsPin")?;

        let Some(mut record) = state_transaction
            .world
            .pin_manifests
            .get(&self.digest)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("manifest {} not registered", manifest_hex(&self.digest)).into(),
            ));
        };

        if self.approved_epoch < record.submitted_epoch {
            return Err(invalid_parameter(format!(
                "manifest {} approval epoch {} predates submission epoch {}",
                manifest_hex(&self.digest),
                self.approved_epoch,
                record.submitted_epoch,
            )));
        }
        if self.approved_epoch >= record.policy.retention_epoch {
            return Err(invalid_parameter(format!(
                "manifest {} approval epoch {} must be earlier than retention epoch {}",
                manifest_hex(&self.digest),
                self.approved_epoch,
                record.policy.retention_epoch,
            )));
        }

        let was_pending = matches!(record.status, PinStatus::Pending);

        let envelope_digest_from_envelope = self
            .council_envelope
            .as_deref()
            .map(|envelope| verify_council_envelope(&record, envelope))
            .transpose()?;

        if let (Some(provided), Some(computed)) =
            (self.council_envelope_digest, envelope_digest_from_envelope)
            && provided != computed
        {
            return Err(invalid_parameter(format!(
                "manifest {} approval digest mismatch with provided envelope",
                manifest_hex(&self.digest)
            )));
        }

        let existing_digest = record.council_envelope_digest;

        let digest_to_store = match record.status {
            PinStatus::Pending => envelope_digest_from_envelope.ok_or_else(|| {
                invalid_parameter(format!(
                    "manifest {} approval requires council envelope payload",
                    manifest_hex(&self.digest)
                ))
            })?,
            PinStatus::Approved(existing_epoch) if existing_epoch == self.approved_epoch => {
                if let Some(existing) = existing_digest {
                    if envelope_digest_from_envelope.is_some_and(|digest| digest != existing)
                        || self
                            .council_envelope_digest
                            .is_some_and(|digest| digest != existing)
                    {
                        return Err(invalid_parameter(format!(
                            "manifest {} re-approval cannot replace its stored council envelope digest",
                            manifest_hex(&self.digest)
                        )));
                    }
                    existing
                } else {
                    envelope_digest_from_envelope.ok_or_else(|| {
                        invalid_parameter(format!(
                            "manifest {} re-approval requires council envelope payload because no digest is stored",
                            manifest_hex(&self.digest)
                        ))
                    })?
                }
            }
            PinStatus::Approved(_) => {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "manifest {} already approved with different epoch",
                        manifest_hex(&self.digest)
                    )
                    .into(),
                ));
            }
            PinStatus::Retired(_) => {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "manifest {} is retired and cannot be approved",
                        manifest_hex(&self.digest)
                    )
                    .into(),
                ));
            }
        };

        record.approve(self.approved_epoch, Some(digest_to_store));

        if let Some(alias) = &record.alias {
            ensure_alias_unique(
                alias,
                &state_transaction.world.pin_manifests,
                &state_transaction.world.manifest_aliases,
                Some(&self.digest),
            )?;
        }

        let auto_order = if was_pending {
            let auto_providers = select_auto_replication_providers(
                state_transaction,
                &record.chunker,
                &record.policy,
                self.approved_epoch,
            )?;
            build_auto_replication_order(&record, authority, self.approved_epoch, &auto_providers)?
        } else {
            None
        };

        // Do not publish an alias until every fallible approval step has
        // completed. This keeps a failed automatic-order build from exposing a
        // binding for a manifest that remains pending.
        if let Some(alias) = &record.alias {
            bind_alias_record(
                state_transaction,
                alias,
                &self.digest,
                authority,
                self.approved_epoch,
                record.policy.retention_epoch,
            );
        }

        state_transaction
            .world
            .pin_manifests
            .insert(self.digest, record);
        if let Some(order) = auto_order {
            state_transaction
                .world
                .replication_orders
                .insert(order.order_id, order);
        }

        Ok(())
    }
}

fn invalid_parameter(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
}

fn has_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission: &str,
) -> bool {
    state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|perms| perms.iter().any(|perm| perm.name() == permission))
}

fn require_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission: &str,
) -> Result<(), Error> {
    if has_permission(state_transaction, authority, permission) {
        Ok(())
    } else {
        Err(invalid_parameter(format!(
            "permission {permission} required for SoraFS operation"
        )))
    }
}

fn grant_repair_worker_permission(
    state_transaction: &mut StateTransaction<'_, '_>,
    owner: &AccountId,
    provider_id: ProviderId,
) {
    let permission = Permission::from(CanOperateSorafsRepair { provider_id });
    if let Some(perms) = state_transaction.world.account_permissions.get_mut(owner) {
        perms.insert(permission);
    } else {
        let mut perms = Permissions::new();
        perms.insert(permission);
        state_transaction
            .world
            .account_permissions
            .insert(owner.clone(), perms);
    }
}

fn revoke_repair_worker_permission(
    state_transaction: &mut StateTransaction<'_, '_>,
    owner: &AccountId,
    provider_id: ProviderId,
) {
    let permission = Permission::from(CanOperateSorafsRepair { provider_id });
    if let Some(perms) = state_transaction.world.account_permissions.get_mut(owner) {
        perms.remove(&permission);
    }
}

#[allow(clippy::too_many_lines)]
fn verify_council_envelope(
    record: &PinManifestRecord,
    envelope: &[u8],
) -> Result<[u8; 32], InstructionExecutionError> {
    let manifest_label = manifest_hex(&record.digest);
    if envelope.is_empty() || envelope.len() > MAX_COUNCIL_ENVELOPE_BYTES {
        return Err(invalid_parameter(format!(
            "council envelope for manifest {manifest_label} is {} bytes; expected 1..={MAX_COUNCIL_ENVELOPE_BYTES}",
            envelope.len(),
        )));
    }
    let parsed: Value = json::from_slice(envelope).map_err(|err| {
        invalid_parameter(format!(
            "invalid council envelope JSON for manifest {manifest_label}: {err}"
        ))
    })?;
    let obj = parsed.as_object().ok_or_else(|| {
        invalid_parameter(format!(
            "council envelope for manifest {manifest_label} must be a JSON object"
        ))
    })?;
    const ALLOWED_ENVELOPE_FIELDS: &[&str] = &[
        "chunk_digest_sha3_256",
        "manifest",
        "manifest_blake3",
        "profile",
        "profile_aliases",
        "signatures",
    ];
    if let Some(unknown) = obj
        .keys()
        .find(|field| !ALLOWED_ENVELOPE_FIELDS.contains(&field.as_str()))
    {
        return Err(invalid_parameter(format!(
            "council envelope for manifest {manifest_label} contains unknown field `{unknown}`"
        )));
    }

    if let Some(manifest_name) = obj.get("manifest") {
        let manifest_name = manifest_name.as_str().ok_or_else(|| {
            invalid_parameter(format!(
                "council envelope `manifest` field for manifest {manifest_label} must be a string"
            ))
        })?;
        if manifest_name.is_empty()
            || manifest_name.len() > MAX_COUNCIL_ENVELOPE_MANIFEST_NAME_BYTES
            || manifest_name != manifest_name.trim()
            || manifest_name.chars().any(char::is_control)
        {
            return Err(invalid_parameter(format!(
                "council envelope `manifest` field for manifest {manifest_label} is not canonical"
            )));
        }
    }

    let expected_manifest_hex = hex::encode(record.digest.as_bytes());
    let manifest_hex_field = obj
        .get("manifest_blake3")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            invalid_parameter(format!(
                "council envelope for manifest {manifest_label} missing `manifest_blake3` field"
            ))
        })?;
    if manifest_hex_field != expected_manifest_hex {
        return Err(invalid_parameter(format!(
            "council envelope manifest digest `{manifest_hex_field}` does not match registered digest `{expected_manifest_hex}` for manifest {manifest_label}"
        )));
    }
    let expected_chunk_hex = hex::encode(record.chunk_digest_sha3_256);
    let chunk_hex_field = obj
        .get("chunk_digest_sha3_256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
                invalid_parameter(format!(
                    "council envelope for manifest {manifest_label} missing `chunk_digest_sha3_256` field"
                ))
        })?;
    if chunk_hex_field != expected_chunk_hex {
        return Err(invalid_parameter(format!(
            "council envelope chunk digest `{chunk_hex_field}` does not match registered digest `{expected_chunk_hex}` for manifest {manifest_label}"
        )));
    }

    let canonical_profile = record.chunker.to_handle();
    let profile_field = obj.get("profile").and_then(Value::as_str).ok_or_else(|| {
        invalid_parameter(format!(
            "council envelope for manifest {manifest_label} missing `profile` field"
        ))
    })?;
    if profile_field != canonical_profile {
        return Err(invalid_parameter(format!(
            "council envelope profile `{profile_field}` does not match registered profile `{canonical_profile}` for manifest {manifest_label}"
        )));
    }

    if let Some(aliases_value) = obj.get("profile_aliases") {
        let aliases = aliases_value.as_array().ok_or_else(|| {
            invalid_parameter(format!(
                "council envelope aliases for manifest {manifest_label} must be an array"
            ))
        })?;
        if aliases.is_empty() || aliases.len() > MAX_COUNCIL_ENVELOPE_PROFILE_ALIASES {
            return Err(invalid_parameter(format!(
                "council envelope aliases for manifest {manifest_label} must contain 1..={MAX_COUNCIL_ENVELOPE_PROFILE_ALIASES} entries"
            )));
        }
        let descriptor = sorafs_manifest::chunker_registry::lookup(ProfileId(
            record.chunker.profile_id,
        ))
        .ok_or_else(|| {
            invalid_parameter(format!(
                "council envelope for manifest {manifest_label} references unknown chunker profile"
            ))
        })?;
        let mut seen = BTreeSet::new();
        for alias in aliases {
            let alias = alias.as_str().ok_or_else(|| {
                invalid_parameter(format!(
                    "council envelope aliases for manifest {manifest_label} must be strings"
                ))
            })?;
            if !descriptor.aliases.contains(&alias) {
                return Err(invalid_parameter(format!(
                    "council envelope for manifest {manifest_label} contains unknown profile alias `{alias}`"
                )));
            }
            if !seen.insert(alias) {
                return Err(invalid_parameter(format!(
                    "council envelope for manifest {manifest_label} repeats profile alias `{alias}`"
                )));
            }
        }
        if !seen.contains(canonical_profile.as_str()) {
            return Err(invalid_parameter(format!(
                "council envelope aliases for manifest {manifest_label} must include canonical profile `{canonical_profile}`"
            )));
        }
    }

    let signatures = obj
        .get("signatures")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid_parameter(format!(
                "council envelope for manifest {manifest_label} missing `signatures` array"
            ))
        })?;
    if signatures.is_empty() {
        return Err(invalid_parameter(format!(
            "council envelope for manifest {manifest_label} must include at least one signature entry"
        )));
    }
    if signatures.len() > MAX_COUNCIL_ENVELOPE_SIGNATURES {
        return Err(invalid_parameter(format!(
            "council envelope for manifest {manifest_label} contains {} signatures; maximum is {MAX_COUNCIL_ENVELOPE_SIGNATURES}",
            signatures.len(),
        )));
    }

    let mut previous_signer: Option<[u8; 32]> = None;
    for entry in signatures {
        let entry = entry.as_object().ok_or_else(|| {
            invalid_parameter(format!(
                "council envelope signature entries for manifest {manifest_label} must be objects"
            ))
        })?;
        const ALLOWED_SIGNATURE_FIELDS: &[&str] =
            &["algorithm", "signature", "signer", "signer_multihash"];
        if let Some(unknown) = entry
            .keys()
            .find(|field| !ALLOWED_SIGNATURE_FIELDS.contains(&field.as_str()))
        {
            return Err(invalid_parameter(format!(
                "council envelope signature for manifest {manifest_label} contains unknown field `{unknown}`"
            )));
        }

        let algorithm = entry
            .get("algorithm")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                invalid_parameter(format!(
                    "council envelope signature entry for manifest {manifest_label} missing `algorithm` field"
                ))
            })?;
        if algorithm != "ed25519" {
            return Err(invalid_parameter(format!(
                "unsupported council signature algorithm `{algorithm}` for manifest {manifest_label}"
            )));
        }

        let signer_hex = entry
            .get("signer")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                invalid_parameter(format!(
                    "council envelope signature entry for manifest {manifest_label} missing `signer` field"
                ))
            })?;
        if !is_canonical_lower_hex(signer_hex, 32) {
            return Err(invalid_parameter(format!(
                "council signer `{signer_hex}` for manifest {manifest_label} must be exactly 32 bytes of lowercase hex"
            )));
        }
        let signer_bytes = hex::decode(signer_hex).map_err(|err| {
            invalid_parameter(format!(
                "invalid signer hex `{signer_hex}` in council envelope for manifest {manifest_label}: {err}"
            ))
        })?;
        let signer_array: [u8; 32] = signer_bytes.as_slice().try_into().map_err(|_| {
            invalid_parameter(format!(
                "council signer `{signer_hex}` for manifest {manifest_label} must be 32 bytes"
            ))
        })?;
        if previous_signer.is_some_and(|previous| previous >= signer_array) {
            return Err(invalid_parameter(format!(
                "council envelope signatures for manifest {manifest_label} must have distinct signer keys in canonical order"
            )));
        }
        previous_signer = Some(signer_array);
        let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &signer_bytes).map_err(|err| {
            invalid_parameter(format!(
                "failed to parse council signer `{signer_hex}` for manifest {manifest_label}: {err}"
            ))
        })?;

        let signature_hex = entry
            .get("signature")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                invalid_parameter(format!(
                    "council envelope signature entry for manifest {manifest_label} missing `signature` field"
                ))
            })?;
        if !is_canonical_lower_hex(signature_hex, 64) {
            return Err(invalid_parameter(format!(
                "council signature for signer `{signer_hex}` in manifest {manifest_label} must be exactly 64 bytes of lowercase hex"
            )));
        }
        let signature_bytes = hex::decode(signature_hex).map_err(|err| {
            invalid_parameter(format!(
                "invalid signature hex for signer `{signer_hex}` in manifest {manifest_label}: {err}"
            ))
        })?;
        let signature = ed25519_parse_signature(&signature_bytes).map_err(|err| {
            invalid_parameter(format!(
                "invalid council signature material for signer `{signer_hex}` in manifest {manifest_label}: {err}"
            ))
        })?;
        signature.verify(&public_key, record.digest.as_bytes()).map_err(|err| {
            invalid_parameter(format!(
                "failed to verify council signature for signer `{signer_hex}` in manifest {manifest_label}: {err}"
            ))
        })?;

        if let Some(multihash) = entry.get("signer_multihash") {
            let multihash = multihash.as_str().ok_or_else(|| {
                invalid_parameter(format!(
                    "council signature `signer_multihash` for signer `{signer_hex}` in manifest {manifest_label} must be a string"
                ))
            })?;
            let expected_multihash = public_key.to_string();
            if multihash != expected_multihash {
                return Err(invalid_parameter(format!(
                    "council signature for signer `{signer_hex}` in manifest {manifest_label} has multihash `{multihash}` but expected `{expected_multihash}`"
                )));
            }
        }
    }

    let mut digest_bytes = [0u8; 32];
    digest_bytes.copy_from_slice(blake3::hash(envelope).as_bytes());
    Ok(digest_bytes)
}

fn is_canonical_lower_hex(value: &str, decoded_len: usize) -> bool {
    decoded_len.checked_mul(2) == Some(value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn ensure_chunker_handle(
    chunker: &iroha_data_model::sorafs::pin_registry::ChunkerProfileHandle,
) -> Result<(), InstructionExecutionError> {
    validate_chunker_handle(
        ProfileId(chunker.profile_id),
        &chunker.namespace,
        &chunker.name,
        &chunker.semver,
        chunker.multihash_code,
        None,
    )
    .map(|_| ())
    .map_err(|err| manifest_error(&err))
}

fn ensure_pin_policy(
    policy: &iroha_data_model::sorafs::pin_registry::PinPolicy,
    constraints: &iroha_config::parameters::actual::SorafsPinPolicyConstraints,
) -> Result<(), InstructionExecutionError> {
    let manifest_policy = ManifestPinPolicy {
        min_replicas: policy.min_replicas,
        storage_class: convert_storage_class(policy.storage_class),
        retention_epoch: policy.retention_epoch,
    };
    let manifest_constraints = manifest_pin_policy_constraints_from_config(constraints);
    validate_pin_policy(&manifest_policy, &manifest_constraints).map_err(|err| manifest_error(&err))
}

fn ensure_successor_chain(
    manifests: &impl StorageReadOnly<ManifestDigest, PinManifestRecord>,
    digest: &ManifestDigest,
    successor_of: &ManifestDigest,
) -> Result<(), InstructionExecutionError> {
    let new_hex = manifest_hex(digest);
    if successor_of.as_bytes() == digest.as_bytes() {
        return Err(invalid_parameter(format!(
            "manifest {new_hex} cannot declare itself as successor"
        )));
    }

    let mut cursor_bytes = *successor_of.as_bytes();
    let mut visited = BTreeSet::new();

    loop {
        if !visited.insert(cursor_bytes) {
            return Err(invalid_parameter(format!(
                "successor chain for manifest {new_hex} forms a cycle"
            )));
        }

        if cursor_bytes == *digest.as_bytes() {
            return Err(invalid_parameter(format!(
                "successor chain for manifest {new_hex} would create a cycle"
            )));
        }

        let cursor_digest = ManifestDigest::new(cursor_bytes);
        let Some(record) = manifests.get(&cursor_digest) else {
            return Err(invalid_parameter(format!(
                "successor manifest {} referenced by {new_hex} is not registered",
                manifest_hex(&cursor_digest)
            )));
        };

        match record.status {
            PinStatus::Approved(_) => {}
            PinStatus::Pending => {
                return Err(invalid_parameter(format!(
                    "successor manifest {} must be approved before registering successor {new_hex}",
                    manifest_hex(&cursor_digest)
                )));
            }
            PinStatus::Retired(epoch) => {
                return Err(invalid_parameter(format!(
                    "successor manifest {} was retired at epoch {epoch} and cannot accept successor {new_hex}",
                    manifest_hex(&cursor_digest)
                )));
            }
        }

        if let Some(parent) = &record.successor_of {
            cursor_bytes = *parent.as_bytes();
        } else {
            break;
        }
    }

    Ok(())
}

fn convert_storage_class(
    storage_class: iroha_data_model::sorafs::pin_registry::StorageClass,
) -> ManifestStorageClass {
    match storage_class {
        iroha_data_model::sorafs::pin_registry::StorageClass::Hot => ManifestStorageClass::Hot,
        iroha_data_model::sorafs::pin_registry::StorageClass::Warm => ManifestStorageClass::Warm,
        iroha_data_model::sorafs::pin_registry::StorageClass::Cold => ManifestStorageClass::Cold,
    }
}

fn order_hex(order_id: &ReplicationOrderId) -> String {
    hex::encode(order_id.as_bytes())
}

const AUTO_REPLICATION_ORDER_NAMESPACE: &[u8] = b"sorafs:auto-replication-order:v1";
const AUTO_REPLICATION_ORDER_SLICE_GIB: u64 = 1;
const AUTO_REPLICATION_ORDER_EPOCH_SLACK: u64 = 1;
const AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS: u32 = 86_400;
const AUTO_REPLICATION_ORDER_AVAILABILITY_PERCENT_MILLI: u32 = 99_500;
const AUTO_REPLICATION_ORDER_POR_SUCCESS_PERCENT_MILLI: u32 = 98_000;
const AUTO_REPLICATION_ORDER_SECS_PER_EPOCH: u64 = 3_600;

fn supports_chunker_profile(declaration: &CapacityDeclarationV1, profile: &str) -> bool {
    declaration.chunker_commitments.iter().any(|commitment| {
        commitment.profile_id == profile
            || commitment
                .profile_aliases
                .as_ref()
                .is_some_and(|aliases| aliases.iter().any(|alias| alias == profile))
    })
}

fn select_auto_replication_providers(
    state_transaction: &StateTransaction<'_, '_>,
    chunker: &iroha_data_model::sorafs::pin_registry::ChunkerProfileHandle,
    policy: &PinPolicy,
    submitted_epoch: u64,
) -> Result<Vec<ProviderId>, InstructionExecutionError> {
    let required_replicas = usize::from(policy.min_replicas);
    if required_replicas == 0 {
        return Ok(Vec::new());
    }

    let canonical_profile = chunker.to_handle();
    let default_storage_class = state_transaction
        .world
        .sorafs_pricing
        .get()
        .default_storage_class;
    let mut providers = Vec::new();
    providers.try_reserve_exact(required_replicas).map_err(|_| {
        invalid_parameter(format!(
            "failed to reserve automatic replication provider set of {required_replicas} entries"
        ))
    })?;

    for (provider_id, declaration_record) in state_transaction.world.capacity_declarations.iter() {
        if providers.len() == required_replicas {
            break;
        }

        if submitted_epoch < declaration_record.valid_from_epoch
            || submitted_epoch > declaration_record.valid_until_epoch
        {
            continue;
        }

        if state_transaction
            .world
            .provider_owners
            .get(provider_id)
            .is_none()
        {
            continue;
        }

        let Ok(storage_class) =
            storage_class_from_declaration_record(declaration_record, default_storage_class)
        else {
            continue;
        };
        if storage_class != policy.storage_class {
            continue;
        }

        let Ok(declaration) = decode_capacity_declaration_payload(&declaration_record.declaration)
        else {
            continue;
        };
        if !supports_chunker_profile(&declaration, &canonical_profile) {
            continue;
        }

        providers.push(*provider_id);
    }

    Ok(providers)
}

fn auto_replication_order_id(
    digest: &ManifestDigest,
    assignments: &[ReplicationAssignmentV1],
) -> ReplicationOrderId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(AUTO_REPLICATION_ORDER_NAMESPACE);
    hasher.update(digest.as_bytes());
    for assignment in assignments {
        hasher.update(&assignment.provider_id);
    }
    ReplicationOrderId::new(*hasher.finalize().as_bytes())
}

fn build_auto_replication_order(
    record: &PinManifestRecord,
    issued_by: &AccountId,
    issued_epoch: u64,
    assignments: &[ProviderId],
) -> Result<Option<ReplicationOrderRecord>, InstructionExecutionError> {
    if assignments.len() < usize::from(record.policy.min_replicas) {
        return Ok(None);
    }

    let assignment_count = usize::from(record.policy.min_replicas);
    let mut canonical_assignments = Vec::new();
    canonical_assignments
        .try_reserve_exact(assignment_count)
        .map_err(|_| {
            invalid_parameter(format!(
                "failed to reserve automatic replication assignment set of {assignment_count} entries"
            ))
        })?;
    canonical_assignments.extend(assignments.iter().take(assignment_count).map(|provider| {
        ReplicationAssignmentV1 {
            provider_id: *provider.as_bytes(),
            slice_gib: AUTO_REPLICATION_ORDER_SLICE_GIB,
            lane: None,
        }
    }));
    let order_id = auto_replication_order_id(&record.digest, &canonical_assignments);
    let deadline_epoch = issued_epoch
        .checked_add(AUTO_REPLICATION_ORDER_EPOCH_SLACK)
        .ok_or_else(|| invalid_parameter("automatic replication deadline epoch overflow"))?;
    let issued_at = issued_epoch
        .checked_mul(AUTO_REPLICATION_ORDER_SECS_PER_EPOCH)
        .ok_or_else(|| invalid_parameter("automatic replication issuance time overflow"))?;
    let deadline_at = issued_at
        .checked_add(u64::from(AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS))
        .ok_or_else(|| invalid_parameter("automatic replication deadline time overflow"))?;
    let order = ReplicationOrderV1 {
        version: REPLICATION_ORDER_VERSION_V1,
        order_id: *order_id.as_bytes(),
        manifest_cid: record.root_cid.as_bytes().to_vec(),
        manifest_digest: *record.digest.as_bytes(),
        chunking_profile: record.chunker.to_handle(),
        target_replicas: record.policy.min_replicas,
        assignments: canonical_assignments,
        issued_at,
        deadline_at,
        sla: ReplicationOrderSlaV1 {
            ingest_deadline_secs: AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS,
            min_availability_percent_milli: AUTO_REPLICATION_ORDER_AVAILABILITY_PERCENT_MILLI,
            min_por_success_percent_milli: AUTO_REPLICATION_ORDER_POR_SUCCESS_PERCENT_MILLI,
        },
        metadata: Vec::new(),
    };
    order.validate().map_err(|error| {
        InstructionExecutionError::InvariantViolation(
            format!("automatic replication order failed validation: {error}").into(),
        )
    })?;

    let canonical_order = norito::to_bytes(&order).map_err(|error| {
        InstructionExecutionError::InvariantViolation(
            format!("failed to encode automatic replication order: {error}").into(),
        )
    })?;
    Ok(Some(ReplicationOrderRecord {
        order_id,
        manifest_digest: record.digest,
        manifest_root_cid: record.root_cid,
        issued_by: issued_by.clone(),
        issued_epoch,
        deadline_epoch,
        canonical_order,
        status: ReplicationOrderStatus::Pending,
    }))
}

fn collect_public_pin_fee(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    policy: &PinPolicy,
    content_length: u64,
    submitted_epoch: u64,
) -> Result<PinFeePayment, InstructionExecutionError> {
    let amount = state_transaction
        .world
        .sorafs_pricing
        .get()
        .public_pin_fee(
            policy.storage_class,
            content_length,
            policy.min_replicas,
            submitted_epoch,
            policy.retention_epoch,
        )
        .map_err(|error| pricing_computation_error("public pin fee", error))?;
    let fee_asset_id = state_transaction.gov.sorafs_pin_fee_asset_id.clone();
    let treasury_account_id = state_transaction
        .gov
        .sorafs_pin_fee_treasury_account
        .clone();
    let source_id = AssetId::new(fee_asset_id.clone(), authority.clone());

    crate::smartcontracts::isi::asset::isi::execute_user_numeric_asset_transfer(
        state_transaction,
        authority,
        source_id,
        treasury_account_id.clone(),
        amount.clone().into_numeric(),
    )?;

    Ok(PinFeePayment {
        paid_by: authority.clone(),
        fee_asset_id,
        treasury_account_id,
        amount,
    })
}

fn manifest_error(err: &ManifestValidationError) -> InstructionExecutionError {
    invalid_parameter(format!("manifest validation failed: {err}"))
}

fn validate_manifest_alias_binding(
    alias: &ManifestAliasBinding,
    expected_manifest: &ManifestDigest,
    expected_root_cid: &ManifestRootCid,
    expected_epoch_bounds: Option<(u64, u64)>,
) -> Result<Vec<u8>, InstructionExecutionError> {
    validate_alias_segment(&alias.namespace, "namespace")?;
    validate_alias_segment(&alias.name, "name")?;

    if alias.proof.is_empty() {
        return Err(invalid_parameter(
            "alias proof must not be empty; provide AliasBindingV1 Norito payload".to_string(),
        ));
    }
    if alias.proof.len() > MAX_ALIAS_PROOF_BYTES {
        return Err(invalid_parameter(format!(
            "alias proof is {} bytes; maximum is {MAX_ALIAS_PROOF_BYTES}",
            alias.proof.len(),
        )));
    }

    let bundle = decode_alias_proof(&alias.proof)
        .map_err(|err| invalid_parameter(format!("alias proof failed verification: {err}")))?;

    let expected_alias = format!("{}/{}", alias.namespace, alias.name);
    if bundle.binding.alias != expected_alias {
        return Err(invalid_parameter(format!(
            "alias proof alias `{}` does not match requested alias `{expected_alias}`",
            bundle.binding.alias
        )));
    }

    if bundle.binding.manifest_cid.as_slice() != expected_root_cid.as_bytes() {
        return Err(invalid_parameter(format!(
            "alias proof manifest CID does not match content root registered for manifest {}",
            manifest_hex(expected_manifest)
        )));
    }

    if let Some((bound_epoch, expiry_epoch)) = expected_epoch_bounds {
        if bundle.binding.bound_at != bound_epoch {
            return Err(invalid_parameter(format!(
                "alias proof bound_at {} does not match requested bound_epoch {}",
                bundle.binding.bound_at, bound_epoch
            )));
        }
        if bundle.binding.expiry_epoch != expiry_epoch {
            return Err(invalid_parameter(format!(
                "alias proof expiry_epoch {} does not match requested expiry_epoch {}",
                bundle.binding.expiry_epoch, expiry_epoch
            )));
        }
    }

    norito::to_bytes(&bundle).map_err(|err| {
        invalid_parameter(format!("failed to canonicalize alias proof bundle: {err}"))
    })
}

fn validate_alias_segment(value: &str, field: &str) -> Result<(), InstructionExecutionError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(invalid_parameter(format!(
            "alias {field} must not be empty"
        )));
    }
    if trimmed.len() > 128 {
        return Err(invalid_parameter(format!(
            "alias {field} `{trimmed}` exceeds 128 characters"
        )));
    }
    if trimmed != value {
        return Err(invalid_parameter(format!(
            "alias {field} must not include surrounding whitespace"
        )));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
    {
        return Err(invalid_parameter(format!(
            "alias {field} `{trimmed}` contains invalid characters; expected lowercase ASCII, digits, '.', '-', '_'"
        )));
    }
    Ok(())
}

fn alias_label(alias: &ManifestAliasBinding) -> String {
    format!("{}/{}", alias.namespace, alias.name)
}

fn ensure_alias_unique(
    alias: &ManifestAliasBinding,
    manifests: &impl StorageReadOnly<ManifestDigest, PinManifestRecord>,
    alias_records: &impl StorageReadOnly<ManifestAliasId, ManifestAliasRecord>,
    current_manifest: Option<&ManifestDigest>,
) -> Result<(), InstructionExecutionError> {
    let requested = alias_label(alias);
    let alias_id = ManifestAliasId::from(alias);

    for (digest, record) in manifests.iter() {
        if record.alias.as_ref().is_some_and(|existing| {
            existing.namespace == alias.namespace && existing.name == alias.name
        }) && (current_manifest != Some(digest))
        {
            return Err(invalid_parameter(format!(
                "alias `{requested}` is already bound to manifest {}",
                manifest_hex(digest)
            )));
        }
    }

    if let Some(existing) = alias_records.get(&alias_id)
        && current_manifest.is_none_or(|current| !existing.targets_manifest(current))
    {
        return Err(invalid_parameter(format!(
            "alias `{requested}` is already associated with manifest {}",
            manifest_hex(&existing.manifest)
        )));
    }

    Ok(())
}

fn bind_alias_record(
    state_transaction: &mut StateTransaction<'_, '_>,
    alias: &ManifestAliasBinding,
    manifest: &ManifestDigest,
    authority: &AccountId,
    bound_epoch: u64,
    expiry_epoch: u64,
) {
    let alias_id = ManifestAliasId::from(alias);
    drop_alias_binding_if_matches(
        &mut state_transaction.world.manifest_aliases,
        alias,
        manifest,
    );
    let record = ManifestAliasRecord::new(
        alias.clone(),
        *manifest,
        authority.clone(),
        bound_epoch,
        expiry_epoch,
    );
    state_transaction
        .world
        .manifest_aliases
        .insert(alias_id, record);
}

fn drop_alias_binding_if_matches(
    aliases: &mut StorageTransaction<'_, '_, ManifestAliasId, ManifestAliasRecord>,
    alias: &ManifestAliasBinding,
    manifest: &ManifestDigest,
) {
    let alias_id = ManifestAliasId::from(alias);
    if let Some(existing) = aliases.get(&alias_id)
        && existing.targets_manifest(manifest)
    {
        aliases.remove(alias_id);
    }
}

#[cfg(test)]
mod pin_policy_tests {
    use super::*;

    #[test]
    fn manifest_constraints_reflect_config() {
        use iroha_config::parameters::actual::SorafsPinPolicyConstraints as ConfigConstraints;
        use iroha_data_model::sorafs::pin_registry::StorageClass as DmStorageClass;

        let mut allowed = BTreeSet::new();
        allowed.insert(DmStorageClass::Hot);
        allowed.insert(DmStorageClass::Cold);

        let config = ConfigConstraints {
            min_replicas_floor: 2,
            max_replicas_ceiling: Some(4),
            max_retention_epoch: Some(10),
            allowed_storage_classes: Some(allowed.clone()),
            require_council_signatures: true,
        };

        let constraints = manifest_pin_policy_constraints_from_config(&config);
        assert_eq!(constraints.min_replicas_floor, 2);
        assert_eq!(constraints.max_replicas_ceiling, Some(4));
        assert_eq!(constraints.max_retention_epoch, Some(10));
        assert!(constraints.require_council_signatures);
        let produced = constraints
            .allowed_storage_classes
            .expect("allowed storage classes propagated");
        assert_eq!(
            produced,
            allowed
                .into_iter()
                .map(super::convert_storage_class)
                .collect::<BTreeSet<_>>()
        );
    }
}

impl Execute for iroha_data_model::isi::sorafs::RetirePinManifest {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(state_transaction, authority, "CanRetireSorafsPin")?;

        let Some(mut record) = state_transaction
            .world
            .pin_manifests
            .get(&self.digest)
            .cloned()
        else {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("manifest {} not registered", manifest_hex(&self.digest)).into(),
            ));
        };

        if self.retired_epoch < record.submitted_epoch {
            return Err(invalid_parameter(format!(
                "manifest {} retirement epoch {} predates submission epoch {}",
                manifest_hex(&self.digest),
                self.retired_epoch,
                record.submitted_epoch,
            )));
        }
        if let PinStatus::Approved(approved_epoch) = record.status
            && self.retired_epoch < approved_epoch
        {
            return Err(invalid_parameter(format!(
                "manifest {} retirement epoch {} predates approval epoch {}",
                manifest_hex(&self.digest),
                self.retired_epoch,
                approved_epoch,
            )));
        }
        if let Some(reason) = self.reason.as_deref()
            && (reason.is_empty()
                || reason.len() > MAX_RETIREMENT_REASON_BYTES
                || reason != reason.trim()
                || reason.chars().any(char::is_control))
        {
            return Err(invalid_parameter(format!(
                "manifest {} retirement reason must be canonical, non-empty, control-free UTF-8 of at most {MAX_RETIREMENT_REASON_BYTES} bytes",
                manifest_hex(&self.digest),
            )));
        }

        if matches!(record.status, PinStatus::Retired(existing) if existing == self.retired_epoch)
            && record.retirement_reason.as_deref() == self.reason.as_deref()
        {
            return Ok(());
        }

        if let PinStatus::Retired(existing_epoch) = record.status {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "manifest {} already retired at epoch {}",
                    manifest_hex(&self.digest),
                    existing_epoch
                )
                .into(),
            ));
        }

        let pending_order_count = state_transaction
            .world
            .replication_orders
            .iter()
            .filter(|(_, order)| {
                order.manifest_digest == self.digest
                    && matches!(order.status, ReplicationOrderStatus::Pending)
            })
            .count();
        let mut pending_order_ids = Vec::new();
        pending_order_ids
            .try_reserve_exact(pending_order_count)
            .map_err(|_| {
                invalid_parameter(format!(
                    "failed to reserve {pending_order_count} replication-order retirements for manifest {}",
                    manifest_hex(&self.digest),
                ))
            })?;
        pending_order_ids.extend(
            state_transaction
                .world
                .replication_orders
                .iter()
                .filter(|(_, order)| {
                    order.manifest_digest == self.digest
                        && matches!(order.status, ReplicationOrderStatus::Pending)
                })
                .map(|(order_id, _)| *order_id),
        );

        for order_id in pending_order_ids {
            let Some(mut order) = state_transaction
                .world
                .replication_orders
                .get(&order_id)
                .cloned()
            else {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "replication order {} disappeared while retiring manifest {}",
                        order_hex(&order_id),
                        manifest_hex(&self.digest),
                    )
                    .into(),
                ));
            };
            order.status = ReplicationOrderStatus::Expired(self.retired_epoch);
            state_transaction
                .world
                .replication_orders
                .insert(order_id, order);
        }

        if let Some(alias) = &record.alias {
            drop_alias_binding_if_matches(
                &mut state_transaction.world.manifest_aliases,
                alias,
                &self.digest,
            );
        }

        record.retire(self.retired_epoch, self.reason.clone());
        state_transaction
            .world
            .pin_manifests
            .insert(self.digest, record);

        Ok(())
    }
}

impl Execute for iroha_data_model::isi::sorafs::BindManifestAlias {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(state_transaction, authority, "CanBindSorafsAlias")?;

        let Self {
            digest,
            mut binding,
            bound_epoch,
            expiry_epoch,
        } = self;

        if expiry_epoch < bound_epoch {
            return Err(invalid_parameter(
                "alias expiry epoch must be greater than or equal to bound epoch",
            ));
        }

        let manifest_label = manifest_hex(&digest);

        let mut record = state_transaction
            .world
            .pin_manifests
            .get(&digest)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("manifest {manifest_label} not registered").into(),
                )
            })?;

        let canonical_proof = validate_manifest_alias_binding(
            &binding,
            &digest,
            &record.root_cid,
            Some((bound_epoch, expiry_epoch)),
        )?;
        binding.proof = canonical_proof;

        if !matches!(record.status, PinStatus::Approved(_)) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("manifest {manifest_label} must be approved before binding an alias")
                    .into(),
            ));
        }

        let approved_epoch = match record.status {
            PinStatus::Approved(epoch) => epoch,
            _ => unreachable!("checked above"),
        };
        if bound_epoch < approved_epoch {
            return Err(invalid_parameter(format!(
                "alias bound_epoch {bound_epoch} precedes manifest approval epoch {approved_epoch} \
                 for {manifest_label}"
            )));
        }
        if expiry_epoch > record.policy.retention_epoch {
            return Err(invalid_parameter(format!(
                "alias expiry epoch {expiry_epoch} exceeds manifest retention epoch \
                 {retention_epoch} for {manifest_label}",
                retention_epoch = record.policy.retention_epoch,
            )));
        }

        ensure_alias_unique(
            &binding,
            &state_transaction.world.pin_manifests,
            &state_transaction.world.manifest_aliases,
            Some(&digest),
        )?;

        if let Some(existing) = &record.alias
            && ManifestAliasId::from(existing) != ManifestAliasId::from(&binding)
        {
            drop_alias_binding_if_matches(
                &mut state_transaction.world.manifest_aliases,
                existing,
                &digest,
            );
        }

        bind_alias_record(
            state_transaction,
            &binding,
            &digest,
            authority,
            bound_epoch,
            expiry_epoch,
        );

        record.alias = Some(binding);
        state_transaction.world.pin_manifests.insert(digest, record);

        Ok(())
    }
}

impl Execute for iroha_data_model::isi::sorafs::RegisterCapacityDeclaration {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut record: CapacityDeclarationRecord = self.record;
        let provider_id = record.provider_id;
        let provider_hex = hex::encode(provider_id.as_bytes());

        let declaration =
            decode_capacity_declaration_payload(&record.declaration).map_err(|err| {
                invalid_parameter(format!(
                    "invalid capacity declaration payload for provider {provider_hex}: {err}"
                ))
            })?;

        declaration.validate().map_err(|err| {
            invalid_parameter(format!(
                "capacity declaration validation failed for provider {provider_hex}: {err}"
            ))
        })?;

        let payload_provider = ProviderId::new(declaration.provider_id);
        if payload_provider != provider_id {
            return Err(invalid_parameter(format!(
                "capacity declaration provider mismatch: record {provider_hex}, payload {}",
                hex::encode(payload_provider.as_bytes())
            )));
        }

        if declaration.committed_capacity_gib != record.committed_capacity_gib {
            return Err(invalid_parameter(format!(
                "capacity declaration committed capacity mismatch for provider {provider_hex}: \
                 summary {} GiB vs payload {} GiB",
                record.committed_capacity_gib, declaration.committed_capacity_gib
            )));
        }

        merge_declaration_metadata_into_record(
            provider_id,
            &mut record.metadata,
            &declaration.metadata,
        )?;
        enforce_provider_owner(
            &state_transaction.world,
            &state_transaction.nexus.dataspace_catalog,
            authority,
            &record.metadata,
            &provider_hex,
        )?;
        if let Some(existing_owner) = state_transaction.world.provider_owners.get(&provider_id) {
            if !same_account_subject(existing_owner, authority) {
                return Err(invalid_parameter(format!(
                    "provider {provider_hex} is already owned by {existing_owner} and cannot be rebound to {authority}"
                )));
            }
        }
        state_transaction
            .world
            .provider_owners
            .insert(provider_id, authority.clone());
        grant_repair_worker_permission(state_transaction, authority, provider_id);

        state_transaction
            .world
            .capacity_declarations
            .insert(provider_id, record.clone());

        let mut ledger = state_transaction
            .world
            .capacity_fee_ledger
            .get(&provider_id)
            .cloned()
            .unwrap_or_else(|| CapacityFeeLedgerEntry {
                provider_id,
                ..Default::default()
            });
        ledger.provider_id = provider_id;
        ledger.total_declared_gib = u128::from(declaration.committed_capacity_gib);
        ledger.last_updated_epoch = record.registered_epoch;

        state_transaction
            .world
            .capacity_fee_ledger
            .insert(provider_id, ledger);

        Ok(())
    }
}

impl Execute for iroha_data_model::isi::sorafs::RecordCapacityTelemetry {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let record: CapacityTelemetryRecord = self.record;
        let provider_id = record.provider_id;
        let policy = &state_transaction.gov.sorafs_telemetry;

        let reject = |reason: &str| {
            #[cfg(feature = "telemetry")]
            {
                state_transaction
                    .telemetry
                    .record_sorafs_capacity_telemetry_reject(
                        &hex::encode(provider_id.as_bytes()),
                        reason,
                    );
            }
            Err(invalid_parameter(format!(
                "capacity telemetry rejected: {reason}"
            )))
        };

        if policy.require_submitter {
            if let Some(overrides) = policy.per_provider_submitters.get(&provider_id) {
                if !overrides
                    .iter()
                    .any(|allowed| same_account_subject(allowed, authority))
                {
                    return reject("unauthorised_submitter_provider");
                }
            } else if !policy
                .submitters
                .iter()
                .any(|allowed| same_account_subject(allowed, authority))
            {
                return reject("unauthorised_submitter");
            }
        }

        if policy.require_nonce && record.nonce == 0 {
            return reject("missing_nonce");
        }

        let declaration_record = state_transaction
            .world
            .capacity_declarations
            .get(&provider_id)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("capacity telemetry received for unknown provider {provider_id:?}")
                        .into(),
                )
            })?;
        ensure_provider_owner_matches_authority(
            authority,
            declaration_record,
            &state_transaction.world,
            &state_transaction.nexus.dataspace_catalog,
        )?;
        if let Some(owner) = state_transaction.world.provider_owners.get(&provider_id)
            && !same_account_subject(owner, authority)
        {
            return reject("provider_owner_mismatch");
        }

        let mut ledger = state_transaction
            .world
            .capacity_fee_ledger
            .get(&provider_id)
            .cloned()
            .unwrap_or_else(|| CapacityFeeLedgerEntry {
                provider_id,
                ..Default::default()
            });
        ledger.provider_id = provider_id;

        if policy.reject_zero_capacity
            && (record.declared_gib == 0 || record.effective_gib == 0 || record.utilised_gib == 0)
        {
            return reject("zero_capacity_window");
        }

        let committed_capacity = declaration_record.committed_capacity_gib;
        if record.declared_gib > committed_capacity
            || record.effective_gib > committed_capacity
            || record.utilised_gib > committed_capacity
        {
            return reject("capacity_exceeds_commitment");
        }

        if record.effective_gib > record.declared_gib || record.utilised_gib > record.declared_gib {
            return reject("capacity_exceeds_declaration");
        }

        if record.window_end_epoch <= record.window_start_epoch {
            return reject("invalid_window_bounds");
        }
        let Some(window_secs) = record
            .window_end_epoch
            .checked_sub(record.window_start_epoch)
        else {
            return reject("invalid_window_bounds");
        };

        if record.nonce != 0 && ledger.last_nonce == record.nonce {
            if record.window_start_epoch == ledger.last_window_start_epoch
                && record.window_end_epoch == ledger.last_window_end_epoch
            {
                // Idempotent submission: already applied.
                return Ok(());
            }
            return reject("replayed_nonce");
        }

        if ledger.last_window_end_epoch > 0 {
            if record.window_start_epoch < ledger.last_window_end_epoch {
                return reject("overlapping_window");
            }
            if record.window_end_epoch <= ledger.last_window_end_epoch {
                return reject("stale_window");
            }
            let Some(gap) = record
                .window_start_epoch
                .checked_sub(ledger.last_window_end_epoch)
            else {
                return reject("overlapping_window");
            };
            if gap > policy.max_window_gap.as_secs() {
                return reject("window_gap_exceeded");
            }
        }

        let pricing_schedule = state_transaction.world.sorafs_pricing.get();
        let storage_class = storage_class_from_declaration_record(
            declaration_record,
            pricing_schedule.default_storage_class,
        )?;
        let storage_fee = pricing_schedule
            .storage_charge(storage_class, record.utilised_gib, window_secs)
            .map_err(|error| pricing_computation_error("storage fee", error))?;
        let uptime_bps = u128::from(record.uptime_bps.min(10_000));
        let por_bps = u128::from(record.por_success_bps.min(10_000));
        let health_multiplier = uptime_bps.checked_mul(por_bps).ok_or_else(|| {
            pricing_computation_error(
                "health multiplier",
                PricingComputationError::ArithmeticOverflow("health multiplier"),
            )
        })?;
        let storage_fee =
            round_xor_quantity_ratio(&storage_fee, health_multiplier, 10_000_u128 * 10_000_u128)
                .map_err(|error| quantity_arithmetic_error("health-adjusted storage fee", error))?;
        let egress_fee = pricing_schedule
            .egress_charge_bytes(storage_class, record.egress_bytes)
            .map_err(|error| pricing_computation_error("egress fee", error))?;
        let expected_storage = pricing_schedule
            .expected_settlement_storage_charge(storage_class, record.utilised_gib)
            .map_err(|error| pricing_computation_error("expected settlement", error))?;
        let expected_settlement = expected_storage
            .checked_add(&egress_fee)
            .map_err(|error| quantity_arithmetic_error("expected settlement", error))?;

        ledger
            .accrue(&CapacityAccrual {
                declared_delta_gib: u128::from(record.declared_gib),
                utilised_delta_gib: u128::from(record.utilised_gib),
                storage_fee_delta: storage_fee.clone(),
                egress_fee_delta: egress_fee.clone(),
                expected_settlement: expected_settlement.clone(),
                window_start_epoch: record.window_start_epoch,
                window_end_epoch: record.window_end_epoch,
                nonce: record.nonce,
            })
            .map_err(|error| {
                InstructionExecutionError::InvariantViolation(
                    format!("SoraFS capacity fee ledger accrual failed: {error}").into(),
                )
            })?;
        let mut proof_alert: Option<SorafsProofHealthAlert> = None;
        if let Some(mut credit_record) = state_transaction
            .world
            .provider_credit_ledger
            .get(&provider_id)
            .cloned()
        {
            let debit = storage_fee
                .checked_add(&egress_fee)
                .map_err(|error| quantity_arithmetic_error("credit debit", error))?;
            credit_record
                .apply_charge(&debit, record.window_end_epoch)
                .map_err(|error| {
                    InstructionExecutionError::InvariantViolation(
                        format!("SoraFS provider credit charge failed: {error}").into(),
                    )
                })?;
            credit_record.required_bond = pricing_schedule
                .required_collateral(
                    storage_class,
                    record.utilised_gib,
                    credit_record.onboarding_epoch,
                    record.window_end_epoch,
                )
                .map_err(|error| pricing_computation_error("required collateral", error))?;
            credit_record.expected_settlement = expected_settlement.clone();
            let low_balance_threshold = pricing_schedule
                .low_balance_threshold(&expected_settlement)
                .map_err(|error| pricing_computation_error("low-balance threshold", error))?;
            credit_record.track_low_balance(&low_balance_threshold, record.window_end_epoch);

            let penalty_policy = &state_transaction.gov.sorafs_penalty;
            // Treat failure counters as authoritative even when challenge/window counters are
            // missing, so we don't suppress proof-failure penalties and alerts.
            let pdp_fail = record.pdp_failures > penalty_policy.max_pdp_failures;
            let potr_fail = record.potr_breaches > penalty_policy.max_potr_breaches;
            let proof_failure = pdp_fail || potr_fail;
            let penalties_enabled =
                penalty_policy.penalty_bond_bps > 0 && penalty_policy.strike_threshold > 0;
            if penalties_enabled {
                let utilisation_ratio_bps = if record.declared_gib == 0 {
                    10_000_u128
                } else {
                    checked_mul_div_round_u128(
                        u128::from(record.utilised_gib),
                        10_000,
                        u128::from(record.declared_gib),
                    )
                    .map_err(|error| pricing_computation_error("utilisation ratio", error))?
                };
                let utilisation_floor =
                    u128::from(penalty_policy.utilisation_floor_bps.min(10_000));
                let uptime_floor = u32::from(penalty_policy.uptime_floor_bps.min(10_000));
                let por_floor = u32::from(penalty_policy.por_success_floor_bps.min(10_000));

                let utilisation_fail = utilisation_ratio_bps < utilisation_floor;
                let uptime_fail = record.uptime_bps < uptime_floor;
                let por_success_below_floor = record.por_success_bps < por_floor;

                if proof_failure {
                    let previous_strikes = credit_record.under_delivery_strikes;
                    credit_record.under_delivery_strikes = penalty_policy.strike_threshold;
                    proof_alert = Some(SorafsProofHealthAlert {
                        provider_id,
                        window_start_epoch: record.window_start_epoch,
                        window_end_epoch: record.window_end_epoch,
                        prior_strikes: previous_strikes,
                        strike_threshold: penalty_policy.strike_threshold,
                        pdp_challenges: record.pdp_challenges,
                        pdp_failures: record.pdp_failures,
                        potr_windows: record.potr_windows,
                        potr_breaches: record.potr_breaches,
                        triggered_by_pdp: pdp_fail,
                        triggered_by_potr: potr_fail,
                        max_pdp_failures: penalty_policy.max_pdp_failures,
                        max_potr_breaches: penalty_policy.max_potr_breaches,
                        penalty_bond_bps: penalty_policy.penalty_bond_bps,
                        penalty_applied: Quantity::zero(),
                        cooldown_active: false,
                    });
                    iroha_logger::warn!(
                        provider_id = %hex::encode(provider_id.as_bytes()),
                        pdp_challenges = record.pdp_challenges,
                        pdp_failures = record.pdp_failures,
                        potr_windows = record.potr_windows,
                        potr_breaches = record.potr_breaches,
                        "capacity telemetry reported PDP/PoTR failures; forcing immediate strike"
                    );
                } else if utilisation_fail || uptime_fail || por_success_below_floor {
                    credit_record.add_strike().map_err(|error| {
                        InstructionExecutionError::InvariantViolation(
                            format!("SoraFS provider strike update failed: {error}").into(),
                        )
                    })?;
                } else {
                    credit_record.reset_strikes();
                }

                let strikes_met =
                    credit_record.under_delivery_strikes >= penalty_policy.strike_threshold;
                let cooldown_secs = penalty_policy
                    .cooldown_window_secs(pricing_schedule.credit.settlement_window_secs);
                let within_cooldown = if let Some(epoch) = credit_record.last_penalty_epoch {
                    let Some(elapsed) = record.window_end_epoch.checked_sub(epoch) else {
                        return reject("penalty_epoch_in_future");
                    };
                    cooldown_secs > 0 && elapsed < cooldown_secs
                } else {
                    false
                };
                if let Some(alert) = proof_alert.as_mut() {
                    alert.cooldown_active = within_cooldown;
                }

                if strikes_met && !within_cooldown {
                    let calculated_penalty = round_xor_quantity_ratio(
                        &credit_record.bonded,
                        u128::from(penalty_policy.penalty_bond_bps),
                        10_000,
                    )
                    .map_err(|error| quantity_arithmetic_error("provider penalty", error))?;
                    let penalty_amount = if calculated_penalty > credit_record.bonded {
                        credit_record.bonded.clone()
                    } else {
                        calculated_penalty
                    };
                    if let Some(alert) = proof_alert.as_mut() {
                        alert.penalty_applied = penalty_amount.clone();
                    }

                    if !penalty_amount.is_zero() {
                        credit_record
                            .apply_penalty(&penalty_amount, record.window_end_epoch)
                            .map_err(|error| {
                                InstructionExecutionError::InvariantViolation(
                                    format!("SoraFS provider penalty failed: {error}").into(),
                                )
                            })?;
                        ledger
                            .apply_penalty(&penalty_amount, record.window_end_epoch)
                            .map_err(|error| {
                                InstructionExecutionError::InvariantViolation(
                                    format!("SoraFS capacity penalty ledger failed: {error}")
                                        .into(),
                                )
                            })?;
                    } else {
                        credit_record.reset_strikes();
                    }
                }
            }

            state_transaction
                .world
                .provider_credit_ledger
                .insert(provider_id, credit_record);

            if proof_failure {
                register_proof_failure_dispute(state_transaction, &record, pdp_fail, potr_fail)?;
            }
        }

        state_transaction
            .world
            .capacity_fee_ledger
            .insert(provider_id, ledger);

        if let Some(alert) = proof_alert {
            #[cfg(feature = "telemetry")]
            {
                state_transaction
                    .telemetry
                    .record_sorafs_proof_health_alert(&alert);
            }
            state_transaction
                .world
                .emit_events(Some(SorafsGatewayEvent::ProofHealth(alert)));
        }

        Ok(())
    }
}

fn auto_dispute_complainant_id() -> [u8; 32] {
    *blake3_hash(b"sorafs:auto-proof-dispute:v1").as_bytes()
}

fn register_proof_failure_dispute(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: &CapacityTelemetryRecord,
    pdp_fail: bool,
    potr_fail: bool,
) -> Result<(), InstructionExecutionError> {
    let telemetry_bytes = norito::to_bytes(record).map_err(|err| {
        InstructionExecutionError::InvariantViolation(
            format!("failed to encode telemetry evidence: {err}").into(),
        )
    })?;
    let telemetry_digest = *blake3_hash(&telemetry_bytes).as_bytes();
    let evidence_uri = format!(
        "norito://sorafs/capacity_telemetry/{}/{:019}-{:019}",
        hex::encode(record.provider_id.as_bytes()),
        record.window_start_epoch,
        record.window_end_epoch,
    );
    let reason = match (pdp_fail, potr_fail) {
        (true, true) => "PDP and PoTR probes failed in this window",
        (true, false) => "PDP probes failed in this window",
        (false, true) => "PoTR probes failed in this window",
        (false, false) => "proof failure recorded",
    };
    let description = format!(
        "{reason}; pdp_challenges={}, pdp_failures={}, potr_windows={}, potr_breaches={}",
        record.pdp_challenges, record.pdp_failures, record.potr_windows, record.potr_breaches
    );
    let dispute = sorafs_manifest::capacity::CapacityDisputeV1 {
        version: CAPACITY_DISPUTE_VERSION_V1,
        provider_id: *record.provider_id.as_bytes(),
        complainant_id: auto_dispute_complainant_id(),
        replication_order_id: None,
        kind: CapacityDisputeKind::ProofFailure,
        evidence: CapacityDisputeEvidenceV1 {
            evidence_digest: telemetry_digest,
            media_type: Some("application/norito".into()),
            uri: Some(evidence_uri),
            size_bytes: Some(u64::try_from(telemetry_bytes.len()).unwrap_or(u64::MAX)),
        },
        submitted_epoch: record.window_end_epoch,
        description,
        requested_remedy: Some("Automatic strike/slash triggered by governance policy".to_owned()),
    };
    let dispute_payload = norito::to_bytes(&dispute).map_err(|err| {
        InstructionExecutionError::InvariantViolation(
            format!("failed to encode dispute payload: {err}").into(),
        )
    })?;
    let dispute_id = CapacityDisputeId::new(*blake3_hash(&dispute_payload).as_bytes());
    if state_transaction
        .world
        .capacity_disputes
        .get(&dispute_id)
        .is_some()
    {
        return Ok(());
    }
    let evidence = CapacityDisputeEvidence {
        digest: dispute.evidence.evidence_digest,
        media_type: dispute.evidence.media_type.clone(),
        uri: dispute.evidence.uri.clone(),
        size_bytes: dispute.evidence.size_bytes,
    };
    let record = CapacityDisputeRecord::new_pending(
        dispute_id,
        record.provider_id,
        dispute.complainant_id,
        dispute.replication_order_id,
        dispute.kind as u8,
        dispute.submitted_epoch,
        dispute.description.clone(),
        dispute.requested_remedy.clone(),
        evidence,
        dispute_payload,
    );
    state_transaction
        .world
        .capacity_disputes
        .insert(dispute_id, record);
    Ok(())
}

impl Execute for iroha_data_model::isi::sorafs::RegisterCapacityDispute {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(state_transaction, authority, "CanFileSorafsCapacityDispute")?;

        let mut record: CapacityDisputeRecord = self.record;
        ensure_provider_owner_registered(state_transaction, &record.provider_id, authority)?;

        let dispute = decode_capacity_dispute_payload(&record.dispute_payload)
            .map_err(|err| invalid_parameter(format!("invalid capacity dispute payload: {err}")))?;
        dispute.validate().map_err(|err| {
            invalid_parameter(format!("capacity dispute validation failed: {err}"))
        })?;

        let dispute_id = CapacityDisputeId::new(*blake3_hash(&record.dispute_payload).as_bytes());
        if record.dispute_id != dispute_id {
            return Err(invalid_parameter(
                "capacity dispute identifier mismatch with payload digest",
            ));
        }

        let provider_id = ProviderId::new(dispute.provider_id);
        if record.provider_id != provider_id {
            return Err(invalid_parameter(
                "capacity dispute provider identifier mismatch",
            ));
        }
        if record.complainant_id != dispute.complainant_id {
            return Err(invalid_parameter(
                "capacity dispute complainant identifier mismatch",
            ));
        }
        if record.replication_order_id != dispute.replication_order_id {
            return Err(invalid_parameter(
                "capacity dispute replication order identifier mismatch",
            ));
        }

        if matches!(dispute.kind, CapacityDisputeKind::ReplicationShortfall)
            && dispute.replication_order_id.is_none()
        {
            return Err(invalid_parameter(
                "capacity dispute replication order identifier is required for replication shortfall",
            ));
        }

        if let Some(replication_order_id) = dispute.replication_order_id {
            let order_id = ReplicationOrderId::new(replication_order_id);
            if state_transaction
                .world
                .replication_orders
                .get(&order_id)
                .is_none()
            {
                return Err(invalid_parameter(
                    "capacity dispute references unknown replication order",
                ));
            }
        }

        record.kind = dispute.kind as u8;
        record.replication_order_id = dispute.replication_order_id;
        record.submitted_epoch = dispute.submitted_epoch;
        record.description.clone_from(&dispute.description);
        record
            .requested_remedy
            .clone_from(&dispute.requested_remedy);
        record.evidence = CapacityDisputeEvidence {
            digest: dispute.evidence.evidence_digest,
            media_type: dispute.evidence.media_type.clone(),
            uri: dispute.evidence.uri.clone(),
            size_bytes: dispute.evidence.size_bytes,
        };
        record.status = CapacityDisputeStatus::Pending;

        if state_transaction
            .world
            .capacity_declarations
            .get(&provider_id)
            .is_none()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("capacity dispute received for unknown provider {provider_id:?}").into(),
            ));
        }

        if state_transaction
            .world
            .capacity_disputes
            .get(&dispute_id)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "duplicate capacity dispute identifier".into(),
            ));
        }

        state_transaction
            .world
            .capacity_disputes
            .insert(dispute_id, record);

        Ok(())
    }
}

impl Execute for iroha_data_model::isi::sorafs::IssueReplicationOrder {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(
            state_transaction,
            authority,
            "CanIssueSorafsReplicationOrder",
        )?;

        let order_label = order_hex(&self.order_id);
        if self.deadline_epoch <= self.issued_epoch {
            return Err(invalid_parameter(format!(
                "replication order {order_label} deadline {} must be greater than issued_epoch {}",
                self.deadline_epoch, self.issued_epoch
            )));
        }

        if self.order_payload.is_empty()
            || self.order_payload.len() > MAX_REPLICATION_ORDER_PAYLOAD_BYTES
        {
            return Err(invalid_parameter(format!(
                "replication order {order_label} payload has {} bytes; expected 1..={MAX_REPLICATION_ORDER_PAYLOAD_BYTES}",
                self.order_payload.len(),
            )));
        }

        if state_transaction
            .world
            .replication_orders
            .get(&self.order_id)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("replication order {order_label} already exists").into(),
            ));
        }

        let order_payload = decode_from_bytes_with_limits::<ReplicationOrderV1>(
            &self.order_payload,
            REPLICATION_ORDER_DECODE_LIMITS,
        )
        .map_err(|err| {
            invalid_parameter(format!(
                "invalid replication order payload for {order_label}: {err}"
            ))
        })?;
        order_payload.validate().map_err(|err| {
            invalid_parameter(format!(
                "replication order validation failed for {order_label}: {err}"
            ))
        })?;
        let canonical_order = norito::to_bytes(&order_payload).map_err(|err| {
            invalid_parameter(format!(
                "failed to canonicalize replication order {order_label}: {err}"
            ))
        })?;
        if canonical_order != self.order_payload {
            return Err(invalid_parameter(format!(
                "replication order {order_label} payload must use canonical first-release Norito"
            )));
        }

        if order_payload.order_id != *self.order_id.as_bytes() {
            return Err(invalid_parameter(format!(
                "replication order {order_label} payload uses mismatched identifier"
            )));
        }

        let manifest_digest = ManifestDigest::new(order_payload.manifest_digest);
        let manifest_label = manifest_hex(&manifest_digest);
        let manifest_record = state_transaction
            .world
            .pin_manifests
            .get(&manifest_digest)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "manifest {manifest_label} not registered for replication order {order_label}"
                    )
                    .into(),
                )
            })?;

        if !manifest_record.status.is_active() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "manifest {manifest_label} must be approved before issuing replication orders"
                )
                .into(),
            ));
        }

        if order_payload.manifest_cid.as_slice() != manifest_record.root_cid.as_bytes() {
            return Err(invalid_parameter(format!(
                "replication order {order_label} manifest CID does not match content root registered for manifest {manifest_label}"
            )));
        }

        let canonical_profile = manifest_record.chunker.to_handle();
        if order_payload.chunking_profile != canonical_profile {
            return Err(invalid_parameter(format!(
                "replication order {order_label} chunking profile `{}` does not match manifest profile `{canonical_profile}`",
                order_payload.chunking_profile
            )));
        }

        if order_payload.target_replicas < manifest_record.policy.min_replicas {
            return Err(invalid_parameter(format!(
                "replication order {order_label} target replicas {} below manifest minimum {}",
                order_payload.target_replicas, manifest_record.policy.min_replicas
            )));
        }

        for assignment in &order_payload.assignments {
            let provider = ProviderId::new(assignment.provider_id);
            if state_transaction
                .world
                .provider_owners
                .get(&provider)
                .is_none()
            {
                return Err(invalid_parameter(format!(
                    "replication order {order_label} references provider {} with no registered owner",
                    hex::encode(provider.as_bytes())
                )));
            }
        }

        let record = ReplicationOrderRecord {
            order_id: self.order_id,
            manifest_digest,
            manifest_root_cid: manifest_record.root_cid,
            issued_by: authority.clone(),
            issued_epoch: self.issued_epoch,
            deadline_epoch: self.deadline_epoch,
            canonical_order: self.order_payload,
            status: ReplicationOrderStatus::Pending,
        };

        state_transaction
            .world
            .replication_orders
            .insert(self.order_id, record);

        Ok(())
    }
}

fn validate_stored_replication_order(
    record: &ReplicationOrderRecord,
    order_label: &str,
) -> Result<ReplicationOrderV1, InstructionExecutionError> {
    validate_manifest_root_cid(
        record.manifest_root_cid.as_bytes(),
        sorafs_manifest::chunker_registry::MANIFEST_DAG_CODEC,
        BLAKE3_256_MULTIHASH_CODE,
    )
    .map_err(|error| {
        InstructionExecutionError::InvariantViolation(
            format!("replication order {order_label} stores an invalid manifest root CID: {error}")
                .into(),
        )
    })?;
    let canonical_payload: ReplicationOrderV1 =
        decode_from_bytes_with_limits(&record.canonical_order, REPLICATION_ORDER_DECODE_LIMITS)
            .map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                format!(
                    "replication order {order_label} canonical payload could not be decoded: {err}"
                )
                .into(),
            )
            })?;
    canonical_payload.validate().map_err(|err| {
        InstructionExecutionError::InvariantViolation(
            format!("replication order {order_label} stored payload failed validation: {err}")
                .into(),
        )
    })?;
    let canonical_bytes = norito::to_bytes(&canonical_payload).map_err(|err| {
        InstructionExecutionError::InvariantViolation(
            format!(
                "replication order {order_label} stored payload could not be canonicalized: {err}"
            )
            .into(),
        )
    })?;
    if canonical_bytes != record.canonical_order
        || canonical_payload.order_id != *record.order_id.as_bytes()
        || canonical_payload.manifest_digest != *record.manifest_digest.as_bytes()
        || canonical_payload.manifest_cid.as_slice() != record.manifest_root_cid.as_bytes()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "replication order {order_label} stored payload is not canonical or bound to its record"
            )
            .into(),
        ));
    }
    Ok(canonical_payload)
}

impl Execute for iroha_data_model::isi::sorafs::CompleteReplicationOrder {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(
            state_transaction,
            authority,
            "CanCompleteSorafsReplicationOrder",
        )?;

        let order_label = order_hex(&self.order_id);
        let mut record = state_transaction
            .world
            .replication_orders
            .get(&self.order_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("replication order {order_label} not found").into(),
                )
            })?;
        validate_stored_replication_order(&record, &order_label)?;

        match record.status {
            ReplicationOrderStatus::Pending => {}
            ReplicationOrderStatus::Completed(epoch) if epoch == self.completion_epoch => {
                return Ok(());
            }
            ReplicationOrderStatus::Completed(epoch) => {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("replication order {order_label} already completed at epoch {epoch}")
                        .into(),
                ));
            }
            ReplicationOrderStatus::Expired(epoch) => {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("replication order {order_label} expired at epoch {epoch}").into(),
                ));
            }
        }

        if self.completion_epoch < record.issued_epoch {
            return Err(invalid_parameter(format!(
                "completion_epoch {} must be >= issued_epoch {} for replication order {order_label}",
                self.completion_epoch, record.issued_epoch
            )));
        }
        if self.completion_epoch > record.deadline_epoch {
            return Err(invalid_parameter(format!(
                "completion_epoch {} exceeds deadline_epoch {} for replication order {order_label}",
                self.completion_epoch, record.deadline_epoch
            )));
        }

        let manifest = state_transaction
            .world
            .pin_manifests
            .get(&record.manifest_digest)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("replication order {order_label} references a missing pin manifest")
                        .into(),
                )
            })?;
        if manifest.root_cid != record.manifest_root_cid {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "replication order {order_label} content root no longer matches its registered manifest"
                )
                .into(),
            ));
        }
        if !manifest.status.is_active() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "replication order {order_label} cannot complete after its manifest is inactive"
                )
                .into(),
            ));
        }

        record.complete(self.completion_epoch);
        state_transaction
            .world
            .replication_orders
            .insert(self.order_id, record);

        Ok(())
    }
}

impl Execute for iroha_data_model::isi::sorafs::ExpireReplicationOrder {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(
            state_transaction,
            authority,
            "CanIssueSorafsReplicationOrder",
        )?;

        let order_label = order_hex(&self.order_id);
        let mut record = state_transaction
            .world
            .replication_orders
            .get(&self.order_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("replication order {order_label} not found").into(),
                )
            })?;
        validate_stored_replication_order(&record, &order_label)?;

        let manifest = state_transaction
            .world
            .pin_manifests
            .get(&record.manifest_digest)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("replication order {order_label} references a missing pin manifest")
                        .into(),
                )
            })?;
        if manifest.root_cid != record.manifest_root_cid {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "replication order {order_label} content root no longer matches its registered manifest"
                )
                .into(),
            ));
        }

        match record.status {
            ReplicationOrderStatus::Pending => {}
            ReplicationOrderStatus::Expired(epoch) if epoch == self.expiration_epoch => {
                return Ok(());
            }
            ReplicationOrderStatus::Expired(epoch) => {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("replication order {order_label} already expired at epoch {epoch}")
                        .into(),
                ));
            }
            ReplicationOrderStatus::Completed(epoch) => {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("replication order {order_label} completed at epoch {epoch}").into(),
                ));
            }
        }

        if self.expiration_epoch <= record.deadline_epoch {
            return Err(invalid_parameter(format!(
                "expiration_epoch {} must be greater than deadline_epoch {} for replication order {order_label}",
                self.expiration_epoch, record.deadline_epoch
            )));
        }

        record.expire(self.expiration_epoch);
        state_transaction
            .world
            .replication_orders
            .insert(self.order_id, record);
        Ok(())
    }
}

impl Execute for iroha_data_model::isi::sorafs::SetPricingSchedule {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(state_transaction, authority, "CanSetSorafsPricing")?;

        let schedule: PricingScheduleRecord = self.schedule;
        schedule.validate().map_err(|err| {
            invalid_parameter(format!("pricing schedule validation failed: {err}"))
        })?;
        *state_transaction.world.sorafs_pricing.get_mut() = schedule;
        Ok(())
    }
}

impl Execute for iroha_data_model::isi::sorafs::UpsertProviderCredit {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(
            state_transaction,
            authority,
            "CanUpsertSorafsProviderCredit",
        )?;

        let record: ProviderCreditRecord = self.record;
        ensure_provider_owner_registered(state_transaction, &record.provider_id, authority)?;
        if record.provider_id == ProviderId::default() {
            return Err(invalid_parameter(
                "provider credit record must reference a non-zero provider identifier",
            ));
        }
        if state_transaction
            .world
            .capacity_declarations
            .get(&record.provider_id)
            .is_none()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "provider credit entry references unknown provider {:?}",
                    record.provider_id
                )
                .into(),
            ));
        }

        state_transaction
            .world
            .provider_credit_ledger
            .insert(record.provider_id, record);
        Ok(())
    }
}

#[cfg(test)]
mod sorafs_tests {
    use core::str::FromStr;
    use std::{collections::BTreeSet, convert::TryInto};

    use blake3::hash as blake3_hash;
    use hex;
    use iroha_crypto::{Algorithm, Hash, KeyPair, PrivateKey, Signature};
    use iroha_data_model::{
        IntoKeyValue, Registrable,
        isi::{
            error::{InstructionExecutionError, InvalidParameterError},
            sorafs::{
                ApprovePinManifest, BindManifestAlias, CompleteReplicationOrder,
                ExpireReplicationOrder, IssueReplicationOrder, RecordCapacityTelemetry,
                RegisterCapacityDeclaration, RegisterCapacityDispute, RegisterPinManifest,
                RegisterProviderOwner, RetirePinManifest, SetPricingSchedule,
                UnregisterProviderOwner, UpsertProviderCredit,
            },
        },
        metadata::Metadata,
        name::Name,
        permission::{Permission as AccountPermission, Permissions},
        prelude::{Account, AccountId, Asset, AssetDefinition, AssetId, Domain},
        query::error::FindError,
        sorafs::{
            capacity::{
                CapacityDisputeEvidence, CapacityDisputeId, CapacityDisputeRecord,
                CapacityDisputeStatus, CapacityFeeLedgerEntry, ProviderId,
            },
            deal::BYTES_PER_GIB,
            pin_registry::{
                ChunkerProfileHandle, ManifestAliasBinding, ManifestAliasId, ManifestDigest,
                PinManifestRecord, PinPolicy, PinStatus, ReplicationOrderId,
                ReplicationOrderStatus, StorageClass,
            },
            pricing::{
                CollateralPolicy, CreditPolicy, PricingScheduleRecord, ProviderCreditRecord,
                SECONDS_PER_BILLING_MONTH, TierRate,
            },
        },
    };
    use iroha_executor_data_model::permission::sorafs::CanOperateSorafsRepair;
    use iroha_primitives::{bigint::BigInt, json::Json};
    use nonzero_ext::nonzero;
    use norito::{json, to_bytes};
    use sorafs_manifest::{
        DagCodecId, GovernanceProofs, ManifestBuilder, ManifestV1,
        capacity::{
            CAPACITY_DECLARATION_VERSION_V1, CAPACITY_DISPUTE_VERSION_V1, CapacityDeclarationV1,
            CapacityDisputeKind, CapacityDisputeV1, CapacityMetadataEntry, ChunkerCommitmentV1,
            REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1,
            ReplicationOrderV1,
        },
        pin_registry::{
            AliasBindingV1, AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest,
        },
        provider_advert::{CapabilityType, StakePointer},
    };

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn canonical_profile(handle: &ChunkerProfileHandle) -> String {
        format!("{}.{}@{}", handle.namespace, handle.name, handle.semver)
    }

    fn build_envelope(record: &PinManifestRecord, keypair: &KeyPair) -> (Vec<u8>, String) {
        let manifest_hex = hex::encode(record.digest.as_bytes());
        let chunk_hex = hex::encode(record.chunk_digest_sha3_256);
        let profile = canonical_profile(&record.chunker);
        let signature = Signature::try_new(keypair.private_key(), record.digest.as_bytes())
            .expect("council envelope fixture should sign");
        let signature_hex = hex::encode(signature.payload());
        let public_key = keypair.public_key();
        let (_, signer_bytes) = public_key
            .try_to_bytes()
            .expect("fixture public key must be valid");
        let signer_hex = hex::encode(signer_bytes);
        let signer_multihash = public_key.to_string();
        let mut signature_entry = json::Map::new();
        signature_entry.insert("algorithm".into(), json::Value::from("ed25519"));
        signature_entry.insert("signer".into(), json::Value::from(signer_hex));
        signature_entry.insert("signature".into(), json::Value::from(signature_hex.clone()));
        signature_entry.insert(
            "signer_multihash".into(),
            json::Value::from(signer_multihash.clone()),
        );
        let signatures = json::Value::Array(vec![json::Value::Object(signature_entry)]);

        let mut envelope_map = json::Map::new();
        envelope_map.insert("chunk_digest_sha3_256".into(), json::Value::from(chunk_hex));
        envelope_map.insert("manifest_blake3".into(), json::Value::from(manifest_hex));
        envelope_map.insert("profile".into(), json::Value::from(profile));
        envelope_map.insert("signatures".into(), signatures);
        let envelope = json::Value::Object(envelope_map);
        let mut serialized = json::to_vec_pretty(&envelope).expect("serialize council envelope");
        serialized.push(b'\n');
        (serialized, signature_hex)
    }

    fn council_envelope_error(record: &PinManifestRecord, envelope: &[u8]) -> String {
        match verify_council_envelope(record, envelope)
            .expect_err("adversarial council envelope must fail")
        {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected council envelope error: {other:?}"),
        }
    }

    const SMALL_ORDER_ED25519_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    const NONCANONICAL_ED25519_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("SoraFS fixture key generation should succeed")
    }

    fn checked_ed25519_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .expect("SoraFS Ed25519 fixture key generation should succeed")
    }

    fn checked_account_id() -> AccountId {
        AccountId::new(checked_keypair().public_key().clone())
    }

    fn xor_quantity_nanos(value: u128) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(value, XOR_QUANTITY_SCALE))
            .expect("u128 nano-XOR test fixture fits Quantity")
    }

    fn exact_xor_nanos(value: &Quantity) -> u128 {
        let nanos = value
            .try_mul_decimal(&Numeric::from(1_000_000_000_u64))
            .expect("SoraFS XOR quantity scales exactly to nanounits");
        assert_eq!(nanos.scale(), 0, "SoraFS XOR quantity is nano-exact");
        nanos
            .as_numeric()
            .try_mantissa_u128()
            .expect("bounded non-negative XOR nanounits fit u128")
    }

    fn max_positive_quantity() -> Quantity {
        let mut bytes = [0xff; 64];
        bytes[63] = 0x7f;
        Quantity::from_canonical_numeric(Numeric::new(
            BigInt::from_twos_bytes(&bytes).expect("512-bit positive mantissa fits"),
            0,
        ))
        .expect("maximum positive Numeric is a Quantity")
    }

    fn provider_credit_nanos(
        provider_id: ProviderId,
        available_credit_nanos: u128,
        bonded_nanos: u128,
    ) -> ProviderCreditRecord {
        ProviderCreditRecord::new(
            provider_id,
            xor_quantity_nanos(available_credit_nanos),
            xor_quantity_nanos(bonded_nanos),
            Quantity::zero(),
            Quantity::zero(),
            0,
            0,
            Metadata::default(),
        )
    }

    #[test]
    fn xor_quantity_ratio_is_exact_checked_and_rounds_at_nano_boundaries() {
        let zero = Quantity::zero();
        let one_nano = xor_quantity_nanos(1);
        let two_nanos = xor_quantity_nanos(2);

        assert_eq!(
            round_xor_quantity_ratio(&xor_quantity_nanos(8), 5_000, 10_000)
                .expect("bounded exact ratio"),
            xor_quantity_nanos(4)
        );
        assert_eq!(
            round_xor_quantity_ratio(&one_nano, 1, 2).expect("half nano rounds away from zero"),
            one_nano
        );
        assert_eq!(
            round_xor_quantity_ratio(&two_nanos, 1, 3)
                .expect("sub-nano result rounds to the XOR scale"),
            xor_quantity_nanos(1)
        );
        assert_eq!(
            round_xor_quantity_ratio(&xor_quantity_nanos(1), 1, 3)
                .expect("sub-half-nano result rounds to zero"),
            zero
        );
        assert_eq!(
            round_xor_quantity_ratio(&Quantity::zero(), u128::MAX, 1)
                .expect("zero remains zero for any bounded multiplier"),
            Quantity::zero()
        );

        let fractional: Quantity = "1.234567891"
            .parse()
            .expect("canonical fractional Quantity");
        assert_eq!(
            round_xor_quantity_ratio(&fractional, 1, 2).expect("bounded fractional ratio"),
            "0.617283946"
                .parse::<Quantity>()
                .expect("canonical rounded Quantity")
        );
        assert_eq!(
            round_xor_quantity_ratio(&xor_quantity_nanos(1), 1, 0),
            Err(NumericOperationError::DivisionByZero)
        );
        assert_eq!(
            xor_quantity_nanos(1).checked_sub(&xor_quantity_nanos(2)),
            Err(NumericOperationError::QuantityUnderflow)
        );
        assert_eq!(
            round_xor_quantity_ratio(&max_positive_quantity(), 2, 1),
            Err(NumericOperationError::MantissaOverflow)
        );
    }

    #[test]
    fn integer_ratio_helper_rejects_invalid_or_overflowing_economic_inputs() {
        assert_eq!(checked_mul_div_round_u128(5, 1, 2), Ok(3));
        assert_eq!(
            checked_mul_div_round_u128(1, 1, 0),
            Err(PricingComputationError::DivisionByZero(
                "u128 multiply/divide"
            ))
        );
        assert_eq!(
            checked_mul_div_round_u128(u128::MAX, 2, 1),
            Err(PricingComputationError::ArithmeticOverflow(
                "u128 multiply/divide"
            ))
        );
    }

    #[test]
    fn checked_keypair_helpers_preserve_requested_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
        assert_eq!(checked_ed25519_keypair().algorithm(), Algorithm::Ed25519);
    }

    pub(super) fn block_header() -> iroha_data_model::block::BlockHeader {
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)
    }

    fn seed_test_call_hash(stx: &mut crate::state::StateTransaction<'_, '_>) {
        stx.tx_call_hash = Some(Hash::prehashed([0x51; Hash::LENGTH]));
    }

    pub(super) fn make_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let handle = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(World::new(), kura, handle);
        seed_public_pin_fee_accounts(&mut state);
        seed_sorafs_permissions(&mut state, &alice());
        state.gov.sorafs_telemetry.require_submitter = true;
        state.gov.sorafs_telemetry.submitters = vec![alice()];
        state
    }

    fn seed_public_pin_fee_accounts(state: &mut State) {
        let fee_asset_id = state.gov.sorafs_pin_fee_asset_id.clone();
        if let Some(domain_id) = fee_asset_id.try_domain().cloned() {
            state
                .world
                .domains
                .insert(domain_id.clone(), Domain::new(domain_id).build(&alice()));
        }
        let (account_id, account_value) = Account::new(alice()).build(&alice()).into_key_value();
        state.world.accounts.insert(account_id, account_value);
        let (account_id, account_value) = Account::new(bob()).build(&alice()).into_key_value();
        state.world.accounts.insert(account_id, account_value);
        let treasury = state.gov.sorafs_pin_fee_treasury_account.clone();
        let (account_id, account_value) = Account::new(treasury).build(&alice()).into_key_value();
        state.world.accounts.insert(account_id, account_value);

        let definition = AssetDefinition::numeric(fee_asset_id.clone())
            .with_name(
                fee_asset_id
                    .try_name()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "xor".to_owned()),
            )
            .build(&alice());
        if let Some(domain_id) = fee_asset_id.try_domain().cloned() {
            state
                .world
                .domain_asset_definitions
                .insert(domain_id, BTreeSet::from([fee_asset_id.clone()]));
        }
        let owner = definition.owned_by().clone();
        state
            .world
            .asset_definitions_by_owner
            .insert(owner, BTreeSet::from([fee_asset_id.clone()]));
        state
            .world
            .asset_definitions
            .insert(fee_asset_id.clone(), definition);
        seed_pin_fee_balance(state, &alice(), 10_000_000_000_000);
        seed_pin_fee_balance(state, &bob(), 10_000_000_000_000);
        let alice_asset = AssetId::new(fee_asset_id.clone(), alice());
        let bob_asset = AssetId::new(fee_asset_id.clone(), bob());
        state.world.asset_definition_holders.insert(
            fee_asset_id.clone(),
            BTreeSet::from([alice_asset.account().clone(), bob_asset.account().clone()]),
        );
        state.world.asset_definition_assets.insert(
            fee_asset_id.clone(),
            BTreeSet::from([alice_asset, bob_asset]),
        );
        state
            .world
            .asset_definition_nonzero_holders
            .insert(fee_asset_id, BTreeSet::from([alice(), bob()]));
    }

    fn seed_pin_fee_balance(state: &mut State, account: &AccountId, amount: u128) {
        let fee_asset_id = state.gov.sorafs_pin_fee_asset_id.clone();
        let asset_id = AssetId::new(fee_asset_id, account.clone());
        let (asset_id, asset_value) = Asset::new(asset_id, Quantity::from(amount)).into_key_value();
        state.world.assets.insert(asset_id, asset_value);
    }

    fn pin_fee_balance(
        stx: &crate::state::StateTransaction<'_, '_>,
        account: &AccountId,
    ) -> Numeric {
        let asset_id = AssetId::new(stx.gov.sorafs_pin_fee_asset_id.clone(), account.clone());
        stx.world
            .assets
            .get(&asset_id)
            .map(|value| value.as_numeric().clone())
            .unwrap_or_else(Numeric::zero)
    }

    fn assert_pin_fee_balances_unchanged(
        stx: &crate::state::StateTransaction<'_, '_>,
        account: &AccountId,
        account_balance_before: Numeric,
        treasury: &AccountId,
        treasury_balance_before: Numeric,
    ) {
        assert_eq!(
            pin_fee_balance(stx, account),
            account_balance_before,
            "rejected pin registration must not charge the submitter"
        );
        assert_eq!(
            pin_fee_balance(stx, treasury),
            treasury_balance_before,
            "rejected pin registration must not credit treasury"
        );
    }

    fn seed_provider_owners(
        stx: &mut crate::state::StateTransaction<'_, '_>,
        providers: &[ProviderId],
        owner: &AccountId,
    ) {
        for provider in providers {
            stx.world.provider_owners.insert(*provider, owner.clone());
        }
    }

    fn seed_sorafs_permissions(state: &mut State, authority: &AccountId) {
        let mut perms = Permissions::default();
        for name in [
            "CanRegisterSorafsPin",
            "CanApproveSorafsPin",
            "CanRetireSorafsPin",
            "CanBindSorafsAlias",
            "CanDeclareSorafsCapacity",
            "CanSubmitSorafsTelemetry",
            "CanFileSorafsCapacityDispute",
            "CanIssueSorafsReplicationOrder",
            "CanCompleteSorafsReplicationOrder",
            "CanSetSorafsPricing",
            "CanUpsertSorafsProviderCredit",
            "CanRegisterSorafsProviderOwner",
            "CanUnregisterSorafsProviderOwner",
        ] {
            perms.insert(AccountPermission::new(name.to_string(), Json::new(())));
        }
        state
            .world
            .account_permissions
            .insert(authority.clone(), perms);
    }

    fn ensure_registered_account(
        stx: &mut crate::state::StateTransaction<'_, '_>,
        account_id: &AccountId,
        domain_id: &DomainId,
    ) {
        if stx.world.domains.get(domain_id).is_none() {
            Register::domain(iroha_data_model::domain::Domain::new(domain_id.clone()))
                .execute(&alice(), stx)
                .expect("register domain for account");
        }
        if stx.world.accounts.get(account_id).is_none() {
            Register::account(iroha_data_model::account::Account::new(account_id.clone()))
                .execute(&alice(), stx)
                .expect("register account");
        }
    }

    fn remove_permission(stx: &mut crate::state::StateTransaction<'_, '_>, name: &str) {
        if let Some(perms) = stx.world.account_permissions.get_mut(&alice()) {
            perms.retain(|perm| perm.name() != name);
        }
    }

    fn grant_permission(stx: &mut crate::state::StateTransaction<'_, '_>, name: &str) {
        stx.world
            .account_permissions
            .get_mut(&alice())
            .expect("Alice permission set")
            .insert(AccountPermission::new(name.to_owned(), Json::new(())));
    }

    fn default_chunker() -> ChunkerProfileHandle {
        ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".into(),
            name: "sf1".into(),
            semver: "1.0.0".into(),
            multihash_code: 0x1f,
        }
    }

    pub(super) fn default_digest() -> ManifestDigest {
        manifest_digest_for_seed(0xAA)
    }

    fn manifest_fixture_with_chunk_digest(seed: u8, chunk_digest_sha3_256: [u8; 32]) -> ManifestV1 {
        let commitment = seed.max(1);
        ManifestBuilder::new()
            .root_cid(sorafs_manifest::canonical_manifest_root_cid(
                [commitment; 32],
            ))
            .dag_codec(DagCodecId(
                sorafs_manifest::chunker_registry::MANIFEST_DAG_CODEC,
            ))
            .chunking_from_registry(sorafs_manifest::chunker_registry::default_descriptor().id)
            .chunk_digest_sha3_256(chunk_digest_sha3_256)
            .content_length(default_content_length())
            .car_digest([commitment.wrapping_add(1).max(1); 32])
            .car_size(
                default_content_length()
                    .checked_add(4096)
                    .expect("fixture CAR size"),
            )
            .pin_policy(sorafs_manifest::PinPolicy {
                min_replicas: default_policy().min_replicas,
                storage_class: sorafs_manifest::StorageClass::Hot,
                retention_epoch: default_policy().retention_epoch,
            })
            .governance(GovernanceProofs::default())
            .build()
            .expect("fixture manifest")
    }

    fn chunk_digest_for_seed(seed: u8) -> [u8; 32] {
        [seed.wrapping_add(0x23).max(1); 32]
    }

    fn manifest_fixture(seed: u8) -> ManifestV1 {
        manifest_fixture_with_chunk_digest(seed, chunk_digest_for_seed(seed))
    }

    fn manifest_payload_for_seed(seed: u8) -> Vec<u8> {
        manifest_fixture(seed)
            .encode()
            .expect("encode fixture manifest")
    }

    fn manifest_digest_for_seed(seed: u8) -> ManifestDigest {
        ManifestDigest::from_manifest(&manifest_fixture(seed)).expect("digest fixture manifest")
    }

    fn fixture_seed_for_digest(digest: ManifestDigest) -> u8 {
        (1..=u8::MAX)
            .find(|seed| manifest_digest_for_seed(*seed) == digest)
            .expect("manifest digest must belong to a test fixture seed")
    }

    pub(super) fn root_cid_for_manifest(digest: ManifestDigest) -> ManifestRootCid {
        let manifest = manifest_fixture(fixture_seed_for_digest(digest));
        ManifestRootCid::try_from_slice(&manifest.root_cid).expect("canonical root CID")
    }

    pub(super) fn default_root_cid() -> ManifestRootCid {
        root_cid_for_manifest(default_digest())
    }

    fn default_manifest_payload() -> Vec<u8> {
        manifest_payload_for_seed(0xAA)
    }

    pub(super) fn default_chunk_digest() -> [u8; 32] {
        chunk_digest_for_seed(0xAA)
    }

    pub(super) fn default_content_length() -> u64 {
        BYTES_PER_GIB
            .try_into()
            .expect("default GiB byte count fits u64")
    }

    pub(super) fn default_policy() -> PinPolicy {
        PinPolicy {
            min_replicas: 3,
            storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Hot,
            retention_epoch: 42,
        }
    }

    pub(super) fn second_digest() -> ManifestDigest {
        manifest_digest_for_seed(0xBB)
    }

    fn third_digest() -> ManifestDigest {
        manifest_digest_for_seed(0xCC)
    }

    pub(super) fn alias_binding_for(
        digest: ManifestDigest,
        namespace: &str,
        name: &str,
        bound_at: u64,
        expiry_epoch: u64,
    ) -> ManifestAliasBinding {
        let binding_payload = AliasBindingV1 {
            alias: format!("{namespace}/{name}"),
            manifest_cid: root_cid_for_manifest(digest).as_bytes().to_vec(),
            bound_at,
            expiry_epoch,
        };
        let mut bundle = AliasProofBundleV1 {
            binding: binding_payload,
            registry_root: [0u8; 32],
            registry_height: 1,
            generated_at_unix: bound_at,
            expires_at_unix: bound_at.checked_add(600).expect("alias proof expiry"),
            merkle_path: Vec::new(),
            council_signatures: Vec::new(),
        };
        let root = alias_merkle_root(&bundle.binding, &bundle.merkle_path)
            .expect("compute alias proof root");
        bundle.registry_root = root;
        let digest_bytes = alias_proof_signature_digest(&bundle);
        let private = PrivateKey::from_bytes(Algorithm::Ed25519, &[0x22; 32]).expect("seeded key");
        let keypair = KeyPair::from_private_key(private).expect("derive keypair");
        let signature = Signature::try_new(keypair.private_key(), digest_bytes.as_ref())
            .expect("alias proof fixture should sign");
        let (_, signer_bytes) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be valid");
        let signer: [u8; 32] = signer_bytes
            .try_into()
            .expect("ed25519 public key must be 32 bytes");
        bundle
            .council_signatures
            .push(sorafs_manifest::CouncilSignature {
                signer,
                signature: signature.payload().to_vec(),
            });
        let proof = to_bytes(&bundle).expect("encode alias proof bundle");
        ManifestAliasBinding {
            name: name.to_owned(),
            namespace: namespace.to_owned(),
            proof,
        }
    }

    fn sample_alias_binding() -> ManifestAliasBinding {
        alias_binding_for(default_digest(), "sora", "docs", 8, 16)
    }

    #[test]
    fn register_pin_manifest_allows_public_submission() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        if let Some(perms) = stx.world.account_permissions.get_mut(&alice()) {
            perms.clear();
        }
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        let expected_amount = stx
            .world
            .sorafs_pricing
            .get()
            .public_pin_fee(
                default_policy().storage_class,
                default_content_length(),
                default_policy().min_replicas,
                register.submitted_epoch,
                default_policy().retention_epoch,
            )
            .expect("default public pin fee");

        register
            .execute(&alice(), &mut stx)
            .expect("public register must succeed");

        let record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored");
        assert_eq!(record.submitted_by, alice());
        assert_eq!(record.status, PinStatus::Approved(5));
        assert_eq!(record.content_length, default_content_length());
        assert_eq!(record.digest, default_digest());
        assert_eq!(record.root_cid, default_root_cid());
        assert_eq!(record.chunker, default_chunker());
        assert_eq!(record.policy, default_policy());
        let payment = record
            .pin_fee_payment
            .as_ref()
            .expect("fee payment recorded");
        assert_eq!(payment.paid_by, alice());
        assert_eq!(payment.fee_asset_id, stx.gov.sorafs_pin_fee_asset_id);
        assert_eq!(
            payment.treasury_account_id,
            stx.gov.sorafs_pin_fee_treasury_account
        );
        assert_eq!(payment.amount, expected_amount);
        assert_eq!(
            pin_fee_balance(&stx, &alice()),
            alice_balance_before
                .checked_sub(&expected_amount)
                .expect("alice has enough fee balance")
        );
        assert_eq!(
            pin_fee_balance(&stx, &treasury_account),
            treasury_balance_before
                .checked_add(&expected_amount)
                .expect("treasury balance remains representable")
        );
    }

    #[test]
    fn public_pin_cannot_reserve_alias_without_alias_permission() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        remove_permission(&mut stx, "CanBindSorafsAlias");
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);
        let alias = default_alias_binding();

        let error = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: Some(alias.clone()),
            successor_of: None,
        }
        .execute(&alice(), &mut stx)
        .expect_err("permissionless public pins must not reserve governed aliases");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("CanBindSorafsAlias")
        ));
        assert!(stx.world.pin_manifests.get(&default_digest()).is_none());
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&alias))
                .is_none()
        );
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_pin_manifest_rejects_unfunded_public_submission_without_side_effects() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        if let Some(perms) = stx.world.account_permissions.get_mut(&alice()) {
            perms.clear();
        }

        let alice_fee_asset = AssetId::new(stx.gov.sorafs_pin_fee_asset_id.clone(), alice());
        stx.world.assets.remove(alice_fee_asset);
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect_err("unfunded public pin registration must fail");

        assert!(
            stx.world.pin_manifests.get(&default_digest()).is_none(),
            "failed paid registration must not leave a manifest record"
        );
        assert_eq!(pin_fee_balance(&stx, &alice()), Numeric::zero());
        assert_eq!(
            pin_fee_balance(&stx, &treasury_account),
            treasury_balance_before,
            "failed paid registration must not credit treasury"
        );
    }

    #[test]
    fn register_pin_manifest_rejects_insufficient_public_fee_without_side_effects() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        if let Some(perms) = stx.world.account_permissions.get_mut(&alice()) {
            perms.clear();
        }

        let alice_fee_asset = AssetId::new(stx.gov.sorafs_pin_fee_asset_id.clone(), alice());
        let low_balance = Numeric::new(1_u32, 9);
        let (asset_id, asset_value) = Asset::new(
            alice_fee_asset,
            Quantity::try_from_numeric(low_balance.clone()).expect("non-negative low balance"),
        )
        .into_key_value();
        stx.world.assets.insert(asset_id, asset_value);
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect_err("underfunded public pin registration must fail");

        assert!(
            stx.world.pin_manifests.get(&default_digest()).is_none(),
            "failed paid registration must not leave a manifest record"
        );
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            low_balance,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn approve_pin_manifest_requires_permission() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        remove_permission(&mut stx, "CanApproveSorafsPin");

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 1,
            council_envelope: None,
            council_envelope_digest: None,
        };

        let error = approve
            .execute(&alice(), &mut stx)
            .expect_err("permissionless approve must fail");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("CanApproveSorafsPin")
        ));
    }

    #[test]
    fn retire_pin_manifest_requires_permission() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        remove_permission(&mut stx, "CanRetireSorafsPin");

        let retire = RetirePinManifest {
            digest: default_digest(),
            retired_epoch: 5,
            reason: None,
        };

        let error = retire
            .execute(&alice(), &mut stx)
            .expect_err("permissionless retire must fail");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("CanRetireSorafsPin")
        ));
    }

    #[test]
    fn bind_manifest_alias_requires_permission() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        remove_permission(&mut stx, "CanBindSorafsAlias");

        let bind = BindManifestAlias {
            digest: default_digest(),
            binding: sample_alias_binding(),
            bound_epoch: 8,
            expiry_epoch: 12,
        };

        let error = bind
            .execute(&alice(), &mut stx)
            .expect_err("permissionless bind must fail");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("CanBindSorafsAlias")
        ));
    }

    pub(super) fn register_and_approve_manifest(
        stx: &mut crate::state::StateTransaction<'_, '_>,
        digest: ManifestDigest,
        chunk_digest: [u8; 32],
    ) {
        let submitted_epoch = 5;
        let seed = fixture_seed_for_digest(digest);
        let manifest = manifest_fixture_with_chunk_digest(seed, chunk_digest);
        assert_eq!(
            ManifestDigest::from_manifest(&manifest).expect("digest registration fixture"),
            digest,
            "registration helper chunk digest must match the fixture digest"
        );
        let register = RegisterPinManifest {
            manifest_payload: manifest.encode().expect("encode registration fixture"),
            submitted_epoch,
            alias: None,
            successor_of: None,
        };
        register.execute(&alice(), stx).expect("register manifest");

        let stored_record = stx
            .world
            .pin_manifests
            .get(&digest)
            .expect("manifest stored")
            .clone();
        let council_key = checked_ed25519_keypair();
        let (envelope, _) = build_envelope(&stored_record, &council_key);

        let approve = ApprovePinManifest {
            digest,
            approved_epoch: submitted_epoch,
            council_envelope: Some(envelope),
            council_envelope_digest: None,
        };
        approve.execute(&alice(), stx).expect("approve manifest");
    }

    fn insert_manifest_with_status(
        stx: &mut crate::state::StateTransaction<'_, '_>,
        digest: ManifestDigest,
        chunk_digest: [u8; 32],
        successor_of: Option<ManifestDigest>,
        status: PinStatus,
    ) {
        let policy = default_policy();
        let content_length = default_content_length();
        let mut record = PinManifestRecord::new(
            digest,
            root_cid_for_manifest(digest),
            default_chunker(),
            chunk_digest,
            policy,
            alice(),
            5,
            None,
            successor_of,
            Metadata::default(),
        )
        .with_content_length(content_length);
        match status {
            PinStatus::Pending => {}
            PinStatus::Approved(epoch) => {
                let amount = stx
                    .world
                    .sorafs_pricing
                    .get()
                    .public_pin_fee(
                        policy.storage_class,
                        content_length,
                        policy.min_replicas,
                        5,
                        policy.retention_epoch,
                    )
                    .expect("fixture public pin fee");
                record.record_pin_fee_payment(PinFeePayment {
                    paid_by: alice(),
                    fee_asset_id: stx.gov.sorafs_pin_fee_asset_id.clone(),
                    treasury_account_id: stx.gov.sorafs_pin_fee_treasury_account.clone(),
                    amount,
                });
                record.approve(epoch, None);
            }
            PinStatus::Retired(epoch) => record.retire(epoch, None),
        }
        stx.world.pin_manifests.insert(digest, record);
    }

    fn insert_pending_manifest(
        stx: &mut crate::state::StateTransaction<'_, '_>,
        digest: ManifestDigest,
        chunk_digest: [u8; 32],
    ) {
        insert_manifest_with_status(stx, digest, chunk_digest, None, PinStatus::Pending);
    }

    fn insert_pending_manifest_with_alias(
        stx: &mut crate::state::StateTransaction<'_, '_>,
        digest: ManifestDigest,
        chunk_digest: [u8; 32],
        alias: ManifestAliasBinding,
    ) -> PinManifestRecord {
        let record = PinManifestRecord::new(
            digest,
            root_cid_for_manifest(digest),
            default_chunker(),
            chunk_digest,
            default_policy(),
            alice(),
            5,
            Some(alias),
            None,
            Metadata::default(),
        )
        .with_content_length(default_content_length());
        stx.world.pin_manifests.insert(digest, record.clone());
        record
    }

    fn default_alias_binding() -> ManifestAliasBinding {
        alias_binding_for(
            default_digest(),
            "sora",
            "docs",
            5,
            default_policy().retention_epoch,
        )
    }

    fn assert_alias_registration_rejected_without_fee(
        alias: ManifestAliasBinding,
        expected_message: &str,
    ) {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);
        let instruction = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: Some(alias),
            successor_of: None,
        };

        let err = instruction
            .execute(&alice(), &mut stx)
            .expect_err("registration must reject adversarial alias");

        match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => assert!(
                message.contains(expected_message),
                "expected error containing `{expected_message}`, got: {message}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(
            stx.world.pin_manifests.get(&default_digest()).is_none(),
            "rejected alias registration must not store a pin manifest"
        );
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    pub(super) fn replication_order_struct(
        order_id: ReplicationOrderId,
        manifest: ManifestDigest,
        providers: &[ProviderId],
        target_replicas: u16,
    ) -> ReplicationOrderV1 {
        let assignments = providers
            .iter()
            .map(|provider| ReplicationAssignmentV1 {
                provider_id: *provider.as_bytes(),
                slice_gib: 512,
                lane: None,
            })
            .collect();
        ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: *order_id.as_bytes(),
            manifest_cid: root_cid_for_manifest(manifest).as_bytes().to_vec(),
            manifest_digest: *manifest.as_bytes(),
            chunking_profile: canonical_profile(&default_chunker()),
            target_replicas,
            assignments,
            issued_at: 1_700_000_000,
            deadline_at: 1_700_086_400,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 86_400,
                min_availability_percent_milli: 99_500,
                min_por_success_percent_milli: 98_000,
            },
            metadata: Vec::new(),
        }
    }

    pub(super) fn encode_replication_order(order: &ReplicationOrderV1) -> Vec<u8> {
        to_bytes(order).expect("serialize replication order")
    }

    fn sample_capacity_declaration() -> CapacityDeclarationV1 {
        CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x11; 32],
            stake: StakePointer {
                pool_id: [0x22; 32],
                stake_amount: "1".parse().expect("canonical XOR stake"),
            },
            committed_capacity_gib: 1_024,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".to_string(),
                profile_aliases: None,
                committed_gib: 1_024,
                capability_refs: vec![CapabilityType::ToriiGateway],
            }],
            lane_commitments: Vec::new(),
            pricing: None,
            valid_from: 1_700_000_000,
            valid_until: 1_700_086_400,
            metadata: vec![CapacityMetadataEntry {
                key: PROVIDER_OWNER_METADATA_KEY.to_owned(),
                value: account_literal(&alice()),
            }],
        }
    }

    fn sample_capacity_record() -> (ProviderId, CapacityDeclarationRecord) {
        let declaration = sample_capacity_declaration();
        let canonical_bytes = norito::to_bytes(&declaration).expect("serialize declaration");
        let provider = ProviderId::new(declaration.provider_id);
        let record = CapacityDeclarationRecord::new(
            provider,
            canonical_bytes,
            declaration.committed_capacity_gib,
            9,
            10,
            20,
            iroha_data_model::metadata::Metadata::default(),
        );
        (provider, record)
    }

    fn capacity_record_with_owner(owner: &AccountId) -> (ProviderId, CapacityDeclarationRecord) {
        let mut declaration = sample_capacity_declaration();
        declaration.metadata = vec![CapacityMetadataEntry {
            key: PROVIDER_OWNER_METADATA_KEY.to_owned(),
            value: account_literal(owner),
        }];
        let canonical_bytes = norito::to_bytes(&declaration).expect("serialize declaration");
        let provider = ProviderId::new(declaration.provider_id);
        let record = CapacityDeclarationRecord::new(
            provider,
            canonical_bytes,
            declaration.committed_capacity_gib,
            9,
            10,
            20,
            iroha_data_model::metadata::Metadata::default(),
        );
        (provider, record)
    }

    #[derive(Clone, Copy, Default)]
    struct ProofWindowCounters {
        pdp_challenges: u32,
        pdp_failures: u32,
        potr_windows: u32,
        potr_breaches: u32,
    }

    #[allow(clippy::too_many_arguments)]
    fn record_capacity_window(
        stx: &mut StateTransaction<'_, '_>,
        provider: ProviderId,
        start_epoch: u64,
        end_epoch: u64,
        declared_gib: u64,
        effective_gib: u64,
        utilised_gib: u64,
        uptime_bps: u32,
        por_success_bps: u32,
        egress_bytes: u64,
    ) {
        record_capacity_window_with_proofs(
            stx,
            provider,
            start_epoch,
            end_epoch,
            declared_gib,
            effective_gib,
            utilised_gib,
            uptime_bps,
            por_success_bps,
            egress_bytes,
            ProofWindowCounters::default(),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn record_capacity_window_with_proofs(
        stx: &mut StateTransaction<'_, '_>,
        provider: ProviderId,
        start_epoch: u64,
        end_epoch: u64,
        declared_gib: u64,
        effective_gib: u64,
        utilised_gib: u64,
        uptime_bps: u32,
        por_success_bps: u32,
        egress_bytes: u64,
        proof: ProofWindowCounters,
    ) {
        let telemetry = CapacityTelemetryRecord::new(
            provider,
            start_epoch,
            end_epoch,
            declared_gib,
            effective_gib,
            utilised_gib,
            1,
            1,
            uptime_bps,
            por_success_bps,
            egress_bytes,
            proof.pdp_challenges,
            proof.pdp_failures,
            proof.potr_windows,
            proof.potr_breaches,
        )
        .with_nonce(end_epoch);
        RecordCapacityTelemetry { record: telemetry }
            .execute(&alice(), stx)
            .expect("record telemetry");
    }

    fn sample_capacity_dispute(provider: ProviderId) -> (CapacityDisputeRecord, CapacityDisputeId) {
        let dispute = CapacityDisputeV1 {
            version: CAPACITY_DISPUTE_VERSION_V1,
            provider_id: provider.as_bytes().to_owned(),
            complainant_id: [0x44; 32],
            replication_order_id: None,
            kind: CapacityDisputeKind::UptimeBreach,
            evidence: sorafs_manifest::capacity::CapacityDisputeEvidenceV1 {
                evidence_digest: [0xAA; 32],
                media_type: Some("application/zip".into()),
                uri: Some("https://evidence.example/dispute.zip".into()),
                size_bytes: Some(1_024),
            },
            submitted_epoch: 1_700_000_128,
            description: "provider uptime dipped below SLA".into(),
            requested_remedy: Some("slash stake".into()),
        };
        let payload = norito::to_bytes(&dispute).expect("encode dispute payload");
        let dispute_id = CapacityDisputeId::new(*blake3_hash(&payload).as_bytes());
        let evidence = CapacityDisputeEvidence {
            digest: dispute.evidence.evidence_digest,
            media_type: dispute.evidence.media_type.clone(),
            uri: dispute.evidence.uri.clone(),
            size_bytes: dispute.evidence.size_bytes,
        };
        let record = CapacityDisputeRecord::new_pending(
            dispute_id,
            provider,
            dispute.complainant_id,
            dispute.replication_order_id,
            dispute.kind as u8,
            dispute.submitted_epoch,
            dispute.description.clone(),
            dispute.requested_remedy.clone(),
            evidence,
            payload,
        );
        (record, dispute_id)
    }

    pub(super) fn alice() -> AccountId {
        AccountId::new(
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                .parse()
                .expect("public key"),
        )
    }

    fn account_literal(account: &AccountId) -> String {
        account.to_string()
    }

    pub(super) fn bob() -> AccountId {
        AccountId::new(
            "ed01208D5C8358EA5B64A79653A76F516E436EB93E3EC7117B0C9DD861B029BEA0FC8B"
                .parse()
                .expect("public key"),
        )
    }

    #[test]
    fn register_capacity_dispute_inserts_record() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration.clone(),
        }
        .execute(&alice(), &mut stx)
        .expect("register declaration");

        let (record, dispute_id) = sample_capacity_dispute(provider);
        let decoded: CapacityDisputeV1 = norito::decode_from_bytes(&record.dispute_payload)
            .expect("decode stored dispute payload");
        assert_eq!(decoded.provider_id, *record.provider_id.as_bytes());

        RegisterCapacityDispute {
            record: record.clone(),
        }
        .execute(&alice(), &mut stx)
        .expect("register capacity dispute");

        let stored = stx
            .world
            .capacity_disputes
            .get(&dispute_id)
            .expect("dispute stored");
        assert_eq!(stored.description, record.description);
        assert!(matches!(stored.status, CapacityDisputeStatus::Pending));
    }

    #[test]
    fn register_capacity_dispute_rejects_noncanonical_and_resource_bomb_payloads() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("register declaration");

        let (base_record, _) = sample_capacity_dispute(provider);
        let dispute: CapacityDisputeV1 = norito::decode_from_bytes(&base_record.dispute_payload)
            .expect("decode dispute fixture");
        let alternate = {
            let _guard = norito::core::DecodeFlagsGuard::enter(0);
            norito::to_bytes(&dispute).expect("encode alternate-layout capacity dispute")
        };
        assert_ne!(alternate, base_record.dispute_payload);
        let mut bomb = dispute;
        bomb.description = "x".repeat(2_049);
        let allocation_bomb = norito::to_bytes(&bomb).expect("encode dispute allocation bomb");
        assert!(allocation_bomb.len() <= MAX_CAPACITY_DISPUTE_PAYLOAD_BYTES);

        for payload in [
            Vec::new(),
            vec![0xFF],
            vec![0xA5; MAX_CAPACITY_DISPUTE_PAYLOAD_BYTES + 1],
            alternate,
            allocation_bomb,
        ] {
            let mut record = base_record.clone();
            record.dispute_payload = payload;
            let dispute_id = record.dispute_id;
            let err = RegisterCapacityDispute { record }
                .execute(&alice(), &mut stx)
                .expect_err("invalid capacity dispute payload must be rejected");
            assert!(matches!(
                err,
                InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(message)
                ) if message.contains("invalid capacity dispute payload")
            ));
            assert!(stx.world.capacity_disputes.get(&dispute_id).is_none());
        }
    }

    #[test]
    fn capacity_declaration_is_permissionless_for_provider_owner() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        if let Some(perms) = stx.world.account_permissions.get_mut(&alice()) {
            perms.clear();
        }
        let (_provider, declaration) = sample_capacity_record();

        let err = RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx);
        assert!(err.is_ok(), "ordinary provider owner should be allowed");
    }

    #[test]
    fn capacity_telemetry_is_permissionless_for_provider_owner() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        remove_permission(&mut stx, "CanSubmitSorafsTelemetry");
        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("register capacity declaration");

        let telemetry = CapacityTelemetryRecord::new(
            provider, 0, 1, 1, 1, 1, 0, 0, 10_000, 10_000, 0, 0, 0, 0, 0,
        )
        .with_nonce(1);

        RecordCapacityTelemetry { record: telemetry }
            .execute(&alice(), &mut stx)
            .expect("ordinary provider owner should be allowed");
    }

    #[test]
    fn capacity_dispute_requires_permission() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        remove_permission(&mut stx, "CanFileSorafsCapacityDispute");

        let record = CapacityDisputeRecord::new_pending(
            CapacityDisputeId::new([0x11; 32]),
            ProviderId::new([0x22; 32]),
            [0x33; 32],
            None,
            0,
            0,
            "dispute".to_string(),
            None,
            CapacityDisputeEvidence {
                digest: [0u8; 32],
                media_type: None,
                uri: None,
                size_bytes: None,
            },
            Vec::new(),
        );

        let err = RegisterCapacityDispute { record }
            .execute(&alice(), &mut stx)
            .expect_err("permissionless dispute must fail");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("CanFileSorafsCapacityDispute")
        ));
    }

    #[test]
    fn capacity_declaration_registers_owner_binding() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, declaration) = capacity_record_with_owner(&alice());

        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("register declaration");

        let owner = stx
            .world
            .provider_owners
            .get(&provider)
            .expect("owner binding recorded");
        assert_eq!(owner, &alice());
        let permission = AccountPermission::from(CanOperateSorafsRepair {
            provider_id: provider,
        });
        let perms = stx
            .world
            .account_permissions
            .get(&alice())
            .expect("permissions should be present");
        assert!(
            perms.contains(&permission),
            "repair worker permission should be granted"
        );
    }

    #[test]
    fn instruction_box_dispatches_capacity_declaration() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, declaration) = capacity_record_with_owner(&alice());

        let instruction = InstructionBox::from(RegisterCapacityDeclaration {
            record: declaration,
        });
        instruction
            .execute(&alice(), &mut stx)
            .expect("instruction box should dispatch SoraFS declaration");

        assert_eq!(
            stx.world.provider_owners.get(&provider),
            Some(&alice()),
            "instruction box execution should record the provider owner"
        );
    }

    #[test]
    fn capacity_declaration_rejects_rebinding_to_new_owner() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (_provider, declaration) = capacity_record_with_owner(&alice());
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("initial declaration");

        let (_provider, second) = capacity_record_with_owner(&bob());
        let err = RegisterCapacityDeclaration { record: second }
            .execute(&bob(), &mut stx)
            .expect_err("rebind to different owner must fail");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("already owned")
        ));
    }

    #[test]
    fn capacity_telemetry_enforces_owner_metadata() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("register declaration");

        stx.gov.sorafs_telemetry.require_submitter = true;
        stx.gov.sorafs_telemetry.submitters = vec![alice(), bob()];

        let telemetry = CapacityTelemetryRecord::new(
            provider, 1, 2, 512, 512, 512, 0, 0, 1_000, 1_000, 0, 0, 0, 0, 0,
        )
        .with_nonce(1);
        let err = RecordCapacityTelemetry { record: telemetry }
            .execute(&bob(), &mut stx)
            .expect_err("non-owner telemetry must fail");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains(PROVIDER_OWNER_METADATA_KEY)
        ));

        RecordCapacityTelemetry { record: telemetry }
            .execute(&alice(), &mut stx)
            .expect("owner telemetry succeeds");
    }

    #[test]
    fn capacity_dispute_requires_owner() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("register declaration");

        let (record, _dispute_id) = sample_capacity_dispute(provider);
        let err = RegisterCapacityDispute { record }
            .execute(&bob(), &mut stx)
            .expect_err("non-owner dispute must fail");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("provider")
        ));
    }

    #[test]
    fn provider_credit_requires_owner() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("register declaration");

        let credit = ProviderCreditRecord::new(
            provider,
            xor_quantity_nanos(1),
            Quantity::zero(),
            Quantity::zero(),
            Quantity::zero(),
            1,
            1,
            Metadata::default(),
        );

        let err = UpsertProviderCredit {
            record: credit.clone(),
        }
        .execute(&bob(), &mut stx)
        .expect_err("non-owner upsert must fail");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("provider")
        ));

        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("owner upsert succeeds");
    }

    #[test]
    fn capacity_declaration_enforces_owner_metadata_when_present() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, mut declaration) = sample_capacity_record();

        let key = Name::from_str(PROVIDER_OWNER_METADATA_KEY).expect("metadata key");
        declaration
            .metadata
            .insert(key.clone(), Json::new(account_literal(&bob())));

        let err = RegisterCapacityDeclaration {
            record: declaration.clone(),
        }
        .execute(&alice(), &mut stx)
        .expect_err("owner mismatch must fail");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains(PROVIDER_OWNER_METADATA_KEY)
        ));

        // Align owner with authority and succeed
        declaration
            .metadata
            .insert(key, Json::new(account_literal(&alice())));
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("owner-aligned declaration must succeed");

        assert!(stx.world.capacity_declarations.get(&provider).is_some());
    }

    #[test]
    fn register_capacity_dispute_rejects_duplicate() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration.clone(),
        }
        .execute(&alice(), &mut stx)
        .expect("register declaration");

        let (record, _dispute_id) = sample_capacity_dispute(provider);
        RegisterCapacityDispute {
            record: record.clone(),
        }
        .execute(&alice(), &mut stx)
        .expect("register capacity dispute");

        let err = RegisterCapacityDispute { record }
            .execute(&alice(), &mut stx)
            .expect_err("duplicate dispute must be rejected");
        let message = match err {
            InstructionExecutionError::InvariantViolation(message) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("duplicate capacity dispute identifier"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn register_capacity_dispute_rejects_unknown_replication_order() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration.clone(),
        }
        .execute(&alice(), &mut stx)
        .expect("register declaration");

        let replication_order_id = ReplicationOrderId::new([0xAB; 32]);
        let dispute = CapacityDisputeV1 {
            version: CAPACITY_DISPUTE_VERSION_V1,
            provider_id: provider.as_bytes().to_owned(),
            complainant_id: [0x99; 32],
            replication_order_id: Some(*replication_order_id.as_bytes()),
            kind: CapacityDisputeKind::ReplicationShortfall,
            evidence: sorafs_manifest::capacity::CapacityDisputeEvidenceV1 {
                evidence_digest: [0xCD; 32],
                media_type: Some("application/json".into()),
                uri: Some("https://evidence.example/dispute.json".into()),
                size_bytes: Some(512),
            },
            submitted_epoch: 1_700_000_256,
            description: "replication order not ingested".into(),
            requested_remedy: Some("slash bond".into()),
        };
        let payload = norito::to_bytes(&dispute).expect("encode dispute payload");
        let dispute_id = CapacityDisputeId::new(*blake3_hash(&payload).as_bytes());
        let evidence = CapacityDisputeEvidence {
            digest: dispute.evidence.evidence_digest,
            media_type: dispute.evidence.media_type.clone(),
            uri: dispute.evidence.uri.clone(),
            size_bytes: dispute.evidence.size_bytes,
        };
        let record = CapacityDisputeRecord::new_pending(
            dispute_id,
            provider,
            dispute.complainant_id,
            Some(*replication_order_id.as_bytes()),
            dispute.kind as u8,
            dispute.submitted_epoch,
            dispute.description.clone(),
            dispute.requested_remedy.clone(),
            evidence,
            payload,
        );

        let err = RegisterCapacityDispute { record }
            .execute(&alice(), &mut stx)
            .expect_err("replication order reference must exist");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message
            )) if message.contains("replication order")
        ));
    }

    #[test]
    fn register_manifest_activates_record_immediately() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let instruction = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };

        instruction
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored");
        assert_eq!(stored.status, PinStatus::Approved(5));
        assert_eq!(stored.chunk_digest_sha3_256, default_chunk_digest());
        assert!(stored.council_envelope_digest.is_none());
        assert_eq!(stx.world.replication_orders.iter().count(), 0);
    }

    #[test]
    fn governed_registration_stays_pending_until_verified_approval() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        stx.gov.sorafs_pin_policy.require_council_signatures = true;

        let (provider, mut declaration) = capacity_record_with_owner(&alice());
        declaration.valid_from_epoch = 4;
        declaration.valid_until_epoch = 20;
        stx.world.provider_owners.insert(provider, alice());
        stx.world
            .capacity_declarations
            .insert(provider, declaration);

        let alias = default_alias_binding();
        let mut manifest = manifest_fixture(0xAA);
        manifest.pin_policy.min_replicas = 1;
        RegisterPinManifest {
            manifest_payload: manifest.encode().expect("encode manifest"),
            submitted_epoch: 5,
            alias: Some(alias.clone()),
            successor_of: None,
        }
        .execute(&alice(), &mut stx)
        .expect("governed registration");

        let pending = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("pending manifest stored")
            .clone();
        assert_eq!(pending.status, PinStatus::Pending);
        assert!(pending.pin_fee_payment.is_some());
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&alias))
                .is_none(),
            "pending manifests must not publish aliases"
        );
        assert_eq!(stx.world.replication_orders.iter().count(), 0);

        let council_key = checked_ed25519_keypair();
        let (envelope, _) = build_envelope(&pending, &council_key);
        ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 6,
            council_envelope: Some(envelope),
            council_envelope_digest: None,
        }
        .execute(&alice(), &mut stx)
        .expect("verified governed approval");

        let approved = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("approved manifest stored");
        assert_eq!(approved.status, PinStatus::Approved(6));
        assert!(approved.council_envelope_digest.is_some());
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&alias))
                .is_some(),
            "approval publishes the reserved alias"
        );
        let (_, order) = stx
            .world
            .replication_orders
            .iter()
            .next()
            .expect("approval issues deferred replication order");
        assert_eq!(order.issued_epoch, 6);
    }

    #[test]
    fn governed_pending_approval_requires_payload_and_nonstale_epoch() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        insert_pending_manifest(&mut stx, default_digest(), default_chunk_digest());

        let digest_only = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 5,
            council_envelope: None,
            council_envelope_digest: Some([0x66; 32]),
        }
        .execute(&alice(), &mut stx)
        .expect_err("first approval must not trust a digest without its envelope");
        assert!(matches!(
            digest_only,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("requires council envelope payload")
        ));

        let record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("pending record")
            .clone();
        let (envelope, _) = build_envelope(&record, &checked_ed25519_keypair());
        let stale = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: record.submitted_epoch - 1,
            council_envelope: Some(envelope),
            council_envelope_digest: None,
        }
        .execute(&alice(), &mut stx)
        .expect_err("approval must not predate submission");
        assert!(matches!(
            stale,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("predates submission")
        ));
        assert_eq!(
            stx.world
                .pin_manifests
                .get(&default_digest())
                .expect("record remains")
                .status,
            PinStatus::Pending
        );

        let (envelope, _) = build_envelope(&record, &checked_ed25519_keypair());
        let expired = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: record.policy.retention_epoch,
            council_envelope: Some(envelope),
            council_envelope_digest: None,
        }
        .execute(&alice(), &mut stx)
        .expect_err("approval at retention expiry must be rejected");
        assert!(matches!(
            expired,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("earlier than retention epoch")
        ));
        assert_eq!(
            stx.world
                .pin_manifests
                .get(&default_digest())
                .expect("record remains")
                .status,
            PinStatus::Pending
        );
    }

    #[test]
    fn register_manifest_rejects_unknown_chunker_profile() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);
        let mut manifest = manifest_fixture(0xAA);
        manifest.chunking.profile_id = ProfileId(u32::MAX);
        let instruction = RegisterPinManifest {
            manifest_payload: manifest.encode().expect("encode invalid manifest fixture"),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        let err = instruction
            .execute(&alice(), &mut stx)
            .expect_err("registration must reject unknown chunker profile");

        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message
            )) if message.contains("manifest payload failed validation")
        ));
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_rejects_inert_commitments_and_expired_retention_before_fee() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);
        let base = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };

        let mut inert_manifest_payload = manifest_fixture(0xAA);
        inert_manifest_payload.root_cid[4..].fill(0);
        let mut zero_manifest = base.clone();
        zero_manifest.manifest_payload = inert_manifest_payload
            .encode()
            .expect("encode inert manifest fixture");
        let mut inert_chunk_digest_manifest = manifest_fixture(0xAA);
        inert_chunk_digest_manifest.chunk_digest_sha3_256 = [0; 32];
        let mut zero_chunks = base.clone();
        zero_chunks.manifest_payload = inert_chunk_digest_manifest
            .encode()
            .expect("encode inert chunk-digest fixture");
        let mut zero_retention_payload = manifest_fixture(0xAA);
        zero_retention_payload.pin_policy.retention_epoch = 0;
        let mut zero_retention = base.clone();
        zero_retention.manifest_payload = zero_retention_payload
            .encode()
            .expect("encode zero-retention fixture");
        let mut zero_successor = base.clone();
        zero_successor.successor_of = Some(ManifestDigest::new([0; 32]));
        let mut expired = base;
        expired.submitted_epoch = default_policy().retention_epoch;

        for (instruction, expected) in [
            (zero_manifest, "root CID digest must not be all zero"),
            (
                zero_chunks,
                "manifest chunk-plan SHA3-256 digest must not be zero",
            ),
            (zero_retention, "pin retention epoch must be positive"),
            (zero_successor, "successor manifest digest must not be zero"),
            (expired, "must be greater than submission epoch"),
        ] {
            let err = instruction
                .execute(&alice(), &mut stx)
                .expect_err("inert manifest commitment must be rejected");
            assert!(matches!(
                err,
                InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(message)
                ) if message.contains(expected)
            ));
        }

        assert_eq!(stx.world.pin_manifests.iter().count(), 0);
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_rejects_malformed_noncanonical_and_oversized_payloads_atomically() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let canonical = default_manifest_payload();
        let alternate = {
            let _guard = norito::core::DecodeFlagsGuard::enter(0);
            norito::to_bytes(&manifest_fixture(0xAA)).expect("encode alternate layout")
        };
        assert_ne!(alternate, canonical, "test requires a non-canonical layout");
        let mut trailing = canonical;
        trailing.push(0);
        let mut allocation_bomb = manifest_fixture(0xAA);
        allocation_bomb
            .alias_claims
            .push(sorafs_manifest::AliasClaim {
                name: "bomb".to_owned(),
                namespace: "adversarial".to_owned(),
                proof: vec![0xA5; sorafs_manifest::MAX_MANIFEST_ALIAS_PROOF_BYTES + 1],
            });
        let allocation_bomb = allocation_bomb
            .encode()
            .expect("encode allocation-bomb manifest fixture");
        assert!(
            allocation_bomb.len() <= sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES,
            "resource-limit fixture must pass the outer byte ceiling"
        );

        for (payload, expected) in [
            (Vec::new(), "manifest payload has"),
            (vec![0xFF, 0x00, 0x81], "invalid canonical ManifestV1"),
            (
                vec![0xA5; sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES + 1],
                "manifest payload has",
            ),
            (alternate, "canonical first-release Norito"),
            (trailing, "ManifestV1"),
            (allocation_bomb, "invalid canonical ManifestV1"),
        ] {
            let error = RegisterPinManifest {
                manifest_payload: payload,
                submitted_epoch: 5,
                alias: None,
                successor_of: None,
            }
            .execute(&alice(), &mut stx)
            .expect_err("invalid manifest payload must fail");
            assert!(matches!(
                error,
                InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(message)
                ) if message.contains(expected)
            ));
            assert_eq!(stx.world.pin_manifests.iter().count(), 0);
            assert_eq!(stx.world.replication_orders.iter().count(), 0);
        }

        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_rejects_invalid_policy() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);
        let mut manifest = manifest_fixture(0xAA);
        manifest.pin_policy.min_replicas = 0;
        let instruction = RegisterPinManifest {
            manifest_payload: manifest.encode().expect("encode invalid policy fixture"),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        let err = instruction
            .execute(&alice(), &mut stx)
            .expect_err("registration must reject invalid policy");

        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message
            )) if message.contains("manifest payload failed validation")
        ));
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_with_alias_persists_binding() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alias = default_alias_binding();
        let instruction = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: Some(alias.clone()),
            successor_of: None,
        };

        instruction
            .execute(&alice(), &mut stx)
            .expect("register manifest with alias");

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored");
        let stored_alias = stored.alias.as_ref().expect("alias stored");
        assert_eq!(stored_alias.name, alias.name);
        assert_eq!(stored_alias.namespace, alias.namespace);
        let alias_record = stx
            .world
            .manifest_aliases
            .get(&ManifestAliasId::from(&alias))
            .expect("alias binding stored");
        assert!(alias_record.targets_manifest(&default_digest()));
        assert_eq!(alias_record.bound_epoch, 5);
    }

    #[test]
    fn approve_manifest_with_alias_records_council_digest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alias = default_alias_binding();

        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: Some(alias.clone()),
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let stored_record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .cloned()
            .expect("manifest stored");
        let council_key = checked_ed25519_keypair();
        let (envelope, _) = build_envelope(&stored_record, &council_key);

        ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 5,
            council_envelope: Some(envelope),
            council_envelope_digest: None,
        }
        .execute(&alice(), &mut stx)
        .expect("approve manifest");

        let alias_id = ManifestAliasId::from(&alias);
        let alias_record = stx
            .world
            .manifest_aliases
            .get(&alias_id)
            .expect("alias record stored");
        assert!(alias_record.targets_manifest(&default_digest()));
        assert_eq!(alias_record.bound_epoch, 5);
        assert_eq!(alias_record.expiry_epoch, default_policy().retention_epoch);
        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored after approval");
        assert!(stored.council_envelope_digest.is_some());
    }

    #[test]
    fn register_manifest_auto_issues_replication_order_for_matching_capacity() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, mut declaration) = capacity_record_with_owner(&alice());
        declaration.valid_from_epoch = 4;
        declaration.valid_until_epoch = 20;
        stx.world.provider_owners.insert(provider, alice());
        stx.world
            .capacity_declarations
            .insert(provider, declaration);

        let mut manifest = manifest_fixture(0xAA);
        manifest.pin_policy.min_replicas = 1;
        RegisterPinManifest {
            manifest_payload: manifest.encode().expect("encode manifest"),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        }
        .execute(&alice(), &mut stx)
        .expect("register manifest");

        let (_order_id, order) = stx
            .world
            .replication_orders
            .iter()
            .next()
            .expect("auto replication order stored");
        assert_eq!(order.manifest_digest, default_digest());
        assert_eq!(order.issued_epoch, 5);
        assert_eq!(order.deadline_epoch, 6);

        let decoded = norito::decode_from_bytes::<ReplicationOrderV1>(&order.canonical_order)
            .expect("decode order");
        assert_eq!(decoded.target_replicas, 1);
        assert_eq!(decoded.assignments.len(), 1);
        assert_eq!(decoded.assignments[0].provider_id, *provider.as_bytes());
        assert_eq!(decoded.assignments[0].slice_gib, 1);
    }

    #[test]
    fn automatic_replication_timestamp_overflow_fails_before_fee_or_state_mutation() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, mut declaration) = capacity_record_with_owner(&alice());
        declaration.valid_from_epoch = 0;
        declaration.valid_until_epoch = u64::MAX;
        stx.world.provider_owners.insert(provider, alice());
        stx.world
            .capacity_declarations
            .insert(provider, declaration);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let mut manifest = manifest_fixture(0xAA);
        manifest.pin_policy.min_replicas = 1;
        manifest.pin_policy.retention_epoch = u64::MAX;
        let error = RegisterPinManifest {
            manifest_payload: manifest.encode().expect("encode manifest"),
            submitted_epoch: u64::MAX - 1,
            alias: None,
            successor_of: None,
        }
        .execute(&alice(), &mut stx)
        .expect_err("automatic replication time overflow must fail closed");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("issuance time overflow")
        ));
        assert_eq!(stx.world.pin_manifests.iter().count(), 0);
        assert_eq!(stx.world.replication_orders.iter().count(), 0);
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn approval_replication_overflow_does_not_publish_pending_alias() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, mut declaration) = capacity_record_with_owner(&alice());
        declaration.valid_from_epoch = 0;
        declaration.valid_until_epoch = u64::MAX;
        stx.world.provider_owners.insert(provider, alice());
        stx.world
            .capacity_declarations
            .insert(provider, declaration);

        let alias = default_alias_binding();
        let mut policy = default_policy();
        policy.min_replicas = 1;
        let approval_epoch = u64::MAX / AUTO_REPLICATION_ORDER_SECS_PER_EPOCH + 1;
        policy.retention_epoch = approval_epoch + 1;
        let record = PinManifestRecord::new(
            default_digest(),
            default_root_cid(),
            default_chunker(),
            default_chunk_digest(),
            policy,
            alice(),
            approval_epoch,
            Some(alias.clone()),
            None,
            Metadata::default(),
        )
        .with_content_length(default_content_length());
        stx.world
            .pin_manifests
            .insert(default_digest(), record.clone());
        let (envelope, _) = build_envelope(&record, &checked_ed25519_keypair());

        let error = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: approval_epoch,
            council_envelope: Some(envelope),
            council_envelope_digest: None,
        }
        .execute(&alice(), &mut stx)
        .expect_err("automatic replication overflow must fail approval atomically");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("issuance time overflow")
        ));
        assert_eq!(
            stx.world
                .pin_manifests
                .get(&default_digest())
                .expect("pending manifest remains")
                .status,
            PinStatus::Pending
        );
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&alias))
                .is_none(),
            "failed approval must not publish the pending alias"
        );
        assert_eq!(stx.world.replication_orders.iter().count(), 0);
    }

    #[test]
    fn register_manifest_rejects_duplicate_alias_binding() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let alias = default_alias_binding();
        let duplicate_alias = alias_binding_for(second_digest(), "sora", "docs", 0, 0);
        let first = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: Some(alias.clone()),
            successor_of: None,
        };
        first
            .execute(&alice(), &mut stx)
            .expect("first alias registration succeeds");
        let alice_balance_after_first = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_after_first = pin_fee_balance(&stx, &treasury_account);

        let second = RegisterPinManifest {
            manifest_payload: manifest_payload_for_seed(0xBB),
            submitted_epoch: 6,
            alias: Some(duplicate_alias),
            successor_of: None,
        };
        let err = second
            .execute(&alice(), &mut stx)
            .expect_err("duplicate alias must be rejected");

        match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => assert!(
                message.contains("alias `sora/docs` is already bound"),
                "unexpected error message: {message}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
        assert_eq!(
            pin_fee_balance(&stx, &alice()),
            alice_balance_after_first,
            "duplicate alias replay must not charge the submitter again"
        );
        assert_eq!(
            pin_fee_balance(&stx, &treasury_account),
            treasury_balance_after_first,
            "duplicate alias replay must not credit treasury again"
        );
    }

    #[test]
    fn register_manifest_rejects_duplicate_digest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        }
        .execute(&alice(), &mut stx)
        .expect("first manifest registration succeeds");
        let alice_balance_after_first = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_after_first = pin_fee_balance(&stx, &treasury_account);

        let err = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 6,
            alias: None,
            successor_of: None,
        }
        .execute(&alice(), &mut stx)
        .expect_err("duplicate manifest digest must be rejected");
        let message = match err {
            InstructionExecutionError::InvariantViolation(message) => message.to_string(),
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("already registered"),
            "unexpected error message: {message}"
        );
        assert_eq!(
            pin_fee_balance(&stx, &alice()),
            alice_balance_after_first,
            "duplicate digest replay must not charge the submitter again"
        );
        assert_eq!(
            pin_fee_balance(&stx, &treasury_account),
            treasury_balance_after_first,
            "duplicate digest replay must not credit treasury again"
        );
    }

    #[test]
    fn register_manifest_rejects_empty_alias_proof_without_side_effects() {
        let alias = ManifestAliasBinding {
            name: "docs".into(),
            namespace: "sora".into(),
            proof: Vec::new(),
        };
        assert_alias_registration_rejected_without_fee(alias, "alias proof must not be empty");
    }

    #[test]
    fn register_manifest_rejects_oversized_alias_proof_without_side_effects() {
        let alias = ManifestAliasBinding {
            name: "docs".into(),
            namespace: "sora".into(),
            proof: vec![0xA5; MAX_ALIAS_PROOF_BYTES + 1],
        };
        assert_alias_registration_rejected_without_fee(alias, "maximum is");
    }

    #[test]
    fn register_manifest_rejects_alias_proof_alias_mismatch_without_side_effects() {
        let mut alias = alias_binding_for(
            default_digest(),
            "sora",
            "other",
            5,
            default_policy().retention_epoch,
        );
        alias.name = "docs".to_owned();
        assert_alias_registration_rejected_without_fee(
            alias,
            "does not match requested alias `sora/docs`",
        );
    }

    #[test]
    fn register_manifest_rejects_alias_proof_with_wrong_epoch_commitments() {
        assert_alias_registration_rejected_without_fee(
            alias_binding_for(
                default_digest(),
                "sora",
                "docs",
                4,
                default_policy().retention_epoch,
            ),
            "alias proof bound_at",
        );
        assert_alias_registration_rejected_without_fee(
            alias_binding_for(default_digest(), "sora", "docs", 5, 41),
            "alias proof expiry_epoch",
        );
    }

    #[test]
    fn register_manifest_rejects_alias_whitespace_without_side_effects() {
        let alias = ManifestAliasBinding {
            name: "docs ".into(),
            namespace: "sora".into(),
            proof: Vec::new(),
        };
        assert_alias_registration_rejected_without_fee(alias, "surrounding whitespace");
    }

    #[test]
    fn register_manifest_rejects_malformed_alias_proof_without_side_effects() {
        let alias = ManifestAliasBinding {
            name: "docs".into(),
            namespace: "sora".into(),
            proof: vec![0xFF, 0x00, 0xAA],
        };
        assert_alias_registration_rejected_without_fee(alias, "alias proof failed verification");
    }

    #[test]
    fn register_manifest_rejects_stale_alias_record_without_side_effects() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let requested_alias = default_alias_binding();
        let stale_alias = alias_binding_for(second_digest(), "sora", "docs", 0, 0);
        let stale_record =
            ManifestAliasRecord::new(stale_alias.clone(), second_digest(), bob(), 1, 10);
        stx.world
            .manifest_aliases
            .insert(ManifestAliasId::from(&stale_alias), stale_record);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let err = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: Some(requested_alias),
            successor_of: None,
        }
        .execute(&alice(), &mut stx)
        .expect_err("stale alias record must reject registration");

        match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => assert!(
                message.contains("alias `sora/docs` is already associated"),
                "unexpected error message: {message}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(
            stx.world.pin_manifests.get(&default_digest()).is_none(),
            "rejected stale-alias registration must not store a pin manifest"
        );
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_rejects_alias_proof_manifest_mismatch() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let mismatched_alias = alias_binding_for(
            second_digest(),
            "sora",
            "docs",
            5,
            default_policy().retention_epoch,
        );
        let instruction = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: Some(mismatched_alias),
            successor_of: None,
        };

        let err = instruction
            .execute(&alice(), &mut stx)
            .expect_err("registration must reject mismatched alias proof");

        match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => assert!(
                message.contains("alias proof manifest CID does not match registered digest"),
                "unexpected error message: {message}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_rejects_invalid_alias_characters() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);
        let alias = ManifestAliasBinding {
            name: "Docs".into(),
            namespace: "sora".into(),
            proof: Vec::new(),
        };
        let instruction = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: Some(alias),
            successor_of: None,
        };

        let err = instruction
            .execute(&alice(), &mut stx)
            .expect_err("registration must reject invalid alias");

        match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => assert!(
                message.contains("alias name `Docs`"),
                "unexpected error message: {message}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_with_approved_predecessor_persists_successor_of() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        RegisterPinManifest {
            manifest_payload: manifest_payload_for_seed(0xBB),
            submitted_epoch: 4,
            alias: None,
            successor_of: None,
        }
        .execute(&alice(), &mut stx)
        .expect("register predecessor manifest");

        RegisterPinManifest {
            manifest_payload: manifest_payload_for_seed(0xCC),
            submitted_epoch: 6,
            alias: None,
            successor_of: Some(second_digest()),
        }
        .execute(&alice(), &mut stx)
        .expect("register successor manifest");

        let stored = stx
            .world
            .pin_manifests
            .get(&third_digest())
            .expect("successor manifest stored");
        assert_eq!(stored.successor_of, Some(second_digest()));
        assert_eq!(stored.status, PinStatus::Approved(6));
    }

    #[test]
    fn register_manifest_rejects_self_successor_reference() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let err = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: Some(default_digest()),
        }
        .execute(&alice(), &mut stx)
        .expect_err("registration must reject self successor");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("cannot declare itself as successor"),
            "unexpected error message: {message}"
        );
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_rejects_unregistered_predecessor() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let err = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: Some(second_digest()),
        }
        .execute(&alice(), &mut stx)
        .expect_err("registration must reject missing predecessor");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("is not registered"),
            "unexpected error message: {message}"
        );
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_rejects_pending_predecessor() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        insert_pending_manifest(&mut stx, second_digest(), [0xEE; 32]);
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let err = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: Some(second_digest()),
        }
        .execute(&alice(), &mut stx)
        .expect_err("registration must reject pending predecessor");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("must be approved before registering successor"),
            "unexpected error message: {message}"
        );
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_rejects_retired_predecessor() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        insert_manifest_with_status(
            &mut stx,
            second_digest(),
            [0xEE; 32],
            None,
            PinStatus::Retired(7),
        );
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let err = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 8,
            alias: None,
            successor_of: Some(second_digest()),
        }
        .execute(&alice(), &mut stx)
        .expect_err("registration must reject retired predecessor");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("was retired at epoch 7"),
            "unexpected error message: {message}"
        );
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_rejects_successor_cycle_closure() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        insert_manifest_with_status(
            &mut stx,
            second_digest(),
            [0xEE; 32],
            Some(third_digest()),
            PinStatus::Approved(5),
        );
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let err = RegisterPinManifest {
            manifest_payload: manifest_payload_for_seed(0xCC),
            submitted_epoch: 6,
            alias: None,
            successor_of: Some(second_digest()),
        }
        .execute(&alice(), &mut stx)
        .expect_err("registration must reject cycle closure");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("would create a cycle"),
            "unexpected error message: {message}"
        );
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn register_manifest_rejects_existing_cycle_in_predecessor_chain() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        insert_manifest_with_status(
            &mut stx,
            second_digest(),
            [0xEE; 32],
            Some(third_digest()),
            PinStatus::Approved(5),
        );
        insert_manifest_with_status(
            &mut stx,
            third_digest(),
            [0xEF; 32],
            Some(second_digest()),
            PinStatus::Approved(5),
        );
        let alice_balance_before = pin_fee_balance(&stx, &alice());
        let treasury_account = stx.gov.sorafs_pin_fee_treasury_account.clone();
        let treasury_balance_before = pin_fee_balance(&stx, &treasury_account);

        let err = RegisterPinManifest {
            manifest_payload: manifest_payload_for_seed(0xDD),
            submitted_epoch: 6,
            alias: None,
            successor_of: Some(second_digest()),
        }
        .execute(&alice(), &mut stx)
        .expect_err("registration must reject malformed existing cycle");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("forms a cycle"),
            "unexpected error message: {message}"
        );
        assert_pin_fee_balances_unchanged(
            &stx,
            &alice(),
            alice_balance_before,
            &treasury_account,
            treasury_balance_before,
        );
    }

    #[test]
    fn approve_manifest_records_council_digest_for_auto_approved_manifest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let stored_record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored")
            .clone();
        let council_key = checked_ed25519_keypair();
        let (envelope, _) = build_envelope(&stored_record, &council_key);
        let expected_digest = {
            let hash = blake3_hash(&envelope);
            let mut out = [0u8; 32];
            out.copy_from_slice(hash.as_bytes());
            out
        };

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 5,
            council_envelope: Some(envelope),
            council_envelope_digest: None,
        };
        approve
            .execute(&alice(), &mut stx)
            .expect("approve manifest");

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored");
        assert!(matches!(stored.status, PinStatus::Approved(5)));
        assert_eq!(stored.council_envelope_digest, Some(expected_digest));
    }

    #[test]
    fn approve_manifest_rejects_mismatched_manifest_digest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let stored_record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored")
            .clone();
        let council_key = checked_ed25519_keypair();
        let (envelope, _signature_hex) = build_envelope(&stored_record, &council_key);

        let mut invalid_json =
            String::from_utf8(envelope.clone()).expect("envelope is valid UTF-8 JSON");
        let manifest_hex = hex::encode(default_digest().as_bytes());
        let bogus_manifest = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        invalid_json = invalid_json.replacen(&manifest_hex, bogus_manifest, 1);
        let invalid_envelope = invalid_json.into_bytes();

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 7,
            council_envelope: Some(invalid_envelope),
            council_envelope_digest: None,
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("approval must reject mismatched manifest digest");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("manifest digest"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn approve_manifest_rejects_invalid_signature() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let stored_record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored")
            .clone();
        let council_key = checked_ed25519_keypair();
        let (envelope, signature_hex) = build_envelope(&stored_record, &council_key);

        let mut modified_signature =
            hex::decode(&signature_hex).expect("signature hex decodes cleanly");
        modified_signature[0] ^= 0xFF;
        let bad_signature_hex = hex::encode(modified_signature);

        let mut invalid_json =
            String::from_utf8(envelope.clone()).expect("envelope is valid UTF-8 JSON");
        invalid_json = invalid_json.replacen(&signature_hex, &bad_signature_hex, 1);
        let invalid_envelope = invalid_json.into_bytes();

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 7,
            council_envelope: Some(invalid_envelope),
            council_envelope_digest: None,
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("approval must reject invalid signature");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("failed to verify council signature")
                || message.contains("invalid council signature material"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn approve_manifest_rejects_all_zero_signature_material() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let stored_record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored")
            .clone();
        let council_key = checked_ed25519_keypair();
        let (envelope, signature_hex) = build_envelope(&stored_record, &council_key);
        let inert_signature_hex = hex::encode([0_u8; 64]);

        let mut invalid_json =
            String::from_utf8(envelope.clone()).expect("envelope is valid UTF-8 JSON");
        invalid_json = invalid_json.replacen(&signature_hex, &inert_signature_hex, 1);
        let invalid_envelope = invalid_json.into_bytes();

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 7,
            council_envelope: Some(invalid_envelope),
            council_envelope_digest: None,
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("approval must reject all-zero signature material");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("signature payload must not be all zero"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn approve_manifest_rejects_inert_or_malformed_ed25519_signer_key() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let stored_record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored")
            .clone();
        let council_key = checked_ed25519_keypair();
        let (envelope, _signature_hex) = build_envelope(&stored_record, &council_key);
        let (_, signer_bytes) = council_key
            .public_key()
            .try_to_bytes()
            .expect("council signer key bytes");
        let signer_hex = hex::encode(signer_bytes);

        for (label, malformed_signer) in [
            ("all-zero", [0_u8; 32]),
            ("small-order", SMALL_ORDER_ED25519_R),
            ("noncanonical", NONCANONICAL_ED25519_R),
        ] {
            let malformed_signer_hex = hex::encode(malformed_signer);
            let mut invalid_json =
                String::from_utf8(envelope.clone()).expect("envelope is valid UTF-8 JSON");
            invalid_json = invalid_json.replacen(&signer_hex, &malformed_signer_hex, 1);

            let approve = ApprovePinManifest {
                digest: default_digest(),
                approved_epoch: 7,
                council_envelope: Some(invalid_json.into_bytes()),
                council_envelope_digest: None,
            };
            let err = approve
                .execute(&alice(), &mut stx)
                .expect_err("approval must reject malformed signer public key material");
            let message = match err {
                InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(message),
                ) => message,
                other => panic!("unexpected error for {label} signer key: {other:?}"),
            };
            assert!(
                message.contains("failed to parse council signer"),
                "{label} signer key produced unexpected error message: {message}"
            );
        }
    }

    #[test]
    fn approve_manifest_rejects_malformed_ed25519_signature_r() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let stored_record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored")
            .clone();
        let council_key = checked_ed25519_keypair();
        let (envelope, signature_hex) = build_envelope(&stored_record, &council_key);

        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_ED25519_R),
            ("noncanonical", NONCANONICAL_ED25519_R),
        ] {
            let mut modified_signature =
                hex::decode(&signature_hex).expect("signature hex decodes cleanly");
            modified_signature[..replacement_r.len()].copy_from_slice(&replacement_r);
            let bad_signature_hex = hex::encode(modified_signature);

            let mut invalid_json =
                String::from_utf8(envelope.clone()).expect("envelope is valid UTF-8 JSON");
            invalid_json = invalid_json.replacen(&signature_hex, &bad_signature_hex, 1);

            let approve = ApprovePinManifest {
                digest: default_digest(),
                approved_epoch: 7,
                council_envelope: Some(invalid_json.into_bytes()),
                council_envelope_digest: None,
            };
            let err = approve
                .execute(&alice(), &mut stx)
                .expect_err("approval must reject malformed signature R");
            let message = match err {
                InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(message),
                ) => message,
                other => panic!("unexpected error: {other:?}"),
            };
            assert!(
                message.contains("invalid council signature material"),
                "{label} signature R produced unexpected error message: {message}"
            );
            assert!(
                !message.contains("failed to verify council signature"),
                "{label} signature R reached backend verification: {message}"
            );
        }
    }

    #[test]
    fn council_envelope_rejects_resource_and_canonicalization_attacks() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        insert_pending_manifest(&mut stx, default_digest(), default_chunk_digest());
        let record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("pending manifest")
            .clone();
        let (envelope, _) = build_envelope(&record, &checked_ed25519_keypair());
        verify_council_envelope(&record, &envelope).expect("baseline envelope verifies");

        let oversized = vec![b' '; MAX_COUNCIL_ENVELOPE_BYTES + 1];
        assert!(council_envelope_error(&record, &oversized).contains("expected 1..="));

        let baseline: Value = json::from_slice(&envelope).expect("parse baseline envelope");

        let mut unknown_top = baseline.clone();
        unknown_top
            .as_object_mut()
            .expect("envelope object")
            .insert("unexpected".into(), Value::from(true));
        assert!(
            council_envelope_error(
                &record,
                &json::to_vec(&unknown_top).expect("encode unknown-field envelope"),
            )
            .contains("unknown field")
        );

        let mut unknown_signature_field = baseline.clone();
        unknown_signature_field
            .get_mut("signatures")
            .and_then(Value::as_array_mut)
            .and_then(|signatures| signatures.first_mut())
            .and_then(Value::as_object_mut)
            .expect("signature object")
            .insert("weight".into(), Value::from(1_u64));
        assert!(
            council_envelope_error(
                &record,
                &json::to_vec(&unknown_signature_field)
                    .expect("encode signature unknown-field envelope"),
            )
            .contains("unknown field")
        );

        let mut invalid_multihash_type = baseline.clone();
        invalid_multihash_type
            .get_mut("signatures")
            .and_then(Value::as_array_mut)
            .and_then(|signatures| signatures.first_mut())
            .and_then(Value::as_object_mut)
            .expect("signature object")
            .insert("signer_multihash".into(), Value::from(7_u64));
        assert!(
            council_envelope_error(
                &record,
                &json::to_vec(&invalid_multihash_type)
                    .expect("encode invalid multihash type envelope"),
            )
            .contains("must be a string")
        );

        let mut uppercase_signer = baseline.clone();
        let signer = uppercase_signer
            .get_mut("signatures")
            .and_then(Value::as_array_mut)
            .and_then(|signatures| signatures.first_mut())
            .and_then(Value::as_object_mut)
            .expect("signature object")
            .get("signer")
            .and_then(Value::as_str)
            .expect("signer hex")
            .to_ascii_uppercase();
        uppercase_signer
            .get_mut("signatures")
            .and_then(Value::as_array_mut)
            .and_then(|signatures| signatures.first_mut())
            .and_then(Value::as_object_mut)
            .expect("signature object")
            .insert("signer".into(), Value::from(signer));
        assert!(
            council_envelope_error(
                &record,
                &json::to_vec(&uppercase_signer).expect("encode uppercase envelope"),
            )
            .contains("lowercase hex")
        );

        let mut duplicate_signer = baseline.clone();
        let signatures = duplicate_signer
            .get_mut("signatures")
            .and_then(Value::as_array_mut)
            .expect("signature array");
        let first_signature = signatures[0].clone();
        signatures.push(first_signature);
        assert!(
            council_envelope_error(
                &record,
                &json::to_vec(&duplicate_signer).expect("encode duplicate signer envelope"),
            )
            .contains("distinct signer keys")
        );

        let mut signature_flood = baseline.clone();
        let signatures = signature_flood
            .get_mut("signatures")
            .and_then(Value::as_array_mut)
            .expect("signature array");
        let first_signature = signatures[0].clone();
        signatures.resize(MAX_COUNCIL_ENVELOPE_SIGNATURES + 1, first_signature);
        assert!(
            council_envelope_error(
                &record,
                &json::to_vec(&signature_flood).expect("encode signature flood"),
            )
            .contains("maximum")
        );

        for aliases in [
            vec![Value::from("sorafs.sf1@1.0.0"), Value::from("unknown")],
            vec![
                Value::from("sorafs.sf1@1.0.0"),
                Value::from("sorafs.sf1@1.0.0"),
            ],
        ] {
            let mut invalid_aliases = baseline.clone();
            invalid_aliases
                .as_object_mut()
                .expect("envelope object")
                .insert("profile_aliases".into(), Value::Array(aliases));
            let message = council_envelope_error(
                &record,
                &json::to_vec(&invalid_aliases).expect("encode invalid aliases"),
            );
            assert!(
                message.contains("unknown profile alias")
                    || message.contains("repeats profile alias"),
                "unexpected aliases error: {message}"
            );
        }
    }

    #[test]
    fn approve_manifest_rejects_digest_mismatch() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let stored_record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored")
            .clone();
        let council_key = checked_ed25519_keypair();
        let (envelope, _signature_hex) = build_envelope(&stored_record, &council_key);

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 7,
            council_envelope: Some(envelope),
            council_envelope_digest: Some([0x42; 32]),
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("approval must reject digest mismatch");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("approval digest mismatch"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn approve_manifest_rejects_provided_digest_mismatch_with_envelope() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let stored_record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored")
            .clone();
        let council_key = checked_ed25519_keypair();
        let (envelope, _) = build_envelope(&stored_record, &council_key);

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 5,
            council_envelope: Some(envelope),
            council_envelope_digest: Some([0x24; 32]),
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("approval must reject provided digest mismatch with envelope");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("approval digest mismatch with provided envelope"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn approve_manifest_rejects_unknown_manifest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 5,
            council_envelope: None,
            council_envelope_digest: None,
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("approval must reject unknown manifest");
        let message = match err {
            InstructionExecutionError::InvariantViolation(message) => message.to_string(),
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("not registered"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn approve_manifest_rejects_different_epoch_for_auto_approved_manifest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 7,
            council_envelope: None,
            council_envelope_digest: None,
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("approval must reject different epoch");
        let message = match err {
            InstructionExecutionError::InvariantViolation(message) => message.to_string(),
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("already approved with different epoch"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn approve_manifest_reapproval_accepts_stored_digest_without_payload() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let expected_digest = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored")
            .council_envelope_digest
            .expect("digest recorded");

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 5,
            council_envelope: None,
            council_envelope_digest: None,
        };
        approve
            .execute(&alice(), &mut stx)
            .expect("re-approval should reuse stored digest");

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored");
        assert_eq!(stored.council_envelope_digest, Some(expected_digest));
        assert!(matches!(stored.status, PinStatus::Approved(5)));
    }

    #[test]
    fn approve_manifest_reapproval_accepts_matching_stored_digest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let expected_digest = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored")
            .council_envelope_digest
            .expect("digest recorded");

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 5,
            council_envelope: None,
            council_envelope_digest: Some(expected_digest),
        };
        approve
            .execute(&alice(), &mut stx)
            .expect("re-approval should accept matching stored digest");

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored");
        assert_eq!(stored.council_envelope_digest, Some(expected_digest));
        assert!(matches!(stored.status, PinStatus::Approved(5)));
    }

    #[test]
    fn approve_manifest_reapproval_rejects_valid_replacement_envelope() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored")
            .clone();
        let expected_digest = record
            .council_envelope_digest
            .expect("initial approval digest recorded");
        let (replacement, _) = build_envelope(&record, &checked_ed25519_keypair());

        let error = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 5,
            council_envelope: Some(replacement),
            council_envelope_digest: None,
        }
        .execute(&alice(), &mut stx)
        .expect_err("a valid alternate envelope must not rewrite approval history");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("cannot replace its stored council envelope digest")
        ));

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest remains stored");
        assert_eq!(stored.council_envelope_digest, Some(expected_digest));
        assert_eq!(stored.status, PinStatus::Approved(5));
    }

    #[test]
    fn approve_manifest_reapproval_rejects_mismatched_stored_digest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 5,
            council_envelope: None,
            council_envelope_digest: Some([0x77; 32]),
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("re-approval must reject mismatched stored digest");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("stored digest"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn approve_manifest_reapproval_requires_payload_without_stored_digest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 5,
            council_envelope: None,
            council_envelope_digest: None,
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("re-approval must require payload when no digest is stored");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("re-approval requires council envelope payload"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn approve_pending_manifest_rejects_provided_digest_without_envelope() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        insert_pending_manifest(&mut stx, default_digest(), default_chunk_digest());

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 9,
            council_envelope: None,
            council_envelope_digest: Some([0x66; 32]),
        };
        let error = approve
            .execute(&alice(), &mut stx)
            .expect_err("pending approval must require the actual envelope");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("requires council envelope payload")
        ));

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored");
        assert!(matches!(stored.status, PinStatus::Pending));
        assert_eq!(stored.council_envelope_digest, None);
    }

    #[test]
    fn approve_pending_manifest_requires_envelope_or_digest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        insert_pending_manifest(&mut stx, default_digest(), default_chunk_digest());

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 9,
            council_envelope: None,
            council_envelope_digest: None,
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("pending approval should require envelope or digest");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("approval requires council envelope payload"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn approve_pending_manifest_rejects_invalid_signature_without_side_effects() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alias = default_alias_binding();
        let record = insert_pending_manifest_with_alias(
            &mut stx,
            default_digest(),
            default_chunk_digest(),
            alias.clone(),
        );
        let council_key = checked_ed25519_keypair();
        let (envelope, signature_hex) = build_envelope(&record, &council_key);

        let mut modified_signature =
            hex::decode(&signature_hex).expect("signature hex decodes cleanly");
        modified_signature[0] ^= 0xAA;
        let bad_signature_hex = hex::encode(modified_signature);
        let mut invalid_json =
            String::from_utf8(envelope.clone()).expect("envelope is valid UTF-8 JSON");
        invalid_json = invalid_json.replacen(&signature_hex, &bad_signature_hex, 1);

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 9,
            council_envelope: Some(invalid_json.into_bytes()),
            council_envelope_digest: None,
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("pending approval must reject invalid signature");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("failed to verify council signature")
                || message.contains("invalid council signature material"),
            "unexpected error message: {message}"
        );

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("pending manifest remains stored");
        assert!(matches!(stored.status, PinStatus::Pending));
        assert!(stored.council_envelope_digest.is_none());
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&alias))
                .is_none(),
            "failed approval must not bind the pending alias"
        );
    }

    #[test]
    fn approve_pending_manifest_rejects_alias_collision_without_side_effects() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alias = default_alias_binding();
        let record = insert_pending_manifest_with_alias(
            &mut stx,
            default_digest(),
            default_chunk_digest(),
            alias.clone(),
        );
        let stale_alias = alias_binding_for(second_digest(), "sora", "docs", 0, 0);
        let stale_record =
            ManifestAliasRecord::new(stale_alias.clone(), second_digest(), bob(), 1, 10);
        let alias_id = ManifestAliasId::from(&stale_alias);
        stx.world
            .manifest_aliases
            .insert(alias_id.clone(), stale_record);

        let council_key = checked_ed25519_keypair();
        let (envelope, _) = build_envelope(&record, &council_key);
        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 9,
            council_envelope: Some(envelope),
            council_envelope_digest: None,
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("pending approval must reject alias collision");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("alias `sora/docs` is already associated"),
            "unexpected error message: {message}"
        );

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("pending manifest remains stored");
        assert!(matches!(stored.status, PinStatus::Pending));
        assert!(stored.council_envelope_digest.is_none());
        let stale = stx
            .world
            .manifest_aliases
            .get(&alias_id)
            .expect("stale alias record remains");
        assert!(stale.targets_manifest(&second_digest()));
    }

    #[test]
    fn approve_pending_manifest_rejects_digest_mismatch_with_envelope_without_side_effects() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let alias = default_alias_binding();
        let record = insert_pending_manifest_with_alias(
            &mut stx,
            default_digest(),
            default_chunk_digest(),
            alias.clone(),
        );
        let council_key = checked_ed25519_keypair();
        let (envelope, _) = build_envelope(&record, &council_key);

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 9,
            council_envelope: Some(envelope),
            council_envelope_digest: Some([0x99; 32]),
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("pending approval must reject mismatched supplied digest");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("approval digest mismatch with provided envelope"),
            "unexpected error message: {message}"
        );

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("pending manifest remains stored");
        assert!(matches!(stored.status, PinStatus::Pending));
        assert!(stored.council_envelope_digest.is_none());
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&alias))
                .is_none(),
            "failed digest-mismatch approval must not bind the pending alias"
        );
    }

    #[test]
    fn approve_manifest_reapproval_rejects_digest_without_stored_digest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 5,
            council_envelope: None,
            council_envelope_digest: Some([0x12; 32]),
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("re-approval should reject digest-only input without stored digest");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("because no digest is stored"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn approve_manifest_rejects_retired_manifest() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let retire = RetirePinManifest {
            digest: default_digest(),
            retired_epoch: 8,
            reason: Some("superseded".into()),
        };
        retire.execute(&alice(), &mut stx).expect("retire manifest");

        let approve = ApprovePinManifest {
            digest: default_digest(),
            approved_epoch: 8,
            council_envelope: None,
            council_envelope_digest: None,
        };
        let err = approve
            .execute(&alice(), &mut stx)
            .expect_err("approval must reject retired manifest");
        let message = match err {
            InstructionExecutionError::InvariantViolation(message) => message.to_string(),
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("is retired and cannot be approved"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn retire_manifest_marks_record() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let register = RegisterPinManifest {
            manifest_payload: default_manifest_payload(),
            submitted_epoch: 5,
            alias: None,
            successor_of: None,
        };
        register
            .execute(&alice(), &mut stx)
            .expect("register manifest");

        let retire = RetirePinManifest {
            digest: default_digest(),
            retired_epoch: 10,
            reason: Some("superseded".into()),
        };
        retire.execute(&alice(), &mut stx).expect("retire manifest");

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored");
        assert!(matches!(stored.status, PinStatus::Retired(10)));
        assert_eq!(stored.retirement_reason.as_deref(), Some("superseded"));
    }

    #[test]
    fn retire_manifest_rejects_nonmonotonic_epochs_and_adversarial_reasons() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        insert_manifest_with_status(
            &mut stx,
            default_digest(),
            default_chunk_digest(),
            None,
            PinStatus::Approved(9),
        );

        for (retired_epoch, reason, expected) in [
            (4, None, "predates submission epoch"),
            (8, None, "predates approval epoch"),
        ] {
            let error = RetirePinManifest {
                digest: default_digest(),
                retired_epoch,
                reason,
            }
            .execute(&alice(), &mut stx)
            .expect_err("nonmonotonic retirement epoch must fail");
            assert!(matches!(
                error,
                InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(message)
                ) if message.contains(expected)
            ));
        }

        for reason in [
            String::new(),
            " padded".to_owned(),
            "line\nbreak".to_owned(),
            "x".repeat(MAX_RETIREMENT_REASON_BYTES + 1),
        ] {
            let error = RetirePinManifest {
                digest: default_digest(),
                retired_epoch: 9,
                reason: Some(reason),
            }
            .execute(&alice(), &mut stx)
            .expect_err("noncanonical retirement reason must fail");
            assert!(matches!(
                error,
                InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(message)
                ) if message.contains("retirement reason must be canonical")
            ));
        }

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest remains stored");
        assert_eq!(stored.status, PinStatus::Approved(9));
        assert!(stored.retirement_reason.is_none());
    }

    #[test]
    fn retire_manifest_rejects_conflicting_repeat_without_side_effects() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let binding = sample_alias_binding();
        BindManifestAlias {
            digest: default_digest(),
            binding: binding.clone(),
            bound_epoch: 8,
            expiry_epoch: 16,
        }
        .execute(&alice(), &mut stx)
        .expect("bind alias before retirement");

        RetirePinManifest {
            digest: default_digest(),
            retired_epoch: 10,
            reason: Some("superseded".into()),
        }
        .execute(&alice(), &mut stx)
        .expect("first retire succeeds");

        let err = RetirePinManifest {
            digest: default_digest(),
            retired_epoch: 11,
            reason: Some("different".into()),
        }
        .execute(&alice(), &mut stx)
        .expect_err("conflicting second retirement must fail");
        let message = match err {
            InstructionExecutionError::InvariantViolation(message) => message.to_string(),
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("already retired at epoch 10"),
            "unexpected error message: {message}"
        );

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest remains stored");
        assert!(matches!(stored.status, PinStatus::Retired(10)));
        assert_eq!(stored.retirement_reason.as_deref(), Some("superseded"));
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&binding))
                .is_none(),
            "retirement must keep the old alias record removed"
        );
    }

    #[test]
    fn bind_manifest_alias_registers_record() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let binding = sample_alias_binding();
        let bind = BindManifestAlias {
            digest: default_digest(),
            binding: binding.clone(),
            bound_epoch: 8,
            expiry_epoch: 16,
        };
        bind.execute(&alice(), &mut stx).expect("bind alias");

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest stored");
        assert_eq!(stored.alias.as_ref(), Some(&binding));

        let alias_id = ManifestAliasId::from(&binding);
        let alias_record = stx
            .world
            .manifest_aliases
            .get(&alias_id)
            .expect("alias record stored");
        assert!(alias_record.targets_manifest(&default_digest()));
        assert_eq!(alias_record.bound_epoch, 8);
        assert_eq!(alias_record.expiry_epoch, 16);
    }

    #[test]
    fn bind_manifest_alias_rejects_duplicates() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());
        register_and_approve_manifest(&mut stx, second_digest(), chunk_digest_for_seed(0xBB));

        let binding = sample_alias_binding();
        let first = BindManifestAlias {
            digest: default_digest(),
            binding: binding.clone(),
            bound_epoch: 8,
            expiry_epoch: 16,
        };
        first.execute(&alice(), &mut stx).expect("bind alias");

        let duplicate = BindManifestAlias {
            digest: second_digest(),
            binding: alias_binding_for(second_digest(), "sora", "docs", 9, 18),
            bound_epoch: 9,
            expiry_epoch: 18,
        };
        let err = duplicate
            .execute(&alice(), &mut stx)
            .expect_err("duplicate alias must fail");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(message.contains("alias"), "unexpected message: {message}");

        let first_record = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("first manifest remains stored");
        assert_eq!(first_record.alias.as_ref(), Some(&binding));
        let second_record = stx
            .world
            .pin_manifests
            .get(&second_digest())
            .expect("second manifest remains stored");
        assert!(
            second_record.alias.is_none(),
            "failed duplicate bind must not attach the contested alias to the second manifest"
        );
        let alias_record = stx
            .world
            .manifest_aliases
            .get(&ManifestAliasId::from(&binding))
            .expect("first alias record remains");
        assert!(alias_record.targets_manifest(&default_digest()));
    }

    #[test]
    fn bind_manifest_alias_rejects_expiry_before_bound_without_side_effects() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let binding = alias_binding_for(default_digest(), "sora", "docs", 8, 7);
        let err = BindManifestAlias {
            digest: default_digest(),
            binding: binding.clone(),
            bound_epoch: 8,
            expiry_epoch: 7,
        }
        .execute(&alice(), &mut stx)
        .expect_err("expiry before bound must fail");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("expiry epoch"),
            "unexpected error message: {message}"
        );

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest remains stored");
        assert!(stored.alias.is_none());
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&binding))
                .is_none(),
            "failed bind must not create an alias record"
        );
    }

    #[test]
    fn bind_manifest_alias_rejects_pending_manifest_without_side_effects() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        insert_pending_manifest(&mut stx, default_digest(), default_chunk_digest());

        let binding = sample_alias_binding();
        let err = BindManifestAlias {
            digest: default_digest(),
            binding: binding.clone(),
            bound_epoch: 8,
            expiry_epoch: 16,
        }
        .execute(&alice(), &mut stx)
        .expect_err("pending manifest must not accept alias binding");
        let message = match err {
            InstructionExecutionError::InvariantViolation(message) => message.to_string(),
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("must be approved before binding an alias"),
            "unexpected error message: {message}"
        );

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest remains stored");
        assert!(matches!(stored.status, PinStatus::Pending));
        assert!(stored.alias.is_none());
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&binding))
                .is_none(),
            "failed pending bind must not create an alias record"
        );
    }

    #[test]
    fn bind_manifest_alias_rejects_expiry_past_retention_without_side_effects() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let binding = alias_binding_for(default_digest(), "sora", "docs", 8, 43);
        let err = BindManifestAlias {
            digest: default_digest(),
            binding: binding.clone(),
            bound_epoch: 8,
            expiry_epoch: 43,
        }
        .execute(&alice(), &mut stx)
        .expect_err("alias expiry beyond retention must fail");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("exceeds manifest retention epoch"),
            "unexpected error message: {message}"
        );

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest remains stored");
        assert!(stored.alias.is_none());
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&binding))
                .is_none(),
            "failed retention bind must not create an alias record"
        );
    }

    #[test]
    fn bind_manifest_alias_rejects_bound_before_approval_without_side_effects() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let binding = alias_binding_for(default_digest(), "sora", "docs", 4, 16);
        let err = BindManifestAlias {
            digest: default_digest(),
            binding: binding.clone(),
            bound_epoch: 4,
            expiry_epoch: 16,
        }
        .execute(&alice(), &mut stx)
        .expect_err("alias bound before approval must fail");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("precedes manifest approval epoch"),
            "unexpected error message: {message}"
        );

        let stored = stx
            .world
            .pin_manifests
            .get(&default_digest())
            .expect("manifest remains stored");
        assert!(matches!(stored.status, PinStatus::Approved(5)));
        assert!(stored.alias.is_none());
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&binding))
                .is_none(),
            "failed early-bound bind must not create an alias record"
        );
    }

    #[test]
    fn bind_manifest_alias_rejects_unknown_manifest_without_side_effects() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let binding = sample_alias_binding();
        let err = BindManifestAlias {
            digest: default_digest(),
            binding: binding.clone(),
            bound_epoch: 8,
            expiry_epoch: 16,
        }
        .execute(&alice(), &mut stx)
        .expect_err("unknown manifest must not accept alias binding");
        let message = match err {
            InstructionExecutionError::InvariantViolation(message) => message.to_string(),
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("not registered"),
            "unexpected error message: {message}"
        );
        assert!(
            stx.world.pin_manifests.get(&default_digest()).is_none(),
            "failed unknown-manifest bind must not create a manifest"
        );
        assert!(
            stx.world
                .manifest_aliases
                .get(&ManifestAliasId::from(&binding))
                .is_none(),
            "failed unknown-manifest bind must not create an alias record"
        );
    }

    #[test]
    fn bind_manifest_alias_rejects_proof_epoch_mismatch() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let mismatched_binding = alias_binding_for(default_digest(), "sora", "docs", 4, 12);
        let bind = BindManifestAlias {
            digest: default_digest(),
            binding: mismatched_binding,
            bound_epoch: 8,
            expiry_epoch: 16,
        };

        let err = bind
            .execute(&alice(), &mut stx)
            .expect_err("binding must reject mismatched alias proof epochs");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("alias proof bound_at"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn issue_replication_order_requires_permission() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        remove_permission(&mut stx, "CanIssueSorafsReplicationOrder");

        let issue = IssueReplicationOrder {
            order_id: ReplicationOrderId::new([0x44; 32]),
            order_payload: Vec::new(),
            issued_epoch: 1,
            deadline_epoch: 2,
        };

        let err = issue
            .execute(&alice(), &mut stx)
            .expect_err("permissionless issue must fail");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("CanIssueSorafsReplicationOrder")
        ));
    }

    #[test]
    fn complete_replication_order_requires_permission() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        remove_permission(&mut stx, "CanCompleteSorafsReplicationOrder");

        let complete = CompleteReplicationOrder {
            order_id: ReplicationOrderId::new([0x55; 32]),
            completion_epoch: 5,
        };

        let err = complete
            .execute(&alice(), &mut stx)
            .expect_err("permissionless completion must fail");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("CanCompleteSorafsReplicationOrder")
        ));
    }

    #[test]
    fn pricing_schedule_requires_permission() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        remove_permission(&mut stx, "CanSetSorafsPricing");

        let schedule = PricingScheduleRecord::launch_default();
        let err = SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect_err("permissionless pricing must fail");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("CanSetSorafsPricing")
        ));
    }

    #[test]
    fn provider_credit_requires_permission() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        remove_permission(&mut stx, "CanUpsertSorafsProviderCredit");

        let credit = ProviderCreditRecord::new(
            ProviderId::new([0x77; 32]),
            xor_quantity_nanos(1),
            Quantity::zero(),
            Quantity::zero(),
            Quantity::zero(),
            1,
            1,
            Metadata::default(),
        );

        let err = UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect_err("permissionless credit update must fail");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("CanUpsertSorafsProviderCredit")
        ));
    }

    #[test]
    fn issue_replication_order_stores_record() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x44; 32]);
        let providers = vec![
            ProviderId::new([0x11; 32]),
            ProviderId::new([0x12; 32]),
            ProviderId::new([0x13; 32]),
        ];
        seed_provider_owners(&mut stx, &providers, &alice());
        let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        let payload = encode_replication_order(&order_struct);
        let issue = IssueReplicationOrder {
            order_id,
            order_payload: payload.clone(),
            issued_epoch: 12,
            deadline_epoch: 32,
        };
        issue
            .execute(&alice(), &mut stx)
            .expect("issue replication order");

        let record = stx
            .world
            .replication_orders
            .get(&order_id)
            .expect("order stored");
        assert_eq!(record.manifest_digest, default_digest());
        assert_eq!(record.issued_epoch, 12);
        assert_eq!(record.deadline_epoch, 32);
        assert_eq!(record.issued_by, alice());
        assert_eq!(record.canonical_order, payload);
        assert!(matches!(record.status, ReplicationOrderStatus::Pending));

        let decoded = norito::decode_from_bytes::<ReplicationOrderV1>(&record.canonical_order)
            .expect("decode order");
        assert_eq!(decoded.order_id, *order_id.as_bytes());
    }

    #[test]
    fn instruction_box_dispatches_replication_order_issue() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x54; 32]);
        let providers = vec![
            ProviderId::new([0x21; 32]),
            ProviderId::new([0x22; 32]),
            ProviderId::new([0x23; 32]),
        ];
        seed_provider_owners(&mut stx, &providers, &alice());
        let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        let payload = encode_replication_order(&order_struct);

        let instruction = InstructionBox::from(IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 12,
            deadline_epoch: 32,
        });
        instruction
            .execute(&alice(), &mut stx)
            .expect("instruction box should dispatch SoraFS replication orders");

        assert!(
            stx.world.replication_orders.get(&order_id).is_some(),
            "instruction box execution should store the replication order"
        );
    }

    #[test]
    fn issue_replication_order_rejects_duplicates() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x55; 32]);
        let providers = vec![
            ProviderId::new([0x21; 32]),
            ProviderId::new([0x22; 32]),
            ProviderId::new([0x23; 32]),
        ];
        seed_provider_owners(&mut stx, &providers, &alice());
        let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        let payload = encode_replication_order(&order_struct);
        let issue = IssueReplicationOrder {
            order_id,
            order_payload: payload.clone(),
            issued_epoch: 20,
            deadline_epoch: 40,
        };
        issue
            .execute(&alice(), &mut stx)
            .expect("issue replication order");

        let duplicate = IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 21,
            deadline_epoch: 41,
        };
        let err = duplicate
            .execute(&alice(), &mut stx)
            .expect_err("duplicate order must fail");
        assert!(matches!(
            err,
            InstructionExecutionError::InvariantViolation(_)
        ));
    }

    #[test]
    fn issue_replication_order_rejects_target_below_policy() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x66; 32]);
        let providers = vec![
            ProviderId::new([0x41; 32]),
            ProviderId::new([0x42; 32]),
            ProviderId::new([0x43; 32]),
        ];
        let mut order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        order_struct.target_replicas = 2;
        let payload = encode_replication_order(&order_struct);
        let issue = IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 10,
            deadline_epoch: 20,
        };
        let err = issue
            .execute(&alice(), &mut stx)
            .expect_err("target replicas below policy should fail");

        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("target replicas"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn issue_replication_order_rejects_chunker_mismatch() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x67; 32]);
        let providers = vec![
            ProviderId::new([0x51; 32]),
            ProviderId::new([0x52; 32]),
            ProviderId::new([0x53; 32]),
        ];
        let mut order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        order_struct.chunking_profile = "sorafs.sf2@2.0.0".to_owned();
        let payload = encode_replication_order(&order_struct);

        let issue = IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 15,
            deadline_epoch: 25,
        };
        let err = issue
            .execute(&alice(), &mut stx)
            .expect_err("chunking profile mismatch should fail");

        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("unknown chunker handle"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn issue_replication_order_rejects_noncanonical_unbound_and_oversized_payloads() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());
        let providers = vec![ProviderId::new([0x50; 32])];
        seed_provider_owners(&mut stx, &providers, &alice());

        let mismatch_id = ReplicationOrderId::new([0x68; 32]);
        let mut mismatch = replication_order_struct(mismatch_id, default_digest(), &providers, 1);
        mismatch.manifest_cid = vec![0x99; 32];
        let error = IssueReplicationOrder {
            order_id: mismatch_id,
            order_payload: encode_replication_order(&mismatch),
            issued_epoch: 5,
            deadline_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect_err("manifest CID must be bound to the digest");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("manifest CID must equal")
        ));

        let noncanonical_id = ReplicationOrderId::new([0x69; 32]);
        let noncanonical =
            replication_order_struct(noncanonical_id, default_digest(), &providers, 1);
        let noncanonical_bytes = {
            let _guard = norito::core::DecodeFlagsGuard::enter(0);
            norito::to_bytes(&noncanonical).expect("encode alternate-layout fixture")
        };
        let error = IssueReplicationOrder {
            order_id: noncanonical_id,
            order_payload: noncanonical_bytes,
            issued_epoch: 5,
            deadline_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect_err("alternate Norito layout must fail");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("canonical first-release Norito")
        ));

        let oversized_id = ReplicationOrderId::new([0x6A; 32]);
        let error = IssueReplicationOrder {
            order_id: oversized_id,
            order_payload: vec![0xA5; MAX_REPLICATION_ORDER_PAYLOAD_BYTES + 1],
            issued_epoch: 5,
            deadline_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect_err("oversized payload must fail before decode");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("expected 1..=")
        ));

        let allocation_bomb_id = ReplicationOrderId::new([0x6C; 32]);
        let mut allocation_bomb =
            replication_order_struct(allocation_bomb_id, default_digest(), &providers, 1);
        allocation_bomb.metadata.push(CapacityMetadataEntry {
            key: "bomb".to_owned(),
            value: "x".repeat(sorafs_manifest::capacity::MAX_CAPACITY_METADATA_VALUE_BYTES + 1),
        });
        let allocation_bomb = encode_replication_order(&allocation_bomb);
        assert!(allocation_bomb.len() <= MAX_REPLICATION_ORDER_PAYLOAD_BYTES);
        let error = IssueReplicationOrder {
            order_id: allocation_bomb_id,
            order_payload: allocation_bomb,
            issued_epoch: 5,
            deadline_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect_err("sequence allocation bomb must fail before semantic validation");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("invalid replication order payload")
        ));

        assert_eq!(stx.world.replication_orders.iter().count(), 0);
    }

    #[test]
    fn issue_replication_order_rejects_zero_length_epoch_window() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let error = IssueReplicationOrder {
            order_id: ReplicationOrderId::new([0x6B; 32]),
            order_payload: vec![1],
            issued_epoch: 5,
            deadline_epoch: 5,
        }
        .execute(&alice(), &mut stx)
        .expect_err("zero-length order epoch window must fail");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("must be greater than issued_epoch")
        ));
    }

    #[test]
    fn issue_replication_order_requires_permission_after_manifest_setup() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        remove_permission(&mut stx, "CanIssueSorafsReplicationOrder");

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x47; 32]);
        let providers = vec![ProviderId::new([0x26; 32])];
        seed_provider_owners(&mut stx, &providers, &alice());

        let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        let payload = encode_replication_order(&order_struct);
        let issue = IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 22,
            deadline_epoch: 33,
        };

        let err = issue
            .execute(&alice(), &mut stx)
            .expect_err("missing permission must reject order issue");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(_))
        ));
    }

    #[test]
    fn issue_replication_order_allows_registered_provider_owned_by_another_account() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x45; 32]);
        let providers = vec![ProviderId::new([0x24; 32])];
        seed_provider_owners(&mut stx, &providers, &bob());

        let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        let payload = encode_replication_order(&order_struct);
        let issue = IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 22,
            deadline_epoch: 33,
        };

        issue
            .execute(&alice(), &mut stx)
            .expect("permissioned governance issuer may assign another owner's provider");
        assert!(matches!(
            stx.world
                .replication_orders
                .get(&order_id)
                .expect("order stored")
                .status,
            ReplicationOrderStatus::Pending
        ));
    }

    #[test]
    fn issue_replication_order_rejects_missing_owner() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x46; 32]);
        let providers = vec![ProviderId::new([0x25; 32])];
        let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        let payload = encode_replication_order(&order_struct);
        let issue = IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 22,
            deadline_epoch: 33,
        };

        let err = issue
            .execute(&alice(), &mut stx)
            .expect_err("missing owner must reject order issue");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(_))
        ));
    }

    #[test]
    fn complete_replication_order_updates_status() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x77; 32]);
        let providers = vec![
            ProviderId::new([0x31; 32]),
            ProviderId::new([0x32; 32]),
            ProviderId::new([0x33; 32]),
        ];
        seed_provider_owners(&mut stx, &providers, &alice());
        let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        let payload = encode_replication_order(&order_struct);
        IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 30,
            deadline_epoch: 60,
        }
        .execute(&alice(), &mut stx)
        .expect("issue replication order");

        let complete = CompleteReplicationOrder {
            order_id,
            completion_epoch: 45,
        };
        complete
            .execute(&alice(), &mut stx)
            .expect("complete replication order");
        CompleteReplicationOrder {
            order_id,
            completion_epoch: 45,
        }
        .execute(&alice(), &mut stx)
        .expect("exact completion replay is idempotent");
        let conflicting_replay = CompleteReplicationOrder {
            order_id,
            completion_epoch: 46,
        }
        .execute(&alice(), &mut stx)
        .expect_err("completion replay at a different epoch must fail");
        assert!(matches!(
            conflicting_replay,
            InstructionExecutionError::InvariantViolation(message)
                if message.contains("already completed at epoch 45")
        ));

        let record = stx
            .world
            .replication_orders
            .get(&order_id)
            .expect("order stored");
        assert!(matches!(
            record.status,
            ReplicationOrderStatus::Completed(epoch) if epoch == 45
        ));
    }

    #[test]
    fn completion_after_deadline_fails_without_changing_pending_order() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x76; 32]);
        let providers = vec![ProviderId::new([0x30; 32])];
        seed_provider_owners(&mut stx, &providers, &alice());
        let payload = encode_replication_order(&replication_order_struct(
            order_id,
            default_digest(),
            &providers,
            1,
        ));
        IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 5,
            deadline_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect("issue order");

        let error = CompleteReplicationOrder {
            order_id,
            completion_epoch: 16,
        }
        .execute(&alice(), &mut stx)
        .expect_err("late completion must fail");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("exceeds deadline_epoch")
        ));
        assert_eq!(
            stx.world
                .replication_orders
                .get(&order_id)
                .expect("order remains")
                .status,
            ReplicationOrderStatus::Pending
        );
    }

    #[test]
    fn expire_replication_order_is_deadline_bound_and_idempotent() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x74; 32]);
        let providers = vec![ProviderId::new([0x2E; 32])];
        seed_provider_owners(&mut stx, &providers, &alice());
        let payload = encode_replication_order(&replication_order_struct(
            order_id,
            default_digest(),
            &providers,
            1,
        ));
        IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 5,
            deadline_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect("issue order");

        let early = ExpireReplicationOrder {
            order_id,
            expiration_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect_err("deadline remains completion-eligible");
        assert!(matches!(
            early,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("must be greater than deadline_epoch")
        ));
        assert_eq!(
            stx.world
                .replication_orders
                .get(&order_id)
                .expect("order remains pending")
                .status,
            ReplicationOrderStatus::Pending
        );

        ExpireReplicationOrder {
            order_id,
            expiration_epoch: 16,
        }
        .execute(&alice(), &mut stx)
        .expect("expire after deadline");
        ExpireReplicationOrder {
            order_id,
            expiration_epoch: 16,
        }
        .execute(&alice(), &mut stx)
        .expect("exact expiration replay is idempotent");
        let conflict = ExpireReplicationOrder {
            order_id,
            expiration_epoch: 17,
        }
        .execute(&alice(), &mut stx)
        .expect_err("conflicting expiration replay must fail");
        assert!(matches!(
            conflict,
            InstructionExecutionError::InvariantViolation(message)
                if message.contains("already expired at epoch 16")
        ));
        assert_eq!(
            stx.world
                .replication_orders
                .get(&order_id)
                .expect("expired order retained")
                .status,
            ReplicationOrderStatus::Expired(16)
        );
        let completion = CompleteReplicationOrder {
            order_id,
            completion_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect_err("expired order cannot complete");
        assert!(matches!(
            completion,
            InstructionExecutionError::InvariantViolation(message)
                if message.contains("expired at epoch 16")
        ));
    }

    #[test]
    fn expire_replication_order_rejects_completed_order_and_missing_permission() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x73; 32]);
        let providers = vec![ProviderId::new([0x2D; 32])];
        seed_provider_owners(&mut stx, &providers, &alice());
        let payload = encode_replication_order(&replication_order_struct(
            order_id,
            default_digest(),
            &providers,
            1,
        ));
        IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 5,
            deadline_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect("issue order");

        remove_permission(&mut stx, "CanIssueSorafsReplicationOrder");
        let denied = ExpireReplicationOrder {
            order_id,
            expiration_epoch: 16,
        }
        .execute(&alice(), &mut stx)
        .expect_err("expiration requires governance issue permission");
        assert!(matches!(
            denied,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(_))
        ));
        grant_permission(&mut stx, "CanIssueSorafsReplicationOrder");
        CompleteReplicationOrder {
            order_id,
            completion_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect("complete at deadline");
        let completed = ExpireReplicationOrder {
            order_id,
            expiration_epoch: 16,
        }
        .execute(&alice(), &mut stx)
        .expect_err("completed order cannot expire");
        assert!(matches!(
            completed,
            InstructionExecutionError::InvariantViolation(message)
                if message.contains("completed at epoch 15")
        ));
    }

    #[test]
    fn retiring_manifest_expires_its_pending_replication_orders() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x75; 32]);
        let providers = vec![ProviderId::new([0x2F; 32])];
        seed_provider_owners(&mut stx, &providers, &alice());
        let payload = encode_replication_order(&replication_order_struct(
            order_id,
            default_digest(),
            &providers,
            1,
        ));
        IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 5,
            deadline_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect("issue order");

        RetirePinManifest {
            digest: default_digest(),
            retired_epoch: 10,
            reason: Some("superseded".to_owned()),
        }
        .execute(&alice(), &mut stx)
        .expect("retire manifest");
        assert_eq!(
            stx.world
                .replication_orders
                .get(&order_id)
                .expect("order remains auditable")
                .status,
            ReplicationOrderStatus::Expired(10)
        );
        let error = CompleteReplicationOrder {
            order_id,
            completion_epoch: 10,
        }
        .execute(&alice(), &mut stx)
        .expect_err("expired order must not complete");
        assert!(matches!(
            error,
            InstructionExecutionError::InvariantViolation(message)
                if message.contains("expired at epoch 10")
        ));
    }

    #[test]
    fn complete_replication_order_requires_permission_after_manifest_setup() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x7A; 32]);
        let providers = vec![
            ProviderId::new([0x36; 32]),
            ProviderId::new([0x37; 32]),
            ProviderId::new([0x38; 32]),
        ];
        seed_provider_owners(&mut stx, &providers, &alice());
        let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        let payload = encode_replication_order(&order_struct);
        IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 30,
            deadline_epoch: 60,
        }
        .execute(&alice(), &mut stx)
        .expect("issue replication order");

        remove_permission(&mut stx, "CanCompleteSorafsReplicationOrder");

        let complete = CompleteReplicationOrder {
            order_id,
            completion_epoch: 45,
        };
        let err = complete
            .execute(&alice(), &mut stx)
            .expect_err("missing permission should reject completion");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(_))
        ));
    }

    #[test]
    fn complete_replication_order_allows_permissioned_non_owner_governance() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x78; 32]);
        let providers = vec![
            ProviderId::new([0x34; 32]),
            ProviderId::new([0x44; 32]),
            ProviderId::new([0x54; 32]),
        ];
        seed_provider_owners(&mut stx, &providers, &alice());
        let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        let payload = encode_replication_order(&order_struct);
        IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 5,
            deadline_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect("issue replication order");

        let complete = CompleteReplicationOrder {
            order_id,
            completion_epoch: 10,
        };
        complete.execute(&bob(), &mut stx).expect(
            "permissioned governance completion must not require one owner for all providers",
        );
        assert!(matches!(
            stx.world
                .replication_orders
                .get(&order_id)
                .expect("order stored")
                .status,
            ReplicationOrderStatus::Completed(10)
        ));
    }

    #[test]
    fn complete_replication_order_allows_current_owner_after_transfer() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        register_and_approve_manifest(&mut stx, default_digest(), default_chunk_digest());

        let order_id = ReplicationOrderId::new([0x79; 32]);
        let providers = vec![
            ProviderId::new([0x35; 32]),
            ProviderId::new([0x45; 32]),
            ProviderId::new([0x55; 32]),
        ];
        seed_provider_owners(&mut stx, &providers, &alice());
        let order_struct = replication_order_struct(order_id, default_digest(), &providers, 3);
        let payload = encode_replication_order(&order_struct);
        IssueReplicationOrder {
            order_id,
            order_payload: payload,
            issued_epoch: 5,
            deadline_epoch: 15,
        }
        .execute(&alice(), &mut stx)
        .expect("issue replication order");

        for provider in &providers {
            stx.world.provider_owners.insert(*provider, bob().clone());
        }

        let complete = CompleteReplicationOrder {
            order_id,
            completion_epoch: 12,
        };
        complete
            .execute(&bob(), &mut stx)
            .expect("current owner should complete order");
    }

    #[test]
    fn register_capacity_declaration_inserts_record() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, record) = sample_capacity_record();
        let instruction = RegisterCapacityDeclaration {
            record: record.clone(),
        };

        instruction
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let stored = stx
            .world
            .capacity_declarations
            .get(&provider)
            .expect("capacity declaration stored");
        assert_eq!(stored.committed_capacity_gib, record.committed_capacity_gib);

        let ledger = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("capacity ledger updated");
        assert_eq!(
            ledger.total_declared_gib,
            u128::from(record.committed_capacity_gib)
        );
    }

    #[test]
    fn capacity_declaration_accepts_i105_owner_literal_without_registry() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let mut declaration = sample_capacity_declaration();
        declaration.metadata = vec![CapacityMetadataEntry {
            key: PROVIDER_OWNER_METADATA_KEY.to_owned(),
            value: alice().to_string(),
        }];
        let provider = ProviderId::new(declaration.provider_id);
        let record = CapacityDeclarationRecord::new(
            provider,
            norito::to_bytes(&declaration).expect("serialize declaration"),
            declaration.committed_capacity_gib,
            9,
            10,
            20,
            Metadata::default(),
        );

        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register declaration with I105 owner");

        assert!(stx.world.capacity_declarations.get(&provider).is_some());
    }

    #[test]
    fn record_capacity_telemetry_updates_pricing_and_credit() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.tiers = vec![TierRate::new(
            StorageClass::Hot,
            xor_quantity_nanos(1_000_000_000),
            xor_quantity_nanos(1_000_000),
        )];
        schedule.credit = CreditPolicy {
            settlement_window_secs: SECONDS_PER_BILLING_MONTH,
            settlement_grace_secs: 0,
            low_balance_alert_bps: 2_000,
        };
        schedule.collateral = CollateralPolicy {
            multiplier_bps: 30_000,
            onboarding_discount_bps: 5_000,
            onboarding_period_secs: SECONDS_PER_BILLING_MONTH,
        };
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        let credit = ProviderCreditRecord::new(
            provider,
            xor_quantity_nanos(15_000_000_000),
            Quantity::zero(),
            Quantity::zero(),
            Quantity::zero(),
            0,
            0,
            Metadata::default(),
        );
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let telemetry = CapacityTelemetryRecord::new(
            provider,
            0,
            SECONDS_PER_BILLING_MONTH,
            75,
            75,
            10,
            1,
            1,
            10_000,
            10_000,
            0,
            0,
            0,
            0,
            0,
        )
        .with_nonce(SECONDS_PER_BILLING_MONTH);
        RecordCapacityTelemetry { record: telemetry }
            .execute(&alice(), &mut stx)
            .expect("record capacity telemetry");

        let ledger = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("capacity fee ledger stored");
        assert_eq!(ledger.storage_fee, xor_quantity_nanos(10_000_000_000));
        assert_eq!(ledger.egress_fee, Quantity::zero());
        assert_eq!(ledger.accrued_fee, xor_quantity_nanos(10_000_000_000));
        assert_eq!(
            ledger.expected_settlement,
            xor_quantity_nanos(10_000_000_000)
        );

        let credit_after = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit ledger stored");
        assert_eq!(
            credit_after.available_credit,
            xor_quantity_nanos(5_000_000_000)
        );
        assert_eq!(
            credit_after.expected_settlement,
            xor_quantity_nanos(10_000_000_000)
        );
        assert_eq!(
            credit_after.required_bond,
            xor_quantity_nanos(30_000_000_000)
        );
        assert_eq!(credit_after.low_balance_since_epoch, None);
    }

    #[test]
    fn record_capacity_telemetry_caps_credit_debit_at_zero() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.tiers = vec![TierRate::new(
            StorageClass::Hot,
            xor_quantity_nanos(1_000_000_000),
            xor_quantity_nanos(1),
        )];
        schedule.credit.settlement_window_secs = SECONDS_PER_BILLING_MONTH;
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        UpsertProviderCredit {
            record: provider_credit_nanos(provider, 1, 0),
        }
        .execute(&alice(), &mut stx)
        .expect("seed one-nano provider credit");

        let telemetry = CapacityTelemetryRecord::new(
            provider,
            0,
            SECONDS_PER_BILLING_MONTH,
            75,
            75,
            1,
            0,
            0,
            10_000,
            10_000,
            0,
            0,
            0,
            0,
            0,
        )
        .with_nonce(SECONDS_PER_BILLING_MONTH);
        RecordCapacityTelemetry { record: telemetry }
            .execute(&alice(), &mut stx)
            .expect("debit larger than credit is capped without underflow");

        let credit = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("provider credit stored");
        assert_eq!(credit.available_credit, Quantity::zero());
        let ledger = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("capacity ledger stored");
        assert_eq!(ledger.storage_fee, xor_quantity_nanos(1_000_000_000));
    }

    #[test]
    fn record_capacity_telemetry_rejects_quantity_overflow_without_partial_mutation() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.tiers = vec![TierRate::new(
            StorageClass::Hot,
            max_positive_quantity(),
            xor_quantity_nanos(1),
        )];
        schedule.credit.settlement_window_secs = SECONDS_PER_BILLING_MONTH;
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("maximum bounded price is structurally valid");

        UpsertProviderCredit {
            record: provider_credit_nanos(provider, 10_000_000_000, 0),
        }
        .execute(&alice(), &mut stx)
        .expect("seed provider credit");
        let ledger_before = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .cloned()
            .expect("capacity ledger exists");
        let credit_before = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .cloned()
            .expect("provider credit exists");

        let telemetry = CapacityTelemetryRecord::new(
            provider,
            0,
            SECONDS_PER_BILLING_MONTH,
            75,
            75,
            10,
            0,
            0,
            10_000,
            10_000,
            0,
            0,
            0,
            0,
            0,
        )
        .with_nonce(SECONDS_PER_BILLING_MONTH);
        let error = RecordCapacityTelemetry { record: telemetry }
            .execute(&alice(), &mut stx)
            .expect_err("overflowing storage charge must be rejected");
        assert!(matches!(
            error,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("capacity storage fee")
        ));
        assert_eq!(
            stx.world.capacity_fee_ledger.get(&provider),
            Some(&ledger_before),
            "rejected arithmetic must not mutate capacity accounting"
        );
        assert_eq!(
            stx.world.provider_credit_ledger.get(&provider),
            Some(&credit_before),
            "rejected arithmetic must not mutate provider credit"
        );
    }

    #[test]
    fn record_capacity_telemetry_rejects_overcommit_and_zero_capacity() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("register capacity declaration");

        let oversized = CapacityTelemetryRecord::new(
            provider, 0, 10, 2_048, 2_048, 2_048, 1, 1, 10_000, 10_000, 1, 0, 0, 0, 0,
        )
        .with_nonce(10);
        let err = RecordCapacityTelemetry { record: oversized }
            .execute(&alice(), &mut stx)
            .expect_err("telemetry exceeding committed capacity must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message
            )) if message.contains("capacity_exceeds_commitment")
        ));

        let zero_capacity = CapacityTelemetryRecord::new(
            provider, 10, 20, 10, 10, 0, 1, 1, 10_000, 10_000, 1, 0, 0, 0, 0,
        )
        .with_nonce(20);
        let err = RecordCapacityTelemetry {
            record: zero_capacity,
        }
        .execute(&alice(), &mut stx)
        .expect_err("telemetry with zero capacity must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message
            )) if message.contains("zero_capacity_window")
        ));
    }

    #[test]
    fn record_capacity_telemetry_rejects_overlap_gap_and_replay() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("register capacity declaration");

        record_capacity_window(&mut stx, provider, 0, 10, 50, 50, 25, 9_500, 9_500, 0);

        let overlap = CapacityTelemetryRecord::new(
            provider, 5, 12, 50, 50, 25, 1, 1, 9_500, 9_500, 0, 0, 0, 0, 0,
        )
        .with_nonce(12);
        let err = RecordCapacityTelemetry { record: overlap }
            .execute(&alice(), &mut stx)
            .expect_err("overlapping telemetry must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message
            )) if message.contains("overlapping_window")
        ));

        let replay = CapacityTelemetryRecord::new(
            provider, 11, 18, 50, 50, 25, 1, 1, 9_500, 9_500, 0, 0, 0, 0, 0,
        )
        .with_nonce(10);
        let err = RecordCapacityTelemetry { record: replay }
            .execute(&alice(), &mut stx)
            .expect_err("replayed nonce must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message
            )) if message.contains("replayed_nonce")
        ));

        stx.gov.sorafs_telemetry.max_window_gap = core::time::Duration::from_secs(2);
        let gap = CapacityTelemetryRecord::new(
            provider, 30, 35, 50, 50, 25, 1, 1, 9_500, 9_500, 0, 0, 0, 0, 0,
        )
        .with_nonce(35);
        let err = RecordCapacityTelemetry { record: gap }
            .execute(&alice(), &mut stx)
            .expect_err("gap beyond policy must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message
            )) if message.contains("window_gap_exceeded")
        ));
    }

    #[test]
    fn record_capacity_telemetry_rejects_replay_when_nonce_optional() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("register capacity declaration");

        stx.gov.sorafs_telemetry.require_nonce = false;
        record_capacity_window(&mut stx, provider, 0, 10, 50, 50, 25, 9_500, 9_500, 0);

        let replay = CapacityTelemetryRecord::new(
            provider, 11, 18, 50, 50, 25, 1, 1, 9_500, 9_500, 0, 0, 0, 0, 0,
        )
        .with_nonce(10);
        let err = RecordCapacityTelemetry { record: replay }
            .execute(&alice(), &mut stx)
            .expect_err("replayed nonce must be rejected even when optional");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message
            )) if message.contains("replayed_nonce")
        ));
    }

    #[test]
    fn record_capacity_telemetry_requires_authorised_submitter() {
        let mut state = make_state();
        let bob = AccountId::new(
            "ed01208B6BD94034D1145C0B149DB43A07F56977AF58C1871F43B6D54A4D3F33D5B451"
                .parse()
                .expect("public key"),
        );
        seed_sorafs_permissions(&mut state, &bob);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("register capacity declaration");

        stx.gov.sorafs_telemetry.submitters = vec![alice()];
        stx.gov.sorafs_telemetry.require_submitter = true;
        let telemetry = CapacityTelemetryRecord::new(
            provider, 0, 10, 50, 50, 25, 1, 1, 10_000, 10_000, 0, 0, 0, 0, 0,
        )
        .with_nonce(10);
        let err = RecordCapacityTelemetry { record: telemetry }
            .execute(&bob, &mut stx)
            .expect_err("unauthorised submitter must be rejected");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("unauthorised_submitter"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn record_capacity_telemetry_provider_override_allows_owner_when_global_rejects() {
        let mut state = make_state();
        seed_sorafs_permissions(&mut state, &bob());
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let (provider, declaration) = capacity_record_with_owner(&bob());
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&bob(), &mut stx)
        .expect("register capacity declaration");

        stx.gov.sorafs_telemetry.require_submitter = true;
        stx.gov.sorafs_telemetry.submitters = vec![alice()];
        stx.gov
            .sorafs_telemetry
            .per_provider_submitters
            .insert(provider, vec![bob()]);

        let telemetry = CapacityTelemetryRecord::new(
            provider, 0, 10, 50, 50, 25, 1, 1, 10_000, 10_000, 0, 0, 0, 0, 0,
        )
        .with_nonce(10);
        RecordCapacityTelemetry { record: telemetry }
            .execute(&bob(), &mut stx)
            .expect("per-provider allow-list should allow owner");
    }

    #[test]
    fn record_capacity_telemetry_provider_override_blocks_owner_not_listed() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let (provider, declaration) = sample_capacity_record();
        RegisterCapacityDeclaration {
            record: declaration,
        }
        .execute(&alice(), &mut stx)
        .expect("register capacity declaration");

        stx.gov.sorafs_telemetry.require_submitter = true;
        stx.gov.sorafs_telemetry.submitters = vec![alice()];
        stx.gov
            .sorafs_telemetry
            .per_provider_submitters
            .insert(provider, vec![bob()]);

        let telemetry = CapacityTelemetryRecord::new(
            provider, 0, 10, 50, 50, 25, 1, 1, 10_000, 10_000, 0, 0, 0, 0, 0,
        )
        .with_nonce(10);
        let err = RecordCapacityTelemetry { record: telemetry }
            .execute(&alice(), &mut stx)
            .expect_err("owner not in provider allow-list must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message
            )) if message.contains("unauthorised_submitter_provider")
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn record_capacity_telemetry_penalises_persistent_under_delivery() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        stx.gov.sorafs_penalty = iroha_config::parameters::actual::SorafsPenaltyPolicy {
            utilisation_floor_bps: 9_500,
            uptime_floor_bps: 9_500,
            por_success_floor_bps: 9_500,
            strike_threshold: 2,
            penalty_bond_bps: 5_000,
            cooldown_windows: 2,
            max_pdp_failures: 0,
            max_potr_breaches: 0,
        };

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.credit = CreditPolicy {
            settlement_window_secs: SECONDS_PER_BILLING_MONTH,
            settlement_grace_secs: 0,
            low_balance_alert_bps: 2_000,
        };
        schedule.collateral = CollateralPolicy {
            multiplier_bps: 30_000,
            onboarding_discount_bps: 5_000,
            onboarding_period_secs: SECONDS_PER_BILLING_MONTH,
        };
        SetPricingSchedule {
            schedule: schedule.clone(),
        }
        .execute(&alice(), &mut stx)
        .expect("set pricing schedule");

        let credit = ProviderCreditRecord::new(
            provider,
            xor_quantity_nanos(25_000_000_000),
            xor_quantity_nanos(8_000_000_000),
            Quantity::zero(),
            Quantity::zero(),
            0,
            0,
            Metadata::default(),
        );
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let window = SECONDS_PER_BILLING_MONTH;

        record_capacity_window(
            &mut stx, provider, 0, window, 100, 90, 40, 7_500, 7_400, 128,
        );
        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        assert_eq!(credit_snapshot.under_delivery_strikes, 1);
        assert_eq!(credit_snapshot.slashed, Quantity::zero());
        assert_eq!(credit_snapshot.last_penalty_epoch, None);
        let ledger_snapshot = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("ledger snapshot");
        assert_eq!(ledger_snapshot.penalty_events, 0);
        assert_eq!(ledger_snapshot.penalty_slashed, Quantity::zero());

        record_capacity_window(
            &mut stx,
            provider,
            window,
            window * 2,
            100,
            90,
            40,
            7_500,
            7_400,
            128,
        );
        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        let first_penalty = credit_snapshot.slashed.clone();
        assert!(!first_penalty.is_zero(), "penalty must slash collateral");
        assert_eq!(credit_snapshot.under_delivery_strikes, 0);
        assert_eq!(credit_snapshot.last_penalty_epoch, Some(window * 2));
        assert_eq!(credit_snapshot.bonded, xor_quantity_nanos(4_000_000_000));
        let ledger_snapshot = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("ledger snapshot");
        assert_eq!(ledger_snapshot.penalty_events, 1);
        assert_eq!(ledger_snapshot.penalty_slashed, first_penalty);

        record_capacity_window(
            &mut stx,
            provider,
            window * 2,
            window * 3,
            100,
            90,
            40,
            7_500,
            7_400,
            128,
        );
        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        assert_eq!(credit_snapshot.slashed, first_penalty);
        assert_eq!(credit_snapshot.under_delivery_strikes, 1);
        assert_eq!(credit_snapshot.last_penalty_epoch, Some(window * 2));
        let ledger_snapshot = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("ledger snapshot");
        assert_eq!(ledger_snapshot.penalty_events, 1);
        assert_eq!(ledger_snapshot.penalty_slashed, first_penalty);

        record_capacity_window(
            &mut stx,
            provider,
            window * 3,
            window * 4,
            100,
            90,
            40,
            7_500,
            7_400,
            128,
        );
        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        assert_eq!(credit_snapshot.bonded, xor_quantity_nanos(2_000_000_000));
        assert_eq!(credit_snapshot.under_delivery_strikes, 0);
        assert_eq!(credit_snapshot.last_penalty_epoch, Some(window * 4));
        let total_slashed = credit_snapshot.slashed.clone();
        assert!(
            total_slashed > first_penalty,
            "penalty total should increase after second slash"
        );
        assert_eq!(total_slashed, xor_quantity_nanos(6_000_000_000));
        let ledger_snapshot = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("ledger snapshot");
        assert_eq!(ledger_snapshot.penalty_events, 2);
        assert_eq!(ledger_snapshot.penalty_slashed, total_slashed);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn record_capacity_telemetry_respects_cooldown_between_penalties() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        stx.gov.sorafs_penalty = iroha_config::parameters::actual::SorafsPenaltyPolicy {
            utilisation_floor_bps: 9_500,
            uptime_floor_bps: 9_500,
            por_success_floor_bps: 9_500,
            strike_threshold: 1,
            penalty_bond_bps: 5_000,
            cooldown_windows: 2,
            max_pdp_failures: u32::MAX,
            max_potr_breaches: u32::MAX,
        };

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.credit = CreditPolicy {
            settlement_window_secs: 10,
            settlement_grace_secs: 0,
            low_balance_alert_bps: 1_000,
        };
        let settlement_window_secs = schedule.credit.settlement_window_secs;
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        let bonded = xor_quantity_nanos(8_000_000_000);
        let credit = ProviderCreditRecord::new(
            provider,
            xor_quantity_nanos(10_000_000_000),
            bonded.clone(),
            Quantity::zero(),
            Quantity::zero(),
            0,
            0,
            Metadata::default(),
        );
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let penalty_policy = stx.gov.sorafs_penalty;
        let cooldown_secs = penalty_policy.cooldown_window_secs(settlement_window_secs);
        let expected_first_penalty =
            round_xor_quantity_ratio(&bonded, u128::from(penalty_policy.penalty_bond_bps), 10_000)
                .expect("expected first penalty");

        record_capacity_window(
            &mut stx,
            provider,
            0,
            settlement_window_secs,
            100,
            90,
            40,
            7_500,
            7_400,
            0,
        );
        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        let ledger_snapshot = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("ledger snapshot");
        assert_eq!(ledger_snapshot.penalty_events, 1);
        assert_eq!(ledger_snapshot.penalty_slashed, expected_first_penalty);
        assert_eq!(credit_snapshot.slashed, expected_first_penalty);
        assert_eq!(
            credit_snapshot.last_penalty_epoch,
            Some(settlement_window_secs)
        );
        let bonded_after_first = bonded
            .checked_sub(&expected_first_penalty)
            .expect("bond covers first penalty");

        record_capacity_window(
            &mut stx,
            provider,
            settlement_window_secs,
            settlement_window_secs * 2,
            100,
            90,
            40,
            7_500,
            7_400,
            0,
        );
        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        let ledger_snapshot = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("ledger snapshot");
        assert_eq!(ledger_snapshot.penalty_events, 1);
        assert_eq!(ledger_snapshot.penalty_slashed, expected_first_penalty);
        assert_eq!(credit_snapshot.slashed, expected_first_penalty);
        assert_eq!(
            credit_snapshot.last_penalty_epoch,
            Some(settlement_window_secs)
        );
        assert_eq!(credit_snapshot.under_delivery_strikes, 1);

        record_capacity_window(
            &mut stx,
            provider,
            settlement_window_secs * 2,
            settlement_window_secs * 3,
            100,
            90,
            40,
            7_500,
            7_400,
            0,
        );
        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        let ledger_snapshot = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("ledger snapshot");
        let expected_second_penalty = round_xor_quantity_ratio(
            &bonded_after_first,
            u128::from(penalty_policy.penalty_bond_bps),
            10_000,
        )
        .expect("expected second penalty");
        let expected_total_penalty = expected_first_penalty
            .checked_add(&expected_second_penalty)
            .expect("expected cumulative penalty");
        assert!(
            settlement_window_secs
                .checked_add(cooldown_secs)
                .expect("settlement plus cooldown")
                <= settlement_window_secs * 3,
            "third window must fall outside cooldown"
        );
        assert_eq!(ledger_snapshot.penalty_events, 2);
        assert_eq!(ledger_snapshot.penalty_slashed, expected_total_penalty);
        assert_eq!(credit_snapshot.slashed, expected_total_penalty);
        assert_eq!(
            credit_snapshot.last_penalty_epoch,
            Some(settlement_window_secs * 3)
        );
        assert_eq!(
            credit_snapshot.bonded,
            bonded
                .checked_sub(&expected_total_penalty)
                .expect("bond covers cumulative penalty")
        );
        assert_eq!(credit_snapshot.under_delivery_strikes, 0);
    }

    #[test]
    fn record_capacity_telemetry_forces_penalty_on_pdp_failure() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        stx.gov.sorafs_penalty = iroha_config::parameters::actual::SorafsPenaltyPolicy {
            utilisation_floor_bps: 7_500,
            uptime_floor_bps: 9_000,
            por_success_floor_bps: 9_000,
            strike_threshold: 3,
            penalty_bond_bps: 5_000,
            cooldown_windows: 1,
            max_pdp_failures: 0,
            max_potr_breaches: 0,
        };

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.credit = CreditPolicy {
            settlement_window_secs: SECONDS_PER_BILLING_MONTH,
            settlement_grace_secs: 0,
            low_balance_alert_bps: 1_000,
        };
        schedule.collateral = CollateralPolicy {
            multiplier_bps: 20_000,
            onboarding_discount_bps: 1,
            onboarding_period_secs: 0,
        };
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        let credit = ProviderCreditRecord::new(
            provider,
            xor_quantity_nanos(10_000_000_000),
            xor_quantity_nanos(6_000_000_000),
            Quantity::zero(),
            Quantity::zero(),
            0,
            0,
            Metadata::default(),
        );
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let window = SECONDS_PER_BILLING_MONTH;
        record_capacity_window_with_proofs(
            &mut stx,
            provider,
            0,
            window,
            100,
            100,
            80,
            10_000,
            10_000,
            0,
            ProofWindowCounters {
                pdp_challenges: 5,
                pdp_failures: 1,
                ..ProofWindowCounters::default()
            },
        );

        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        assert!(
            !credit_snapshot.slashed.is_zero(),
            "PDP failure should slash collateral immediately"
        );
        assert_eq!(credit_snapshot.under_delivery_strikes, 0);
        assert_eq!(credit_snapshot.last_penalty_epoch, Some(window));

        let ledger_snapshot = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("ledger snapshot");
        assert_eq!(ledger_snapshot.penalty_events, 1);
        assert_eq!(ledger_snapshot.penalty_slashed, credit_snapshot.slashed);
    }

    #[test]
    fn record_capacity_telemetry_penalises_pdp_failures_without_challenge_count() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        stx.gov.sorafs_penalty = iroha_config::parameters::actual::SorafsPenaltyPolicy {
            utilisation_floor_bps: 7_500,
            uptime_floor_bps: 9_000,
            por_success_floor_bps: 9_000,
            strike_threshold: 3,
            penalty_bond_bps: 5_000,
            cooldown_windows: 1,
            max_pdp_failures: 0,
            max_potr_breaches: 0,
        };

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.credit = CreditPolicy {
            settlement_window_secs: SECONDS_PER_BILLING_MONTH,
            settlement_grace_secs: 0,
            low_balance_alert_bps: 1_000,
        };
        schedule.collateral = CollateralPolicy {
            multiplier_bps: 20_000,
            onboarding_discount_bps: 1,
            onboarding_period_secs: 0,
        };
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        let credit = ProviderCreditRecord::new(
            provider,
            xor_quantity_nanos(10_000_000_000),
            xor_quantity_nanos(6_000_000_000),
            Quantity::zero(),
            Quantity::zero(),
            0,
            0,
            Metadata::default(),
        );
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let window = SECONDS_PER_BILLING_MONTH;
        record_capacity_window_with_proofs(
            &mut stx,
            provider,
            0,
            window,
            100,
            100,
            80,
            10_000,
            10_000,
            0,
            ProofWindowCounters {
                pdp_challenges: 0,
                pdp_failures: 1,
                ..ProofWindowCounters::default()
            },
        );

        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        assert!(
            !credit_snapshot.slashed.is_zero(),
            "PDP failure without challenges should still slash collateral"
        );
        assert_eq!(credit_snapshot.last_penalty_epoch, Some(window));
        let ledger_snapshot = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("ledger snapshot");
        assert_eq!(ledger_snapshot.penalty_events, 1);
        assert_eq!(ledger_snapshot.penalty_slashed, credit_snapshot.slashed);
    }

    #[test]
    fn record_capacity_telemetry_records_proof_failure_dispute() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        stx.gov.sorafs_penalty = iroha_config::parameters::actual::SorafsPenaltyPolicy {
            utilisation_floor_bps: 7_500,
            uptime_floor_bps: 9_000,
            por_success_floor_bps: 9_000,
            strike_threshold: 3,
            penalty_bond_bps: 5_000,
            cooldown_windows: 1,
            max_pdp_failures: 0,
            max_potr_breaches: 0,
        };

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.credit = CreditPolicy {
            settlement_window_secs: SECONDS_PER_BILLING_MONTH,
            settlement_grace_secs: 0,
            low_balance_alert_bps: 1_000,
        };
        schedule.collateral = CollateralPolicy {
            multiplier_bps: 20_000,
            onboarding_discount_bps: 1,
            onboarding_period_secs: 0,
        };
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        let credit = provider_credit_nanos(provider, 10_000_000_000, 6_000_000_000);
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let window = SECONDS_PER_BILLING_MONTH;
        let proof = ProofWindowCounters {
            pdp_challenges: 5,
            pdp_failures: 2,
            potr_windows: 0,
            potr_breaches: 0,
        };
        record_capacity_window_with_proofs(
            &mut stx, provider, 0, window, 100, 90, 40, 7_500, 7_400, 128, proof,
        );

        let disputes: Vec<_> = stx.world.capacity_disputes.iter().collect();
        assert_eq!(disputes.len(), 1, "expected proof failure dispute");
        let (_id, dispute) = &disputes[0];
        assert_eq!(dispute.provider_id, provider);
        assert_eq!(dispute.kind, CapacityDisputeKind::ProofFailure as u8);
        assert!(
            dispute.description.contains("PDP probes failed"),
            "description must mention PDP failures"
        );

        let expected_telemetry = CapacityTelemetryRecord::new(
            provider,
            0,
            window,
            100,
            90,
            40,
            1,
            1,
            7_500,
            7_400,
            128,
            proof.pdp_challenges,
            proof.pdp_failures,
            proof.potr_windows,
            proof.potr_breaches,
        )
        .with_nonce(window);
        let telemetry_bytes =
            norito::to_bytes(&expected_telemetry).expect("encode telemetry evidence");
        assert_eq!(
            dispute.evidence.digest,
            *blake3_hash(&telemetry_bytes).as_bytes(),
            "evidence digest must match telemetry payload"
        );
        let expected_uri = format!(
            "norito://sorafs/capacity_telemetry/{}/{:019}-{:019}",
            hex::encode(provider.as_bytes()),
            0,
            window,
        );
        assert_eq!(dispute.evidence.uri.as_deref(), Some(expected_uri.as_str()));
    }

    #[test]
    fn record_capacity_telemetry_deduplicates_proof_failure_dispute() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        stx.gov.sorafs_penalty = iroha_config::parameters::actual::SorafsPenaltyPolicy {
            utilisation_floor_bps: 7_500,
            uptime_floor_bps: 9_000,
            por_success_floor_bps: 9_000,
            strike_threshold: 3,
            penalty_bond_bps: 5_000,
            cooldown_windows: 1,
            max_pdp_failures: 0,
            max_potr_breaches: 0,
        };

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.credit = CreditPolicy {
            settlement_window_secs: SECONDS_PER_BILLING_MONTH,
            settlement_grace_secs: 0,
            low_balance_alert_bps: 1_000,
        };
        schedule.collateral = CollateralPolicy {
            multiplier_bps: 20_000,
            onboarding_discount_bps: 1,
            onboarding_period_secs: 0,
        };
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        let credit = provider_credit_nanos(provider, 10_000_000_000, 6_000_000_000);
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let window = SECONDS_PER_BILLING_MONTH;
        let telemetry = CapacityTelemetryRecord::new(
            provider, 0, window, 100, 90, 40, 1, 1, 7_500, 7_400, 128, 5, 1, 0, 0,
        )
        .with_nonce(window);
        RecordCapacityTelemetry { record: telemetry }
            .execute(&alice(), &mut stx)
            .expect("first telemetry submission");
        RecordCapacityTelemetry { record: telemetry }
            .execute(&alice(), &mut stx)
            .expect("duplicate telemetry submission");

        let disputes: Vec<_> = stx.world.capacity_disputes.iter().collect();
        assert_eq!(
            disputes.len(),
            1,
            "duplicate telemetry should not create duplicate disputes"
        );
    }

    #[test]
    fn capacity_dispute_replay_is_deterministic() {
        fn run_once() -> (
            CapacityFeeLedgerEntry,
            ProviderCreditRecord,
            Vec<(CapacityDisputeId, CapacityDisputeRecord)>,
        ) {
            let state = make_state();
            let mut block = state.block(block_header());
            let mut stx = block.transaction();
            seed_test_call_hash(&mut stx);

            stx.gov.sorafs_penalty = iroha_config::parameters::actual::SorafsPenaltyPolicy {
                utilisation_floor_bps: 7_500,
                uptime_floor_bps: 9_000,
                por_success_floor_bps: 9_000,
                strike_threshold: 3,
                penalty_bond_bps: 5_000,
                cooldown_windows: 1,
                max_pdp_failures: 0,
                max_potr_breaches: 0,
            };

            let (provider, record) = sample_capacity_record();
            RegisterCapacityDeclaration { record }
                .execute(&alice(), &mut stx)
                .expect("register capacity declaration");

            let mut schedule = PricingScheduleRecord::launch_default();
            schedule.credit = CreditPolicy {
                settlement_window_secs: SECONDS_PER_BILLING_MONTH,
                settlement_grace_secs: 0,
                low_balance_alert_bps: 1_000,
            };
            schedule.collateral = CollateralPolicy {
                multiplier_bps: 20_000,
                onboarding_discount_bps: 1,
                onboarding_period_secs: 0,
            };
            SetPricingSchedule { schedule }
                .execute(&alice(), &mut stx)
                .expect("set pricing schedule");

            let credit = provider_credit_nanos(provider, 9_000_000_000, 6_000_000_000);
            UpsertProviderCredit { record: credit }
                .execute(&alice(), &mut stx)
                .expect("seed provider credit");

            let window = SECONDS_PER_BILLING_MONTH;
            record_capacity_window_with_proofs(
                &mut stx,
                provider,
                0,
                window,
                96,
                96,
                70,
                9_800,
                9_600,
                256,
                ProofWindowCounters {
                    pdp_challenges: 4,
                    pdp_failures: 2,
                    potr_windows: 2,
                    potr_breaches: 1,
                },
            );

            let ledger = stx
                .world
                .capacity_fee_ledger
                .get(&provider)
                .cloned()
                .expect("ledger snapshot");
            let credit_snapshot = stx
                .world
                .provider_credit_ledger
                .get(&provider)
                .cloned()
                .expect("credit snapshot");
            let disputes: Vec<(CapacityDisputeId, CapacityDisputeRecord)> = stx
                .world
                .capacity_disputes
                .iter()
                .map(|(id, record)| (*id, record.clone()))
                .collect();
            assert!(
                !disputes.is_empty(),
                "proof failures should emit a capacity dispute"
            );
            (ledger, credit_snapshot, disputes)
        }

        let (ledger_a, credit_a, disputes_a) = run_once();
        let (ledger_b, credit_b, disputes_b) = run_once();

        assert_eq!(ledger_a, ledger_b, "ledger replay must be deterministic");
        assert_eq!(credit_a, credit_b, "credits must replay identically");
        assert_eq!(disputes_a, disputes_b, "disputes must replay identically");
    }

    #[test]
    fn record_capacity_telemetry_emits_proof_health_event() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        stx.gov.sorafs_penalty = iroha_config::parameters::actual::SorafsPenaltyPolicy {
            utilisation_floor_bps: 7_500,
            uptime_floor_bps: 9_000,
            por_success_floor_bps: 9_000,
            strike_threshold: 2,
            penalty_bond_bps: 5_000,
            cooldown_windows: 0,
            max_pdp_failures: 0,
            max_potr_breaches: 0,
        };

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.credit = CreditPolicy {
            settlement_window_secs: SECONDS_PER_BILLING_MONTH,
            settlement_grace_secs: 0,
            low_balance_alert_bps: 1_000,
        };
        schedule.collateral = CollateralPolicy {
            multiplier_bps: 20_000,
            onboarding_discount_bps: 1,
            onboarding_period_secs: 0,
        };
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        let credit = provider_credit_nanos(provider, 7_500_000_000, 5_000_000_000);
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let window = SECONDS_PER_BILLING_MONTH;
        record_capacity_window_with_proofs(
            &mut stx,
            provider,
            0,
            window,
            64,
            64,
            40,
            10_000,
            10_000,
            0,
            ProofWindowCounters {
                pdp_challenges: 4,
                pdp_failures: 2,
                ..ProofWindowCounters::default()
            },
        );

        let event = stx
            .world
            .internal_event_buf
            .iter()
            .find_map(|entry| match entry.as_ref() {
                DataEvent::Sorafs(SorafsGatewayEvent::ProofHealth(alert)) => Some(alert.clone()),
                _ => None,
            })
            .expect("proof health alert should be emitted");

        assert_eq!(event.provider_id, provider);
        assert_eq!(event.window_end_epoch, window);
        assert!(event.triggered_by_pdp);
        assert!(!event.triggered_by_potr);
        assert_eq!(event.pdp_failures, 2);
        assert_eq!(event.max_pdp_failures, 0);
        assert!(!event.cooldown_active);
        assert_eq!(event.prior_strikes, 0);
        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        assert_eq!(event.penalty_applied, credit_snapshot.slashed);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn proof_health_alert_emitted_for_potr_failures() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        stx.gov.sorafs_penalty = iroha_config::parameters::actual::SorafsPenaltyPolicy {
            utilisation_floor_bps: 7_500,
            uptime_floor_bps: 9_000,
            por_success_floor_bps: 9_000,
            strike_threshold: 1,
            penalty_bond_bps: 4_000,
            cooldown_windows: 0,
            max_pdp_failures: u32::MAX,
            max_potr_breaches: 0,
        };

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.credit = CreditPolicy {
            settlement_window_secs: SECONDS_PER_BILLING_MONTH,
            settlement_grace_secs: 0,
            low_balance_alert_bps: 1_000,
        };
        schedule.collateral = CollateralPolicy {
            multiplier_bps: 20_000,
            onboarding_discount_bps: 1,
            onboarding_period_secs: 0,
        };
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        let credit = provider_credit_nanos(provider, 5_500_000_000, 3_500_000_000);
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let window = SECONDS_PER_BILLING_MONTH;
        record_capacity_window_with_proofs(
            &mut stx,
            provider,
            0,
            window,
            64,
            64,
            50,
            10_000,
            10_000,
            0,
            ProofWindowCounters {
                potr_windows: 3,
                potr_breaches: 1,
                ..ProofWindowCounters::default()
            },
        );

        let event = stx
            .world
            .internal_event_buf
            .iter()
            .find_map(|entry| match entry.as_ref() {
                DataEvent::Sorafs(SorafsGatewayEvent::ProofHealth(alert)) => Some(alert.clone()),
                _ => None,
            })
            .expect("proof health alert should be emitted");

        assert_eq!(event.provider_id, provider);
        assert!(event.triggered_by_potr);
        assert!(!event.triggered_by_pdp);
        assert_eq!(event.potr_breaches, 1);
        assert_eq!(event.max_potr_breaches, 0);
        assert_eq!(event.prior_strikes, 0);
        assert!(!event.cooldown_active);
        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        assert_eq!(event.penalty_applied, credit_snapshot.slashed);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn proof_health_alert_reports_cooldown_state() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        stx.gov.sorafs_penalty = iroha_config::parameters::actual::SorafsPenaltyPolicy {
            utilisation_floor_bps: 7_500,
            uptime_floor_bps: 9_000,
            por_success_floor_bps: 9_000,
            strike_threshold: 1,
            penalty_bond_bps: 5_000,
            cooldown_windows: 2,
            max_pdp_failures: 0,
            max_potr_breaches: u32::MAX,
        };

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.credit = CreditPolicy {
            settlement_window_secs: SECONDS_PER_BILLING_MONTH,
            settlement_grace_secs: 0,
            low_balance_alert_bps: 1_000,
        };
        schedule.collateral = CollateralPolicy {
            multiplier_bps: 20_000,
            onboarding_discount_bps: 1,
            onboarding_period_secs: 0,
        };
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        let credit = provider_credit_nanos(provider, 6_000_000_000, 4_000_000_000);
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let window = SECONDS_PER_BILLING_MONTH;
        record_capacity_window_with_proofs(
            &mut stx,
            provider,
            0,
            window,
            64,
            64,
            40,
            10_000,
            10_000,
            0,
            ProofWindowCounters {
                pdp_challenges: 2,
                pdp_failures: 1,
                ..ProofWindowCounters::default()
            },
        );
        record_capacity_window_with_proofs(
            &mut stx,
            provider,
            window,
            window * 2,
            64,
            64,
            38,
            10_000,
            10_000,
            0,
            ProofWindowCounters {
                pdp_challenges: 1,
                pdp_failures: 1,
                ..ProofWindowCounters::default()
            },
        );

        let alerts: Vec<_> = stx
            .world
            .internal_event_buf
            .iter()
            .filter_map(|entry| match entry.as_ref() {
                DataEvent::Sorafs(SorafsGatewayEvent::ProofHealth(alert)) => Some(alert.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(alerts.len(), 2);
        let first = &alerts[0];
        let second = &alerts[1];
        assert!(!first.cooldown_active);
        assert!(second.cooldown_active);
        assert!(!first.penalty_applied.is_zero());
        assert_eq!(second.penalty_applied, Quantity::zero());
        assert!(first.window_end_epoch < second.window_end_epoch);
        let credit_snapshot = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("credit snapshot");
        assert_eq!(credit_snapshot.slashed, first.penalty_applied);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn capacity_fee_ledger_30_day_soak_deterministic() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        stx.gov.sorafs_penalty.penalty_bond_bps = 0;
        stx.gov.sorafs_penalty.strike_threshold = u32::MAX;

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.credit = CreditPolicy {
            settlement_window_secs: SECONDS_PER_BILLING_MONTH,
            settlement_grace_secs: 0,
            low_balance_alert_bps: 1_000,
        };
        schedule.collateral = CollateralPolicy {
            multiplier_bps: 30_000,
            onboarding_discount_bps: 5_000,
            onboarding_period_secs: SECONDS_PER_BILLING_MONTH,
        };
        SetPricingSchedule {
            schedule: schedule.clone(),
        }
        .execute(&alice(), &mut stx)
        .expect("set pricing schedule");

        let mut expected_ledgers: std::collections::BTreeMap<ProviderId, CapacityFeeLedgerEntry> =
            std::collections::BTreeMap::new();
        let mut providers = Vec::new();
        for index in 0_u8..5 {
            let provider = ProviderId::new([index.wrapping_add(1); 32]);
            let committed = 256 + u64::from(index) * 32;
            let declaration = CapacityDeclarationV1 {
                version: CAPACITY_DECLARATION_VERSION_V1,
                provider_id: provider.as_bytes().to_owned(),
                stake: StakePointer {
                    pool_id: [index.wrapping_add(40); 32],
                    stake_amount: "1".parse().expect("canonical XOR stake"),
                },
                committed_capacity_gib: committed,
                chunker_commitments: vec![ChunkerCommitmentV1 {
                    profile_id: "sorafs.sf1@1.0.0".to_string(),
                    profile_aliases: None,
                    committed_gib: committed,
                    capability_refs: vec![CapabilityType::ToriiGateway],
                }],
                lane_commitments: Vec::new(),
                pricing: None,
                valid_from: 1_700_000_000,
                valid_until: 1_700_000_000 + (SECONDS_PER_BILLING_MONTH * 64),
                metadata: vec![CapacityMetadataEntry {
                    key: PROVIDER_OWNER_METADATA_KEY.to_owned(),
                    value: account_literal(&alice()),
                }],
            };
            let canonical_bytes =
                norito::to_bytes(&declaration).expect("serialize capacity declaration");
            let record = CapacityDeclarationRecord::new(
                provider,
                canonical_bytes,
                committed,
                0,
                0,
                1_800_000_000,
                Metadata::default(),
            );
            RegisterCapacityDeclaration {
                record: record.clone(),
            }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

            let credit = provider_credit_nanos(provider, 40_000_000_000, 12_000_000_000);
            UpsertProviderCredit { record: credit }
                .execute(&alice(), &mut stx)
                .expect("seed provider credit");

            expected_ledgers.insert(
                provider,
                CapacityFeeLedgerEntry {
                    provider_id: provider,
                    total_declared_gib: u128::from(committed),
                    last_updated_epoch: record.registered_epoch,
                    ..Default::default()
                },
            );
            providers.push(provider);
        }

        let window = SECONDS_PER_BILLING_MONTH;

        for day in 0_u64..30 {
            let start = day * window;
            let end = (day + 1) * window;

            for (index, provider) in providers.iter().enumerate() {
                let index_u64 = index as u64;
                let declared = 192 + index_u64 * 16;
                let utilised = 120 + ((day + index_u64) % 48);
                let egress_bytes = (index_u64 + 1)
                    .checked_mul((day + 5) * 2_048)
                    .expect("soak egress bytes");
                record_capacity_window(
                    &mut stx,
                    *provider,
                    start,
                    end,
                    declared,
                    declared,
                    utilised,
                    10_000,
                    9_900,
                    egress_bytes,
                );

                let storage_fee = schedule
                    .storage_charge(StorageClass::Hot, utilised, window)
                    .expect("soak storage fee");
                let uptime_bps = u128::from(10_000_u32);
                let por_bps = u128::from(9_900_u32);
                let health_multiplier = uptime_bps
                    .checked_mul(por_bps)
                    .expect("soak health multiplier");
                let storage_fee = round_xor_quantity_ratio(
                    &storage_fee,
                    health_multiplier,
                    10_000_u128 * 10_000_u128,
                )
                .expect("soak health-adjusted storage fee");
                let egress_fee = schedule
                    .egress_charge_bytes(StorageClass::Hot, egress_bytes)
                    .expect("soak egress fee");
                let expected_settlement = schedule
                    .expected_settlement_storage_charge(StorageClass::Hot, utilised)
                    .expect("soak expected settlement storage fee")
                    .checked_add(&egress_fee)
                    .expect("soak expected settlement sum");

                let entry =
                    expected_ledgers
                        .entry(*provider)
                        .or_insert_with(|| CapacityFeeLedgerEntry {
                            provider_id: *provider,
                            ..Default::default()
                        });
                entry
                    .accrue(&CapacityAccrual {
                        declared_delta_gib: u128::from(declared),
                        utilised_delta_gib: u128::from(utilised),
                        storage_fee_delta: storage_fee,
                        egress_fee_delta: egress_fee,
                        expected_settlement,
                        window_start_epoch: start,
                        window_end_epoch: end,
                        nonce: end,
                    })
                    .expect("soak capacity ledger accrual");
            }
        }

        let actual_ledgers: std::collections::BTreeMap<ProviderId, CapacityFeeLedgerEntry> = stx
            .world
            .capacity_fee_ledger
            .iter()
            .map(|(provider, entry)| (*provider, entry.clone()))
            .collect();

        assert_eq!(actual_ledgers.len(), providers.len());
        for provider in providers {
            let expected = expected_ledgers.get(&provider).expect("expected ledger");
            let actual = actual_ledgers.get(&provider).expect("actual ledger");
            assert_eq!(actual.total_declared_gib, expected.total_declared_gib);
            assert_eq!(actual.total_utilised_gib, expected.total_utilised_gib);
            assert_eq!(actual.storage_fee, expected.storage_fee);
            assert_eq!(actual.egress_fee, expected.egress_fee);
            assert_eq!(actual.accrued_fee, expected.accrued_fee);
            assert_eq!(actual.expected_settlement, expected.expected_settlement);
            assert_eq!(actual.penalty_slashed, Quantity::zero());
            assert_eq!(actual.penalty_events, 0);
        }

        let mut hasher = blake3::Hasher::new();
        for (provider, entry) in &actual_ledgers {
            hasher.update(provider.as_bytes());
            hasher.update(&entry.total_declared_gib.to_le_bytes());
            hasher.update(&entry.total_utilised_gib.to_le_bytes());
            hasher.update(&exact_xor_nanos(&entry.storage_fee).to_le_bytes());
            hasher.update(&exact_xor_nanos(&entry.egress_fee).to_le_bytes());
            hasher.update(&exact_xor_nanos(&entry.accrued_fee).to_le_bytes());
            hasher.update(&exact_xor_nanos(&entry.expected_settlement).to_le_bytes());
        }
        let digest = hasher.finalize().to_hex().to_string();
        // Update `expected_digest` when SoraFS capacity accrual semantics or the
        // launch pricing schedule change. Run this test with `-- --nocapture`
        // to print the current digest before refreshing the value below.
        let expected_digest = "71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1";
        println!("capacity_soak_digest={digest}");
        assert_eq!(
            digest, expected_digest,
            "refresh the soak digest if the accrual math or launch pricing schedule changes"
        );
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn record_capacity_telemetry_charges_egress() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let (provider, record) = sample_capacity_record();
        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.tiers = vec![TierRate::new(
            StorageClass::Hot,
            xor_quantity_nanos(1),
            xor_quantity_nanos(2_000_000),
        )];
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        let credit = provider_credit_nanos(provider, 5_000_000_000, 0);
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let telemetry = CapacityTelemetryRecord::new(
            provider,
            0,
            10,
            100,
            100,
            1,
            0,
            0,
            10_000,
            10_000,
            BYTES_PER_GIB as u64,
            0,
            0,
            0,
            0,
        )
        .with_nonce(10);
        RecordCapacityTelemetry { record: telemetry }
            .execute(&alice(), &mut stx)
            .expect("record capacity telemetry with egress");

        let ledger = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("capacity fee ledger stored");
        let pricing = stx.world.sorafs_pricing.get();
        let expected_storage_fee = pricing
            .storage_charge(StorageClass::Hot, 1, telemetry.window_end_epoch)
            .expect("expected storage fee");
        let expected_egress_fee = pricing
            .egress_charge_bytes(StorageClass::Hot, BYTES_PER_GIB as u64)
            .expect("expected egress fee");
        let expected_settlement = pricing
            .expected_settlement_storage_charge(StorageClass::Hot, 1)
            .expect("expected settlement storage fee")
            .checked_add(&expected_egress_fee)
            .expect("expected settlement sum");
        assert_eq!(ledger.storage_fee, expected_storage_fee);
        assert_eq!(ledger.egress_fee, expected_egress_fee);
        assert_eq!(
            ledger.accrued_fee,
            expected_storage_fee
                .checked_add(&expected_egress_fee)
                .expect("bounded expected accrued fee")
        );
        assert_eq!(ledger.expected_settlement, expected_settlement);

        let credit_after = stx
            .world
            .provider_credit_ledger
            .get(&provider)
            .expect("provider credit stored");
        let debit = expected_storage_fee
            .checked_add(&expected_egress_fee)
            .expect("expected debit sum");
        assert_eq!(
            credit_after.available_credit,
            xor_quantity_nanos(5_000_000_000)
                .checked_sub(&debit)
                .expect("fixture credit covers debit")
        );
        assert_eq!(credit_after.expected_settlement, expected_settlement);
    }

    #[test]
    fn record_capacity_telemetry_uses_declaration_storage_class() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let mut declaration = sample_capacity_declaration();
        declaration.metadata.push(CapacityMetadataEntry {
            key: STORAGE_CLASS_METADATA_KEY.to_string(),
            value: "cold".to_string(),
        });
        let canonical_bytes = norito::to_bytes(&declaration).expect("serialize declaration");
        let provider = ProviderId::new(declaration.provider_id);
        let record = CapacityDeclarationRecord::new(
            provider,
            canonical_bytes,
            declaration.committed_capacity_gib,
            9,
            10,
            20,
            Metadata::default(),
        );

        RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect("register capacity declaration");

        let key: Name = STORAGE_CLASS_METADATA_KEY
            .parse()
            .expect("metadata key parses");
        let stored = stx
            .world
            .capacity_declarations
            .get(&provider)
            .expect("declaration stored");
        let stored_value = stored
            .metadata
            .get(&key)
            .expect("metadata copied from declaration");
        let stored_str: String = stored_value
            .try_into_any()
            .expect("metadata decodes as string");
        assert_eq!(stored_str, "cold");

        let mut schedule = PricingScheduleRecord::launch_default();
        schedule.default_storage_class = StorageClass::Hot;
        schedule.tiers = vec![
            TierRate::new(
                StorageClass::Hot,
                xor_quantity_nanos(5_000_000),
                xor_quantity_nanos(5_000),
            ),
            TierRate::new(
                StorageClass::Cold,
                xor_quantity_nanos(1_000_000),
                xor_quantity_nanos(1_000),
            ),
        ];
        SetPricingSchedule { schedule }
            .execute(&alice(), &mut stx)
            .expect("set pricing schedule");

        let credit = provider_credit_nanos(provider, 1_000_000_000, 0);
        UpsertProviderCredit { record: credit }
            .execute(&alice(), &mut stx)
            .expect("seed provider credit");

        let telemetry = CapacityTelemetryRecord::new(
            provider,
            0,
            SECONDS_PER_BILLING_MONTH,
            100,
            100,
            100,
            0,
            0,
            10_000,
            10_000,
            0,
            0,
            0,
            0,
            0,
        )
        .with_nonce(SECONDS_PER_BILLING_MONTH);
        RecordCapacityTelemetry { record: telemetry }
            .execute(&alice(), &mut stx)
            .expect("record capacity telemetry");

        let ledger = stx
            .world
            .capacity_fee_ledger
            .get(&provider)
            .expect("capacity fee ledger stored");
        assert_eq!(ledger.storage_fee, xor_quantity_nanos(100_000_000));
    }

    #[test]
    fn register_capacity_declaration_rejects_metadata_conflict() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let mut declaration = sample_capacity_declaration();
        declaration.metadata.push(CapacityMetadataEntry {
            key: STORAGE_CLASS_METADATA_KEY.to_string(),
            value: "cold".to_string(),
        });
        let canonical_bytes = norito::to_bytes(&declaration).expect("serialize declaration");
        let provider = ProviderId::new(declaration.provider_id);
        let mut metadata = Metadata::default();
        let key: Name = STORAGE_CLASS_METADATA_KEY
            .parse()
            .expect("metadata key parses");
        metadata.insert(key, Json::new("hot"));

        let record = CapacityDeclarationRecord::new(
            provider,
            canonical_bytes,
            declaration.committed_capacity_gib,
            9,
            10,
            20,
            metadata,
        );

        let err = RegisterCapacityDeclaration { record }
            .execute(&alice(), &mut stx)
            .expect_err("conflicting metadata must be rejected");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error variant: {other:?}"),
        };
        assert!(
            message.contains("metadata conflict"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn storage_class_metadata_defaults_when_missing() {
        let metadata = Metadata::default();
        let provider = ProviderId::new([0x11; 32]);
        let class =
            super::storage_class_from_declaration_metadata(provider, &metadata, StorageClass::Warm)
                .expect("fallback must succeed");
        assert_eq!(class, StorageClass::Warm);
    }

    #[test]
    fn storage_class_metadata_overrides_case_insensitively() {
        let provider = ProviderId::new([0x22; 32]);
        let mut metadata = Metadata::default();
        let key = STORAGE_CLASS_METADATA_KEY
            .parse()
            .expect("metadata key must parse");
        let _ = metadata.insert(key, Json::new("CoLd"));

        let class =
            super::storage_class_from_declaration_metadata(provider, &metadata, StorageClass::Hot)
                .expect("metadata override must succeed");
        assert_eq!(class, StorageClass::Cold);
    }

    #[test]
    fn storage_class_metadata_rejects_invalid_value() {
        let provider = ProviderId::new([0x33; 32]);
        let mut metadata = Metadata::default();
        let key = STORAGE_CLASS_METADATA_KEY
            .parse()
            .expect("metadata key must parse");
        let _ = metadata.insert(key, Json::new("glacier"));

        let err =
            super::storage_class_from_declaration_metadata(provider, &metadata, StorageClass::Hot)
                .expect_err("invalid value must error");
        match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => {
                assert!(
                    message.contains("must be one of hot, warm, or cold"),
                    "unexpected error: {message}"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn storage_class_from_declaration_record_reads_payload_metadata() {
        let mut declaration = sample_capacity_declaration();
        declaration.metadata.push(CapacityMetadataEntry {
            key: STORAGE_CLASS_METADATA_KEY.to_string(),
            value: "cold".to_string(),
        });
        let canonical_bytes = norito::to_bytes(&declaration).expect("serialize declaration");
        let provider = ProviderId::new(declaration.provider_id);
        let record = CapacityDeclarationRecord::new(
            provider,
            canonical_bytes,
            declaration.committed_capacity_gib,
            9,
            10,
            20,
            Metadata::default(),
        );

        let class = super::storage_class_from_declaration_record(&record, StorageClass::Hot)
            .expect("payload metadata lookup must succeed");
        assert_eq!(class, StorageClass::Cold);
    }

    #[test]
    fn upsert_provider_credit_requires_registered_provider() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);

        let record = provider_credit_nanos(ProviderId::new([0x55; 32]), 1_000, 0);
        let err = UpsertProviderCredit { record }
            .execute(&alice(), &mut stx)
            .expect_err("provider must exist before credit entry");
        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("has no registered owner"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn register_capacity_declaration_rejects_provider_mismatch() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (_provider, mut record) = sample_capacity_record();
        record.provider_id = ProviderId::new([0x33; 32]);
        let instruction = RegisterCapacityDeclaration { record };

        let err = instruction
            .execute(&alice(), &mut stx)
            .expect_err("payload/provider mismatch must be rejected");

        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("provider mismatch"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn register_capacity_declaration_rejects_committed_capacity_mismatch() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (_provider, mut record) = sample_capacity_record();
        record.committed_capacity_gib += 1;
        let instruction = RegisterCapacityDeclaration { record };

        let err = instruction
            .execute(&alice(), &mut stx)
            .expect_err("committed capacity mismatch must be rejected");

        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("committed capacity mismatch"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn register_capacity_declaration_rejects_noncanonical_and_resource_bomb_payloads() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let (provider, base_record) = sample_capacity_record();
        let alternate = {
            let _guard = norito::core::DecodeFlagsGuard::enter(0);
            norito::to_bytes(&sample_capacity_declaration())
                .expect("encode alternate-layout capacity declaration")
        };
        assert_ne!(alternate, base_record.declaration);
        let mut allocation_bomb = sample_capacity_declaration();
        allocation_bomb.metadata.push(CapacityMetadataEntry {
            key: "bomb".to_owned(),
            value: "x".repeat(sorafs_manifest::capacity::MAX_CAPACITY_METADATA_VALUE_BYTES + 1),
        });
        let allocation_bomb = norito::to_bytes(&allocation_bomb)
            .expect("encode capacity declaration allocation bomb");
        assert!(allocation_bomb.len() <= MAX_CAPACITY_DECLARATION_PAYLOAD_BYTES);

        for payload in [
            Vec::new(),
            vec![0xFF],
            vec![0xA5; MAX_CAPACITY_DECLARATION_PAYLOAD_BYTES + 1],
            alternate,
            allocation_bomb,
        ] {
            let mut record = base_record.clone();
            record.declaration = payload;
            let err = RegisterCapacityDeclaration { record }
                .execute(&alice(), &mut stx)
                .expect_err("invalid capacity declaration payload must be rejected");

            let message = match err {
                InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(message),
                ) => message,
                other => panic!("unexpected error: {other:?}"),
            };
            assert!(
                message.contains("invalid capacity declaration payload"),
                "unexpected error message: {message}"
            );
            assert!(stx.world.capacity_declarations.get(&provider).is_none());
            assert!(stx.world.provider_owners.get(&provider).is_none());
        }
    }

    #[test]
    fn register_provider_owner_sets_binding() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let provider = ProviderId::new([0xA1; 32]);
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        ensure_registered_account(&mut stx, &bob(), &domain_id);

        RegisterProviderOwner {
            provider_id: provider,
            owner: bob(),
        }
        .execute(&alice(), &mut stx)
        .expect("register provider owner");

        assert_eq!(
            stx.world.provider_owners.get(&provider),
            Some(&bob()),
            "binding should be inserted"
        );
        let permission = AccountPermission::from(CanOperateSorafsRepair {
            provider_id: provider,
        });
        let perms = stx
            .world
            .account_permissions
            .get(&bob())
            .expect("permissions should be seeded");
        assert!(
            perms.contains(&permission),
            "repair worker permission should be granted"
        );
    }

    #[test]
    fn register_provider_owner_rejects_missing_owner_account() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let provider = ProviderId::new([0xA5; 32]);
        let missing_owner = checked_account_id();

        let err = RegisterProviderOwner {
            provider_id: provider,
            owner: missing_owner.clone(),
        }
        .execute(&alice(), &mut stx)
        .expect_err("owner account must exist");

        assert!(
            matches!(err, InstructionExecutionError::Find(FindError::Account(ref id)) if *id == missing_owner),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn register_provider_owner_rejects_rebind() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let provider = ProviderId::new([0xA2; 32]);
        stx.world.provider_owners.insert(provider, alice());

        let err = RegisterProviderOwner {
            provider_id: provider,
            owner: bob(),
        }
        .execute(&alice(), &mut stx)
        .expect_err("rebinding must fail");

        let message = match err {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message,
            other => panic!("unexpected error: {other:?}"),
        };
        assert!(
            message.contains("already owned"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn unregister_provider_owner_removes_binding() {
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx);
        let provider = ProviderId::new([0xA3; 32]);
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        ensure_registered_account(&mut stx, &alice(), &domain_id);
        RegisterProviderOwner {
            provider_id: provider,
            owner: alice(),
        }
        .execute(&alice(), &mut stx)
        .expect("register provider owner");

        UnregisterProviderOwner {
            provider_id: provider,
        }
        .execute(&alice(), &mut stx)
        .expect("unregister provider owner");

        assert!(
            stx.world.provider_owners.get(&provider).is_none(),
            "binding should be removed"
        );
        let permission = AccountPermission::from(CanOperateSorafsRepair {
            provider_id: provider,
        });
        if let Some(perms) = stx.world.account_permissions.get(&alice()) {
            assert!(
                !perms.contains(&permission),
                "repair worker permission should be revoked"
            );
        }
    }

    #[test]
    fn sorafs_provider_owner_query_resolves_binding() {
        use crate::smartcontracts::ValidSingularQuery;

        let mut state = make_state();
        let provider = ProviderId::new([0xA4; 32]);
        state.world.provider_owners.insert(provider, alice());

        let query = iroha_data_model::query::sorafs::prelude::FindSorafsProviderOwner {
            provider_id: provider,
        };
        let result = query.execute(&state.view()).expect("query resolves owner");

        assert_eq!(result, alice());
    }
}
