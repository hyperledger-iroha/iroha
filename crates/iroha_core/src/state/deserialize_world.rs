struct MusubiPersistedState<'a> {
    namespace_bindings: &'a Storage<MusubiNamespaceV1, MusubiNamespaceBindingV1>,
    packages: &'a Storage<MusubiPackageIdV1, MusubiPackageRecordV1>,
    package_metadata: &'a Storage<MusubiPackageIdV1, MusubiPackageMetadataRecordV1>,
    package_members: &'a Storage<MusubiPackageMemberKeyV1, MusubiPackageMemberV1>,
    package_invitations: &'a Storage<MusubiInviteIdV1, MusubiMaintainerInvitationV1>,
    maintainer_directory:
        &'a Storage<MusubiMaintainerDirectoryKeyV1, MusubiMaintainerDirectoryEntryV1>,
    releases: &'a Storage<MusubiReleaseIdV1, MusubiReleaseRecordV1>,
    archives: &'a Storage<ArchiveId, MusubiArchiveRecordV1>,
    provider_bundle_attestations:
        &'a Storage<MusubiProviderBundleAttestationKeyV1, MusubiProviderBundleAttestationRecordV1>,
    archive_locations: &'a Storage<MusubiArchiveLocationKeyV1, MusubiArchiveLocationV1>,
    archive_availability: &'a Storage<ArchiveId, MusubiArchiveAvailabilityV1>,
    archive_reverse_references: &'a Storage<ArchiveId, MusubiArchiveReverseReferencesV1>,
    resolver_index: &'a Storage<MusubiReleaseIdV1, MusubiResolverReleaseRowV1>,
    resolver_index_checkpoints:
        &'a Storage<MusubiResolverIndexRevisionV1, MusubiRegistrySnapshotV1>,
    public_directory: &'a Storage<MusubiPackageSelectorV1, MusubiOrderedPackageEntryV1>,
    aliases: &'a Storage<MusubiAliasNameV1, MusubiAliasRecordV1>,
    alias_history: &'a Storage<MusubiAliasHistoryKeyV1, MusubiAliasHistoryEntryV1>,
    governance_decisions: &'a Storage<[u8; 32], MusubiGovernanceDecisionConsumptionV1>,
    resolver_index_revision: u64,
    replication_shortfall_releases: u64,
}

fn proposal_status_matches_latest_attempt_v1(
    proposal_status: GovernanceProposalStatus,
    attempt_status: Option<iroha_data_model::governance::types::GovernanceAttemptStatusV1>,
) -> bool {
    attempt_status.map_or(
        proposal_status == GovernanceProposalStatus::Proposed,
        |status| proposal_status == GovernanceProposalStatus::from_attempt_status(status),
    )
}

struct SoracloudInrouPersistedStateV1<'a> {
    service_revisions: &'a Storage<(String, String), SoraDeploymentBundleV1>,
    service_deployments: &'a Storage<Name, SoraServiceDeploymentStateV1>,
    app_infra_states: &'a Storage<Name, SoraAppInfraStateV1>,
    service_runtime: &'a Storage<Name, SoraServiceRuntimeStateV1>,
    inrou_replica_runtime: &'a Storage<(String, String, String), SoraInrouReplicaRuntimeStateV1>,
    service_audit_events: &'a Storage<u64, SoraServiceAuditEventV1>,
    app_infra_audit_events: &'a Storage<u64, SoraAppInfraAuditEventV1>,
    training_job_audit_events: &'a Storage<u64, SoraTrainingJobAuditEventV1>,
    model_weight_audit_events: &'a Storage<u64, SoraModelWeightAuditEventV1>,
    model_artifact_audit_events: &'a Storage<u64, SoraModelArtifactAuditEventV1>,
    hf_shared_lease_audit_events: &'a Storage<u64, SoraHfSharedLeaseAuditEventV1>,
    model_host_violation_evidence: &'a Storage<Hash, SoraModelHostViolationEvidenceRecordV1>,
    agent_apartment_audit_events: &'a Storage<u64, SoraAgentApartmentAuditEventV1>,
    service_state_entries: &'a Storage<(String, String, String), SoraServiceStateEntryV1>,
    inrou_host_capabilities: &'a Storage<AccountId, SoraInrouHostCapabilityRecordV1>,
    inrou_service_placements: &'a Storage<(String, String), SoraInrouServicePlacementRecordV1>,
    uploaded_model_bundles:
        &'a Storage<(String, String, String), SoraUploadedModelBundleV1>,
    mailbox_messages: &'a Storage<Hash, SoraServiceMailboxMessageV1>,
    runtime_receipts: &'a Storage<Hash, SoraRuntimeReceiptV1>,
    private_uploaded_model_execution_receipts:
        &'a Storage<Hash, SoraPrivateUploadedModelExecutionReceiptV1>,
}

impl SoracloudInrouPersistedStateV1<'_> {
    #[allow(
        clippy::too_many_lines,
        reason = "first-release restore validation keeps every authoritative Inrou-reachable store in one fail-closed boundary"
    )]
    fn validate(self) -> Result<(), json::Error> {
        let mut authoritative_sequences = std::collections::BTreeSet::new();
        let service_revisions = self.service_revisions.view();
        for (key, bundle) in service_revisions.iter() {
            bundle.validate_for_admission().map_err(|error| {
                invalid_soracloud_state("soracloud_service_revisions", error.to_string())
            })?;
            let expected_key = (
                bundle.service.service_name.as_ref().to_owned(),
                bundle.service.service_version.clone(),
            );
            if key != &expected_key {
                return Err(invalid_soracloud_state(
                    "soracloud_service_revisions",
                    "storage key must match the embedded service_name and service_version",
                ));
            }
        }

        let uploaded_model_bundles = self.uploaded_model_bundles.view();
        for (key, bundle) in uploaded_model_bundles.iter() {
            bundle.validate().map_err(|error| {
                invalid_soracloud_state(
                    "soracloud_uploaded_model_bundles",
                    error.to_string(),
                )
            })?;
            let expected_key = (
                bundle.service_name.as_ref().to_owned(),
                bundle.model_id.clone(),
                bundle.weight_version.clone(),
            );
            if key != &expected_key {
                return Err(invalid_soracloud_state(
                    "soracloud_uploaded_model_bundles",
                    "storage key must match embedded service_name, model_id, and weight_version",
                ));
            }
        }

        let service_deployments = self.service_deployments.view();
        for (key, deployment) in service_deployments.iter() {
            deployment.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_service_deployments", error.to_string())
            })?;
            if key != &deployment.service_name {
                return Err(invalid_soracloud_state(
                    "soracloud_service_deployments",
                    "storage key must match the embedded service_name",
                ));
            }
            let exact_revision_count = u32::try_from(
                service_revisions
                    .iter()
                    .filter(|((service_name, _), _)| {
                        service_name.as_str() == deployment.service_name.as_ref()
                    })
                    .count(),
            )
            .map_err(|error| {
                invalid_soracloud_state(
                    "soracloud_service_deployments",
                    format!(
                        "service `{}` admitted revision count does not fit u32: {error}",
                        deployment.service_name
                    ),
                )
            })?;
            if deployment.revision_count != exact_revision_count {
                return Err(invalid_soracloud_state(
                    "soracloud_service_deployments",
                    format!(
                        "service `{}` revision_count {} must equal its exact admitted revision count {exact_revision_count}",
                        deployment.service_name, deployment.revision_count
                    ),
                ));
            }
            let current_revision_key = (
                deployment.service_name.as_ref().to_owned(),
                deployment.current_service_version.clone(),
            );
            let current_bundle = service_revisions.get(&current_revision_key).ok_or_else(|| {
                invalid_soracloud_state(
                    "soracloud_service_deployments",
                    format!(
                        "service `{}` current revision `{}` is missing from soracloud_service_revisions",
                        deployment.service_name, deployment.current_service_version
                    ),
                )
            })?;
            if deployment.current_service_manifest_hash != current_bundle.service_manifest_hash()
                || deployment.current_container_manifest_hash
                    != current_bundle.container_manifest_hash()
            {
                return Err(invalid_soracloud_state(
                    "soracloud_service_deployments",
                    format!(
                        "service `{}` current revision `{}` manifest hashes must exactly match its admitted bundle",
                        deployment.service_name, deployment.current_service_version
                    ),
                ));
            }
            for (rollout_field, rollout) in [
                ("active_rollout", deployment.active_rollout.as_ref()),
                ("last_rollout", deployment.last_rollout.as_ref()),
            ] {
                let Some(rollout) = rollout else {
                    continue;
                };
                for (version_field, version) in [
                    (
                        "candidate_version",
                        Some(rollout.candidate_version.as_str()),
                    ),
                    ("baseline_version", Some(rollout.baseline_version.as_str())),
                ] {
                    let Some(version) = version else {
                        continue;
                    };
                    let revision_key = (
                        deployment.service_name.as_ref().to_owned(),
                        version.to_owned(),
                    );
                    if service_revisions.get(&revision_key).is_none() {
                        return Err(invalid_soracloud_state(
                            "soracloud_service_deployments",
                            format!(
                                "service `{}` {rollout_field}.{version_field} `{version}` is missing from soracloud_service_revisions",
                                deployment.service_name
                            ),
                        ));
                    }
                }
            }
        }

        let app_infra_audit_events = self.app_infra_audit_events.view();
        for (sequence, event) in app_infra_audit_events.iter() {
            event.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_app_infra_audit_events", error.to_string())
            })?;
            if sequence != &event.sequence {
                return Err(invalid_soracloud_state(
                    "soracloud_app_infra_audit_events",
                    "storage key must match the embedded audit sequence",
                ));
            }
            register_soracloud_sequence(
                &mut authoritative_sequences,
                "soracloud_app_infra_audit_events",
                event.sequence,
            )?;
        }
        for (key, state) in self.app_infra_states.view().iter() {
            state.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_app_infra_states", error.to_string())
            })?;
            if key != &state.app_name {
                return Err(invalid_soracloud_state(
                    "soracloud_app_infra_states",
                    "storage key must match the embedded app_name",
                ));
            }
            let updated_event = app_infra_audit_events
                .get(&state.updated_sequence)
                .ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_app_infra_audit_events",
                        "app state is missing its exact updated_sequence audit event",
                    )
                })?;
            if updated_event.app_name != state.app_name
                || updated_event.to_version != state.current_app_version
                || updated_event.app_manifest_hash != state.current_manifest_hash
            {
                return Err(invalid_soracloud_state(
                    "soracloud_app_infra_audit_events",
                    "app state's current revision does not match its updated_sequence audit event",
                ));
            }
            let deployed_event = app_infra_audit_events
                .get(&state.deployed_sequence)
                .ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_app_infra_audit_events",
                        "app state is missing its exact deployed_sequence audit event",
                    )
                })?;
            if deployed_event.app_name != state.app_name {
                return Err(invalid_soracloud_state(
                    "soracloud_app_infra_audit_events",
                    "app state's deployed_sequence audit event belongs to another app",
                ));
            }
            for service_ref in &state.manifest.services {
                let deployment = service_deployments
                    .get(&service_ref.service_name)
                    .ok_or_else(|| {
                        invalid_soracloud_state(
                            "soracloud_app_infra_states",
                            format!(
                                "app `{}` service `{}` has no authoritative deployment",
                                state.app_name, service_ref.service_name
                            ),
                        )
                    })?;
                if deployment.current_service_version != service_ref.service_version
                    || deployment.current_service_manifest_hash != service_ref.service_manifest_hash
                    || deployment.current_container_manifest_hash
                        != service_ref.container_manifest_hash
                {
                    return Err(invalid_soracloud_state(
                        "soracloud_app_infra_states",
                        format!(
                            "app `{}` service `{}` must exactly match its active deployment revision and manifest hashes",
                            state.app_name, service_ref.service_name
                        ),
                    ));
                }
                let revision_key = (
                    service_ref.service_name.as_ref().to_owned(),
                    service_ref.service_version.clone(),
                );
                let admitted_bundle = service_revisions.get(&revision_key).ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_app_infra_states",
                        format!(
                            "app `{}` service `{}` revision `{}` has no admitted bundle",
                            state.app_name, service_ref.service_name, service_ref.service_version
                        ),
                    )
                })?;
                if admitted_bundle.service.execution_plane != service_ref.execution_plane
                    || admitted_bundle.container.runtime != service_ref.runtime
                {
                    return Err(invalid_soracloud_state(
                        "soracloud_app_infra_states",
                        format!(
                            "app `{}` service `{}` execution plane and runtime must match its admitted bundle",
                            state.app_name, service_ref.service_name
                        ),
                    ));
                }
            }
        }

        for (key, state) in self.service_runtime.view().iter() {
            state.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_service_runtime", error.to_string())
            })?;
            if key != &state.service_name {
                return Err(invalid_soracloud_state(
                    "soracloud_service_runtime",
                    "storage key must match the embedded service_name",
                ));
            }
            let deployment = service_deployments.get(key).ok_or_else(|| {
                invalid_soracloud_state(
                    "soracloud_service_runtime",
                    format!("service `{key}` runtime state has no authoritative deployment"),
                )
            })?;
            if state.active_service_version != deployment.current_service_version {
                return Err(invalid_soracloud_state(
                    "soracloud_service_runtime",
                    format!(
                        "service `{key}` runtime revision `{}` must equal its active deployment revision `{}`",
                        state.active_service_version, deployment.current_service_version
                    ),
                ));
            }
        }

        let inrou_service_placements = self.inrou_service_placements.view();
        for (key, state) in self.inrou_replica_runtime.view().iter() {
            state.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_inrou_replica_runtime", error.to_string())
            })?;
            let canonical_slot = state.replica_slot.to_string();
            let parsed_slot = key.2.parse::<u16>().map_err(|error| {
                invalid_soracloud_state(
                    "soracloud_inrou_replica_runtime",
                    format!("replica-slot storage key is not a canonical u16: {error}"),
                )
            })?;
            if key.0.as_str() != state.service_name.as_ref()
                || key.1.as_str() != state.service_version.as_str()
                || parsed_slot != state.replica_slot
                || key.2.as_str() != canonical_slot.as_str()
            {
                return Err(invalid_soracloud_state(
                    "soracloud_inrou_replica_runtime",
                    "storage key must exactly match the embedded service revision and canonical replica slot",
                ));
            }
            let placement_record = inrou_service_placements
                .get(&(
                    state.service_name.as_ref().to_owned(),
                    state.service_version.clone(),
                ))
                .ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_inrou_replica_runtime",
                        format!(
                            "service `{}` revision `{}` replica {} has no authoritative placement record",
                            state.service_name, state.service_version, state.replica_slot
                        ),
                    )
                })?;
            let assignment = placement_record
                .placements
                .iter()
                .find(|placement| placement.replica_slot == state.replica_slot)
                .ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_inrou_replica_runtime",
                        format!(
                            "service `{}` revision `{}` replica {} has no authoritative placement assignment",
                            state.service_name, state.service_version, state.replica_slot
                        ),
                    )
                })?;
            if state.validator_account_id != assignment.validator_account_id
                || state.peer_id != assignment.peer_id
                || state.selected_guest_isa != assignment.selected_guest_isa
            {
                return Err(invalid_soracloud_state(
                    "soracloud_inrou_replica_runtime",
                    "replica runtime identity must exactly match its authoritative placement assignment",
                ));
            }
        }

        for (sequence, event) in self.service_audit_events.view().iter() {
            event.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_service_audit_events", error.to_string())
            })?;
            if sequence != &event.sequence {
                return Err(invalid_soracloud_state(
                    "soracloud_service_audit_events",
                    "storage key must match the embedded audit sequence",
                ));
            }
            register_soracloud_sequence(
                &mut authoritative_sequences,
                "soracloud_service_audit_events",
                event.sequence,
            )?;
        }

        for (sequence, event) in self.training_job_audit_events.view().iter() {
            event.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_training_job_audit_events", error.to_string())
            })?;
            if sequence != &event.sequence {
                return Err(invalid_soracloud_state(
                    "soracloud_training_job_audit_events",
                    "storage key must match the embedded audit sequence",
                ));
            }
            register_soracloud_sequence(
                &mut authoritative_sequences,
                "soracloud_training_job_audit_events",
                event.sequence,
            )?;
        }

        for (sequence, event) in self.model_weight_audit_events.view().iter() {
            event.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_model_weight_audit_events", error.to_string())
            })?;
            if sequence != &event.sequence {
                return Err(invalid_soracloud_state(
                    "soracloud_model_weight_audit_events",
                    "storage key must match the embedded audit sequence",
                ));
            }
            register_soracloud_sequence(
                &mut authoritative_sequences,
                "soracloud_model_weight_audit_events",
                event.sequence,
            )?;
        }

        for (sequence, event) in self.model_artifact_audit_events.view().iter() {
            event.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_model_artifact_audit_events", error.to_string())
            })?;
            if sequence != &event.sequence {
                return Err(invalid_soracloud_state(
                    "soracloud_model_artifact_audit_events",
                    "storage key must match the embedded audit sequence",
                ));
            }
            register_soracloud_sequence(
                &mut authoritative_sequences,
                "soracloud_model_artifact_audit_events",
                event.sequence,
            )?;
        }

        for (sequence, event) in self.hf_shared_lease_audit_events.view().iter() {
            event.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_hf_shared_lease_audit_events", error.to_string())
            })?;
            if sequence != &event.sequence {
                return Err(invalid_soracloud_state(
                    "soracloud_hf_shared_lease_audit_events",
                    "storage key must match the embedded audit sequence",
                ));
            }
            register_soracloud_sequence(
                &mut authoritative_sequences,
                "soracloud_hf_shared_lease_audit_events",
                event.sequence,
            )?;
        }

        for (key, record) in self.model_host_violation_evidence.view().iter() {
            record.validate().map_err(|error| {
                invalid_soracloud_state(
                    "soracloud_model_host_violation_evidence",
                    error.to_string(),
                )
            })?;
            if key != &record.evidence_id {
                return Err(invalid_soracloud_state(
                    "soracloud_model_host_violation_evidence",
                    "storage key must match the embedded evidence_id",
                ));
            }
            register_soracloud_sequence(
                &mut authoritative_sequences,
                "soracloud_model_host_violation_evidence",
                record.sequence,
            )?;
        }

        for (sequence, event) in self.agent_apartment_audit_events.view().iter() {
            event.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_agent_apartment_audit_events", error.to_string())
            })?;
            if sequence != &event.sequence {
                return Err(invalid_soracloud_state(
                    "soracloud_agent_apartment_audit_events",
                    "storage key must match the embedded audit sequence",
                ));
            }
            register_soracloud_sequence(
                &mut authoritative_sequences,
                "soracloud_agent_apartment_audit_events",
                event.sequence,
            )?;
        }

        for (key, entry) in self.service_state_entries.view().iter() {
            entry.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_service_state_entries", error.to_string())
            })?;
            let expected_key = (
                entry.service_name.as_ref().to_owned(),
                entry.binding_name.as_ref().to_owned(),
                entry.state_key.clone(),
            );
            if key != &expected_key {
                return Err(invalid_soracloud_state(
                    "soracloud_service_state_entries",
                    "storage key must match the embedded service_name, binding_name, and state_key",
                ));
            }
        }

        for (key, capability) in self.inrou_host_capabilities.view().iter() {
            capability.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_inrou_host_capabilities", error.to_string())
            })?;
            if key != &capability.validator_account_id {
                return Err(invalid_soracloud_state(
                    "soracloud_inrou_host_capabilities",
                    "storage key must match the embedded validator_account_id",
                ));
            }
        }

        for (key, placement) in inrou_service_placements.iter() {
            placement.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_inrou_service_placements", error.to_string())
            })?;
            let expected_key = (
                placement.service_name.as_ref().to_owned(),
                placement.service_version.clone(),
            );
            if key != &expected_key {
                return Err(invalid_soracloud_state(
                    "soracloud_inrou_service_placements",
                    "storage key must match the embedded service_name and service_version",
                ));
            }
            let admitted_key = (
                placement.service_name.as_ref().to_owned(),
                placement.service_version.clone(),
            );
            let admitted_bundle = service_revisions.get(&admitted_key).ok_or_else(|| {
                invalid_soracloud_state(
                    "soracloud_inrou_service_placements",
                    format!(
                        "service `{}` revision `{}` placement has no admitted bundle",
                        placement.service_name, placement.service_version
                    ),
                )
            })?;
            if admitted_bundle.container.runtime
                != iroha_data_model::soracloud::SoraContainerRuntimeV1::Inrou
                || admitted_bundle.service.execution_plane
                    != iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::HttpService
            {
                return Err(invalid_soracloud_state(
                    "soracloud_inrou_service_placements",
                    "Inrou placement must reference an admitted Inrou HTTP-service revision",
                ));
            }
            if placement.desired_replica_count != admitted_bundle.service.replicas.get() {
                return Err(invalid_soracloud_state(
                    "soracloud_inrou_service_placements",
                    format!(
                        "service `{}` revision `{}` desired replica count {} must equal admitted manifest count {}",
                        placement.service_name,
                        placement.service_version,
                        placement.desired_replica_count,
                        admitted_bundle.service.replicas.get()
                    ),
                ));
            }
            let inrou = admitted_bundle.container.inrou.as_ref().ok_or_else(|| {
                invalid_soracloud_state(
                    "soracloud_inrou_service_placements",
                    "admitted Inrou revision is missing its canonical Inrou manifest",
                )
            })?;
            for assignment in &placement.placements {
                if !inrou
                    .guest_images
                    .contains_key(&assignment.selected_guest_isa)
                {
                    return Err(invalid_soracloud_state(
                        "soracloud_inrou_service_placements",
                        format!(
                            "service `{}` revision `{}` replica {} selects a guest ISA absent from its admitted Inrou manifest",
                            placement.service_name,
                            placement.service_version,
                            assignment.replica_slot
                        ),
                    ));
                }
            }
        }

        let mailbox_messages = self.mailbox_messages.view();
        for (key, message) in mailbox_messages.iter() {
            message.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_mailbox_messages", error.to_string())
            })?;
            if key != &message.message_id {
                return Err(invalid_soracloud_state(
                    "soracloud_mailbox_messages",
                    "storage key must match the embedded message_id",
                ));
            }
            register_soracloud_sequence(
                &mut authoritative_sequences,
                "soracloud_mailbox_messages",
                message.enqueue_sequence,
            )?;
            let source_bundle = service_revisions
                .get(&(
                    message.from_service.as_ref().to_owned(),
                    message.from_service_version.clone(),
                ))
                .ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_mailbox_messages",
                        "source service revision must exist in admitted Soracloud state",
                    )
                })?;
            let source_handler = source_bundle
                .service
                .handlers
                .iter()
                .find(|handler| handler.handler_name == message.from_handler)
                .ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_mailbox_messages",
                        "source handler must exist in the bound admitted service revision",
                    )
                })?;
            if !matches!(
                source_handler.class,
                iroha_data_model::soracloud::SoraServiceHandlerClassV1::Update
                    | iroha_data_model::soracloud::SoraServiceHandlerClassV1::PrivateUpdate
            ) || source_handler.mailbox.is_none()
            {
                return Err(invalid_soracloud_state(
                    "soracloud_mailbox_messages",
                    "source handler must be an update/private_update mailbox handler",
                ));
            }
            let destination_bundle = service_revisions
                .get(&(
                    message.to_service.as_ref().to_owned(),
                    message.to_service_version.clone(),
                ))
                .ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_mailbox_messages",
                        "destination service revision must exist in admitted Soracloud state",
                    )
                })?;
            let destination_handler = destination_bundle
                .service
                .handlers
                .iter()
                .find(|handler| handler.handler_name == message.to_handler)
                .ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_mailbox_messages",
                        "destination handler must exist in the bound admitted service revision",
                    )
                })?;
            let destination_mailbox = destination_handler.mailbox.as_ref().ok_or_else(|| {
                invalid_soracloud_state(
                    "soracloud_mailbox_messages",
                    "destination handler must carry an admitted mailbox contract",
                )
            })?;
            if !matches!(
                destination_handler.class,
                iroha_data_model::soracloud::SoraServiceHandlerClassV1::Update
                    | iroha_data_model::soracloud::SoraServiceHandlerClassV1::PrivateUpdate
            ) {
                return Err(invalid_soracloud_state(
                    "soracloud_mailbox_messages",
                    "destination handler must be update/private_update",
                ));
            }
            let payload_len = u64::try_from(message.payload_bytes.len()).map_err(|error| {
                invalid_soracloud_state(
                    "soracloud_mailbox_messages",
                    format!("payload length does not fit u64: {error}"),
                )
            })?;
            if payload_len > destination_mailbox.max_message_bytes.get() {
                return Err(invalid_soracloud_state(
                    "soracloud_mailbox_messages",
                    "payload exceeds the bound destination mailbox contract",
                ));
            }
            let retention_sequences = destination_mailbox.retention_sequences.get();
            if message.delivery_delay_sequences >= retention_sequences
                || message.available_after_sequence
                    != message
                        .enqueue_sequence
                        .checked_add(u64::from(message.delivery_delay_sequences))
                        .ok_or_else(|| {
                            invalid_soracloud_state(
                                "soracloud_mailbox_messages",
                                "derived availability sequence overflows",
                            )
                        })?
                || message.expires_at_sequence
                    != message
                        .enqueue_sequence
                        .checked_add(u64::from(retention_sequences))
                        .ok_or_else(|| {
                            invalid_soracloud_state(
                                "soracloud_mailbox_messages",
                                "derived expiry sequence overflows",
                            )
                        })?
            {
                return Err(invalid_soracloud_state(
                    "soracloud_mailbox_messages",
                    "ledger schedule must be exactly derived from enqueue, delay, and destination retention",
                ));
            }
        }

        let mut consumed_mailbox_messages = std::collections::BTreeSet::new();
        for (key, receipt) in self.runtime_receipts.view().iter() {
            receipt.validate().map_err(|error| {
                invalid_soracloud_state("soracloud_runtime_receipts", error.to_string())
            })?;
            if key != &receipt.receipt_id {
                return Err(invalid_soracloud_state(
                    "soracloud_runtime_receipts",
                    "storage key must match the embedded receipt_id",
                ));
            }
            register_soracloud_sequence(
                &mut authoritative_sequences,
                "soracloud_runtime_receipts",
                receipt.emitted_sequence,
            )?;
            let receipt_bundle = service_revisions
                .get(&(
                    receipt.service_name.as_ref().to_owned(),
                    receipt.service_version.clone(),
                ))
                .ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_runtime_receipts",
                        "receipt service revision must exist in admitted Soracloud state",
                    )
                })?;
            let receipt_handler = receipt_bundle
                .service
                .handlers
                .iter()
                .find(|handler| handler.handler_name == receipt.handler_name)
                .ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_runtime_receipts",
                        "receipt handler must exist in the bound admitted service revision",
                    )
                })?;
            if receipt_handler.class != receipt.handler_class
                || receipt_handler.certified_response != receipt.certified_by
            {
                return Err(invalid_soracloud_state(
                    "soracloud_runtime_receipts",
                    "receipt class and certification must match the admitted handler contract",
                ));
            }
            let hf_generated =
                crate::soracloud_runtime::soracloud_hf_generated_source_binding(receipt_bundle)
                    .is_some();
            if hf_generated
                != matches!(
                    receipt.execution_host.as_ref(),
                    Some(
                        iroha_data_model::soracloud::SoraRuntimeExecutionHostV1::HfModelHost(_)
                    )
                )
            {
                return Err(invalid_soracloud_state(
                    "soracloud_runtime_receipts",
                    "HF-generated receipts and only those receipts must carry HF model-host attribution",
                ));
            }
            if let Some(message_id) = receipt.mailbox_message_id {
                let message = mailbox_messages.get(&message_id).ok_or_else(|| {
                        invalid_soracloud_state(
                            "soracloud_runtime_receipts",
                            "mailbox receipt must reference an authoritative mailbox message",
                        )
                    })?;
                if message.to_service != receipt.service_name
                    || message.to_service_version != receipt.service_version
                    || message.to_handler != receipt.handler_name
                    || message.payload_commitment != receipt.request_commitment
                {
                    return Err(invalid_soracloud_state(
                        "soracloud_runtime_receipts",
                        "mailbox receipt must match the exact destination revision, handler, and payload commitment",
                    ));
                }
                if receipt.emitted_sequence < message.available_after_sequence
                    || receipt.emitted_sequence >= message.expires_at_sequence
                {
                    return Err(invalid_soracloud_state(
                        "soracloud_runtime_receipts",
                        "mailbox receipt sequence must be within the message delivery window",
                    ));
                }
                if !consumed_mailbox_messages.insert(message_id) {
                    return Err(invalid_soracloud_state(
                        "soracloud_runtime_receipts",
                        "one mailbox message must not be consumed by multiple receipts",
                    ));
                }
            }
        }

        for (key, receipt) in self
            .private_uploaded_model_execution_receipts
            .view()
            .iter()
        {
            receipt.validate().map_err(|error| {
                invalid_soracloud_state(
                    "soracloud_private_uploaded_model_execution_receipts",
                    error.to_string(),
                )
            })?;
            if key != &receipt.receipt_id {
                return Err(invalid_soracloud_state(
                    "soracloud_private_uploaded_model_execution_receipts",
                    "storage key must match the embedded receipt_id",
                ));
            }
            register_soracloud_sequence(
                &mut authoritative_sequences,
                "soracloud_private_uploaded_model_execution_receipts",
                receipt.emitted_sequence,
            )?;
            let bundle = uploaded_model_bundles
                .get(&(
                    receipt.service_name.as_ref().to_owned(),
                    receipt.model_id.clone(),
                    receipt.weight_version.clone(),
                ))
                .ok_or_else(|| {
                    invalid_soracloud_state(
                        "soracloud_private_uploaded_model_execution_receipts",
                        "private receipt must reference an authoritative uploaded-model bundle",
                    )
                })?;
            if bundle.sorafs_manifest_digest != receipt.model_manifest_digest
                || bundle.bundle_root != receipt.model_bundle_root
                || bundle.decryption_policy_ref != receipt.policy_id
            {
                return Err(invalid_soracloud_state(
                    "soracloud_private_uploaded_model_execution_receipts",
                    "private receipt must exactly match its uploaded-model bundle and policy",
                ));
            }
        }
        Ok(())
    }
}

