//! Moderation operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn moderation_panel_notification_archive_qualify(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let slot = request.binding.slot;
    let qualify = decode_canonical::<ModerationPanelNotificationArchiveQualifyRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )?;
    validate_moderation_panel_notification_archive_wire_scope(
        qualify.version,
        qualify.slot,
        &qualify.network_id,
        &state.network_id,
    )?;
    let archive = broker_backend!(state, moderation_panel_notification_archive);
    let qualification = archive
        .qualification()
        .map_err(|_| BrokerError::StaleOrRevoked)?;
    let exact = required_binding_value!(
        &request.binding,
        moderation_panel_notification_archive_binding
    );
    if archive.handle() != request.binding.handle
        || qualification.revision() != required_binding_value!(&request.binding, revision)
        || qualification.policy_digest() != required_binding_value!(&request.binding, policy_digest)
        || archive.archive_id() != exact.archive_id
        || archive.signing_public_key() != exact.public_key
    {
        return Err(BrokerError::BindingMismatch);
    }
    encode_canonical(
        &ModerationPanelNotificationArchiveQualificationWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot,
            revision: qualification.revision(),
            policy_digest: qualification.policy_digest(),
            archive_id: exact.archive_id,
            public_key: exact.public_key,
        },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
}

pub(super) fn qualify_quarantine_wrapper(
    state: &BrokerServerStateV1,
) -> Result<Vec<u8>, BrokerError> {
    let qualification = broker_backend!(state, moderation_quarantine_key_wrapper)
        .qualification()
        .map_err(|error| match error {
            sorafs_node::ModerationQuarantineKeyProviderReadinessErrorV1::Unavailable => {
                BrokerError::Unavailable
            }
            sorafs_node::ModerationQuarantineKeyProviderReadinessErrorV1::Rejected => {
                BrokerError::StaleOrRevoked
            }
        })?;
    encode_canonical(
        &QualificationResultWireV1 {
            revision: qualification.revision(),
            policy_digest: qualification.policy_digest(),
        },
        MAX_OPERATION_FRAME_BYTES_V1,
    )
}

pub(super) fn moderation_quarantine_wrap_dek(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wrap = decode_canonical::<ModerationQuarantineWrapDekRequestWireV1>(
        &request.payload,
        MAX_MODERATION_QUARANTINE_OPERATION_BYTES_V1,
    )?;
    validate_moderation_quarantine_context_and_dek(wrap.context_digest, wrap.dek)?;
    let mut wrapped_dek = ScrubbedBytes::new(
        broker_backend!(state, moderation_quarantine_key_wrapper)
            .wrap_dek(wrap.context_digest, &wrap.dek)
            .map_err(|error| moderation_quarantine_operation_error(error, true))?,
    );
    validate_moderation_quarantine_wrapped_dek(&wrapped_dek)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(
        &ModerationQuarantineWrapDekResultWireV1 {
            wrapped_dek: wrapped_dek.take(),
        },
        MAX_MODERATION_QUARANTINE_OPERATION_BYTES_V1,
    )
}

pub(super) fn moderation_quarantine_unwrap_dek(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let unwrap = decode_nested_canonical::<ModerationQuarantineUnwrapDekRequestWireV1>(
        &request.payload,
        MAX_MODERATION_QUARANTINE_OPERATION_BYTES_V1,
    )?;
    validate_moderation_quarantine_key_id(&unwrap.key_id)?;
    if unwrap.context_digest == [0; 32] {
        return Err(BrokerError::Rejected);
    }
    validate_moderation_quarantine_wrapped_dek(&unwrap.wrapped_dek)?;
    let unwrapped = ModerationQuarantineUnwrapDekResultWireV1 {
        dek: broker_backend!(state, moderation_quarantine_key_wrapper)
            .unwrap_dek(&unwrap.key_id, unwrap.context_digest, &unwrap.wrapped_dek)
            .map_err(|error| moderation_quarantine_operation_error(error, false))?,
    };
    if unwrapped.dek == [0; 32] {
        return Err(BrokerError::Rejected);
    }
    requalify()?;
    encode_canonical(&unwrapped, MAX_MODERATION_QUARANTINE_OPERATION_BYTES_V1)
}

