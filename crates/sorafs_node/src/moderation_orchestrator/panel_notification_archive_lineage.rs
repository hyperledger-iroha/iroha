fn panel_notification_archive_payload_digest(
    payload: &ModerationPanelNotificationArchivePayloadV1,
) -> Result<[u8; 32], ModerationOrchestratorError> {
    let bytes = norito::to_bytes(payload).map_err(|error| {
        ModerationOrchestratorError::CheckpointCorrupt(format!(
            "encode panel notification archive payload: {error}"
        ))
    })?;
    Ok(domain_hash(
        PANEL_NOTIFICATION_ARCHIVE_PAYLOAD_DOMAIN_V1,
        &[
            &u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes(),
            &bytes,
        ],
    ))
}

fn hash_optional_archive_digest(hasher: &mut blake3::Hasher, value: Option<[u8; 32]>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hasher.update(&value);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn panel_notification_archive_signer_epoch_digest(
    epoch: &ModerationPanelNotificationArchiveSignerEpochV1,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PANEL_NOTIFICATION_ARCHIVE_SIGNER_EPOCH_DOMAIN_V1);
    hasher.update(&epoch.version.to_le_bytes());
    hasher.update(&epoch.epoch.to_le_bytes());
    hasher.update(&epoch.activated_at_generation.to_le_bytes());
    hasher.update(&epoch.archive_id);
    hasher.update(
        &u64::try_from(epoch.archive_handle.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(epoch.archive_handle.as_bytes());
    hasher.update(&epoch.archive_revision.to_le_bytes());
    hasher.update(&epoch.archive_policy_digest);
    hasher.update(&epoch.archive_public_key);
    hash_optional_archive_digest(&mut hasher, epoch.predecessor_epoch_digest);
    match epoch.predecessor_revocation_generation {
        Some(generation) => {
            hasher.update(&[1]);
            hasher.update(&generation.to_le_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    for signature in [
        epoch.predecessor_authorization_signature,
        epoch.new_key_possession_signature,
    ] {
        match signature {
            Some(signature) => {
                hasher.update(&[1]);
                hasher.update(&signature);
            }
            None => {
                hasher.update(&[0]);
            }
        }
    }
    *hasher.finalize().as_bytes()
}

fn panel_notification_archive_signer_rotation_message(
    chain_id: &iroha_data_model::ChainId,
    epoch: &ModerationPanelNotificationArchiveSignerEpochV1,
) -> [u8; 32] {
    domain_hash(
        PANEL_NOTIFICATION_ARCHIVE_SIGNER_ROTATION_DOMAIN_V1,
        &[
            chain_id.as_str().as_bytes(),
            &epoch.epoch.to_le_bytes(),
            &epoch.activated_at_generation.to_le_bytes(),
            &epoch.archive_id,
            epoch.archive_handle.as_bytes(),
            &epoch.archive_revision.to_le_bytes(),
            &epoch.archive_policy_digest,
            &epoch.archive_public_key,
            &epoch.predecessor_epoch_digest.unwrap_or([0; 32]),
            &epoch
                .predecessor_revocation_generation
                .unwrap_or(0)
                .to_le_bytes(),
        ],
    )
}

fn panel_notification_archive_signer_pop_message(rotation_message: [u8; 32]) -> [u8; 32] {
    domain_hash(
        PANEL_NOTIFICATION_ARCHIVE_SIGNER_POP_DOMAIN_V1,
        &[&rotation_message],
    )
}

fn verify_archive_ed25519_signature(
    public_key: [u8; 32],
    signature: [u8; 64],
    message: [u8; 32],
) -> bool {
    let Ok(public_key) = PublicKey::from_bytes(Algorithm::Ed25519, &public_key) else {
        return false;
    };
    let Ok(signature) = IrohaSignature::try_from_bytes(&signature) else {
        return false;
    };
    signature.verify(&public_key, &message).is_ok()
}

fn validate_panel_notification_archive_signer_epochs(
    epochs: &[ModerationPanelNotificationArchiveSignerEpochV1],
    chain_id: &iroha_data_model::ChainId,
    expected_bootstrap_public_key: [u8; 32],
    expected_archive_id: [u8; 32],
) -> Result<(), ModerationOrchestratorError> {
    if epochs.is_empty()
        || epochs.len() > MODERATION_PANEL_NOTIFICATION_ARCHIVE_MAX_SIGNER_EPOCHS_V1
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    for (index, epoch) in epochs.iter().enumerate() {
        let qualification = ModerationRuntimeProviderQualificationV1::new(
            epoch.archive_revision,
            epoch.archive_policy_digest,
        );
        if epoch.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1
            || epoch.epoch != u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1)
            || epoch.activated_at_generation == 0
            || epoch.archive_id != expected_archive_id
            || validate_production_runtime_handle(&epoch.archive_handle).is_err()
            || !qualification.is_valid()
            || epoch.archive_public_key == [0; 32]
            || PublicKey::from_bytes(Algorithm::Ed25519, &epoch.archive_public_key).is_err()
            || epoch.epoch_digest == [0; 32]
            || epoch.epoch_digest != panel_notification_archive_signer_epoch_digest(epoch)
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        let Some(predecessor) = index.checked_sub(1).and_then(|value| epochs.get(value)) else {
            if epoch.archive_public_key != expected_bootstrap_public_key
                || epoch.activated_at_generation != 1
                || epoch.predecessor_epoch_digest.is_some()
                || epoch.predecessor_revocation_generation.is_some()
                || epoch.predecessor_authorization_signature.is_some()
                || epoch.new_key_possession_signature.is_some()
            {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
            continue;
        };
        let (
            Some(predecessor_epoch_digest),
            Some(predecessor_revocation_generation),
            Some(predecessor_authorization_signature),
            Some(new_key_possession_signature),
        ) = (
            epoch.predecessor_epoch_digest,
            epoch.predecessor_revocation_generation,
            epoch.predecessor_authorization_signature,
            epoch.new_key_possession_signature,
        )
        else {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        };
        let rotation_message = panel_notification_archive_signer_rotation_message(chain_id, epoch);
        if predecessor_epoch_digest != predecessor.epoch_digest
            || predecessor_revocation_generation.checked_add(1)
                != Some(epoch.activated_at_generation)
            || !verify_archive_ed25519_signature(
                predecessor.archive_public_key,
                predecessor_authorization_signature,
                rotation_message,
            )
            || !verify_archive_ed25519_signature(
                epoch.archive_public_key,
                new_key_possession_signature,
                panel_notification_archive_signer_pop_message(rotation_message),
            )
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
    }
    Ok(())
}

fn verify_panel_notification_archive_head_signer_epoch(
    head: &ModerationPanelNotificationArchiveHeadV1,
    epochs: &[ModerationPanelNotificationArchiveSignerEpochV1],
) -> Result<(), ModerationOrchestratorError> {
    let index = usize::try_from(head.archive_signer_epoch.saturating_sub(1))
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let epoch = epochs
        .get(index)
        .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let next_epoch = epochs.get(index.saturating_add(1));
    if head.archive_signer_epoch_digest != epoch.epoch_digest
        || head.generation < epoch.activated_at_generation
        || next_epoch.is_some_and(|next| {
            next.predecessor_revocation_generation
                .is_none_or(|cutoff| head.generation > cutoff)
        })
        || head.archive_id != epoch.archive_id
        || head.archive_handle != epoch.archive_handle
        || head.archive_revision != epoch.archive_revision
        || head.archive_policy_digest != epoch.archive_policy_digest
        || head.archive_public_key != epoch.archive_public_key
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    Ok(())
}

fn reconcile_panel_notification_archive_signer_epochs(
    config: &ModerationOrchestratorConfigV1,
    chain_id: &iroha_data_model::ChainId,
    state: &mut ModerationOrchestratorCheckpointV1,
) -> Result<bool, ModerationOrchestratorError> {
    let mut changed = false;
    if state.panel_notification_archive_signer_epochs.is_empty() {
        if state.panel_notification_archive_head.is_some()
            || state
                .panel_notification_archive_pending_publication
                .is_some()
            || state.panel_notification_archive_published_head.is_some()
            || state.panel_notification_archived_dead_letter_count != 0
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        let mut bootstrap = ModerationPanelNotificationArchiveSignerEpochV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
            epoch: 1,
            activated_at_generation: 1,
            archive_id: config.panel_notification_archive_id,
            archive_handle: config.panel_notification_archive_handle.clone(),
            archive_revision: config
                .expected_panel_notification_archive_qualification
                .revision(),
            archive_policy_digest: config
                .expected_panel_notification_archive_qualification
                .policy_digest(),
            archive_public_key: config.panel_notification_archive_bootstrap_public_key,
            predecessor_epoch_digest: None,
            predecessor_revocation_generation: None,
            predecessor_authorization_signature: None,
            new_key_possession_signature: None,
            epoch_digest: [0; 32],
        };
        bootstrap.epoch_digest = panel_notification_archive_signer_epoch_digest(&bootstrap);
        state
            .panel_notification_archive_signer_epochs
            .push(bootstrap);
        changed = true;
    }
    validate_panel_notification_archive_signer_epochs(
        &state.panel_notification_archive_signer_epochs,
        chain_id,
        config.panel_notification_archive_bootstrap_public_key,
        config.panel_notification_archive_id,
    )?;
    let latest = state
        .panel_notification_archive_signer_epochs
        .last()
        .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let current_matches = latest.archive_handle == config.panel_notification_archive_handle
        && latest.archive_revision
            == config
                .expected_panel_notification_archive_qualification
                .revision()
        && latest.archive_policy_digest
            == config
                .expected_panel_notification_archive_qualification
                .policy_digest()
        && latest.archive_id == config.panel_notification_archive_id
        && latest.archive_public_key == config.panel_notification_archive_public_key;
    if !current_matches {
        if state
            .panel_notification_archive_pending_publication
            .is_some()
            || state
                .panel_notification_archive_compaction_reservation
                .is_some()
        {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "archive signer rotation requires no pending compaction and a durably published predecessor head"
                    .to_owned(),
            ));
        }
        if state.panel_notification_archive_signer_epochs.len()
            >= MODERATION_PANEL_NOTIFICATION_ARCHIVE_MAX_SIGNER_EPOCHS_V1
        {
            return Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "panel notification archive signer epochs",
                limit: MODERATION_PANEL_NOTIFICATION_ARCHIVE_MAX_SIGNER_EPOCHS_V1,
            });
        }
        let cutoff = state
            .panel_notification_archive_head
            .as_ref()
            .map_or(0, |head| head.generation);
        let (
            Some(configured_cutoff),
            Some(predecessor_authorization_signature),
            Some(new_key_possession_signature),
        ) = (
            config.panel_notification_archive_predecessor_revocation_generation,
            config.panel_notification_archive_predecessor_authorization_signature,
            config.panel_notification_archive_new_key_possession_signature,
        )
        else {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "archive signer transition is missing dual-control evidence".to_owned(),
            ));
        };
        if configured_cutoff != cutoff {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "archive signer predecessor cutoff does not equal the sealed archive head"
                    .to_owned(),
            ));
        }
        let latest = latest.clone();
        let mut next = ModerationPanelNotificationArchiveSignerEpochV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
            epoch: latest
                .epoch
                .checked_add(1)
                .ok_or(ModerationOrchestratorError::GenerationOverflow)?,
            activated_at_generation: cutoff
                .checked_add(1)
                .ok_or(ModerationOrchestratorError::GenerationOverflow)?,
            archive_id: config.panel_notification_archive_id,
            archive_handle: config.panel_notification_archive_handle.clone(),
            archive_revision: config
                .expected_panel_notification_archive_qualification
                .revision(),
            archive_policy_digest: config
                .expected_panel_notification_archive_qualification
                .policy_digest(),
            archive_public_key: config.panel_notification_archive_public_key,
            predecessor_epoch_digest: Some(latest.epoch_digest),
            predecessor_revocation_generation: Some(cutoff),
            predecessor_authorization_signature: Some(predecessor_authorization_signature),
            new_key_possession_signature: Some(new_key_possession_signature),
            epoch_digest: [0; 32],
        };
        next.epoch_digest = panel_notification_archive_signer_epoch_digest(&next);
        let mut candidate = state.panel_notification_archive_signer_epochs.clone();
        candidate.push(next.clone());
        validate_panel_notification_archive_signer_epochs(
            &candidate,
            chain_id,
            config.panel_notification_archive_bootstrap_public_key,
            config.panel_notification_archive_id,
        )?;
        state.panel_notification_archive_signer_epochs.push(next);
        changed = true;
    }
    let latest = state
        .panel_notification_archive_signer_epochs
        .last()
        .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    if latest.archive_handle != config.panel_notification_archive_handle
        || latest.archive_revision
            != config
                .expected_panel_notification_archive_qualification
                .revision()
        || latest.archive_policy_digest
            != config
                .expected_panel_notification_archive_qualification
                .policy_digest()
        || latest.archive_public_key != config.panel_notification_archive_public_key
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    Ok(changed)
}