fn register_soracloud_sequence(
    authoritative_sequences: &mut std::collections::BTreeSet<u64>,
    field: &str,
    sequence: u64,
) -> Result<(), json::Error> {
    if authoritative_sequences.insert(sequence) {
        Ok(())
    } else {
        Err(invalid_soracloud_state(
            field,
            format!(
                "authoritative sequence `{sequence}` collides with another Soracloud record"
            ),
        ))
    }
}

fn invalid_soracloud_state(field: &str, message: impl Into<String>) -> json::Error {
    json::Error::InvalidField {
        field: format!("world.{field}"),
        message: message.into(),
    }
}
impl MusubiPersistedState<'_> {
    #[allow(clippy::too_many_lines)]
    fn validate(self) -> Result<(), json::Error> {
        let namespace_bindings = self.namespace_bindings.view();
        let packages = self.packages.view();
        let package_metadata = self.package_metadata.view();
        let package_members = self.package_members.view();
        let package_invitations = self.package_invitations.view();
        let maintainer_directory = self.maintainer_directory.view();
        let releases = self.releases.view();
        let archives = self.archives.view();
        let provider_bundle_attestations = self.provider_bundle_attestations.view();
        let archive_locations = self.archive_locations.view();
        let archive_availability = self.archive_availability.view();
        let archive_reverse_references = self.archive_reverse_references.view();
        let resolver_index = self.resolver_index.view();
        validate_musubi_resolver_checkpoint_structure(
            self.resolver_index_checkpoints,
            self.resolver_index_revision,
        )?;
        let public_directory = self.public_directory.view();
        let aliases = self.aliases.view();
        let alias_history = self.alias_history.view();
        let governance_decisions = self.governance_decisions.view();
        let mut decision_actions = Vec::new();
        for (decision_id, consumption) in governance_decisions.iter() {
            consumption.validate().map_err(|error| {
                invalid_musubi_state("musubi_governance_decisions", error.to_string())
            })?;
            let decision = &consumption.decision;
            if decision_id != &decision.decision_id {
                return Err(invalid_musubi_state(
                    "musubi_governance_decisions",
                    "governance-decision key does not match its embedded decision id",
                ));
            }
            decision_actions.push(decision.action_digest);
        }
        for (package_id, metadata) in package_metadata.iter() {
            metadata.package.validate().map_err(|error| {
                invalid_musubi_state("musubi_package_metadata", error.to_string())
            })?;
            metadata.validate().map_err(|error| {
                invalid_musubi_state("musubi_package_metadata", error.to_string())
            })?;
            if package_id != &metadata.package || packages.get(package_id).is_none() {
                return Err(invalid_musubi_state(
                    "musubi_package_metadata",
                    "metadata key/identity is inconsistent with the package store",
                ));
            }
        }
        let mut member_accounts = BTreeMap::<MusubiPackageIdV1, Vec<AccountId>>::new();
        let mut owner_accounts = BTreeMap::<MusubiPackageIdV1, Vec<AccountId>>::new();
        for (member_key, member) in package_members.iter() {
            member.package.validate().map_err(|error| {
                invalid_musubi_state("musubi_package_members", error.to_string())
            })?;
            member.validate().map_err(|error| {
                invalid_musubi_state("musubi_package_members", error.to_string())
            })?;
            let package = packages.get(&member.package).ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_package_members",
                    "member references a missing package",
                )
            })?;
            if member_key != &member.key()
                || member.governance_revision > package.revisions.governance
            {
                return Err(invalid_musubi_state(
                    "musubi_package_members",
                    "member key or governance revision is inconsistent with its package",
                ));
            }
            let directory_key = MusubiMaintainerDirectoryKeyV1::accepted(
                member.package.clone(),
                member.account.clone(),
            );
            if maintainer_directory.get(&directory_key)
                != Some(&MusubiMaintainerDirectoryEntryV1::Accepted(member.clone()))
            {
                return Err(invalid_musubi_state(
                    "musubi_maintainer_directory",
                    "accepted member is missing from the exact maintainer directory",
                ));
            }
            member_accounts
                .entry(member.package.clone())
                .or_default()
                .push(member.account.clone());
            if matches!(
                member.role,
                iroha_data_model::musubi::MusubiPackageRoleV1::Owner
            ) {
                owner_accounts
                    .entry(member.package.clone())
                    .or_default()
                    .push(member.account.clone());
            }
        }
        for (invite_id, invitation) in package_invitations.iter() {
            invitation.package.validate().map_err(|error| {
                invalid_musubi_state("musubi_package_invitations", error.to_string())
            })?;
            invitation.validate().map_err(|error| {
                invalid_musubi_state("musubi_package_invitations", error.to_string())
            })?;
            let package = packages.get(&invitation.package).ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_package_invitations",
                    "invitation references a missing package",
                )
            })?;
            let revision_is_invalid =
                if invitation.state == iroha_data_model::musubi::MusubiInvitationStateV1::Pending {
                    invitation.expected_governance_revision != package.revisions.governance
                } else {
                    invitation.expected_governance_revision > package.revisions.governance
                };
            if invite_id != &invitation.invite_id || revision_is_invalid {
                return Err(invalid_musubi_state(
                    "musubi_package_invitations",
                    "invitation key or pending governance revision is inconsistent with its package",
                ));
            }
            let directory_key = MusubiMaintainerDirectoryKeyV1::pending(
                invitation.package.clone(),
                invitation.invited_account.clone(),
                invitation.invite_id,
            );
            let indexed = maintainer_directory.get(&directory_key);
            if invitation.state == iroha_data_model::musubi::MusubiInvitationStateV1::Pending {
                if indexed
                    != Some(&MusubiMaintainerDirectoryEntryV1::PendingInvitation(
                        invitation.clone(),
                    ))
                {
                    return Err(invalid_musubi_state(
                        "musubi_maintainer_directory",
                        "pending invitation is missing from the exact maintainer directory",
                    ));
                }
            } else if indexed.is_some() {
                return Err(invalid_musubi_state(
                    "musubi_maintainer_directory",
                    "non-pending invitation remains selectable in the maintainer directory",
                ));
            }
        }
        let mut pending_directory_counts = BTreeMap::<MusubiPackageIdV1, usize>::new();
        for (directory_key, entry) in maintainer_directory.iter() {
            entry.validate().map_err(|error| {
                invalid_musubi_state("musubi_maintainer_directory", error.to_string())
            })?;
            if directory_key != &entry.key() {
                return Err(invalid_musubi_state(
                    "musubi_maintainer_directory",
                    "maintainer directory key disagrees with its embedded entry",
                ));
            }
            let authoritative = match entry {
                MusubiMaintainerDirectoryEntryV1::Accepted(member) => package_members
                    .get(&member.key())
                    .is_some_and(|stored| stored == member),
                MusubiMaintainerDirectoryEntryV1::PendingInvitation(invitation) => {
                    let count = pending_directory_counts
                        .entry(invitation.package.clone())
                        .or_default();
                    *count = count.saturating_add(1);
                    if *count > MUSUBI_MAX_PENDING_INVITATIONS_V1 {
                        return Err(invalid_musubi_state(
                            "musubi_maintainer_directory",
                            "package exceeds the pending-invitation bound",
                        ));
                    }
                    package_invitations
                        .get(&invitation.invite_id)
                        .is_some_and(|stored| stored == invitation)
                }
            };
            if !authoritative {
                return Err(invalid_musubi_state(
                    "musubi_maintainer_directory",
                    "maintainer directory entry lacks an exact authoritative record",
                ));
            }
        }
        for (package_id, package) in packages.iter() {
            package
                .validate()
                .map_err(|error| invalid_musubi_state("musubi_packages", error.to_string()))?;
            if package_id != &package.package {
                return Err(invalid_musubi_state(
                    "musubi_packages",
                    "package key does not match the embedded package identity",
                ));
            }
            let binding = namespace_bindings
                .get(&package.claimed_namespace)
                .ok_or_else(|| {
                    invalid_musubi_state(
                        "musubi_packages",
                        "package references a missing immutable namespace binding",
                    )
                })?;
            if binding.digest() != package.claimed_namespace_binding
                || binding.home_dataspace != package.package.home_dataspace
                || binding.scope != package.package.scope
            {
                return Err(invalid_musubi_state(
                    "musubi_packages",
                    "package identity disagrees with its immutable namespace binding",
                ));
            }
            let metadata = package_metadata.get(package_id).ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_package_metadata",
                    "package is missing its authoritative metadata projection",
                )
            })?;
            if metadata.revision != package.revisions.metadata {
                return Err(invalid_musubi_state(
                    "musubi_package_metadata",
                    "metadata revision disagrees with the package revision",
                ));
            }
            let actual_members = member_accounts.remove(package_id).unwrap_or_default();
            let actual_owners = owner_accounts.remove(package_id).unwrap_or_default();
            if actual_members != package.member_accounts || actual_owners != package.owners {
                return Err(invalid_musubi_state(
                    "musubi_package_members",
                    "authoritative member roles disagree with package owner/member indexes",
                ));
            }
        }
        let mut current_location_ids =
            BTreeMap::<ArchiveId, Vec<iroha_data_model::musubi::MusubiArchiveLocationIdV1>>::new();
        let mut location_providers =
            BTreeMap::<ArchiveId, BTreeSet<iroha_data_model::sorafs::capacity::ProviderId>>::new();
        for (attestation_key, record) in provider_bundle_attestations.iter() {
            record.validate().map_err(|error| {
                invalid_musubi_state("musubi_provider_bundle_attestations", error.to_string())
            })?;
            if attestation_key != &record.key {
                return Err(invalid_musubi_state(
                    "musubi_provider_bundle_attestations",
                    "provider-attestation key does not match the stored record",
                ));
            }
            let archive = archives.get(&record.key.archive_id).ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_provider_bundle_attestations",
                    "provider attestation references a missing archive",
                )
            })?;
            let receipt = &archive.staging_receipt.payload.binding;
            let binding = &record.attestation.payload.binding;
            if record.registered_at_height < archive.registered_at_height
                || binding.network_id != receipt.network_id
                || binding.archive_id != archive.archive_id
                || binding.bundle_digest != archive.commitment.bundle_digest
                || binding.descriptor_digest != archive.commitment.descriptor_digest
                || binding.semantic_release_manifest_digest
                    != receipt.semantic_release_manifest_digest
                || binding.source_tree_digest != archive.commitment.source_tree_digest
            {
                return Err(invalid_musubi_state(
                    "musubi_provider_bundle_attestations",
                    "provider attestation does not match the archive and ingress receipt commitments",
                ));
            }
        }
        for (location_key, location) in archive_locations.iter() {
            location.validate().map_err(|error| {
                invalid_musubi_state("musubi_archive_locations", error.to_string())
            })?;
            let archive = archives.get(&location.archive_id).ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_archive_locations",
                    "archive location references a missing archive",
                )
            })?;
            let mut attestation_references = Vec::with_capacity(location.providers.len());
            let mut verification_lock_digest = None;
            for provider_id in &location.providers {
                let key = MusubiProviderBundleAttestationKeyV1 {
                    archive_id: location.archive_id,
                    replication_order: location.replication_order,
                    provider_id: *provider_id,
                };
                let record = provider_bundle_attestations.get(&key).ok_or_else(|| {
                    invalid_musubi_state(
                        "musubi_archive_locations",
                        "archive location references a missing exact provider attestation",
                    )
                })?;
                let digest = record.attestation.payload.binding.verification_lock_digest;
                if verification_lock_digest.is_some_and(|expected| expected != digest) {
                    return Err(invalid_musubi_state(
                        "musubi_archive_locations",
                        "archive-location provider attestations disagree on the verification lock",
                    ));
                }
                verification_lock_digest = Some(digest);
                attestation_references.push(MusubiProviderBundleAttestationRefV1 {
                    provider_id: *provider_id,
                    digest: record.attestation_digest,
                });
            }
            let attestation_set_digest = musubi_provider_bundle_attestation_set_digest_v1(
                location.archive_id,
                location.replication_order,
                &attestation_references,
            )
            .map_err(|error| invalid_musubi_state("musubi_archive_locations", error.to_string()))?;
            if attestation_set_digest != location.provider_attestation_set_digest {
                return Err(invalid_musubi_state(
                    "musubi_archive_locations",
                    "archive-location provider-attestation set digest is not exact",
                ));
            }
            if location_key != &location.key() || location.revision > archive.location_revision {
                return Err(invalid_musubi_state(
                    "musubi_archive_locations",
                    "archive-location key or revision is inconsistent with its archive",
                ));
            }
            if location.state != MusubiArchiveLocationStateV1::Retired {
                current_location_ids
                    .entry(location.archive_id)
                    .or_default()
                    .push(location.location_id);
                location_providers
                    .entry(location.archive_id)
                    .or_default()
                    .extend(location.providers.iter().copied());
            }
        }
        for (archive_id, archive) in archives.iter() {
            archive
                .validate()
                .map_err(|error| invalid_musubi_state("musubi_archives", error.to_string()))?;
            if archive_id != &archive.archive_id {
                return Err(invalid_musubi_state(
                    "musubi_archives",
                    "archive key does not match the embedded archive identity",
                ));
            }
            let mut expected_locations =
                current_location_ids.remove(archive_id).unwrap_or_default();
            expected_locations.sort();
            if expected_locations != archive.location_ids {
                return Err(invalid_musubi_state(
                    "musubi_archives",
                    "archive location directory is not the exact current non-retired set",
                ));
            }
        }
        for (archive_id, availability) in archive_availability.iter() {
            availability.validate().map_err(|error| {
                invalid_musubi_state("musubi_archive_availability", error.to_string())
            })?;
            let archive = archives.get(archive_id).ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_archive_availability",
                    "availability projection references a missing archive",
                )
            })?;
            let provider_bound = location_providers.get(archive_id).map_or(0, BTreeSet::len);
            if archive_id != &availability.archive_id
                || usize::from(availability.active_locations) > archive.location_ids.len()
                || usize::from(availability.healthy_replicas) > provider_bound
                || availability.index_revision > self.resolver_index_revision
            {
                return Err(invalid_musubi_state(
                    "musubi_archive_availability",
                    "availability key, counts, or revision disagrees with authoritative archive state",
                ));
            }
        }
        for (archive_id, _) in archives.iter() {
            if archive_availability.get(archive_id).is_none() {
                return Err(invalid_musubi_state(
                    "musubi_archive_availability",
                    "archive is missing its authoritative availability projection",
                ));
            }
        }
        let mut expected_references = BTreeMap::<ArchiveId, Vec<MusubiReleaseIdV1>>::new();
        for (archive_id, _) in archives.iter() {
            expected_references.insert(*archive_id, Vec::new());
        }
        for (release_id, release) in releases.iter() {
            release
                .validate()
                .map_err(|error| invalid_musubi_state("musubi_releases", error.to_string()))?;
            if release_id != &release.manifest.release
                || packages.get(&release_id.package).is_none()
            {
                return Err(invalid_musubi_state(
                    "musubi_releases",
                    "release key or package reference is inconsistent",
                ));
            }
            let archive = archives.get(&release.manifest.archive_id).ok_or_else(|| {
                invalid_musubi_state("musubi_releases", "release references a missing archive")
            })?;
            let receipt = &archive.staging_receipt.payload.binding;
            if release.manifest.semantic_digest() != receipt.semantic_release_manifest_digest
                || release.published_by != receipt.publisher
            {
                return Err(invalid_musubi_state(
                    "musubi_releases",
                    "release does not match its archive ingress receipt",
                ));
            }
            let references = expected_references
                .get_mut(&release.manifest.archive_id)
                .expect("validated archive has a reverse-reference accumulator");
            references.push(release_id.clone());
            if let iroha_data_model::musubi::MusubiArtifactGovernanceStateV1::TakenDown(takedown) =
                &release.artifact_governance
                && !decision_actions.contains(&takedown.action_digest)
            {
                return Err(invalid_musubi_state(
                    "musubi_releases",
                    "artifact takedown does not reference a retained governance decision",
                ));
            }
        }
        for (archive_id, references) in archive_reverse_references.iter() {
            references.validate().map_err(|error| {
                invalid_musubi_state("musubi_archive_reverse_references", error.to_string())
            })?;
            let expected = expected_references.get(archive_id).ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_archive_reverse_references",
                    "reverse-reference record names a missing archive",
                )
            })?;
            if archive_id != &references.archive_id || &references.releases != expected {
                return Err(invalid_musubi_state(
                    "musubi_archive_reverse_references",
                    "archive reverse references are not the exact release set",
                ));
            }
        }
        let mut expected_replication_shortfall_releases = 0_u64;
        for (archive_id, expected) in &expected_references {
            if archive_reverse_references.get(archive_id).is_none() {
                return Err(invalid_musubi_state(
                    "musubi_archive_reverse_references",
                    format!(
                        "archive is missing its reverse-reference set of {} releases",
                        expected.len()
                    ),
                ));
            }
            let availability = archive_availability
                .get(archive_id)
                .expect("every validated archive has an availability projection");
            if availability.availability != MusubiStorageAvailabilityV1::Selectable {
                let release_count = u64::try_from(expected.len()).map_err(|_| {
                    invalid_musubi_state(
                        "musubi_replication_shortfall_releases",
                        "archive reverse-reference count overflows u64",
                    )
                })?;
                expected_replication_shortfall_releases = expected_replication_shortfall_releases
                    .checked_add(release_count)
                    .ok_or_else(|| {
                        invalid_musubi_state(
                            "musubi_replication_shortfall_releases",
                            "replication-shortfall release count overflows u64",
                        )
                    })?;
            }
        }
        if self.replication_shortfall_releases != expected_replication_shortfall_releases {
            return Err(invalid_musubi_state(
                "musubi_replication_shortfall_releases",
                format!(
                    "persisted count {} does not match the exact derived count {expected_replication_shortfall_releases}",
                    self.replication_shortfall_releases
                ),
            ));
        }
        let mut latest_selectable =
            BTreeMap::<MusubiPackageIdV1, Option<iroha_data_model::musubi::MusubiVersionV1>>::new();
        for (package_id, _) in packages.iter() {
            latest_selectable.insert(package_id.clone(), None);
        }
        for (release_id, row) in resolver_index.iter() {
            row.validate().map_err(|error| {
                invalid_musubi_state("musubi_resolver_index", error.to_string())
            })?;
            let release = releases.get(release_id).ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_resolver_index",
                    "resolver row references a missing release",
                )
            })?;
            let archive = archives.get(&release.manifest.archive_id).ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_resolver_index",
                    "resolver row references a missing archive",
                )
            })?;
            let availability = archive_availability
                .get(&release.manifest.archive_id)
                .ok_or_else(|| {
                    invalid_musubi_state(
                        "musubi_resolver_index",
                        "resolver row is missing its authoritative storage projection",
                    )
                })?;
            if release_id != &row.release
                || row.release_digest != release.release_digest
                || row.archive_id != release.manifest.archive_id
                || row.source_digest != archive.commitment.source_tree_digest
                || row.interface_digest != release.manifest.interface_digest
                || row.abi != release.manifest.abi
                || row.dependencies.as_slice() != release.manifest.dependencies.as_slice()
                || row.selection.yank != release.yank
                || row.selection.governance != release.artifact_governance
                || row.selection.storage != *availability
                || row.index_revision > self.resolver_index_revision
            {
                return Err(invalid_musubi_state(
                    "musubi_resolver_index",
                    "resolver row diverges from authoritative release/archive projections",
                ));
            }
            if row.selection.fresh_selectable() {
                let latest = latest_selectable
                    .get_mut(&release_id.package)
                    .expect("validated release package has a directory accumulator");
                if latest
                    .as_ref()
                    .is_none_or(|version| version < &release_id.version)
                {
                    *latest = Some(release_id.version.clone());
                }
            }
        }
        for (release_id, _) in releases.iter() {
            if resolver_index.get(release_id).is_none() {
                return Err(invalid_musubi_state(
                    "musubi_resolver_index",
                    "release is missing its exact resolver row",
                ));
            }
        }
        for (selector, entry) in public_directory.iter() {
            entry.validate().map_err(|error| {
                invalid_musubi_state("musubi_public_directory", error.to_string())
            })?;
            let package = packages.get(&entry.package).ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_public_directory",
                    "directory entry references a missing package",
                )
            })?;
            let expected_latest = latest_selectable
                .get(&entry.package)
                .expect("validated directory package has a latest-version accumulator");
            if selector != &entry.selector
                || entry.selector.namespace != package.claimed_namespace
                || entry.selector.name != entry.package.name
                || entry.metadata_revision != package.revisions.metadata
                || &entry.latest_selectable != expected_latest
                || entry.index_revision > self.resolver_index_revision
            {
                return Err(invalid_musubi_state(
                    "musubi_public_directory",
                    "directory entry diverges from its package and resolver rows",
                ));
            }
        }
        for (package_id, package) in packages.iter() {
            let selector = MusubiPackageSelectorV1 {
                namespace: package.claimed_namespace.clone(),
                name: package_id.name.clone(),
            };
            if public_directory.get(&selector).is_none() {
                return Err(invalid_musubi_state(
                    "musubi_public_directory",
                    "package is missing its canonical public-directory entry",
                ));
            }
        }
        for (alias, record) in aliases.iter() {
            record
                .alias
                .validate()
                .map_err(|error| invalid_musubi_state("musubi_aliases", error.to_string()))?;
            record
                .target
                .validate()
                .map_err(|error| invalid_musubi_state("musubi_aliases", error.to_string()))?;
            if alias != &record.alias
                || packages.get(&record.target).is_none()
                || record.pricing_revision == 0
                || record.paid_xor == 0
                || record.registered_at_height == 0
                || record.history_revision == 0
            {
                return Err(invalid_musubi_state(
                    "musubi_aliases",
                    "alias record is intrinsically invalid or references a missing package",
                ));
            }
        }
        let mut histories = BTreeMap::<MusubiAliasNameV1, Vec<&MusubiAliasHistoryEntryV1>>::new();
        for (history_key, entry) in alias_history.iter() {
            entry
                .validate()
                .map_err(|error| invalid_musubi_state("musubi_alias_history", error.to_string()))?;
            if history_key != &entry.key() || aliases.get(&entry.alias).is_none() {
                return Err(invalid_musubi_state(
                    "musubi_alias_history",
                    "alias-history key or alias reference is inconsistent",
                ));
            }
            if packages.get(&entry.target).is_none()
                || entry
                    .previous_target
                    .as_ref()
                    .is_some_and(|target| packages.get(target).is_none())
            {
                return Err(invalid_musubi_state(
                    "musubi_alias_history",
                    "alias history references a missing package",
                ));
            }
            if entry
                .governance_action
                .is_some_and(|digest| !decision_actions.contains(&digest))
            {
                return Err(invalid_musubi_state(
                    "musubi_alias_history",
                    "alias retarget does not reference a retained governance decision",
                ));
            }
            histories
                .entry(entry.alias.clone())
                .or_default()
                .push(entry);
        }
        for (alias, record) in aliases.iter() {
            let entries = histories.get(alias).ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_alias_history",
                    "alias is missing its complete history",
                )
            })?;
            let expected_len = usize::try_from(record.history_revision).map_err(|_| {
                invalid_musubi_state(
                    "musubi_alias_history",
                    "alias history revision does not fit usize",
                )
            })?;
            if entries.len() != expected_len {
                return Err(invalid_musubi_state(
                    "musubi_alias_history",
                    "alias history revisions are not dense from one",
                ));
            }
            let mut previous_target = None;
            let mut previous_height = 0_u64;
            for (index, entry) in entries.iter().enumerate() {
                let expected_revision = u64::try_from(index + 1).map_err(|_| {
                    invalid_musubi_state(
                        "musubi_alias_history",
                        "alias history index overflows u64",
                    )
                })?;
                if entry.revision != expected_revision
                    || (index == 0 && entry.finalized_height != record.registered_at_height)
                    || (index > 0
                        && (entry.previous_target.as_ref() != previous_target.as_ref()
                            || previous_target.as_ref() == Some(&entry.target)
                            || entry.finalized_height < previous_height))
                {
                    return Err(invalid_musubi_state(
                        "musubi_alias_history",
                        "alias history has a gap or broken target chain",
                    ));
                }
                previous_target = Some(entry.target.clone());
                previous_height = entry.finalized_height;
            }
            if previous_target.as_ref() != Some(&record.target) {
                return Err(invalid_musubi_state(
                    "musubi_alias_history",
                    "alias history final target disagrees with the current alias",
                ));
            }
        }
        Ok(())
    }
}
fn validate_musubi_governance_provenance(world: &World) -> Result<(), json::Error> {
    let decisions = world.musubi_governance_decisions.view();
    let proposals = world.governance_proposals.view();
    let packages = world.musubi_packages.view();
    let package_members = world.musubi_package_members.view();
    let releases = world.musubi_releases.view();
    let aliases = world.musubi_aliases.view();
    let alias_history = world.musubi_alias_history.view();
    let registry_policy = world.musubi_registry_policy.view();
    let mut actions_by_digest = BTreeMap::new();
    let mut policy_actions = Vec::new();
    let mut owner_recovery_revisions = BTreeSet::new();
    let mut owner_recovery_history: BTreeMap<MusubiPackageIdV1, Vec<(u64, u64)>> = BTreeMap::new();
    for (decision_id, consumption) in decisions.iter() {
        consumption.validate().map_err(|error| {
            invalid_musubi_state("musubi_governance_decisions", error.to_string())
        })?;
        let decision = &consumption.decision;
        let proposal = proposals.get(decision_id).ok_or_else(|| {
            invalid_musubi_state(
                "musubi_governance_decisions",
                "consumed Parliament decision has no retained governance proposal",
            )
        })?;
        let action = proposal.as_musubi_registry_governance().ok_or_else(|| {
            invalid_musubi_state(
                "musubi_governance_decisions",
                "consumed Parliament decision does not reference a Musubi action",
            )
        })?;
        action.validate().map_err(|error| {
            invalid_musubi_state("musubi_governance_decisions", error.to_string())
        })?;
        if decision.decision_id != *decision_id || proposal.kind.fingerprint() != *decision_id {
            return Err(invalid_musubi_state(
                "musubi_governance_decisions",
                "consumed Parliament decision key is not the exact typed proposal fingerprint",
            ));
        }
        if proposal.status != GovernanceProposalStatus::Enacted
            || action.action_digest() != decision.action_digest
        {
            return Err(invalid_musubi_state(
                "musubi_governance_decisions",
                "consumed Parliament decision disagrees with its enacted proposal",
            ));
        }
        let proposal_content_id =
            iroha_data_model::governance::types::ProposalContentId::new(*decision_id);
        let enacted_attempts = world
            .parliament_attempts
            .view()
            .iter()
            .filter(|(_, attempt)| attempt.proposal_content_id() == proposal_content_id)
            .filter(|(_, attempt)| {
                attempt.attempt().status
                    == iroha_data_model::governance::types::GovernanceAttemptStatusV1::Enacted
                    && attempt.terminal_height() == Some(decision.enacted_at_height)
                    && attempt.certificate().is_some_and(|certificate| {
                        certificate.proposal_content_id == proposal_content_id
                            && certificate.enact_at_height == decision.enacted_at_height
                    })
            })
            .count();
        if enacted_attempts != 1 {
            return Err(invalid_musubi_state(
                "musubi_governance_decisions",
                "consumed Parliament decision has no unique enacted attempt at its claimed height",
            ));
        }
        if actions_by_digest
            .insert(decision.action_digest, (action, consumption))
            .is_some()
        {
            return Err(invalid_musubi_state(
                "musubi_governance_decisions",
                "the same Parliament action was consumed more than once",
            ));
        }
        if let iroha_data_model::musubi::MusubiParliamentActionV1::SetRegistryPolicy(replacement) =
            action
        {
            policy_actions.push((replacement, consumption.consumed_at_height));
        }
    }
    policy_actions.sort_by_key(|(replacement, _)| replacement.policy.revision);
    let mut reconstructed_policy = MusubiRegistryPolicyV1::default();
    let mut previous_policy_consumed_at = None;
    let mut pricing_policies = BTreeMap::new();
    pricing_policies.insert(
        reconstructed_policy.alias_pricing.revision,
        reconstructed_policy.alias_pricing,
    );
    for (replacement, consumed_at_height) in policy_actions {
        if previous_policy_consumed_at.is_some_and(|previous| consumed_at_height < previous) {
            return Err(invalid_musubi_state(
                "musubi_registry_policy",
                "policy decision consumption heights do not follow policy revision order",
            ));
        }
        previous_policy_consumed_at = Some(consumed_at_height);
        if replacement.expected_revision != reconstructed_policy.revision {
            return Err(invalid_musubi_state(
                "musubi_registry_policy",
                "retained policy decisions do not form one dense revision history",
            ));
        }
        replacement
            .policy
            .validate_successor(&reconstructed_policy)
            .map_err(|error| invalid_musubi_state("musubi_registry_policy", error.to_string()))?;
        if pricing_policies
            .insert(
                replacement.policy.alias_pricing.revision,
                replacement.policy.alias_pricing,
            )
            .is_some_and(|previous| previous != replacement.policy.alias_pricing)
        {
            return Err(invalid_musubi_state(
                "musubi_registry_policy",
                "a pricing revision was reused for different alias prices",
            ));
        }
        reconstructed_policy = replacement.policy.clone();
    }
    if registry_policy.get() != &reconstructed_policy {
        return Err(invalid_musubi_state(
            "musubi_registry_policy",
            "current policy is not the exact result of retained Parliament decisions",
        ));
    }
    for (_, alias) in aliases.iter() {
        let pricing = pricing_policies
            .get(&alias.pricing_revision)
            .ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_aliases",
                    "alias references an unknown historical pricing revision",
                )
            })?;
        alias
            .validate(pricing)
            .map_err(|error| invalid_musubi_state("musubi_aliases", error.to_string()))?;
    }
    for (action, consumption) in actions_by_digest.values().copied() {
        let digest = action.action_digest();
        match action {
            iroha_data_model::musubi::MusubiParliamentActionV1::RecoverPackageOwners(recovery) => {
                let resulting_revision =
                    recovery.expected_revision.checked_add(1).ok_or_else(|| {
                        invalid_musubi_state(
                            "musubi_governance_decisions",
                            "owner-recovery governance result revision overflows",
                        )
                    })?;
                if !owner_recovery_revisions.insert((recovery.package.clone(), resulting_revision))
                {
                    return Err(invalid_musubi_state(
                        "musubi_governance_decisions",
                        "more than one owner recovery targets the same package result revision",
                    ));
                }
                owner_recovery_history
                    .entry(recovery.package.clone())
                    .or_default()
                    .push((resulting_revision, consumption.consumed_at_height));
                let package = packages.get(&recovery.package).ok_or_else(|| {
                    invalid_musubi_state(
                        "musubi_governance_decisions",
                        "owner-recovery action references a missing package",
                    )
                })?;
                if package.revisions.governance < resulting_revision {
                    return Err(invalid_musubi_state(
                        "musubi_governance_decisions",
                        "owner-recovery action has no persisted governance revision effect",
                    ));
                }
                if package.revisions.governance == resulting_revision {
                    if package.owners.as_slice() != recovery.owners.as_slice() {
                        return Err(invalid_musubi_state(
                            "musubi_governance_decisions",
                            "current owner-recovery projection does not contain the exact enacted owners",
                        ));
                    }
                    for owner in &recovery.owners {
                        let key =
                            MusubiPackageMemberKeyV1::new(recovery.package.clone(), owner.clone());
                        let member = package_members.get(&key).ok_or_else(|| {
                            invalid_musubi_state(
                                "musubi_package_members",
                                "current recovered owner has no exact member projection",
                            )
                        })?;
                        if member.role != MusubiPackageRoleV1::Owner
                            || member.governance_revision != resulting_revision
                            || member.accepted_at_height != consumption.consumed_at_height
                        {
                            return Err(invalid_musubi_state(
                                "musubi_package_members",
                                "current owner-recovery projection disagrees with its decision consumption height",
                            ));
                        }
                    }
                }
            }
            iroha_data_model::musubi::MusubiParliamentActionV1::RetargetAlias(retarget) => {
                let revision = retarget.expected_revision.checked_add(1).ok_or_else(|| {
                    invalid_musubi_state(
                        "musubi_alias_history",
                        "alias-retarget revision overflows",
                    )
                })?;
                let key = MusubiAliasHistoryKeyV1::new(retarget.alias.clone(), revision);
                let entry = alias_history.get(&key).ok_or_else(|| {
                    invalid_musubi_state(
                        "musubi_alias_history",
                        "alias-retarget action has no exact persisted history entry",
                    )
                })?;
                if entry.target != retarget.target
                    || entry.governance_action != Some(digest)
                    || entry.finalized_height != consumption.consumed_at_height
                {
                    return Err(invalid_musubi_state(
                        "musubi_alias_history",
                        "alias-retarget history does not match the enacted action and decision consumption height",
                    ));
                }
            }
            iroha_data_model::musubi::MusubiParliamentActionV1::TakedownArtifact(takedown) => {
                let release = releases.get(&takedown.release).ok_or_else(|| {
                    invalid_musubi_state(
                        "musubi_releases",
                        "artifact-takedown action references a missing release",
                    )
                })?;
                let expected_revision = takedown
                    .expected_artifact_governance_revision
                    .checked_add(1);
                let iroha_data_model::musubi::MusubiArtifactGovernanceStateV1::TakenDown(persisted) =
                    &release.artifact_governance
                else {
                    return Err(invalid_musubi_state(
                        "musubi_releases",
                        "artifact-takedown action has no persisted takedown state",
                    ));
                };
                if persisted.action_digest != digest
                    || persisted.reason != takedown.reason
                    || persisted.applied_at_height != consumption.consumed_at_height
                    || expected_revision != Some(release.revisions.artifact_governance)
                {
                    return Err(invalid_musubi_state(
                        "musubi_releases",
                        "artifact-takedown state does not match the enacted action and decision consumption height",
                    ));
                }
            }
            iroha_data_model::musubi::MusubiParliamentActionV1::SetRegistryPolicy(_) => {}
        }
    }
    for (_, mut history) in owner_recovery_history {
        history.sort_by_key(|(revision, _)| *revision);
        let mut previous_consumed_at = None;
        for (_, consumed_at_height) in history {
            if previous_consumed_at.is_some_and(|previous| consumed_at_height < previous) {
                return Err(invalid_musubi_state(
                    "musubi_governance_decisions",
                    "owner-recovery decision consumption heights do not follow governance revision order",
                ));
            }
            previous_consumed_at = Some(consumed_at_height);
        }
    }
    Ok(())
}
fn validate_musubi_live_projections(world: &World) -> Result<(), json::Error> {
    let world = world.view();
    for (archive_id, archive) in world.musubi_archives().iter() {
        let mut active_locations = 0_usize;
        let mut healthy_providers = BTreeSet::new();
        let mut maximum_location_revision = 1_u64;
        for (key, location) in world
            .musubi_archive_locations()
            .iter()
            .filter(|(key, _)| key.archive_id == *archive_id)
        {
            maximum_location_revision = maximum_location_revision.max(location.revision);
            if location.state == MusubiArchiveLocationStateV1::Retired {
                continue;
            }
            if archive
                .location_ids
                .binary_search(&key.location_id)
                .is_err()
            {
                return Err(invalid_musubi_state(
                    "musubi_archive_locations",
                    "non-retired location is absent from its archive directory",
                ));
            }
            let current =
                crate::smartcontracts::isi::musubi::current_location_providers(location, &world);
            let current_count = current.as_ref().map_or(0, Vec::len);
            let expected_state = if current_count
                >= usize::from(iroha_data_model::musubi::MUSUBI_MIN_HEALTHY_REPLICAS_V1)
            {
                MusubiArchiveLocationStateV1::Healthy
            } else {
                MusubiArchiveLocationStateV1::Degraded
            };
            if location.state != expected_state {
                return Err(invalid_musubi_state(
                    "musubi_archive_locations",
                    "archive-location lifecycle state disagrees with current SoraFS evidence",
                ));
            }
            if let Some(providers) = current {
                active_locations = active_locations.checked_add(1).ok_or_else(|| {
                    invalid_musubi_state(
                        "musubi_archive_availability",
                        "active archive-location count overflows usize",
                    )
                })?;
                healthy_providers.extend(providers);
            }
        }
        if archive.location_revision != maximum_location_revision {
            return Err(invalid_musubi_state(
                "musubi_archives",
                "archive location revision is not the exact maximum retained location revision",
            ));
        }
        let active_locations = u8::try_from(active_locations).map_err(|_| {
            invalid_musubi_state(
                "musubi_archive_availability",
                "active archive-location count overflows u8",
            )
        })?;
        let healthy_replicas = u16::try_from(healthy_providers.len()).map_err(|_| {
            invalid_musubi_state(
                "musubi_archive_availability",
                "healthy provider count overflows u16",
            )
        })?;
        let expected_availability =
            if healthy_replicas >= iroha_data_model::musubi::MUSUBI_MIN_HEALTHY_REPLICAS_V1 {
                iroha_data_model::musubi::MusubiStorageAvailabilityV1::Selectable
            } else if active_locations > 0 && healthy_replicas > 0 {
                iroha_data_model::musubi::MusubiStorageAvailabilityV1::BelowQuorum
            } else {
                iroha_data_model::musubi::MusubiStorageAvailabilityV1::Unavailable
            };
        let projection = world
            .musubi_archive_availability()
            .get(archive_id)
            .ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_archive_availability",
                    "archive is missing its availability projection",
                )
            })?;
        if projection.active_locations != active_locations
            || projection.healthy_replicas != healthy_replicas
            || projection.availability != expected_availability
        {
            return Err(invalid_musubi_state(
                "musubi_archive_availability",
                "availability projection is not the exact result of current SoraFS evidence",
            ));
        }
    }
    for (_, row) in world.musubi_resolver_index().iter() {
        if row.index_revision < row.selection.storage.index_revision {
            return Err(invalid_musubi_state(
                "musubi_resolver_index",
                "resolver row predates its embedded availability projection",
            ));
        }
    }
    for (_, entry) in world.musubi_public_directory().iter() {
        let maximum_row_revision = world
            .musubi_resolver_index()
            .iter()
            .filter(|(release, _)| release.package == entry.package)
            .map(|(_, row)| row.index_revision)
            .max()
            .ok_or_else(|| {
                invalid_musubi_state(
                    "musubi_public_directory",
                    "directory package has no resolver rows",
                )
            })?;
        if entry.index_revision < maximum_row_revision {
            return Err(invalid_musubi_state(
                "musubi_public_directory",
                "directory entry predates its package resolver rows",
            ));
        }
    }
    Ok(())
}