pub(super) fn moderation_handoff_deliver_once(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let slot = request.binding.slot;
    let moderation_settlement_handoff_slot =
        IrohaRuntimeProviderSlotV1::ModerationSettlementHandoff.wire_id();
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<ModerationDurableHandoffRequestWireV1>(
        &request.payload,
        MAX_MODERATION_HANDOFF_FRAME_BYTES_V1,
    )?;
    let handoff = validate_moderation_handoff_request(&wire, slot, Some(&state.network_id))?;
    let boundary = if slot == moderation_settlement_handoff_slot {
        state.backends.moderation_settlement_handoff.as_ref()
    } else {
        state.backends.moderation_publication_handoff.as_ref()
    }
    .ok_or(BrokerError::BindingMismatch)?;
    let outcome = boundary.deliver_once(&handoff).map_err(|error| {
        match error {
    iroha_torii::sorafs::moderation_runtime::
        ModerationDurableHandoffFailureV1::NotDelivered => {
        BrokerError::Unavailable
    }
    iroha_torii::sorafs::moderation_runtime::
        ModerationDurableHandoffFailureV1::Ambiguous => BrokerError::Ambiguous,
    iroha_torii::sorafs::moderation_runtime::
        ModerationDurableHandoffFailureV1::Permanent => BrokerError::Rejected,
}
    })?;
    let outcome = ModerationDurableHandoffOutcomeWireV1 {
    outcome: match outcome {
        iroha_torii::sorafs::moderation_runtime::
            ModerationDurableHandoffOutcomeV1::Delivered => 1,
        iroha_torii::sorafs::moderation_runtime::
            ModerationDurableHandoffOutcomeV1::AlreadyDelivered => 2,
    },
};
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&outcome, MAX_MODERATION_HANDOFF_FRAME_BYTES_V1)
        .map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn moderation_panel_notification_archive_head_publish(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let slot = request.binding.slot;
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<ModerationPanelNotificationArchiveHeadPublishRequestWireV1>(
        &request.payload,
        MAX_MODERATION_HANDOFF_FRAME_BYTES_V1,
    )?;
    let publication = validate_moderation_panel_notification_archive_head_publish_request(
        &wire,
        &state.network_id,
    )?;
    let (validated_head, validated) =
        validate_moderation_panel_notification_archive_head_at_broker_boundary(
            &publication.canonical_head,
            &state.network_id,
            &state.catalog,
        )?;
    if validated_head != publication.head {
        return Err(BrokerError::Rejected);
    }
    let outcome = broker_backend!(state, moderation_publication_handoff)
        .publish_archive_head_once(&publication)
        .map_err(|error| {
            match error {
        iroha_torii::sorafs::moderation_runtime::
            ModerationDurableHandoffFailureV1::NotDelivered => {
            BrokerError::Unavailable
        }
        iroha_torii::sorafs::moderation_runtime::
            ModerationDurableHandoffFailureV1::Ambiguous => {
            BrokerError::Ambiguous
        }
        iroha_torii::sorafs::moderation_runtime::
            ModerationDurableHandoffFailureV1::Permanent => {
            BrokerError::Rejected
        }
    }
        })?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(
    &ModerationPanelNotificationArchiveHeadPublishResultWireV1 {
        version:
            MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
        slot,
        operation_id: validated.operation_id,
        head_digest: validated.head_digest,
        chain_commitment: validated.chain_commitment,
        outcome: match outcome {
            iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffOutcomeV1::Delivered => 1,
            iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffOutcomeV1::AlreadyDelivered => 2,
        },
    },
    MAX_MODERATION_HANDOFF_FRAME_BYTES_V1,
)
.map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn moderation_panel_notification_archive_head_read(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let slot = request.binding.slot;
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    decode_canonical::<()>(&request.payload, MAX_MODERATION_HANDOFF_FRAME_BYTES_V1)?;
    let head = broker_backend!(state, moderation_publication_handoff)
        .read_published_archive_head()
        .map_err(|error| {
            match error {
        iroha_torii::sorafs::moderation_runtime::
            ModerationDurableHandoffFailureV1::NotDelivered => {
                BrokerError::Unavailable
            }
        iroha_torii::sorafs::moderation_runtime::
            ModerationDurableHandoffFailureV1::Ambiguous => {
                BrokerError::Ambiguous
            }
        iroha_torii::sorafs::moderation_runtime::
            ModerationDurableHandoffFailureV1::Permanent => {
                BrokerError::Rejected
            }
    }
        })?;
    let canonical_head = head
        .as_ref()
        .map(norito::to_bytes)
        .transpose()
        .map_err(|_| BrokerError::Rejected)?;
    if let (Some(head), Some(canonical_head)) = (head.as_ref(), canonical_head.as_ref()) {
        let validated_head =
            validate_moderation_panel_notification_archive_public_head_readback_at_broker_boundary(
                canonical_head,
                &state.network_id,
            )?;
        if &validated_head != head {
            return Err(BrokerError::Rejected);
        }
    }
    requalify()?;
    encode_canonical(
        &ModerationPanelNotificationArchiveHeadReadResultWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot,
            canonical_head,
        },
        MAX_MODERATION_HANDOFF_FRAME_BYTES_V1,
    )
}