fn hash_panel_notification_archive_head_fields(
    hasher: &mut blake3::Hasher,
    head: &ModerationPanelNotificationArchiveHeadV1,
) {
    hasher.update(&head.version.to_le_bytes());
    hasher.update(
        &u64::try_from(head.chain_id.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(head.chain_id.as_bytes());
    hasher.update(&head.generation.to_le_bytes());
    hash_optional_archive_digest(hasher, head.predecessor_head_digest);
    hash_optional_archive_digest(hasher, head.predecessor_operation_id);
    hash_optional_archive_digest(hasher, head.predecessor_chain_commitment);
    hasher.update(&head.source_checkpoint_generation.to_le_bytes());
    hasher.update(&head.source_checkpoint_namespace_digest);
    hasher.update(&head.source_checkpoint_revision);
    hasher.update(&head.source_checkpoint_digest);
    hasher.update(&head.source_manifest_digest);
    hasher.update(&head.source_binding_digest);
    hasher.update(
        &u64::try_from(head.source_attestor_handle.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(head.source_attestor_handle.as_bytes());
    hasher.update(&head.source_attestor_revision.to_le_bytes());
    hasher.update(&head.source_attestor_policy_digest);
    hasher.update(&head.source_attestor_public_key);
    hasher.update(&head.source_attestation_digest);
    hasher.update(&head.source_attestation_signature);
    hasher.update(&head.terminal_record_count.to_le_bytes());
    hasher.update(&head.dead_letter_record_count.to_le_bytes());
    hasher.update(&head.cumulative_dead_letter_count.to_le_bytes());
    hasher.update(&head.first_notification_id);
    hasher.update(&head.last_notification_id);
    hasher.update(&head.payload_digest);
    hasher.update(
        &u64::try_from(head.archive_handle.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(head.archive_handle.as_bytes());
    hasher.update(&head.archive_revision.to_le_bytes());
    hasher.update(&head.archive_policy_digest);
    hasher.update(&head.archive_id);
    hasher.update(&head.archive_public_key);
    hasher.update(&head.archive_signer_epoch.to_le_bytes());
    hasher.update(&head.archive_signer_epoch_digest);
}

fn panel_notification_archive_source_binding_digest(
    statement: &ModerationPanelNotificationSourceAttestationV1,
) -> [u8; 32] {
    domain_hash(
        PANEL_NOTIFICATION_ARCHIVE_SOURCE_DOMAIN_V1,
        &[&panel_notification_source_attestation_message(statement)],
    )
}

fn panel_notification_archive_source_manifest_digest(
    manifest: &ModerationPanelNotificationArchiveSourceManifestV1,
) -> Result<[u8; 32], ModerationOrchestratorError> {
    let bytes = norito::to_bytes(manifest).map_err(|error| {
        ModerationOrchestratorError::CheckpointCorrupt(format!(
            "encode panel notification archive source manifest: {error}"
        ))
    })?;
    Ok(domain_hash(
        PANEL_NOTIFICATION_ARCHIVE_SOURCE_MANIFEST_DOMAIN_V1,
        &[
            &u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes(),
            &bytes,
        ],
    ))
}

fn panel_notification_source_attestation_message(
    statement: &ModerationPanelNotificationSourceAttestationV1,
) -> [u8; 32] {
    domain_hash(
        PANEL_NOTIFICATION_ARCHIVE_SOURCE_ATTESTATION_DOMAIN_V1,
        &[
            &statement.version.to_le_bytes(),
            &statement.attestor_slot.to_le_bytes(),
            &u64::try_from(statement.chain_id.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
            statement.chain_id.as_bytes(),
            &statement.checkpoint_namespace_digest,
            &statement.checkpoint_generation.to_le_bytes(),
            &statement.checkpoint_revision,
            &statement.checkpoint_digest,
            &statement.source_manifest_digest,
            &statement.terminal_set_digest,
            &statement.terminal_record_count.to_le_bytes(),
            &statement.first_notification_id,
            &statement.last_notification_id,
            &u64::try_from(statement.attestor_handle.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
            statement.attestor_handle.as_bytes(),
            &statement.attestor_revision.to_le_bytes(),
            &statement.attestor_policy_digest,
            &statement.attestor_public_key,
        ],
    )
}

fn validate_panel_notification_source_attestation(
    statement: &ModerationPanelNotificationSourceAttestationV1,
) -> Result<(), ModerationOrchestratorError> {
    let qualification = ModerationRuntimeProviderQualificationV1::new(
        statement.attestor_revision,
        statement.attestor_policy_digest,
    );
    if statement.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1
        || statement.attestor_slot != MODERATION_PANEL_NOTIFICATION_SOURCE_ATTESTOR_BROKER_SLOT_V1
        || statement.chain_id.is_empty()
        || statement.checkpoint_namespace_digest == [0; 32]
        || statement.checkpoint_generation == 0
        || statement.checkpoint_revision == [0; 32]
        || statement.checkpoint_digest == [0; 32]
        || statement.source_manifest_digest == [0; 32]
        || statement.terminal_set_digest == [0; 32]
        || statement.terminal_record_count == 0
        || statement.first_notification_id == [0; 32]
        || statement.last_notification_id == [0; 32]
        || validate_production_runtime_handle(&statement.attestor_handle).is_err()
        || !qualification.is_valid()
        || statement.attestor_public_key == [0; 32]
        || PublicKey::from_bytes(Algorithm::Ed25519, &statement.attestor_public_key).is_err()
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    Ok(())
}

/// Validate one checkpoint-attestor broker request without accepting a caller-supplied message.
///
/// The exact terminal payload must equal the pre-CAS reservation sealed into
/// `current_record`. The reservation is revalidated as the complete canonical
/// eligible prefix before signing, so a caller cannot obtain signatures for
/// shorter, longer, or substituted batches at the same checkpoint generation.
///
/// # Errors
///
/// Rejects a stale/substituted record, missing or noncanonical reservation,
/// provider mismatch, or any statement field not derivable from the current sealed record.
pub fn validate_moderation_panel_notification_source_attestation_for_broker_v1(
    statement: &ModerationPanelNotificationSourceAttestationV1,
    expected_chain_id: &iroha_data_model::ChainId,
    expected_handle: &str,
    expected_qualification: ModerationRuntimeProviderQualificationV1,
    expected_public_key: [u8; 32],
    current_record: &ModerationCheckpointStoreRecordV1,
) -> Result<[u8; 32], ModerationOrchestratorError> {
    validate_panel_notification_source_attestation(statement)?;
    if statement.chain_id != expected_chain_id.as_str()
        || statement.attestor_handle != expected_handle
        || statement.attestor_revision != expected_qualification.revision()
        || statement.attestor_policy_digest != expected_qualification.policy_digest()
        || statement.attestor_public_key != expected_public_key
        || !current_record.has_valid_provider_envelope(
            expected_handle,
            expected_qualification,
            MODERATION_ORCHESTRATOR_CHECKPOINT_MAX_BYTES_V1,
        )
        || current_record.namespace_digest
            != checkpoint_store::checkpoint_namespace(expected_chain_id)
        || statement.checkpoint_namespace_digest != current_record.namespace_digest
        || statement.checkpoint_generation != current_record.checkpoint_generation
        || statement.checkpoint_revision != current_record.revision
        || statement.checkpoint_digest != current_record.checkpoint_digest
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    let limits = checkpoint_decode_limits(MODERATION_ORCHESTRATOR_CHECKPOINT_MAX_BYTES_V1)?;
    let source = decode_from_bytes_with_limits::<ModerationOrchestratorCheckpointV1>(
        &current_record.checkpoint_bytes,
        limits,
    )
    .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    if norito::to_bytes(&source)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?
        != current_record.checkpoint_bytes
        || source.version != MODERATION_ORCHESTRATOR_CHECKPOINT_VERSION_V1
        || source.chain_id != expected_chain_id.as_str()
        || source.generation != current_record.checkpoint_generation
        || source.panel_notification_outbox_digest != panel_notification_outbox_digest(&source)
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    let source_manifest = ModerationPanelNotificationArchiveSourceManifestV1 {
        version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
        chain_id: expected_chain_id.as_str().to_owned(),
        checkpoint_namespace_digest: current_record.namespace_digest,
        checkpoint_generation: current_record.checkpoint_generation,
        checkpoint_revision: current_record.revision,
        checkpoint_digest: current_record.checkpoint_digest,
        archive_signer_epochs: source.panel_notification_archive_signer_epochs.clone(),
        predecessor_archive_head: source.panel_notification_archive_head.clone(),
    };
    if statement.source_manifest_digest
        != panel_notification_archive_source_manifest_digest(&source_manifest)?
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    let requested_count = usize::try_from(statement.terminal_record_count)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let payload = source
        .panel_notification_archive_compaction_reservation
        .as_ref()
        .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    if payload.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1
        || payload.records.is_empty()
        || payload.records.len() > MODERATION_PANEL_NOTIFICATION_ARCHIVE_MAX_RECORDS_V1
        || requested_count != payload.records.len()
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    let terminal_records = collect_terminal_archive_records(&source)?;
    if terminal_records.len() < payload.records.len()
        || terminal_records[..payload.records.len()] != payload.records
        || safe_terminal_archive_prefix_len(&terminal_records, payload.records.len())?
            != payload.records.len()
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    if payload
        .records
        .first()
        .map(terminal_archive_record_boundary_id)
        .transpose()?
        != Some(statement.first_notification_id)
        || payload
            .records
            .last()
            .map(terminal_archive_record_boundary_id)
            .transpose()?
            != Some(statement.last_notification_id)
        || panel_notification_archive_payload_digest(payload)? != statement.terminal_set_digest
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    Ok(panel_notification_source_attestation_message(statement))
}

fn panel_notification_source_attestation_from_head(
    head: &ModerationPanelNotificationArchiveHeadV1,
) -> ModerationPanelNotificationSourceAttestationV1 {
    ModerationPanelNotificationSourceAttestationV1 {
        version: head.version,
        attestor_slot: MODERATION_PANEL_NOTIFICATION_SOURCE_ATTESTOR_BROKER_SLOT_V1,
        chain_id: head.chain_id.clone(),
        checkpoint_namespace_digest: head.source_checkpoint_namespace_digest,
        checkpoint_generation: head.source_checkpoint_generation,
        checkpoint_revision: head.source_checkpoint_revision,
        checkpoint_digest: head.source_checkpoint_digest,
        source_manifest_digest: head.source_manifest_digest,
        terminal_set_digest: head.payload_digest,
        terminal_record_count: head.terminal_record_count,
        first_notification_id: head.first_notification_id,
        last_notification_id: head.last_notification_id,
        attestor_handle: head.source_attestor_handle.clone(),
        attestor_revision: head.source_attestor_revision,
        attestor_policy_digest: head.source_attestor_policy_digest,
        attestor_public_key: head.source_attestor_public_key,
    }
}

fn panel_notification_archive_operation_id(
    head: &ModerationPanelNotificationArchiveHeadV1,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PANEL_NOTIFICATION_ARCHIVE_OPERATION_DOMAIN_V1);
    hash_panel_notification_archive_head_fields(&mut hasher, head);
    *hasher.finalize().as_bytes()
}

fn panel_notification_archive_head_digest(
    head: &ModerationPanelNotificationArchiveHeadV1,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PANEL_NOTIFICATION_ARCHIVE_HEAD_DOMAIN_V1);
    hash_panel_notification_archive_head_fields(&mut hasher, head);
    hasher.update(&head.operation_id);
    *hasher.finalize().as_bytes()
}

fn panel_notification_archive_chain_commitment(
    head: &ModerationPanelNotificationArchiveHeadV1,
) -> [u8; 32] {
    let predecessor = head.predecessor_chain_commitment.unwrap_or([0; 32]);
    domain_hash(
        PANEL_NOTIFICATION_ARCHIVE_AUDIT_DOMAIN_V1,
        &[
            &head.generation.to_le_bytes(),
            &predecessor,
            &head.operation_id,
            &head.head_digest,
        ],
    )
}

fn panel_notification_archive_audit_page_commitment(
    previous: [u8; 32],
    head: &ModerationPanelNotificationArchiveHeadV1,
) -> [u8; 32] {
    domain_hash(
        PANEL_NOTIFICATION_ARCHIVE_AUDIT_DOMAIN_V1,
        &[
            b"page",
            &previous,
            &head.generation.to_le_bytes(),
            &head.operation_id,
            &head.head_digest,
            &head.chain_commitment,
        ],
    )
}

fn panel_notification_archive_receipt_message(
    head: &ModerationPanelNotificationArchiveHeadV1,
) -> [u8; 32] {
    domain_hash(
        PANEL_NOTIFICATION_ARCHIVE_RECEIPT_DOMAIN_V1,
        &[
            &MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_SLOT_V1.to_le_bytes(),
            head.chain_id.as_bytes(),
            &head.archive_id,
            &head.archive_public_key,
            &head.operation_id,
            &head.head_digest,
            &head.chain_commitment,
        ],
    )
}

fn verify_panel_notification_archive_head_core(
    head: &ModerationPanelNotificationArchiveHeadV1,
) -> Result<(), ModerationOrchestratorError> {
    let head_qualification = ModerationRuntimeProviderQualificationV1::new(
        head.archive_revision,
        head.archive_policy_digest,
    );
    let source_attestation = panel_notification_source_attestation_from_head(head);
    let source_attestation_message =
        panel_notification_source_attestation_message(&source_attestation);
    let lineage_valid = match head.generation {
        1 => {
            head.predecessor_head_digest.is_none()
                && head.predecessor_operation_id.is_none()
                && head.predecessor_chain_commitment.is_none()
        }
        2.. => {
            head.predecessor_head_digest
                .is_some_and(|digest| digest != [0; 32])
                && head
                    .predecessor_operation_id
                    .is_some_and(|operation_id| operation_id != [0; 32])
                && head
                    .predecessor_chain_commitment
                    .is_some_and(|commitment| commitment != [0; 32])
        }
        0 => false,
    };
    if head.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1
        || head.chain_id.is_empty()
        || !lineage_valid
        || head.source_checkpoint_generation == 0
        || head.source_checkpoint_namespace_digest == [0; 32]
        || head.source_checkpoint_revision == [0; 32]
        || head.source_checkpoint_digest == [0; 32]
        || head.source_manifest_digest == [0; 32]
        || head.source_binding_digest == [0; 32]
        || head.source_binding_digest
            != panel_notification_archive_source_binding_digest(&source_attestation)
        || head.source_attestation_digest != source_attestation_message
        || source_attestation
            .verify(head.source_attestation_signature)
            .is_err()
        || head.terminal_record_count == 0
        || head.dead_letter_record_count > head.terminal_record_count
        || head.cumulative_dead_letter_count < u64::from(head.dead_letter_record_count)
        || (head.generation == 1
            && head.cumulative_dead_letter_count != u64::from(head.dead_letter_record_count))
        || head.first_notification_id == [0; 32]
        || head.last_notification_id == [0; 32]
        || head.payload_digest == [0; 32]
        || validate_production_runtime_handle(&head.archive_handle).is_err()
        || !head_qualification.is_valid()
        || head.archive_id == [0; 32]
        || head.archive_public_key == [0; 32]
        || head.archive_signer_epoch == 0
        || head.archive_signer_epoch_digest == [0; 32]
        || head.operation_id == [0; 32]
        || head.head_digest == [0; 32]
        || head.chain_commitment == [0; 32]
        || head.predecessor_head_digest == Some(head.head_digest)
        || head.predecessor_operation_id == Some(head.operation_id)
        || head.operation_id != panel_notification_archive_operation_id(head)
        || head.head_digest != panel_notification_archive_head_digest(head)
        || head.chain_commitment != panel_notification_archive_chain_commitment(head)
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    PublicKey::from_bytes(Algorithm::Ed25519, &head.archive_public_key)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    Ok(())
}

fn verify_panel_notification_archive_head(
    head: &ModerationPanelNotificationArchiveHeadV1,
) -> Result<(), ModerationOrchestratorError> {
    verify_panel_notification_archive_head_core(head)?;
    if head.archive_signature == [0; 64] {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    let key = PublicKey::from_bytes(Algorithm::Ed25519, &head.archive_public_key)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let signature = IrohaSignature::try_from_bytes(&head.archive_signature)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    signature
        .verify(&key, &panel_notification_archive_receipt_message(head))
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)
}

fn verify_panel_notification_archive_head_is_current(
    head: &ModerationPanelNotificationArchiveHeadV1,
    expected_handle: &str,
    expected_qualification: ModerationRuntimeProviderQualificationV1,
    expected_archive_id: [u8; 32],
    expected_public_key: [u8; 32],
) -> Result<(), ModerationOrchestratorError> {
    verify_panel_notification_archive_head(head)?;
    verify_panel_notification_archive_head_core_is_current(
        head,
        expected_handle,
        expected_qualification,
        expected_archive_id,
        expected_public_key,
    )
}

fn verify_panel_notification_archive_head_core_is_current(
    head: &ModerationPanelNotificationArchiveHeadV1,
    expected_handle: &str,
    expected_qualification: ModerationRuntimeProviderQualificationV1,
    expected_archive_id: [u8; 32],
    expected_public_key: [u8; 32],
) -> Result<(), ModerationOrchestratorError> {
    verify_panel_notification_archive_head_core(head)?;
    if head.archive_handle != expected_handle
        || head.archive_revision != expected_qualification.revision()
        || head.archive_policy_digest != expected_qualification.policy_digest()
        || head.archive_id != expected_archive_id
        || head.archive_public_key != expected_public_key
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    Ok(())
}

fn verify_panel_notification_archive_lineage_link(
    successor: &ModerationPanelNotificationArchiveHeadV1,
    predecessor: &ModerationPanelNotificationArchiveHeadV1,
) -> Result<(), ModerationOrchestratorError> {
    if predecessor.generation.checked_add(1) != Some(successor.generation)
        || successor.predecessor_head_digest != Some(predecessor.head_digest)
        || successor.predecessor_operation_id != Some(predecessor.operation_id)
        || successor.predecessor_chain_commitment != Some(predecessor.chain_commitment)
        || successor.source_checkpoint_generation <= predecessor.source_checkpoint_generation
        || successor.chain_id != predecessor.chain_id
        || predecessor
            .cumulative_dead_letter_count
            .checked_add(u64::from(successor.dead_letter_record_count))
            != Some(successor.cumulative_dead_letter_count)
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    Ok(())
}

fn panel_notification_archive_record_from_stored(
    entry: &StoredPanelNotificationV1,
) -> Result<ModerationPanelNotificationArchiveRecordV1, ModerationOrchestratorError> {
    let terminal_status = match entry.state {
        StoredPanelNotificationStateV1::Delivered => {
            let (Some(receipt_digest), Some(delivered_at_unix_ms)) =
                (entry.receipt_digest, entry.delivered_at_unix_ms)
            else {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            };
            if entry.dead_letter_reason.is_some() || entry.dead_lettered_at_unix_ms.is_some() {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
            ModerationPanelNotificationArchiveTerminalStatusV1::Delivered {
                receipt_digest,
                delivered_at_unix_ms,
            }
        }
        StoredPanelNotificationStateV1::DeadLetter => {
            let (Some(reason), Some(dead_lettered_at_unix_ms)) =
                (entry.dead_letter_reason, entry.dead_lettered_at_unix_ms)
            else {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            };
            if entry.claimed_by.is_some()
                || entry.lease_token.is_some()
                || entry.claimed_at_unix_ms.is_some()
                || entry.lease_expires_at_unix_ms.is_some()
                || entry.receipt_digest.is_some()
                || entry.delivered_at_unix_ms.is_some()
            {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
            ModerationPanelNotificationArchiveTerminalStatusV1::DeadLettered {
                reason,
                dead_lettered_at_unix_ms,
            }
        }
        StoredPanelNotificationStateV1::Pending | StoredPanelNotificationStateV1::Claimed => {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
    };
    Ok(ModerationPanelNotificationArchiveRecordV1 {
        notification_id: entry.notification.notification_id,
        terminal_status,
        source_record_digest: entry.record_digest,
    })
}

fn archived_panel_notification_record_matches_source(
    record: &ModerationPanelNotificationArchiveRecordV1,
) -> impl FnOnce(&StoredPanelNotificationV1) -> bool + '_ {
    move |source| {
        if source.notification.notification_id != record.notification_id
            || source.record_digest != record.source_record_digest
            || source.record_digest != panel_notification_record_digest(source)
        {
            return false;
        }
        match record.terminal_status {
            ModerationPanelNotificationArchiveTerminalStatusV1::Delivered {
                receipt_digest,
                delivered_at_unix_ms,
            } => {
                source.state == StoredPanelNotificationStateV1::Delivered
                    && source.receipt_digest == Some(receipt_digest)
                    && source.delivered_at_unix_ms == Some(delivered_at_unix_ms)
                    && source.dead_letter_reason.is_none()
                    && source.dead_lettered_at_unix_ms.is_none()
            }
            ModerationPanelNotificationArchiveTerminalStatusV1::DeadLettered {
                reason,
                dead_lettered_at_unix_ms,
            } => {
                source.state == StoredPanelNotificationStateV1::DeadLetter
                    && source.dead_letter_reason == Some(reason)
                    && source.dead_lettered_at_unix_ms == Some(dead_lettered_at_unix_ms)
                    && source.receipt_digest.is_none()
                    && source.delivered_at_unix_ms.is_none()
            }
        }
    }
}

fn validate_archived_panel_notification_record_shape(
    record: &ModerationPanelNotificationArchiveRecordV1,
) -> Result<(), ModerationOrchestratorError> {
    if record.notification_id == [0; 32] || record.source_record_digest == [0; 32] {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    match record.terminal_status {
        ModerationPanelNotificationArchiveTerminalStatusV1::Delivered {
            receipt_digest,
            delivered_at_unix_ms,
        } if receipt_digest != [0; 32] && delivered_at_unix_ms != 0 => Ok(()),
        ModerationPanelNotificationArchiveTerminalStatusV1::DeadLettered {
            dead_lettered_at_unix_ms,
            ..
        } if dead_lettered_at_unix_ms != 0 => Ok(()),
        ModerationPanelNotificationArchiveTerminalStatusV1::Delivered { .. }
        | ModerationPanelNotificationArchiveTerminalStatusV1::DeadLettered { .. } => {
            Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
        }
    }
}

fn validate_archived_panel_notification_record(
    record: &ModerationPanelNotificationArchiveRecordV1,
    source: &StoredPanelNotificationV1,
) -> Result<(), ModerationOrchestratorError> {
    validate_archived_panel_notification_record_shape(record)?;
    if !archived_panel_notification_record_matches_source(record)(source) {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    match record.terminal_status {
        ModerationPanelNotificationArchiveTerminalStatusV1::Delivered {
            receipt_digest,
            delivered_at_unix_ms,
        } => {
            if receipt_digest == [0; 32]
                || delivered_at_unix_ms < source.notification.source_occurred_at_unix_ms
            {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
        }
        ModerationPanelNotificationArchiveTerminalStatusV1::DeadLettered {
            reason,
            dead_lettered_at_unix_ms,
        } => {
            if dead_lettered_at_unix_ms < source.notification.source_occurred_at_unix_ms
                || dead_lettered_at_unix_ms < source.available_at_unix_ms
                || (reason == ModerationPanelNotificationDeadLetterReasonV1::RetryExhausted
                    && source.attempts != source.attempt_limit)
            {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
        }
    }
    Ok(())
}

fn terminal_handoff_outcome_group_identity(
    cursor: ModerationFinalizedEventCursorV1,
    outcome_digest: [u8; 32],
) -> [u8; 32] {
    domain_hash(
        TERMINAL_ARCHIVE_RECORD_KEY_DOMAIN_V1,
        &[
            b"handoff-outcome",
            &cursor.sequence.to_le_bytes(),
            &cursor.block_height.to_le_bytes(),
            &cursor.block_hash,
            &cursor.event_index.to_le_bytes(),
            &outcome_digest,
        ],
    )
}

fn terminal_archive_record_key(
    record: &ModerationTerminalArchiveRecordV1,
) -> Result<(u8, [u8; 32], [u8; 32]), ModerationOrchestratorError> {
    match record {
        ModerationTerminalArchiveRecordV1::PanelNotification(record) => {
            Ok((0, record.notification_id, record.source_record_digest))
        }
        ModerationTerminalArchiveRecordV1::ResolvedPanelDeadLetter {
            terminal_record,
            source_record_digest,
            ..
        } => Ok((1, terminal_record.notification_id, *source_record_digest)),
        ModerationTerminalArchiveRecordV1::NativeOperation {
            operation_id,
            source_record_digest,
            ..
        } => Ok((2, *operation_id, *source_record_digest)),
        ModerationTerminalArchiveRecordV1::DurableDeadLetter {
            identity,
            resolution,
            handoff_outcome_digest,
            handoff_finalized_cursor,
            source_record_digest,
            ..
        } => {
            if resolution.kind == ModerationDeadLetterKindV1::TerminalHandoff
                && resolution.action == ModerationDeadLetterResolutionActionV1::Acknowledge
            {
                let outcome_digest = handoff_outcome_digest
                    .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
                let cursor = handoff_finalized_cursor
                    .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
                Ok((
                    4,
                    terminal_handoff_outcome_group_identity(cursor, outcome_digest),
                    *source_record_digest,
                ))
            } else {
                Ok((3, *identity, *source_record_digest))
            }
        }
        ModerationTerminalArchiveRecordV1::CompletedHandoff {
            finalized_cursor,
            outcome_digest,
            source_record_digest,
            ..
        } => Ok((
            4,
            terminal_handoff_outcome_group_identity(*finalized_cursor, *outcome_digest),
            *source_record_digest,
        )),
    }
}

fn terminal_archive_record_boundary_id(
    record: &ModerationTerminalArchiveRecordV1,
) -> Result<[u8; 32], ModerationOrchestratorError> {
    let (tag, identity, source_digest) = terminal_archive_record_key(record)?;
    Ok(domain_hash(
        TERMINAL_ARCHIVE_RECORD_KEY_DOMAIN_V1,
        &[&[tag], &identity, &source_digest],
    ))
}

fn terminal_archive_record_is_dead_letter(record: &ModerationTerminalArchiveRecordV1) -> bool {
    matches!(
        record,
        ModerationTerminalArchiveRecordV1::ResolvedPanelDeadLetter { .. }
            | ModerationTerminalArchiveRecordV1::DurableDeadLetter { .. }
    )
}

fn validate_terminal_archive_record_shape(
    record: &ModerationTerminalArchiveRecordV1,
) -> Result<(), ModerationOrchestratorError> {
    match record {
        ModerationTerminalArchiveRecordV1::PanelNotification(record) => {
            validate_archived_panel_notification_record_shape(record)?;
            if !matches!(
                record.terminal_status,
                ModerationPanelNotificationArchiveTerminalStatusV1::Delivered { .. }
            ) {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
        }
        ModerationTerminalArchiveRecordV1::ResolvedPanelDeadLetter {
            terminal_record,
            resolution,
            resolution_signature,
            source_record_digest,
        } => {
            validate_archived_panel_notification_record_shape(terminal_record)?;
            if !matches!(
                terminal_record.terminal_status,
                ModerationPanelNotificationArchiveTerminalStatusV1::DeadLettered { .. }
            ) || resolution.kind != ModerationDeadLetterKindV1::PanelNotification
                || resolution.identity != terminal_record.notification_id
                || resolution.source_record_digest != terminal_record.source_record_digest
                || *source_record_digest
                    != panel_notification_resolution_record_digest(
                        terminal_record,
                        resolution,
                        *resolution_signature,
                    )?
            {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
            verify_dead_letter_resolution_signature(resolution, *resolution_signature)?;
        }
        ModerationTerminalArchiveRecordV1::NativeOperation {
            operation_id,
            status,
            source_record_digest,
            ..
        } => {
            if *operation_id == [0; 32]
                || *status != StoredOperationStatusV1::Finalized
                || *source_record_digest == [0; 32]
            {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
        }
        ModerationTerminalArchiveRecordV1::DurableDeadLetter {
            incident_sequence,
            identity,
            reason,
            finalized_cursor,
            dead_lettered_at_unix_ms,
            resolution,
            resolution_signature,
            operation_source_record_digest,
            handoff_kind,
            handoff_outcome_digest,
            handoff_finalized_cursor,
            source_record_digest,
        } => {
            let expected_kind = match reason {
                StoredDeadLetterReasonV1::HandoffPermanentRejection
                | StoredDeadLetterReasonV1::HandoffRetryExhausted => {
                    ModerationDeadLetterKindV1::TerminalHandoff
                }
                StoredDeadLetterReasonV1::PermanentRejection
                | StoredDeadLetterReasonV1::FinalizedConflict
                | StoredDeadLetterReasonV1::RetryExhaustedNotFound => {
                    ModerationDeadLetterKindV1::NativeSubmission
                }
            };
            if *incident_sequence == 0
                || *identity == [0; 32]
                || finalized_cursor.height == 0
                || finalized_cursor.block_hash == [0; 32]
                || *dead_lettered_at_unix_ms == 0
                || resolution.authorized_at_unix_ms < *dead_lettered_at_unix_ms
                || resolution.identity != *identity
                || resolution.kind != expected_kind
                || resolution.source_record_digest == [0; 32]
                || resolution.source_record_digest != *source_record_digest
                || *source_record_digest == [0; 32]
                || (expected_kind == ModerationDeadLetterKindV1::TerminalHandoff
                    && operation_source_record_digest.is_some())
                || (expected_kind == ModerationDeadLetterKindV1::TerminalHandoff
                    && (handoff_kind.is_none()
                        || handoff_outcome_digest.is_none_or(|digest| digest == [0; 32])
                        || handoff_finalized_cursor.is_none()))
                || (expected_kind == ModerationDeadLetterKindV1::NativeSubmission
                    && (handoff_kind.is_some()
                        || handoff_outcome_digest.is_some()
                        || handoff_finalized_cursor.is_some()))
            {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
            verify_dead_letter_resolution_signature(resolution, *resolution_signature)?;
        }
        ModerationTerminalArchiveRecordV1::CompletedHandoff {
            handoff_id,
            outcome_digest,
            finalized_cursor,
            completed_at_finalized_cursor,
            source_record_digest,
            ..
        } => {
            if *handoff_id == [0; 32]
                || *outcome_digest == [0; 32]
                || finalized_cursor.sequence == 0
                || finalized_cursor.block_height == 0
                || finalized_cursor.block_hash == [0; 32]
                || completed_at_finalized_cursor.height < finalized_cursor.block_height
                || completed_at_finalized_cursor.block_hash == [0; 32]
                || *source_record_digest == [0; 32]
            {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
        }
    }
    Ok(())
}

fn collect_terminal_archive_records(
    state: &ModerationOrchestratorCheckpointV1,
) -> Result<Vec<ModerationTerminalArchiveRecordV1>, ModerationOrchestratorError> {
    let mut records = Vec::new();
    for entry in &state.panel_notifications {
        if entry.state == StoredPanelNotificationStateV1::Delivered {
            records.push(ModerationTerminalArchiveRecordV1::PanelNotification(
                panel_notification_archive_record_from_stored(entry)?,
            ));
        }
    }
    for entry in &state.panel_notification_dead_letter_resolutions {
        records.push(ModerationTerminalArchiveRecordV1::ResolvedPanelDeadLetter {
            terminal_record: entry.terminal_record.clone(),
            resolution: entry.resolution.clone(),
            resolution_signature: entry.resolution_signature,
            source_record_digest: entry.record_digest,
        });
    }
    for entry in &state.operations {
        if entry.status == StoredOperationStatusV1::Finalized {
            records.push(ModerationTerminalArchiveRecordV1::NativeOperation {
                operation_id: entry.operation_id,
                status: entry.status,
                transaction_id: entry.transaction_id,
                source_record_digest: native_operation_record_digest(entry)?,
            });
        }
    }
    let latest_native_incident = state
        .dead_letters
        .iter()
        .filter(|entry| {
            matches!(
                entry.redrive,
                Some(StoredDeadLetterRedriveV1::NativeSubmission { .. })
            )
        })
        .fold(BTreeMap::<[u8; 32], u64>::new(), |mut latest, entry| {
            latest
                .entry(entry.identity)
                .and_modify(|sequence| *sequence = (*sequence).max(entry.incident_sequence))
                .or_insert(entry.incident_sequence);
            latest
        });
    for entry in &state.dead_letters {
        let (Some(resolution), Some(resolution_signature)) =
            (entry.resolution.as_ref(), entry.resolution_signature)
        else {
            continue;
        };
        let operation_source_record_digest = if resolution.kind
            == ModerationDeadLetterKindV1::NativeSubmission
            && resolution.action == ModerationDeadLetterResolutionActionV1::Acknowledge
            && latest_native_incident.get(&entry.identity) == Some(&entry.incident_sequence)
        {
            state
                .operations
                .iter()
                .find(|operation| {
                    operation.operation_id == entry.identity
                        && operation.status == StoredOperationStatusV1::Rejected
                })
                .map(native_operation_record_digest)
                .transpose()?
        } else {
            None
        };
        let (handoff_kind, handoff_outcome_digest, handoff_finalized_cursor) = match entry
            .redrive
            .as_ref()
        {
            Some(StoredDeadLetterRedriveV1::TerminalHandoff(handoff)) => (
                Some(handoff.kind),
                Some(handoff.outcome_digest),
                Some(handoff.finalized_cursor),
            ),
            Some(StoredDeadLetterRedriveV1::NativeSubmission { .. }) | None => (None, None, None),
        };
        records.push(ModerationTerminalArchiveRecordV1::DurableDeadLetter {
            incident_sequence: entry.incident_sequence,
            identity: entry.identity,
            reason: entry.reason,
            finalized_cursor: entry.finalized_cursor,
            dead_lettered_at_unix_ms: entry.dead_lettered_at_unix_ms,
            resolution: resolution.clone(),
            resolution_signature,
            operation_source_record_digest,
            handoff_kind,
            handoff_outcome_digest,
            handoff_finalized_cursor,
            source_record_digest: durable_dead_letter_source_record_digest(entry)?,
        });
    }

    for completed in &state.completed_handoffs {
        records.push(ModerationTerminalArchiveRecordV1::CompletedHandoff {
            handoff_id: completed.handoff.handoff_id,
            kind: completed.handoff.kind,
            outcome_digest: completed.handoff.outcome_digest,
            finalized_cursor: completed.handoff.finalized_cursor,
            completed_at_finalized_cursor: completed.completed_at_finalized_cursor,
            source_record_digest: completed.record_digest,
        });
    }
    let mut terminal_groups = BTreeMap::<[u8; 32], (usize, BTreeSet<u8>)>::new();
    for record in &records {
        let key = terminal_archive_record_key(record)?;
        if key.0 != 4 {
            continue;
        }
        let kind = match record {
            ModerationTerminalArchiveRecordV1::CompletedHandoff { kind, .. } => *kind,
            ModerationTerminalArchiveRecordV1::DurableDeadLetter {
                handoff_kind: Some(kind),
                ..
            } => *kind,
            _ => return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid),
        };
        let entry = terminal_groups.entry(key.1).or_default();
        entry.0 = entry.0.saturating_add(1);
        entry.1.insert(match kind {
            ModerationTerminalHandoffKindV1::Settlement => 0,
            ModerationTerminalHandoffKindV1::Publication => 1,
        });
    }
    let mut terminal_outcome_order = BTreeMap::<(u64, u64, u32, [u8; 32]), [u8; 32]>::new();
    let mut observe_handoff = |handoff: &ModerationTerminalHandoffV1| {
        let group = terminal_handoff_outcome_group_identity(
            handoff.finalized_cursor,
            handoff.outcome_digest,
        );
        terminal_outcome_order.insert(
            (
                handoff.finalized_cursor.sequence,
                handoff.finalized_cursor.block_height,
                handoff.finalized_cursor.event_index,
                handoff.finalized_cursor.block_hash,
            ),
            group,
        );
    };
    for entry in &state.pending_handoffs {
        observe_handoff(&entry.handoff);
    }
    for entry in &state.completed_handoffs {
        observe_handoff(&entry.handoff);
    }
    for entry in &state.dead_letters {
        if let Some(StoredDeadLetterRedriveV1::TerminalHandoff(handoff)) = entry.redrive.as_ref() {
            observe_handoff(handoff);
        }
    }
    let mut allowed_terminal_groups = BTreeSet::new();
    for ((sequence, _, _, _), group) in terminal_outcome_order {
        if state
            .terminal_handoff_archived_cursor
            .is_some_and(|archived| sequence <= archived.sequence)
        {
            continue;
        }
        if terminal_groups
            .get(&group)
            .is_some_and(|(count, kinds)| *count == 2 && kinds.len() == 2)
        {
            allowed_terminal_groups.insert(group);
        } else {
            break;
        }
    }
    records.retain(|record| {
        let Ok((tag, identity, _)) = terminal_archive_record_key(record) else {
            return false;
        };
        tag != 4 || allowed_terminal_groups.contains(&identity)
    });
    records.sort_by_key(|record| {
        terminal_archive_record_key(record).unwrap_or((u8::MAX, [u8::MAX; 32], [u8::MAX; 32]))
    });
    let mut previous = None;
    for record in &records {
        let key = terminal_archive_record_key(record)?;
        if previous.as_ref().is_some_and(|prior| prior >= &key)
            || validate_terminal_archive_record_shape(record).is_err()
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        previous = Some(key);
    }
    Ok(records)
}

fn safe_terminal_archive_prefix_len(
    records: &[ModerationTerminalArchiveRecordV1],
    requested: usize,
) -> Result<usize, ModerationOrchestratorError> {
    let mut length = requested.min(records.len());
    if length == records.len() || length == 0 {
        return Ok(length);
    }
    let next = terminal_archive_record_key(&records[length])?;
    while length != 0 {
        let prior = terminal_archive_record_key(&records[length - 1])?;
        if prior.0 != 4 || next.0 != 4 || prior.1 != next.1 {
            break;
        }
        length -= 1;
    }
    Ok(length)
}