fn invalid_global_beacon_persistence(message: impl Into<String>) -> json::Error {
    json::Error::InvalidField {
        field: "world.global_beacon".to_owned(),
        message: message.into(),
    }
}

fn validate_global_beacon_persistence(world: &World) -> Result<(), json::Error> {
    let dkg = world.global_beacon_dkg.view();
    let key_sessions = world.global_beacon_key_sessions.view();
    let active_sessions = world.global_beacon_active_session.view();
    let latest_pulses = world.global_beacon_latest_pulse.view();
    let pulses = world.global_beacon_pulses.view();

    for (session_id, snapshot) in dkg.iter() {
        snapshot.validate().map_err(|error| {
            invalid_global_beacon_persistence(format!(
                "invalid active DKG snapshot {}: {error}",
                hex::encode(session_id)
            ))
        })?;
        if session_id != &snapshot.session.session_id {
            return Err(invalid_global_beacon_persistence(
                "active DKG storage key differs from its embedded session id",
            ));
        }
        if key_sessions.get(session_id).is_some() {
            return Err(invalid_global_beacon_persistence(
                "one beacon session is both active DKG and finalized",
            ));
        }
    }

    for (key, _) in active_sessions.iter() {
        if *key != GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY {
            return Err(invalid_global_beacon_persistence(
                "active-session pointer uses a noncanonical singleton key",
            ));
        }
    }
    for (key, _) in latest_pulses.iter() {
        if *key != GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY {
            return Err(invalid_global_beacon_persistence(
                "latest-pulse link uses a noncanonical singleton key",
            ));
        }
    }
    let active_session = active_sessions
        .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY)
        .copied();
    for (session_id, record) in key_sessions.iter() {
        record.validate().map_err(|error| {
            invalid_global_beacon_persistence(format!(
                "invalid finalized key session {}: {error}",
                hex::encode(session_id)
            ))
        })?;
        if session_id != &record.session.session_id {
            return Err(invalid_global_beacon_persistence(
                "finalized key storage key differs from its embedded session id",
            ));
        }
        let lifecycle_is_live =
            record.activated_at_height.is_some() && record.retired_at_height.is_none();
        if lifecycle_is_live != (active_session == Some(*session_id)) {
            return Err(invalid_global_beacon_persistence(
                "active-session pointer and key lifecycle metadata disagree",
            ));
        }
    }
    if active_session.is_some_and(|session_id| key_sessions.get(&session_id).is_none()) {
        return Err(invalid_global_beacon_persistence(
            "active-session pointer references a missing finalized key",
        ));
    }

    let mut ordered_pulses = Vec::with_capacity(pulses.len());
    for (pulse_id, pulse) in pulses.iter() {
        let link = validate_persisted_global_threshold_beacon_pulse_v1(pulse).map_err(|error| {
            invalid_global_beacon_persistence(format!(
                "invalid finalized pulse {}: {error}",
                hex::encode(pulse_id)
            ))
        })?;
        if pulse_id != &pulse.pulse_id || pulse_id != &link.pulse_id {
            return Err(invalid_global_beacon_persistence(
                "pulse storage key differs from its canonical pulse id",
            ));
        }
        let key_session = key_sessions.get(&pulse.session_id).ok_or_else(|| {
            invalid_global_beacon_persistence("pulse references a missing finalized key session")
        })?;
        if !key_session.is_active_at(pulse.height)
            || pulse.network_id != key_session.session.network_id
            || pulse.roster_hash != key_session.session.roster_hash
            || pulse.transcript_hash != key_session.session.transcript_hash
        {
            return Err(invalid_global_beacon_persistence(
                "pulse is outside its key lifecycle or immutable session binding",
            ));
        }
        ordered_pulses.push(pulse);
    }
    ordered_pulses.sort_by_key(|pulse| (pulse.height, pulse.round, pulse.pulse_id));
    for pair in ordered_pulses.windows(2) {
        let previous = pair[0];
        let current = pair[1];
        if (current.height, current.round) <= (previous.height, previous.round) {
            return Err(invalid_global_beacon_persistence(
                "finalized pulse history is not strictly monotonic",
            ));
        }
    }

    let latest = latest_pulses
        .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY)
        .copied();
    match ordered_pulses.last() {
        Some(last) => {
            let expected = GlobalThresholdBeaconPulseLinkV1 {
                pulse_id: last.pulse_id,
                seed: last.seed,
                height: last.height,
                round: last.round,
            };
            if latest != Some(expected) {
                return Err(invalid_global_beacon_persistence(
                    "latest-pulse link does not name the history tail",
                ));
            }
        }
        None => {
            if let Some(origin) = latest {
                origin.validate_origin().map_err(|error| {
                    invalid_global_beacon_persistence(format!(
                        "invalid genesis beacon origin: {error}"
                    ))
                })?;
            }
        }
    }
    Ok(())
}