pub(super) fn moderation_panel_notification_deliver_once(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let wire = decode_canonical::<ModerationDurablePanelNotificationRequestWireV1>(
        &request.payload,
        MAX_MODERATION_PANEL_NOTIFICATION_FRAME_BYTES_V1,
    )?;
    let notification =
        validate_moderation_panel_notification_request(&wire, Some(&state.network_id))?;
    let receipt = broker_backend!(state, moderation_panel_notification)
        .deliver_once(&notification)
        .map_err(|error| {
            match error {
        sorafs_node::moderation_orchestrator::
            ModerationPanelNotificationFailureV1::NotDelivered => {
            BrokerError::Unavailable
        }
        sorafs_node::moderation_orchestrator::
            ModerationPanelNotificationFailureV1::Ambiguous => {
            BrokerError::Ambiguous
        }
        sorafs_node::moderation_orchestrator::
            ModerationPanelNotificationFailureV1::Permanent => {
            BrokerError::Rejected
        }
    }
        })?;
    let receipt = moderation_panel_notification_receipt_to_wire(receipt);
    validate_moderation_panel_notification_receipt(receipt, &wire)
        .map_err(|_| BrokerError::Ambiguous)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&receipt, MAX_MODERATION_PANEL_NOTIFICATION_FRAME_BYTES_V1)
        .map_err(|_| BrokerError::Ambiguous)
}

pub(super) fn moderation_checkpoint_load(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let store = broker_backend!(state, moderation_checkpoint_store);
    let record = store
        .load_latest()
        .map_err(moderation_checkpoint_backend_error)?;
    let record = record
        .map(|record| {
            let bytes = encode_canonical(
                &record,
                moderation_checkpoint_record_limit(&request.binding)?,
            )?;
            decode_moderation_checkpoint_record(&bytes, &request.binding, Some(&state.network_id))?;
            Ok(bytes)
        })
        .transpose()?;
    requalify()?;
    encode_canonical(&record, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
}

pub(super) fn moderation_checkpoint_compare_and_swap(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let compare = decode_canonical::<EvidenceViewerCheckpointCompareAndSwapRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
    )?;
    let next = decode_moderation_checkpoint_record(
        &compare.next_record,
        &request.binding,
        Some(&state.network_id),
    )?;
    let store = broker_backend!(state, moderation_checkpoint_store);
    let current = store
        .load_latest()
        .map_err(moderation_checkpoint_backend_error)?;
    if let Some(current) = current.as_ref() {
        let bytes = encode_canonical(
            current,
            moderation_checkpoint_record_limit(&request.binding)?,
        )?;
        decode_moderation_checkpoint_record(&bytes, &request.binding, Some(&state.network_id))
            .map_err(|_| BrokerError::Protocol)?;
    }
    validate_moderation_checkpoint_successor(current.as_ref(), compare.expected_revision, &next)?;
    store
        .compare_and_swap_latest(compare.expected_revision, &next)
        .map_err(|error| {
            match error {
        sorafs_node::moderation_orchestrator::
            ModerationCheckpointStoreExternalErrorV1::Rejected => {
            BrokerError::Conflict
        }
        sorafs_node::moderation_orchestrator::
            ModerationCheckpointStoreExternalErrorV1::Unavailable
        | sorafs_node::moderation_orchestrator::
            ModerationCheckpointStoreExternalErrorV1::Ambiguous => {
            BrokerError::Ambiguous
        }
    }
        })?;
    let readback = store.load_latest().map_err(|_| BrokerError::Ambiguous)?;
    if readback.as_ref() != Some(&next) {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(&(), MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
}

pub(super) fn moderation_panel_notification_source_attest(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let slot = request.binding.slot;
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let attest = decode_canonical::<ModerationPanelNotificationSourceAttestRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )?;
    validate_moderation_panel_notification_source_attest_wire_scope(
        attest.version,
        attest.slot,
        &attest.network_id,
        &state.network_id,
    )?;
    let store = broker_backend!(state, moderation_checkpoint_store);
    let current_record = store
        .load_latest()
        .map_err(moderation_checkpoint_backend_error)?
        .ok_or(BrokerError::Rejected)?;
    let statement_digest =
        validate_moderation_panel_notification_source_attestation_at_broker_boundary(
            &attest.statement,
            &state.network_id,
            &request.binding,
            &current_record,
        )?;
    let signature = store
        .attest_terminal_set(&attest.statement)
        .map_err(moderation_checkpoint_backend_error)?;
    attest
        .statement
        .verify(signature)
        .map_err(|_| BrokerError::Ambiguous)?;
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(
        &ModerationPanelNotificationSourceAttestResultWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot,
            statement_digest,
            signature,
        },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
}

pub(super) fn moderation_panel_notification_archive_install(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let slot = request.binding.slot;
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let install = decode_canonical::<ModerationPanelNotificationArchiveInstallRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
    )?;
    validate_moderation_panel_notification_archive_wire_scope(
        install.version,
        install.slot,
        &install.network_id,
        &state.network_id,
    )?;
    let validated = validate_moderation_panel_notification_archive_artifact_at_broker_boundary(
        &install.canonical_artifact,
        &state.network_id,
        &request.binding,
        &state.catalog,
    )?;
    if install.operation_id != validated.operation_id
        || install.receipt_message != validated.receipt_message
    {
        return Err(BrokerError::Rejected);
    }
    let archive = broker_backend!(state, moderation_panel_notification_archive);
    let signature = archive
        .install(
            validated.operation_id,
            validated.receipt_message,
            &install.canonical_artifact,
        )
        .map_err(moderation_panel_notification_archive_backend_error)?;
    let public_key = required_binding_value!(
        &request.binding,
        moderation_panel_notification_archive_binding
    );
    verify_evidence_viewer_ed25519_signature(
        public_key.public_key,
        signature,
        &validated.receipt_message,
    )?;
    let readback = archive
        .read(validated.operation_id)
        .map_err(|_| BrokerError::Ambiguous)?
        .ok_or(BrokerError::Ambiguous)?;
    if readback.canonical_artifact != install.canonical_artifact || readback.signature != signature
    {
        return Err(BrokerError::Ambiguous);
    }
    requalify().map_err(|_| BrokerError::Ambiguous)?;
    encode_canonical(
        &ModerationPanelNotificationArchiveInstallResultWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot,
            signature,
        },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
}

