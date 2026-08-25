//! Shared Soracloud runtime snapshot types, generated HF manifests, and execution traits.
use crate::state::WorldReadOnly;
use iroha_crypto::Hash;
#[cfg(test)]
use iroha_data_model::soracloud::{
    SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1,
    SORACLOUD_PRIVATE_MODEL_RUNTIME_VERSION_V1, SoraUploadedModelRuntimeFormatV1,
    derive_soracloud_private_model_request_commitment_v1,
    derive_soracloud_private_model_result_commitment_v1,
    derive_soracloud_private_uploaded_model_execution_receipt_id_v1,
};
use iroha_data_model::{
    account::AccountId,
    isi::InstructionBox,
    name::Name,
    nexus::{LaneId, staking::PublicLaneValidatorStatus},
    peer::PeerId,
    smart_contract::manifest::EntryPointKind,
    soracloud::{
        AGENT_APARTMENT_MANIFEST_VERSION_V1, AgentApartmentManifestV1, AgentToolCapabilityV1,
        AgentUpgradePolicyV1, SORA_CONTAINER_MANIFEST_VERSION_V1,
        SORA_DEPLOYMENT_BUNDLE_VERSION_V1, SORA_SERVICE_MANIFEST_VERSION_V1,
        SoraAgentRuntimeStatusV1, SoraArtifactKindV1, SoraCapabilityPolicyV1,
        SoraCertifiedResponsePolicyV1, SoraConfigExportV1, SoraContainerManifestRefV1,
        SoraContainerManifestV1, SoraContainerRuntimeV1, SoraDeploymentBundleV1,
        SoraHfPlacementHostAssignmentV1, SoraHfPlacementHostRoleV1, SoraHfPlacementHostStatusV1,
        SoraHfPlacementRecordV1, SoraHfPlacementStatusV1, SoraHfSharedLeaseMemberStatusV1,
        SoraHfSharedLeaseStatusV1, SoraHfSourceStatusV1, SoraHttpServiceEconomicsV1,
        SoraInrouGuestIsaV1, SoraInrouGuestOsV1, SoraInrouReplicaPlacementV1,
        SoraInrouServicePlacementRecordV1, SoraLeaseVolumeKindV1, SoraLifecycleHooksV1,
        SoraNetworkAllowlistEntryV1, SoraNetworkPolicyV1, SoraOrderedMailboxResultV1,
        SoraPrivateModelArtifactRefV1, SoraPrivateUploadedModelExecutionReceiptV1,
        SoraResourceLimitsV1, SoraRolloutPolicyV1, SoraRouteTargetV1, SoraRouteVisibilityV1,
        SoraRuntimeDeterministicValidatorHostV1, SoraRuntimeReceiptV1,
        SoraServiceDeploymentStateV1, SoraServiceExecutionPlaneV1, SoraServiceHandlerClassV1,
        SoraServiceHandlerV1, SoraServiceHealthStatusV1, SoraServiceLeaseStatusV1,
        SoraServiceMailboxMessageV1, SoraServiceManifestV1, SoraServiceRuntimeStateV1,
        SoraStateEncryptionV1, SoraStateMutationOperationV1, SoraTlsModeV1,
        SoraUploadedModelBundleV1, SoraUploadedModelEncryptionRecipientV1,
        SoraUploadedModelKeyEncapsulationV1, SoraUploadedModelKeyWrapAeadV1,
    },
    sorafs::pin_registry::{ManifestDigest, StorageClass},
    transaction::SignedTransaction,
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
const HF_GENERATED_SERVICE_VERSION_V1: &str = "hf.generated.v1";
const HF_GENERATED_SERVICE_MARKER_ENV: &str = "SORACLOUD_HF_GENERATED";
const HF_GENERATED_SOURCE_ID_ENV: &str = "SORACLOUD_HF_SOURCE_ID";
const HF_GENERATED_REPO_ID_ENV: &str = "SORACLOUD_HF_REPO_ID";
const HF_GENERATED_REVISION_ENV: &str = "SORACLOUD_HF_REVISION";
const HF_GENERATED_MODEL_NAME_ENV: &str = "SORACLOUD_HF_MODEL_NAME";
const HF_GENERATED_ROUTE_SUFFIX: &str = ".hf.soracloud.internal";
const HF_GENERATED_ENTRYPOINT_INFER: &str = "infer";
const HF_GENERATED_ENTRYPOINT_METADATA: &str = "metadata";
/// Return the greatest globally allocated Soracloud sequence.
///
/// The persisted transactional watermark makes allocation O(1) and permits independently pruned
/// history stores without reusing sequence numbers.
#[must_use]
pub fn latest_soracloud_sequence(world: &impl WorldReadOnly) -> u64 {
    world.soracloud_sequence_watermark()
}
/// Return the next authoritative Soracloud sequence visible from committed world state.
#[must_use]
pub fn authoritative_soracloud_sequence(world: &impl WorldReadOnly) -> u64 {
    latest_soracloud_sequence(world).saturating_add(1)
}
/// Return whether an account has an exact, active validator record on an authoritative lane.
///
/// Soracloud adverts are only eligibility claims; validator lifecycle state remains the
/// authoritative admission gate for both placement and request serving.
#[must_use]
pub fn soracloud_validator_is_active(
    world: &impl WorldReadOnly,
    validator_account_id: &AccountId,
    lane_is_active_for_authority: impl Fn(LaneId) -> bool,
) -> bool {
    let Some(signatory) = validator_account_id.try_signatory() else {
        return false;
    };
    let canonical_peer_id = PeerId::from(signatory.clone());
    world.public_lane_validators().iter().any(|(key, record)| {
        &key.1 == validator_account_id
            && crate::state::public_lane_validator_record_matches_key(key, record)
            && record.status == PublicLaneValidatorStatus::Active
            && record.peer_id == canonical_peer_id
            && lane_is_active_for_authority(key.0)
    })
}
/// Return whether an account has one exact active validator record bound to its canonical peer.
///
/// First-release Soracloud host identities are intentionally singular: the validator account must expose
/// exactly one signatory, the peer must be derived from that signatory, and the same peer must be
/// present in an active authoritative validator record. A stale advert therefore becomes
/// ineligible immediately when validator topology changes.
#[must_use]
pub fn soracloud_validator_has_active_peer_binding(
    world: &impl WorldReadOnly,
    validator_account_id: &AccountId,
    peer_id: &str,
    lane_is_active_for_authority: impl Fn(LaneId) -> bool,
) -> bool {
    let Some(signatory) = validator_account_id.try_signatory() else {
        return false;
    };
    let canonical_peer_id = PeerId::from(signatory.clone());
    if canonical_peer_id.to_string() != peer_id {
        return false;
    }
    world.public_lane_validators().iter().any(|(key, record)| {
        &key.1 == validator_account_id
            && crate::state::public_lane_validator_record_matches_key(key, record)
            && record.status == PublicLaneValidatorStatus::Active
            && record.peer_id == canonical_peer_id
            && lane_is_active_for_authority(key.0)
    })
}

#[derive(Encode)]
struct OrderedMailboxDestinationFingerprintV1 {
    service_name: Name,
    service_version: String,
    handler_name: Name,
}

#[derive(Encode)]
struct OrderedMailboxExecutorPreimageV1 {
    domain: String,
    beacon_height: u64,
    beacon_round: u64,
    beacon_pulse_id: [u8; 32],
    beacon_seed: [u8; 32],
    destination: OrderedMailboxDestinationFingerprintV1,
    host: SoraRuntimeDeterministicValidatorHostV1,
}

#[derive(Encode)]
struct OrderedMailboxPayloadFingerprintV1 {
    payload_bytes: u64,
    payload_commitment: Hash,
}

#[derive(Encode)]
struct OrderedMailboxMutationFingerprintV1 {
    schema_version: u16,
    binding_name: Name,
    state_key: String,
    operation: SoraStateMutationOperationV1,
    encryption: SoraStateEncryptionV1,
    value_payload: Option<OrderedMailboxPayloadFingerprintV1>,
}

#[derive(Encode)]
struct OrderedMailboxMessageFingerprintV1 {
    schema_version: u16,
    message_id: Hash,
    from_service: Name,
    from_service_version: String,
    from_handler: Name,
    to_service: Name,
    to_service_version: String,
    to_handler: Name,
    payload_commitment: Hash,
    delivery_delay_blocks: u32,
    enqueue_sequence: u64,
    enqueue_height: u64,
    available_after_height: u64,
    expires_at_height: u64,
}

#[derive(Encode)]
struct OrderedMailboxReceiptFingerprintV1 {
    mailbox_message_id: Option<Hash>,
    service_name: Name,
    service_version: String,
    handler_name: Name,
    handler_class: SoraServiceHandlerClassV1,
    request_commitment: Hash,
    certified_by: SoraCertifiedResponsePolicyV1,
    execution_host: Option<iroha_data_model::soracloud::SoraRuntimeExecutionHostV1>,
    journal_artifact_hash: Option<Hash>,
    checkpoint_artifact_hash: Option<Hash>,
}

#[derive(Encode)]
struct OrderedMailboxReceiptIdentityPreimageV1 {
    domain: String,
    schema_version: u16,
    mailbox_message_id: Hash,
    service_name: Name,
    service_version: String,
    handler_name: Name,
    handler_class: SoraServiceHandlerClassV1,
    request_commitment: Hash,
    result_commitment: Hash,
    certified_by: SoraCertifiedResponsePolicyV1,
    execution_host: Option<iroha_data_model::soracloud::SoraRuntimeExecutionHostV1>,
    journal_artifact_hash: Option<Hash>,
    checkpoint_artifact_hash: Option<Hash>,
}

#[derive(Encode)]
struct OrderedMailboxRuntimeStateFingerprintV1 {
    schema_version: u16,
    service_name: Name,
    active_service_version: String,
    health_status: SoraServiceHealthStatusV1,
    load_factor_bps: u16,
    materialized_bundle_hash: Hash,
}

#[derive(Encode)]
struct OrderedMailboxResultPreimageV1 {
    domain: String,
    receipt: OrderedMailboxReceiptFingerprintV1,
    state_mutations: Vec<OrderedMailboxMutationFingerprintV1>,
    outbound_mailbox_messages: Vec<OrderedMailboxMessageFingerprintV1>,
    response_commitment: Hash,
    runtime_execution_commitment: Hash,
    content_type: Option<String>,
    runtime_state: Option<OrderedMailboxRuntimeStateFingerprintV1>,
}

/// Select the exact active public-lane validator responsible for one ordered mailbox message.
///
/// Selection is a deterministic rendezvous over the first finalized threshold-beacon pulse after
/// enqueue, the immutable destination, and each unique exact active validator identity. The
/// current height advances through the ranked candidates after a fixed grace, providing bounded
/// deterministic failover without caller-controlled entropy.
#[must_use]
pub fn resolve_ordered_mailbox_executor(
    world: &impl WorldReadOnly,
    message: &SoraServiceMailboxMessageV1,
    current_height: u64,
    lane_is_active_for_authority: impl Fn(LaneId) -> bool,
) -> Option<SoraRuntimeDeterministicValidatorHostV1> {
    const EXECUTOR_FAILOVER_GRACE_BLOCKS: u64 = 10;
    let pulse = world
        .global_beacon_pulses()
        .iter()
        .map(|(_pulse_id, pulse)| *pulse)
        .filter(|pulse| pulse.height > message.enqueue_height)
        .min_by_key(|pulse| (pulse.height, pulse.round, pulse.pulse_id))?;
    let mut unique_hosts =
        BTreeMap::<(AccountId, String), SoraRuntimeDeterministicValidatorHostV1>::new();
    for (key, record) in world.public_lane_validators().iter() {
        if !crate::state::public_lane_validator_record_matches_key(key, record)
            || record.status != PublicLaneValidatorStatus::Active
            || !lane_is_active_for_authority(key.0)
        {
            continue;
        }
        let Some(signatory) = key.1.try_signatory() else {
            continue;
        };
        let canonical_peer_id = PeerId::from(signatory.clone());
        if record.peer_id != canonical_peer_id {
            continue;
        }
        let peer_id = record.peer_id.to_string();
        let host = SoraRuntimeDeterministicValidatorHostV1 {
            lane_id: key.0,
            validator_account_id: key.1.clone(),
            peer_id: peer_id.clone(),
        };
        unique_hosts
            .entry((key.1.clone(), peer_id))
            .and_modify(|existing| {
                if host.lane_id < existing.lane_id {
                    *existing = host.clone();
                }
            })
            .or_insert(host);
    }
    let mut ranked = unique_hosts
        .into_values()
        .map(|host| {
            let score = Hash::new(Encode::encode(&OrderedMailboxExecutorPreimageV1 {
                domain: "soracloud:ordered-mailbox-executor:v1".to_owned(),
                beacon_height: pulse.height,
                beacon_round: pulse.round,
                beacon_pulse_id: pulse.pulse_id,
                beacon_seed: pulse.seed,
                destination: OrderedMailboxDestinationFingerprintV1 {
                    service_name: message.to_service.clone(),
                    service_version: message.to_service_version.clone(),
                    handler_name: message.to_handler.clone(),
                },
                host: host.clone(),
            }));
            (score, host)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.validator_account_id.cmp(&right.validator_account_id))
            .then_with(|| left.peer_id.cmp(&right.peer_id))
            .then_with(|| left.lane_id.cmp(&right.lane_id))
    });
    let failover_started_at = message.available_after_height.max(pulse.height);
    let elapsed = current_height.saturating_sub(failover_started_at);
    let rank = usize::try_from(elapsed / EXECUTOR_FAILOVER_GRACE_BLOCKS).unwrap_or(usize::MAX);
    let selected_rank = rank.min(ranked.len().saturating_sub(1));
    ranked
        .into_iter()
        .nth(selected_rank)
        .map(|(_score, host)| host)
}
/// Compute the canonical commitment to every authoritative effect of an ordered mailbox result.
///
/// Observation height/sequence are optimistic-concurrency preconditions, not result identity.
/// The ledger-assigned receipt sequence is likewise deliberately excluded.
#[must_use]
pub fn ordered_mailbox_result_commitment(result: &SoraOrderedMailboxResultV1) -> Hash {
    let mutation_fingerprints = result
        .state_mutations
        .iter()
        .map(|mutation| OrderedMailboxMutationFingerprintV1 {
            schema_version: mutation.schema_version,
            binding_name: mutation.binding_name.clone(),
            state_key: mutation.state_key.clone(),
            operation: mutation.operation,
            encryption: mutation.encryption,
            value_payload: mutation.value_payload.as_ref().map(|payload| {
                OrderedMailboxPayloadFingerprintV1 {
                    payload_bytes: u64::try_from(payload.len()).unwrap_or(u64::MAX),
                    payload_commitment: Hash::new(payload),
                }
            }),
        })
        .collect::<Vec<_>>();
    let outbound_fingerprints = result
        .outbound_mailbox_messages
        .iter()
        .map(|message| OrderedMailboxMessageFingerprintV1 {
            schema_version: message.schema_version,
            message_id: message.message_id,
            from_service: message.from_service.clone(),
            from_service_version: message.from_service_version.clone(),
            from_handler: message.from_handler.clone(),
            to_service: message.to_service.clone(),
            to_service_version: message.to_service_version.clone(),
            to_handler: message.to_handler.clone(),
            payload_commitment: message.payload_commitment,
            delivery_delay_blocks: message.delivery_delay_blocks,
            enqueue_sequence: message.enqueue_sequence,
            enqueue_height: message.enqueue_height,
            available_after_height: message.available_after_height,
            expires_at_height: message.expires_at_height,
        })
        .collect::<Vec<_>>();
    let receipt = &result.runtime_receipt;
    let runtime_state_fingerprint =
        result
            .runtime_state
            .as_ref()
            .map(|state| OrderedMailboxRuntimeStateFingerprintV1 {
                schema_version: state.schema_version,
                service_name: state.service_name.clone(),
                active_service_version: state.active_service_version.clone(),
                health_status: state.health_status,
                load_factor_bps: state.load_factor_bps,
                materialized_bundle_hash: state.materialized_bundle_hash,
            });
    Hash::new(Encode::encode(&OrderedMailboxResultPreimageV1 {
        domain: "soracloud:ordered-mailbox-result:v1".to_owned(),
        receipt: OrderedMailboxReceiptFingerprintV1 {
            mailbox_message_id: receipt.mailbox_message_id,
            service_name: receipt.service_name.clone(),
            service_version: receipt.service_version.clone(),
            handler_name: receipt.handler_name.clone(),
            handler_class: receipt.handler_class,
            request_commitment: receipt.request_commitment,
            certified_by: receipt.certified_by,
            execution_host: receipt.execution_host.clone(),
            journal_artifact_hash: receipt.journal_artifact_hash,
            checkpoint_artifact_hash: receipt.checkpoint_artifact_hash,
        },
        state_mutations: mutation_fingerprints,
        outbound_mailbox_messages: outbound_fingerprints,
        response_commitment: result.response_commitment,
        runtime_execution_commitment: result.runtime_execution_commitment,
        content_type: result.content_type.clone(),
        runtime_state: runtime_state_fingerprint,
    }))
}
/// Derive the sequence-independent identifier for an ordered mailbox receipt.
///
/// The identifier binds every immutable receipt field while excluding only the identifier itself
/// and the ledger-assigned emission sequence. Binding the full retained receipt directly keeps
/// snapshot validation self-contained after the ordered-result preimage has been discarded.
#[must_use]
pub fn ordered_mailbox_runtime_receipt_id(receipt: &SoraRuntimeReceiptV1) -> Option<Hash> {
    let message_id = receipt.mailbox_message_id?;
    Some(Hash::new(Encode::encode(
        &OrderedMailboxReceiptIdentityPreimageV1 {
            domain: "soracloud:ordered-mailbox-receipt:v1".to_owned(),
            schema_version: receipt.schema_version,
            mailbox_message_id: message_id,
            service_name: receipt.service_name.clone(),
            service_version: receipt.service_version.clone(),
            handler_name: receipt.handler_name.clone(),
            handler_class: receipt.handler_class,
            request_commitment: receipt.request_commitment,
            result_commitment: receipt.result_commitment,
            certified_by: receipt.certified_by,
            execution_host: receipt.execution_host.clone(),
            journal_artifact_hash: receipt.journal_artifact_hash,
            checkpoint_artifact_hash: receipt.checkpoint_artifact_hash,
        },
    )))
}

/// Derive the sequence-independent identifier from a validated ordered mailbox result.
#[must_use]
pub fn ordered_mailbox_receipt_id(result: &SoraOrderedMailboxResultV1) -> Hash {
    ordered_mailbox_runtime_receipt_id(&result.runtime_receipt)
        .expect("a validated ordered mailbox result must reference its source message")
}

/// Validate the exact authoritative lease-volume economics for one admitted service bundle.
///
/// Deployment state is the source used for prepaid storage accounting, so accepting missing,
/// extra, or altered rows would let routing and placement observe economics that differ from the
/// admitted manifest. This cross-record invariant deliberately has no compatibility fallback.
pub fn validate_soracloud_deployment_lease_volume_bindings(
    deployment: &SoraServiceDeploymentStateV1,
    bundle: &SoraDeploymentBundleV1,
) -> Result<(), String> {
    if deployment.service_name != bundle.service.service_name {
        return Err(format!(
            "deployment service `{}` does not match admitted bundle service `{}`",
            deployment.service_name, bundle.service.service_name
        ));
    }
    let mut declared_names = BTreeSet::new();
    for binding in &bundle.service.lease_volumes {
        if !declared_names.insert(binding.volume_name.clone()) {
            return Err(format!(
                "service `{}` admitted bundle contains duplicate lease-volume binding `{}`",
                deployment.service_name, binding.volume_name
            ));
        }
    }
    let mut authoritative_names = BTreeSet::new();
    for state in &deployment.lease_volume_states {
        if !authoritative_names.insert(state.volume_name.clone()) {
            return Err(format!(
                "service `{}` contains duplicate authoritative lease-volume state `{}`",
                deployment.service_name, state.volume_name
            ));
        }
    }
    if declared_names != authoritative_names {
        let missing = declared_names
            .difference(&authoritative_names)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let unexpected = authoritative_names
            .difference(&declared_names)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        return Err(format!(
            "service `{}` requires exact 1:1 admitted-to-authoritative lease-volume state; missing {missing:?}, unexpected {unexpected:?}",
            deployment.service_name
        ));
    }
    for binding in &bundle.service.lease_volumes {
        let Some(state) = deployment
            .lease_volume_states
            .iter()
            .find(|state| state.volume_name == binding.volume_name)
        else {
            return Err(format!(
                "service `{}` is missing authoritative lease-volume state for admitted binding `{}`",
                deployment.service_name, binding.volume_name
            ));
        };
        if state.authoritative_generation != 1 {
            return Err(format!(
                "service `{}` authoritative lease-volume state `{}` field `authoritative_generation` must equal 1 in the first-release protocol",
                deployment.service_name, binding.volume_name
            ));
        }
        for (field, matches) in [
            ("kind", state.kind == binding.kind),
            (
                "storage_class",
                state.storage_class == binding.storage_class,
            ),
            ("mount_path", state.mount_path == binding.mount_path),
            (
                "max_total_bytes",
                state.max_total_bytes == binding.max_total_bytes.get(),
            ),
        ] {
            if !matches {
                return Err(format!(
                    "service `{}` authoritative lease-volume state `{}` field `{field}` does not exactly match its admitted binding",
                    deployment.service_name, binding.volume_name
                ));
            }
        }
    }
    Ok(())
}