fn invalid_tle_ovn_persistence(field: &'static str, message: impl Into<String>) -> json::Error {
    json::Error::InvalidField {
        field: field.to_owned(),
        message: message.into(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PersistedTimedOvnPhaseV1 {
    Registered,
    RegistrationClosed,
    SurvivorsFrozen,
    Sealed,
    Released,
}

fn persisted_timed_ovn_phase_v1(lifecycle: &TimedOvnLifecycleStateV1) -> PersistedTimedOvnPhaseV1 {
    match lifecycle {
        TimedOvnLifecycleStateV1::Registered(_) => PersistedTimedOvnPhaseV1::Registered,
        TimedOvnLifecycleStateV1::RegistrationClosed(_) => {
            PersistedTimedOvnPhaseV1::RegistrationClosed
        }
        TimedOvnLifecycleStateV1::SurvivorsFrozen(_) => PersistedTimedOvnPhaseV1::SurvivorsFrozen,
        TimedOvnLifecycleStateV1::Sealed(_) => PersistedTimedOvnPhaseV1::Sealed,
        TimedOvnLifecycleStateV1::Released(_) => PersistedTimedOvnPhaseV1::Released,
    }
}

fn timed_ovn_phase_matches_ballot_status_v1(
    status: iroha_data_model::governance::types::BallotAttemptStatusV1,
    failure_kind: Option<iroha_data_model::governance::types::ParliamentBallotFailureKindV1>,
    phase: PersistedTimedOvnPhaseV1,
) -> bool {
    use iroha_data_model::governance::types::BallotAttemptStatusV1 as BallotStatus;
    use iroha_data_model::governance::types::ParliamentBallotFailureKindV1 as FailureKind;
    match (status, failure_kind) {
        (BallotStatus::Registration, None) => phase == PersistedTimedOvnPhaseV1::Registered,
        (BallotStatus::SurvivorFreeze, None) => {
            phase == PersistedTimedOvnPhaseV1::RegistrationClosed
        }
        (BallotStatus::TimedCommitment, None) => phase == PersistedTimedOvnPhaseV1::SurvivorsFrozen,
        (BallotStatus::AwaitingRelease | BallotStatus::Opening, None) => {
            phase == PersistedTimedOvnPhaseV1::Sealed
        }
        (BallotStatus::Finalized, None) => phase == PersistedTimedOvnPhaseV1::Released,
        (
            BallotStatus::NoResult | BallotStatus::Superseded,
            Some(FailureKind::RegistrationDeadlineExpired),
        ) => phase == PersistedTimedOvnPhaseV1::Registered,
        (
            BallotStatus::NoResult | BallotStatus::Superseded,
            Some(FailureKind::SurvivorDeadlineExpired),
        ) => phase == PersistedTimedOvnPhaseV1::RegistrationClosed,
        (
            BallotStatus::NoResult | BallotStatus::Superseded,
            Some(FailureKind::CommitmentDeadlineExpired),
        ) => phase == PersistedTimedOvnPhaseV1::SurvivorsFrozen,
        (
            BallotStatus::NoResult | BallotStatus::Superseded,
            Some(FailureKind::ReleasePulseUnavailable | FailureKind::OpeningDeadlineExpired),
        ) => phase == PersistedTimedOvnPhaseV1::Sealed,
        _ => false,
    }
}

fn validate_tle_ovn_persistence(world: &World) -> Result<(), json::Error> {
    let mut validated_key_sessions = BTreeMap::new();
    let parliament_attempts = world.parliament_attempts.view();
    let timed_ovn_evidence = world.timed_ovn_evidence.view();
    let finalized_beacon_heights = world
        .global_beacon_pulses
        .view()
        .iter()
        .map(|(_, pulse)| pulse.height)
        .collect::<BTreeSet<_>>();
    for (key_session_id, public_state) in world.tle_key_sessions.view().iter() {
        if key_session_id != &public_state.key_session_id {
            return Err(invalid_tle_ovn_persistence(
                "tle_key_sessions",
                "TLE key-session storage key differs from its embedded canonical id",
            ));
        }
        let validated = public_state.clone().validate().map_err(|error| {
            invalid_tle_ovn_persistence(
                "tle_key_sessions",
                format!("invalid persisted adaptive TLE key session {key_session_id}: {error}"),
            )
        })?;
        validated_key_sessions.insert(*key_session_id, validated);
    }
    let active_tle_sessions = world.tle_active_key_session.view();
    for (key, key_session_id) in active_tle_sessions.iter() {
        if *key != TLE_KEY_SESSION_SINGLETON_KEY {
            return Err(invalid_tle_ovn_persistence(
                "tle_active_key_session",
                "active TLE session pointer uses a noncanonical singleton key",
            ));
        }
        if !validated_key_sessions.contains_key(key_session_id) {
            return Err(invalid_tle_ovn_persistence(
                "tle_active_key_session",
                "active TLE session pointer references a missing or invalid public session",
            ));
        }
    }

    for (ballot_attempt_id, lifecycle) in timed_ovn_evidence.iter() {
        if ballot_attempt_id.as_bytes() != &lifecycle.ballot_attempt_id() {
            return Err(invalid_tle_ovn_persistence(
                "timed_ovn_evidence",
                "timed-OVN storage key differs from its embedded ballot-attempt id",
            ));
        }
        let key_session = validated_key_sessions
            .get(&lifecycle.tle_key_session_id())
            .ok_or_else(|| {
                invalid_tle_ovn_persistence(
                    "timed_ovn_evidence",
                    "timed-OVN lifecycle references a missing TLE key session",
                )
            })?;
        lifecycle.validate(key_session).map_err(|error| {
            invalid_tle_ovn_persistence(
                "timed_ovn_evidence",
                format!("invalid persisted timed-OVN lifecycle {ballot_attempt_id}: {error}"),
            )
        })?;

        let session = lifecycle.session();
        if session.parameter_hash != crate::governance::timed_ovn::timed_ovn_parameter_hash_v1() {
            return Err(invalid_tle_ovn_persistence(
                "timed_ovn_evidence",
                "timed-OVN lifecycle does not use the fixed v1 parameter profile",
            ));
        }
        let governance_attempt_id = iroha_data_model::governance::types::GovernanceAttemptId::new(
            session.governance_attempt_id,
        );
        let governance_attempt =
            parliament_attempts
                .get(&governance_attempt_id)
                .ok_or_else(|| {
                    invalid_tle_ovn_persistence(
                        "timed_ovn_evidence",
                        "timed-OVN lifecycle references a missing Parliament attempt",
                    )
                })?;
        if governance_attempt.proposal_content_id().as_bytes() != &session.proposal_content_id {
            return Err(invalid_tle_ovn_persistence(
                "timed_ovn_evidence",
                "timed-OVN proposal binding differs from its Parliament attempt",
            ));
        }
        let ballot = governance_attempt
            .ballot(ballot_attempt_id)
            .ok_or_else(|| {
                invalid_tle_ovn_persistence(
                    "timed_ovn_evidence",
                    "timed-OVN lifecycle references a missing Parliament ballot attempt",
                )
            })?;
        if ballot.attempt().body_instance_id.as_bytes() != &session.body_instance_id
            || ballot.release_height() != Some(lifecycle.target_finalized_height())
        {
            return Err(invalid_tle_ovn_persistence(
                "timed_ovn_evidence",
                "timed-OVN body or release-height binding differs from Parliament state",
            ));
        }
        let body = governance_attempt
            .body(&ballot.attempt().body_instance_id)
            .ok_or_else(|| {
                invalid_tle_ovn_persistence(
                    "timed_ovn_evidence",
                    "timed-OVN ballot references a missing sealed Parliament body",
                )
            })?;
        let eligible_participant_hashes = body
            .assignments()
            .iter()
            .filter(|assignment| {
                !body
                    .excluded_assignments()
                    .contains(&assignment.assignment_id)
            })
            .map(|assignment| {
                iroha_data_model::governance::types::parliament_ballot_participant_hash_v1(
                    *ballot_attempt_id,
                    &assignment.member,
                )
            })
            .collect::<BTreeSet<_>>();
        let registered_participant_hashes = lifecycle
            .validated_registration_participant_hashes(key_session)
            .map_err(|error| {
                invalid_tle_ovn_persistence(
                    "timed_ovn_evidence",
                    format!(
                        "could not rebuild authenticated Parliament participant roster: {error}"
                    ),
                )
            })?;
        let expected_registered_voters = u32::try_from(registered_participant_hashes.len()).ok();
        let registration_count_matches =
            if persisted_timed_ovn_phase_v1(lifecycle) == PersistedTimedOvnPhaseV1::Registered {
                ballot.registered_voters().is_none()
            } else {
                ballot.registered_voters() == expected_registered_voters
            };
        if registered_participant_hashes
            .iter()
            .any(|participant_hash| !eligible_participant_hashes.contains(participant_hash))
            || !registration_count_matches
        {
            return Err(invalid_tle_ovn_persistence(
                "timed_ovn_evidence",
                "timed-OVN registration corpus is not the authenticated nonexcluded body-member subset",
            ));
        }
    }

    let concurrent_casting_contexts = timed_ovn_evidence
        .iter()
        .filter(|(ballot_attempt_id, lifecycle)| {
            let phase = persisted_timed_ovn_phase_v1(lifecycle);
            if !matches!(
                phase,
                PersistedTimedOvnPhaseV1::Registered
                    | PersistedTimedOvnPhaseV1::RegistrationClosed
                    | PersistedTimedOvnPhaseV1::SurvivorsFrozen
            ) {
                return false;
            }
            let governance_attempt_id =
                iroha_data_model::governance::types::GovernanceAttemptId::new(
                    lifecycle.session().governance_attempt_id,
                );
            let Some(attempt) = parliament_attempts.get(&governance_attempt_id) else {
                return false;
            };
            if attempt.attempt().status
                != iroha_data_model::governance::types::GovernanceAttemptStatusV1::Active
            {
                return false;
            }
            let Some(ballot) = attempt.ballot(ballot_attempt_id) else {
                return false;
            };
            let Some(body) = attempt.body(&ballot.attempt().body_instance_id) else {
                return false;
            };
            body.instance().status
                == iroha_data_model::governance::types::BodyInstanceStatusV1::Balloting
                && attempt
                    .active_ballot_for_body(&ballot.attempt().body_instance_id)
                    .is_some_and(|active| active.attempt().id == **ballot_attempt_id)
                && timed_ovn_phase_matches_ballot_status_v1(
                    ballot.attempt().status,
                    ballot.failure_kind(),
                    phase,
                )
        })
        .count();
    let maximum_casting_contexts = usize::try_from(
        iroha_data_model::parliament_casting::MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1,
    )
    .expect("u32 casting-context bound fits usize");
    if concurrent_casting_contexts > maximum_casting_contexts {
        return Err(invalid_tle_ovn_persistence(
            "timed_ovn_evidence",
            "concurrent cast-capable timed-OVN contexts exceed the protocol maximum",
        ));
    }

    for (governance_attempt_id, governance_attempt) in parliament_attempts.iter() {
        for (ballot_attempt_id, ballot) in governance_attempt.ballot_attempts() {
            if ballot.failure_kind()
                == Some(
                    iroha_data_model::governance::types::ParliamentBallotFailureKindV1::ReleasePulseUnavailable,
                )
            {
                ballot.release_beacon_session_id().ok_or_else(|| {
                    invalid_tle_ovn_persistence(
                        "parliament_attempts",
                        "release-pulse failure is missing its committed beacon session",
                    )
                })?;
                let release_height = ballot.release_height().ok_or_else(|| {
                    invalid_tle_ovn_persistence(
                        "parliament_attempts",
                        "release-pulse failure is missing its committed height",
                    )
                })?;
                if finalized_beacon_heights.contains(&release_height) {
                    return Err(invalid_tle_ovn_persistence(
                        "parliament_attempts",
                        "release-pulse failure conflicts with an authoritative finalized pulse",
                    ));
                }
            }
            let lifecycle = timed_ovn_evidence.get(ballot_attempt_id).ok_or_else(|| {
                invalid_tle_ovn_persistence(
                    "timed_ovn_evidence",
                    "Parliament ballot attempt is missing its authoritative timed-OVN lifecycle",
                )
            })?;
            if lifecycle.session().governance_attempt_id != *governance_attempt_id.as_bytes() {
                return Err(invalid_tle_ovn_persistence(
                    "timed_ovn_evidence",
                    "timed-OVN lifecycle belongs to a different Parliament attempt",
                ));
            }
            if !timed_ovn_phase_matches_ballot_status_v1(
                ballot.attempt().status,
                ballot.failure_kind(),
                persisted_timed_ovn_phase_v1(lifecycle),
            ) {
                return Err(invalid_tle_ovn_persistence(
                    "timed_ovn_evidence",
                    "Parliament ballot phase disagrees with its timed-OVN lifecycle",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod timed_ovn_persistence_phase_tests {
    use super::*;
    use iroha_data_model::governance::types::{
        BallotAttemptStatusV1 as BallotStatus, ParliamentBallotFailureKindV1 as FailureKind,
    };

    #[test]
    fn ballot_status_accepts_only_its_authoritative_timed_ovn_phase() {
        use PersistedTimedOvnPhaseV1 as Phase;

        let phases = [
            Phase::Registered,
            Phase::RegistrationClosed,
            Phase::SurvivorsFrozen,
            Phase::Sealed,
            Phase::Released,
        ];
        let exact = [
            (BallotStatus::Registration, Phase::Registered),
            (BallotStatus::SurvivorFreeze, Phase::RegistrationClosed),
            (BallotStatus::TimedCommitment, Phase::SurvivorsFrozen),
            (BallotStatus::AwaitingRelease, Phase::Sealed),
            (BallotStatus::Opening, Phase::Sealed),
            (BallotStatus::Finalized, Phase::Released),
        ];
        for (status, expected) in exact {
            for phase in phases {
                assert_eq!(
                    timed_ovn_phase_matches_ballot_status_v1(status, None, phase),
                    phase == expected,
                    "status {status:?}, phase {phase:?}"
                );
            }
        }
        let terminal = [
            (FailureKind::RegistrationDeadlineExpired, Phase::Registered),
            (
                FailureKind::SurvivorDeadlineExpired,
                Phase::RegistrationClosed,
            ),
            (
                FailureKind::CommitmentDeadlineExpired,
                Phase::SurvivorsFrozen,
            ),
            (FailureKind::ReleasePulseUnavailable, Phase::Sealed),
            (FailureKind::OpeningDeadlineExpired, Phase::Sealed),
        ];
        for status in [BallotStatus::NoResult, BallotStatus::Superseded] {
            assert!(
                phases
                    .into_iter()
                    .all(|phase| !timed_ovn_phase_matches_ballot_status_v1(status, None, phase)),
                "terminal status without a derived failure must fail closed"
            );
            for (failure_kind, expected) in terminal {
                for phase in phases {
                    assert_eq!(
                        timed_ovn_phase_matches_ballot_status_v1(status, Some(failure_kind), phase,),
                        phase == expected,
                        "terminal status {status:?}, failure {failure_kind:?}, phase {phase:?}"
                    );
                }
            }
        }
        for status in [
            BallotStatus::Registration,
            BallotStatus::SurvivorFreeze,
            BallotStatus::TimedCommitment,
            BallotStatus::AwaitingRelease,
            BallotStatus::Opening,
            BallotStatus::Finalized,
        ] {
            for phase in phases {
                assert!(
                    !timed_ovn_phase_matches_ballot_status_v1(
                        status,
                        Some(FailureKind::OpeningDeadlineExpired),
                        phase,
                    ),
                    "nonterminal status with a failure kind must fail closed"
                );
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn parse_world(
    mut map: SnapshotJsonMap<'_>,
    ivm_seed: &IvmSeed<'_, World>,
) -> Result<World, json::Error> {
    if let Some(actual) = map.source_order.as_ref() {
        let expected = canonical_world_field_order();
        if let Some(unknown) = actual.iter().find(|key| !expected.contains(key)) {
            return Err(json::Error::InvalidField {
                field: format!("world.{unknown}"),
                message: "unknown field is not permitted in a signed first-release snapshot"
                    .to_owned(),
            });
        }
        if actual != expected {
            return Err(json::Error::InvalidField {
                field: "world".to_owned(),
                message: "snapshot world fields are not in canonical schema order".to_owned(),
            });
        }
    }
    let parameters = take_parameters_cell(&mut map, "parameters")?;
    let peers: Cell<Peers> = take_required(&mut map, "peers")?;
    let domain_committees = take_required(&mut map, "domain_committees")?;
    let domain_endorsement_policies = take_required(&mut map, "domain_endorsement_policies")?;
    let domain_endorsements = take_required(&mut map, "domain_endorsements")?;
    let domains: Storage<DomainId, Domain> = take_required(&mut map, "domains")?;
    let accounts: Storage<AccountId, AccountValue> = take_required(&mut map, "accounts")?;
    let account_aliases = take_optional_default(&mut map, "account_aliases")?;
    let account_aliases_by_account = take_optional_default(&mut map, "account_aliases_by_account")?;
    let account_scope_directory = take_optional_default(&mut map, "account_scope_directory")?;
    let ram_lfe_program_policies = take_ram_lfe_program_policies(&mut map)?;
    validate_ram_lfe_program_policies(&ram_lfe_program_policies)?;
    let identifier_policies = take_optional_default(&mut map, "identifier_policies")?;
    let fee_sponsor_programs = take_optional_default(&mut map, "fee_sponsor_programs")?;
    let fee_sponsor_program_revisions =
        take_optional_default(&mut map, "fee_sponsor_program_revisions")?;
    let fee_sponsor_enrollments = take_optional_default(&mut map, "fee_sponsor_enrollments")?;
    let fee_sponsor_vaults = take_optional_default(&mut map, "fee_sponsor_vaults")?;
    let fee_sponsor_budget_counters =
        take_optional_default(&mut map, "fee_sponsor_budget_counters")?;
    let identifier_claims = take_optional_default(&mut map, "identifier_claims")?;
    let account_recovery_policies = take_optional_default(&mut map, "account_recovery_policies")?;
    let account_recovery_requests = take_optional_default(&mut map, "account_recovery_requests")?;
    let asset_definitions: Storage<AssetDefinitionId, AssetDefinition> =
        take_required(&mut map, "asset_definitions")?;
    let asset_definition_alias_bindings =
        take_optional_default(&mut map, "asset_definition_alias_bindings")?;
    let contract_alias_bindings = take_optional_default(&mut map, "contract_alias_bindings")?;
    let assets: Storage<AssetId, AssetValue> = take_required(&mut map, "assets")?;
    let account_rekey_records = take_optional_default(&mut map, "account_rekey_records")?;
    let asset_metadata = take_optional_default(&mut map, "asset_metadata")?;
    let nfts: Storage<NftId, NftValue> = take_required(&mut map, "nfts")?;
    let rwas: Storage<RwaId, RwaValue> = take_optional_default(&mut map, "rwas")?;
    let roles: Storage<RoleId, Role> = take_required(&mut map, "roles")?;
    let account_permissions: Storage<AccountId, Permissions> =
        take_required(&mut map, "account_permissions")?;
    let account_roles: Storage<RoleIdWithOwner, ()> = take_required(&mut map, "account_roles")?;
    let oracle_feeds = take_optional_default(&mut map, "oracle_feeds")?;
    let oracle_observations = take_optional_default(&mut map, "oracle_observations")?;
    let oracle_history = take_optional_default(&mut map, "oracle_history")?;
    let oracle_provider_stats = take_optional_default(&mut map, "oracle_provider_stats")?;
    let oracle_disputes = take_optional_default(&mut map, "oracle_disputes")?;
    let oracle_changes = take_optional_default(&mut map, "oracle_changes")?;
    let defi_oracle_attestations = take_optional_default(&mut map, "defi_oracle_attestations")?;
    let twitter_bindings = take_optional_default(&mut map, "twitter_bindings")?;
    let twitter_bindings_by_uaid = take_optional_default(&mut map, "twitter_bindings_by_uaid")?;
    let viral_reward_budget = take_required(&mut map, "viral_reward_budget")?;
    let viral_campaign_budget = take_required(&mut map, "viral_campaign_budget")?;
    let viral_daily_counters = take_required(&mut map, "viral_daily_counters")?;
    let viral_binding_claims = take_required(&mut map, "viral_binding_claims")?;
    let viral_escrows = take_required(&mut map, "viral_escrows")?;
    let viral_bonus_paid = take_required(&mut map, "viral_bonus_paid")?;
    let axt_policies: Storage<DataSpaceId, AxtPolicyEntry> =
        take_required(&mut map, "axt_policies")?;
    let axt_handle_counters: Storage<DataSpaceId, AxtHandleCounterRecord> =
        take_required(&mut map, "axt_handle_counters")?;
    {
        let counters = axt_handle_counters.view();
        for (_, record) in counters.iter() {
            record
                .validate()
                .map_err(|error| json::Error::InvalidField {
                    field: "world.axt_handle_counters".to_owned(),
                    message: error.to_string(),
                })?;
        }
    }
    let axt_asset_incarnations: Storage<AssetDefinitionId, AxtAssetIncarnationV1> =
        take_required(&mut map, "axt_asset_incarnations")?;
    {
        let definitions = asset_definitions.view();
        let incarnations = axt_asset_incarnations.view();
        for (asset_definition_id, incarnation) in incarnations.iter() {
            incarnation
                .validate()
                .map_err(|error| json::Error::InvalidField {
                    field: "world.axt_asset_incarnations".to_owned(),
                    message: error.to_string(),
                })?;
            if definitions.get(asset_definition_id).is_none() {
                return Err(json::Error::InvalidField {
                    field: "world.axt_asset_incarnations".to_owned(),
                    message: format!(
                        "AXT incarnation references unregistered asset definition {asset_definition_id}"
                    ),
                });
            }
        }
        for (asset_definition_id, _) in definitions.iter() {
            if incarnations.get(asset_definition_id).is_none() {
                return Err(json::Error::InvalidField {
                    field: "world.axt_asset_incarnations".to_owned(),
                    message: format!(
                        "registered asset definition {asset_definition_id} has no AXT incarnation"
                    ),
                });
            }
        }
    }
    let axt_replay_ledger: Storage<AxtHandleReplayKey, AxtReplayRecord> =
        take_required(&mut map, "axt_replay_ledger")?;
    let axt_handle_budget_ledger: Storage<AxtHandleBudgetKey, AxtHandleBudgetRecord> =
        take_required(&mut map, "axt_handle_budget_ledger")?;
    {
        let ledger = axt_handle_budget_ledger.view();
        for (key, record) in ledger.iter() {
            record
                .validate_for_key(key)
                .map_err(|error| json::Error::InvalidField {
                    field: "world.axt_handle_budget_ledger".to_owned(),
                    message: error.to_string(),
                })?;
        }
    }
    {
        let replay_ledger = axt_replay_ledger.view();
        let budget_ledger = axt_handle_budget_ledger.view();
        for (key, record) in replay_ledger.iter() {
            record
                .validate_for_key(key)
                .map_err(|error| json::Error::InvalidField {
                    field: "world.axt_replay_ledger".to_owned(),
                    message: error.to_string(),
                })?;
            let budget_record =
                budget_ledger
                    .get(&record.budget_key)
                    .ok_or_else(|| json::Error::InvalidField {
                        field: "world.axt_replay_ledger".to_owned(),
                        message: "AXT replay record references a missing permanent budget family"
                            .to_owned(),
                    })?;
            budget_record
                .validate_for_key(&record.budget_key)
                .map_err(|error| json::Error::InvalidField {
                    field: "world.axt_replay_ledger".to_owned(),
                    message: format!(
                        "AXT replay record references invalid budget evidence: {error}"
                    ),
                })?;
        }
    }
    {
        let policies = axt_policies.view();
        let counters = axt_handle_counters.view();
        for (dataspace, policy) in policies.iter() {
            let counter = counters.get(dataspace);
            if policy.manifest_root != [0; 32] && counter.is_none() {
                return Err(json::Error::InvalidField {
                    field: "world.axt_handle_counters".to_owned(),
                    message: format!(
                        "active AXT policy for dataspace {} has no permanent counter ratchet",
                        dataspace.as_u64()
                    ),
                });
            }
            let expected = counter.map_or(0, AxtHandleCounterRecord::next);
            if policy.next_handle_counter != expected {
                return Err(json::Error::InvalidField {
                    field: "world.axt_handle_counters".to_owned(),
                    message: format!(
                        "active AXT policy counter {} for dataspace {} disagrees with permanent ratchet {}",
                        policy.next_handle_counter,
                        dataspace.as_u64(),
                        expected
                    ),
                });
            }
            let expected_generation =
                counter.map_or(0, AxtHandleCounterRecord::authorization_generation);
            if policy.active_handle_era != expected_generation {
                return Err(json::Error::InvalidField {
                    field: "world.axt_handle_counters".to_owned(),
                    message: format!(
                        "AXT policy generation {} for dataspace {} disagrees with permanent ratchet {}",
                        policy.active_handle_era,
                        dataspace.as_u64(),
                        expected_generation
                    ),
                });
            }
        }
        let replay = axt_replay_ledger.view();
        for (key, _) in replay.iter() {
            let counter =
                counters
                    .get(&key.asset_dsid)
                    .ok_or_else(|| json::Error::InvalidField {
                        field: "world.axt_handle_counters".to_owned(),
                        message: format!(
                            "AXT replay key for dataspace {} has no permanent counter ratchet",
                            key.asset_dsid.as_u64()
                        ),
                    })?;
            if counter.next() <= key.sub_nonce {
                return Err(json::Error::InvalidField {
                    field: "world.axt_handle_counters".to_owned(),
                    message: format!(
                        "AXT replay key sub-nonce {} is not below permanent ratchet {}",
                        key.sub_nonce,
                        counter.next()
                    ),
                });
            }
            if counter.authorization_generation() < key.handle_era {
                return Err(json::Error::InvalidField {
                    field: "world.axt_handle_counters".to_owned(),
                    message: format!(
                        "AXT replay-key generation {} exceeds permanent ratchet generation {}",
                        key.handle_era,
                        counter.authorization_generation()
                    ),
                });
            }
        }
        let budgets = axt_handle_budget_ledger.view();
        for (key, _) in budgets.iter() {
            let counter =
                counters
                    .get(&key.asset_dsid())
                    .ok_or_else(|| json::Error::InvalidField {
                        field: "world.axt_handle_counters".to_owned(),
                        message: format!(
                            "AXT budget family for dataspace {} has no permanent counter ratchet",
                            key.asset_dsid().as_u64()
                        ),
                    })?;
            if counter.next() <= AxtHandleCounterRecord::initial(0).next() {
                return Err(json::Error::InvalidField {
                    field: "world.axt_handle_counters".to_owned(),
                    message: format!(
                        "AXT budget family for dataspace {} has an unadvanced permanent ratchet",
                        key.asset_dsid().as_u64()
                    ),
                });
            }
            if key.authorization_generation() > counter.authorization_generation() {
                return Err(json::Error::InvalidField {
                    field: "world.axt_handle_counters".to_owned(),
                    message: format!(
                        "AXT budget-family generation {} exceeds permanent ratchet generation {}",
                        key.authorization_generation(),
                        counter.authorization_generation()
                    ),
                });
            }
        }
    }
    map.get("sccp_registry")
        .ok_or_else(|| json::Error::missing_field("sccp_registry"))?
        .validate_sccp_registry()?;
    let sccp_registry: Cell<iroha_data_model::bridge::SccpRegistryV1> =
        take_required(&mut map, "sccp_registry")?;
    let sccp_outbound_pending_usage = take_required(&mut map, "sccp_outbound_pending_usage")?;
    let sccp_outbound_pending_messages = take_required(&mut map, "sccp_outbound_pending_messages")?;
    let sccp_outbound_message_locator = take_required(&mut map, "sccp_outbound_message_locator")?;
    let sccp_outbound_message_index = take_required(&mut map, "sccp_outbound_message_index")?;
    let sccp_outbound_proofs = take_required(&mut map, "sccp_outbound_proofs")?;
    let sccp_inbound_messages = take_required(&mut map, "sccp_inbound_messages")?;
    let sccp_inbound_anchor_high_water: Storage<SccpInboundAnchorHighWaterKeyV1, u64> =
        take_required(&mut map, "sccp_inbound_anchor_high_water")?;
    validate_sccp_outbound_pending_messages(&sccp_outbound_pending_messages)?;
    validate_sccp_outbound_pending_usage(
        &sccp_outbound_pending_messages,
        &sccp_outbound_pending_usage,
    )?;
    validate_sccp_outbound_indexes(
        &sccp_outbound_pending_messages,
        &sccp_outbound_proofs,
        &sccp_outbound_message_locator,
        &sccp_outbound_message_index,
    )?;
    validate_sccp_outbound_proofs(&sccp_outbound_proofs, &sccp_outbound_message_locator)?;
    validate_sccp_inbound_messages(&sccp_inbound_messages)?;
    let sccp_inbound_messages_view = sccp_inbound_messages.view();
    let sccp_inbound_anchor_high_water_view = sccp_inbound_anchor_high_water.view();
    validate_sccp_inbound_anchor_high_water_index(
        &sccp_inbound_messages_view,
        &sccp_inbound_anchor_high_water_view,
    )
    .map_err(|message| json::Error::InvalidField {
        field: "world.sccp_inbound_anchor_high_water".to_owned(),
        message,
    })?;
    let tx_sequences: Storage<AccountId, u64> = take_optional_default(&mut map, "tx_sequences")?;
    let triggers_value = map
        .remove("triggers")
        .ok_or_else(|| json::Error::missing_field("triggers"))?;
    let triggers = ivm_seed
        .cast::<TriggerSet>()
        .parse_trigger_set(triggers_value)?;
    let executor: Cell<Executor> = take_required(&mut map, "executor")?;
    let executor_data_model: Cell<ExecutorDataModel> =
        take_required(&mut map, "executor_data_model")?;
    let external_event_buf: Cell<Vec<EventBox>> = take_required(&mut map, "external_event_buf")?;
    let verifying_keys = take_optional_default(&mut map, "verifying_keys")?;
    let verifying_keys_by_circuit = take_optional_default(&mut map, "verifying_keys_by_circuit")?;
    let consensus_keys = take_optional_default(&mut map, "consensus_keys")?;
    let consensus_keys_by_pk = take_optional_default(&mut map, "consensus_keys_by_pk")?;
    let pedersen_params = take_optional_default(&mut map, "pedersen_params")?;
    let poseidon_params = take_optional_default(&mut map, "poseidon_params")?;
    let runtime_upgrades = take_optional_default(&mut map, "runtime_upgrades")?;
    let privacy_consensus_policy: Cell<iroha_data_model::privacy::PrivacyConsensusPolicyV1> =
        take_required(&mut map, "privacy_consensus_policy")?;
    let privacy_activations: Storage<
        crate::privacy_state::PrivacyActivationKeyV1,
        iroha_data_model::privacy::PrivacyProtocolActivationRecordV1,
    > = take_required(&mut map, "privacy_activations")?;
    let privacy_pgc_accounts: Storage<
        crate::privacy_state::PrivacyPgcAccountKeyV1,
        crate::privacy_state::PrivacyPgcAccountStateV1,
    > = take_required(&mut map, "privacy_pgc_accounts")?;
    let privacy_pgc_pool_invariants: Storage<
        crate::privacy_state::PrivacyPgcPoolInvariantKeyV1,
        crate::privacy_state::PrivacyPgcPoolInvariantV1,
    > = take_required(&mut map, "privacy_pgc_pool_invariants")?;
    let privacy_nullifiers: Storage<
        crate::privacy_state::PrivacyNullifierKeyV1,
        crate::privacy_state::PrivacyStateItemRecordV1,
    > = take_required(&mut map, "privacy_nullifiers")?;
    let privacy_commitments: Storage<
        crate::privacy_state::PrivacyCommitmentKeyV1,
        crate::privacy_state::PrivacyStateItemRecordV1,
    > = take_required(&mut map, "privacy_commitments")?;
    let privacy_roots: Storage<
        crate::privacy_state::PrivacyRootKeyV1,
        crate::privacy_state::PrivacyRootProvenanceV1,
    > = take_required(&mut map, "privacy_roots")?;
    let privacy_root_heads: Storage<
        crate::privacy_state::PrivacyRootHeadKeyV1,
        crate::privacy_state::PrivacyRootHeadRecordV1,
    > = take_required(&mut map, "privacy_root_heads")?;
    crate::privacy_state::validate_privacy_persisted_state_v1(
        privacy_consensus_policy.view().get(),
        &privacy_activations.view(),
        &privacy_pgc_accounts.view(),
        &privacy_pgc_pool_invariants.view(),
        &privacy_nullifiers.view(),
        &privacy_commitments.view(),
        &privacy_roots.view(),
        &privacy_root_heads.view(),
    )
    .map_err(|message| json::Error::InvalidField {
        field: "world.privacy_state".to_owned(),
        message,
    })?;
    crate::privacy_state::validate_privacy_orchard_public_dependencies_v1(
        &privacy_commitments.view(),
        &accounts.view(),
        &asset_definitions.view(),
    )
    .map_err(|message| json::Error::InvalidField {
        field: "world.privacy_state".to_owned(),
        message,
    })?;
    let proofs = take_optional_default(&mut map, "proofs")?;
    let proof_tags = take_optional_default(&mut map, "proof_tags")?;
    let proofs_by_tag = take_optional_default(&mut map, "proofs_by_tag")?;
    let contract_manifests = take_optional_default(&mut map, "contract_manifests")?;
    let contract_code = take_optional_default(&mut map, "contract_code")?;
    let contract_code_uploads = take_required(&mut map, "contract_code_uploads")?;
    let contract_code_upload_chunks = take_required(&mut map, "contract_code_upload_chunks")?;
    let contract_instances = take_optional_default(&mut map, "contract_instances")?;
    let contract_subject_bindings = take_optional_default(&mut map, "contract_subject_bindings")?;
    let smart_contract_state = take_optional_default(&mut map, "smart_contract_state")?;
    reject_legacy_musubi_state(&smart_contract_state)?;
    let musubi_namespace_bindings = take_musubi_namespace_bindings(&mut map)?;
    let musubi_domain_ownership_generations = take_musubi_domain_ownership_generations(&mut map)?;
    let musubi_packages = take_required(&mut map, "musubi_packages")?;
    let musubi_package_metadata = take_required(&mut map, "musubi_package_metadata")?;
    let musubi_package_members = take_required(&mut map, "musubi_package_members")?;
    let musubi_package_invitations = take_required(&mut map, "musubi_package_invitations")?;
    let musubi_maintainer_directory = take_required(&mut map, "musubi_maintainer_directory")?;
    let musubi_releases = take_required(&mut map, "musubi_releases")?;
    let musubi_archives = take_required(&mut map, "musubi_archives")?;
    let musubi_provider_bundle_attestations =
        take_required(&mut map, "musubi_provider_bundle_attestations")?;
    let musubi_archive_locations = take_required(&mut map, "musubi_archive_locations")?;
    let musubi_locations_by_pin = take_required(&mut map, "musubi_locations_by_pin")?;
    let musubi_locations_by_replication_order =
        take_required(&mut map, "musubi_locations_by_replication_order")?;
    let musubi_locations_by_provider = take_required(&mut map, "musubi_locations_by_provider")?;
    let musubi_archive_availability = take_required(&mut map, "musubi_archive_availability")?;
    let musubi_archive_reverse_references =
        take_required(&mut map, "musubi_archive_reverse_references")?;
    let musubi_resolver_index = take_required(&mut map, "musubi_resolver_index")?;
    let musubi_resolver_index_checkpoints = take_musubi_resolver_index_checkpoints(&mut map)?;
    let musubi_public_directory = take_required(&mut map, "musubi_public_directory")?;
    let musubi_aliases = take_required(&mut map, "musubi_aliases")?;
    let musubi_alias_history = take_required(&mut map, "musubi_alias_history")?;
    let musubi_governance_decisions = take_required(&mut map, "musubi_governance_decisions")?;
    let musubi_registry_policy = take_musubi_registry_policy(&mut map)?;
    let musubi_resolver_index_revision = take_musubi_resolver_index_revision(&mut map)?;
    let musubi_replication_shortfall_releases =
        take_musubi_replication_shortfall_releases(&mut map)?;
    let soracloud_service_revisions = take_required(&mut map, "soracloud_service_revisions")?;
    let soracloud_service_deployments = take_required(&mut map, "soracloud_service_deployments")?;
    let soracloud_app_infra_states = take_required(&mut map, "soracloud_app_infra_states")?;
    let soracloud_service_runtime = take_required(&mut map, "soracloud_service_runtime")?;
    let soracloud_inrou_replica_runtime =
        take_required(&mut map, "soracloud_inrou_replica_runtime")?;
    let soracloud_service_audit_events = take_required(&mut map, "soracloud_service_audit_events")?;
    let soracloud_app_infra_audit_events =
        take_required(&mut map, "soracloud_app_infra_audit_events")?;
    let soracloud_service_state_entries =
        take_required(&mut map, "soracloud_service_state_entries")?;
    let soracloud_decryption_request_records =
        take_optional_default(&mut map, "soracloud_decryption_request_records")?;
    let soracloud_agent_apartments = take_optional_default(&mut map, "soracloud_agent_apartments")?;
    let soracloud_agent_apartment_audit_events =
        take_required(&mut map, "soracloud_agent_apartment_audit_events")?;
    let soracloud_training_jobs = take_optional_default(&mut map, "soracloud_training_jobs")?;
    let soracloud_training_job_audit_events =
        take_required(&mut map, "soracloud_training_job_audit_events")?;
    let soracloud_model_registries = take_optional_default(&mut map, "soracloud_model_registries")?;
    let soracloud_model_weight_versions =
        take_optional_default(&mut map, "soracloud_model_weight_versions")?;
    let soracloud_model_weight_audit_events =
        take_required(&mut map, "soracloud_model_weight_audit_events")?;
    let soracloud_model_artifacts = take_optional_default(&mut map, "soracloud_model_artifacts")?;
    let soracloud_model_artifact_audit_events =
        take_required(&mut map, "soracloud_model_artifact_audit_events")?;
    let soracloud_uploaded_model_bundles =
        take_required(&mut map, "soracloud_uploaded_model_bundles")?;
    let soracloud_model_host_capabilities =
        take_optional_default(&mut map, "soracloud_model_host_capabilities")?;
    let soracloud_inrou_host_capabilities =
        take_required(&mut map, "soracloud_inrou_host_capabilities")?;
    let soracloud_hf_sources = take_optional_default(&mut map, "soracloud_hf_sources")?;
    let soracloud_hf_shared_lease_pools =
        take_optional_default(&mut map, "soracloud_hf_shared_lease_pools")?;
    let soracloud_hf_shared_lease_members =
        take_optional_default(&mut map, "soracloud_hf_shared_lease_members")?;
    let soracloud_hf_shared_lease_audit_events =
        take_required(&mut map, "soracloud_hf_shared_lease_audit_events")?;
    let soracloud_model_host_violation_evidence =
        take_required(&mut map, "soracloud_model_host_violation_evidence")?;
    let soracloud_hf_placements = take_optional_default(&mut map, "soracloud_hf_placements")?;
    let soracloud_inrou_service_placements =
        take_required(&mut map, "soracloud_inrou_service_placements")?;
    let soracloud_mailbox_messages = take_required(&mut map, "soracloud_mailbox_messages")?;
    let soracloud_runtime_receipts = take_required(&mut map, "soracloud_runtime_receipts")?;
    let soracloud_private_uploaded_model_execution_receipts =
        take_required(&mut map, "soracloud_private_uploaded_model_execution_receipts")?;
    SoracloudInrouPersistedStateV1 {
        service_revisions: &soracloud_service_revisions,
        service_deployments: &soracloud_service_deployments,
        app_infra_states: &soracloud_app_infra_states,
        service_runtime: &soracloud_service_runtime,
        inrou_replica_runtime: &soracloud_inrou_replica_runtime,
        service_audit_events: &soracloud_service_audit_events,
        app_infra_audit_events: &soracloud_app_infra_audit_events,
        training_job_audit_events: &soracloud_training_job_audit_events,
        model_weight_audit_events: &soracloud_model_weight_audit_events,
        model_artifact_audit_events: &soracloud_model_artifact_audit_events,
        hf_shared_lease_audit_events: &soracloud_hf_shared_lease_audit_events,
        model_host_violation_evidence: &soracloud_model_host_violation_evidence,
        agent_apartment_audit_events: &soracloud_agent_apartment_audit_events,
        service_state_entries: &soracloud_service_state_entries,
        inrou_host_capabilities: &soracloud_inrou_host_capabilities,
        inrou_service_placements: &soracloud_inrou_service_placements,
        uploaded_model_bundles: &soracloud_uploaded_model_bundles,
        mailbox_messages: &soracloud_mailbox_messages,
        runtime_receipts: &soracloud_runtime_receipts,
        private_uploaded_model_execution_receipts:
            &soracloud_private_uploaded_model_execution_receipts,
    }
    .validate()?;
    let provider_owners = take_required(&mut map, "provider_owners")?;
    let provider_ingest_completion_authorities =
        take_required(&mut map, "provider_ingest_completion_authorities")?;
    validate_provider_ingest_completion_authorities(
        &provider_owners,
        &provider_ingest_completion_authorities,
    )?;
    let pin_manifests = take_optional_default(&mut map, "pin_manifests")?;
    let zk_assets = take_optional_default(&mut map, "zk_assets")?;
    let elections = take_required(&mut map, "elections")?;
    let citizens = take_required(&mut map, "citizens")?;
    let ministry_agenda_proposals = take_required(&mut map, "ministry_agenda_proposals")?;
    let governance_proposals: Storage<[u8; 32], GovernanceProposalRecord> =
        take_required(&mut map, "governance_proposals")?;
    let governance_referenda = take_required(&mut map, "governance_referenda")?;
    let governance_locks = take_required(&mut map, "governance_locks")?;
    let governance_slashes = take_required(&mut map, "governance_slashes")?;
    let governance_last_unlock_sweep_height =
        take_required(&mut map, "governance_last_unlock_sweep_height")?;
    let governance_unlock_stats = take_required(&mut map, "governance_unlock_stats")?;
    let council = take_required(&mut map, "council")?;
    let parliament_bodies = take_required(&mut map, "parliament_bodies")?;
    let parliament_attempts = take_required(&mut map, "parliament_attempts")?;
    let tle_key_sessions = take_required(&mut map, "tle_key_sessions")?;
    let tle_active_key_session = take_optional_default(&mut map, "tle_active_key_session")?;
    let timed_ovn_evidence = take_required(&mut map, "timed_ovn_evidence")?;
    let global_beacon_dkg = take_required(&mut map, "global_beacon_dkg")?;
    let global_beacon_key_sessions = take_required(&mut map, "global_beacon_key_sessions")?;
    let global_beacon_active_session = take_required(&mut map, "global_beacon_active_session")?;
    let global_beacon_latest_pulse = take_required(&mut map, "global_beacon_latest_pulse")?;
    let global_beacon_pulses = take_required(&mut map, "global_beacon_pulses")?;
    let vrf_epochs = take_required(&mut map, "vrf_epochs")?;
    let repo_agreements = take_optional_default(&mut map, "repo_agreements")?;
    let settlement_receipts = take_optional_default(&mut map, "settlement_receipts")?;
    let kagemusha_replay_keys = take_required(&mut map, "kagemusha_replay_keys")?;
    let direct_lane_block_application_markers =
        take_optional_default(&mut map, "direct_lane_block_application_markers")?;
    let lane_relay_emergency_validators =
        take_optional_default(&mut map, "lane_relay_emergency_validators")?;
    let manifest_aliases = take_optional_default(&mut map, "manifest_aliases")?;
    let replication_orders = take_optional_default(&mut map, "replication_orders")?;
    validate_musubi_location_reverse_indices(
        &musubi_archives,
        &musubi_archive_locations,
        &pin_manifests,
        &replication_orders,
        &musubi_locations_by_pin,
        &musubi_locations_by_replication_order,
        &musubi_locations_by_provider,
    )?;
    let content_bundles = take_optional_default(&mut map, "content_bundles")?;
    let content_chunks = take_optional_default(&mut map, "content_chunks")?;
    let asset_escrows = take_optional_default(&mut map, "asset_escrows")?;
    let vpn_leases = take_optional_default(&mut map, "vpn_leases")?;
    let merge_hint_roots: Cell<Vec<Hash>> = take_optional_default(&mut map, "merge_hint_roots")?;
    let merge_global_state_root: Cell<Option<Hash>> =
        take_optional_default(&mut map, "merge_global_state_root")?;
    reject_unknown(&map, "world")?;
    let mut world = World {
        parameters,
        peers,
        domains,
        domains_by_owner: Storage::default(),
        accounts,
        uaid_accounts: Storage::default(),
        account_aliases,
        account_aliases_by_account,
        account_scope_directory,
        account_scope_accounts: Storage::default(),
        opaque_uaids: Storage::default(),
        ram_lfe_program_policies,
        identifier_policies,
        fee_sponsor_programs,
        fee_sponsor_program_revisions,
        fee_sponsor_enrollments,
        fee_sponsor_vaults,
        fee_sponsor_budget_counters,
        identifier_claims,
        account_rekey_records,
        account_recovery_policies,
        account_recovery_requests,
        asset_definitions,
        asset_definition_aliases: Storage::default(),
        asset_definition_alias_bindings,
        contract_aliases: Storage::default(),
        contract_alias_bindings,
        asset_definition_domains: Storage::default(),
        domain_asset_definitions: Storage::default(),
        asset_definitions_by_owner: Storage::default(),
        asset_definition_holders: Storage::default(),
        asset_definition_assets: Storage::default(),
        assets_by_account: Storage::default(),
        assets_by_domain: Storage::default(),
        asset_definition_nonzero_holders: Storage::default(),
        assets,
        asset_metadata,
        nfts,
        nfts_by_owner: Storage::default(),
        nfts_by_domain: Storage::default(),
        rwas,
        rwas_by_owner: Storage::default(),
        rwas_by_status: Storage::default(),
        rwas_by_frozen: Storage::default(),
        roles,
        account_permissions,
        account_roles,
        oracle_feeds,
        oracle_observations,
        oracle_history,
        oracle_provider_stats,
        oracle_disputes,
        oracle_changes,
        defi_oracle_attestations,
        twitter_bindings,
        twitter_bindings_by_uaid,
        viral_reward_budget,
        viral_campaign_budget,
        viral_daily_counters,
        viral_binding_claims,
        viral_escrows,
        viral_bonus_paid,
        asset_escrows,
        asset_escrows_by_seller: Storage::default(),
        asset_escrows_by_buyer: Storage::default(),
        asset_escrows_by_status: Storage::default(),
        vpn_leases,
        vpn_active_lease_by_account: Storage::default(),
        vpn_active_lease_by_address_slot: Storage::default(),
        vpn_settled_leases_by_account: Storage::default(),
        uaid_dataspaces: Storage::default(),
        space_directory_manifests: Storage::default(),
        axt_policies,
        axt_handle_counters,
        axt_asset_incarnations,
        axt_replay_ledger,
        axt_handle_budget_ledger,
        sccp_registry,
        sccp_outbound_pending_usage,
        sccp_outbound_pending_messages,
        sccp_outbound_message_locator,
        sccp_outbound_message_index,
        sccp_outbound_proofs,
        sccp_inbound_messages,
        sccp_inbound_anchor_high_water,
        tx_sequences,
        triggers,
        executor,
        executor_data_model,
        verifying_keys,
        verifying_keys_by_circuit,
        consensus_keys,
        consensus_keys_by_pk,
        pedersen_params,
        poseidon_params,
        runtime_upgrades,
        privacy_consensus_policy,
        privacy_activations,
        privacy_pgc_accounts,
        privacy_pgc_pool_invariants,
        privacy_nullifiers,
        privacy_commitments,
        privacy_roots,
        privacy_root_heads,
        proofs,
        proofs_by_status: Storage::default(),
        proof_tags,
        proofs_by_tag,
        contract_manifests,
        contract_code,
        contract_code_uploads,
        contract_code_upload_chunks,
        contract_instances,
        contract_subject_bindings,
        contract_subject_addresses: Storage::default(),
        smart_contract_state,
        musubi_namespace_bindings,
        musubi_domain_ownership_generations,
        musubi_packages,
        musubi_package_metadata,
        musubi_package_members,
        musubi_package_invitations,
        musubi_maintainer_directory,
        musubi_releases,
        musubi_archives,
        musubi_provider_bundle_attestations,
        musubi_archive_locations,
        musubi_locations_by_pin,
        musubi_locations_by_replication_order,
        musubi_locations_by_provider,
        musubi_archive_availability,
        musubi_archive_reverse_references,
        musubi_resolver_index,
        musubi_resolver_index_checkpoints,
        musubi_public_directory,
        musubi_aliases,
        musubi_alias_history,
        musubi_governance_decisions,
        musubi_registry_policy,
        musubi_resolver_index_revision,
        musubi_replication_shortfall_releases,
        soracloud_service_revisions,
        soracloud_service_deployments,
        soracloud_app_infra_states,
        soracloud_service_runtime,
        soracloud_inrou_replica_runtime,
        soracloud_service_audit_events,
        soracloud_app_infra_audit_events,
        soracloud_service_state_entries,
        soracloud_decryption_request_records,
        soracloud_agent_apartments,
        soracloud_agent_apartment_audit_events,
        soracloud_training_jobs,
        soracloud_training_job_audit_events,
        soracloud_model_registries,
        soracloud_model_weight_versions,
        soracloud_model_weight_audit_events,
        soracloud_model_artifacts,
        soracloud_model_artifact_audit_events,
        soracloud_uploaded_model_bundles,
        soracloud_model_host_capabilities,
        soracloud_inrou_host_capabilities,
        soracloud_hf_sources,
        soracloud_hf_shared_lease_pools,
        soracloud_hf_shared_lease_members,
        soracloud_hf_shared_lease_audit_events,
        soracloud_model_host_violation_evidence,
        soracloud_hf_placements,
        soracloud_inrou_service_placements,
        soracloud_mailbox_messages,
        soracloud_runtime_receipts,
        soracloud_private_uploaded_model_execution_receipts,
        capacity_declarations: Storage::default(),
        capacity_fee_ledger: Storage::default(),
        capacity_disputes: Storage::default(),
        provider_owners,
        provider_ingest_completion_authorities,
        da_pin_intents_by_ticket: Storage::default(),
        da_pin_intents_by_alias: Storage::default(),
        da_pin_intents_by_manifest: Storage::default(),
        da_pin_intents_by_lane_epoch: Storage::default(),
        sorafs_pricing: Cell::default(),
        provider_credit_ledger: Storage::default(),
        pin_manifests,
        manifest_aliases,
        replication_orders,
        content_bundles,
        content_chunks,
        soradns_directory_records: Storage::default(),
        soradns_directory_pending: Storage::default(),
        soradns_directory_latest: Cell::default(),
        soradns_directory_history: Storage::default(),
        soradns_directory_prev_of: Storage::default(),
        soradns_directory_revocations: Storage::default(),
        soradns_release_signers: Storage::default(),
        soradns_rotation_policy: Cell::default(),
        soradns_last_publish_ms: Cell::default(),
        soradns_history_len: Cell::default(),
        repo_agreements,
        repo_agreements_by_initiator: Storage::default(),
        repo_agreements_by_counterparty: Storage::default(),
        repo_agreements_by_custodian: Storage::default(),
        settlement_receipts,
        kagemusha_replay_keys,
        direct_lane_block_application_markers,
        domain_committees,
        domain_endorsement_policies,
        domain_endorsements,
        domain_endorsements_by_domain: Storage::default(),
        public_lane_validators: Storage::default(),
        public_lane_stake_shares: Storage::default(),
        public_lane_rewards: Storage::default(),
        public_lane_reward_claims: Storage::default(),
        lane_relay_emergency_validators,
        zk_assets,
        confidential_policy_transition_index: Storage::default(),
        confidential_policy_transition_counts: Storage::default(),
        elections,
        citizens,
        ministry_agenda_proposals,
        governance_proposals,
        governance_referenda,
        governance_locks,
        governance_lock_expiry_index: Storage::default(),
        validation_fee_proposal_index: Storage::default(),
        governance_slashes,
        governance_last_unlock_sweep_height,
        governance_unlock_stats,
        council,
        parliament_bodies,
        parliament_attempts,
        tle_key_sessions,
        tle_active_key_session,
        timed_ovn_evidence,
        global_beacon_dkg,
        global_beacon_key_sessions,
        global_beacon_active_session,
        global_beacon_latest_pulse,
        global_beacon_pulses,
        vrf_epochs,
        merge_hint_roots,
        merge_global_state_root,
        consensus_evidence: Storage::default(),
        external_event_buf,
    };
    let parliament_attempts_view = world.parliament_attempts.view();
    let governance_proposals_view = world.governance_proposals.view();
    for (attempt_id, attempt) in parliament_attempts_view.iter() {
        if attempt_id != &attempt.attempt().id {
            return Err(json::Error::InvalidField {
                field: "parliament_attempts".into(),
                message: "Parliament attempt storage key differs from its embedded canonical id"
                    .into(),
            });
        }
        attempt
            .validate()
            .map_err(|error| json::Error::InvalidField {
                field: "parliament_attempts".into(),
                message: format!("invalid persisted Parliament attempt: {error}"),
            })?;
        let proposal_id = *attempt.proposal_content_id().as_bytes();
        let proposal = governance_proposals_view.get(&proposal_id).ok_or_else(|| {
            json::Error::InvalidField {
                field: "parliament_attempts".into(),
                message: "Parliament attempt references a missing exact governance proposal"
                    .to_owned(),
            }
        })?;
        attempt
            .validate_proposal_bindings_v1(&proposal.kind)
            .map_err(|error| json::Error::InvalidField {
                field: "parliament_attempts".into(),
                message: format!(
                    "Parliament attempt differs from its exact governance proposal policy: {error}"
                ),
            })?;
    }
    validate_tle_ovn_persistence(&world)?;
    validate_global_beacon_persistence(&world)?;
    for (proposal_id, proposal) in governance_proposals_view.iter() {
        if let Some(reason) = proposal.first_release_exact_json_u64_invariant_error() {
            return Err(json::Error::InvalidField {
                field: "governance_proposals".into(),
                message: reason.to_owned(),
            });
        }
        if proposal.kind.fingerprint() != *proposal_id {
            return Err(json::Error::InvalidField {
                field: "governance_proposals".into(),
                message: "governance proposal storage key differs from its exact typed fingerprint"
                    .to_owned(),
            });
        }
        let proposal_operator = match &proposal.kind {
            iroha_data_model::governance::types::ProposalKind::ValidationFeePolicy(payload) => {
                Some(&payload.proposal_operator)
            }
            iroha_data_model::governance::types::ProposalKind::ValidationFeePayoutLifecycle(
                payload,
            ) => Some(&payload.proposal_operator),
            _ => None,
        };
        if proposal_operator.is_some_and(|operator| operator != &proposal.proposer) {
            return Err(json::Error::InvalidField {
                field: "governance_proposals".into(),
                message: "validation-fee proposal operator differs from its retained proposer"
                    .to_owned(),
            });
        }
        let referendum_id = hex::encode(proposal_id);
        if world
            .governance_referenda
            .view()
            .get(&referendum_id)
            .is_some()
            || world.governance_locks.view().get(&referendum_id).is_some()
            || world
                .governance_slashes
                .view()
                .get(&referendum_id)
                .is_some()
            || world.elections.view().get(&referendum_id).is_some()
        {
            return Err(json::Error::InvalidField {
                field: "governance_proposals".into(),
                message:
                    "certificate-only governance proposals cannot retain legacy public referendum or pipeline state"
                        .to_owned(),
            });
        }
        let governed_subject =
            proposal
                .kind
                .governed_subject_id_v1()
                .map_err(|error| json::Error::InvalidField {
                    field: "governance_proposals".into(),
                    message: format!(
                        "failed to derive governance proposal's governed subject: {error}"
                    ),
                })?;
        let proposal_content_id =
            iroha_data_model::governance::types::ProposalContentId::new(*proposal_id);
        let mut proposal_attempts = parliament_attempts_view
            .iter()
            .filter(|(_, attempt)| attempt.proposal_content_id() == proposal_content_id)
            .map(|(_, attempt)| attempt)
            .collect::<Vec<_>>();
        proposal_attempts.sort_unstable_by_key(|attempt| attempt.attempt().sequence);
        for (index, attempt) in proposal_attempts.iter().enumerate() {
            let expected_sequence =
                u32::try_from(index).map_err(|_| json::Error::InvalidField {
                    field: "parliament_attempts".into(),
                    message:
                        "governance Parliament attempt history exceeds the u32 sequence domain"
                            .to_owned(),
                })?;
            if attempt.attempt().sequence != expected_sequence {
                return Err(json::Error::InvalidField {
                    field: "parliament_attempts".into(),
                    message:
                        "governance Parliament attempt history is not an exact contiguous sequence"
                            .to_owned(),
                });
            }
            if attempt.policy_version() != 1 {
                return Err(json::Error::InvalidField {
                    field: "parliament_attempts".into(),
                    message: "governance Parliament attempt has a non-V1 policy version".to_owned(),
                });
            }
            if let Some(certificate) = attempt.certificate() {
                let certificate_subject = match certificate.expected_head {
                    iroha_data_model::governance::types::GovernanceExpectedHeadV1::Absent(head) => {
                        head.subject_id
                    }
                    iroha_data_model::governance::types::GovernanceExpectedHeadV1::Present(
                        head,
                    ) => head.subject_id,
                };
                if certificate.proposal_content_id != proposal_content_id
                    || certificate.governance_attempt_sequence != expected_sequence
                    || certificate.effect_preimage_hash != proposal.kind.effect_preimage_hash_v1()
                    || certificate_subject != governed_subject
                {
                    return Err(json::Error::InvalidField {
                        field: "parliament_attempts".into(),
                        message: "Parliament certificate differs from its exact governance proposal effect"
                            .to_owned(),
                    });
                }
            }
            let is_latest = index + 1 == proposal_attempts.len();
            if !is_latest
                && !matches!(
                    attempt.attempt().status,
                    iroha_data_model::governance::types::GovernanceAttemptStatusV1::Rejected
                        | iroha_data_model::governance::types::GovernanceAttemptStatusV1::Superseded
                        | iroha_data_model::governance::types::GovernanceAttemptStatusV1::ExecutionFailed
                )
            {
                return Err(json::Error::InvalidField {
                    field: "parliament_attempts".into(),
                    message:
                        "a non-latest Parliament attempt is not a retryable terminal predecessor"
                            .to_owned(),
                });
            }
        }
        let latest_attempt = proposal_attempts.last().copied();
        let status_matches_attempt = proposal_status_matches_latest_attempt_v1(
            proposal.status,
            latest_attempt.map(|attempt| attempt.attempt().status),
        ) && latest_attempt.is_none_or(|attempt| {
            proposal.status != GovernanceProposalStatus::Enacted
                || attempt.certificate().is_some_and(|certificate| {
                    attempt.terminal_height() == Some(certificate.enact_at_height)
                })
        });
        if !status_matches_attempt {
            return Err(json::Error::InvalidField {
                field: "governance_proposals".into(),
                message:
                    "governance proposal status does not match its exact Parliament attempt outcome"
                        .to_owned(),
            });
        }
    }
    MusubiPersistedState {
        namespace_bindings: &world.musubi_namespace_bindings,
        packages: &world.musubi_packages,
        package_metadata: &world.musubi_package_metadata,
        package_members: &world.musubi_package_members,
        package_invitations: &world.musubi_package_invitations,
        maintainer_directory: &world.musubi_maintainer_directory,
        releases: &world.musubi_releases,
        archives: &world.musubi_archives,
        provider_bundle_attestations: &world.musubi_provider_bundle_attestations,
        archive_locations: &world.musubi_archive_locations,
        archive_availability: &world.musubi_archive_availability,
        archive_reverse_references: &world.musubi_archive_reverse_references,
        resolver_index: &world.musubi_resolver_index,
        resolver_index_checkpoints: &world.musubi_resolver_index_checkpoints,
        public_directory: &world.musubi_public_directory,
        aliases: &world.musubi_aliases,
        alias_history: &world.musubi_alias_history,
        governance_decisions: &world.musubi_governance_decisions,
        resolver_index_revision: world.musubi_resolver_index_revision.view().get().get(),
        replication_shortfall_releases: *world.musubi_replication_shortfall_releases.view().get(),
    }
    .validate()?;
    validate_musubi_governance_provenance(&world)?;
    validate_musubi_live_projections(&world)?;
    world
        .validate_numeric_asset_invariants()
        .map_err(|message| json::Error::InvalidField {
            field: "world.assets".into(),
            message,
        })?;
    world
        .validate_quantity_ledger_invariants()
        .map_err(|message| json::Error::InvalidField {
            field: "world.numeric_ledgers".into(),
            message,
        })?;
    world.rebuild_domain_owner_index();
    world
        .rebuild_uaid_account_index()
        .map_err(|message| json::Error::InvalidField {
            field: "uaid_accounts".into(),
            message,
        })?;
    world
        .rebuild_account_alias_index()
        .map_err(|message| json::Error::InvalidField {
            field: "account_aliases".into(),
            message,
        })?;
    world
        .rebuild_account_scope_directory()
        .map_err(|message| json::Error::InvalidField {
            field: "account_scope_directory".into(),
            message,
        })?;
    world
        .rebuild_account_rekey_records()
        .map_err(|message| json::Error::InvalidField {
            field: "account_rekey_records".into(),
            message,
        })?;
    world
        .rebuild_asset_definition_alias_indexes()
        .map_err(|message| json::Error::InvalidField {
            field: "asset_definition_alias_bindings".into(),
            message,
        })?;
    world
        .rebuild_contract_alias_indexes()
        .map_err(|message| json::Error::InvalidField {
            field: "contract_alias_bindings".into(),
            message,
        })?;
    world
        .rebuild_asset_definition_indexes()
        .map_err(|message| json::Error::InvalidField {
            field: "asset_definition_domains".into(),
            message,
        })?;
    world
        .rebuild_confidential_policy_transition_index()
        .map_err(|message| json::Error::InvalidField {
            field: "asset_definitions.confidential_policy.pending_transition".into(),
            message,
        })?;
    world.rebuild_governance_read_indexes();
    world.rebuild_nft_owner_index();
    world.rebuild_rwa_indexes();
    world.rebuild_escrow_indexes();
    world
        .rebuild_vpn_lease_indexes()
        .map_err(|message| json::Error::InvalidField {
            field: "vpn_leases".into(),
            message,
        })?;
    world.rebuild_repo_agreement_indexes();
    world.rebuild_proof_status_index();
    world
        .rebuild_opaque_uaid_index()
        .map_err(|message| json::Error::InvalidField {
            field: "opaque_uaids".into(),
            message,
        })?;
    world
        .validate_identifier_claims()
        .map_err(|message| json::Error::InvalidField {
            field: "identifier_claims".into(),
            message,
        })?;
    Ok(world)
}
struct BuildStateInputs {
    world: World,
    block_hashes: BlockHashes,
    transactions: TransactionsStorage,
    commit_topology: Cell<Vec<PeerId>>,
    prev_commit_topology: Cell<Vec<PeerId>>,
    ivm: IVM,
    nexus: iroha_config::parameters::actual::Nexus,
    lane_incarnations: BTreeMap<LaneId, Hash>,
    lane_incarnation_lineage: BTreeMap<LaneId, LaneIncarnationLineage>,
    lane_incarnation_activation_heights: BTreeMap<LaneId, u64>,
    autoscale_sample_history: VecDeque<AutoscaleSampleRecord>,
    chain_id: iroha_data_model::ChainId,
    network_id: iroha_data_model::NetworkId,
    snapshot_v2_bootstrap_candidate: Option<SnapshotV2BootstrapRecord>,
    nexus_runtime_restored_from_snapshot: bool,
    kura: Arc<Kura>,
    query_handle: LiveQueryStoreHandle,
    #[cfg(feature = "telemetry")]
    telemetry: StateTelemetry,
}
fn build_state(
    inputs: BuildStateInputs,
    allow_durable_recovery: bool,
) -> Result<State, MergeLedgerCommitError> {
    let BuildStateInputs {
        world,
        block_hashes,
        transactions,
        commit_topology,
        prev_commit_topology,
        ivm,
        nexus,
        lane_incarnations,
        lane_incarnation_lineage,
        lane_incarnation_activation_heights,
        autoscale_sample_history,
        chain_id,
        network_id,
        snapshot_v2_bootstrap_candidate,
        nexus_runtime_restored_from_snapshot,
        kura,
        query_handle,
        #[cfg(feature = "telemetry")]
        telemetry,
    } = inputs;
    #[cfg(feature = "telemetry")]
    let telemetry_seed = telemetry.clone();
    let initial_crypto = iroha_config::parameters::actual::Crypto::default();
    let streaming_storage_paths = StreamingStoragePaths::default();
    let da_receipt_cursors = parking_lot::RwLock::new(DaReceiptCursorIndex::default());
    let da_shard_cursors = parking_lot::RwLock::new(DaShardCursorIndex::default());
    let restored_height = u64::try_from(block_hashes.committed_height()).map_err(|error| {
        MergeLedgerCommitError::ExecutionStatePublication(format!(
            "restored committed height does not fit the Parliament height domain: {error}"
        ))
    })?;
    for (proposal_id, proposal) in world.governance_proposals.view().iter() {
        if let Some(reason) = proposal.first_release_exact_json_u64_invariant_error() {
            return Err(MergeLedgerCommitError::ExecutionStatePublication(format!(
                "restored governance proposal {} violates the first-release JSON number invariant: {reason}",
                hex::encode(proposal_id),
            )));
        }
        if proposal.created_height > restored_height {
            return Err(MergeLedgerCommitError::ExecutionStatePublication(format!(
                "restored governance proposal {} was created at future height {} beyond committed height {restored_height}",
                hex::encode(proposal_id),
                proposal.created_height,
            )));
        }
    }
    for (governance_attempt_id, attempt) in world.parliament_attempts.view().iter() {
        attempt
            .validate_restored_height_v1(restored_height)
            .map_err(|error| {
                MergeLedgerCommitError::ExecutionStatePublication(format!(
                    "restored Parliament attempt {governance_attempt_id:?} has impossible lifecycle state at committed height {restored_height}: {error}"
                ))
            })?;
    }
    let canonical_query_index_status = {
        (restored_height > 0).then(|| QueryIndexStatus {
            indexed_height: restored_height,
            indexed_block_hash: block_hashes.view().last().copied(),
        })
    };
    let LoadedStateJournals {
        query_index: query_index_journal,
        query_projection_checkpoint: query_projection_checkpoint_journal,
    } = load_state_journals(&kura, canonical_query_index_status, allow_durable_recovery);
    let pipeline = default_pipeline();
    let pipeline_parallelism = if allow_durable_recovery {
        PipelineParallelism::new(&pipeline)
    } else {
        PipelineParallelism::inert(&pipeline)
    };
    let stateless_cache_cap = pipeline.stateless_cache_cap;
    let pipeline_cache_size = pipeline.cache_size;
    let tiered_backend = Arc::new(parking_lot::Mutex::new(TieredStateBackend::default()));
    let tiered_snapshot_worker = if allow_durable_recovery {
        TieredSnapshotWorker::new(
            Arc::clone(&tiered_backend),
            #[cfg(feature = "telemetry")]
            Some(telemetry_seed.clone()),
        )
    } else {
        TieredSnapshotWorker::inert(
            Arc::clone(&tiered_backend),
            #[cfg(feature = "telemetry")]
            None,
        )
    };
    let durable_blocks = kura.exact_durable_blocks_count().map_err(|error| {
        MergeLedgerCommitError::ExecutionStatePublication(format!(
            "failed to read the exact durable Kura boundary: {error}"
        ))
    })?;
    let latest_block_header = NonZeroUsize::new(durable_blocks)
        .and_then(|height| kura.get_block(height))
        .map(|block| block.header());
    let mut state = State {
        world,
        block_hashes,
        latest_block_header: parking_lot::RwLock::new(latest_block_header),
        merge_ledger: MergeLedgerStore::with_default_capacity(),
        merge_admission: parking_lot::RwLock::new(MergeAdmissionState::default()),
        replay_merge_carriers: parking_lot::RwLock::new(BTreeMap::new()),
        transactions,
        commit_topology,
        prev_commit_topology,
        da_commitments: parking_lot::RwLock::new(DaCommitmentStore::default()),
        da_confidential_compute: parking_lot::RwLock::new(ConfidentialComputeStore::default()),
        da_receipt_cursors,
        da_shard_cursors,
        da_shard_cursor_persistor: DaShardCursorJournalPersistor::new(),
        query_index_journal: parking_lot::RwLock::new(query_index_journal),
        query_index_journal_persistence_lock: parking_lot::Mutex::new(()),
        query_projection_checkpoint_journal: parking_lot::RwLock::new(
            query_projection_checkpoint_journal,
        ),
        query_projection_checkpoint_journal_persistence_lock: parking_lot::Mutex::new(()),
        da_pin_intents: parking_lot::RwLock::new(DaPinStore::default()),
        lane_relays: parking_lot::RwLock::new(LaneRelayStore::default()),
        settled_nexus_fee_receipts: parking_lot::RwLock::new(BTreeSet::new()),
        lane_manifests: parking_lot::RwLock::new(Arc::new(LaneManifestRegistry::empty())),
        lane_privacy_registry: parking_lot::RwLock::new(Arc::new(LanePrivacyRegistry::empty())),
        lane_compliance: parking_lot::RwLock::new(None),
        da_index_hydration_fence: parking_lot::Mutex::new(()),
        da_indexes_hydrated: parking_lot::RwLock::new(None),
        ivm,
        kura,
        query_handle,
        oracle: default_oracle(),
        pipeline,
        pipeline_parallelism,
        soracloud_runtime: parking_lot::RwLock::new(None),
        stateless_validation_cache: parking_lot::Mutex::new(StatelessValidationCache::new(
            stateless_cache_cap,
        )),
        trigger_ivm_cache: parking_lot::Mutex::new(IvmCache::with_capacity(pipeline_cache_size)),
        contract_query_ivm_cache: parking_lot::Mutex::new(IvmCache::with_capacity(
            pipeline_cache_size,
        )),
        pipeline_ivm_prepared_cache: parking_lot::RwLock::new(
            PreparedContractCache::with_capacity(pipeline_cache_size),
        ),
        streaming_storage_paths,
        crypto: parking_lot::RwLock::new(Arc::new(initial_crypto.clone())),
        nexus: parking_lot::RwLock::new(nexus),
        lane_incarnations: parking_lot::RwLock::new(lane_incarnations),
        lane_incarnation_lineage: parking_lot::RwLock::new(lane_incarnation_lineage),
        lane_incarnation_activation_heights: parking_lot::RwLock::new(
            lane_incarnation_activation_heights,
        ),
        nexus_runtime_restored_from_snapshot,
        nexus_storage_budget_last_check_height: AtomicU64::new(0),
        autoscale_sample_history: parking_lot::RwLock::new(autoscale_sample_history),
        tiered_backend: Arc::clone(&tiered_backend),
        tiered_snapshot_worker,
        fraud_monitoring: default_fraud_monitoring_cfg(),
        zk: default_zk(),
        gov: default_governance(),
        content: default_content_cfg(),
        settlement: iroha_config::parameters::actual::Settlement::default(),
        kagemusha_release_catalog: Arc::new(
            crate::smartcontracts::isi::offline::KagemushaReleaseCatalogV4::empty(),
        ),
        kagemusha_runtime_effective_config_sha256: SyncOnceCell::new(),
        settlement_engine: SettlementEngine::new_roadmap_default(),
        chain_id,
        network_id,
        snapshot_v2_bootstrap_candidate,
        authenticated_snapshot_v2_bootstrap: None,
        authenticated_snapshot_bootstrap_payload: None,
        #[cfg(feature = "telemetry")]
        telemetry,
        lane_lifecycle_lock: parking_lot::Mutex::new(()),
        state_commit_lock: Arc::new(parking_lot::Mutex::new(())),
        state_write_lock: parking_lot::Mutex::new(()),
        view_generation: AtomicU64::new(0),
        view_lock_contention_log: parking_lot::Mutex::new(ViewLockContentionLog::default()),
        sccp_registry_cache: parking_lot::Mutex::new(SccpRegistryCache::default()),
    };
    state
        .rebuild_derived_state_indexes()
        .map_err(MergeLedgerCommitError::ExecutionStatePublication)?;
    #[cfg(feature = "sm")]
    if allow_durable_recovery {
        Sm2PublicKey::set_default_distid(initial_crypto.sm2_distid_default.clone())
            .expect("sm2_distid_default must be valid");
    }
    #[cfg(feature = "telemetry")]
    {
        let view = state.world.governance_proposals.view();
        let records: Vec<_> = view.iter().map(|(id, rec)| (*id, rec.status)).collect();
        telemetry_seed.seed_governance_proposals(records);
        let citizens_total =
            u64::try_from(state.world.citizens.view().iter().count()).unwrap_or(u64::MAX);
        telemetry_seed.record_citizens_total(citizens_total);
    }
    if allow_durable_recovery && !state.kura.provisional_snapshot_bootstrap_pending() {
        state.recover_merge_ledger_from_kura()?;
    }
    Ok(state)
}
fn default_pipeline() -> iroha_config::parameters::actual::Pipeline {
    iroha_config::parameters::actual::Pipeline {
        dynamic_prepass: iroha_config::parameters::defaults::pipeline::DYNAMIC_PREPASS,
        access_set_cache_enabled:
            iroha_config::parameters::defaults::pipeline::ACCESS_SET_CACHE_ENABLED,
        parallel_overlay: iroha_config::parameters::defaults::pipeline::PARALLEL_OVERLAY,
        workers: iroha_config::parameters::defaults::pipeline::WORKERS,
        stateless_cache_cap: iroha_config::parameters::defaults::pipeline::STATELESS_CACHE_CAP,
        parallel_apply: iroha_config::parameters::defaults::pipeline::PARALLEL_APPLY,
        ready_queue_heap: iroha_config::parameters::defaults::pipeline::READY_QUEUE_HEAP,
        gpu_key_bucket: iroha_config::parameters::defaults::pipeline::GPU_KEY_BUCKET,
        debug_trace_scheduler_inputs:
            iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_SCHEDULER_INPUTS,
        debug_trace_tx_eval: iroha_config::parameters::defaults::pipeline::DEBUG_TRACE_TX_EVAL,
        signature_batch_max_ed25519:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_ED25519,
        signature_batch_max_secp256k1:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_SECP256K1,
        signature_batch_max_pqc:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_PQC,
        signature_batch_max_bls:
            iroha_config::parameters::defaults::pipeline::SIGNATURE_BATCH_MAX_BLS,
        cache_size: iroha_config::parameters::defaults::pipeline::CACHE_SIZE,
        ivm_cache_max_decoded_ops:
            iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_DECODED_OPS,
        ivm_cache_max_bytes: iroha_config::parameters::defaults::pipeline::IVM_CACHE_MAX_BYTES,
        ivm_prover_threads: iroha_config::parameters::defaults::pipeline::IVM_PROVER_THREADS,
        overlay_max_instructions:
            iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_INSTRUCTIONS,
        overlay_max_bytes: iroha_config::parameters::defaults::pipeline::OVERLAY_MAX_BYTES,
        overlay_chunk_instructions:
            iroha_config::parameters::defaults::pipeline::OVERLAY_CHUNK_INSTRUCTIONS,
        gas: iroha_config::parameters::actual::Gas {
            tech_account_id: iroha_config::parameters::defaults::pipeline::GAS_TECH_ACCOUNT_ID
                .to_string(),
            accepted_assets: Vec::new(),
            units_per_gas: Vec::new(),
        },
        ivm_max_cycles_upper_bound:
            iroha_config::parameters::defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND,
        ivm_max_decoded_instructions:
            iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_INSTRUCTIONS,
        ivm_max_decoded_bytes: iroha_config::parameters::defaults::pipeline::IVM_MAX_DECODED_BYTES,
        quarantine_max_txs_per_block:
            iroha_config::parameters::defaults::pipeline::QUARANTINE_MAX_TXS_PER_BLOCK,
        quarantine_tx_max_cycles:
            iroha_config::parameters::defaults::pipeline::QUARANTINE_TX_MAX_CYCLES,
        query_default_cursor_mode: iroha_config::parameters::actual::QueryCursorMode::Ephemeral,
        query_max_fetch_size: iroha_config::parameters::defaults::pipeline::QUERY_MAX_FETCH_SIZE,
        query_stored_min_gas_units:
            iroha_config::parameters::defaults::pipeline::QUERY_STORED_MIN_GAS_UNITS,
        amx_per_dataspace_budget_ms:
            iroha_config::parameters::defaults::pipeline::AMX_PER_DATASPACE_BUDGET_MS,
        amx_group_budget_ms: iroha_config::parameters::defaults::pipeline::AMX_GROUP_BUDGET_MS,
        amx_per_instruction_ns:
            iroha_config::parameters::defaults::pipeline::AMX_PER_INSTRUCTION_NS,
        amx_per_memory_access_ns:
            iroha_config::parameters::defaults::pipeline::AMX_PER_MEMORY_ACCESS_NS,
        amx_per_syscall_ns: iroha_config::parameters::defaults::pipeline::AMX_PER_SYSCALL_NS,
    }
}
pub(super) fn default_zk() -> iroha_config::parameters::actual::Zk {
    iroha_config::parameters::actual::Zk {
        halo2: iroha_config::parameters::actual::Halo2::default(),
        fastpq: iroha_config::parameters::actual::Fastpq {
            execution_mode: iroha_config::parameters::actual::FastpqExecutionMode::Cpu,
            poseidon_mode: iroha_config::parameters::actual::FastpqPoseidonMode::Cpu,
            proof_sidecar_queue_cap:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_QUEUE_CAP,
            proof_sidecar_max_bytes:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_BYTES,
            proof_sidecar_max_retries:
                iroha_config::parameters::defaults::zk::fastpq::PROOF_SIDECAR_MAX_RETRIES,
            device_class: None,
            chip_family: None,
            gpu_kind: None,
            metal_queue_fanout: None,
            metal_queue_column_threshold: None,
            metal_max_in_flight: None,
            metal_threadgroup_width: None,
            metal_trace: iroha_config::parameters::defaults::zk::fastpq::METAL_TRACE,
            metal_debug_enum: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_ENUM,
            metal_debug_fused: iroha_config::parameters::defaults::zk::fastpq::METAL_DEBUG_FUSED,
        },
        stark: iroha_config::parameters::actual::Stark::default(),
        sccp: iroha_config::parameters::actual::Sccp::default(),
        ballot_history_cap: iroha_config::parameters::defaults::zk::vote::BALLOT_HISTORY_CAP,
        preverify_max_bytes: iroha_config::parameters::defaults::zk::preverify::MAX_BYTES,
        preverify_budget_bytes: iroha_config::parameters::defaults::zk::preverify::BUDGET_BYTES,
        proof_history_cap: iroha_config::parameters::defaults::zk::proof::RECORD_HISTORY_CAP,
        proof_retention_grace_blocks:
            iroha_config::parameters::defaults::zk::proof::RETENTION_GRACE_BLOCKS,
        proof_prune_batch: iroha_config::parameters::defaults::zk::proof::PRUNE_BATCH_SIZE,
        bridge_proof_max_range_len:
            iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_RANGE_LEN,
        bridge_proof_max_past_age_blocks:
            iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_PAST_AGE_BLOCKS,
        bridge_proof_max_future_drift_blocks:
            iroha_config::parameters::defaults::zk::proof::BRIDGE_MAX_FUTURE_DRIFT_BLOCKS,
        poseidon_params_id: iroha_config::parameters::defaults::confidential::POSEIDON_PARAMS_ID,
        pedersen_params_id: iroha_config::parameters::defaults::confidential::PEDERSEN_PARAMS_ID,
        kaigi_roster_join_vk: None,
        kaigi_roster_leave_vk: None,
        kaigi_usage_vk: None,
        max_proof_size_bytes:
            iroha_config::parameters::defaults::confidential::MAX_PROOF_SIZE_BYTES,
        max_nullifiers_per_tx:
            iroha_config::parameters::defaults::confidential::MAX_NULLIFIERS_PER_TX,
        max_commitments_per_tx:
            iroha_config::parameters::defaults::confidential::MAX_COMMITMENTS_PER_TX,
        max_confidential_ops_per_block:
            iroha_config::parameters::defaults::confidential::MAX_CONFIDENTIAL_OPS_PER_BLOCK,
        verify_timeout: iroha_config::parameters::defaults::confidential::VERIFY_TIMEOUT,
        max_anchor_age_blocks:
            iroha_config::parameters::defaults::confidential::MAX_ANCHOR_AGE_BLOCKS,
        max_proof_bytes_block:
            iroha_config::parameters::defaults::confidential::MAX_PROOF_BYTES_BLOCK,
        max_verify_calls_per_tx:
            iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_TX,
        max_verify_calls_per_block:
            iroha_config::parameters::defaults::confidential::MAX_VERIFY_CALLS_PER_BLOCK,
        max_public_inputs: iroha_config::parameters::defaults::confidential::MAX_PUBLIC_INPUTS,
        reorg_depth_bound: iroha_config::parameters::defaults::confidential::REORG_DEPTH_BOUND,
        policy_transition_delay_blocks:
            iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_DELAY_BLOCKS,
        policy_transition_window_blocks:
            iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_WINDOW_BLOCKS,
        policy_transition_max_per_height:
            iroha_config::parameters::defaults::confidential::POLICY_TRANSITION_MAX_PER_HEIGHT,
        tree_roots_history_len:
            iroha_config::parameters::defaults::confidential::TREE_ROOTS_HISTORY_LEN,
        tree_frontier_checkpoint_interval:
            iroha_config::parameters::defaults::confidential::TREE_FRONTIER_CHECKPOINT_INTERVAL,
        registry_max_vk_entries:
            iroha_config::parameters::defaults::confidential::REGISTRY_MAX_VK_ENTRIES,
        registry_max_params_entries:
            iroha_config::parameters::defaults::confidential::REGISTRY_MAX_PARAMS_ENTRIES,
        registry_max_delta_per_block:
            iroha_config::parameters::defaults::confidential::REGISTRY_MAX_DELTA_PER_BLOCK,
        gas: iroha_config::parameters::actual::ConfidentialGas {
            proof_base: iroha_config::parameters::defaults::confidential::gas::PROOF_BASE,
            per_public_input:
                iroha_config::parameters::defaults::confidential::gas::PER_PUBLIC_INPUT,
            per_proof_byte: iroha_config::parameters::defaults::confidential::gas::PER_PROOF_BYTE,
            per_nullifier: iroha_config::parameters::defaults::confidential::gas::PER_NULLIFIER,
            per_commitment: iroha_config::parameters::defaults::confidential::gas::PER_COMMITMENT,
        },
    }
}
#[allow(clippy::too_many_lines)]
fn default_governance() -> iroha_config::parameters::actual::Governance {
    iroha_config::parameters::actual::Governance {
        vk_ballot: None,
        vk_tally: None,
        voting_asset_id: iroha_config::parameters::defaults::governance::voting_asset_id()
            .parse()
            .expect("valid default voting asset id"),
        citizenship_asset_id: iroha_config::parameters::defaults::governance::citizenship_asset_id(
        )
        .parse()
        .expect("valid default citizenship asset id"),
        citizenship_bond_amount:
            iroha_config::parameters::defaults::governance::citizenship_bond_amount(),
        citizenship_escrow_account:
            iroha_config::parameters::defaults::governance::citizenship_escrow_account_id(),
        min_bond_amount: 150_u64.into(),
        bond_escrow_account: iroha_config::parameters::defaults::governance::bond_escrow_account_id(
        ),
        slash_receiver_account:
            iroha_config::parameters::defaults::governance::slash_receiver_account_id(),
        slash_double_vote_bps:
            iroha_config::parameters::defaults::governance::slash_policy::DOUBLE_VOTE_BPS,
        slash_invalid_proof_bps:
            iroha_config::parameters::defaults::governance::slash_policy::MISCONDUCT_BPS,
        slash_ineligible_proof_bps:
            iroha_config::parameters::defaults::governance::slash_policy::INELIGIBLE_PROOF_BPS,
        alias_teu_minimum: iroha_config::parameters::defaults::governance::alias_teu_minimum(),
        alias_frontier_telemetry:
            iroha_config::parameters::defaults::governance::alias_frontier_telemetry(),
        debug_trace_pipeline: iroha_config::parameters::defaults::governance::DEBUG_TRACE_PIPELINE,
        jdg_signature_schemes:
            iroha_config::parameters::defaults::governance::jdg_signature_schemes()
                .into_iter()
                .map(|scheme| {
                    scheme
                        .parse::<iroha_data_model::jurisdiction::JdgSignatureScheme>()
                        .expect("valid default JDG signature scheme")
                })
                .collect(),
        runtime_upgrade_provenance:
            iroha_config::parameters::actual::RuntimeUpgradeProvenancePolicy::default(),
        citizen_service: iroha_config::parameters::actual::CitizenServiceDiscipline::default(),
        viral_incentives: iroha_config::parameters::actual::ViralIncentives::default(),
        sorafs_pin_policy: iroha_config::parameters::actual::SorafsPinPolicyConstraints::default(),
        sorafs_pin_fee_asset_id:
            iroha_config::parameters::defaults::governance::sorafs_pin_fee::asset_id()
                .parse()
                .expect("default SoraFS pin fee asset id"),
        sorafs_pin_fee_treasury_account:
            iroha_config::parameters::defaults::governance::sorafs_pin_fee::treasury_account_id(),
        sorafs_pricing: iroha_data_model::sorafs::pricing::PricingScheduleRecord::launch_default(),
        sorafs_penalty: iroha_config::parameters::actual::SorafsPenaltyPolicy::default(),
        sorafs_telemetry: iroha_config::parameters::actual::SorafsTelemetryPolicy::default(),
        sorafs_provider_owners: std::collections::BTreeMap::new(),
        conviction_step_blocks: 100,
        max_conviction: 6,
        min_enactment_delay: 20,
        window_span: 100,
        plain_voting_enabled: false,
        approval_threshold_q_num: 1,
        approval_threshold_q_den: 2,
        min_turnout: 0,
        parliament_committee_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_COMMITTEE_SIZE,
        parliament_term_blocks:
            iroha_config::parameters::defaults::governance::PARLIAMENT_TERM_BLOCKS,
        parliament_min_stake: iroha_config::parameters::defaults::governance::parliament_min_stake(
        ),
        parliament_eligibility_asset_id:
            iroha_config::parameters::defaults::governance::parliament_eligibility_asset_id()
                .parse()
                .expect("valid default governance asset id"),
        parliament_alternate_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_ALTERNATE_SIZE,
        parliament_quorum_bps:
            iroha_config::parameters::defaults::governance::PARLIAMENT_QUORUM_BPS,
        parliament_invitation_phase_blocks:
            iroha_config::parameters::defaults::governance::PARLIAMENT_INVITATION_PHASE_BLOCKS,
        parliament_public_finding_phase_blocks:
            iroha_config::parameters::defaults::governance::PARLIAMENT_PUBLIC_FINDING_PHASE_BLOCKS,
        parliament_timed_ovn: iroha_config::parameters::actual::ParliamentTimedOvn::default(),
        parliament_tle_partial_release_signer_provider_handle: None,
        parliament_tle_partial_release_signer_provider_revision: None,
        parliament_tle_partial_release_signer_provider_policy_digest: None,
        rules_committee_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_RULES_COMMITTEE_SIZE,
        agenda_council_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_AGENDA_COUNCIL_SIZE,
        interest_panel_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_INTEREST_PANEL_SIZE,
        review_panel_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_REVIEW_PANEL_SIZE,
        coordination_council_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_COORDINATION_COUNCIL_SIZE,
        policy_jury_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_POLICY_JURY_SIZE,
        confirmation_jury_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_CONFIRMATION_JURY_SIZE,
        oversight_committee_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_OVERSIGHT_COMMITTEE_SIZE,
        mpc_committee_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_MPC_COMMITTEE_SIZE,
        fma_committee_size:
            iroha_config::parameters::defaults::governance::PARLIAMENT_FMA_COMMITTEE_SIZE,
    }
}
fn reject_unknown(map: &SnapshotJsonMap<'_>, context: &str) -> Result<(), json::Error> {
    let Some(field) = map.first_key() else {
        return Ok(());
    };
    Err(json::Error::InvalidField {
        field: format!("{context}.{field}"),
        message: "unknown field is not permitted in a signed first-release snapshot".to_owned(),
    })
}
#[cfg(test)]
mod decode_tests {
    use super::*;
    use iroha_crypto::SignatureOf;
    use iroha_data_model::musubi::{
        MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiAliasHistoryActionV1,
        MusubiArchiveCommitmentV1, MusubiArchiveLocationIdV1, MusubiArtifactGovernanceStateV1,
        MusubiArtifactTakedownV1, MusubiContentDigestV1, MusubiGovernanceDecisionV1,
        MusubiMaintainerPermissionsV1, MusubiNamespaceBindingDigestV1, MusubiPackageRevisionsV1,
        MusubiPackageScopeV1, MusubiParliamentActionV1, MusubiProviderBundleAttestationDigestV1,
        MusubiProviderBundleAttestationSetDigestV1, MusubiProviderBundleVerificationApprovalV1,
        MusubiProviderBundleVerificationAttestationV1, MusubiProviderBundleVerificationBindingV1,
        MusubiProviderBundleVerificationPayloadV1, MusubiReasonV1, MusubiRecoverPackageOwnersV1,
        MusubiRegistryAdmissionModeV1, MusubiReleaseManifestV1, MusubiReleaseMetadataV1,
        MusubiReleaseRevisionsV1, MusubiReleaseSelectionStateV1, MusubiReleaseYankV1,
        MusubiRetargetAliasV1, MusubiSeedIngressReceiptApprovalV1,
        MusubiSeedIngressReceiptBindingV1, MusubiSeedIngressReceiptPayloadV1,
        MusubiSeedIngressReceiptV1, MusubiSetRegistryPolicyActionV1, MusubiStorageAvailabilityV1,
        MusubiTakedownArtifactActionV1, MusubiVerificationLockDigestV1,
    };
    use iroha_data_model::sorafs::pin_registry::{
        ChunkerProfileHandle, ManifestRootCid, ProviderIngestCompletionSignerPolicyV1,
        ProviderIngestFinalizedAnchorV1,
    };

    #[test]
    fn restored_proposal_status_must_match_latest_attempt_exactly() {
        use iroha_data_model::governance::types::GovernanceAttemptStatusV1 as Attempt;

        assert!(proposal_status_matches_latest_attempt_v1(
            GovernanceProposalStatus::Rejected,
            Some(Attempt::Rejected),
        ));
        assert!(proposal_status_matches_latest_attempt_v1(
            GovernanceProposalStatus::ExecutionFailed,
            Some(Attempt::ExecutionFailed),
        ));
        assert!(!proposal_status_matches_latest_attempt_v1(
            GovernanceProposalStatus::Proposed,
            Some(Attempt::Rejected),
        ));
        assert!(!proposal_status_matches_latest_attempt_v1(
            GovernanceProposalStatus::Rejected,
            Some(Attempt::ExecutionFailed),
        ));
    }

    fn musubi_account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive deterministic Musubi snapshot account");
        AccountId::new(key_pair.public_key().clone())
    }
    fn musubi_package(name: &str) -> MusubiPackageIdV1 {
        MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            name.parse().expect("Musubi package name"),
        )
    }
    #[test]
    fn legacy_musubi_generic_state_rejects_entire_reserved_namespace() {
        for path in [
            "musubi",
            "musubi_package_catalog",
            "musubi_registry",
            "musubi_short_aliases",
            "musubi/releases",
            "musubi.v0.aliases",
            "musubi:registry",
        ] {
            let mut state = Storage::<StatePath, Vec<u8>>::default();
            state.insert(path.parse().expect("valid legacy state path"), vec![1]);
            let error = reject_legacy_musubi_state(&state)
                .expect_err("every reserved legacy Musubi state path must block loading");
            assert!(
                error.to_string().contains(path),
                "rejection must identify the exact legacy path: {error}"
            );
        }
        let mut unrelated = Storage::<StatePath, Vec<u8>>::default();
        unrelated.insert(
            "musubian_contract"
                .parse()
                .expect("valid unrelated state path"),
            vec![1],
        );
        reject_legacy_musubi_state(&unrelated)
            .expect("a non-reserved prefix must not be mistaken for Musubi state");
    }
    fn retain_musubi_consumption_as(
        world: &mut World,
        decision_id: [u8; 32],
        action: MusubiParliamentActionV1,
        enacted_at_height: u64,
        execute_after_height: u64,
        consumed_at_height: u64,
    ) -> iroha_data_model::musubi::MusubiGovernanceActionDigestV1 {
        let action_digest = action.action_digest();
        let kind = ProposalKind::MusubiRegistryGovernance(action);
        let network_id = iroha_data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x5E; 32])),
        );
        let attempt = crate::governance::parliament::enacted_parliament_attempt_for_testing(
            &kind,
            vec![musubi_account(61), musubi_account(62), musubi_account(63)],
            &network_id,
            enacted_at_height,
        );
        let attempt_id = attempt.attempt().id;
        world.governance_proposals.insert(
            decision_id,
            GovernanceProposalRecord {
                proposer: musubi_account(60),
                kind,
                created_height: 1,
                status: GovernanceProposalStatus::Enacted,
            },
        );
        world.parliament_attempts.insert(attempt_id, attempt);
        let consumption = MusubiGovernanceDecisionConsumptionV1 {
            decision: MusubiGovernanceDecisionV1 {
                decision_id,
                action_digest,
                enacted_at_height,
                execute_after_height,
            },
            minimum_enactment_delay: execute_after_height
                .checked_sub(enacted_at_height)
                .expect("execution boundary follows enactment height"),
            consumed_at_height,
        };
        consumption.validate().expect("valid decision consumption");
        world
            .musubi_governance_decisions
            .insert(decision_id, consumption);
        action_digest
    }
    fn retain_musubi_consumption(
        world: &mut World,
        action: MusubiParliamentActionV1,
        enacted_at_height: u64,
        execute_after_height: u64,
        consumed_at_height: u64,
    ) -> iroha_data_model::musubi::MusubiGovernanceActionDigestV1 {
        let decision_id = ProposalKind::MusubiRegistryGovernance(action.clone()).fingerprint();
        retain_musubi_consumption_as(
            world,
            decision_id,
            action,
            enacted_at_height,
            execute_after_height,
            consumed_at_height,
        )
    }
    fn musubi_policy_successor(
        current: &MusubiRegistryPolicyV1,
        mode: MusubiRegistryAdmissionModeV1,
    ) -> MusubiRegistryPolicyV1 {
        let mut successor = current.clone();
        successor.revision = current.revision.checked_add(1).expect("policy revision");
        successor.mode = mode;
        successor.allowlisted_dataspaces.clear();
        successor
            .validate_successor(current)
            .expect("canonical policy successor");
        successor
    }
    fn seed_current_musubi_package(
        world: &mut World,
        package: &MusubiPackageIdV1,
        owner: &AccountId,
        governance_revision: u64,
        accepted_at_height: u64,
    ) -> MusubiPackageMemberV1 {
        let record = MusubiPackageRecordV1 {
            package: package.clone(),
            claimed_namespace: "sora".parse().expect("namespace"),
            claimed_namespace_binding: MusubiNamespaceBindingDigestV1::new([1; 32]),
            owners: vec![owner.clone()],
            member_accounts: vec![owner.clone()],
            claimed_at_height: 1,
            revisions: MusubiPackageRevisionsV1 {
                governance: governance_revision,
                metadata: 1,
                archive_locations: 1,
            },
        };
        record.validate().expect("valid package fixture");
        world.musubi_packages.insert(package.clone(), record);
        let member = MusubiPackageMemberV1 {
            package: package.clone(),
            account: owner.clone(),
            role: MusubiPackageRoleV1::Owner,
            accepted_at_height,
            governance_revision,
        };
        member.validate().expect("valid member fixture");
        world
            .musubi_package_members
            .insert(member.key(), member.clone());
        member
    }
    fn musubi_release_record(
        release: MusubiReleaseIdV1,
        artifact_governance: MusubiArtifactGovernanceStateV1,
        artifact_governance_revision: u64,
    ) -> MusubiReleaseRecordV1 {
        let manifest = MusubiReleaseManifestV1 {
            release: release.clone(),
            edition: iroha_data_model::musubi::MusubiKotodamaEditionV1::V1,
            abi: MusubiAbiBindingV1::new([0xA1; 32]).expect("nonzero ABI hash"),
            dependencies: Vec::new(),
            exports: Vec::new(),
            interface_digest: MusubiContentDigestV1::new([0xA2; 32]),
            metadata: MusubiReleaseMetadataV1::default(),
            archive_id: ArchiveId::new([0xA3; 32]),
            verification_lock_digest: MusubiVerificationLockDigestV1::new([0xA4; 32]),
        };
        let publisher = musubi_account(61);
        let record = MusubiReleaseRecordV1 {
            release_digest: manifest.release_digest(),
            yank: MusubiReleaseYankV1 {
                release,
                yanked: false,
                reason: "initial publication".parse().expect("bounded reason"),
                changed_by: publisher.clone(),
                changed_at_height: 1,
                revision: 1,
            },
            artifact_governance,
            revisions: MusubiReleaseRevisionsV1 {
                yank: 1,
                artifact_governance: artifact_governance_revision,
            },
            manifest,
            published_by: publisher,
            published_at_height: 1,
        };
        record.validate().expect("valid release fixture");
        record
    }
    fn validate_musubi_persisted_snapshot(world: &World) -> Result<(), json::Error> {
        MusubiPersistedState {
            namespace_bindings: &world.musubi_namespace_bindings,
            packages: &world.musubi_packages,
            package_metadata: &world.musubi_package_metadata,
            package_members: &world.musubi_package_members,
            package_invitations: &world.musubi_package_invitations,
            maintainer_directory: &world.musubi_maintainer_directory,
            releases: &world.musubi_releases,
            archives: &world.musubi_archives,
            provider_bundle_attestations: &world.musubi_provider_bundle_attestations,
            archive_locations: &world.musubi_archive_locations,
            archive_availability: &world.musubi_archive_availability,
            archive_reverse_references: &world.musubi_archive_reverse_references,
            resolver_index: &world.musubi_resolver_index,
            resolver_index_checkpoints: &world.musubi_resolver_index_checkpoints,
            public_directory: &world.musubi_public_directory,
            aliases: &world.musubi_aliases,
            alias_history: &world.musubi_alias_history,
            governance_decisions: &world.musubi_governance_decisions,
            resolver_index_revision: world.musubi_resolver_index_revision.view().get().get(),
            replication_shortfall_releases: *world
                .musubi_replication_shortfall_releases
                .view()
                .get(),
        }
        .validate()
    }
    fn validate_musubi_publication_snapshot(world: &World) -> Result<(), json::Error> {
        validate_musubi_location_reverse_indices(
            &world.musubi_archives,
            &world.musubi_archive_locations,
            &world.pin_manifests,
            &world.replication_orders,
            &world.musubi_locations_by_pin,
            &world.musubi_locations_by_replication_order,
            &world.musubi_locations_by_provider,
        )?;
        validate_musubi_persisted_snapshot(world)?;
        validate_musubi_live_projections(world)
    }
    #[allow(clippy::too_many_lines)]
    fn seeded_musubi_publication_snapshot()
    -> (World, MusubiReleaseIdV1, ArchiveId, MusubiPackageSelectorV1) {
        let mut world = World::default();
        let package = musubi_package("atomic-publication");
        let namespace: MusubiNamespaceV1 = "sora".parse().expect("namespace");
        let binding = MusubiNamespaceBindingV1 {
            namespace: namespace.clone(),
            home_dataspace: package.home_dataspace,
            scope: package.scope.clone(),
            generation: 1,
        };
        let publisher_keypair = KeyPair::try_from_seed(vec![61; 32], Algorithm::Ed25519)
            .expect("derive deterministic publisher key");
        let publisher = AccountId::new(publisher_keypair.public_key().clone());
        let package_record = MusubiPackageRecordV1 {
            package: package.clone(),
            claimed_namespace: namespace.clone(),
            claimed_namespace_binding: binding.digest(),
            owners: vec![publisher.clone()],
            member_accounts: vec![publisher.clone()],
            claimed_at_height: 1,
            revisions: MusubiPackageRevisionsV1 {
                governance: 1,
                metadata: 1,
                archive_locations: 1,
            },
        };
        package_record.validate().expect("valid package record");
        let member = MusubiPackageMemberV1 {
            package: package.clone(),
            account: publisher.clone(),
            role: MusubiPackageRoleV1::Owner,
            accepted_at_height: 1,
            governance_revision: 1,
        };
        member.validate().expect("valid package owner");
        let metadata = MusubiPackageMetadataRecordV1 {
            package: package.clone(),
            metadata: MusubiReleaseMetadataV1::default(),
            revision: 1,
            changed_by: publisher.clone(),
            changed_at_height: 1,
        };
        metadata.validate().expect("valid package metadata");
        let commitment = MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::from_blake3_digest([0x11; 32]).expect("archive root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: 1,
                namespace: "sorafs".to_owned(),
                name: "sf1".to_owned(),
                semver: "1.0.0".to_owned(),
                multihash_code: 0x1f,
            },
            chunk_plan_digest: MusubiContentDigestV1::new([0x12; 32]),
            por_root: MusubiContentDigestV1::new([0x13; 32]),
            content_length: 1,
            car_digest: MusubiContentDigestV1::new([0x14; 32]),
            car_size: 1,
            bundle_digest: MusubiContentDigestV1::new([0x15; 32]),
            source_tree_digest: MusubiContentDigestV1::new([0x16; 32]),
            descriptor_digest: MusubiContentDigestV1::new([0x17; 32]),
            file_count: 1,
            chunk_count: 1,
        };
        let archive_id = commitment.archive_id();
        let release =
            MusubiReleaseIdV1::new(package.clone(), "1.0.0".parse().expect("release version"));
        let manifest = MusubiReleaseManifestV1 {
            release: release.clone(),
            edition: iroha_data_model::musubi::MusubiKotodamaEditionV1::V1,
            abi: MusubiAbiBindingV1::new([0x18; 32]).expect("ABI binding"),
            dependencies: Vec::new(),
            exports: Vec::new(),
            interface_digest: MusubiContentDigestV1::new([0x19; 32]),
            metadata: MusubiReleaseMetadataV1::default(),
            archive_id,
            verification_lock_digest: MusubiVerificationLockDigestV1::new([0x1A; 32]),
        };
        manifest.validate().expect("valid release manifest");
        let broker_keypair = KeyPair::try_from_seed(vec![62; 32], Algorithm::Ed25519)
            .expect("derive deterministic ingress broker key");
        let receipt_payload = MusubiSeedIngressReceiptPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: MusubiSeedIngressReceiptBindingV1 {
                network_id: iroha_data_model::NetworkId::from_genesis_hash(
                    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x1B; 32])),
                ),
                publisher: publisher.clone(),
                ingress_broker: AccountId::new(broker_keypair.public_key().clone()),
                seed_provider: ProviderId::new([0x1C; 32]),
                semantic_release_manifest_digest: manifest.semantic_digest(),
                archive_id,
                car_body_digest: commitment.car_digest,
                car_body_length: commitment.car_size,
                nonce: [0x1D; 32],
            },
            issued_at_ms: 1,
            expires_at_ms: 2,
        };
        let receipt = MusubiSeedIngressReceiptV1 {
            approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
                public_key: broker_keypair.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    broker_keypair.private_key(),
                    receipt_payload.signing_hash(),
                )
                .expect("sign ingress receipt"),
            }],
            payload: receipt_payload,
        };
        let archive = MusubiArchiveRecordV1 {
            archive_id,
            commitment: commitment.clone(),
            staging_receipt: receipt,
            registered_by: publisher.clone(),
            registered_at_height: 1,
            location_revision: 1,
            location_ids: Vec::new(),
        };
        archive.validate().expect("valid archive record");
        let availability = MusubiArchiveAvailabilityV1 {
            archive_id,
            availability: MusubiStorageAvailabilityV1::Unavailable,
            healthy_replicas: 0,
            active_locations: 0,
            finalized_height: 1,
            finalized_block_hash: [0x1E; 32],
            index_revision: 1,
        };
        availability.validate().expect("valid archive availability");
        let yank = MusubiReleaseYankV1 {
            release: release.clone(),
            yanked: false,
            reason: "initial publication".parse().expect("bounded reason"),
            changed_by: publisher.clone(),
            changed_at_height: 2,
            revision: 1,
        };
        let release_record = MusubiReleaseRecordV1 {
            release_digest: manifest.release_digest(),
            manifest: manifest.clone(),
            published_by: publisher,
            published_at_height: 2,
            yank: yank.clone(),
            artifact_governance: MusubiArtifactGovernanceStateV1::Available,
            revisions: MusubiReleaseRevisionsV1 {
                yank: 1,
                artifact_governance: 1,
            },
        };
        release_record.validate().expect("valid release record");
        let resolver_row = MusubiResolverReleaseRowV1 {
            release: release.clone(),
            release_digest: release_record.release_digest,
            archive_id,
            source_digest: commitment.source_tree_digest,
            interface_digest: manifest.interface_digest,
            abi: manifest.abi,
            dependencies: Vec::new(),
            selection: MusubiReleaseSelectionStateV1 {
                yank,
                storage: availability,
                governance: MusubiArtifactGovernanceStateV1::Available,
            },
            index_revision: 2,
        };
        resolver_row.validate().expect("valid resolver row");
        let selector = MusubiPackageSelectorV1 {
            namespace,
            name: package.name.clone(),
        };
        let directory = MusubiOrderedPackageEntryV1 {
            selector: selector.clone(),
            package: package.clone(),
            latest_selectable: None,
            metadata_revision: 1,
            index_revision: 2,
        };
        directory.validate().expect("valid public directory entry");
        world
            .musubi_namespace_bindings
            .insert(binding.namespace.clone(), binding);
        world
            .musubi_package_members
            .insert(member.key(), member.clone());
        world.musubi_maintainer_directory.insert(
            MusubiMaintainerDirectoryKeyV1::accepted(package.clone(), member.account.clone()),
            MusubiMaintainerDirectoryEntryV1::Accepted(member),
        );
        world
            .musubi_package_metadata
            .insert(package.clone(), metadata);
        world.musubi_packages.insert(package, package_record);
        world.musubi_archives.insert(archive_id, archive);
        world
            .musubi_archive_availability
            .insert(archive_id, availability);
        world.musubi_archive_reverse_references.insert(
            archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id,
                releases: vec![release.clone()],
            },
        );
        world
            .musubi_releases
            .insert(release.clone(), release_record);
        world
            .musubi_resolver_index
            .insert(release.clone(), resolver_row);
        world
            .musubi_public_directory
            .insert(selector.clone(), directory);
        world.musubi_resolver_index_revision =
            Cell::new(MusubiResolverIndexRevisionV1::new(2).expect("resolver revision"));
        world.musubi_replication_shortfall_releases = Cell::new(1);
        (world, release, archive_id, selector)
    }
    #[allow(clippy::too_many_lines)]
    fn seed_provider_attested_location(
        world: &mut World,
        release: &MusubiReleaseIdV1,
        archive_id: ArchiveId,
    ) -> MusubiProviderBundleAttestationKeyV1 {
        let archive = world
            .musubi_archives
            .view()
            .get(&archive_id)
            .cloned()
            .expect("seeded archive");
        let verification_lock_digest = world
            .musubi_releases
            .view()
            .get(release)
            .map(|record| record.manifest.verification_lock_digest)
            .expect("seeded release");
        let provider_keypair = KeyPair::try_from_seed(vec![70; 32], Algorithm::Ed25519)
            .expect("derive deterministic provider key");
        let provider_owner = AccountId::new(provider_keypair.public_key().clone());
        let provider_id = ProviderId::new([0x31; 32]);
        let replication_order = ReplicationOrderId::new([0x32; 32]);
        let location_id = MusubiArchiveLocationIdV1::new([0x33; 32]);
        let completion_authority = ProviderIngestCompletionAuthorityV1::new(
            provider_owner.clone(),
            ProviderIngestCompletionSignerPolicyV1 {
                policy_id: [0x34; 32],
                revision: 1,
                predecessor_digest: None,
                policy_digest: [0x35; 32],
            },
        );
        let binding = MusubiProviderBundleVerificationBindingV1 {
            network_id: archive.staging_receipt.payload.binding.network_id,
            provider_id,
            completed_by: provider_owner,
            completion_authority,
            replication_order,
            assignment_revision: 1,
            completion_epoch: 1,
            finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                height: 2,
                block_hash: [0x36; 32],
            },
            archive_id,
            bundle_digest: archive.commitment.bundle_digest,
            descriptor_digest: archive.commitment.descriptor_digest,
            semantic_release_manifest_digest: archive
                .staging_receipt
                .payload
                .binding
                .semantic_release_manifest_digest,
            verification_lock_digest,
            source_tree_digest: archive.commitment.source_tree_digest,
        };
        let payload = MusubiProviderBundleVerificationPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: binding.clone(),
        };
        let attestation = MusubiProviderBundleVerificationAttestationV1 {
            approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
                public_key: provider_keypair.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    provider_keypair.private_key(),
                    payload.signing_hash(),
                )
                .expect("sign provider bundle attestation"),
            }],
            payload,
        };
        attestation
            .verify(&binding)
            .expect("provider bundle attestation fixture verifies");
        let attestation_key = attestation.key();
        let attestation_reference = attestation.reference();
        let attestation_record = MusubiProviderBundleAttestationRecordV1 {
            key: attestation_key,
            attestation_digest: attestation.digest(),
            attestation,
            registered_by: archive.registered_by.clone(),
            registered_at_height: 2,
        };
        attestation_record
            .validate()
            .expect("valid provider attestation record");
        let provider_attestation_set_digest = musubi_provider_bundle_attestation_set_digest_v1(
            archive_id,
            replication_order,
            &[attestation_reference],
        )
        .expect("valid provider attestation set");
        let location = MusubiArchiveLocationV1 {
            location_id,
            archive_id,
            pin_manifest: ManifestDigest::new([0x37; 32]),
            replication_order,
            providers: vec![provider_id],
            provider_attestation_set_digest,
            renew_after_epoch: 1,
            expires_at_epoch: 2,
            finalized_height: 2,
            revision: 2,
            state: MusubiArchiveLocationStateV1::Degraded,
        };
        location.validate().expect("valid archive location fixture");
        let mut updated_archive = archive;
        updated_archive.location_revision = 2;
        updated_archive.location_ids = vec![location_id];
        updated_archive
            .validate()
            .expect("archive contains the exact current location directory");
        world.musubi_archives.insert(archive_id, updated_archive);
        world
            .musubi_provider_bundle_attestations
            .insert(attestation_key, attestation_record);
        world
            .musubi_archive_locations
            .insert(location.key(), location);
        attestation_key
    }
    #[test]
    fn musubi_publication_snapshot_validates_replication_shortfall_aggregate() {
        let (baseline, _, _, _) = seeded_musubi_publication_snapshot();
        validate_musubi_publication_snapshot(&baseline)
            .expect("one release on an unavailable archive has shortfall count one");
        let (mut mismatched, _, _, _) = seeded_musubi_publication_snapshot();
        mismatched.musubi_replication_shortfall_releases = Cell::new(0);
        let error = validate_musubi_publication_snapshot(&mismatched)
            .expect_err("a stale persisted shortfall aggregate must fail closed");
        assert!(
            error
                .to_string()
                .contains("musubi_replication_shortfall_releases"),
            "unexpected shortfall mismatch diagnostic: {error}"
        );
    }
    #[test]
    fn musubi_persisted_state_rejects_missing_provider_attestation_record() {
        let (mut world, release, archive_id, _) = seeded_musubi_publication_snapshot();
        let attestation_key = seed_provider_attested_location(&mut world, &release, archive_id);
        validate_musubi_persisted_snapshot(&world)
            .expect("an archive location with its exact attestation record is valid");
        {
            let mut mutation = world.musubi_provider_bundle_attestations.block();
            mutation.remove(attestation_key);
            mutation.commit();
        }
        let error = validate_musubi_persisted_snapshot(&world)
            .expect_err("a location must not outlive its exact provider attestation record");
        assert!(
            error
                .to_string()
                .contains("missing exact provider attestation"),
            "unexpected missing-attestation diagnostic: {error}"
        );
    }
    #[test]
    fn musubi_persisted_state_rejects_corrupt_provider_attestation_record() {
        let (mut world, release, archive_id, _) = seeded_musubi_publication_snapshot();
        let attestation_key = seed_provider_attested_location(&mut world, &release, archive_id);
        let mut corrupt_record = world
            .musubi_provider_bundle_attestations
            .view()
            .get(&attestation_key)
            .cloned()
            .expect("seeded provider attestation record");
        corrupt_record.attestation_digest =
            MusubiProviderBundleAttestationDigestV1::new([0xFF; 32]);
        world
            .musubi_provider_bundle_attestations
            .insert(attestation_key, corrupt_record);
        let error = validate_musubi_persisted_snapshot(&world)
            .expect_err("a corrupt provider attestation record must fail closed");
        assert!(
            error
                .to_string()
                .contains("musubi_provider_bundle_attestations"),
            "unexpected corrupt-attestation diagnostic: {error}"
        );
    }
    #[test]
    fn musubi_persisted_state_rejects_corrupt_provider_attestation_set_digest() {
        let (mut world, release, archive_id, _) = seeded_musubi_publication_snapshot();
        seed_provider_attested_location(&mut world, &release, archive_id);
        let (location_key, mut location) = world
            .musubi_archive_locations
            .view()
            .iter()
            .next()
            .map(|(key, location)| (*key, location.clone()))
            .expect("seeded archive location");
        location.provider_attestation_set_digest =
            MusubiProviderBundleAttestationSetDigestV1::new([0xFE; 32]);
        world
            .musubi_archive_locations
            .insert(location_key, location);
        let error = validate_musubi_persisted_snapshot(&world)
            .expect_err("a corrupt provider-attestation aggregate must fail closed");
        assert!(
            error
                .to_string()
                .contains("provider-attestation set digest is not exact"),
            "unexpected corrupt-attestation-set diagnostic: {error}"
        );
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn musubi_publication_snapshot_rejects_every_one_sided_projection_cut() {
        let (baseline, _, _, _) = seeded_musubi_publication_snapshot();
        validate_musubi_publication_snapshot(&baseline)
            .expect("complete home and universal publication projections are valid");
        let (mut universal_only, release, archive_id, _) = seeded_musubi_publication_snapshot();
        {
            let mut mutation = universal_only.musubi_releases.block();
            mutation.remove(release.clone());
            mutation.commit();
        }
        universal_only.musubi_archive_reverse_references.insert(
            archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id,
                releases: Vec::new(),
            },
        );
        universal_only.musubi_replication_shortfall_releases = Cell::new(0);
        let error = validate_musubi_publication_snapshot(&universal_only)
            .expect_err("a universal resolver row cannot survive without its home release");
        assert!(
            error
                .to_string()
                .contains("resolver row references a missing release"),
            "unexpected universal-only diagnostic: {error}"
        );
        let (mut home_only, release, archive_id, selector) = seeded_musubi_publication_snapshot();
        home_only.musubi_archive_reverse_references.insert(
            archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id,
                releases: Vec::new(),
            },
        );
        {
            let mut mutation = home_only.musubi_resolver_index.block();
            mutation.remove(release.clone());
            mutation.commit();
        }
        {
            let mut mutation = home_only.musubi_public_directory.block();
            mutation.remove(selector.clone());
            mutation.commit();
        }
        let error = validate_musubi_publication_snapshot(&home_only)
            .expect_err("a home release cannot survive without universal projections");
        assert!(
            error
                .to_string()
                .contains("archive reverse references are not the exact release set"),
            "unexpected home-only diagnostic: {error}"
        );
        let (missing_resolver, release, _, selector) = seeded_musubi_publication_snapshot();
        {
            let mut mutation = missing_resolver.musubi_resolver_index.block();
            mutation.remove(release);
            mutation.commit();
        }
        {
            let mut mutation = missing_resolver.musubi_public_directory.block();
            mutation.remove(selector);
            mutation.commit();
        }
        let error = validate_musubi_publication_snapshot(&missing_resolver)
            .expect_err("a reverse reference cannot survive without its resolver row");
        assert!(
            error
                .to_string()
                .contains("release is missing its exact resolver row"),
            "unexpected missing-resolver diagnostic: {error}"
        );
        let (missing_directory, _, _, selector) = seeded_musubi_publication_snapshot();
        {
            let mut mutation = missing_directory.musubi_public_directory.block();
            mutation.remove(selector);
            mutation.commit();
        }
        let error = validate_musubi_publication_snapshot(&missing_directory)
            .expect_err("a resolver row cannot survive without its directory projection");
        assert!(
            error
                .to_string()
                .contains("package is missing its canonical public-directory entry"),
            "unexpected missing-directory diagnostic: {error}"
        );
        let (mut replayed, release, archive_id, selector) = seeded_musubi_publication_snapshot();
        let replayed_release = replayed
            .musubi_releases
            .view()
            .get(&release)
            .cloned()
            .expect("seeded release");
        let replayed_references = replayed
            .musubi_archive_reverse_references
            .view()
            .get(&archive_id)
            .cloned()
            .expect("seeded reverse references");
        let replayed_resolver = replayed
            .musubi_resolver_index
            .view()
            .get(&release)
            .cloned()
            .expect("seeded resolver row");
        let replayed_directory = replayed
            .musubi_public_directory
            .view()
            .get(&selector)
            .cloned()
            .expect("seeded public directory");
        assert_eq!(
            replayed
                .musubi_releases
                .insert(release.clone(), replayed_release.clone()),
            Some(replayed_release)
        );
        assert_eq!(
            replayed
                .musubi_archive_reverse_references
                .insert(archive_id, replayed_references.clone()),
            Some(replayed_references)
        );
        assert_eq!(
            replayed
                .musubi_resolver_index
                .insert(release, replayed_resolver.clone()),
            Some(replayed_resolver)
        );
        assert_eq!(
            replayed
                .musubi_public_directory
                .insert(selector, replayed_directory.clone()),
            Some(replayed_directory)
        );
        validate_musubi_publication_snapshot(&replayed)
            .expect("an exact projection replay preserves the complete snapshot");
    }
    #[test]
    fn take_parameters_cell_accepts_canonical_mv_envelope() {
        let expected = Parameters::default();
        let blocks = norito::json::to_value(&expected).expect("serialize parameters");
        let mut canonical_map = json::native::Map::new();
        canonical_map.insert("revert".to_owned(), json::Value::Null);
        canonical_map.insert("blocks".to_owned(), blocks);
        let canonical = json::Value::Object(canonical_map);
        let mut map = json::native::Map::new();
        map.insert("parameters".to_owned(), canonical);
        let mut map = SnapshotJsonMap::from_owned(map);
        let parsed = take_parameters_cell(&mut map, "parameters")
            .expect("canonical parameters MV envelope must be accepted");
        assert_eq!(parsed.view().get(), &expected);
    }
    #[test]
    fn borrowed_snapshot_map_rejects_duplicates_and_noncanonical_order() {
        let canonical = SnapshotJsonMap::parse(r#"{"first":0,"second":1}"#, "fixture")
            .expect("borrowed fixture map");
        canonical
            .require_source_order(&["first", "second"], "fixture")
            .expect("declared schema order must match");
        assert!(
            canonical
                .require_source_order(&["second", "first"], "fixture")
                .is_err()
        );
        assert!(
            SnapshotJsonMap::parse(r#"{"first":0,"first":1}"#, "fixture").is_err(),
            "duplicate signed snapshot fields must fail closed"
        );
    }
    #[test]
    fn borrowed_snapshot_field_errors_retain_the_schema_field() {
        let error = match SnapshotJsonField::Borrowed("[]")
            .decode_canonical::<Cell<Vec<PeerId>>>("commit_topology")
        {
            Ok(_) => panic!("an array must not decode as an MV topology cell"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            json::Error::InvalidField { field, message }
                if field == "commit_topology" && message.contains("expected object start")
        ));
    }
    #[test]
    fn snapshot_record_order_must_be_strict_and_duplicate_free() {
        validate_canonical_snapshot_record_order(&[1_u8, 2, 3], "fixture", |value| *value)
            .expect("strict order is canonical");
        assert!(
            validate_canonical_snapshot_record_order(&[2_u8, 1], "fixture", |value| *value)
                .is_err()
        );
        assert!(
            validate_canonical_snapshot_record_order(&[1_u8, 1], "fixture", |value| *value)
                .is_err()
        );
    }
    #[test]
    fn snapshot_norito_records_require_exact_canonical_bytes() {
        let encoded = 7_u64.encode();
        let record = SnapshotNoritoBlob {
            encoded_hex: hex::encode(&encoded),
        };
        assert_eq!(
            decode_snapshot_records::<u64>(vec![record], "fixture")
                .expect("canonical Norito record"),
            [7]
        );
        let mut trailing = encoded;
        trailing.push(0);
        let error = decode_snapshot_records::<u64>(
            vec![SnapshotNoritoBlob {
                encoded_hex: hex::encode(trailing),
            }],
            "fixture",
        )
        .expect_err("trailing or alternate Norito bytes must fail closed");
        assert!(error.to_string().contains("fixture"));
    }
    #[test]
    fn take_parameters_cell_rejects_legacy_blocks_envelope() {
        let expected = Parameters::default();
        let blocks = norito::json::to_value(&expected).expect("serialize parameters");
        let mut legacy_map = json::native::Map::new();
        legacy_map.insert("blocks".to_owned(), blocks);
        let mut map = json::native::Map::new();
        map.insert("parameters".to_owned(), json::Value::Object(legacy_map));
        let mut map = SnapshotJsonMap::from_owned(map);
        let error = take_parameters_cell(&mut map, "parameters")
            .err()
            .expect("legacy blocks-only parameters envelope must be rejected");
        assert!(
            error.to_string().contains("parameters"),
            "unexpected diagnostic: {error}"
        );
    }
    #[test]
    fn musubi_provider_attestation_snapshot_store_roundtrips_and_is_required() {
        assert!(
            World::default()
                .musubi_provider_bundle_attestations
                .view()
                .is_empty(),
            "a new first-release world starts with an empty attestation registry"
        );
        let (mut world, release, archive_id, _) = seeded_musubi_publication_snapshot();
        let attestation_key = seed_provider_attested_location(&mut world, &release, archive_id);
        let expected = world
            .musubi_provider_bundle_attestations
            .view()
            .get(&attestation_key)
            .cloned()
            .expect("seeded provider attestation record");
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_provider_bundle_attestations".to_owned(),
            json::to_value(&world.musubi_provider_bundle_attestations)
                .expect("serialize provider attestation snapshot store"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        let parsed: Storage<
            MusubiProviderBundleAttestationKeyV1,
            MusubiProviderBundleAttestationRecordV1,
        > = take_required(&mut map, "musubi_provider_bundle_attestations")
            .expect("decode provider attestation snapshot store");
        assert_eq!(parsed.view().get(&attestation_key), Some(&expected));
        assert!(map.is_empty());
        let error = take_required::<
            Storage<MusubiProviderBundleAttestationKeyV1, MusubiProviderBundleAttestationRecordV1>,
        >(
            &mut SnapshotJsonMap::from_owned(json::native::Map::new()),
            "musubi_provider_bundle_attestations",
        )
        .err()
        .expect("missing provider attestation snapshot store must fail");
        assert!(
            error
                .to_string()
                .contains("musubi_provider_bundle_attestations"),
            "unexpected missing-field error: {error}"
        );
    }
    #[test]
    fn musubi_maintainer_directory_snapshot_store_roundtrips_and_is_required() {
        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            iroha_data_model::musubi::MusubiPackageScopeV1::DataspaceRoot,
            "snapshot-package".parse().expect("package name"),
        );
        let owner = musubi_account(41);
        let invited = musubi_account(42);
        let accepted = MusubiPackageMemberV1 {
            package: package.clone(),
            account: owner.clone(),
            role: iroha_data_model::musubi::MusubiPackageRoleV1::Owner,
            accepted_at_height: 5,
            governance_revision: 1,
        };
        let invitation = MusubiMaintainerInvitationV1 {
            invite_id: MusubiInviteIdV1::new([0x2A; 32]),
            package,
            invited_by: owner,
            invited_account: invited,
            role: iroha_data_model::musubi::MusubiPackageRoleV1::Maintainer(
                iroha_data_model::musubi::MusubiMaintainerPermissionsV1 {
                    publish: true,
                    yank: false,
                    metadata: true,
                    archive_locations: false,
                },
            ),
            expected_governance_revision: 1,
            expires_at_height: 50,
            state: iroha_data_model::musubi::MusubiInvitationStateV1::Pending,
        };
        let accepted_entry = MusubiMaintainerDirectoryEntryV1::Accepted(accepted);
        let invitation_entry = MusubiMaintainerDirectoryEntryV1::PendingInvitation(invitation);
        let mut expected = Storage::default();
        expected.insert(accepted_entry.key(), accepted_entry.clone());
        expected.insert(invitation_entry.key(), invitation_entry.clone());
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_maintainer_directory".to_owned(),
            json::to_value(&expected).expect("serialize maintainer directory snapshot store"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        let parsed: Storage<MusubiMaintainerDirectoryKeyV1, MusubiMaintainerDirectoryEntryV1> =
            take_required(&mut map, "musubi_maintainer_directory")
                .expect("decode maintainer directory snapshot store");
        assert_eq!(
            parsed.view().get(&accepted_entry.key()),
            Some(&accepted_entry)
        );
        assert_eq!(
            parsed.view().get(&invitation_entry.key()),
            Some(&invitation_entry)
        );
        assert!(map.is_empty());
        let error = take_required::<
            Storage<MusubiMaintainerDirectoryKeyV1, MusubiMaintainerDirectoryEntryV1>,
        >(
            &mut SnapshotJsonMap::from_owned(json::native::Map::new()),
            "musubi_maintainer_directory",
        )
        .err()
        .expect("missing maintainer directory snapshot store must fail");
        assert!(
            error.to_string().contains("musubi_maintainer_directory"),
            "unexpected missing-field error: {error}"
        );
    }
    #[test]
    fn take_musubi_namespace_bindings_rejects_missing_malformed_and_key_mismatch() {
        let namespace: MusubiNamespaceV1 = "universal".parse().expect("namespace");
        let binding = MusubiNamespaceBindingV1 {
            namespace: namespace.clone(),
            home_dataspace: DataSpaceId::UNIVERSAL,
            scope: MusubiPackageScopeV1::DataspaceRoot,
            generation: 1,
        };
        let mut bindings = Storage::default();
        bindings.insert(namespace.clone(), binding.clone());
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_namespace_bindings".to_owned(),
            json::to_value(&bindings).expect("serialize bindings"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        let parsed =
            take_musubi_namespace_bindings(&mut map).expect("canonical namespace binding map");
        assert_eq!(parsed.view().get(&namespace), Some(&binding));
        let error = take_musubi_namespace_bindings(&mut SnapshotJsonMap::from_owned(
            json::native::Map::new(),
        ))
        .err()
        .expect("missing namespace binding map must fail");
        assert!(
            error.to_string().contains("musubi_namespace_bindings"),
            "unexpected missing-field error: {error}"
        );
        let mut malformed = Storage::default();
        malformed.insert(
            namespace.clone(),
            MusubiNamespaceBindingV1 {
                generation: 0,
                ..binding.clone()
            },
        );
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_namespace_bindings".to_owned(),
            json::to_value(&malformed).expect("serialize malformed binding"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        let error = take_musubi_namespace_bindings(&mut map)
            .err()
            .expect("malformed namespace binding must fail");
        assert!(
            error.to_string().contains("musubi_namespace_bindings"),
            "unexpected malformed-binding error: {error}"
        );
        let mismatched_namespace: MusubiNamespaceV1 =
            "other.universal".parse().expect("mismatched namespace");
        let mut mismatched = Storage::default();
        mismatched.insert(mismatched_namespace, binding);
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_namespace_bindings".to_owned(),
            json::to_value(&mismatched).expect("serialize mismatched binding"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        let error = take_musubi_namespace_bindings(&mut map)
            .err()
            .expect("namespace key/value mismatch must fail");
        assert!(
            error
                .to_string()
                .contains("does not match embedded namespace"),
            "unexpected key-mismatch error: {error}"
        );
    }
    #[test]
    fn take_musubi_domain_generations_requires_canonical_persisted_values() {
        let domain = DomainId::try_new("packages", "universal").expect("domain id");
        let error = take_musubi_domain_ownership_generations(&mut SnapshotJsonMap::from_owned(
            json::native::Map::new(),
        ))
        .err()
        .expect("missing generation map must fail");
        assert!(
            error
                .to_string()
                .contains("musubi_domain_ownership_generations"),
            "unexpected missing-field error: {error}"
        );
        let empty: Storage<DomainId, u64> = Storage::default();
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_domain_ownership_generations".to_owned(),
            json::to_value(&empty).expect("serialize empty generation map"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        assert!(
            take_musubi_domain_ownership_generations(&mut map)
                .expect("empty required map represents all generation-one domains")
                .view()
                .is_empty()
        );
        for generation in [0, 1] {
            let mut generations = Storage::default();
            generations.insert(domain.clone(), generation);
            let mut map = json::native::Map::new();
            map.insert(
                "musubi_domain_ownership_generations".to_owned(),
                json::to_value(&generations).expect("serialize invalid generation map"),
            );
            let mut map = SnapshotJsonMap::from_owned(map);
            let error = take_musubi_domain_ownership_generations(&mut map)
                .err()
                .unwrap_or_else(|| panic!("persisted generation {generation} must fail"));
            assert!(
                error.to_string().contains("noncanonical generation"),
                "unexpected generation {generation} error: {error}"
            );
        }
        let mut generations = Storage::default();
        generations.insert(domain.clone(), 4);
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_domain_ownership_generations".to_owned(),
            json::to_value(&generations).expect("serialize canonical generation map"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        let parsed = take_musubi_domain_ownership_generations(&mut map)
            .expect("persisted generation four is canonical");
        assert_eq!(parsed.view().get(&domain), Some(&4));
    }
    #[test]
    fn take_musubi_policy_revision_and_shortfall_fail_closed() {
        let policy = MusubiRegistryPolicyV1::default();
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_registry_policy".to_owned(),
            json::to_value(&Cell::new(policy.clone())).expect("serialize policy"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        assert_eq!(
            take_musubi_registry_policy(&mut map)
                .expect("canonical policy")
                .view()
                .get(),
            &policy
        );
        let mut invalid_policy = policy;
        invalid_policy.revision = 0;
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_registry_policy".to_owned(),
            json::to_value(&Cell::new(invalid_policy)).expect("serialize invalid policy"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        let error = take_musubi_registry_policy(&mut map)
            .err()
            .expect("invalid policy must fail");
        assert!(
            error.to_string().contains("musubi_registry_policy"),
            "unexpected policy error: {error}"
        );
        assert!(
            take_musubi_registry_policy(
                &mut SnapshotJsonMap::from_owned(json::native::Map::new(),)
            )
            .is_err(),
            "missing policy must fail"
        );
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_resolver_index_revision".to_owned(),
            json::to_value(&Cell::new(MusubiResolverIndexRevisionV1::default()))
                .expect("serialize resolver revision"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        assert_eq!(
            take_musubi_resolver_index_revision(&mut map)
                .expect("canonical resolver revision")
                .view()
                .get()
                .get(),
            1
        );
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_resolver_index_revision".to_owned(),
            json::to_value(&Cell::new(MusubiResolverIndexRevisionV1(0)))
                .expect("serialize zero resolver revision"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        let error = take_musubi_resolver_index_revision(&mut map)
            .err()
            .expect("zero resolver revision must fail");
        assert!(
            error.to_string().contains("musubi_resolver_index_revision"),
            "unexpected resolver revision error: {error}"
        );
        assert!(
            take_musubi_resolver_index_revision(&mut SnapshotJsonMap::from_owned(
                json::native::Map::new(),
            ))
            .is_err(),
            "missing resolver revision must fail"
        );
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_replication_shortfall_releases".to_owned(),
            json::to_value(&Cell::new(7_u64)).expect("serialize shortfall count"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        assert_eq!(
            *take_musubi_replication_shortfall_releases(&mut map)
                .expect("canonical shortfall count")
                .view()
                .get(),
            7
        );
        let error = take_musubi_replication_shortfall_releases(&mut SnapshotJsonMap::from_owned(
            json::native::Map::new(),
        ))
        .err()
        .expect("missing shortfall count must fail cleanly");
        assert!(
            error
                .to_string()
                .contains("musubi_replication_shortfall_releases"),
            "unexpected missing-shortfall error: {error}"
        );
    }
    #[test]
    fn musubi_resolver_checkpoint_keys_use_canonical_nonzero_decimal() {
        use mv::json::JsonKeyCodec;
        let revision = MusubiResolverIndexRevisionV1::new(42).expect("revision forty-two");
        let mut encoded = String::new();
        revision.encode_json_key(&mut encoded);
        assert_eq!(encoded, "\"42\"");
        assert_eq!(
            MusubiResolverIndexRevisionV1::decode_json_key("42").expect("canonical revision key"),
            revision
        );
        for invalid in ["0", "00", "01", "+1", " 1", "1 "] {
            assert!(
                MusubiResolverIndexRevisionV1::decode_json_key(invalid).is_err(),
                "noncanonical revision key must fail: {invalid:?}"
            );
        }
    }
    #[test]
    fn musubi_resolver_checkpoints_are_required_and_canonically_anchored() {
        let revision = MusubiResolverIndexRevisionV1::new(1).expect("revision one");
        let checkpoint = MusubiRegistrySnapshotV1 {
            finalized_height: 1,
            finalized_block_hash: [0xA1; 32],
            index_revision: 1,
        };
        let mut checkpoints = Storage::default();
        checkpoints.insert(revision, checkpoint.clone());
        let mut map = json::native::Map::new();
        map.insert(
            "musubi_resolver_index_checkpoints".to_owned(),
            json::to_value(&checkpoints).expect("serialize resolver checkpoints"),
        );
        let mut map = SnapshotJsonMap::from_owned(map);
        let decoded = take_musubi_resolver_index_checkpoints(&mut map)
            .expect("checkpoint store is required and decodable");
        validate_musubi_resolver_checkpoint_structure(&decoded, 1)
            .expect("canonical sparse checkpoint structure");
        assert!(
            take_musubi_resolver_index_checkpoints(&mut SnapshotJsonMap::from_owned(
                json::native::Map::new(),
            ))
            .is_err(),
            "the clean schema must reject snapshots without checkpoint history"
        );
        let mut mismatched = Storage::default();
        mismatched.insert(
            MusubiResolverIndexRevisionV1::new(2).expect("revision two"),
            checkpoint,
        );
        assert!(
            validate_musubi_resolver_checkpoint_structure(&mismatched, 2).is_err(),
            "checkpoint keys must match embedded revisions"
        );
        let mut world = World::default();
        world.musubi_resolver_index_checkpoints = decoded;
        let canonical_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA1; Hash::LENGTH]));
        validate_musubi_resolver_checkpoint_anchors(&world, std::slice::from_ref(&canonical_hash))
            .expect("checkpoint binds its canonical finalized block");
        let fabricated_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xB2; Hash::LENGTH]));
        assert!(
            validate_musubi_resolver_checkpoint_anchors(
                &world,
                std::slice::from_ref(&fabricated_hash)
            )
            .is_err(),
            "a fabricated checkpoint tuple must fail restoration"
        );
    }
    #[test]
    fn musubi_governance_requires_exact_fingerprint_and_execution_boundary() {
        let genesis = MusubiRegistryPolicyV1::default();
        let successor = musubi_policy_successor(&genesis, MusubiRegistryAdmissionModeV1::Closed);
        let action = MusubiParliamentActionV1::SetRegistryPolicy(MusubiSetRegistryPolicyActionV1 {
            policy: successor.clone(),
            expected_revision: genesis.revision,
        });
        let decision_id = ProposalKind::MusubiRegistryGovernance(action.clone()).fingerprint();
        let mut world = World::default();
        world.musubi_registry_policy = Cell::new(successor.clone());
        retain_musubi_consumption(&mut world, action.clone(), 10, 20, 20);
        validate_musubi_governance_provenance(&world)
            .expect("execution exactly at the boundary must pass");
        let mut consumption = world
            .musubi_governance_decisions
            .view()
            .get(&decision_id)
            .copied()
            .expect("retained consumption");
        consumption.consumed_at_height = 19;
        world
            .musubi_governance_decisions
            .insert(decision_id, consumption);
        let error = validate_musubi_governance_provenance(&world)
            .expect_err("pre-boundary consumption must fail");
        assert!(error.to_string().contains("consumed before"), "{error}");
        let wrong_id = if decision_id == [0xA5; 32] {
            [0x5A; 32]
        } else {
            [0xA5; 32]
        };
        let mut mismatched = World::default();
        mismatched.musubi_registry_policy = Cell::new(successor);
        retain_musubi_consumption_as(&mut mismatched, wrong_id, action, 10, 20, 20);
        let error = validate_musubi_governance_provenance(&mismatched)
            .expect_err("proposal fingerprint mismatch must fail");
        assert!(error.to_string().contains("fingerprint"), "{error}");
    }
    #[test]
    fn musubi_owner_recovery_current_projection_is_exact() {
        let mut world = World::default();
        let package = musubi_package("current-recovery");
        let owner = musubi_account(71);
        let member = seed_current_musubi_package(&mut world, &package, &owner, 2, 20);
        let action = MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
            package: package.clone(),
            owners: vec![owner.clone()],
            expected_revision: 1,
        });
        retain_musubi_consumption(&mut world, action, 10, 20, 20);
        validate_musubi_governance_provenance(&world).expect("exact current recovery projection");
        let canonical_package = world
            .musubi_packages
            .view()
            .get(&package)
            .cloned()
            .expect("package");
        let mut wrong_package = canonical_package.clone();
        let unrelated = musubi_account(72);
        wrong_package.owners = vec![unrelated.clone()];
        wrong_package.member_accounts = vec![unrelated];
        wrong_package
            .validate()
            .expect("structurally valid package");
        world.musubi_packages.insert(package.clone(), wrong_package);
        assert!(
            validate_musubi_governance_provenance(&world)
                .expect_err("wrong owner set")
                .to_string()
                .contains("owner-recovery projection")
        );
        world
            .musubi_packages
            .insert(package.clone(), canonical_package);
        let key = member.key();
        {
            let mut members = world.musubi_package_members.block();
            members.remove(key.clone());
            members.commit();
        }
        assert!(
            validate_musubi_governance_provenance(&world)
                .expect_err("missing recovered owner member")
                .to_string()
                .contains("exact member projection")
        );
        world
            .musubi_package_members
            .insert(key.clone(), member.clone());
        let mut wrong_member = member.clone();
        wrong_member.role = MusubiPackageRoleV1::Maintainer(MusubiMaintainerPermissionsV1 {
            publish: true,
            yank: false,
            metadata: false,
            archive_locations: false,
        });
        world
            .musubi_package_members
            .insert(key.clone(), wrong_member);
        assert!(
            validate_musubi_governance_provenance(&world)
                .expect_err("wrong recovered owner role")
                .to_string()
                .contains("owner-recovery projection")
        );
        let mut wrong_member = member.clone();
        wrong_member.governance_revision = 1;
        world
            .musubi_package_members
            .insert(key.clone(), wrong_member);
        assert!(
            validate_musubi_governance_provenance(&world)
                .expect_err("wrong recovered owner revision")
                .to_string()
                .contains("owner-recovery projection")
        );
        let mut wrong_member = member;
        wrong_member.accepted_at_height = 21;
        world.musubi_package_members.insert(key, wrong_member);
        assert!(
            validate_musubi_governance_provenance(&world)
                .expect_err("wrong recovered owner acceptance height")
                .to_string()
                .contains("consumption height")
        );
    }
    #[test]
    fn musubi_owner_recovery_rejects_duplicate_result_revision_but_accepts_later_state() {
        let mut world = World::default();
        let package = musubi_package("recovery-history");
        let current_owner = musubi_account(80);
        seed_current_musubi_package(&mut world, &package, &current_owner, 3, 30);
        let first = MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
            package: package.clone(),
            owners: vec![musubi_account(81)],
            expected_revision: 1,
        });
        retain_musubi_consumption(&mut world, first, 10, 20, 20);
        validate_musubi_governance_provenance(&world)
            .expect("later current revision need not reproduce historical owners");
        let duplicate =
            MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
                package,
                owners: vec![musubi_account(82)],
                expected_revision: 1,
            });
        retain_musubi_consumption(&mut world, duplicate, 11, 21, 21);
        let error = validate_musubi_governance_provenance(&world)
            .expect_err("two recoveries cannot claim the same result revision");
        assert!(error.to_string().contains("result revision"), "{error}");
    }
    #[test]
    fn musubi_policy_consumption_heights_are_nondecreasing() {
        let genesis = MusubiRegistryPolicyV1::default();
        let second = musubi_policy_successor(&genesis, MusubiRegistryAdmissionModeV1::Closed);
        let third = musubi_policy_successor(&second, MusubiRegistryAdmissionModeV1::Open);
        let first_action =
            MusubiParliamentActionV1::SetRegistryPolicy(MusubiSetRegistryPolicyActionV1 {
                policy: second.clone(),
                expected_revision: genesis.revision,
            });
        let first_id = ProposalKind::MusubiRegistryGovernance(first_action.clone()).fingerprint();
        let second_action =
            MusubiParliamentActionV1::SetRegistryPolicy(MusubiSetRegistryPolicyActionV1 {
                policy: third.clone(),
                expected_revision: second.revision,
            });
        let mut world = World::default();
        world.musubi_registry_policy = Cell::new(third);
        retain_musubi_consumption(&mut world, first_action, 10, 20, 21);
        retain_musubi_consumption(&mut world, second_action, 11, 21, 21);
        validate_musubi_governance_provenance(&world)
            .expect("equal same-block consumption heights are nondecreasing");
        let mut first = world
            .musubi_governance_decisions
            .view()
            .get(&first_id)
            .copied()
            .expect("first policy consumption");
        first.consumed_at_height = 22;
        first.validate().expect("still individually valid");
        world.musubi_governance_decisions.insert(first_id, first);
        let error = validate_musubi_governance_provenance(&world)
            .expect_err("policy revision history cannot move backward in height");
        assert!(error.to_string().contains("consumption heights"), "{error}");
    }
    #[test]
    fn musubi_owner_recovery_consumption_heights_are_nondecreasing() {
        let mut world = World::default();
        let package = musubi_package("recovery-height-history");
        let current_owner = musubi_account(90);
        seed_current_musubi_package(&mut world, &package, &current_owner, 3, 21);
        let first = MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
            package: package.clone(),
            owners: vec![musubi_account(91)],
            expected_revision: 1,
        });
        retain_musubi_consumption(&mut world, first, 10, 20, 22);
        let second = MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
            package,
            owners: vec![current_owner],
            expected_revision: 2,
        });
        retain_musubi_consumption(&mut world, second, 11, 21, 21);
        let error = validate_musubi_governance_provenance(&world)
            .expect_err("owner-recovery history cannot move backward in execution height");
        assert!(error.to_string().contains("consumption heights"), "{error}");
    }
    #[test]
    fn alias_retarget_history_binds_exact_decision_consumption_height() {
        const CONSUMED_AT: u64 = 30;
        let mut world = World::default();
        let alias: MusubiAliasNameV1 = "stable".parse().expect("alias");
        let previous_target = musubi_package("previous");
        let target = musubi_package("replacement");
        let action = MusubiParliamentActionV1::RetargetAlias(MusubiRetargetAliasV1 {
            alias: alias.clone(),
            target: target.clone(),
            expected_revision: 1,
        });
        let action_digest = retain_musubi_consumption(&mut world, action, 10, 20, CONSUMED_AT);
        let mut history = MusubiAliasHistoryEntryV1 {
            alias,
            revision: 2,
            action: MusubiAliasHistoryActionV1::ParliamentRetarget,
            previous_target: Some(previous_target),
            target,
            governance_action: Some(action_digest),
            finalized_height: CONSUMED_AT,
        };
        history.validate().expect("valid alias history fixture");
        let key = history.key();
        world
            .musubi_alias_history
            .insert(key.clone(), history.clone());
        validate_musubi_governance_provenance(&world)
            .expect("exact consumption-height binding must pass");
        history.finalized_height = CONSUMED_AT + 1;
        history
            .validate()
            .expect("height mismatch remains structurally valid");
        world.musubi_alias_history.insert(key, history);
        let error = validate_musubi_governance_provenance(&world)
            .expect_err("a different finalized height must fail closed");
        assert!(
            error.to_string().contains("decision consumption height"),
            "unexpected alias-height diagnostic: {error}"
        );
    }
    #[test]
    fn artifact_takedown_binds_exact_decision_consumption_height() {
        const CONSUMED_AT: u64 = 30;
        let mut world = World::default();
        let release = MusubiReleaseIdV1::new(
            musubi_package("withdrawn"),
            "1.2.3".parse().expect("release version"),
        );
        let reason: MusubiReasonV1 = "security response".parse().expect("bounded reason");
        let action = MusubiParliamentActionV1::TakedownArtifact(MusubiTakedownArtifactActionV1 {
            release: release.clone(),
            reason: reason.clone(),
            expected_artifact_governance_revision: 1,
        });
        let action_digest = retain_musubi_consumption(&mut world, action, 10, 20, CONSUMED_AT);
        let record = musubi_release_record(
            release.clone(),
            MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
                action_digest,
                reason,
                applied_at_height: CONSUMED_AT,
            }),
            2,
        );
        world
            .musubi_releases
            .insert(release.clone(), record.clone());
        validate_musubi_governance_provenance(&world)
            .expect("exact consumption-height binding must pass");
        let mut mismatched = record;
        let MusubiArtifactGovernanceStateV1::TakenDown(takedown) =
            &mut mismatched.artifact_governance
        else {
            unreachable!("takedown fixture")
        };
        takedown.applied_at_height = CONSUMED_AT + 1;
        mismatched
            .validate()
            .expect("height mismatch remains structurally valid");
        world.musubi_releases.insert(release, mismatched);
        let error = validate_musubi_governance_provenance(&world)
            .expect_err("a different takedown height must fail closed");
        assert!(
            error.to_string().contains("decision consumption height"),
            "unexpected takedown-height diagnostic: {error}"
        );
    }
    #[test]
    fn musubi_governance_provenance_reconstructs_policy_and_requires_proposals() {
        let mut world = World::default();
        validate_musubi_governance_provenance(&world)
            .expect("genesis Musubi policy has an empty decision history");
        let mut unexplained_policy = MusubiRegistryPolicyV1::default();
        unexplained_policy.revision += 1;
        unexplained_policy.mode = iroha_data_model::musubi::MusubiRegistryAdmissionModeV1::Closed;
        world.musubi_registry_policy = Cell::new(unexplained_policy);
        let error = validate_musubi_governance_provenance(&world)
            .expect_err("policy state without retained decisions must fail closed");
        assert!(
            error.to_string().contains("exact result"),
            "unexpected unexplained-policy diagnostic: {error}"
        );
        let mut world = World::default();
        let mut successor = MusubiRegistryPolicyV1::default();
        successor.revision += 1;
        successor.mode = iroha_data_model::musubi::MusubiRegistryAdmissionModeV1::Closed;
        let action = iroha_data_model::musubi::MusubiParliamentActionV1::SetRegistryPolicy(
            iroha_data_model::musubi::MusubiSetRegistryPolicyActionV1 {
                policy: successor,
                expected_revision: 1,
            },
        );
        let decision_id = [0x31; 32];
        world.musubi_governance_decisions.insert(
            decision_id,
            MusubiGovernanceDecisionConsumptionV1 {
                decision: MusubiGovernanceDecisionV1 {
                    decision_id,
                    action_digest: action.action_digest(),
                    enacted_at_height: 10,
                    execute_after_height: 20,
                },
                minimum_enactment_delay: 10,
                consumed_at_height: 20,
            },
        );
        let error = validate_musubi_governance_provenance(&world)
            .expect_err("consumed decision without its proposal must fail closed");
        assert!(
            error
                .to_string()
                .contains("no retained governance proposal"),
            "unexpected missing-proposal diagnostic: {error}"
        );
    }
}