pub(super) fn moderation_panel_notification_archive_read(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let slot = request.binding.slot;
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let read = decode_canonical::<ModerationPanelNotificationArchiveReadRequestWireV1>(
        &request.payload,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )?;
    validate_moderation_panel_notification_archive_wire_scope(
        read.version,
        read.slot,
        &read.network_id,
        &state.network_id,
    )?;
    let max_bytes = usize::try_from(
        required_binding_value!(
            &request.binding,
            moderation_panel_notification_archive_binding
        )
        .max_bytes,
    )
    .map_err(|_| BrokerError::Rejected)?;
    let readback = broker_backend!(state, moderation_panel_notification_archive)
        .read(read.operation_id)
        .map_err(|error| {
            match error {
        sorafs_node::moderation_orchestrator::
            ModerationPanelNotificationArchiveExternalErrorV1::Unavailable
        | sorafs_node::moderation_orchestrator::
            ModerationPanelNotificationArchiveExternalErrorV1::Ambiguous => {
            BrokerError::Unavailable
        }
        sorafs_node::moderation_orchestrator::
            ModerationPanelNotificationArchiveExternalErrorV1::Rejected => {
            BrokerError::Rejected
        }
    }
        })?
        .map(|readback| {
            if readback.canonical_artifact.is_empty()
                || readback.canonical_artifact.len() > max_bytes
                || readback.signature == [0; 64]
            {
                return Err(BrokerError::Rejected);
            }
            let validated =
                validate_moderation_panel_notification_archive_readback_at_broker_boundary(
                    &readback.canonical_artifact,
                    &state.network_id,
                    &request.binding,
                    &state.catalog,
                )?;
            if validated.operation_id != read.operation_id {
                return Err(BrokerError::Rejected);
            }
            verify_evidence_viewer_ed25519_signature(
                validated.archive_public_key,
                readback.signature,
                &validated.receipt_message,
            )?;
            Ok(ModerationPanelNotificationArchiveReadbackWireV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                slot,
                canonical_artifact: readback.canonical_artifact,
                signature: readback.signature,
            })
        })
        .transpose()?;
    requalify()?;
    encode_canonical(&readback, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
}