/// Validate the immutable identity shared by every admitted revision of one service.
///
/// A revision changes implementation, not execution, routing, or durable-state identity. Retained
/// revisions are rollback targets, so this invariant applies to upgrades, rollback, and restored
/// state alike.
/// The first release deliberately has no compatibility fallback for older revision shapes.
pub fn validate_soracloud_service_revision_identity(
    current: &SoraDeploymentBundleV1,
    candidate: &SoraDeploymentBundleV1,
) -> Result<(), String> {
    if candidate.service.service_name != current.service.service_name {
        return Err(format!(
            "service revision identity mismatch: candidate service `{}` does not match current service `{}`",
            candidate.service.service_name, current.service.service_name
        ));
    }
    if candidate.service.execution_plane != current.service.execution_plane {
        return Err(
            "service revision cannot change execution_plane; deploy a distinct service identity"
                .to_owned(),
        );
    }
    if candidate.container.runtime != current.container.runtime {
        return Err(
            "service revision cannot change container runtime; deploy a distinct service identity"
                .to_owned(),
        );
    }
    if candidate.service.route != current.service.route {
        return Err(
            "service revision cannot change route identity; deploy a distinct service identity"
                .to_owned(),
        );
    }
    if candidate.service.lease_volumes != current.service.lease_volumes {
        return Err(
            "service revision cannot change lease-volume identity or economics; deploy a distinct service identity"
                .to_owned(),
        );
    }
    if candidate.service.state_bindings != current.service.state_bindings {
        return Err(
            "service revision cannot change durable state-binding contracts; deploy a distinct service identity"
                .to_owned(),
        );
    }
    Ok(())
}
/// Resolve one authoritative Inrou placement record through its exact active deployment binding.
///
/// Missing or inactive state resolves to `None`. Malformed records and cross-keyed authoritative
/// state are errors so callers can fail closed and reconciliation can repair the row.
pub fn resolve_active_inrou_placement_record(
    world: &impl WorldReadOnly,
    service_name: &str,
    service_version: &str,
    current_height: u64,
) -> Result<Option<SoraInrouServicePlacementRecordV1>, String> {
    let service_name_id = service_name.parse::<Name>().map_err(|error| {
        format!(
            "Inrou placement storage key contains invalid service name `{service_name}`: {error}"
        )
    })?;
    let key = (service_name.to_owned(), service_version.to_owned());
    let Some(record) = world
        .soracloud_inrou_service_placements()
        .get(&key)
        .cloned()
    else {
        return Ok(None);
    };
    record.validate().map_err(|error| {
        format!(
            "Inrou placement for service `{service_name}` revision `{service_version}` is malformed: {error}"
        )
    })?;
    if record.service_name != service_name_id || record.service_version != service_version {
        return Err(format!(
            "Inrou placement storage key `{service_name}`/`{service_version}` does not match embedded service `{}` revision `{}`",
            record.service_name, record.service_version
        ));
    }

    let Some(deployment) = world.soracloud_service_deployments().get(&service_name_id) else {
        return Ok(None);
    };
    deployment.validate().map_err(|error| {
        format!(
            "Inrou placement for service `{service_name}` revision `{service_version}` references a malformed deployment: {error}"
        )
    })?;
    if deployment.service_name != service_name_id {
        return Err(format!(
            "Inrou placement for service `{service_name}` revision `{service_version}` references a deployment whose embedded service is `{}`",
            deployment.service_name
        ));
    }
    let version_is_active = deployment.active_rollout.as_ref().map_or_else(
        || deployment.current_service_version == service_version,
        |rollout| {
            rollout.baseline_version == service_version
                || rollout.candidate_version == service_version
        },
    );
    if !version_is_active {
        return Ok(None);
    }

    let Some(bundle) = world.soracloud_service_revisions().get(&key) else {
        return Err(format!(
            "active Inrou placement for service `{service_name}` revision `{service_version}` has no admitted deployment bundle"
        ));
    };
    bundle.validate_for_admission().map_err(|error| {
        format!(
            "active Inrou placement for service `{service_name}` revision `{service_version}` references a malformed deployment bundle: {error}"
        )
    })?;
    if bundle.service.service_name != service_name_id
        || bundle.service.service_version != service_version
    {
        return Err(format!(
            "Inrou revision storage key `{service_name}`/`{service_version}` does not match embedded service `{}` revision `{}`",
            bundle.service.service_name, bundle.service.service_version
        ));
    }
    if bundle.container.runtime != SoraContainerRuntimeV1::Inrou
        || bundle.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService
    {
        return Err(format!(
            "active Inrou placement for service `{service_name}` revision `{service_version}` is not bound to an Inrou HTTP-service bundle"
        ));
    }
    validate_soracloud_deployment_lease_volume_bindings(deployment, bundle).map_err(|error| {
        format!(
            "active Inrou placement for service `{service_name}` revision `{service_version}` has noncanonical lease-volume economics: {error}"
        )
    })?;
    if !deployment
        .hosted_service_lease_active_at(current_height)
        .map_err(|error| {
            format!(
                "Inrou placement lifecycle for service `{service_name}` revision `{service_version}` could not be calculated: {error}"
            )
        })?
    {
        return Ok(None);
    }
    if deployment.lease_volume_states.iter().any(|volume| {
        current_height < volume.lease_started_height
            || current_height >= volume.lease_expires_height
    }) {
        return Ok(None);
    }
    if record.desired_replica_count != bundle.service.replicas.get() {
        return Err(format!(
            "Inrou placement for service `{service_name}` revision `{service_version}` declares {} replicas but the admitted bundle declares {}",
            record.desired_replica_count,
            bundle.service.replicas.get()
        ));
    }
    if service_version == deployment.current_service_version
        && (deployment.current_service_manifest_hash != bundle.service_manifest_hash()
            || deployment.current_container_manifest_hash != bundle.container_manifest_hash())
    {
        return Err(format!(
            "active Inrou placement for service `{service_name}` revision `{service_version}` does not match the deployment's admitted manifest hashes"
        ));
    }
    Ok(Some(record))
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ActiveInrouReservationUsage {
    hosted_replicas: u32,
    cpu_millis: u64,
    memory_bytes: u64,
    storage_bytes: u64,
}
fn inrou_bundle_per_replica_storage_bytes(bundle: &SoraDeploymentBundleV1) -> Option<u64> {
    bundle
        .service
        .lease_volumes
        .iter()
        .filter(|volume| volume.kind.is_per_replica())
        .try_fold(
            bundle.container.resources.ephemeral_storage_bytes.get(),
            |total, volume| total.checked_add(volume.max_total_bytes.get()),
        )
}
fn active_inrou_reservation_usage_by_validator(
    world: &impl WorldReadOnly,
    current_height: u64,
) -> Result<BTreeMap<AccountId, ActiveInrouReservationUsage>, String> {
    let mut usage_by_validator = BTreeMap::new();
    for ((service_name, service_version), _record) in
        world.soracloud_inrou_service_placements().iter()
    {
        let Some(record) = resolve_active_inrou_placement_record(
            world,
            service_name,
            service_version,
            current_height,
        )?
        else {
            continue;
        };
        let bundle = world
            .soracloud_service_revisions()
            .get(&(
                service_name.as_str().to_owned(),
                service_version.as_str().to_owned(),
            ))
            .ok_or_else(|| {
                format!(
                    "active Inrou placement for service `{service_name}` revision `{service_version}` lost its admitted deployment bundle"
                )
            })?;
        let cpu_millis = u64::from(bundle.container.resources.cpu_millis.get());
        let memory_bytes = bundle.container.resources.memory_bytes.get();
        let storage_bytes = inrou_bundle_per_replica_storage_bytes(bundle).ok_or_else(|| {
            format!(
                "active Inrou per-replica storage reservation overflows for service `{service_name}` revision `{service_version}`"
            )
        })?;
        for assignment in record.placements {
            let validator_account_id = assignment.validator_account_id;
            let usage = usage_by_validator
                .entry(validator_account_id.clone())
                .or_insert_with(ActiveInrouReservationUsage::default);
            usage.hosted_replicas = usage.hosted_replicas.checked_add(1).ok_or_else(|| {
                format!(
                    "active Inrou replica reservations overflow for validator `{validator_account_id}`"
                )
            })?;
            usage.cpu_millis = usage.cpu_millis.checked_add(cpu_millis).ok_or_else(|| {
                format!(
                    "active Inrou CPU reservations overflow for validator `{validator_account_id}`"
                )
            })?;
            usage.memory_bytes = usage.memory_bytes.checked_add(memory_bytes).ok_or_else(|| {
                format!(
                    "active Inrou memory reservations overflow for validator `{validator_account_id}`"
                )
            })?;
            usage.storage_bytes = usage.storage_bytes.checked_add(storage_bytes).ok_or_else(|| {
                format!(
                    "active Inrou storage reservations overflow for validator `{validator_account_id}`"
                )
            })?;
        }
    }
    Ok(usage_by_validator)
}
fn inrou_replica_assignment_has_active_capability(
    world: &impl WorldReadOnly,
    bundle: &SoraDeploymentBundleV1,
    assignment: &SoraInrouReplicaPlacementV1,
    now_ms: u64,
    lane_is_active_for_authority: impl Fn(LaneId) -> bool,
) -> bool {
    let Some(capability) = world
        .soracloud_inrou_host_capabilities()
        .get(&assignment.validator_account_id)
    else {
        return false;
    };
    let Some(required_storage_bytes) = inrou_bundle_per_replica_storage_bytes(bundle) else {
        return false;
    };
    capability.validate().is_ok()
        && capability.validator_account_id == assignment.validator_account_id
        && capability.peer_id == assignment.peer_id
        && capability.can_host_replicas_at(now_ms)
        && capability
            .supported_guest_isas
            .contains(&assignment.selected_guest_isa)
        && bundle.container.inrou.as_ref().is_some_and(|inrou| {
            inrou
                .guest_images
                .contains_key(&assignment.selected_guest_isa)
        })
        && u64::from(capability.max_cpu_millis)
            >= u64::from(bundle.container.resources.cpu_millis.get())
        && capability.max_memory_bytes >= bundle.container.resources.memory_bytes.get()
        && capability.max_storage_bytes >= required_storage_bytes
        && soracloud_validator_has_active_peer_binding(
            world,
            &assignment.validator_account_id,
            &assignment.peer_id,
            lane_is_active_for_authority,
        )
}
/// Resolve all exact active replica assignments for an active Inrou placement record.
pub fn resolve_active_inrou_replica_assignments(
    world: &impl WorldReadOnly,
    service_name: &str,
    service_version: &str,
    now_ms: u64,
    current_height: u64,
    lane_is_active_for_authority: impl Fn(LaneId) -> bool,
) -> Result<Vec<SoraInrouReplicaPlacementV1>, String> {
    let Some(record) = resolve_active_inrou_placement_record(
        world,
        service_name,
        service_version,
        current_height,
    )?
    else {
        return Ok(Vec::new());
    };
    let Some(bundle) = world
        .soracloud_service_revisions()
        .get(&(service_name.to_owned(), service_version.to_owned()))
    else {
        return Err(format!(
            "active Inrou placement for service `{service_name}` revision `{service_version}` lost its admitted deployment bundle"
        ));
    };
    let reserved_usage = active_inrou_reservation_usage_by_validator(world, current_height)?;
    Ok(record
        .placements
        .into_iter()
        .filter(|assignment| {
            let aggregate_capacity_matches = world
                .soracloud_inrou_host_capabilities()
                .get(&assignment.validator_account_id)
                .zip(reserved_usage.get(&assignment.validator_account_id))
                .is_some_and(|(capability, usage)| {
                    usage.hosted_replicas <= u32::from(capability.max_hosted_replica_capacity)
                        && usage.cpu_millis <= u64::from(capability.max_cpu_millis)
                        && usage.memory_bytes <= capability.max_memory_bytes
                        && usage.storage_bytes <= capability.max_storage_bytes
                });
            inrou_replica_assignment_has_active_capability(
                world,
                bundle,
                assignment,
                now_ms,
                &lane_is_active_for_authority,
            ) && aggregate_capacity_matches
        })
        .collect())
}
/// Resolve one exact active replica-slot assignment for an active Inrou placement record.
pub fn resolve_active_inrou_replica_assignment(
    world: &impl WorldReadOnly,
    service_name: &str,
    service_version: &str,
    replica_slot: u16,
    now_ms: u64,
    current_height: u64,
    lane_is_active_for_authority: impl Fn(LaneId) -> bool,
) -> Result<Option<SoraInrouReplicaPlacementV1>, String> {
    Ok(resolve_active_inrou_replica_assignments(
        world,
        service_name,
        service_version,
        now_ms,
        current_height,
        lane_is_active_for_authority,
    )?
    .into_iter()
    .find(|assignment| assignment.replica_slot == replica_slot))
}
/// Canonical Hugging Face source markers embedded into generated Soracloud service bundles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudHfGeneratedSourceBinding {
    /// Stable canonical source identifier.
    pub source_id: String,
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision resolved by the control plane.
    pub resolved_revision: String,
    /// Normalized Soracloud model name.
    pub model_name: String,
}
fn hf_generated_entrypoint(name: &str, entry_pc: u64) -> ivm::EmbeddedEntrypointDescriptor {
    ivm::EmbeddedEntrypointDescriptor {
        name: name.to_owned(),
        kind: EntryPointKind::View,
        params: Vec::new(),
        argument_schema: None,
        return_type: None,
        return_schema: None,
        permission: None,
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: Some(true),
        access_hints_skipped: Vec::new(),
        triggers: Vec::new(),
        entry_pc,
    }
}
fn hf_generated_internal_host(service_name: &Name) -> String {
    format!(
        "{}{HF_GENERATED_ROUTE_SUFFIX}",
        service_name.as_ref().replace('_', "-")
    )
}
/// Return the deterministic shared IVM artifact used by generated HF services.
#[must_use]
pub fn soracloud_hf_generated_service_contract_artifact() -> Vec<u8> {
    let metadata = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: iroha_config::parameters::defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND.get(),
        abi_version: 1,
    };
    let contract_interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "SoracloudRuntime".to_owned(),
        compiler_fingerprint: "iroha-soracloud-hf-generated".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: [
            hf_generated_entrypoint(HF_GENERATED_ENTRYPOINT_INFER, 0),
            hf_generated_entrypoint(HF_GENERATED_ENTRYPOINT_METADATA, 4),
        ]
        .into_iter()
        .collect(),
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut bytes = metadata.encode();
    bytes.extend_from_slice(&contract_interface.encode_section());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    bytes
}
/// Lease term used for deterministic HF-generated agent apartments.
pub const HF_GENERATED_AGENT_LEASE_BLOCKS: u64 = 86_400;
/// Autonomy budget applied to deterministic HF-generated agent apartments.
pub const HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS: u64 = 1_000;
/// Build the canonical generated Soracloud service bundle used for HF-backed deployments.
#[must_use]
pub fn build_soracloud_hf_generated_service_bundle(
    service_name: Name,
    source_id: &str,
    repo_id: &str,
    resolved_revision: &str,
    model_name: &str,
) -> SoraDeploymentBundleV1 {
    let bundle_bytes = soracloud_hf_generated_service_contract_artifact();
    let bundle_hash = Hash::new(&bundle_bytes);
    let mut env = BTreeMap::new();
    env.insert(HF_GENERATED_SERVICE_MARKER_ENV.to_owned(), "1".to_owned());
    env.insert(HF_GENERATED_SOURCE_ID_ENV.to_owned(), source_id.to_owned());
    env.insert(HF_GENERATED_REPO_ID_ENV.to_owned(), repo_id.to_owned());
    env.insert(
        HF_GENERATED_REVISION_ENV.to_owned(),
        resolved_revision.to_owned(),
    );
    env.insert(
        HF_GENERATED_MODEL_NAME_ENV.to_owned(),
        model_name.to_owned(),
    );
    let container = SoraContainerManifestV1 {
        schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        runtime: SoraContainerRuntimeV1::Ivm,
        bundle_hash,
        bundle_path: "/bundles/hf_generated_inference.to".to_owned(),
        entrypoint: HF_GENERATED_ENTRYPOINT_INFER.to_owned(),
        args: Vec::new(),
        env,
        inrou: None,
        required_config_names: Vec::new(),
        required_secret_names: Vec::new(),
        config_exports: Vec::new(),
        capabilities: SoraCapabilityPolicyV1 {
            network: SoraNetworkPolicyV1::Isolated,
            allow_wallet_signing: false,
            allow_state_writes: false,
            allow_model_inference: true,
            allow_model_training: false,
        },
        resources: SoraResourceLimitsV1 {
            cpu_millis: NonZeroU32::new(500).expect("non-zero cpu budget"),
            memory_bytes: NonZeroU64::new(256 * 1024 * 1024).expect("non-zero memory budget"),
            ephemeral_storage_bytes: NonZeroU64::new(128 * 1024 * 1024)
                .expect("non-zero storage budget"),
            max_open_files: NonZeroU32::new(256).expect("non-zero open-file cap"),
            max_tasks: NonZeroU16::new(32).expect("non-zero task cap"),
        },
        lifecycle: SoraLifecycleHooksV1 {
            start_grace_secs: NonZeroU32::new(15).expect("non-zero start grace"),
            stop_grace_secs: NonZeroU32::new(10).expect("non-zero stop grace"),
            healthcheck_path: Some("/healthz".to_owned()),
        },
    };
    let container_manifest_hash = Hash::new(Encode::encode(&container));
    let route_host = hf_generated_internal_host(&service_name);
    let service = SoraServiceManifestV1 {
        schema_version: SORA_SERVICE_MANIFEST_VERSION_V1,
        service_name,
        service_version: HF_GENERATED_SERVICE_VERSION_V1.to_owned(),
        execution_plane: SoraServiceExecutionPlaneV1::DeterministicService,
        container: SoraContainerManifestRefV1 {
            manifest_hash: container_manifest_hash,
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        replicas: NonZeroU16::new(1).expect("non-zero replicas"),
        route: Some(SoraRouteTargetV1 {
            host: route_host,
            path_prefix: "/".to_owned(),
            service_port: NonZeroU16::new(8080).expect("non-zero port"),
            visibility: SoraRouteVisibilityV1::Internal,
            tls_mode: SoraTlsModeV1::Disabled,
        }),
        rollout: SoraRolloutPolicyV1 {
            canary_percent: 0,
            max_unavailable_replicas: 0,
            health_window_secs: NonZeroU32::new(30).expect("non-zero health window"),
            automatic_rollback_failures: NonZeroU32::new(1).expect("non-zero rollback failures"),
        },
        economics: SoraHttpServiceEconomicsV1::default(),
        state_bindings: Vec::new(),
        lease_volumes: Vec::new(),
        handlers: vec![
            SoraServiceHandlerV1 {
                handler_name: "infer".parse().expect("valid literal handler name"),
                class: SoraServiceHandlerClassV1::Query,
                entrypoint: HF_GENERATED_ENTRYPOINT_INFER.to_owned(),
                route_path: Some("/infer".to_owned()),
                certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
                mailbox: None,
            },
            SoraServiceHandlerV1 {
                handler_name: "metadata".parse().expect("valid literal handler name"),
                class: SoraServiceHandlerClassV1::Query,
                entrypoint: HF_GENERATED_ENTRYPOINT_METADATA.to_owned(),
                route_path: Some("/metadata".to_owned()),
                certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
                mailbox: None,
            },
        ],
        artifacts: Vec::new(),
    };
    SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    }
}
/// Build the canonical generated agent apartment manifest used for HF-bound agents.
#[must_use]
pub fn build_soracloud_hf_generated_agent_manifest(
    apartment_name: Name,
    service_bundle: &SoraDeploymentBundleV1,
) -> AgentApartmentManifestV1 {
    let service_host = service_bundle
        .service
        .route
        .as_ref()
        .map(|route| route.host.clone())
        .unwrap_or_else(|| hf_generated_internal_host(&service_bundle.service.service_name));
    AgentApartmentManifestV1 {
        schema_version: AGENT_APARTMENT_MANIFEST_VERSION_V1,
        apartment_name,
        container: SoraContainerManifestRefV1 {
            manifest_hash: service_bundle.container_manifest_hash(),
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        tool_capabilities: vec![
            AgentToolCapabilityV1 {
                tool: "soracloud.hf.infer".to_owned(),
                max_invocations_per_epoch: NonZeroU32::new(10_000)
                    .expect("non-zero infer invocation limit"),
                allow_network: true,
                allow_filesystem_write: false,
            },
            AgentToolCapabilityV1 {
                tool: "soracloud.hf.metadata".to_owned(),
                max_invocations_per_epoch: NonZeroU32::new(10_000)
                    .expect("non-zero metadata invocation limit"),
                allow_network: true,
                allow_filesystem_write: false,
            },
        ],
        policy_capabilities: vec![
            "agent.autonomy.allow"
                .parse()
                .expect("valid literal policy capability"),
            "agent.autonomy.run"
                .parse()
                .expect("valid literal policy capability"),
        ],
        spend_limits: Vec::new(),
        state_quota_bytes: NonZeroU64::new(16 * 1024 * 1024).expect("non-zero state quota"),
        network_egress: SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            service_host,
            [443],
        )]),
        upgrade_policy: AgentUpgradePolicyV1::Governed,
    }
}
/// Extract canonical HF source markers from a generated Soracloud service bundle.
#[must_use]
pub fn soracloud_hf_generated_source_binding(
    bundle: &SoraDeploymentBundleV1,
) -> Option<SoracloudHfGeneratedSourceBinding> {
    if bundle.service.service_version != HF_GENERATED_SERVICE_VERSION_V1 {
        return None;
    }
    if bundle.service.execution_plane != SoraServiceExecutionPlaneV1::DeterministicService
        || bundle.container.runtime != SoraContainerRuntimeV1::Ivm
        || !bundle.container.capabilities.allow_model_inference
        || bundle.container.capabilities.allow_state_writes
        || bundle.container.capabilities.allow_model_training
    {
        return None;
    }
    if bundle.container.bundle_hash != Hash::new(soracloud_hf_generated_service_contract_artifact())
    {
        return None;
    }
    let source_id = bundle
        .container
        .env
        .get(HF_GENERATED_SOURCE_ID_ENV)?
        .clone();
    let repo_id = bundle.container.env.get(HF_GENERATED_REPO_ID_ENV)?.clone();
    let resolved_revision = bundle.container.env.get(HF_GENERATED_REVISION_ENV)?.clone();
    let model_name = bundle
        .container
        .env
        .get(HF_GENERATED_MODEL_NAME_ENV)?
        .clone();
    let marker = bundle.container.env.get(HF_GENERATED_SERVICE_MARKER_ENV)?;
    if marker != "1" {
        return None;
    }
    Some(SoracloudHfGeneratedSourceBinding {
        source_id,
        repo_id,
        resolved_revision,
        model_name,
    })
}
/// Return the synthesized HF bundle bytes when a service bundle is one of the canonical generated
/// HF service bundles.
#[must_use]
pub fn soracloud_hf_generated_bundle_payload_if_applicable(
    bundle: &SoraDeploymentBundleV1,
) -> Option<Vec<u8>> {
    soracloud_hf_generated_source_binding(bundle)
        .map(|_binding| soracloud_hf_generated_service_contract_artifact())
}
/// Resolve the authoritative active HF placement serving a generated service binding.
pub fn resolve_generated_hf_active_placement(
    world: &impl WorldReadOnly,
    service_name: &str,
    source_id: &str,
    now_ms: u64,
) -> Result<Option<SoraHfPlacementRecordV1>, String> {
    let mut matching_pool_ids = BTreeSet::new();
    for ((member_pool_id, member_account_id), member) in
        world.soracloud_hf_shared_lease_members().iter()
    {
        if member.status != SoraHfSharedLeaseMemberStatusV1::Active
            || !member.service_bindings.contains(service_name)
            || member.source_id.to_string() != source_id
        {
            continue;
        }
        member.validate().map_err(|error| {
            format!(
                "generated HF service `{service_name}` has a malformed active lease member for source `{source_id}`: {error}"
            )
        })?;
        if member_pool_id != &member.pool_id.to_string()
            || member_account_id != &member.account_id.to_string()
        {
            return Err(format!(
                "generated HF service `{service_name}` has a miskeyed active lease member for source `{source_id}`"
            ));
        }
        let Some(pool) = world.soracloud_hf_shared_lease_pools().get(&member.pool_id) else {
            continue;
        };
        pool.validate().map_err(|error| {
            format!(
                "generated HF service `{service_name}` has a malformed active lease pool for source `{source_id}`: {error}"
            )
        })?;
        if pool.pool_id != member.pool_id {
            return Err(format!(
                "generated HF service `{service_name}` has a miskeyed active lease pool for source `{source_id}`"
            ));
        }
        if pool.source_id == member.source_id
            && pool.window_started_at_ms <= now_ms
            && pool.window_expires_at_ms > now_ms
            && matches!(
                pool.status,
                SoraHfSharedLeaseStatusV1::Active | SoraHfSharedLeaseStatusV1::Draining
            )
        {
            matching_pool_ids.insert(member.pool_id.clone());
        }
    }
    if matching_pool_ids.len() > 1 {
        return Err(format!(
            "generated HF service `{service_name}` is bound to multiple active lease pools for source `{source_id}`"
        ));
    }
    let Some(pool_id) = matching_pool_ids.into_iter().next() else {
        return Ok(None);
    };
    let Some(placement) = world.soracloud_hf_placements().get(&pool_id).cloned() else {
        return Err(format!(
            "generated HF service `{service_name}` is missing an authoritative placement for pool `{pool_id}`"
        ));
    };
    placement.validate().map_err(|error| {
        format!(
            "generated HF service `{service_name}` has a malformed authoritative placement for pool `{pool_id}`: {error}"
        )
    })?;
    if placement.pool_id != pool_id || placement.source_id.to_string() != source_id {
        return Err(format!(
            "generated HF service `{service_name}` authoritative placement does not match pool `{pool_id}` and source `{source_id}`"
        ));
    }
    if !matches!(
        placement.status,
        SoraHfPlacementStatusV1::Ready | SoraHfPlacementStatusV1::Degraded
    ) {
        return Ok(None);
    }
    Ok(Some(placement))
}
/// Return whether an HF placement assignment is backed by the exact active host capability.
#[must_use]
pub fn soracloud_hf_placement_assignment_has_active_capability(
    world: &impl WorldReadOnly,
    placement: &SoraHfPlacementRecordV1,
    assignment: &SoraHfPlacementHostAssignmentV1,
    now_ms: u64,
    lane_is_active_for_authority: impl Fn(LaneId) -> bool,
) -> bool {
    let Some(capability) = world
        .soracloud_model_host_capabilities()
        .get(&assignment.validator_account_id)
    else {
        return false;
    };
    capability.validate().is_ok()
        && capability.is_active_at(now_ms)
        && capability.validator_account_id == assignment.validator_account_id
        && soracloud_validator_has_active_peer_binding(
            world,
            &assignment.validator_account_id,
            &capability.peer_id,
            lane_is_active_for_authority,
        )
        && capability.peer_id == assignment.peer_id
        && capability.host_class == assignment.host_class
        && capability
            .supported_backends
            .contains(&placement.resource_profile.backend_family)
        && capability
            .supported_formats
            .contains(&placement.resource_profile.model_format)
        && capability.max_model_bytes >= placement.resource_profile.required_model_bytes
        && capability.max_disk_cache_bytes >= placement.resource_profile.disk_cache_bytes_floor
        && capability.max_ram_bytes >= placement.resource_profile.ram_bytes_floor
        && capability.max_vram_bytes >= placement.resource_profile.vram_bytes_floor
        && capability.max_concurrent_resident_models > 0
}
/// Resolve the current authoritative primary host for a generated HF service.
pub fn resolve_generated_hf_primary_assignment(
    world: &impl WorldReadOnly,
    service_name: &str,
    source_id: &str,
    now_ms: u64,
    lane_is_active_for_authority: impl Fn(LaneId) -> bool,
) -> Result<Option<SoraHfPlacementHostAssignmentV1>, String> {
    let Some(placement) =
        resolve_generated_hf_active_placement(world, service_name, source_id, now_ms)?
    else {
        return Ok(None);
    };
    let Some(assignment) = placement
        .assigned_hosts
        .iter()
        .find(|assignment| {
            assignment.role == SoraHfPlacementHostRoleV1::Primary
                && assignment.status == SoraHfPlacementHostStatusV1::Warm
        })
        .cloned()
    else {
        return Ok(None);
    };
    if !soracloud_hf_placement_assignment_has_active_capability(
        world,
        &placement,
        &assignment,
        now_ms,
        lane_is_active_for_authority,
    ) {
        return Ok(None);
    }
    Ok(Some(assignment))
}
/// Distinguishes the local runtime role of a materialized service revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "revision_role", content = "value")]
#[norito(deny_unknown_fields)]
pub enum SoracloudRuntimeRevisionRole {
    /// The currently active deployment revision.
    Active,
    /// A canary candidate revision that must be materialized during rollout.
    CanaryCandidate,
}
/// Node-local mailbox materialization metadata for a handler.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeMailboxPlan {
    /// Stable handler identifier.
    pub handler_name: String,
    /// Stable logical queue name.
    pub queue_name: String,
    /// Maximum retained pending messages.
    pub max_pending_messages: u32,
    /// Maximum message size.
    pub max_message_bytes: u64,
    /// Retention bound in consensus blocks.
    pub retention_blocks: u32,
}
/// Node-local hydration/materialization metadata for a referenced artifact.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeArtifactPlan {
    /// Artifact class.
    pub kind: SoraArtifactKindV1,
    /// Content-addressed artifact digest.
    pub artifact_hash: String,
    /// Logical artifact path inside the service revision.
    pub artifact_path: String,
    /// Optional consuming handler.
    #[norito(required)]
    pub handler_name: Option<String>,
    /// Local cache path where the runtime manager expects the artifact.
    pub local_cache_path: String,
    /// Whether the artifact is already present in the node-local cache.
    pub available_locally: bool,
}
/// Node-local materialization plan for one active service revision.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeServicePlan {
    /// Service identifier.
    pub service_name: String,
    /// Materialized revision/version.
    pub service_version: String,
    /// Whether this revision is the active one or a rollout candidate.
    pub role: SoracloudRuntimeRevisionRole,
    /// Requested traffic percentage for this revision.
    pub traffic_percent: u8,
    /// Runtime target.
    pub runtime: SoraContainerRuntimeV1,
    /// Execution plane selected for this revision.
    pub execution_plane: SoraServiceExecutionPlaneV1,
    /// Bundle digest.
    pub bundle_hash: String,
    /// Bundle path declared by the container manifest.
    pub bundle_path: String,
    /// Entrypoint declared by the container manifest.
    pub entrypoint: String,
    /// Explicit Inrou VM metadata projected for hosted HTTP microVMs.
    #[norito(required)]
    pub inrou: Option<SoracloudRuntimeInrouPlan>,
    /// Node-local cache path for the executable bundle.
    pub bundle_cache_path: String,
    /// Whether the bundle is already present locally.
    pub bundle_available_locally: bool,
    /// Current deployment process generation when known for this revision.
    #[norito(required)]
    pub process_generation: Option<u64>,
    /// Desired replica count declared by the admitted service manifest.
    pub desired_replica_count: u16,
    /// Replica slots this runtime host is projecting locally for the revision.
    pub local_replica_slots: Vec<u16>,
    /// Replica-local runtime topology currently projected on this host for the revision.
    pub local_replicas: Vec<SoracloudRuntimeReplicaPlan>,
    /// Current runtime health projection.
    pub health_status: SoraServiceHealthStatusV1,
    /// Current runtime load projection.
    pub load_factor_bps: u16,
    /// Pending mailbox messages currently stored in authoritative state.
    pub authoritative_pending_mailbox_messages: u32,
    /// Active rollout handle when this revision is part of a canary rollout.
    #[norito(required)]
    pub rollout_handle: Option<String>,
    /// Monotonic generation of committed service config updates.
    pub config_generation: u64,
    /// Monotonic generation of committed service secret updates.
    pub secret_generation: u64,
    /// Hosted-service quota class when the service uses the HTTP plane.
    #[norito(required)]
    pub quota_class: Option<String>,
    /// Effective hosted-service lease status at the observed consensus height.
    #[norito(required)]
    pub service_lease_status: Option<SoraServiceLeaseStatusV1>,
    /// Consensus height when hosted-service routing/materialization expires.
    #[norito(required)]
    pub lease_expires_height: Option<u64>,
    /// Remaining prepaid runtime balance estimated at snapshot build time.
    #[norito(required)]
    pub remaining_runtime_balance: Option<Quantity>,
    /// Number of committed service config entries projected into runtime materialization.
    pub config_entry_count: u32,
    /// Number of committed service secret entries projected into runtime materialization.
    pub secret_entry_count: u32,
    /// Explicit config exports declared by the admitted container manifest.
    pub config_exports: Vec<SoraConfigExportV1>,
    /// Whether ordinary handlers on this revision can read authoritative config payloads.
    pub supports_host_read_config: bool,
    /// Whether ordinary handlers on this revision can read authoritative secret envelopes.
    pub supports_host_read_secret_envelope: bool,
    /// Whether this revision exposes at least one handler allowed to read raw secret payload bytes.
    pub supports_private_secret_payload_reads: bool,
    /// Local directory where the revision plan is materialized.
    pub materialization_dir: String,
    /// Local directory containing canonical JSON config files for this revision.
    pub config_materialization_dir: String,
    /// Effective launch environment after applying explicit config env exports.
    pub effective_env: BTreeMap<String, String>,
    /// Local file containing the effective launch environment projection.
    pub effective_env_materialization_path: String,
    /// Local directory containing explicit config file exports for this revision.
    pub config_exports_materialization_dir: String,
    /// Local directory containing committed secret-envelope files for this revision.
    pub secret_envelopes_materialization_dir: String,
    /// Lease-backed mutable storage materialized for this revision.
    pub lease_volumes: Vec<SoracloudRuntimeLeaseVolumePlan>,
    /// Declared replicated handler mailboxes.
    pub mailboxes: Vec<SoracloudRuntimeMailboxPlan>,
    /// Referenced artifacts that still need local hydration.
    pub artifacts: Vec<SoracloudRuntimeArtifactPlan>,
}
/// Node-local materialization plan for one Inrou microVM guest.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeInrouPlan {
    /// Guest userspace profile expected by the VM image.
    pub guest_os: SoraInrouGuestOsV1,
    /// Guest ISA profile selected locally for this replica.
    pub selected_guest_isa: SoraInrouGuestIsaV1,
    /// Kernel image path for the selected guest ISA inside the hydrated Soracloud bundle.
    pub kernel_image_path: String,
    /// Immutable base root filesystem image path for the selected guest ISA.
    pub rootfs_image_path: String,
    /// Optional initrd image path for the selected guest ISA.
    #[norito(required)]
    pub initrd_image_path: Option<String>,
    /// Optional bootstrap user-data overlay path inside the hydrated Soracloud bundle.
    #[norito(required)]
    pub bootstrap_user_data_path: Option<String>,
    /// SSH public keys injected into the guest bootstrap seed.
    pub ssh_authorized_keys: Vec<String>,
    /// Logical volume identifier used as the authoritative mutable root disk.
    pub root_volume_name: String,
}
/// Node-local materialization plan for one lease-backed service volume.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeLeaseVolumePlan {
    /// Logical volume identifier.
    pub volume_name: String,
    /// Soracloud lease-backed volume kind.
    pub kind: SoraLeaseVolumeKindV1,
    /// Requested Sorafs storage class.
    pub storage_class: StorageClass,
    /// Declared in-runtime mount path.
    pub mount_path: String,
    /// Maximum logical bytes retained for this volume.
    pub max_total_bytes: u64,
    /// Consensus height when the authoritative volume lease expires.
    pub lease_expires_height: u64,
    /// Monotonic generation of the authoritative lease binding.
    pub authoritative_generation: u64,
    /// Node-local materialization directory used by the current host.
    ///
    /// `PersistentRootLeaseVolume` plans use a per-revision/per-replica namespace, while non-root
    /// service volumes are shared across replicas of the same revision by default.
    pub local_materialization_dir: String,
}
/// Node-local runtime topology projected for one hosted-HTTP replica slot.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeReplicaPlan {
    /// One-based replica slot within the revision.
    pub replica_slot: u16,
    /// Local directory where this replica slot is materialized.
    pub materialization_dir: String,
    /// Current runtime health projection for this replica slot.
    pub health_status: SoraServiceHealthStatusV1,
    /// Loopback listener currently exposed by this replica, when healthy.
    #[norito(required)]
    pub listen_base_url: Option<String>,
    /// Local process identifier when the replica is running.
    #[norito(required)]
    pub pid: Option<u32>,
    /// Human-readable startup or healthcheck failure detail for the replica, when present.
    #[norito(required)]
    pub last_error: Option<String>,
}
/// Node-local materialization plan for an active agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeApartmentPlan {
    /// Apartment identifier.
    pub apartment_name: String,
    /// Canonical manifest hash.
    pub manifest_hash: String,
    /// Current runtime status.
    pub status: SoraAgentRuntimeStatusV1,
    /// Current process generation.
    pub process_generation: u64,
    /// Consensus height when the lease expires.
    pub lease_expires_height: u64,
    /// Audit sequence of the most recent observed activity.
    pub last_active_sequence: u64,
    /// Node-local directory where the apartment plan is materialized.
    pub materialization_dir: String,
    /// Number of pending wallet approvals.
    pub pending_wallet_request_count: u32,
    /// Number of queued mailbox messages.
    pub pending_mailbox_message_count: u32,
    /// Remaining autonomy budget.
    pub autonomy_budget_remaining_units: u64,
    /// Number of explicitly approved autonomy artifacts.
    pub approved_artifact_count: u32,
    /// Number of recorded autonomy runs.
    pub autonomy_run_count: u32,
    /// Number of revoked policy capabilities.
    pub revoked_policy_capability_count: u32,
}
/// Schema version for persisted hosted-HTTP runtime state snapshots.
pub const SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_VERSION_V1: u16 = 1;
/// Canonical runtime-state filename written beside hosted-HTTP service materializations.
pub const SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_FILE_V1: &str = "hosted_http_runtime.json";
/// Node-local state projected for one hosted-HTTP replica materialized by the local runtime manager.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudHostedHttpReplicaRuntimeStateV1 {
    /// One-based replica slot for the revision-local materialization.
    pub replica_slot: u16,
    /// Projected health state of the replica runtime.
    pub health_status: SoraServiceHealthStatusV1,
    /// Base URL for the loopback listener exposed by the replica process, when present.
    #[norito(required)]
    pub listen_base_url: Option<String>,
    /// Child process identifier while the replica is running.
    #[norito(required)]
    pub pid: Option<u32>,
    /// Human-readable startup or healthcheck failure detail for this replica, when present.
    #[norito(required)]
    pub last_error: Option<String>,
    /// Timestamp when the replica state was last refreshed.
    pub updated_at_ms: u64,
}
/// Node-local state projected for a supervised hosted-HTTP Soracloud service revision.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudHostedHttpRuntimeStateV1 {
    /// Schema version for this runtime-state document.
    pub schema_version: u16,
    /// Service identifier.
    pub service_name: String,
    /// Materialized revision/version.
    pub service_version: String,
    /// Process generation currently hosted by the local runtime-manager.
    pub process_generation: u64,
    /// Projected health state of the child process.
    pub health_status: SoraServiceHealthStatusV1,
    /// Base URL for the loopback listener exposed by the child process.
    #[norito(required)]
    pub listen_base_url: Option<String>,
    /// Child process identifier while the process is running.
    #[norito(required)]
    pub pid: Option<u32>,
    /// Total authoritative egress bytes accounted by the local supervisor.
    pub accounted_egress_bytes: u64,
    /// Healthy and unhealthy replica listeners currently materialized on this host for the revision.
    pub replicas: Vec<SoracloudHostedHttpReplicaRuntimeStateV1>,
    /// Human-readable startup or healthcheck failure detail, when present.
    #[norito(required)]
    pub last_error: Option<String>,
    /// Timestamp when the state file was last refreshed.
    pub updated_at_ms: u64,
}
/// Schema version for [`SoracloudApartmentAutonomyExecutionSummaryV1`].
pub const SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1: u16 = 1;
/// Maximum canonical JSON bytes persisted for one V1 apartment autonomy summary.
///
/// Runtime writers and every local control-plane reader share this ceiling so
/// corrupted state cannot turn a status read into an unbounded allocation.
pub const SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_MAX_BYTES_V1: u64 = 16 * 1024 * 1024;
/// One successful service step executed inside a generated apartment autonomy workflow.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudApartmentAutonomyWorkflowStepSummaryV1 {
    /// Zero-based workflow step index.
    pub step_index: u32,
    /// Optional stable workflow step identifier.
    #[norito(required)]
    pub step_id: Option<String>,
    /// Deterministic commitment over the local-read request for this step.
    pub request_commitment: Hash,
    /// Deterministic commitment over the step result.
    pub result_commitment: Hash,
    /// Optional certified runtime receipt emitted by the bound service query.
    #[norito(required)]
    pub runtime_receipt: Option<SoraRuntimeReceiptV1>,
    /// Response content type reported by the bound service, when available.
    #[norito(required)]
    pub content_type: Option<String>,
    /// Parsed JSON response body for successful JSON results, when available.
    #[norito(required)]
    pub response_json: Option<norito::json::Value>,
    /// UTF-8 response text for non-JSON results, when available.
    #[norito(required)]
    pub response_text: Option<String>,
}
/// Node-local execution summary for one approved apartment autonomy run.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudApartmentAutonomyExecutionSummaryV1 {
    /// Schema version; must equal [`SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1`].
    pub schema_version: u16,
    /// Apartment that executed the run.
    pub apartment_name: String,
    /// Stable authoritative autonomy-run identifier.
    pub run_id: String,
    /// Bound service name used for execution, when one was resolved locally.
    #[norito(required)]
    pub service_name: Option<String>,
    /// Bound service version used for execution, when one was resolved locally.
    #[norito(required)]
    pub service_version: Option<String>,
    /// Service handler used for execution, when one was resolved locally.
    #[norito(required)]
    pub handler_name: Option<String>,
    /// Whether the runtime obtained a successful inference result.
    pub succeeded: bool,
    /// Deterministic commitment over the execution outcome.
    pub result_commitment: Hash,
    /// Optional raw checkpoint artifact hash produced by the run.
    #[norito(required)]
    pub checkpoint_artifact_hash: Option<Hash>,
    /// Optional certified runtime receipt emitted by the bound service query.
    #[norito(required)]
    pub runtime_receipt: Option<SoraRuntimeReceiptV1>,
    /// Successful workflow steps executed before the final response was produced.
    pub workflow_steps: Vec<SoracloudApartmentAutonomyWorkflowStepSummaryV1>,
    /// Response content type reported by the bound service, when available.
    #[norito(required)]
    pub content_type: Option<String>,
    /// Parsed JSON response body for successful JSON results, when available.
    #[norito(required)]
    pub response_json: Option<norito::json::Value>,
    /// UTF-8 response text for non-JSON results, when available.
    #[norito(required)]
    pub response_text: Option<String>,
    /// Human-readable execution error, when the run failed locally.
    #[norito(required)]
    pub error: Option<String>,
}
/// Error returned when a persisted autonomy execution summary is not bound to
/// the exact authoritative V1 run that owns its storage path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudApartmentAutonomySummaryValidationError(String);
impl std::fmt::Display for SoracloudApartmentAutonomySummaryValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for SoracloudApartmentAutonomySummaryValidationError {}
/// Derive the V1 commitment for a node-local apartment autonomy summary.
///
/// `process_generation` and `request_commitment` must come from authoritative
/// apartment state rather than from the node-local summary document. This
/// keeps summary reuse and control-plane reads bound to the approved run. The
/// preimage covers every summary field except the circular
/// `result_commitment` field itself.
pub fn derive_soracloud_apartment_autonomy_result_commitment_v1(
    summary: &SoracloudApartmentAutonomyExecutionSummaryV1,
    process_generation: u64,
    request_commitment: Hash,
) -> Result<Hash, norito::json::Error> {
    let workflow_steps_commitment = summary
        .workflow_steps
        .iter()
        .map(|step| {
            Ok((
                step.step_index,
                step.step_id.as_deref(),
                step.request_commitment,
                step.result_commitment,
                step.runtime_receipt
                    .as_ref()
                    .map(|receipt| Encode::encode(receipt)),
                step.content_type.as_deref(),
                step.response_text.as_deref(),
                step.response_json
                    .as_ref()
                    .map(norito::json::to_string)
                    .transpose()?,
            ))
        })
        .collect::<Result<Vec<_>, norito::json::Error>>()?;
    let runtime_receipt = summary
        .runtime_receipt
        .as_ref()
        .map(|receipt| Encode::encode(receipt));
    Ok(Hash::new(Encode::encode(&(
        "soracloud.apartment.autonomy.v1",
        (
            summary.schema_version,
            summary.apartment_name.as_str(),
            process_generation,
            summary.run_id.as_str(),
            request_commitment,
        ),
        (
            summary.service_name.as_deref(),
            summary.service_version.as_deref(),
            summary.handler_name.as_deref(),
            summary.succeeded,
        ),
        (
            summary.checkpoint_artifact_hash,
            runtime_receipt,
            workflow_steps_commitment,
        ),
        (
            summary.content_type.as_deref(),
            summary.response_text.as_deref(),
            summary
                .response_json
                .as_ref()
                .map(norito::json::to_string)
                .transpose()?,
            summary.error.as_deref(),
        ),
    ))))
}
/// Validate a persisted V1 apartment autonomy summary against its storage path
/// and the authoritative apartment/run values used to derive its commitment.
pub fn validate_soracloud_apartment_autonomy_execution_summary_v1(
    summary: &SoracloudApartmentAutonomyExecutionSummaryV1,
    expected_apartment_name: &str,
    expected_run_id: &str,
    process_generation: u64,
    request_commitment: Hash,
) -> Result<(), SoracloudApartmentAutonomySummaryValidationError> {
    if summary.schema_version != SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1 {
        return Err(SoracloudApartmentAutonomySummaryValidationError(format!(
            "autonomy summary schema version {} does not match V1",
            summary.schema_version
        )));
    }
    if summary.apartment_name != expected_apartment_name {
        return Err(SoracloudApartmentAutonomySummaryValidationError(format!(
            "autonomy summary apartment `{}` does not match path apartment `{expected_apartment_name}`",
            summary.apartment_name
        )));
    }
    if summary.run_id != expected_run_id {
        return Err(SoracloudApartmentAutonomySummaryValidationError(format!(
            "autonomy summary run `{}` does not match path run `{expected_run_id}`",
            summary.run_id
        )));
    }
    let expected_result_commitment = derive_soracloud_apartment_autonomy_result_commitment_v1(
        summary,
        process_generation,
        request_commitment,
    )
    .map_err(|error| {
        SoracloudApartmentAutonomySummaryValidationError(format!(
            "autonomy summary cannot derive its canonical V1 result commitment: {error}"
        ))
    })?;
    if summary.result_commitment != expected_result_commitment {
        return Err(SoracloudApartmentAutonomySummaryValidationError(format!(
            "autonomy summary result commitment {} does not match authoritative V1 commitment {expected_result_commitment}",
            summary.result_commitment
        )));
    }
    Ok(())
}
/// Runtime-manager materialization state for a canonical Hugging Face source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "status", content = "value")]
#[norito(deny_unknown_fields)]
pub enum SoracloudRuntimeHfSourceStatus {
    /// The canonical import has no runtime-visible bindings yet.
    PendingImport,
    /// A shared lease is bound, but no runtime service/apartment is materialized yet.
    PendingDeployment,
    /// Runtime materialization exists, but local artifact hydration is incomplete.
    Hydrating,
    /// The runtime can serve the source from the local materialized snapshot.
    Ready,
    /// The canonical source is blocked on a recorded failure.
    Failed,
    /// The canonical source was retired and should not accept fresh joins.
    Retired,
}
/// Runtime-manager projection for one canonical Hugging Face source.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeHfSourcePlan {
    /// Stable canonical source identifier.
    pub source_id: String,
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision resolved by the control plane.
    pub resolved_revision: String,
    /// Normalized Soracloud model name.
    pub model_name: String,
    /// Runtime adapter selected for the source.
    pub adapter_id: String,
    /// Authoritative world-state lifecycle status.
    pub authoritative_status: SoraHfSourceStatusV1,
    /// Runtime-manager materialization status for the source.
    pub runtime_status: SoracloudRuntimeHfSourceStatus,
    /// Number of shared-lease pools currently tracked for the source.
    pub pool_count: u32,
    /// Number of active or draining pools still visible to the runtime manager.
    pub active_pool_count: u32,
    /// Aggregate active shared-lease membership count across tracked pools.
    pub active_member_count: u32,
    /// Number of queued next-window sponsors for the source.
    pub queued_window_count: u32,
    /// Number of distinct bound Soracloud services.
    pub bound_service_count: u32,
    /// Bound Soracloud service names in deterministic order.
    pub bound_service_names: Vec<String>,
    /// Number of bound services already present in the runtime snapshot.
    pub materialized_service_count: u32,
    /// Materialized service names in deterministic order.
    pub materialized_service_names: Vec<String>,
    /// Number of materialized services still hydrating their artifacts.
    pub hydrating_service_count: u32,
    /// Number of distinct bound Soracloud apartments.
    pub bound_apartment_count: u32,
    /// Bound apartment names in deterministic order.
    pub bound_apartment_names: Vec<String>,
    /// Number of bound apartments already present in the runtime snapshot.
    pub materialized_apartment_count: u32,
    /// Materialized apartment names in deterministic order.
    pub materialized_apartment_names: Vec<String>,
    /// Number of missing bundle cache entries across bound services.
    pub bundle_cache_miss_count: u32,
    /// Number of missing non-bundle artifact cache entries across bound services.
    pub artifact_cache_miss_count: u32,
    /// Latest authoritative failure string, when present.
    #[norito(required)]
    pub last_error: Option<String>,
}
/// Schema version for [`SoracloudRuntimeSnapshot`].
pub const SORACLOUD_RUNTIME_SNAPSHOT_VERSION_V1: u16 = 1;
/// Persisted snapshot of node-local Soracloud runtime materialization state.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudRuntimeSnapshot {
    /// Schema version for the local runtime snapshot format.
    pub schema_version: u16,
    /// Height of the authoritative state view used to build this snapshot.
    pub observed_height: u64,
    /// Latest committed block hash at snapshot time, when present.
    #[norito(required)]
    pub observed_block_hash: Option<String>,
    /// Peer identity of the runtime host that produced this snapshot, when known.
    #[norito(required)]
    pub local_peer_id: Option<String>,
    /// Materialized active service revisions grouped by service name then version.
    pub services: BTreeMap<String, BTreeMap<String, SoracloudRuntimeServicePlan>>,
    /// Materialized active agent apartments keyed by apartment name.
    pub apartments: BTreeMap<String, SoracloudRuntimeApartmentPlan>,
    /// Runtime-manager Hugging Face source projections keyed by canonical source id.
    pub hf_sources: BTreeMap<String, SoracloudRuntimeHfSourcePlan>,
}
impl Default for SoracloudRuntimeSnapshot {
    fn default() -> Self {
        Self {
            schema_version: SORACLOUD_RUNTIME_SNAPSHOT_VERSION_V1,
            observed_height: 0,
            observed_block_hash: None,
            local_peer_id: None,
            services: BTreeMap::new(),
            apartments: BTreeMap::new(),
            hf_sources: BTreeMap::new(),
        }
    }
}
/// Read-only Soracloud runtime handle exposed to Torii and other consumers.
pub trait SoracloudRuntimeReadHandle: Send + Sync {
    /// Return the latest node-local runtime materialization snapshot.
    fn snapshot(&self) -> SoracloudRuntimeSnapshot;
    /// Return the local runtime-manager state directory.
    fn state_dir(&self) -> PathBuf;
    /// Return the Soracloud-upload recipient advertised for user model uploads, when available.
    fn uploaded_model_encryption_recipient(
        &self,
    ) -> Option<SoracloudUploadedModelEncryptionRecipient> {
        None
    }
    /// Execute one exact encrypted uploaded-model request inside the node-local private boundary.
    ///
    /// Implementations must load their own durable decryption secret, derive the local active
    /// validator identity, and never expose decrypted model, input, or output payloads.
    fn execute_private_uploaded_model(
        &self,
        _request: SoracloudPrivateUploadedModelExecutionRequestV1,
    ) -> Result<SoracloudPrivateUploadedModelExecutionResultV1, SoracloudRuntimeExecutionError>
    {
        Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "private uploaded-model execution is unavailable on this runtime",
        ))
    }
    /// Load durable ciphertext-only recovery state for one private execution request.
    ///
    /// A production runtime must implement this together with `store_...` and `remove_...` so a
    /// node restart after inference never requires repeating the decrypted computation merely to
    /// recover or resubmit its idempotent ledger commit.
    fn load_private_uploaded_model_execution_journal(
        &self,
        _service_name: &Name,
        _decryption_request_id: &str,
    ) -> Result<
        Option<SoracloudPrivateUploadedModelExecutionJournalV1>,
        SoracloudRuntimeExecutionError,
    > {
        Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "durable private uploaded-model execution recovery is unavailable on this runtime",
        ))
    }
    /// Durably store prepared or submitted ciphertext-only recovery state.
    fn store_private_uploaded_model_execution_journal(
        &self,
        _entry: SoracloudPrivateUploadedModelExecutionJournalV1,
    ) -> Result<(), SoracloudRuntimeExecutionError> {
        Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "durable private uploaded-model execution recovery is unavailable on this runtime",
        ))
    }
    /// Remove recovery state after its exact receipt is visible in committed world state.
    fn remove_private_uploaded_model_execution_journal(
        &self,
        _service_name: &Name,
        _decryption_request_id: &str,
    ) -> Result<(), SoracloudRuntimeExecutionError> {
        Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "durable private uploaded-model execution recovery is unavailable on this runtime",
        ))
    }
    /// Atomically submit output-manifest registration and private receipt persistence.
    ///
    /// `output_manifest_payload` must be the exact canonical `ManifestV1` bytes whose digest and
    /// content length are carried by `receipt.output_artifact`. The returned hash identifies the
    /// one signed transaction containing both ordered instructions.
    fn persist_private_uploaded_model_execution(
        &self,
        _output_manifest_payload: Vec<u8>,
        _receipt: SoraPrivateUploadedModelExecutionReceiptV1,
        _replace_expired_transaction: Option<Hash>,
    ) -> Result<Hash, SoracloudRuntimeExecutionError> {
        Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "private uploaded-model receipt persistence is unavailable on this runtime",
        ))
    }
    /// Return the local peer id, when the runtime knows its host identity.
    fn local_peer_id(&self) -> Option<String> {
        None
    }
    /// Return the maximum time Torii should wait for an internal Soracloud proxy read.
    fn local_read_proxy_timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
    /// Return the maximum interval between durable private-execution outbox recovery passes.
    fn private_execution_recovery_interval(&self) -> Duration {
        Duration::from_secs(60)
    }
    /// Report a failed generated-HF proxy read targeting the authoritative primary host.
    fn report_generated_hf_proxy_failure(
        &self,
        _request: &SoracloudLocalReadRequest,
        _target_peer_id: &str,
        _error: &SoracloudRuntimeExecutionError,
    ) {
    }
    /// Report a generated-HF proxy forwarding failure caused by the local assigned host before
    /// the request reached the authoritative primary.
    fn report_generated_hf_local_proxy_failure(
        &self,
        _request: &SoracloudLocalReadRequest,
        _error: &SoracloudRuntimeExecutionError,
    ) {
    }
    /// Request authoritative HF host reconciliation after ingress observes routing failure.
    fn request_generated_hf_reconcile(
        &self,
        _request: &SoracloudLocalReadRequest,
        _error: &SoracloudRuntimeExecutionError,
    ) {
    }
    /// Request authoritative HF host reconciliation after ingress observes an assigned
    /// non-primary host answer a proxy request that targeted a different authoritative primary.
    ///
    /// Runtime implementations may also turn that authority drift into a model-host health
    /// signal for the unexpected responder before enqueuing reconciliation.
    fn request_generated_hf_proxy_responder_reconcile(
        &self,
        _request: &SoracloudLocalReadRequest,
        _responder_peer_id: &str,
        _expected_peer_id: &str,
    ) {
    }
}
/// Node-local uploaded-model encryption recipient descriptor exposed by the runtime handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudUploadedModelEncryptionRecipient {
    /// Schema version used for the advertised recipient record.
    pub schema_version: u16,
    /// Stable recipient key identifier.
    pub key_id: String,
    /// Recipient key version under the same `key_id`.
    pub key_version: NonZeroU32,
    /// Key-encapsulation suite expected by the recipient.
    pub kem: SoraUploadedModelKeyEncapsulationV1,
    /// AEAD suite expected by the recipient.
    pub aead: SoraUploadedModelKeyWrapAeadV1,
    /// Raw public key bytes used for upload-time envelope encryption.
    pub public_key_bytes: Vec<u8>,
    /// Commitment over the public key bytes.
    pub public_key_fingerprint: Hash,
}
/// Fixed rounding rule used by the v1 quantized CPU runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoracloudQuantizedRoundingV1 {
    /// Arithmetic right shift after adding half the scale, with ties rounded away from zero.
    NearestAwayFromZero,
}
/// Deterministic quantized linear model accepted by the v1 private CPU runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudQuantizedCpuModelV1 {
    /// Number of signed 32-bit integer inputs.
    pub input_len: usize,
    /// Number of signed 32-bit integer outputs.
    pub output_len: usize,
    /// Row-major signed 8-bit weights, `output_len * input_len` entries.
    pub weights_i8: Vec<i8>,
    /// Signed 32-bit output biases.
    pub bias_i32: Vec<i32>,
    /// Non-negative right-shift applied after accumulation.
    pub output_shift: u8,
    /// Saturating lower bound for each output.
    pub output_min: i32,
    /// Saturating upper bound for each output.
    pub output_max: i32,
    /// Explicit rounding mode, fixed for v1.
    pub rounding: SoracloudQuantizedRoundingV1,
}
impl SoracloudQuantizedCpuModelV1 {
    /// Validate the deterministic quantized model shape and arithmetic bounds.
    pub fn validate(&self) -> Result<(), SoracloudRuntimeExecutionError> {
        if self.input_len == 0 || self.output_len == 0 {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model input_len and output_len must be non-zero",
            ));
        }
        if self.weights_i8.len() != self.input_len.saturating_mul(self.output_len) {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model weights must be row-major output_len * input_len",
            ));
        }
        if self.bias_i32.len() != self.output_len {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model bias length must equal output_len",
            ));
        }
        if self.output_shift > 30 {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model output_shift must be <= 30",
            ));
        }
        if self.output_min > self.output_max {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model output_min must be <= output_max",
            ));
        }
        Ok(())
    }
    /// Execute deterministic signed-integer inference on the CPU reference path.
    pub fn evaluate(&self, input: &[i32]) -> Result<Vec<i32>, SoracloudRuntimeExecutionError> {
        self.validate()?;
        if input.len() != self.input_len {
            return Err(SoracloudRuntimeExecutionError::new(
                SoracloudRuntimeExecutionErrorKind::InvalidRequest,
                "quantized model input length does not match input_len",
            ));
        }
        let mut outputs = Vec::with_capacity(self.output_len);
        for output_index in 0..self.output_len {
            let row_start = output_index
                .checked_mul(self.input_len)
                .expect("validated model dimensions fit usize");
            let mut acc = i64::from(self.bias_i32[output_index]);
            for (input_index, input_value) in input.iter().enumerate() {
                let weight = self.weights_i8[row_start + input_index];
                acc = acc.saturating_add(i64::from(*input_value).saturating_mul(i64::from(weight)));
            }
            let rounded = round_quantized_accumulator(acc, self.output_shift, self.rounding);
            let saturated = rounded.clamp(i64::from(self.output_min), i64::from(self.output_max));
            outputs.push(i32::try_from(saturated).expect("clamped to i32 range"));
        }
        Ok(outputs)
    }
}
fn round_quantized_accumulator(
    value: i64,
    shift: u8,
    rounding: SoracloudQuantizedRoundingV1,
) -> i64 {
    if shift == 0 {
        return value;
    }
    match rounding {
        SoracloudQuantizedRoundingV1::NearestAwayFromZero => {
            let half = 1_i64 << (shift - 1);
            if value >= 0 {
                value.saturating_add(half) >> shift
            } else {
                -((-value).saturating_add(half) >> shift)
            }
        }
    }
}
/// Input envelope for deterministic private uploaded-model execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudPrivateUploadedModelExecutionRequestV1 {
    /// Admitted uploaded model package metadata.
    pub bundle: SoraUploadedModelBundleV1,
    /// Exact active service revision authorized for this execution.
    pub service_version: String,
    /// Decryption policy approved for this execution.
    pub policy_id: String,
    /// Exact committed authorization record that released the encrypted input.
    pub decryption_request_id: String,
    /// Persisted encrypted input artifact reference.
    pub input_artifact: SoraPrivateModelArtifactRefV1,
    /// Exact recipient requested for the encrypted output.
    pub output_recipient: SoraUploadedModelEncryptionRecipientV1,
    /// Exact encoded encrypted model artifact loaded under a SoraFS read lease.
    pub encrypted_model_artifact_bytes: Vec<u8>,
    /// Exact encoded encrypted input artifact loaded under a SoraFS read lease.
    pub encrypted_input_artifact_bytes: Vec<u8>,
}
/// Result emitted by the deterministic private uploaded-model CPU runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudPrivateUploadedModelExecutionResultV1 {
    /// Exact encoded encrypted output artifact ready for durable SoraFS ingestion.
    pub encrypted_output_artifact_bytes: Vec<u8>,
    /// Runtime-blinded commitment over the canonical decrypted input payload.
    pub input_commitment: Hash,
    /// Runtime-blinded commitment over the canonical output payload before encryption.
    pub output_commitment: Hash,
    /// Exact recipient metadata used to wrap the encrypted output.
    pub output_recipient: SoraUploadedModelEncryptionRecipientV1,
    /// Locally resolved active validator that must attest receipt persistence.
    pub attesting_validator: SoraRuntimeDeterministicValidatorHostV1,
    /// Canonical deterministic runtime version used for execution.
    pub runtime_version: String,
}
/// Schema version for durable private uploaded-model execution recovery state.
pub const SORACLOUD_PRIVATE_UPLOADED_MODEL_EXECUTION_JOURNAL_VERSION_V1: u16 = 1;
/// Maximum freshly signed transaction attempts retained for one exact private execution.
pub const SORACLOUD_PRIVATE_UPLOADED_MODEL_EXECUTION_MAX_SUBMISSION_ATTEMPTS_V1: u8 = 3;
/// Ciphertext-only prepared/submitted state used to recover private execution across restarts.
///
/// Decrypted model, input, and output payloads are deliberately absent. The referenced encrypted
/// output must already be durably ingested into local SoraFS before this record is published.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SoracloudPrivateUploadedModelExecutionJournalV1 {
    /// Journal schema version.
    pub schema_version: u16,
    /// Service and decryption request forming the unique journal key.
    pub service_name: Name,
    /// Exact committed decryption request identifier.
    pub decryption_request_id: String,
    /// Commitment to the complete canonical Torii execution request.
    pub request_fingerprint: Hash,
    /// Canonical output `ManifestV1` payload carried by the idempotent commit instruction.
    pub output_manifest_payload: Vec<u8>,
    /// Canonical receipt submission with ledger-owned sequence and height still zero.
    pub receipt: SoraPrivateUploadedModelExecutionReceiptV1,
    /// Most recently submitted signed transaction hash, absent while merely prepared.
    pub transaction_hash: Option<Hash>,
    /// Exact signed transaction persisted before enqueue, absent while merely prepared.
    pub signed_transaction: Option<SignedTransaction>,
    /// Number of distinct signed transactions produced for this exact prepared execution.
    pub submission_attempt: u8,
}
impl SoracloudPrivateUploadedModelExecutionJournalV1 {
    /// Validate bounded canonical recovery state and all output/receipt bindings.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SORACLOUD_PRIVATE_UPLOADED_MODEL_EXECUTION_JOURNAL_VERSION_V1 {
            return Err(format!(
                "unsupported private execution journal schema_version {}",
                self.schema_version
            ));
        }
        if self.decryption_request_id.trim().is_empty()
            || self.decryption_request_id.len() > 256
            || self.service_name != self.receipt.service_name
            || self.decryption_request_id != self.receipt.decryption_request_id
        {
            return Err(
                "private execution journal key does not exactly match its receipt".to_owned(),
            );
        }
        self.receipt
            .validate_submission()
            .map_err(|error| format!("invalid private execution journal receipt: {error}"))?;
        if self.output_manifest_payload.is_empty()
            || self.output_manifest_payload.len() > sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES
        {
            return Err(format!(
                "private execution journal manifest length must be in 1..={} bytes",
                sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES
            ));
        }
        let manifest = sorafs_manifest::decode_manifest_v1_canonical(&self.output_manifest_payload)
            .map_err(|error| format!("invalid journal output ManifestV1: {error}"))?;
        let digest = ManifestDigest::from_manifest(&manifest)
            .map_err(|error| format!("derive journal output manifest digest: {error}"))?;
        if digest != self.receipt.output_artifact.sorafs_manifest_digest
            || manifest.content_length != self.receipt.output_artifact.ciphertext_bytes
        {
            return Err(
                "private execution journal manifest does not match its receipt output artifact"
                    .to_owned(),
            );
        }
        match (&self.signed_transaction, self.transaction_hash) {
            (None, None) if self.submission_attempt == 0 => {}
            (Some(transaction), Some(hash))
                if self.submission_attempt > 0
                    && self.submission_attempt
                        <= SORACLOUD_PRIVATE_UPLOADED_MODEL_EXECUTION_MAX_SUBMISSION_ATTEMPTS_V1
                    && Hash::from(transaction.hash()) == hash => {}
            _ => {
                return Err(
                    "private execution journal signed transaction, hash, and attempt are inconsistent"
                        .to_owned(),
                );
            }
        }
        Ok(())
    }
}
/// Shared Soracloud runtime handle type used across crate boundaries.
pub type SharedSoracloudRuntimeHandle = Arc<dyn SoracloudRuntimeReadHandle>;
/// Coarse execution failure category for embedded Soracloud runtime requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SoracloudRuntimeExecutionErrorKind {
    /// The runtime cannot execute the request in the current node process.
    Unavailable,
    /// The request is structurally invalid for the configured runtime surface.
    InvalidRequest,
    /// The runtime hit an internal execution failure.
    Internal,
}
/// Structured error returned by the shared Soracloud runtime execution trait.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SoracloudRuntimeExecutionError {
    /// High-level error category.
    pub kind: SoracloudRuntimeExecutionErrorKind,
    /// Human-readable detail preserved for logging and deterministic receipts.
    pub message: String,
}
impl SoracloudRuntimeExecutionError {
    /// Construct a new structured runtime execution error.
    #[must_use]
    pub fn new(kind: SoracloudRuntimeExecutionErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for SoracloudRuntimeExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let category = match self.kind {
            SoracloudRuntimeExecutionErrorKind::Unavailable => "unavailable",
            SoracloudRuntimeExecutionErrorKind::InvalidRequest => "invalid request",
            SoracloudRuntimeExecutionErrorKind::Internal => "internal",
        };
        write!(
            formatter,
            "Soracloud runtime {category} error: {}",
            self.message
        )
    }
}
impl std::error::Error for SoracloudRuntimeExecutionError {}
/// Deterministic local read class for the Soracloud fast path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SoracloudLocalReadKind {
    /// Static asset read bound to committed artifacts.
    Asset,
    /// Read-only query bound to the committed state snapshot.
    Query,
}
/// Shared request envelope for deterministic local Soracloud reads.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SoracloudLocalReadRequest {
    /// Authoritative height used for the local read snapshot.
    pub observed_height: u64,
    /// Latest committed block hash visible to the caller.
    pub observed_block_hash: Option<Hash>,
    /// Service targeted by the read.
    pub service_name: String,
    /// Active service version used for the read.
    pub service_version: String,
    /// Handler servicing the request.
    pub handler_name: String,
    /// Handler class for the request.
    pub handler_class: SoracloudLocalReadKind,
    /// HTTP method or logical read method used to invoke the handler.
    pub request_method: String,
    /// Full request path as received by Torii.
    pub request_path: String,
    /// Request path relative to the matched handler route.
    pub handler_path: String,
    /// Optional raw query string without the leading `?`.
    pub request_query: Option<String>,
    /// Canonicalized request headers made visible to the handler.
    pub request_headers: BTreeMap<String, String>,
    /// Opaque request payload bytes supplied to the handler.
    pub request_body: Vec<u8>,
    /// Deterministic commitment over the request envelope.
    pub request_commitment: Hash,
}
/// Committed artifact/state binding attached to a certified local read response.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SoracloudLocalReadBinding {
    /// Binding name when the response is derived from authoritative service state.
    #[norito(required)]
    pub binding_name: Option<String>,
    /// State key when the response is derived from a specific state entry.
    #[norito(required)]
    pub state_key: Option<String>,
    /// Commitment for the bound state entry, when applicable.
    #[norito(required)]
    pub payload_commitment: Option<Hash>,
    /// Bound artifact digest when the response is served from hydrated local content.
    #[norito(required)]
    pub artifact_hash: Option<Hash>,
}
/// Shared response envelope for deterministic local Soracloud reads.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SoracloudLocalReadResponse {
    /// Raw response bytes emitted by the runtime.
    pub response_bytes: Vec<u8>,
    /// MIME type of the response payload, when known.
    pub content_type: Option<String>,
    /// Optional content encoding metadata for the response.
    pub content_encoding: Option<String>,
    /// Optional cache-control metadata for the response.
    pub cache_control: Option<String>,
    /// Committed bindings that certify the response payload.
    pub bindings: Vec<SoracloudLocalReadBinding>,
    /// Commitment over the response envelope.
    pub result_commitment: Hash,
    /// Certification mode selected for this read.
    pub certified_by: SoraCertifiedResponsePolicyV1,
    /// Optional receipt emitted for audit-style certifications.
    pub runtime_receipt: Option<SoraRuntimeReceiptV1>,
}
/// Schema version for peer-to-peer Soracloud local-read proxy requests.
pub const SORACLOUD_LOCAL_READ_PROXY_REQUEST_VERSION_V1: u16 = 1;
/// Schema version for peer-to-peer Soracloud local-read proxy responses.
pub const SORACLOUD_LOCAL_READ_PROXY_RESPONSE_VERSION_V1: u16 = 1;
/// Peer-to-peer local-read proxy request sent to the authoritative primary host.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SoracloudLocalReadProxyRequestV1 {
    /// Version of the proxy request envelope.
    pub schema_version: u16,
    /// Correlation id chosen by the ingress node.
    pub request_id: Hash,
    /// Canonical local-read request to execute on the primary host.
    pub request: SoracloudLocalReadRequest,
}
/// Outcome for a peer-to-peer local-read proxy request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SoracloudLocalReadProxyOutcomeV1 {
    /// Successful local-read execution.
    Ok(SoracloudLocalReadResponse),
    /// Failed local-read execution.
    Err(SoracloudRuntimeExecutionError),
}
/// Peer-to-peer local-read proxy response sent back to the ingress node.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SoracloudLocalReadProxyResponseV1 {
    /// Version of the proxy response envelope.
    pub schema_version: u16,
    /// Correlation id selected by the ingress node.
    pub request_id: Hash,
    /// Execution result returned by the primary host.
    pub outcome: SoracloudLocalReadProxyOutcomeV1,
}
/// Deterministic state mutation produced by ordered Soracloud execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudDeterministicStateMutation {
    /// Binding mutated by the runtime.
    pub binding_name: String,
    /// Canonical key scoped under the binding prefix.
    pub state_key: String,
    /// Mutation mode to apply.
    pub operation: SoraStateMutationOperationV1,
    /// Encryption contract enforced by the binding.
    pub encryption: SoraStateEncryptionV1,
    /// Declared payload size when the mutation upserts content.
    pub payload_bytes: Option<u64>,
    /// Full payload bytes when the mutation upserts content.
    pub payload: Option<Vec<u8>>,
    /// Deterministic commitment over the opaque payload.
    pub payload_commitment: Option<Hash>,
}
/// Shared request envelope for ordered Soracloud mailbox execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudOrderedMailboxExecutionRequest {
    /// Authoritative height pinned for the execution.
    pub observed_height: u64,
    /// Latest committed block hash visible to the executor.
    pub observed_block_hash: Option<Hash>,
    /// Authoritative Soracloud sequence observed by the VM as read-only execution context.
    pub observed_sequence: u64,
    /// Current deployment state for the target service.
    pub deployment: SoraServiceDeploymentStateV1,
    /// Admitted active bundle for the target service revision.
    pub bundle: SoraDeploymentBundleV1,
    /// Resolved target handler when it exists in the active bundle.
    pub handler: Option<SoraServiceHandlerV1>,
    /// Mailbox message being delivered through replicated progression.
    pub mailbox_message: SoraServiceMailboxMessageV1,
    /// Latest runtime state observed for the target service.
    pub runtime_state: Option<SoraServiceRuntimeStateV1>,
    /// Outstanding mailbox message count before this execution is applied.
    pub authoritative_pending_mailbox_messages: u32,
}
/// Deterministic result of ordered Soracloud mailbox execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudOrderedMailboxExecutionResult {
    /// Deterministic state mutations to apply to authoritative service state.
    pub state_mutations: Vec<SoracloudDeterministicStateMutation>,
    /// Cross-service messages emitted by the execution.
    pub outbound_mailbox_messages: Vec<SoraServiceMailboxMessageV1>,
    /// Optional response payload returned by the executed handler.
    pub response_bytes: Vec<u8>,
    /// MIME type associated with `response_bytes`, when known.
    pub content_type: Option<String>,
    /// Runtime-state observation to persist after execution.
    pub runtime_state: Option<SoraServiceRuntimeStateV1>,
    /// Deterministic runtime receipt for the execution.
    pub runtime_receipt: SoraRuntimeReceiptV1,
}
/// Shared request envelope for deterministic apartment execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudApartmentExecutionRequest {
    /// Authoritative height pinned for the apartment execution.
    pub observed_height: u64,
    /// Latest committed block hash visible to the runtime.
    pub observed_block_hash: Option<Hash>,
    /// Apartment targeted by the runtime.
    pub apartment_name: String,
    /// Expected apartment process generation.
    pub process_generation: u64,
    /// Logical apartment operation to execute.
    pub operation: String,
    /// Deterministic commitment over the apartment request.
    pub request_commitment: Hash,
}
/// Shared result for deterministic apartment execution.
#[allow(missing_copy_implementations)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoracloudApartmentExecutionResult {
    /// Latest apartment status reported by the runtime.
    pub status: SoraAgentRuntimeStatusV1,
    /// Optional committed checkpoint hash materialized by the operation.
    pub checkpoint_artifact_hash: Option<Hash>,
    /// Optional committed journal hash materialized by the operation.
    pub journal_artifact_hash: Option<Hash>,
    /// Deterministic commitment over the apartment result.
    pub result_commitment: Hash,
}
/// Shared execution interface for the embedded Soracloud runtime.
pub trait SoracloudRuntime: SoracloudRuntimeReadHandle {
    /// Execute a deterministic local read against the committed runtime snapshot.
    fn execute_local_read(
        &self,
        request: SoracloudLocalReadRequest,
    ) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError>;
    /// Execute an ordered mailbox handler against a pinned committed snapshot.
    ///
    /// This is a node-local proposal step. Production consensus persists the returned effects only
    /// through an explicit `ApplySoracloudOrderedMailboxResult` instruction that revalidates the
    /// snapshot preconditions and executor identity.
    fn execute_ordered_mailbox(
        &self,
        request: SoracloudOrderedMailboxExecutionRequest,
    ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError>;
    /// Execute deterministic apartment work owned by the embedded runtime manager.
    fn execute_apartment(
        &self,
        request: SoracloudApartmentExecutionRequest,
    ) -> Result<SoracloudApartmentExecutionResult, SoracloudRuntimeExecutionError>;
}
/// Shared node-local Soracloud runtime trait object.
///
/// Production replicated execution never invokes this handle implicitly.
pub type SharedSoracloudRuntime = Arc<dyn SoracloudRuntime>;
impl SoracloudLocalReadKind {
    /// Return the Soracloud handler class represented by this local read kind.
    #[must_use]
    pub fn handler_class(self) -> SoraServiceHandlerClassV1 {
        match self {
            Self::Asset => SoraServiceHandlerClassV1::Asset,
            Self::Query => SoraServiceHandlerClassV1::Query,
        }
    }
}
impl SoracloudRuntimeExecutionErrorKind {
    /// Stable label used when hashing synthetic failure receipts.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::InvalidRequest => "invalid_request",
            Self::Internal => "internal",
        }
    }
}
impl SoracloudDeterministicStateMutation {
    /// Return `true` when this mutation writes payload bytes into authoritative service state.
    #[must_use]
    pub fn is_upsert(&self) -> bool {
        matches!(self.operation, SoraStateMutationOperationV1::Upsert)
    }
}
impl From<SoracloudLocalReadKind> for SoraServiceHandlerClassV1 {
    fn from(value: SoracloudLocalReadKind) -> Self {
        value.handler_class()
    }
}
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
/// Bounded runtime write-back instruction set used for internal Soracloud integration points.
pub enum SoracloudRuntimeInstruction {
    /// Persist an updated runtime-state snapshot.
    SetRuntimeState(iroha_data_model::isi::soracloud::SetSoracloudRuntimeState),
    /// Persist an outbound cross-service mailbox message.
    RecordMailboxMessage(iroha_data_model::isi::soracloud::RecordSoracloudMailboxMessage),
    /// Persist an authoritative runtime receipt.
    RecordRuntimeReceipt(iroha_data_model::isi::soracloud::RecordSoracloudRuntimeReceipt),
}
impl SoracloudRuntimeInstruction {
    /// Convert the bounded runtime write-back into a regular instruction box.
    #[must_use]
    pub fn into_instruction_box(self) -> InstructionBox {
        match self {
            Self::SetRuntimeState(isi) => InstructionBox::from(isi),
            Self::RecordMailboxMessage(isi) => InstructionBox::from(isi),
            Self::RecordRuntimeReceipt(isi) => InstructionBox::from(isi),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::State, state::World};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        asset::AssetDefinitionId,
        domain::DomainId,
        peer::PeerId,
        soracloud::{
            SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1, SORA_HF_PLACEMENT_RECORD_VERSION_V1,
            SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1, SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
            SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1,
            SORA_MODEL_HOST_VIOLATION_EVIDENCE_RECORD_VERSION_V1, SoraAppInfraActionV1,
            SoraAppInfraAuditEventV1, SoraHfBackendFamilyV1, SoraHfModelFormatV1,
            SoraHfPlacementHostAssignmentV1, SoraHfPlacementStatusV1, SoraHfResourceProfileV1,
            SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1, SoraModelHostCapabilityRecordV1,
            SoraModelHostViolationEvidenceRecordV1, SoraModelHostViolationKindV1,
            SoraServiceMailboxMessageV1, derive_hf_placement_id_v1,
            derive_soracloud_mailbox_message_id_v1,
        },
        sorafs::pin_registry::{ManifestDigest, StorageClass},
    };
    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("Soracloud runtime fixture key generation should succeed")
    }
    fn checked_account_id() -> AccountId {
        AccountId::new(checked_keypair().public_key().clone())
    }
    fn checked_peer_id() -> PeerId {
        PeerId::from(checked_keypair().public_key().clone())
    }
    fn checked_validator_host() -> SoraRuntimeDeterministicValidatorHostV1 {
        let validator_account_id = checked_account_id();
        SoraRuntimeDeterministicValidatorHostV1 {
            lane_id: LaneId::SINGLE,
            peer_id: PeerId::from(validator_account_id.expect_single_signatory().clone())
                .to_string(),
            validator_account_id,
        }
    }
    fn sample_ordered_mailbox_result() -> SoraOrderedMailboxResultV1 {
        let service_name: Name = "mailbox_service".parse().expect("valid service name");
        let handler_name: Name = "update".parse().expect("valid handler name");
        let mailbox_message_id = Hash::new(b"ordered mailbox source message");
        let response_commitment = Hash::new(b"ordered mailbox response");
        let runtime_execution_commitment = Hash::new(b"ordered mailbox runtime outcome");
        let runtime_state = SoraServiceRuntimeStateV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
            service_name: service_name.clone(),
            active_service_version: "1.0.0".to_owned(),
            health_status: SoraServiceHealthStatusV1::Healthy,
            load_factor_bps: 125,
            materialized_bundle_hash: Hash::new(b"ordered mailbox bundle"),
        };
        let validator_account_id = checked_account_id();
        let validator_peer_id =
            PeerId::from(validator_account_id.expect_single_signatory().clone());
        let payload = b"ordered outbound payload".to_vec();
        SoraOrderedMailboxResultV1 {
            schema_version: iroha_data_model::soracloud::SORA_ORDERED_MAILBOX_RESULT_VERSION_V1,
            observed_height: 7,
            observed_block_hash: Some(Hash::new(b"ordered mailbox observed block")),
            observed_sequence: 11,
            state_mutations: vec![
                iroha_data_model::soracloud::SoraOrderedMailboxStateMutationV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_ORDERED_MAILBOX_STATE_MUTATION_VERSION_V1,
                    binding_name: "state".parse().expect("valid binding name"),
                    state_key: "/counter".to_owned(),
                    operation: SoraStateMutationOperationV1::Upsert,
                    encryption: SoraStateEncryptionV1::Plaintext,
                    value_payload: Some(b"1".to_vec()),
                },
            ],
            outbound_mailbox_messages: vec![SoraServiceMailboxMessageV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
                message_id: Hash::prehashed([0; Hash::LENGTH]),
                from_service: service_name.clone(),
                from_service_version: String::new(),
                from_handler: handler_name.clone(),
                to_service: "mailbox_destination"
                    .parse()
                    .expect("valid destination service name"),
                to_service_version: String::new(),
                to_handler: "receive".parse().expect("valid destination handler name"),
                payload_commitment: Hash::new(&payload),
                payload_bytes: payload,
                delivery_delay_blocks: 1,
                enqueue_sequence: 0,
                enqueue_height: 0,
                available_after_height: 0,
                expires_at_height: 0,
            }],
            response_commitment,
            runtime_execution_commitment,
            content_type: Some("application/octet-stream".to_owned()),
            observed_runtime_state: None,
            runtime_state: Some(runtime_state),
            runtime_receipt: SoraRuntimeReceiptV1 {
                schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id: Hash::new(b"uncommitted ordered mailbox receipt id"),
                service_name,
                service_version: "1.0.0".to_owned(),
                handler_name,
                handler_class: SoraServiceHandlerClassV1::Update,
                request_commitment: Hash::new(b"ordered mailbox request"),
                result_commitment: Hash::new(b"uncommitted ordered mailbox result"),
                certified_by: SoraCertifiedResponsePolicyV1::None,
                emitted_sequence: 0,
                mailbox_message_id: Some(mailbox_message_id),
                journal_artifact_hash: Some(Hash::new(b"ordered mailbox journal")),
                checkpoint_artifact_hash: None,
                execution_host: Some(
                    iroha_data_model::soracloud::SoraRuntimeExecutionHostV1::DeterministicValidator(
                        SoraRuntimeDeterministicValidatorHostV1 {
                            lane_id: LaneId::SINGLE,
                            validator_account_id,
                            peer_id: validator_peer_id.to_string(),
                        },
                    ),
                ),
            },
        }
    }
    fn sample_runtime_service_plan() -> SoracloudRuntimeServicePlan {
        SoracloudRuntimeServicePlan {
            service_name: "service".to_owned(),
            service_version: "1.0.0".to_owned(),
            role: SoracloudRuntimeRevisionRole::Active,
            traffic_percent: 100,
            runtime: SoraContainerRuntimeV1::Inrou,
            execution_plane: SoraServiceExecutionPlaneV1::HttpService,
            bundle_hash: "bundle-hash".to_owned(),
            bundle_path: "sorafs://bundle".to_owned(),
            entrypoint: "/sbin/init".to_owned(),
            inrou: Some(SoracloudRuntimeInrouPlan {
                guest_os: SoraInrouGuestOsV1::DebianSlim,
                selected_guest_isa: SoraInrouGuestIsaV1::X8664,
                kernel_image_path: "guest/vmlinuz".to_owned(),
                rootfs_image_path: "guest/rootfs.ext4".to_owned(),
                initrd_image_path: None,
                bootstrap_user_data_path: None,
                ssh_authorized_keys: Vec::new(),
                root_volume_name: "root".to_owned(),
            }),
            bundle_cache_path: "/runtime/cache/bundle".to_owned(),
            bundle_available_locally: true,
            process_generation: None,
            desired_replica_count: 1,
            local_replica_slots: vec![1],
            local_replicas: vec![SoracloudRuntimeReplicaPlan {
                replica_slot: 1,
                materialization_dir: "/runtime/services/service/1.0.0/replica-0001".to_owned(),
                health_status: SoraServiceHealthStatusV1::Unavailable,
                listen_base_url: None,
                pid: None,
                last_error: None,
            }],
            health_status: SoraServiceHealthStatusV1::Unavailable,
            load_factor_bps: 0,
            authoritative_pending_mailbox_messages: 0,
            rollout_handle: None,
            config_generation: 0,
            secret_generation: 0,
            quota_class: None,
            service_lease_status: None,
            lease_expires_height: None,
            remaining_runtime_balance: None,
            config_entry_count: 0,
            secret_entry_count: 0,
            config_exports: Vec::new(),
            supports_host_read_config: false,
            supports_host_read_secret_envelope: false,
            supports_private_secret_payload_reads: false,
            materialization_dir: "/runtime/services/service/1.0.0".to_owned(),
            config_materialization_dir: "/runtime/services/service/1.0.0/config".to_owned(),
            effective_env: BTreeMap::new(),
            effective_env_materialization_path: "/runtime/services/service/1.0.0/env.json"
                .to_owned(),
            config_exports_materialization_dir: "/runtime/services/service/1.0.0/exports"
                .to_owned(),
            secret_envelopes_materialization_dir: "/runtime/services/service/1.0.0/secrets"
                .to_owned(),
            lease_volumes: Vec::new(),
            mailboxes: Vec::new(),
            artifacts: Vec::new(),
        }
    }
    #[test]
    fn checked_keypair_preserves_default_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    }
    #[test]
    fn ordered_mailbox_result_commitment_binds_effects_but_not_occ_preconditions() {
        let result = sample_ordered_mailbox_result();
        let commitment = ordered_mailbox_result_commitment(&result);

        let mut changed_mutation = result.clone();
        changed_mutation.state_mutations[0].value_payload = Some(b"2".to_vec());
        assert_ne!(
            ordered_mailbox_result_commitment(&changed_mutation),
            commitment
        );

        let mut changed_outbound = result.clone();
        changed_outbound.outbound_mailbox_messages[0].expires_at_height += 1;
        assert_ne!(
            ordered_mailbox_result_commitment(&changed_outbound),
            commitment
        );

        let mut changed_runtime_state = result.clone();
        changed_runtime_state
            .runtime_state
            .as_mut()
            .expect("runtime-state projection")
            .load_factor_bps += 1;
        assert_ne!(
            ordered_mailbox_result_commitment(&changed_runtime_state),
            commitment
        );

        let mut changed_preconditions = result.clone();
        changed_preconditions.observed_height += 1;
        changed_preconditions.observed_sequence += 1;
        changed_preconditions.observed_block_hash = Some(Hash::new(b"different observed block"));
        changed_preconditions.observed_runtime_state = result.runtime_state.clone();
        assert_eq!(
            ordered_mailbox_result_commitment(&changed_preconditions),
            commitment
        );
    }
    #[test]
    fn ordered_mailbox_receipt_id_binds_every_immutable_receipt_field() {
        let receipt = sample_ordered_mailbox_result().runtime_receipt;
        let canonical = ordered_mailbox_runtime_receipt_id(&receipt)
            .expect("ordered-mailbox fixture names its source message");

        macro_rules! assert_field_bound {
            ($field:literal, $mutate:expr) => {{
                let mut changed = receipt.clone();
                ($mutate)(&mut changed);
                assert_ne!(
                    ordered_mailbox_runtime_receipt_id(&changed),
                    Some(canonical),
                    "ordered-mailbox receipt ID must bind {}",
                    $field
                );
            }};
        }

        assert_field_bound!("schema_version", |changed: &mut SoraRuntimeReceiptV1| {
            changed.schema_version += 1;
        });
        assert_field_bound!(
            "mailbox_message_id",
            |changed: &mut SoraRuntimeReceiptV1| {
                changed.mailbox_message_id = Some(Hash::new(b"different source message"));
            }
        );
        assert_field_bound!("service_name", |changed: &mut SoraRuntimeReceiptV1| {
            changed.service_name = "different_service".parse().expect("valid service name");
        });
        assert_field_bound!("service_version", |changed: &mut SoraRuntimeReceiptV1| {
            changed.service_version = "2.0.0".to_owned();
        });
        assert_field_bound!("handler_name", |changed: &mut SoraRuntimeReceiptV1| {
            changed.handler_name = "different_handler".parse().expect("valid handler name");
        });
        assert_field_bound!("handler_class", |changed: &mut SoraRuntimeReceiptV1| {
            changed.handler_class = SoraServiceHandlerClassV1::PrivateUpdate;
        });
        assert_field_bound!(
            "request_commitment",
            |changed: &mut SoraRuntimeReceiptV1| {
                changed.request_commitment = Hash::new(b"different request");
            }
        );
        assert_field_bound!("result_commitment", |changed: &mut SoraRuntimeReceiptV1| {
            changed.result_commitment = Hash::new(b"different result");
        });
        assert_field_bound!("certified_by", |changed: &mut SoraRuntimeReceiptV1| {
            changed.certified_by = SoraCertifiedResponsePolicyV1::AuditReceipt;
        });
        assert_field_bound!("execution_host", |changed: &mut SoraRuntimeReceiptV1| {
            changed.execution_host = None;
        });
        assert_field_bound!(
            "journal_artifact_hash",
            |changed: &mut SoraRuntimeReceiptV1| {
                changed.journal_artifact_hash = Some(Hash::new(b"different journal"));
            }
        );
        assert_field_bound!(
            "checkpoint_artifact_hash",
            |changed: &mut SoraRuntimeReceiptV1| {
                changed.checkpoint_artifact_hash = Some(Hash::new(b"different checkpoint"));
            }
        );

        let mut ledger_owned = receipt;
        ledger_owned.receipt_id = Hash::new(b"ignored prior receipt identifier");
        ledger_owned.emitted_sequence = 42;
        assert_eq!(
            ordered_mailbox_runtime_receipt_id(&ledger_owned),
            Some(canonical),
            "receipt identity must remain sequence-independent and non-recursive"
        );
    }
    #[test]
    fn ordered_mailbox_executor_requires_the_exact_canonical_active_validator_record() {
        let peer_id = checked_peer_id().to_string();
        let (mut world, _service_name, _service_version, _source_id) =
            seed_generated_hf_world_with_primary(&peer_id);
        let payload = b"executor selection payload".to_vec();
        let mut message = SoraServiceMailboxMessageV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            message_id: Hash::prehashed([0; Hash::LENGTH]),
            from_service: "source".parse().expect("valid source service"),
            from_service_version: "1.0.0".to_owned(),
            from_handler: "send".parse().expect("valid source handler"),
            to_service: "destination".parse().expect("valid destination service"),
            to_service_version: "2.0.0".to_owned(),
            to_handler: "receive".parse().expect("valid destination handler"),
            payload_commitment: Hash::new(&payload),
            payload_bytes: payload,
            delivery_delay_blocks: 0,
            enqueue_sequence: 3,
            enqueue_height: 3,
            available_after_height: 3,
            expires_at_height: 9,
        };
        message.message_id = derive_soracloud_mailbox_message_id_v1(&message);
        let validator_key = {
            let view = world.view();
            let (validator_key, validator_record) = view
                .public_lane_validators()
                .iter()
                .next()
                .map(|(key, record)| (key.clone(), record.clone()))
                .expect("active validator fixture");
            let selected = resolve_ordered_mailbox_executor(&view, &message, 4, |_| true)
                .expect("canonical active validator must be selected");

            assert_eq!(selected.lane_id, validator_key.0);
            assert_eq!(selected.validator_account_id, validator_key.1);
            assert_eq!(selected.peer_id, validator_record.peer_id.to_string());
            validator_key
        };

        let mut noncanonical_record = world
            .view()
            .public_lane_validators()
            .get(&validator_key)
            .cloned()
            .expect("active validator fixture");
        noncanonical_record.peer_id = checked_peer_id();
        world
            .public_lane_validators_mut_for_testing()
            .insert(validator_key, noncanonical_record);
        assert!(
            resolve_ordered_mailbox_executor(&world.view(), &message, 4, |_| true).is_none(),
            "an active record rebound away from the account's canonical peer must be ineligible"
        );
    }
    #[test]
    fn ordered_mailbox_executor_cannot_be_grinded_by_message_identity_or_payload() {
        let peer_id = checked_peer_id().to_string();
        let (mut world, _service_name, _service_version, _source_id) =
            seed_generated_hf_world_with_primary(&peer_id);
        let mut second_record = world
            .view()
            .public_lane_validators()
            .iter()
            .next()
            .map(|(_key, record)| record.clone())
            .expect("first active validator fixture");
        let second_validator = checked_account_id();
        second_record.validator = second_validator.clone();
        second_record.stake_account = second_validator.clone();
        second_record.peer_id = PeerId::from(second_validator.expect_single_signatory().clone());
        world.public_lane_validators_mut_for_testing().insert(
            (iroha_data_model::nexus::LaneId::SINGLE, second_validator),
            second_record,
        );

        let payload = b"executor selection payload".to_vec();
        let mut message = SoraServiceMailboxMessageV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            message_id: Hash::prehashed([0; Hash::LENGTH]),
            from_service: "source".parse().expect("valid source service"),
            from_service_version: "1.0.0".to_owned(),
            from_handler: "send".parse().expect("valid source handler"),
            to_service: "destination".parse().expect("valid destination service"),
            to_service_version: "2.0.0".to_owned(),
            to_handler: "receive".parse().expect("valid destination handler"),
            payload_commitment: Hash::new(&payload),
            payload_bytes: payload,
            delivery_delay_blocks: 0,
            enqueue_sequence: 3,
            enqueue_height: 3,
            available_after_height: 3,
            expires_at_height: 9,
        };
        message.message_id = derive_soracloud_mailbox_message_id_v1(&message);
        let selected = resolve_ordered_mailbox_executor(&world.view(), &message, 4, |_| true)
            .expect("one of two active validators must be selected");

        let mut payload_grind = message.clone();
        payload_grind.payload_bytes = b"grinded payload".to_vec();
        payload_grind.payload_commitment = Hash::new(&payload_grind.payload_bytes);
        payload_grind.message_id = derive_soracloud_mailbox_message_id_v1(&payload_grind);
        assert_ne!(payload_grind.message_id, message.message_id);
        assert_eq!(
            resolve_ordered_mailbox_executor(&world.view(), &payload_grind, 4, |_| true),
            Some(selected.clone()),
            "caller-controlled payload and its canonical id must not steer executor selection"
        );

        let mut identifier_grind = message;
        identifier_grind.message_id = Hash::new(b"caller-grinded message id");
        assert_eq!(
            resolve_ordered_mailbox_executor(&world.view(), &identifier_grind, 4, |_| true),
            Some(selected),
            "a substituted message id must not steer executor selection"
        );
    }
    #[test]
    fn latest_and_authoritative_sequences_use_the_persisted_watermark_and_saturate() {
        let mut world = World::new();
        let set_watermark = |world: &World, value| {
            let mut block = world.block();
            *block.soracloud_sequence_watermark.get_mut() = value;
            block.commit();
        };
        assert_eq!(latest_soracloud_sequence(&world.view()), 0);
        assert_eq!(authoritative_soracloud_sequence(&world.view()), 1);

        let signer = checked_keypair().public_key().clone();
        world
            .soracloud_app_infra_audit_events_mut_for_testing()
            .insert(
                7,
                SoraAppInfraAuditEventV1 {
                    schema_version: SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1,
                    sequence: 7,
                    action: SoraAppInfraActionV1::Deploy,
                    app_name: "sequence_app".parse().expect("valid app name"),
                    from_version: None,
                    to_version: "1.0.0".to_owned(),
                    app_manifest_hash: Hash::new(b"sequence-app-manifest"),
                    service_count: 1,
                    signer: signer.clone(),
                },
            );
        set_watermark(&world, 7);
        assert_eq!(latest_soracloud_sequence(&world.view()), 7);
        assert_eq!(authoritative_soracloud_sequence(&world.view()), 8);

        let evidence_id = Hash::new(b"sequence-host-violation");
        world
            .soracloud_model_host_violation_evidence_mut_for_testing()
            .insert(
                evidence_id,
                SoraModelHostViolationEvidenceRecordV1 {
                    schema_version: SORA_MODEL_HOST_VIOLATION_EVIDENCE_RECORD_VERSION_V1,
                    evidence_id,
                    sequence: 13,
                    validator_account_id: checked_account_id(),
                    kind: SoraModelHostViolationKindV1::AdvertContradiction,
                    placement_id: None,
                    pool_id: None,
                    source_id: None,
                    window_started_at_ms: None,
                    observed_at_ms: 1,
                    detail: Some("cross-domain sequence fixture".to_owned()),
                    strike_count: 1,
                    penalty_applied: false,
                    host_evicted: true,
                    slash_id: None,
                },
            );
        set_watermark(&world, 13);
        assert_eq!(authoritative_soracloud_sequence(&world.view()), 14);

        let receipt_id = Hash::new(b"sequence-runtime-receipt");
        world.soracloud_runtime_receipts_mut_for_testing().insert(
            receipt_id,
            SoraRuntimeReceiptV1 {
                schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id,
                service_name: "sequence_service".parse().expect("valid service name"),
                service_version: "1.0.0".to_owned(),
                handler_name: "query".parse().expect("valid handler name"),
                handler_class: SoraServiceHandlerClassV1::Query,
                request_commitment: Hash::new(b"sequence-request"),
                result_commitment: Hash::new(b"sequence-result"),
                certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
                emitted_sequence: 21,
                mailbox_message_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                execution_host: None,
            },
        );
        set_watermark(&world, 21);
        assert_eq!(authoritative_soracloud_sequence(&world.view()), 22);

        let private_bundle = sample_quantized_uploaded_model_bundle();
        let output_recipient = private_bundle.upload_recipient.clone();
        let mut private_receipt = SoraPrivateUploadedModelExecutionReceiptV1 {
            schema_version: SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1,
            network_id: iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                iroha_data_model::block::BlockHeader,
            >::from_untyped_unchecked(
                Hash::prehashed([0x93; Hash::LENGTH]),
            )),
            receipt_id: Hash::prehashed([0; 32]),
            service_name: private_bundle.service_name,
            service_version: "1.0.0".to_owned(),
            model_id: private_bundle.model_id,
            weight_version: private_bundle.weight_version,
            runtime_version: SORACLOUD_PRIVATE_MODEL_RUNTIME_VERSION_V1.to_owned(),
            model_manifest_digest: private_bundle.sorafs_manifest_digest,
            model_bundle_root: private_bundle.bundle_root,
            policy_id: private_bundle.decryption_policy_ref,
            decryption_request_id: "decrypt-upload-1".to_owned(),
            attesting_validator: checked_validator_host(),
            input_artifact: sample_private_model_artifact_ref("input", 0x51),
            output_artifact: sample_private_model_artifact_ref("output", 0x52),
            input_commitment: Hash::new(b"sequence-private-input"),
            output_commitment: Hash::new(b"sequence-private-output"),
            output_recipient,
            request_commitment: Hash::prehashed([0; 32]),
            result_commitment: Hash::prehashed([0; 32]),
            emitted_sequence: 34,
            emitted_block_height: 1,
        };
        private_receipt.request_commitment =
            derive_soracloud_private_model_request_commitment_v1(&private_receipt);
        private_receipt.result_commitment =
            derive_soracloud_private_model_result_commitment_v1(&private_receipt);
        private_receipt.receipt_id =
            derive_soracloud_private_uploaded_model_execution_receipt_id_v1(&private_receipt);
        let private_receipt_id = private_receipt.receipt_id;
        world
            .soracloud_private_uploaded_model_execution_receipts_mut_for_testing()
            .insert(private_receipt_id, private_receipt);
        set_watermark(&world, 34);
        assert_eq!(latest_soracloud_sequence(&world.view()), 34);
        assert_eq!(authoritative_soracloud_sequence(&world.view()), 35);

        let mut mailbox_message = SoraServiceMailboxMessageV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            message_id: Hash::prehashed([0; Hash::LENGTH]),
            from_service: "sequence_source".parse().expect("valid service name"),
            from_service_version: "1.0.0".to_string(),
            from_handler: "update".parse().expect("valid handler name"),
            to_service: "sequence_destination".parse().expect("valid service name"),
            to_service_version: "1.0.0".to_string(),
            to_handler: "update".parse().expect("valid handler name"),
            payload_bytes: b"sequence-mailbox-payload".to_vec(),
            payload_commitment: Hash::new(b"sequence-mailbox-payload"),
            delivery_delay_blocks: 0,
            enqueue_sequence: 35,
            enqueue_height: 35,
            available_after_height: 35,
            expires_at_height: 40,
        };
        mailbox_message.message_id = derive_soracloud_mailbox_message_id_v1(&mailbox_message);
        world
            .soracloud_mailbox_messages_mut_for_testing()
            .insert(mailbox_message.message_id, mailbox_message);
        set_watermark(&world, 35);
        assert_eq!(latest_soracloud_sequence(&world.view()), 35);
        assert_eq!(authoritative_soracloud_sequence(&world.view()), 36);

        world
            .soracloud_app_infra_audit_events_mut_for_testing()
            .insert(
                u64::MAX,
                SoraAppInfraAuditEventV1 {
                    schema_version: SORA_APP_INFRA_AUDIT_EVENT_VERSION_V1,
                    sequence: u64::MAX,
                    action: SoraAppInfraActionV1::Upgrade,
                    app_name: "sequence_app".parse().expect("valid app name"),
                    from_version: Some("1.0.0".to_owned()),
                    to_version: "2.0.0".to_owned(),
                    app_manifest_hash: Hash::new(b"terminal-sequence-app-manifest"),
                    service_count: 1,
                    signer,
                },
            );
        set_watermark(&world, u64::MAX);
        assert_eq!(latest_soracloud_sequence(&world.view()), u64::MAX);
        assert_eq!(authoritative_soracloud_sequence(&world.view()), u64::MAX);
    }
    #[test]
    fn runtime_snapshot_json_requires_the_exact_v1_field_set() {
        let canonical = SoracloudRuntimeSnapshot::default();
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical runtime snapshot");
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeSnapshot>(canonical_value.clone())
                .expect("decode canonical runtime snapshot"),
            canonical
        );

        let mut unknown = canonical_value.clone();
        unknown
            .as_object_mut()
            .expect("snapshot JSON object")
            .insert("legacy_runtime".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeSnapshot>(unknown).is_err(),
            "same-version snapshots must reject unknown fields"
        );

        for required_field in ["observed_block_hash", "local_peer_id", "hf_sources"] {
            let mut missing = canonical_value.clone();
            missing
                .as_object_mut()
                .expect("snapshot JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudRuntimeSnapshot>(missing).is_err(),
                "same-version snapshots must require the canonically emitted {required_field} field"
            );
        }
    }

    #[test]
    fn runtime_snapshot_nested_records_require_the_exact_v1_field_set() {
        let apartment = SoracloudRuntimeApartmentPlan {
            apartment_name: "apartment".to_owned(),
            manifest_hash: "manifest-hash".to_owned(),
            status: SoraAgentRuntimeStatusV1::Running,
            process_generation: 1,
            lease_expires_height: 100,
            last_active_sequence: 1,
            materialization_dir: "/runtime/apartments/apartment".to_owned(),
            pending_wallet_request_count: 0,
            pending_mailbox_message_count: 0,
            autonomy_budget_remaining_units: 0,
            approved_artifact_count: 0,
            autonomy_run_count: 0,
            revoked_policy_capability_count: 0,
        };
        let mut apartment_value =
            norito::json::to_value(&apartment).expect("encode canonical runtime apartment plan");
        apartment_value
            .as_object_mut()
            .expect("runtime apartment plan JSON object")
            .insert("legacy_status".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeApartmentPlan>(apartment_value).is_err(),
            "same-version runtime apartment plans must reject unknown fields"
        );

        let source = SoracloudRuntimeHfSourcePlan {
            source_id: "source".to_owned(),
            repo_id: "owner/model".to_owned(),
            resolved_revision: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            model_name: "model".to_owned(),
            adapter_id: "adapter".to_owned(),
            authoritative_status: SoraHfSourceStatusV1::Ready,
            runtime_status: SoracloudRuntimeHfSourceStatus::Ready,
            pool_count: 0,
            active_pool_count: 0,
            active_member_count: 0,
            queued_window_count: 0,
            bound_service_count: 0,
            bound_service_names: Vec::new(),
            materialized_service_count: 0,
            materialized_service_names: Vec::new(),
            hydrating_service_count: 0,
            bound_apartment_count: 0,
            bound_apartment_names: Vec::new(),
            materialized_apartment_count: 0,
            materialized_apartment_names: Vec::new(),
            bundle_cache_miss_count: 0,
            artifact_cache_miss_count: 0,
            last_error: None,
        };
        let source_value =
            norito::json::to_value(&source).expect("encode canonical runtime HF source plan");
        assert!(
            source_value
                .get("last_error")
                .is_some_and(norito::json::Value::is_null),
            "canonical runtime HF source plans must emit an explicit null last_error"
        );
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeHfSourcePlan>(source_value.clone())
                .expect("decode canonical runtime HF source plan"),
            source
        );
        for required_field in [
            "bound_service_names",
            "materialized_service_names",
            "bound_apartment_names",
            "materialized_apartment_names",
            "last_error",
        ] {
            let mut missing = source_value.clone();
            missing
                .as_object_mut()
                .expect("runtime HF source plan JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudRuntimeHfSourcePlan>(missing).is_err(),
                "same-version runtime HF source plans must require {required_field}"
            );
        }
        let mut unknown_source = source_value;
        unknown_source
            .as_object_mut()
            .expect("runtime HF source plan JSON object")
            .insert("legacy_source".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeHfSourcePlan>(unknown_source).is_err(),
            "same-version runtime HF source plans must reject unknown fields"
        );
    }

    #[test]
    fn runtime_service_plan_json_requires_the_exact_v1_field_set() {
        let canonical = sample_runtime_service_plan();
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical runtime service plan");
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeServicePlan>(canonical_value.clone())
                .expect("decode canonical runtime service plan"),
            canonical
        );
        let canonical_role = canonical.role;
        let canonical_role_value =
            norito::json::to_value(&canonical_role).expect("encode canonical runtime role");
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeRevisionRole>(canonical_role_value.clone())
                .expect("decode canonical runtime role"),
            canonical_role
        );
        let mut unknown_role = canonical_role_value;
        unknown_role
            .as_object_mut()
            .expect("runtime role JSON object")
            .insert("legacy_role".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeRevisionRole>(unknown_role).is_err(),
            "same-version runtime roles must reject unknown envelope fields"
        );
        for nullable_field in [
            "process_generation",
            "rollout_handle",
            "quota_class",
            "service_lease_status",
            "lease_expires_height",
            "remaining_runtime_balance",
        ] {
            assert!(
                canonical_value
                    .get(nullable_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical runtime service plan must emit explicit null for {nullable_field}"
            );
        }

        let mut unknown = canonical_value.clone();
        unknown
            .as_object_mut()
            .expect("runtime service plan JSON object")
            .insert("legacy_backend".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeServicePlan>(unknown).is_err(),
            "same-version runtime service plans must reject unknown fields"
        );

        for required_field in [
            "inrou",
            "process_generation",
            "desired_replica_count",
            "local_replica_slots",
            "local_replicas",
            "rollout_handle",
            "quota_class",
            "service_lease_status",
            "lease_expires_height",
            "remaining_runtime_balance",
            "config_exports",
            "effective_env",
            "effective_env_materialization_path",
            "config_exports_materialization_dir",
            "lease_volumes",
        ] {
            let mut missing = canonical_value.clone();
            missing
                .as_object_mut()
                .expect("runtime service plan JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudRuntimeServicePlan>(missing).is_err(),
                "same-version runtime service plans must require {required_field}"
            );
        }

        let inrou = canonical.inrou.as_ref().expect("Inrou plan fixture");
        let inrou_value =
            norito::json::to_value(inrou).expect("encode canonical runtime Inrou plan");
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeInrouPlan>(inrou_value.clone())
                .expect("decode canonical runtime Inrou plan"),
            *inrou
        );
        for nullable_field in ["initrd_image_path", "bootstrap_user_data_path"] {
            assert!(
                inrou_value
                    .get(nullable_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical runtime Inrou plan must emit explicit null for {nullable_field}"
            );
        }
        for required_field in [
            "initrd_image_path",
            "bootstrap_user_data_path",
            "ssh_authorized_keys",
        ] {
            let mut missing = inrou_value.clone();
            missing
                .as_object_mut()
                .expect("runtime Inrou plan JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudRuntimeInrouPlan>(missing).is_err(),
                "same-version runtime Inrou plans must require {required_field}"
            );
        }
        let mut unknown_inrou = inrou_value;
        unknown_inrou
            .as_object_mut()
            .expect("runtime Inrou plan JSON object")
            .insert("legacy_backend".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeInrouPlan>(unknown_inrou).is_err(),
            "same-version runtime Inrou plans must reject unknown fields"
        );

        let replica = canonical
            .local_replicas
            .first()
            .expect("runtime replica plan fixture");
        let replica_value =
            norito::json::to_value(replica).expect("encode canonical runtime replica plan");
        assert_eq!(
            norito::json::from_value::<SoracloudRuntimeReplicaPlan>(replica_value.clone())
                .expect("decode canonical runtime replica plan"),
            *replica
        );
        for required_field in ["listen_base_url", "pid", "last_error"] {
            assert!(
                replica_value
                    .get(required_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical runtime replica plan must emit explicit null for {required_field}"
            );
            let mut missing = replica_value.clone();
            missing
                .as_object_mut()
                .expect("runtime replica plan JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudRuntimeReplicaPlan>(missing).is_err(),
                "same-version runtime replica plans must require {required_field}"
            );
        }
        let mut unknown_replica = replica_value;
        unknown_replica
            .as_object_mut()
            .expect("runtime replica plan JSON object")
            .insert("legacy_pid".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeReplicaPlan>(unknown_replica).is_err(),
            "same-version runtime replica plans must reject unknown fields"
        );
    }

    #[test]
    fn runtime_nested_plan_json_requires_the_exact_v1_field_set() {
        let mailbox = SoracloudRuntimeMailboxPlan {
            handler_name: "dispatch".to_owned(),
            queue_name: "dispatch-queue".to_owned(),
            max_pending_messages: 4,
            max_message_bytes: 1024,
            retention_blocks: 16,
        };
        let mut mailbox_value =
            norito::json::to_value(&mailbox).expect("encode canonical runtime mailbox plan");
        mailbox_value
            .as_object_mut()
            .expect("runtime mailbox plan JSON object")
            .insert("legacy_queue".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeMailboxPlan>(mailbox_value).is_err(),
            "same-version runtime mailbox plans must reject unknown fields"
        );

        let artifact = SoracloudRuntimeArtifactPlan {
            kind: SoraArtifactKindV1::Bundle,
            artifact_hash: "artifact-hash".to_owned(),
            artifact_path: "bundle.to".to_owned(),
            handler_name: None,
            local_cache_path: "/runtime/cache/artifact".to_owned(),
            available_locally: false,
        };
        let artifact_value =
            norito::json::to_value(&artifact).expect("encode canonical runtime artifact plan");
        assert!(
            artifact_value
                .get("handler_name")
                .is_some_and(norito::json::Value::is_null),
            "canonical runtime artifact plans must emit an explicit null handler_name"
        );
        let mut missing_handler = artifact_value.clone();
        missing_handler
            .as_object_mut()
            .expect("runtime artifact plan JSON object")
            .remove("handler_name");
        assert!(
            norito::json::from_value::<SoracloudRuntimeArtifactPlan>(missing_handler).is_err(),
            "same-version runtime artifact plans must require handler_name"
        );
        let mut unknown_artifact = artifact_value;
        unknown_artifact
            .as_object_mut()
            .expect("runtime artifact plan JSON object")
            .insert("legacy_handler".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeArtifactPlan>(unknown_artifact).is_err(),
            "same-version runtime artifact plans must reject unknown fields"
        );

        let lease = SoracloudRuntimeLeaseVolumePlan {
            volume_name: "root".to_owned(),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/".to_owned(),
            max_total_bytes: 1024,
            lease_expires_height: 100,
            authoritative_generation: 1,
            local_materialization_dir: "/runtime/volumes/root".to_owned(),
        };
        let mut lease_value =
            norito::json::to_value(&lease).expect("encode canonical runtime lease-volume plan");
        lease_value
            .as_object_mut()
            .expect("runtime lease-volume plan JSON object")
            .insert("legacy_path".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeLeaseVolumePlan>(lease_value).is_err(),
            "same-version runtime lease-volume plans must reject unknown fields"
        );
    }

    #[test]
    fn hosted_http_runtime_state_json_requires_the_exact_v1_field_set() {
        let canonical = SoracloudHostedHttpRuntimeStateV1 {
            schema_version: SORACLOUD_HOSTED_HTTP_RUNTIME_STATE_VERSION_V1,
            service_name: "service".to_owned(),
            service_version: "1.0.0".to_owned(),
            process_generation: 1,
            health_status: SoraServiceHealthStatusV1::Healthy,
            listen_base_url: None,
            pid: None,
            accounted_egress_bytes: 0,
            replicas: vec![SoracloudHostedHttpReplicaRuntimeStateV1 {
                replica_slot: 1,
                health_status: SoraServiceHealthStatusV1::Healthy,
                listen_base_url: None,
                pid: None,
                last_error: None,
                updated_at_ms: 1,
            }],
            last_error: None,
            updated_at_ms: 1,
        };
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical hosted runtime state");
        assert_eq!(
            norito::json::from_value::<SoracloudHostedHttpRuntimeStateV1>(canonical_value.clone())
                .expect("decode canonical hosted runtime state"),
            canonical
        );
        for nullable_field in ["listen_base_url", "pid", "last_error"] {
            assert!(
                canonical_value
                    .get(nullable_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical hosted runtime state must emit explicit null for {nullable_field}"
            );
            assert!(
                canonical_value
                    .pointer(&format!("/replicas/0/{nullable_field}"))
                    .is_some_and(norito::json::Value::is_null),
                "canonical hosted replica state must emit explicit null for {nullable_field}"
            );
        }

        let mut unknown = canonical_value.clone();
        unknown
            .as_object_mut()
            .expect("hosted runtime state JSON object")
            .insert("legacy_backend".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudHostedHttpRuntimeStateV1>(unknown).is_err(),
            "same-version hosted runtime state must reject unknown fields"
        );

        let mut unknown_replica = canonical_value.clone();
        unknown_replica
            .pointer_mut("/replicas/0")
            .and_then(norito::json::Value::as_object_mut)
            .expect("hosted replica JSON object")
            .insert("legacy_pid".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudHostedHttpRuntimeStateV1>(unknown_replica).is_err(),
            "same-version hosted replica state must reject unknown fields"
        );

        for required_field in [
            "listen_base_url",
            "pid",
            "accounted_egress_bytes",
            "replicas",
            "last_error",
        ] {
            let mut missing = canonical_value.clone();
            missing
                .as_object_mut()
                .expect("hosted runtime state JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudHostedHttpRuntimeStateV1>(missing).is_err(),
                "same-version hosted runtime state must require {required_field}"
            );
        }
        for required_field in ["listen_base_url", "pid", "last_error"] {
            let mut missing = canonical_value.clone();
            missing
                .pointer_mut("/replicas/0")
                .and_then(norito::json::Value::as_object_mut)
                .expect("hosted replica JSON object")
                .remove(required_field);
            assert!(
                norito::json::from_value::<SoracloudHostedHttpRuntimeStateV1>(missing).is_err(),
                "same-version hosted replica state must require {required_field}"
            );
        }
    }
    #[test]
    fn apartment_autonomy_summary_json_requires_the_exact_v1_field_set() {
        let canonical = SoracloudApartmentAutonomyExecutionSummaryV1 {
            schema_version: SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
            apartment_name: "ops_agent".to_owned(),
            run_id: "run-1".to_owned(),
            service_name: None,
            service_version: None,
            handler_name: None,
            succeeded: false,
            result_commitment: Hash::new(b"failed autonomy result"),
            checkpoint_artifact_hash: None,
            runtime_receipt: None,
            workflow_steps: vec![SoracloudApartmentAutonomyWorkflowStepSummaryV1 {
                step_index: 0,
                step_id: None,
                request_commitment: Hash::new(b"autonomy request"),
                result_commitment: Hash::new(b"autonomy result"),
                runtime_receipt: None,
                content_type: None,
                response_json: None,
                response_text: None,
            }],
            content_type: None,
            response_json: None,
            response_text: None,
            error: None,
        };
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical autonomy summary");
        norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(
            canonical_value.clone(),
        )
        .expect("decode canonical autonomy summary");

        for nullable_field in [
            "service_name",
            "service_version",
            "handler_name",
            "checkpoint_artifact_hash",
            "runtime_receipt",
            "content_type",
            "response_json",
            "response_text",
            "error",
        ] {
            assert!(
                canonical_value
                    .get(nullable_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical autonomy summary must emit explicit null for {nullable_field}"
            );
            let mut missing = canonical_value.clone();
            missing
                .as_object_mut()
                .expect("autonomy summary JSON object")
                .remove(nullable_field);
            assert!(
                norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(missing)
                    .is_err(),
                "same-version autonomy summary must require {nullable_field}"
            );
        }
        for nullable_field in [
            "step_id",
            "runtime_receipt",
            "content_type",
            "response_json",
            "response_text",
        ] {
            assert!(
                canonical_value
                    .pointer(&format!("/workflow_steps/0/{nullable_field}"))
                    .is_some_and(norito::json::Value::is_null),
                "canonical autonomy workflow step must emit explicit null for {nullable_field}"
            );
            let mut missing = canonical_value.clone();
            missing
                .pointer_mut("/workflow_steps/0")
                .and_then(norito::json::Value::as_object_mut)
                .expect("autonomy workflow-step JSON object")
                .remove(nullable_field);
            assert!(
                norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(missing)
                    .is_err(),
                "same-version autonomy workflow step must require {nullable_field}"
            );
        }

        let mut missing_steps = canonical_value.clone();
        missing_steps
            .as_object_mut()
            .expect("autonomy summary JSON object")
            .remove("workflow_steps");
        assert!(
            norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(missing_steps)
                .is_err(),
            "same-version autonomy summary must require workflow_steps"
        );

        let mut unknown = canonical_value.clone();
        unknown
            .as_object_mut()
            .expect("autonomy summary JSON object")
            .insert("legacy_result".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(unknown)
                .is_err(),
            "same-version autonomy summary must reject unknown fields"
        );

        let mut unknown_step = canonical_value;
        unknown_step
            .pointer_mut("/workflow_steps/0")
            .and_then(norito::json::Value::as_object_mut)
            .expect("autonomy workflow-step JSON object")
            .insert("legacy_result".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudApartmentAutonomyExecutionSummaryV1>(unknown_step)
                .is_err(),
            "same-version autonomy workflow step must reject unknown fields"
        );

        let mut unknown_status = norito::json::to_value(&SoracloudRuntimeHfSourceStatus::Ready)
            .expect("encode canonical HF runtime status");
        unknown_status
            .as_object_mut()
            .expect("HF runtime status JSON object")
            .insert("legacy_status".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudRuntimeHfSourceStatus>(unknown_status).is_err(),
            "same-version HF runtime status must reject unknown fields"
        );
    }
    #[test]
    fn local_read_binding_json_requires_explicit_nullable_slots_and_rejects_unknown_fields() {
        let canonical = SoracloudLocalReadBinding {
            binding_name: None,
            state_key: None,
            payload_commitment: None,
            artifact_hash: None,
        };
        let canonical_value =
            norito::json::to_value(&canonical).expect("encode canonical local-read binding");
        for nullable_field in [
            "binding_name",
            "state_key",
            "payload_commitment",
            "artifact_hash",
        ] {
            assert!(
                canonical_value
                    .get(nullable_field)
                    .is_some_and(norito::json::Value::is_null),
                "canonical local-read binding must emit explicit null for {nullable_field}"
            );
            let mut missing = canonical_value.clone();
            missing
                .as_object_mut()
                .expect("local-read binding JSON object")
                .remove(nullable_field);
            assert!(
                norito::json::from_value::<SoracloudLocalReadBinding>(missing).is_err(),
                "local-read binding must require {nullable_field}"
            );
        }
        let mut unknown = canonical_value;
        unknown
            .as_object_mut()
            .expect("local-read binding JSON object")
            .insert("legacy_binding".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<SoracloudLocalReadBinding>(unknown).is_err(),
            "local-read binding must reject unknown nested fields"
        );
    }
    #[test]
    fn apartment_autonomy_summary_validation_binds_schema_path_and_authoritative_commitment() {
        let process_generation = 7;
        let request_commitment = Hash::new(b"authoritative autonomy request");
        let mut summary = SoracloudApartmentAutonomyExecutionSummaryV1 {
            schema_version: SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1,
            apartment_name: "ops_agent".to_owned(),
            run_id: "run-7".to_owned(),
            service_name: None,
            service_version: None,
            handler_name: None,
            succeeded: false,
            result_commitment: Hash::new(b"placeholder"),
            checkpoint_artifact_hash: None,
            runtime_receipt: None,
            workflow_steps: Vec::new(),
            content_type: None,
            response_json: None,
            response_text: None,
            error: Some("fixture failure".to_owned()),
        };
        summary.result_commitment = derive_soracloud_apartment_autonomy_result_commitment_v1(
            &summary,
            process_generation,
            request_commitment,
        )
        .expect("derive canonical autonomy result commitment");
        validate_soracloud_apartment_autonomy_execution_summary_v1(
            &summary,
            "ops_agent",
            "run-7",
            process_generation,
            request_commitment,
        )
        .expect("canonical autonomy summary must validate");

        let mut changed_success = summary.clone();
        changed_success.succeeded = true;
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &changed_success,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err(),
            "result commitment must bind the success outcome"
        );
        let mut changed_service = summary.clone();
        changed_service.service_name = Some("different_service".to_owned());
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &changed_service,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err(),
            "result commitment must bind resolved service identity"
        );
        let mut changed_receipt = summary.clone();
        changed_receipt.runtime_receipt = Some(SoraRuntimeReceiptV1 {
            schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
            receipt_id: Hash::new(b"injected autonomy receipt"),
            service_name: "different_service".parse().expect("valid service name"),
            service_version: "v1".to_owned(),
            handler_name: "infer".parse().expect("valid handler name"),
            handler_class: SoraServiceHandlerClassV1::Query,
            request_commitment: Hash::new(b"injected receipt request"),
            result_commitment: Hash::new(b"injected receipt result"),
            certified_by: SoraCertifiedResponsePolicyV1::AuditReceipt,
            emitted_sequence: 1,
            mailbox_message_id: None,
            journal_artifact_hash: None,
            checkpoint_artifact_hash: None,
            execution_host: None,
        });
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &changed_receipt,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err(),
            "result commitment must bind the runtime receipt used for authoritative ISIs"
        );
        let mut changed_content_type = summary.clone();
        changed_content_type.content_type = Some("application/json".to_owned());
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &changed_content_type,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err(),
            "result commitment must bind response metadata"
        );

        let mut wrong_schema = summary.clone();
        wrong_schema.schema_version =
            SORACLOUD_APARTMENT_AUTONOMY_EXECUTION_SUMMARY_VERSION_V1.saturating_add(1);
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &wrong_schema,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err()
        );
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &summary,
                "other_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err()
        );
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &summary,
                "ops_agent",
                "run-other",
                process_generation,
                request_commitment,
            )
            .is_err()
        );
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &summary,
                "ops_agent",
                "run-7",
                process_generation.saturating_add(1),
                request_commitment,
            )
            .is_err()
        );
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &summary,
                "ops_agent",
                "run-7",
                process_generation,
                Hash::new(b"different authoritative request"),
            )
            .is_err()
        );
        let mut wrong_result = summary;
        wrong_result.result_commitment = Hash::new(b"different autonomy result");
        assert!(
            validate_soracloud_apartment_autonomy_execution_summary_v1(
                &wrong_result,
                "ops_agent",
                "run-7",
                process_generation,
                request_commitment,
            )
            .is_err()
        );
    }
    fn sample_private_model_artifact_ref(role: &str, seed: u8) -> SoraPrivateModelArtifactRefV1 {
        SoraPrivateModelArtifactRefV1 {
            schema_version: iroha_data_model::soracloud::SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
            sorafs_manifest_digest: ManifestDigest::new([seed; 32]),
            sorafs_root_cid:
                iroha_data_model::sorafs::pin_registry::ManifestRootCid::from_blake3_digest(
                    [seed; 32],
                )
                .expect("fixture root CID"),
            artifact_hash: Hash::new([seed; 16]),
            ciphertext_bytes: 128,
            artifact_role: role.to_owned(),
        }
    }
    fn sample_quantized_uploaded_model_bundle() -> SoraUploadedModelBundleV1 {
        let public_key_bytes = vec![7u8; 32];
        let wrapped_key_ciphertext = vec![9u8; 48];
        SoraUploadedModelBundleV1 {
            schema_version: iroha_data_model::soracloud::SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
            service_name: "private_model_host".parse().expect("valid service name"),
            model_id: "upload-quant-v1".to_owned(),
            weight_version: "v1".to_owned(),
            family: "linear-demo".to_owned(),
            modalities: vec!["tabular".to_owned()],
            plaintext_root: Hash::new(b"plaintext model root"),
            runtime_format: SoraUploadedModelRuntimeFormatV1::DeterministicQuantizedCpuV1,
            bundle_root: Hash::new(b"quantized bundle root"),
            sorafs_manifest_digest: ManifestDigest::new([0xA5; 32]),
            chunk_count: 1,
            plaintext_bytes: 256,
            ciphertext_bytes: 512,
            chunk_manifest_root: Hash::new(b"chunk manifest root"),
            upload_recipient: iroha_data_model::soracloud::SoraUploadedModelEncryptionRecipientV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
                key_id: "recipient".to_owned(),
                key_version: std::num::NonZeroU32::new(1).expect("non-zero key version"),
                kem: SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                aead: SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                public_key_bytes: public_key_bytes.clone(),
                public_key_fingerprint: Hash::new(public_key_bytes.as_slice()),
            },
            wrapped_bundle_key: iroha_data_model::soracloud::SoraUploadedModelWrappedKeyV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1,
                recipient_key_id: "recipient".to_owned(),
                recipient_key_version: std::num::NonZeroU32::new(1).expect("non-zero key version"),
                kem: SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
                aead: SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
                ephemeral_public_key: vec![8u8; 32],
                nonce: vec![5u8; 12],
                wrapped_key_ciphertext: wrapped_key_ciphertext.clone(),
                ciphertext_hash: Hash::new(wrapped_key_ciphertext.as_slice()),
                aad_digest: Hash::new(b"aad"),
            },
            pricing_policy: iroha_data_model::soracloud::SoraUploadedModelPricingPolicyV1 {
                storage_price: "0.000000001"
                    .parse()
                    .expect("canonical uploaded-model storage price"),
            },
            decryption_policy_ref: "policy/v1".to_owned(),
        }
    }
    fn seed_generated_hf_world_with_primary(
        primary_peer_id: &str,
    ) -> (World, String, String, String) {
        let mut world = World::new();
        let mailbox_pulse_id = [0x42; 32];
        {
            let mut block = world.block();
            block.global_beacon_pulses.insert(
                mailbox_pulse_id,
                iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1 {
                    version: iroha_data_model::consensus::GLOBAL_THRESHOLD_BEACON_VERSION_V1,
                    network_id:
                        iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                            iroha_data_model::block::BlockHeader,
                        >::from_untyped_unchecked(
                            Hash::prehashed([0x11; Hash::LENGTH]),
                        )),
                    session_id: [0x22; 32],
                    roster_hash: [0x23; 32],
                    transcript_hash: [0x24; 32],
                    height: 4,
                    round: 0,
                    finalized_chain_anchor:
                        iroha_data_model::consensus::GlobalThresholdBeaconChainAnchorV1 {
                            height: 3,
                            block_hash: iroha_crypto::HashOf::from_untyped_unchecked(
                                Hash::prehashed([0x12; Hash::LENGTH]),
                            ),
                        },
                    signature: [0x25; 48],
                    seed: [0x26; 32],
                    pulse_id: mailbox_pulse_id,
                },
            );
            block.commit();
        }
        let service_name: Name = "hf_service".parse().expect("valid service name");
        let service_name_string = service_name.as_ref().to_owned();
        let source_id = Hash::new(b"hf-source");
        let pool_id = Hash::new(b"hf-pool");
        let primary_peer_id: PeerId = primary_peer_id.parse().expect("valid primary peer id");
        let primary_validator = AccountId::new(primary_peer_id.public_key().clone());
        let member_account = checked_account_id();
        world.public_lane_validators_mut_for_testing().insert(
            (
                iroha_data_model::nexus::LaneId::SINGLE,
                primary_validator.clone(),
            ),
            iroha_data_model::nexus::staking::PublicLaneValidatorRecord {
                lane_id: iroha_data_model::nexus::LaneId::SINGLE,
                validator: primary_validator.clone(),
                peer_id: primary_peer_id.clone(),
                stake_account: primary_validator.clone(),
                total_stake: Quantity::from(1_u64),
                self_stake: Quantity::from(1_u64),
                metadata: iroha_data_model::metadata::Metadata::default(),
                status: PublicLaneValidatorStatus::Active,
                activation_epoch: Some(0),
                activation_height: Some(0),
                last_reward_epoch: None,
            },
        );
        let bundle = build_soracloud_hf_generated_service_bundle(
            service_name.clone(),
            &source_id.to_string(),
            "openai/gpt-oss",
            "0123456789abcdef0123456789abcdef01234567",
            "gpt-oss",
        );
        let service_version = bundle.service.service_version.clone();
        world.soracloud_service_revisions_mut_for_testing().insert(
            (service_name_string.clone(), service_version.clone()),
            bundle.clone(),
        );
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(
                service_name,
                SoraServiceDeploymentStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                    current_service_version: service_version.clone(),
                    current_service_manifest_hash: bundle.service_manifest_hash(),
                    current_container_manifest_hash: bundle.container_manifest_hash(),
                    revision_count: 1,
                    process_generation: 1,
                    process_started_sequence: 1,
                    active_rollout: None,
                    last_rollout: None,
                    config_generation: 0,
                    secret_generation: 0,
                    service_configs: BTreeMap::new(),
                    service_secrets: BTreeMap::new(),
                    fhe_policy_records: BTreeMap::new(),
                    service_name: bundle.service.service_name.clone(),
                    service_lease: None,
                    lease_volume_states: Vec::new(),
                },
            );
        world
            .soracloud_hf_shared_lease_pools_mut_for_testing()
            .insert(
                pool_id,
                SoraHfSharedLeasePoolV1 {
                    schema_version: SORA_HF_SHARED_LEASE_POOL_VERSION_V1,
                    pool_id,
                    source_id,
                    storage_class: StorageClass::Warm,
                    lease_asset_definition_id: AssetDefinitionId::derive_from_components(
                        DomainId::try_new("wonderland", "universal").expect("domain"),
                        "xor".parse().expect("asset"),
                    ),
                    base_fee: "0.00001".parse().expect("base fee"),
                    lease_term_ms: 60_000,
                    window_started_at_ms: 1,
                    window_expires_at_ms: 60_001,
                    active_member_count: 1,
                    status: SoraHfSharedLeaseStatusV1::Active,
                    queued_next_window: None,
                },
            );
        world
            .soracloud_hf_shared_lease_members_mut_for_testing()
            .insert(
                (pool_id.to_string(), member_account.to_string()),
                SoraHfSharedLeaseMemberV1 {
                    schema_version: SORA_HF_SHARED_LEASE_MEMBER_VERSION_V1,
                    pool_id,
                    source_id,
                    account_id: member_account,
                    status: SoraHfSharedLeaseMemberStatusV1::Active,
                    joined_at_ms: 1,
                    updated_at_ms: 1,
                    total_paid: "0.00001".parse().expect("total paid"),
                    total_refunded: Quantity::zero(),
                    last_charge: "0.00001".parse().expect("last charge"),
                    total_compute_paid: "0.000005".parse().expect("total compute paid"),
                    total_compute_refunded: Quantity::zero(),
                    last_compute_charge: "0.000005".parse().expect("last compute charge"),
                    service_bindings: std::collections::BTreeSet::from([
                        service_name_string.clone()
                    ]),
                    apartment_bindings: std::collections::BTreeSet::new(),
                },
            );
        world
            .soracloud_model_host_capabilities_mut_for_testing()
            .insert(
                primary_validator.clone(),
                SoraModelHostCapabilityRecordV1 {
                    schema_version: SORA_MODEL_HOST_CAPABILITY_RECORD_VERSION_V1,
                    validator_account_id: primary_validator.clone(),
                    peer_id: primary_peer_id.to_string(),
                    supported_backends: BTreeSet::from([SoraHfBackendFamilyV1::Transformers]),
                    supported_formats: BTreeSet::from([SoraHfModelFormatV1::Safetensors]),
                    max_model_bytes: 8 * 1024 * 1024 * 1024,
                    max_disk_cache_bytes: 32 * 1024 * 1024 * 1024,
                    max_ram_bytes: 32 * 1024 * 1024 * 1024,
                    max_vram_bytes: 0,
                    max_concurrent_resident_models: 2,
                    host_class: "gpu.large".to_owned(),
                    advertised_at_ms: 1,
                    heartbeat_expires_at_ms: u64::MAX,
                },
            );
        let selection_seed_hash = Hash::new(b"hf-seed");
        world.soracloud_hf_placements_mut_for_testing().insert(
            pool_id,
            SoraHfPlacementRecordV1 {
                schema_version: SORA_HF_PLACEMENT_RECORD_VERSION_V1,
                placement_id: derive_hf_placement_id_v1(pool_id, selection_seed_hash)
                    .expect("canonical HF placement id"),
                source_id,
                pool_id,
                status: SoraHfPlacementStatusV1::Ready,
                selection_seed_hash,
                resource_profile: SoraHfResourceProfileV1 {
                    required_model_bytes: 1024,
                    backend_family: SoraHfBackendFamilyV1::Transformers,
                    model_format: SoraHfModelFormatV1::Safetensors,
                    selected_weight_file_count: 1,
                    weight_selection_commitment: Hash::new(b"hf-runtime-test-selection"),
                    disk_cache_bytes_floor: 2048,
                    ram_bytes_floor: 2048,
                    vram_bytes_floor: 0,
                },
                eligible_validator_count: 1,
                adaptive_target_host_count: 1,
                assigned_hosts: vec![SoraHfPlacementHostAssignmentV1 {
                    validator_account_id: primary_validator,
                    peer_id: primary_peer_id.to_string(),
                    role: SoraHfPlacementHostRoleV1::Primary,
                    status: SoraHfPlacementHostStatusV1::Warm,
                    host_class: "gpu.large".to_owned(),
                }],
                total_reservation_fee: "0.000005".parse().expect("total reservation fee"),
                last_rebalance_at_ms: 1,
                last_error: None,
            },
        );
        (
            world,
            service_name_string,
            service_version,
            source_id.to_string(),
        )
    }
    #[test]
    fn generated_hf_service_bundle_is_admissible_and_tagged() {
        let bundle = build_soracloud_hf_generated_service_bundle(
            "hf_service".parse().expect("valid service name"),
            "hash:1111111111111111111111111111111111111111111111111111111111111111#0001",
            "openai/gpt-oss",
            "0123456789abcdef0123456789abcdef01234567",
            "gpt-oss",
        );
        bundle
            .validate_for_admission()
            .expect("generated HF bundle should validate");
        assert!(bundle.container.capabilities.allow_model_inference);
        assert!(!bundle.container.capabilities.allow_state_writes);
        assert_eq!(
            bundle.service.service_version,
            HF_GENERATED_SERVICE_VERSION_V1
        );
        assert_eq!(
            bundle
                .service
                .route
                .as_ref()
                .expect("generated route")
                .visibility,
            SoraRouteVisibilityV1::Internal
        );
        let binding = soracloud_hf_generated_source_binding(&bundle)
            .expect("generated bundle should expose HF markers");
        assert_eq!(binding.repo_id, "openai/gpt-oss");
        assert_eq!(
            binding.resolved_revision,
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert_eq!(binding.model_name, "gpt-oss");
    }
    #[test]
    fn generated_hf_bundle_payload_matches_declared_bundle_hash() {
        let bundle = build_soracloud_hf_generated_service_bundle(
            "hf_service".parse().expect("valid service name"),
            "hash:2222222222222222222222222222222222222222222222222222222222222222#0002",
            "meta/llama",
            "1123456789abcdef0123456789abcdef01234567",
            "llama",
        );
        let payload = soracloud_hf_generated_bundle_payload_if_applicable(&bundle)
            .expect("generated HF bundle should synthesize payload");
        assert_eq!(Hash::new(&payload), bundle.container.bundle_hash);
    }
    #[test]
    fn generated_hf_agent_manifest_tracks_service_container_and_host() {
        let bundle = build_soracloud_hf_generated_service_bundle(
            "hf_agent_service".parse().expect("valid service name"),
            "hash:3333333333333333333333333333333333333333333333333333333333333333#0003",
            "huggingface/smol",
            "2123456789abcdef0123456789abcdef01234567",
            "smol",
        );
        let manifest = build_soracloud_hf_generated_agent_manifest(
            "hf_agent".parse().expect("valid apartment name"),
            &bundle,
        );
        manifest
            .validate()
            .expect("generated HF apartment manifest should validate");
        assert_eq!(
            manifest.container.manifest_hash,
            bundle.container_manifest_hash()
        );
        assert_eq!(
            manifest.network_egress,
            SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
                bundle
                    .service
                    .route
                    .as_ref()
                    .expect("generated service route")
                    .host
                    .clone(),
                [443],
            )])
        );
        assert_eq!(manifest.tool_capabilities.len(), 2);
    }
    #[test]
    fn resolve_generated_hf_primary_assignment_returns_warm_primary_for_bound_service() {
        let primary_peer_id = checked_peer_id().to_string();
        let (world, service_name, _service_version, source_id) =
            seed_generated_hf_world_with_primary(&primary_peer_id);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query_handle);
        let view = state.view();
        let primary = resolve_generated_hf_primary_assignment(
            view.world(),
            &service_name,
            &source_id,
            1,
            |lane_id| view.is_lane_active_for_authority(lane_id),
        )
        .expect("primary lookup should succeed")
        .expect("generated service should resolve a primary assignment");
        assert_eq!(primary.peer_id, primary_peer_id);
        assert_eq!(primary.role, SoraHfPlacementHostRoleV1::Primary);
        assert_eq!(primary.status, SoraHfPlacementHostStatusV1::Warm);
    }
    #[test]
    fn validator_peer_rebind_invalidates_stale_hf_capability_and_assignment() {
        let stale_peer_id = checked_peer_id().to_string();
        let (mut world, service_name, _service_version, source_id) =
            seed_generated_hf_world_with_primary(&stale_peer_id);
        let (validator_key, mut validator_record) = world
            .public_lane_validators
            .view()
            .iter()
            .next()
            .map(|(key, record)| (key.clone(), record.clone()))
            .expect("primary validator record");
        let validator_account_id = validator_record.validator.clone();
        let rebound_peer_id = checked_peer_id();
        assert_ne!(rebound_peer_id.to_string(), stale_peer_id);
        validator_record.peer_id = rebound_peer_id.clone();
        world
            .public_lane_validators_mut_for_testing()
            .insert(validator_key, validator_record);

        {
            let world_view = world.view();
            assert!(!soracloud_validator_is_active(
                &world_view,
                &validator_account_id,
                |lane_id| lane_id == LaneId::SINGLE,
            ));
            assert!(!soracloud_validator_has_active_peer_binding(
                &world_view,
                &validator_account_id,
                &stale_peer_id,
                |lane_id| lane_id == LaneId::SINGLE,
            ));
            assert!(!soracloud_validator_has_active_peer_binding(
                &world_view,
                &validator_account_id,
                &rebound_peer_id.to_string(),
                |lane_id| lane_id == LaneId::SINGLE,
            ));
        }

        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let view = state.view();
        assert!(
            resolve_generated_hf_primary_assignment(
                view.world(),
                &service_name,
                &source_id,
                1,
                |lane_id| view.is_lane_active_for_authority(lane_id),
            )
            .expect("peer-rebound primary lookup must fail closed without an error")
            .is_none(),
            "a stale capability and placement must not survive an authoritative validator peer rebind"
        );
    }
    #[test]
    fn resolve_generated_hf_primary_assignment_rejects_non_serving_stale_warm_primary() {
        let primary_peer_id = checked_peer_id().to_string();
        for non_serving_status in [
            SoraHfPlacementStatusV1::Selecting,
            SoraHfPlacementStatusV1::Warming,
            SoraHfPlacementStatusV1::Unavailable,
            SoraHfPlacementStatusV1::Retired,
        ] {
            let (mut world, service_name, _service_version, source_id) =
                seed_generated_hf_world_with_primary(&primary_peer_id);
            let (pool_id, mut placement) = world
                .soracloud_hf_placements
                .view()
                .iter()
                .next()
                .map(|(pool_id, placement)| (*pool_id, placement.clone()))
                .expect("generated HF placement fixture");
            placement.status = non_serving_status;
            assert_eq!(
                placement.assigned_hosts[0].status,
                SoraHfPlacementHostStatusV1::Warm,
                "fixture must retain a stale historical Warm assignment"
            );
            world
                .soracloud_hf_placements_mut_for_testing()
                .insert(pool_id, placement);
            let state = State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let view = state.view();
            assert!(
                resolve_generated_hf_primary_assignment(
                    view.world(),
                    &service_name,
                    &source_id,
                    1,
                    |lane_id| view.is_lane_active_for_authority(lane_id),
                )
                .expect("non-serving placement lookup must fail closed")
                .is_none(),
                "a {non_serving_status:?} placement must not route through a stale Warm assignment and live capability"
            );
        }
    }
    #[test]
    fn resolve_generated_hf_active_placement_rejects_cross_bound_placement() {
        let primary_peer_id = checked_peer_id().to_string();
        for corrupt_source_id in [false, true] {
            let (mut world, service_name, _service_version, source_id) =
                seed_generated_hf_world_with_primary(&primary_peer_id);
            let (pool_id, mut placement) = world
                .soracloud_hf_placements
                .view()
                .iter()
                .next()
                .map(|(pool_id, placement)| (*pool_id, placement.clone()))
                .expect("generated HF placement fixture");
            if corrupt_source_id {
                placement.source_id = Hash::new(b"cross-bound-hf-source");
            } else {
                placement.pool_id = Hash::new(b"cross-bound-hf-pool");
            }
            world
                .soracloud_hf_placements_mut_for_testing()
                .insert(pool_id, placement);
            let state = State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let view = state.view();
            let error =
                resolve_generated_hf_active_placement(view.world(), &service_name, &source_id, 1)
                    .expect_err("cross-bound authoritative placement must fail closed");
            assert!(
                error.contains("does not match pool"),
                "unexpected error: {error}"
            );
        }
    }
    #[test]
    fn resolve_generated_hf_primary_assignment_rejects_stale_mismatched_or_inactive_capability()
    -> Result<(), String> {
        let primary_peer_id = checked_peer_id().to_string();
        let (mut expired_world, service_name, _service_version, source_id) =
            seed_generated_hf_world_with_primary(&primary_peer_id);
        let primary_validator = expired_world
            .soracloud_hf_placements
            .view()
            .iter()
            .next()
            .and_then(|(_pool_id, placement)| placement.assigned_hosts.first())
            .map(|assignment| assignment.validator_account_id.clone())
            .expect("primary validator");
        let mut expired_capability = expired_world
            .soracloud_model_host_capabilities
            .view()
            .get(&primary_validator)
            .cloned()
            .expect("primary capability");
        expired_capability.heartbeat_expires_at_ms = 10;
        expired_world
            .soracloud_model_host_capabilities_mut_for_testing()
            .insert(primary_validator, expired_capability);
        let state = State::new_for_testing(
            expired_world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let view = state.view();
        assert!(
            resolve_generated_hf_primary_assignment(
                view.world(),
                &service_name,
                &source_id,
                11,
                |lane_id| view.is_lane_active_for_authority(lane_id),
            )?
            .is_none(),
            "an expired capability must not remain routable through a warm placement"
        );

        let (mut mismatched_world, service_name, _service_version, source_id) =
            seed_generated_hf_world_with_primary(&primary_peer_id);
        let primary_validator = mismatched_world
            .soracloud_hf_placements
            .view()
            .iter()
            .next()
            .and_then(|(_pool_id, placement)| placement.assigned_hosts.first())
            .map(|assignment| assignment.validator_account_id.clone())
            .expect("primary validator");
        let mut mismatched_capability = mismatched_world
            .soracloud_model_host_capabilities
            .view()
            .get(&primary_validator)
            .cloned()
            .expect("primary capability");
        mismatched_capability.peer_id = "12D3KooWMismatchedRuntimePeer".to_owned();
        mismatched_world
            .soracloud_model_host_capabilities_mut_for_testing()
            .insert(primary_validator, mismatched_capability);
        let state = State::new_for_testing(
            mismatched_world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let view = state.view();
        assert!(
            resolve_generated_hf_primary_assignment(
                view.world(),
                &service_name,
                &source_id,
                1,
                |lane_id| view.is_lane_active_for_authority(lane_id),
            )?
            .is_none(),
            "a capability identity mismatch must fail generated-HF routing closed"
        );

        let (mut malformed_world, service_name, _service_version, source_id) =
            seed_generated_hf_world_with_primary(&primary_peer_id);
        let primary_validator = malformed_world
            .soracloud_hf_placements
            .view()
            .iter()
            .next()
            .and_then(|(_pool_id, placement)| placement.assigned_hosts.first())
            .map(|assignment| assignment.validator_account_id.clone())
            .expect("primary validator");
        let mut malformed_capability = malformed_world
            .soracloud_model_host_capabilities
            .view()
            .get(&primary_validator)
            .cloned()
            .expect("primary capability");
        malformed_capability.schema_version = 0;
        malformed_world
            .soracloud_model_host_capabilities_mut_for_testing()
            .insert(primary_validator, malformed_capability);
        let state = State::new_for_testing(
            malformed_world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let view = state.view();
        assert!(
            resolve_generated_hf_primary_assignment(
                view.world(),
                &service_name,
                &source_id,
                1,
                |lane_id| view.is_lane_active_for_authority(lane_id),
            )?
            .is_none(),
            "a malformed capability record must fail generated-HF routing closed"
        );

        let (mut inactive_world, service_name, _service_version, source_id) =
            seed_generated_hf_world_with_primary(&primary_peer_id);
        let primary_validator = inactive_world
            .soracloud_hf_placements
            .view()
            .iter()
            .next()
            .and_then(|(_pool_id, placement)| placement.assigned_hosts.first())
            .map(|assignment| assignment.validator_account_id.clone())
            .expect("primary validator");
        let validator_key = (iroha_data_model::nexus::LaneId::SINGLE, primary_validator);
        let mut validator_record = inactive_world
            .public_lane_validators
            .view()
            .get(&validator_key)
            .cloned()
            .expect("primary validator record");
        validator_record.status = PublicLaneValidatorStatus::Exited;
        inactive_world
            .public_lane_validators_mut_for_testing()
            .insert(validator_key, validator_record);
        let state = State::new_for_testing(
            inactive_world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let view = state.view();
        assert!(
            resolve_generated_hf_primary_assignment(
                view.world(),
                &service_name,
                &source_id,
                1,
                |lane_id| view.is_lane_active_for_authority(lane_id),
            )?
            .is_none(),
            "an exited validator must stop serving immediately even while its advert TTL is active"
        );

        let (mut inactive_lane_world, service_name, _service_version, source_id) =
            seed_generated_hf_world_with_primary(&primary_peer_id);
        let (validator_key, mut validator_record) = inactive_lane_world
            .public_lane_validators
            .view()
            .iter()
            .next()
            .map(|(key, record)| (key.clone(), record.clone()))
            .expect("primary validator record");
        let inactive_lane_id = LaneId::new(1);
        validator_record.lane_id = inactive_lane_id;
        {
            let mut validators = inactive_lane_world
                .public_lane_validators_mut_for_testing()
                .block();
            validators.remove(validator_key);
            validators.insert(
                (inactive_lane_id, validator_record.validator.clone()),
                validator_record,
            );
            validators.commit();
        }
        let state = State::new_for_testing(
            inactive_lane_world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let view = state.view();
        assert!(
            resolve_generated_hf_primary_assignment(
                view.world(),
                &service_name,
                &source_id,
                1,
                |lane_id| view.is_lane_active_for_authority(lane_id),
            )?
            .is_none(),
            "a validator record on an inactive lane must not authorize generated-HF serving"
        );
        Ok(())
    }
    #[test]
    fn private_uploaded_model_quantized_cpu_operator_is_deterministic() {
        let model = SoracloudQuantizedCpuModelV1 {
            input_len: 3,
            output_len: 2,
            weights_i8: vec![2, -1, 4, -3, 1, 2],
            bias_i32: vec![3, -4],
            output_shift: 1,
            output_min: -32,
            output_max: 32,
            rounding: SoracloudQuantizedRoundingV1::NearestAwayFromZero,
        };
        let first = model.evaluate(&[7, -2, 5]).expect("run");
        let second = model.evaluate(&[7, -2, 5]).expect("rerun");
        assert_eq!(first, vec![20, -9]);
        assert_eq!(first, second);
    }
    #[test]
    fn private_uploaded_model_quantized_cpu_operator_rejects_wrong_input_shape() {
        let model = SoracloudQuantizedCpuModelV1 {
            input_len: 1,
            output_len: 1,
            weights_i8: vec![1],
            bias_i32: vec![0],
            output_shift: 0,
            output_min: i32::MIN,
            output_max: i32::MAX,
            rounding: SoracloudQuantizedRoundingV1::NearestAwayFromZero,
        };
        let err = model
            .evaluate(&[])
            .expect_err("wrong input shape must fail closed");
        assert_eq!(err.kind, SoracloudRuntimeExecutionErrorKind::InvalidRequest);
    }
    #[test]
    fn runtime_execution_error_has_stable_user_facing_context() {
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            "missing canonical step_id",
        );
        assert_eq!(
            error.to_string(),
            "Soracloud runtime invalid request error: missing canonical step_id"
        );
        assert!(std::error::Error::source(&error).is_none());
    }
}
