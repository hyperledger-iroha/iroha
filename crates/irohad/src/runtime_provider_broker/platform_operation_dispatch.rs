#[expect(
    clippy::too_many_lines,
    reason = "the fixed V1 operation dispatch matrix is exhaustive"
)]
fn dispatch_server_operation_with_session(
    state: &BrokerServerStateV1,
    pop_session: &mut PopBrokerServerSessionV1,
    request: &OperationRequestV1,
) -> Result<ScrubbedBytes, BrokerError> {
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    requalify()?;
    let moderation_quarantine_slot =
        IrohaRuntimeProviderSlotV1::ModerationQuarantineKeyWrapper.wire_id();
    let moderation_transaction_signer_slot =
        IrohaRuntimeProviderSlotV1::ModerationTransactionSigner.wire_id();
    let moderation_settlement_handoff_slot =
        IrohaRuntimeProviderSlotV1::ModerationSettlementHandoff.wire_id();
    let moderation_publication_handoff_slot =
        IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff.wire_id();
    let moderation_panel_notification_slot =
        IrohaRuntimeProviderSlotV1::ModerationPanelNotification.wire_id();
    let privacy_cycle_prf_slot = IrohaRuntimeProviderSlotV1::PrivacyCyclePrfProvider.wire_id();
    let privacy_release_anchor_slot = IrohaRuntimeProviderSlotV1::PrivacyReleaseAnchor.wire_id();
    let transparency_leader_lease_slot =
        IrohaRuntimeProviderSlotV1::TransparencyLeaderLease.wire_id();
    let fenced_privacy_publisher_slot =
        IrohaRuntimeProviderSlotV1::FencedPrivacyPublisher.wire_id();
    let fenced_privacy_head_reader_slot =
        IrohaRuntimeProviderSlotV1::FencedPrivacyHeadReader.wire_id();
    let governance_signer_slot = IrohaRuntimeProviderSlotV1::GovernanceDagSigner.wire_id();
    let governance_ipfs_auth_slot =
        IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator.wire_id();
    let governance_head_auth_slot =
        IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator.wire_id();
    let governance_checkpoint_slot =
        IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore.wire_id();
    let stream_token_slot = IrohaRuntimeProviderSlotV1::StreamTokenSigner.wire_id();
    let stream_token_gateway_admission_slot =
        IrohaRuntimeProviderSlotV1::StreamTokenGatewayAdmission.wire_id();
    let appeal_signer_slot = IrohaRuntimeProviderSlotV1::AppealFinanceTransactionSigner.wire_id();
    let appeal_checkpoint_slot = IrohaRuntimeProviderSlotV1::AppealFinanceCheckpoint.wire_id();
    let potr_gateway_slot = IrohaRuntimeProviderSlotV1::PotrGatewaySigner.wire_id();
    let potr_provider_slot = IrohaRuntimeProviderSlotV1::PotrProviderSigner.wire_id();
    let gateway_acme_slot = IrohaRuntimeProviderSlotV1::GatewayAcmeClient.wire_id();
    let gateway_compliance_slot =
        IrohaRuntimeProviderSlotV1::GatewayComplianceFeedTransport.wire_id();
    let pop_registry_slot = IrohaRuntimeProviderSlotV1::PopCredentialProviderRegistry.wire_id();
    let por_replay_archive_slot = IrohaRuntimeProviderSlotV1::PorFinalizedReplayArchive.wire_id();
    let provider_resolver_slot =
        IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver.wire_id();
    let provider_source_slot =
        IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource.wire_id();
    let provider_signer_slot = IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSigner.wire_id();
    let provider_checkpoint_slot =
        IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore.wire_id();
    let provider_retention_slot =
        IrohaRuntimeProviderSlotV1::ProviderIngestRetentionAuthority.wire_id();
    let reputation_retention_slot =
        IrohaRuntimeProviderSlotV1::ReputationFinalizedArchiveRetentionAuthority.wire_id();
    let reputation_journal_slot =
        IrohaRuntimeProviderSlotV1::ReputationJournalTransactionSubmitter.wire_id();
    let reputation_threshold_slot = IrohaRuntimeProviderSlotV1::ReputationThresholdSigner.wire_id();
    let reputation_governance_slot = IrohaRuntimeProviderSlotV1::ReputationGovernanceDag.wire_id();
    let reputation_checkpoint_slot =
        IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint.wire_id();
    let billing_finalized_query_slot = IrohaRuntimeProviderSlotV1::BillingFinalizedQuery.wire_id();
    let billing_journal_verifier_slot =
        IrohaRuntimeProviderSlotV1::BillingJournalVerifier.wire_id();
    let billing_statement_signer_slot =
        IrohaRuntimeProviderSlotV1::BillingStatementSigner.wire_id();
    let billing_statement_publisher_slot =
        IrohaRuntimeProviderSlotV1::BillingStatementPublisher.wire_id();
    let billing_acknowledgement_authority_slot =
        IrohaRuntimeProviderSlotV1::BillingAcknowledgementAuthority.wire_id();
    let billing_epoch_witness_store_slot =
        IrohaRuntimeProviderSlotV1::BillingEpochWitnessStore.wire_id();
    let evidence_webauthn_slot = IrohaRuntimeProviderSlotV1::EvidenceViewerWebAuthn.wire_id();
    let evidence_grants_slot = IrohaRuntimeProviderSlotV1::EvidenceViewerGrantAuthority.wire_id();
    let evidence_receipt_signer_slot =
        IrohaRuntimeProviderSlotV1::EvidenceViewerReceiptSigner.wire_id();
    let evidence_erasure_slot = IrohaRuntimeProviderSlotV1::EvidenceViewerErasure.wire_id();
    let evidence_checkpoint_slot =
        IrohaRuntimeProviderSlotV1::EvidenceViewerCheckpointStore.wire_id();
    let moderation_checkpoint_slot =
        IrohaRuntimeProviderSlotV1::ModerationCheckpointStore.wire_id();
    let moderation_panel_notification_archive_slot =
        IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id();
    let evidence_archive_slot =
        IrohaRuntimeProviderSlotV1::EvidenceViewerCompactionArchive.wire_id();
    let evidence_transparency_publisher_slot =
        IrohaRuntimeProviderSlotV1::EvidenceViewerTransparencyPublisher.wire_id();
    let soracloud_runtime_signer_slot =
        IrohaRuntimeProviderSlotV1::SoracloudRuntimeMutationSigner.wire_id();
    let bootle_lantern_issuance_slot =
        IrohaRuntimeProviderSlotV1::BootleLanternIssuanceProviderRegistry.wire_id();
    let global_beacon_partial_signer_slot =
        IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner.wire_id();
    let parliament_tle_partial_release_signer_slot =
        IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner.wire_id();
    let result = match (request.binding.slot, request.operation) {
        (slot, OPERATION_QUALIFY_V1)
            if slot == global_beacon_partial_signer_slot
                || slot == parliament_tle_partial_release_signer_slot =>
        {
            let qualification = if slot == global_beacon_partial_signer_slot {
                broker_backend!(state, global_beacon_partial_signer)
                    .qualification()
                    .map_err(|_| BrokerError::StaleOrRevoked)?
            } else {
                broker_backend!(state, parliament_tle_partial_release_signer)
                    .qualification()
                    .map_err(|_| BrokerError::StaleOrRevoked)?
            };
            if qualification.test_marked
                || qualification.revision == 0
                || qualification.policy_digest == [0; 32]
            {
                return Err(BrokerError::StaleOrRevoked);
            }
            encode_canonical(
                &QualificationResultWireV1 {
                    revision: qualification.revision,
                    policy_digest: qualification.policy_digest,
                },
                MAX_CONSENSUS_SIGNER_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_GLOBAL_BEACON_PARTIAL_SIGN_V1)
            if slot == global_beacon_partial_signer_slot =>
        {
            let (_, mut aggregator) =
                decode_global_beacon_partial_sign_request(&request.payload, &state.network_id)?;
            let backend = broker_backend!(state, global_beacon_partial_signer);
            let partial = backend
                .sign_partial(aggregator.session(), aggregator.payload())
                .map_err(|_| BrokerError::Unavailable)?;
            aggregator
                .accept_partial(partial)
                .map_err(|_| BrokerError::Rejected)?;
            requalify()?;
            encode_canonical(
                &GlobalBeaconPartialSignResultWireV1 { partial },
                MAX_CONSENSUS_SIGNER_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_PARLIAMENT_TLE_PARTIAL_RELEASE_SIGN_V1)
            if slot == parliament_tle_partial_release_signer_slot =>
        {
            let (_, projection) = decode_parliament_tle_partial_release_sign_request(
                &request.payload,
                &state.network_id,
            )?;
            let backend = broker_backend!(state, parliament_tle_partial_release_signer);
            let partial = backend
                .sign_projected_partial_release(&projection)
                .map_err(|_| BrokerError::Unavailable)?;
            verify_parliament_tle_partial_release_result(&projection, &partial)?;
            requalify()?;
            encode_canonical(
                &ParliamentTlePartialReleaseSignResultWireV1 { partial },
                MAX_CONSENSUS_SIGNER_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_QUALIFY_V1)
            if slot == moderation_panel_notification_archive_slot =>
        {
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
                || qualification.policy_digest()
                    != required_binding_value!(&request.binding, policy_digest)
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
        (slot, OPERATION_QUALIFY_V1) if slot == bootle_lantern_issuance_slot => {
            let qualification = broker_backend!(state, bootle_lantern_issuance)
                .qualification()
                .map_err(|error| {
                    match error {
                    iroha_torii::privacy_issuance_api::
                        BootleLanternIssuanceRuntimeProviderRegistryErrorV1::Unavailable =>
                    {
                        BrokerError::Unavailable
                    }
                    iroha_torii::privacy_issuance_api::
                        BootleLanternIssuanceRuntimeProviderRegistryErrorV1::StaleOrRevoked =>
                    {
                        BrokerError::StaleOrRevoked
                    }
                    iroha_torii::privacy_issuance_api::
                        BootleLanternIssuanceRuntimeProviderRegistryErrorV1::RejectedBindings =>
                    {
                        BrokerError::BindingMismatch
                    }
                }
                })?;
            encode_canonical(
                &QualificationResultWireV1 {
                    revision: qualification.revision,
                    policy_digest: qualification.policy_digest,
                },
                MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_BOOTLE_LANTERN_ISSUANCE_AUTHENTICATE_V1)
            if slot == bootle_lantern_issuance_slot =>
        {
            let authenticate = decode_canonical::<BootleLanternAuthenticateRequestWireV1>(
                &request.payload,
                MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
            )?;
            let action = bootle_lantern_action_from_wire(authenticate.action)?;
            let outcome = broker_backend!(state, bootle_lantern_issuance).authenticate(
                &authenticate.opaque_credential,
                action,
                authenticate.request_binding,
                authenticate.committed_height,
            );
            requalify()?;
            let principal = outcome.map_err(|error| {
                match error {
                iroha_torii::privacy_issuance_api::
                    BootleLanternIssuanceAuthenticationErrorV1::Denied =>
                {
                    BrokerError::Rejected
                }
                iroha_torii::privacy_issuance_api::
                    BootleLanternIssuanceAuthenticationErrorV1::Unavailable =>
                {
                    BrokerError::Unavailable
                }
            }
            })?;
            encode_canonical(
                &BootleLanternAuthenticatedPrincipalWireV1 {
                    principal_digest: principal.principal_digest,
                    issued_at_height: principal.issued_at_height,
                    expires_at_height: principal.expires_at_height,
                },
                MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_BOOTLE_LANTERN_ISSUANCE_PREPARE_AUTHORIZATION_V1)
            if slot == bootle_lantern_issuance_slot =>
        {
            let prepare = decode_canonical::<BootleLanternPrepareAuthorizationRequestWireV1>(
                &request.payload,
                MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
            )?;
            let outcome = broker_backend!(state, bootle_lantern_issuance).prepare_authorization(
                &prepare.context,
                prepare.canonical_genesis_hash,
                &prepare.policy,
                prepare.requester_authorization_digest,
                prepare.issued_at_height,
                prepare.expires_at_height,
            );
            requalify()?;
            let authorization = outcome.map_err(|error| {
                match error {
                crate::runtime_provider_broker::
                    BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest =>
                {
                    BrokerError::Rejected
                }
                crate::runtime_provider_broker::
                    BootleLanternIssuanceBrokerBackendErrorV1::PolicyMismatch =>
                {
                    BrokerError::StaleOrRevoked
                }
                crate::runtime_provider_broker::
                    BootleLanternIssuanceBrokerBackendErrorV1::Unavailable =>
                {
                    BrokerError::Unavailable
                }
            }
            })?;
            iroha_core::privacy_engines::bootle_lantern::issuer::
                issuer_validate_prepared_blind_issuance_authorization_v1(
                    &prepare.context,
                    prepare.canonical_genesis_hash,
                    &prepare.policy,
                    &authorization,
                )
                .map_err(|_| BrokerError::Rejected)?;
            let authorization = authorization.encode().map_err(|_| BrokerError::Rejected)?;
            if authorization.len() != BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 {
                return Err(BrokerError::Rejected);
            }
            encode_canonical(
                &BootleLanternAuthorizationWireV1 { authorization },
                MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_BOOTLE_LANTERN_ISSUANCE_VALIDATE_REQUEST_V1)
            if slot == bootle_lantern_issuance_slot =>
        {
            let (issue, authorization) = decode_bootle_lantern_issue_request(
                &request.payload,
                &request.binding,
                &state.network_id,
            )?;
            let expected = iroha_core::privacy_engines::bootle_lantern::issuer::
                issuer_validate_blind_issuance_request_encoded_v1(
                    &issue.context,
                    issue.canonical_genesis_hash,
                    &issue.policy,
                    &authorization,
                    &issue.request,
                    issue.current_height,
                )
                .map_err(|_| BrokerError::Rejected)?;
            let outcome = broker_backend!(state, bootle_lantern_issuance).validate_request(
                &issue.context,
                issue.canonical_genesis_hash,
                &issue.policy,
                &authorization,
                &issue.request,
                issue.current_height,
            );
            requalify()?;
            let digest = outcome.map_err(|error| {
                match error {
                crate::runtime_provider_broker::
                    BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest =>
                {
                    BrokerError::Rejected
                }
                crate::runtime_provider_broker::
                    BootleLanternIssuanceBrokerBackendErrorV1::PolicyMismatch =>
                {
                    BrokerError::StaleOrRevoked
                }
                crate::runtime_provider_broker::
                    BootleLanternIssuanceBrokerBackendErrorV1::Unavailable =>
                {
                    BrokerError::Unavailable
                }
            }
            })?;
            if digest == [0; 32] || digest != expected {
                return Err(BrokerError::Rejected);
            }
            encode_canonical(&digest, MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BOOTLE_LANTERN_ISSUANCE_ISSUE_VALIDATED_V1)
            if slot == bootle_lantern_issuance_slot =>
        {
            let (issue, authorization) = decode_bootle_lantern_issue_request(
                &request.payload,
                &request.binding,
                &state.network_id,
            )?;
            let outcome = broker_backend!(state, bootle_lantern_issuance).issue_validated(
                &issue.context,
                issue.canonical_genesis_hash,
                &issue.policy,
                &authorization,
                &issue.request,
                issue.current_height,
            );
            requalify()?;
            let response = outcome.map_err(|error| {
                match error {
                crate::runtime_provider_broker::
                    BootleLanternIssuanceBrokerBackendErrorV1::InvalidRequest =>
                {
                    BrokerError::Rejected
                }
                crate::runtime_provider_broker::
                    BootleLanternIssuanceBrokerBackendErrorV1::PolicyMismatch =>
                {
                    BrokerError::StaleOrRevoked
                }
                crate::runtime_provider_broker::
                    BootleLanternIssuanceBrokerBackendErrorV1::Unavailable =>
                {
                    BrokerError::Unavailable
                }
            }
            })?;
            let response = response.encode().map_err(|_| BrokerError::Rejected)?;
            if response.len() != BOOTLE_LANTERN_RESPONSE_BYTES_V1 {
                return Err(BrokerError::Rejected);
            }
            iroha_core::privacy_engines::bootle_lantern::issuer::
                issuer_validate_cached_blind_issuance_response_encoded_v1(
                    &issue.context,
                    issue.canonical_genesis_hash,
                    &issue.policy,
                    &authorization,
                    &issue.request,
                    &response,
                )
                .map_err(|_| BrokerError::Rejected)?;
            encode_canonical(
                &BootleLanternIssuanceResponseWireV1 { response },
                MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_QUALIFY_V1) if slot == moderation_quarantine_slot => {
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
        (slot, OPERATION_MODERATION_QUARANTINE_WRAP_DEK_V1)
            if slot == moderation_quarantine_slot =>
        {
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
        (slot, OPERATION_MODERATION_QUARANTINE_UNWRAP_DEK_V1)
            if slot == moderation_quarantine_slot =>
        {
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
        (slot, OPERATION_QUALIFY_V1) if slot == provider_source_slot => {
            let source = broker_backend!(state, provider_ingest_authenticated_source);
            let qualification = source
                .qualification()
                .map_err(|_| BrokerError::StaleOrRevoked)?;
            encode_canonical(
                &ProviderIngestRuntimeQualificationWireV1 {
                    revision: qualification.revision,
                    policy_digest: qualification.policy_digest,
                },
                MAX_OPERATION_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_PROVIDER_INGEST_SOURCE_READINESS_V1) if slot == provider_source_slot => {
            broker_backend!(state, provider_ingest_authenticated_source)
                .check_readiness()
                .map_err(|error| match error {
                    sorafs_node::ProviderIngestSourceFetchErrorV1::Unavailable => {
                        BrokerError::Unavailable
                    }
                    sorafs_node::ProviderIngestSourceFetchErrorV1::ContentRejected => {
                        BrokerError::Rejected
                    }
                    sorafs_node::ProviderIngestSourceFetchErrorV1::Rejected => {
                        BrokerError::StaleOrRevoked
                    }
                })?;
            encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
        }
        (slot, OPERATION_QUALIFY_V1) if slot == provider_resolver_slot => {
            let resolver = broker_backend!(state, provider_ingest_signer_resolver);
            let qualification = resolver
                .qualification()
                .map_err(|_| BrokerError::StaleOrRevoked)?;
            let signer_binding = resolver
                .signer_binding()
                .map_err(|_| BrokerError::StaleOrRevoked)?;
            let signer_binding =
                ProviderIngestSignerBindingWireV1::try_from_binding(&signer_binding)
                    .map_err(|_| BrokerError::StaleOrRevoked)?;
            encode_canonical(
                &ProviderIngestResolverQualificationWireV1 {
                    revision: qualification.revision,
                    policy_digest: qualification.policy_digest,
                    signer_binding,
                },
                MAX_OPERATION_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_QUALIFY_V1) if slot == soracloud_runtime_signer_slot => {
            let signer = qualified_soracloud_runtime_signer(state, &request.binding)?;
            let qualification = signer
                .qualification()
                .map_err(|_| BrokerError::Unavailable)?;
            encode_canonical(
                &SoracloudSignerQualificationWireV1 {
                    revision: qualification.revision(),
                    policy_digest: qualification.policy_digest(),
                    active: qualification.active(),
                    test_only: qualification.test_only(),
                },
                MAX_OPERATION_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_QUALIFY_V1)
            if slot == governance_ipfs_auth_slot || slot == governance_head_auth_slot =>
        {
            let authenticator = if slot == governance_ipfs_auth_slot {
                state.backends.governance_dag_ipfs_authenticator.as_ref()
            } else {
                state.backends.governance_dag_head_authenticator.as_ref()
            }
            .ok_or(BrokerError::BindingMismatch)?;
            let qualification = authenticator
                .ingress_qualification()
                .map_err(|_| BrokerError::StaleOrRevoked)?;
            let expected_binding =
                governance_request_ingress_binding_from_provider_binding(&request.binding)?;
            if authenticator.handle() != request.binding.handle
                || !qualification_matches(
                    &request.binding,
                    qualification.provider().revision,
                    qualification.provider().policy_digest,
                )
                || qualification.binding() != expected_binding
            {
                return Err(BrokerError::BindingMismatch);
            }
            encode_canonical(
                &governance_request_ingress_qualification_to_wire(qualification),
                MAX_OPERATION_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_QUALIFY_V1)
            if slot == governance_signer_slot
                || slot == privacy_cycle_prf_slot
                || slot == privacy_release_anchor_slot
                || slot == transparency_leader_lease_slot
                || slot == fenced_privacy_publisher_slot
                || slot == fenced_privacy_head_reader_slot
                || slot == governance_checkpoint_slot
                || slot == stream_token_slot
                || slot == stream_token_gateway_admission_slot
                || slot == appeal_signer_slot
                || slot == appeal_checkpoint_slot
                || slot == potr_gateway_slot
                || slot == potr_provider_slot
                || slot == gateway_acme_slot
                || slot == gateway_compliance_slot
                || slot == pop_registry_slot
                || slot == por_replay_archive_slot
                || slot == moderation_transaction_signer_slot
                || slot == moderation_settlement_handoff_slot
                || slot == moderation_publication_handoff_slot
                || slot == moderation_panel_notification_slot
                || native_transaction_signer_role_for_slot(slot).is_some()
                || slot == provider_checkpoint_slot
                || slot == provider_retention_slot
                || slot == reputation_retention_slot
                || slot == reputation_journal_slot
                || slot == reputation_threshold_slot
                || slot == reputation_governance_slot
                || slot == reputation_checkpoint_slot
                || slot == billing_finalized_query_slot
                || slot == billing_journal_verifier_slot
                || slot == billing_statement_signer_slot
                || slot == billing_statement_publisher_slot
                || slot == billing_acknowledgement_authority_slot
                || slot == billing_epoch_witness_store_slot
                || slot == evidence_webauthn_slot
                || slot == evidence_grants_slot
                || slot == evidence_receipt_signer_slot
                || slot == evidence_erasure_slot
                || slot == evidence_checkpoint_slot
                || slot == moderation_checkpoint_slot
                || slot == evidence_archive_slot
                || slot == evidence_transparency_publisher_slot =>
        {
            let qualification = qualification_from_binding(&request.binding)?;
            encode_canonical(
                &QualificationResultWireV1 {
                    revision: qualification.revision,
                    policy_digest: qualification.policy_digest,
                },
                MAX_OPERATION_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_REPUTATION_JOURNAL_SUPPORTS_AUTHORITY_V1)
            if slot == reputation_journal_slot =>
        {
            let supports = decode_canonical::<ReputationJournalSupportsAuthorityRequestWireV1>(
                &request.payload,
                MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
            )?;
            let submitter = broker_backend!(state, reputation_journal_transaction_submitter);
            let supported = submitter.supports_authority(&supports.authority);
            requalify()?;
            encode_canonical(&supported, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_REPUTATION_JOURNAL_SUBMIT_V1) if slot == reputation_journal_slot => {
            let wire = decode_canonical::<ReputationJournalTransactionRequestWireV1>(
                &request.payload,
                MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
            )?;
            let submit = reputation_journal_request_from_wire(wire)?;
            ensure_reputation_session_network(&submit.network_id, &state.network_id)?;
            let submitter = broker_backend!(state, reputation_journal_transaction_submitter);
            if !submitter.supports_authority(&submit.authority) {
                return Err(BrokerError::Rejected);
            }
            let outcome = reputation_journal_submit_result_to_wire(submitter.submit(&submit))?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&outcome, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_REPUTATION_THRESHOLD_RECONCILE_V1)
            if slot == reputation_threshold_slot =>
        {
            let wire = decode_canonical::<ReputationThresholdSigningRequestWireV1>(
                &request.payload,
                MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
            )?;
            let signing = reputation_threshold_request_from_wire(wire)?;
            ensure_reputation_session_network(&signing.material.network_id, &state.network_id)?;
            let threshold_signer = broker_backend!(state, reputation_threshold_signer);
            let reconciled = threshold_signer.reconcile_signature(&signing);
            let result = match reconciled {
                Ok(None) => ReputationReconcileResultWireV1 {
                    outcome: 0,
                    canonical_result: Vec::new(),
                    failure_receipt: [0; 32],
                },
                Ok(Some(signature)) => ReputationReconcileResultWireV1 {
                    outcome: 1,
                    canonical_result: validate_reputation_signature(&signing, &signature)?,
                    failure_receipt: [0; 32],
                },
                Err(error) => ReputationReconcileResultWireV1 {
                    outcome: 2,
                    canonical_result: Vec::new(),
                    failure_receipt: error.receipt(),
                },
            };
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&result, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_REPUTATION_GOVERNANCE_RECONCILE_V1)
            if slot == reputation_governance_slot =>
        {
            let wire = decode_canonical::<ReputationGovernanceDagPublicationRequestWireV1>(
                &request.payload,
                MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
            )?;
            let publication = reputation_governance_request_from_wire(wire)?;
            let governance_dag = broker_backend!(state, reputation_governance_dag);
            let reconciled = governance_dag.reconcile_publication(&publication);
            let result = match reconciled {
                Ok(None) => ReputationReconcileResultWireV1 {
                    outcome: 0,
                    canonical_result: Vec::new(),
                    failure_receipt: [0; 32],
                },
                Ok(Some(readback)) => {
                    validate_reputation_governance_readback(&readback, &publication.signed_result)?;
                    ReputationReconcileResultWireV1 {
                        outcome: 1,
                        canonical_result: encode_canonical(
                            &readback,
                            MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
                        )?,
                        failure_receipt: [0; 32],
                    }
                }
                Err(error) => ReputationReconcileResultWireV1 {
                    outcome: 2,
                    canonical_result: Vec::new(),
                    failure_receipt: error.receipt(),
                },
            };
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&result, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_REPUTATION_JOURNAL_CHECKPOINT_LOAD_V1)
            if slot == reputation_checkpoint_slot =>
        {
            let checkpoint = broker_backend!(state, reputation_journal_checkpoint);
            let record = checkpoint.load_latest().map_err(|error| {
                match error {
                sorafs_node::reputation::runtime::
                    ReputationJournalCheckpointExternalErrorV1::Unavailable => {
                    BrokerError::Unavailable
                }
                sorafs_node::reputation::runtime::
                    ReputationJournalCheckpointExternalErrorV1::Rejected => {
                    BrokerError::Rejected
                }
                sorafs_node::reputation::runtime::
                    ReputationJournalCheckpointExternalErrorV1::Ambiguous => {
                    BrokerError::Protocol
                }
            }
            })?;
            let record = record
                .map(|record| {
                    record
                        .to_canonical_bytes(
                            sorafs_node::reputation::runtime::
                                REPUTATION_JOURNAL_PRODUCER_MAX_CHECKPOINT_BYTES_V1,
                        )
                        .map_err(|_| BrokerError::Protocol)
                })
                .transpose()?;
            requalify()?;
            encode_canonical(&record, MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1)
        }
        (slot, OPERATION_REPUTATION_JOURNAL_CHECKPOINT_COMPARE_AND_SWAP_V1)
            if slot == reputation_checkpoint_slot =>
        {
            let compare = decode_canonical::<ReputationJournalCheckpointCompareAndSwapRequestWireV1>(
                &request.payload,
                MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1,
            )?;
            let next = sorafs_node::reputation::runtime::
                ReputationJournalSealedCheckpointRecordV1::from_canonical_bytes(
                    &compare.next_record,
                    sorafs_node::reputation::runtime::
                        REPUTATION_JOURNAL_PRODUCER_MAX_CHECKPOINT_BYTES_V1,
                )
                .map_err(|_| BrokerError::Rejected)?;
            let checkpoint = broker_backend!(state, reputation_journal_checkpoint);
            let current = checkpoint.load_latest().map_err(|error| {
                match error {
                sorafs_node::reputation::runtime::
                    ReputationJournalCheckpointExternalErrorV1::Unavailable => {
                    BrokerError::Unavailable
                }
                sorafs_node::reputation::runtime::
                    ReputationJournalCheckpointExternalErrorV1::Rejected => {
                    BrokerError::Rejected
                }
                sorafs_node::reputation::runtime::
                    ReputationJournalCheckpointExternalErrorV1::Ambiguous => {
                    BrokerError::Protocol
                }
            }
            })?;
            let monotonic = match &current {
                None => {
                    compare.expected_revision.is_none()
                        && next.checkpoint_sequence() == 1
                        && next.predecessor_checkpoint_digest().is_none()
                }
                Some(previous) => {
                    compare.expected_revision == Some(previous.revision())
                        && previous
                            .checkpoint_sequence()
                            .checked_add(1)
                            .is_some_and(|sequence| sequence == next.checkpoint_sequence())
                        && next.predecessor_checkpoint_digest()
                            == Some(previous.checkpoint_digest())
                }
            };
            if !monotonic {
                return Err(BrokerError::Rejected);
            }
            checkpoint
                .compare_and_swap_latest(compare.expected_revision, &next)
                .map_err(|error| {
                    match error {
                    sorafs_node::reputation::runtime::
                        ReputationJournalCheckpointExternalErrorV1::Unavailable
                    | sorafs_node::reputation::runtime::
                        ReputationJournalCheckpointExternalErrorV1::Ambiguous => {
                        BrokerError::Ambiguous
                    }
                    sorafs_node::reputation::runtime::
                        ReputationJournalCheckpointExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                }
                })?;
            let readback = checkpoint
                .load_latest()
                .map_err(|_| BrokerError::Ambiguous)?;
            if readback.as_ref() != Some(&next) {
                return Err(BrokerError::Ambiguous);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&(), MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_IDENTITY_V1) if slot == billing_finalized_query_slot => {
            let identity = broker_backend!(state, billing_finalized_query)
                .identity()
                .map_err(|error| billing_external_error(error, false))?;
            if identity.handle != request.binding.handle {
                return Err(BrokerError::BindingMismatch);
            }
            requalify()?;
            encode_canonical(
                &BillingAdapterIdentityWireV1 {
                    handle: identity.handle,
                },
                MAX_BILLING_CONTROL_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_BILLING_IDENTITY_V1) if slot == billing_journal_verifier_slot => {
            let identity = broker_backend!(state, billing_journal_verifier)
                .identity()
                .map_err(|error| billing_external_error(error, false))?;
            if identity.handle != request.binding.handle {
                return Err(BrokerError::BindingMismatch);
            }
            requalify()?;
            encode_canonical(
                &BillingAdapterIdentityWireV1 {
                    handle: identity.handle,
                },
                MAX_BILLING_CONTROL_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_BILLING_IDENTITY_V1) if slot == billing_statement_signer_slot => {
            let identity = broker_backend!(state, billing_statement_signer)
                .identity()
                .map_err(|error| billing_external_error(error, false))?;
            let wire = BillingStatementSignerIdentityWireV1 {
                provider_handle: identity.provider_handle,
                signer_id: identity.signer_id,
                public_key: identity.public_key,
            };
            if wire.provider_handle != request.binding.handle
                || !validate_billing_public_identity_text(
                    &wire.signer_id,
                    sorafs_node::hedging_billing_service::BILLING_SIGNER_ID_MAX_BYTES_V1,
                )
                || iroha_crypto::ed25519_parse_public_key(&wire.public_key).is_err()
            {
                return Err(BrokerError::BindingMismatch);
            }
            requalify()?;
            encode_canonical(&wire, MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_IDENTITY_V1) if slot == billing_statement_publisher_slot => {
            let identity = broker_backend!(state, billing_statement_publisher)
                .identity()
                .map_err(|error| billing_external_error(error, false))?;
            let wire = BillingStatementPublisherIdentityWireV1 {
                provider_handle: identity.provider_handle,
                publisher_id: identity.publisher_id,
                route_id: identity.route_id,
                public_key: identity.public_key,
            };
            if wire.provider_handle != request.binding.handle
                || !validate_billing_public_identity_text(
                    &wire.publisher_id,
                    sorafs_node::hedging_billing_service::BILLING_SIGNER_ID_MAX_BYTES_V1,
                )
                || !validate_billing_public_identity_text(
                    &wire.route_id,
                    sorafs_node::hedging_billing_service::BILLING_PUBLICATION_ROUTE_MAX_BYTES_V1,
                )
                || iroha_crypto::ed25519_parse_public_key(&wire.public_key).is_err()
            {
                return Err(BrokerError::BindingMismatch);
            }
            requalify()?;
            encode_canonical(&wire, MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_IDENTITY_V1) if slot == billing_acknowledgement_authority_slot => {
            let identity = broker_backend!(state, billing_acknowledgement_authority)
                .identity()
                .map_err(|error| billing_external_error(error, false))?;
            if identity.provider_handle != request.binding.handle {
                return Err(BrokerError::BindingMismatch);
            }
            requalify()?;
            encode_canonical(
                &BillingAdapterIdentityWireV1 {
                    handle: identity.provider_handle,
                },
                MAX_BILLING_CONTROL_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_BILLING_READINESS_V1) if slot == billing_finalized_query_slot => {
            broker_backend!(state, billing_finalized_query)
                .check_readiness()
                .map_err(|error| billing_external_error(error, false))?;
            requalify()?;
            encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_READINESS_V1) if slot == billing_journal_verifier_slot => {
            broker_backend!(state, billing_journal_verifier)
                .check_readiness()
                .map_err(|error| billing_external_error(error, false))?;
            requalify()?;
            encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_READINESS_V1) if slot == billing_statement_signer_slot => {
            broker_backend!(state, billing_statement_signer)
                .check_readiness()
                .map_err(|error| billing_external_error(error, false))?;
            requalify()?;
            encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_READINESS_V1) if slot == billing_statement_publisher_slot => {
            broker_backend!(state, billing_statement_publisher)
                .check_readiness()
                .map_err(|error| billing_external_error(error, false))?;
            requalify()?;
            encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_READINESS_V1)
            if slot == billing_acknowledgement_authority_slot =>
        {
            broker_backend!(state, billing_acknowledgement_authority)
                .check_readiness()
                .map_err(|error| billing_external_error(error, false))?;
            requalify()?;
            encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_READINESS_V1) if slot == billing_epoch_witness_store_slot => {
            broker_backend!(state, billing_epoch_witness_store)
                .check_readiness()
                .map_err(|error| billing_external_error(error, false))?;
            requalify()?;
            encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_QUERY_CAPABILITIES_V1) if slot == billing_finalized_query_slot => {
            let supplies_period_closes =
                broker_backend!(state, billing_finalized_query).supplies_period_closes();
            if !supplies_period_closes {
                return Err(BrokerError::StaleOrRevoked);
            }
            requalify()?;
            encode_canonical(
                &BillingFinalizedQueryCapabilitiesWireV1 {
                    supplies_period_closes,
                },
                MAX_BILLING_CONTROL_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_BILLING_FINALIZED_HEAD_V1) if slot == billing_finalized_query_slot => {
            let head = broker_backend!(state, billing_finalized_query)
                .finalized_head()
                .map_err(|error| billing_external_error(error, false))?;
            validate_billing_cursor(head)?;
            requalify()?;
            encode_canonical(&head, MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_QUERY_PAGE_V1) if slot == billing_finalized_query_slot => {
            let query = decode_canonical::<BillingQueryPageRequestWireV1>(
                &request.payload,
                MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
            )?;
            validate_billing_query_position(query.position, state.network_id)?;
            let page = broker_backend!(state, billing_finalized_query)
                .query_finalized_page(
                    billing_query_position_from_wire(query.position),
                    query.max_events,
                )
                .map_err(|error| billing_external_error(error, false))?;
            if let Some(page) = page.as_ref() {
                validate_billing_page_shape(page, Some((query.position, query.max_events)))?;
                if page.network_id != state.network_id {
                    return Err(BrokerError::BindingMismatch);
                }
            }
            requalify()?;
            encode_canonical(&page, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_QUERY_PERIOD_CLOSE_V1) if slot == billing_finalized_query_slot => {
            let query = decode_canonical::<BillingQueryPeriodCloseRequestWireV1>(
                &request.payload,
                MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
            )?;
            validate_billing_query_position(query.position, state.network_id)?;
            let close = broker_backend!(state, billing_finalized_query)
                .query_finalized_period_close(
                    query.period_end_unix,
                    billing_query_position_from_wire(query.position),
                )
                .map_err(|error| billing_external_error(error, false))?;
            if let Some(close) = close.as_ref() {
                validate_billing_period_close_shape(close, Some(query.period_end_unix))?;
                if close.network_id != state.network_id {
                    return Err(BrokerError::BindingMismatch);
                }
            }
            requalify()?;
            encode_canonical(&close, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_VERIFY_PAGE_V1) if slot == billing_journal_verifier_slot => {
            let verify = decode_canonical::<BillingVerifyPageRequestWireV1>(
                &request.payload,
                MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
            )?;
            if verify.network_id != state.network_id || verify.page.network_id != verify.network_id
            {
                return Err(BrokerError::BindingMismatch);
            }
            validate_billing_page_shape(&verify.page, None)?;
            if let Some(previous) = verify.previous {
                validate_billing_journal_commitment(previous, verify.network_id)?;
            }
            broker_backend!(state, billing_journal_verifier)
                .verify_page(&verify.network_id, verify.previous, &verify.page)
                .map_err(|error| billing_external_error(error, false))?;
            requalify()?;
            encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_VERIFY_PERIOD_CLOSE_V1)
            if slot == billing_journal_verifier_slot =>
        {
            let verify = decode_canonical::<BillingVerifyPeriodCloseRequestWireV1>(
                &request.payload,
                MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
            )?;
            if verify.network_id != state.network_id || verify.close.network_id != verify.network_id
            {
                return Err(BrokerError::BindingMismatch);
            }
            validate_billing_period_close_shape(&verify.close, None)?;
            broker_backend!(state, billing_journal_verifier)
                .verify_period_close(&verify.network_id, &verify.close)
                .map_err(|error| billing_external_error(error, false))?;
            requalify()?;
            encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_VERIFY_EPOCH_TRANSITION_V1)
            if slot == billing_journal_verifier_slot =>
        {
            let verify = decode_canonical::<BillingVerifyEpochTransitionRequestWireV1>(
                &request.payload,
                MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
            )?;
            if verify.network_id != state.network_id
                || verify.transition.previous_service_policy.network_id != verify.network_id
                || verify.transition.next_service_policy.network_id != verify.network_id
            {
                return Err(BrokerError::BindingMismatch);
            }
            broker_backend!(state, billing_journal_verifier)
                .verify_epoch_transition(&verify.network_id, &verify.transition)
                .map_err(|error| billing_external_error(error, false))?;
            requalify()?;
            encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_SIGN_STATEMENT_DIGEST_V1)
            if slot == billing_statement_signer_slot =>
        {
            let sign = decode_canonical::<BillingSignDigestRequestWireV1>(
                &request.payload,
                MAX_BILLING_CONTROL_FRAME_BYTES_V1,
            )?;
            let signer = broker_backend!(state, billing_statement_signer);
            let identity = signer
                .identity()
                .map_err(|error| billing_external_error(error, false))?;
            let signature = signer
                .sign_digest(sign.digest)
                .map_err(|error| billing_external_error(error, false))?;
            let identity_after = signer
                .identity()
                .map_err(|error| billing_external_error(error, false))?;
            if identity_after != identity || identity.provider_handle != request.binding.handle {
                return Err(BrokerError::StaleOrRevoked);
            }
            verify_evidence_viewer_ed25519_signature(identity.public_key, signature, &sign.digest)?;
            requalify()?;
            encode_canonical(
                &BillingSignDigestResultWireV1 { signature },
                MAX_BILLING_CONTROL_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_BILLING_PUBLISH_STATEMENT_V1)
            if slot == billing_statement_publisher_slot =>
        {
            let publish = decode_canonical::<BillingPublishStatementRequestWireV1>(
                &request.payload,
                MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
            )?;
            validate_billing_publish_request(&publish, state.network_id)?;
            let publisher = broker_backend!(state, billing_statement_publisher);
            let identity = publisher
                .identity()
                .map_err(|error| billing_external_error(error, false))?;
            let identity_wire = BillingStatementPublisherIdentityWireV1 {
                provider_handle: identity.provider_handle,
                publisher_id: identity.publisher_id,
                route_id: identity.route_id,
                public_key: identity.public_key,
            };
            let receipt = publisher
                .publish(
                    publish.idempotency_key,
                    publish.signed_statement_digest,
                    &publish.statement,
                )
                .map_err(|error| billing_external_error(error, true))?;
            let readback = publisher
                .lookup(publish.idempotency_key)
                .map_err(|error| billing_external_error(error, true))?
                .ok_or(BrokerError::Ambiguous)?;
            let readback_wire = BillingAuthoritativePublicationWireV1 {
                signed_statement: readback.signed_statement,
                receipt: readback.receipt,
            };
            if readback_wire.signed_statement != publish.statement
                || readback_wire.receipt != receipt
            {
                return Err(BrokerError::Ambiguous);
            }
            validate_billing_publication_shape(
                &readback_wire,
                publish.idempotency_key,
                &identity_wire,
                state.network_id,
            )
            .map_err(|_| BrokerError::Ambiguous)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&receipt, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_LOOKUP_PUBLICATION_V1)
            if slot == billing_statement_publisher_slot =>
        {
            let lookup = decode_canonical::<BillingLookupRequestWireV1>(
                &request.payload,
                MAX_BILLING_CONTROL_FRAME_BYTES_V1,
            )?;
            let publisher = broker_backend!(state, billing_statement_publisher);
            let identity = publisher
                .identity()
                .map_err(|error| billing_external_error(error, false))?;
            let identity_wire = BillingStatementPublisherIdentityWireV1 {
                provider_handle: identity.provider_handle,
                publisher_id: identity.publisher_id,
                route_id: identity.route_id,
                public_key: identity.public_key,
            };
            let publication = publisher
                .lookup(lookup.record_id)
                .map_err(|error| billing_external_error(error, false))?
                .map(|publication| BillingAuthoritativePublicationWireV1 {
                    signed_statement: publication.signed_statement,
                    receipt: publication.receipt,
                });
            if let Some(publication) = publication.as_ref() {
                validate_billing_publication_shape(
                    publication,
                    lookup.record_id,
                    &identity_wire,
                    state.network_id,
                )?;
            }
            requalify()?;
            encode_canonical(&publication, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_VERIFY_ACKNOWLEDGEMENT_V1)
            if slot == billing_acknowledgement_authority_slot =>
        {
            let acknowledgement = decode_canonical::<BillingAcknowledgementRequestWireV1>(
                &request.payload,
                MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
            )?;
            validate_billing_acknowledgement_request(&acknowledgement, state.network_id)?;
            broker_backend!(state, billing_acknowledgement_authority)
                .verify(&acknowledgement.statement, &acknowledgement.acknowledgement)
                .map_err(|error| billing_external_error(error, false))?;
            requalify()?;
            encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_RECORD_ACKNOWLEDGEMENT_V1)
            if slot == billing_acknowledgement_authority_slot =>
        {
            let acknowledgement = decode_canonical::<BillingAcknowledgementRequestWireV1>(
                &request.payload,
                MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
            )?;
            validate_billing_acknowledgement_request(&acknowledgement, state.network_id)?;
            let authority = broker_backend!(state, billing_acknowledgement_authority);
            let recorded = authority
                .record(&acknowledgement.statement, &acknowledgement.acknowledgement)
                .map_err(|error| billing_external_error(error, true))?;
            if recorded != acknowledgement.acknowledgement {
                return Err(BrokerError::Ambiguous);
            }
            let readback = authority
                .lookup(recorded.statement_id)
                .map_err(|error| billing_external_error(error, true))?;
            if readback.as_ref() != Some(&recorded) {
                return Err(BrokerError::Ambiguous);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&recorded, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_LOOKUP_ACKNOWLEDGEMENT_V1)
            if slot == billing_acknowledgement_authority_slot =>
        {
            let lookup = decode_canonical::<BillingLookupRequestWireV1>(
                &request.payload,
                MAX_BILLING_CONTROL_FRAME_BYTES_V1,
            )?;
            let acknowledgement = broker_backend!(state, billing_acknowledgement_authority)
                .lookup(lookup.record_id)
                .map_err(|error| billing_external_error(error, false))?;
            if let Some(acknowledgement) = acknowledgement.as_ref()
                && (acknowledgement.statement_id.ne(&lookup.record_id)
                    || acknowledgement.network_id != state.network_id)
            {
                return Err(BrokerError::Rejected);
            }
            requalify()?;
            encode_canonical(&acknowledgement, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_LOAD_LATEST_EPOCH_V1)
            if slot == billing_epoch_witness_store_slot =>
        {
            let record = broker_backend!(state, billing_epoch_witness_store)
                .load_latest()
                .map_err(|error| billing_external_error(error, false))?;
            if let Some(record) = record.as_ref() {
                if record.network_id != state.network_id {
                    return Err(BrokerError::BindingMismatch);
                }
                record
                    .validate(
                        sorafs_node::hedging_billing_service::
                            HEDGING_BILLING_MAX_CHECKPOINT_BYTES_V1,
                    )
                    .map_err(|_| BrokerError::Rejected)?;
            }
            requalify()?;
            encode_canonical(&record, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_LOAD_EPOCH_V1) if slot == billing_epoch_witness_store_slot => {
            let load = decode_canonical::<BillingLoadEpochRequestWireV1>(
                &request.payload,
                MAX_BILLING_CONTROL_FRAME_BYTES_V1,
            )?;
            let record = broker_backend!(state, billing_epoch_witness_store)
                .load_epoch(load.epoch_sequence)
                .map_err(|error| billing_external_error(error, false))?;
            if let Some(record) = record.as_ref() {
                if record.network_id != state.network_id {
                    return Err(BrokerError::BindingMismatch);
                }
                record
                    .validate(
                        sorafs_node::hedging_billing_service::
                            HEDGING_BILLING_MAX_CHECKPOINT_BYTES_V1,
                    )
                    .map_err(|_| BrokerError::Rejected)?;
                if record.epoch_sequence != load.epoch_sequence {
                    return Err(BrokerError::Rejected);
                }
            }
            requalify()?;
            encode_canonical(&record, MAX_BILLING_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_BILLING_COMPARE_AND_SWAP_EPOCH_V1)
            if slot == billing_epoch_witness_store_slot =>
        {
            let compare = decode_canonical::<BillingCompareAndSwapEpochRequestWireV1>(
                &request.payload,
                MAX_BILLING_RUNTIME_FRAME_BYTES_V1,
            )?;
            compare
                .next
                .validate(
                    sorafs_node::hedging_billing_service::HEDGING_BILLING_MAX_CHECKPOINT_BYTES_V1,
                )
                .map_err(|_| BrokerError::Rejected)?;
            if compare.next.network_id != state.network_id {
                return Err(BrokerError::BindingMismatch);
            }
            let store = broker_backend!(state, billing_epoch_witness_store);
            let current = store
                .load_latest()
                .map_err(|error| billing_external_error(error, false))?;
            if current
                .as_ref()
                .is_some_and(|record| record.network_id != state.network_id)
            {
                return Err(BrokerError::BindingMismatch);
            }
            let monotonic = match current.as_ref() {
                None => compare.expected_revision.is_none() && compare.next.epoch_sequence == 1,
                Some(current) => {
                    current.revision == compare.expected_revision.unwrap_or([0; 32])
                        && current
                            .epoch_sequence
                            .checked_add(1)
                            .is_some_and(|next| next == compare.next.epoch_sequence)
                }
            };
            if current.as_ref().map(|record| record.revision) != compare.expected_revision {
                return Err(BrokerError::Conflict);
            }
            if !monotonic {
                return Err(BrokerError::Rejected);
            }
            store
                .compare_and_swap_latest(compare.expected_revision, &compare.next)
                .map_err(|error| billing_external_error(error, true))?;
            let latest = store
                .load_latest()
                .map_err(|error| billing_external_error(error, true))?;
            let historical = store
                .load_epoch(compare.next.epoch_sequence)
                .map_err(|error| billing_external_error(error, true))?;
            if latest.as_ref() != Some(&compare.next) || historical.as_ref() != Some(&compare.next)
            {
                return Err(BrokerError::Ambiguous);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&(), MAX_BILLING_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_PRIVACY_CYCLE_PRF_DERIVE_V1) if slot == privacy_cycle_prf_slot => {
            let wire = decode_canonical::<PrivacyCyclePrfRequestWireV1>(
                &request.payload,
                MAX_TRANSPARENCY_PRF_FRAME_BYTES_V1,
            )?;
            let request_value = wire.to_request()?;
            let output = broker_backend!(state, privacy_cycle_prf_provider)
                .derive_cycle_output(&request_value)
                .map_err(|error| match error {
                    sorafs_node::PrivacyCyclePrfProviderErrorV1::Unavailable
                    | sorafs_node::PrivacyCyclePrfProviderErrorV1::RateLimited => {
                        BrokerError::Unavailable
                    }
                    sorafs_node::PrivacyCyclePrfProviderErrorV1::AuthenticationFailed
                    | sorafs_node::PrivacyCyclePrfProviderErrorV1::Internal => {
                        BrokerError::Rejected
                    }
                })?;
            let wire = PrivacyCyclePrfOutputWireV1 {
                output: output.runtime_transport_bytes(),
            };
            requalify()?;
            encode_canonical(&wire, MAX_TRANSPARENCY_PRF_FRAME_BYTES_V1)
        }
        (slot, OPERATION_PRIVACY_RELEASE_ANCHOR_FINALIZED_HEAD_V1)
            if slot == privacy_release_anchor_slot =>
        {
            let query = validate_privacy_release_anchor_query(decode_canonical::<
                PrivacyReleaseAnchorFinalizedHeadRequestWireV1,
            >(
                &request.payload,
                MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1,
            )?)?;
            let anchor = broker_backend!(state, privacy_release_anchor);
            let head = anchor.finalized_head(query).map_err(|error| match error {
                sorafs_node::PrivacyReleaseAnchorErrorV1::Unavailable
                | sorafs_node::PrivacyReleaseAnchorErrorV1::Internal => BrokerError::Unavailable,
                sorafs_node::PrivacyReleaseAnchorErrorV1::AuthenticationFailed
                | sorafs_node::PrivacyReleaseAnchorErrorV1::Conflict
                | sorafs_node::PrivacyReleaseAnchorErrorV1::InvalidState => BrokerError::Rejected,
            })?;
            let head = PrivacyReleaseAnchorHeadWireV1::from_head(head);
            if head.query_id != query || head.to_head().is_err() {
                return Err(BrokerError::Rejected);
            }
            requalify()?;
            encode_canonical(&head, MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1)
        }
        (slot, OPERATION_PRIVACY_RELEASE_ANCHOR_COMPARE_AND_SET_V1)
            if slot == privacy_release_anchor_slot =>
        {
            let compare = decode_canonical::<PrivacyReleaseAnchorCompareAndSetRequestWireV1>(
                &request.payload,
                MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1,
            )?;
            let (expected, next, lease) =
                validate_privacy_release_anchor_compare_and_set(&compare)?;
            let anchor = broker_backend!(state, privacy_release_anchor);
            anchor
                .compare_and_set_finalized_head(expected, next, &lease)
                .map_err(|error| match error {
                    sorafs_node::PrivacyReleaseAnchorErrorV1::Conflict => BrokerError::Conflict,
                    sorafs_node::PrivacyReleaseAnchorErrorV1::AuthenticationFailed
                    | sorafs_node::PrivacyReleaseAnchorErrorV1::InvalidState => {
                        BrokerError::Rejected
                    }
                    sorafs_node::PrivacyReleaseAnchorErrorV1::Unavailable
                    | sorafs_node::PrivacyReleaseAnchorErrorV1::Internal => BrokerError::Ambiguous,
                })?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            let readback = anchor
                .finalized_head(next.query_id())
                .map_err(|_| BrokerError::Ambiguous)?;
            if readback != next {
                return Err(BrokerError::Ambiguous);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&(), MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1)
                .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_TRANSPARENCY_LEADER_LEASE_ACQUIRE_V1)
            if slot == transparency_leader_lease_slot =>
        {
            let configured = transparency_runtime_binding_from_wire(&request.binding)?;
            let wire = decode_canonical::<TransparencyLeaderLeaseAcquireRequestWireV1>(
                &request.payload,
                MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
            )?;
            let lease_request = validate_transparency_leader_lease_acquire(&wire, &configured)?;
            let provider = broker_backend!(state, transparency_leader_lease_provider);
            let grant = provider
                .acquire(&lease_request)
                .map_err(transparency_leader_lease_provider_error)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            validate_transparency_leader_lease_acquire_grant(&lease_request, &grant, &configured)
                .map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(
                &TransparencyLeaderLeaseGrantWireV1::from_grant(&grant),
                MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
            )
            .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_TRANSPARENCY_LEADER_LEASE_RENEW_V1)
            if slot == transparency_leader_lease_slot =>
        {
            let configured = transparency_runtime_binding_from_wire(&request.binding)?;
            let wire = decode_canonical::<TransparencyLeaderLeaseRenewRequestWireV1>(
                &request.payload,
                MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
            )?;
            let lease_request = validate_transparency_leader_lease_renew(&wire, &configured)?;
            let provider = broker_backend!(state, transparency_leader_lease_provider);
            let grant = provider
                .renew(&lease_request)
                .map_err(transparency_leader_lease_provider_error)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            validate_transparency_leader_lease_renew_grant(&lease_request, &grant, &configured)
                .map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(
                &TransparencyLeaderLeaseGrantWireV1::from_grant(&grant),
                MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
            )
            .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_TRANSPARENCY_LEADER_LEASE_RELEASE_V1)
            if slot == transparency_leader_lease_slot =>
        {
            let configured = transparency_runtime_binding_from_wire(&request.binding)?;
            let wire = decode_canonical::<TransparencyLeaderLeaseReleaseRequestWireV1>(
                &request.payload,
                MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
            )?;
            let lease_request = validate_transparency_leader_lease_release(&wire, &configured)?;
            let provider = broker_backend!(state, transparency_leader_lease_provider);
            let receipt = provider
                .release(&lease_request)
                .map_err(transparency_leader_lease_provider_error)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            validate_transparency_leader_lease_release_receipt(
                &lease_request,
                &receipt,
                &configured,
            )
            .map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(
                &TransparencyLeaderLeaseReleaseReceiptWireV1::from_receipt(&receipt),
                MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
            )
            .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_FENCED_PRIVACY_COMPARE_AND_APPEND_V1)
            if slot == fenced_privacy_publisher_slot =>
        {
            let publish = decode_canonical::<FencedPrivacyPublicationRequestWireV1>(
                &request.payload,
                MAX_FENCED_PRIVACY_PUBLICATION_FRAME_BYTES_V1,
            )?
            .to_request()?;
            let publisher = broker_backend!(state, fenced_privacy_publisher);
            let receipt = publisher
                .compare_and_append_privacy(&publish)
                .map_err(fenced_privacy_publish_error)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            let qualification = qualification_from_binding(&request.binding)?;
            receipt
                .validate_for_request(&publish, &request.binding.handle, qualification)
                .map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(
                &FencedPrivacyPublicationReceiptWireV1::from_receipt(&receipt),
                MAX_FENCED_PRIVACY_PUBLICATION_FRAME_BYTES_V1,
            )
            .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_FENCED_PRIVACY_READ_HEAD_WITH_ANCESTRY_V1)
            if slot == fenced_privacy_head_reader_slot =>
        {
            let (required_ancestors, required_publications) =
                decode_canonical::<FencedPrivacyHeadReadRequestWireV1>(
                    &request.payload,
                    MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1,
                )?
                .to_required_evidence()?;
            let reader = broker_backend!(state, fenced_privacy_head_reader);
            let proof = reader
                .read_authoritative_head_with_ancestry(&required_ancestors, &required_publications)
                .map_err(|_| BrokerError::Unavailable)?;
            requalify()?;
            let proof_wire = FencedTransparencyHeadAncestryProofWireV1::from_proof(&proof);
            proof_wire
                .to_proof(&required_ancestors, &required_publications)
                .map_err(|_| BrokerError::Rejected)?;
            encode_canonical(&proof_wire, MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1)
        }
        (slot, OPERATION_STREAM_TOKEN_SIGN_V1) if slot == stream_token_slot => {
            let sign = decode_canonical::<SignRequestWireV1>(
                &request.payload,
                MAX_STREAM_TOKEN_FRAME_BYTES_V1,
            )?;
            validate_stream_token_signing_payload(&sign.payload)?;
            let signer = broker_backend!(state, stream_token_signer);
            let signature = signer.sign(&sign.payload).map_err(|error| match error {
                iroha_torii::sorafs::StreamTokenSigningError::Unavailable => {
                    BrokerError::Unavailable
                }
                iroha_torii::sorafs::StreamTokenSigningError::Refused => BrokerError::Rejected,
            })?;
            let public_key =
                required_binding_value!(&request.binding, stream_token_signer_public_key);
            verify_evidence_viewer_ed25519_signature(public_key, signature, &sign.payload)
                .map_err(|_| BrokerError::Rejected)?;
            requalify()?;
            encode_canonical(
                &SignResultWireV1 { signature },
                MAX_STREAM_TOKEN_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_STREAM_TOKEN_GATEWAY_ADMIT_V1)
            if slot == stream_token_gateway_admission_slot =>
        {
            let admission = decode_canonical::<
                iroha_torii::sorafs::StreamTokenGatewayAdmissionRequestV1,
            >(&request.payload, MAX_BROKER_UNARY_FRAME_BYTES_V1)?;
            admission.validate().map_err(|_| BrokerError::Rejected)?;
            let provider = broker_backend!(state, stream_token_gateway_admission);
            let admission_result = provider
                .admit(&admission)
                .map_err(stream_token_gateway_provider_error)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            let qualification = required_binding_value!(
                &request.binding,
                stream_token_gateway_admission_qualification
            );
            admission_result
                .validate_for_request(&admission, qualification)
                .map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&admission_result, MAX_BROKER_UNARY_FRAME_BYTES_V1)
        }
        (slot, OPERATION_STREAM_TOKEN_GATEWAY_PENDING_V1)
            if slot == stream_token_gateway_admission_slot =>
        {
            let max_items =
                decode_canonical::<u32>(&request.payload, MAX_BROKER_UNARY_FRAME_BYTES_V1)?;
            let configured = required_binding_value!(
                &request.binding,
                stream_token_gateway_admission_reconcile_max_items
            );
            if max_items == 0 || max_items > configured {
                return Err(BrokerError::Rejected);
            }
            let provider = broker_backend!(state, stream_token_gateway_admission);
            let pending = provider
                .pending(max_items)
                .map_err(stream_token_gateway_provider_error)?;
            let qualification = required_binding_value!(
                &request.binding,
                stream_token_gateway_admission_qualification
            );
            pending
                .validate(max_items, qualification)
                .map_err(|_| BrokerError::Rejected)?;
            requalify()?;
            encode_canonical(&pending, MAX_BROKER_UNARY_FRAME_BYTES_V1)
        }
        (
            slot,
            OPERATION_STREAM_TOKEN_GATEWAY_ACKNOWLEDGE_V1
            | OPERATION_STREAM_TOKEN_GATEWAY_RELEASE_LEASE_V1,
        ) if slot == stream_token_gateway_admission_slot => {
            let record = decode_canonical::<
                iroha_torii::sorafs::StreamTokenGatewayAdmissionRecordV1,
            >(&request.payload, MAX_BROKER_UNARY_FRAME_BYTES_V1)?;
            let qualification = required_binding_value!(
                &request.binding,
                stream_token_gateway_admission_qualification
            );
            record
                .validate_shape(qualification)
                .map_err(|_| BrokerError::Rejected)?;
            let provider = broker_backend!(state, stream_token_gateway_admission);
            let outcome = if request.operation == OPERATION_STREAM_TOKEN_GATEWAY_ACKNOWLEDGE_V1 {
                provider.acknowledge(record)
            } else {
                provider.release_lease(record)
            }
            .map_err(stream_token_gateway_provider_error)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&outcome, MAX_BROKER_UNARY_FRAME_BYTES_V1)
        }
        (slot, OPERATION_APPEAL_FINANCE_TRANSACTION_SIGN_V1) if slot == appeal_signer_slot => {
            let payload = decode_transaction_payload_bounded(
                &request.payload,
                MAX_APPEAL_FINANCE_TRANSACTION_BYTES_V1,
            )?;
            ensure_transaction_session_network(&payload, &state.network_id)?;
            let expected = payload.clone();
            let exact = required_binding_ref!(&request.binding, appeal_finance_signer_binding);
            if payload.authority() != &exact.authority {
                return Err(BrokerError::Rejected);
            }
            let backend = appeal_finance_signer_backend(&state.backends, &request.binding.handle)
                .map_err(|_| BrokerError::BindingMismatch)?;
            let transaction = backend.sign(payload).map_err(|error| match error {
                iroha_torii::SoraFsAppealFinanceSigningError::Unavailable => {
                    BrokerError::Unavailable
                }
                iroha_torii::SoraFsAppealFinanceSigningError::Refused => BrokerError::Rejected,
                iroha_torii::SoraFsAppealFinanceSigningError::QualificationChanged => {
                    BrokerError::StaleOrRevoked
                }
            })?;
            if transaction.payload() != &expected
                || transaction.authority() != &exact.authority
                || transaction.verify_signature().is_err()
            {
                return Err(BrokerError::Rejected);
            }
            requalify().map_err(|_| BrokerError::StaleOrRevoked)?;
            encode_canonical(&transaction, MAX_APPEAL_FINANCE_TRANSACTION_FRAME_BYTES_V1)
                .map_err(|_| BrokerError::Protocol)
        }
        (slot, OPERATION_APPEAL_FINANCE_CHECKPOINT_SIGN_V1) if slot == appeal_checkpoint_slot => {
            let digest =
                decode_canonical::<[u8; 32]>(&request.payload, MAX_STREAM_TOKEN_FRAME_BYTES_V1)?;
            if digest == [0; 32] {
                return Err(BrokerError::Rejected);
            }
            let checkpoint = broker_backend!(state, appeal_finance_checkpoint);
            let signature = checkpoint.sign_digest(digest).map_err(|error| {
                match error {
                    sorafs_node::appeal_finance_transaction_forwarder::
                        AppealFinanceCheckpointExternalError::Unavailable
                    | sorafs_node::appeal_finance_transaction_forwarder::
                        AppealFinanceCheckpointExternalError::Ambiguous => {
                            BrokerError::Unavailable
                        }
                    sorafs_node::appeal_finance_transaction_forwarder::
                        AppealFinanceCheckpointExternalError::Rejected => {
                            BrokerError::Rejected
                        }
                    }
            })?;
            let public_key = exact_ed25519_public_key_bytes(
                &required_binding_ref!(&request.binding, appeal_finance_checkpoint_binding)
                    .public_key,
            )?;
            verify_evidence_viewer_ed25519_signature(public_key, signature, &digest)
                .map_err(|_| BrokerError::Rejected)?;
            requalify()?;
            encode_canonical(
                &SignResultWireV1 { signature },
                MAX_STREAM_TOKEN_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_APPEAL_FINANCE_CHECKPOINT_LOAD_V1) if slot == appeal_checkpoint_slot => {
            decode_canonical::<()>(&request.payload, MAX_STREAM_TOKEN_FRAME_BYTES_V1)?;
            let checkpoint_max =
                required_binding_value!(&request.binding, appeal_finance_checkpoint_max_bytes);
            let record = broker_backend!(state, appeal_finance_checkpoint)
                .load_latest()
                .map_err(|error| {
                    match error {
                        sorafs_node::appeal_finance_transaction_forwarder::
                            AppealFinanceCheckpointExternalError::Unavailable
                        | sorafs_node::appeal_finance_transaction_forwarder::
                            AppealFinanceCheckpointExternalError::Ambiguous => {
                                BrokerError::Unavailable
                            }
                        sorafs_node::appeal_finance_transaction_forwarder::
                            AppealFinanceCheckpointExternalError::Rejected => {
                                BrokerError::Rejected
                            }
                        }
                })?;
            if let Some(record) = record.as_ref() {
                record
                    .validate(checkpoint_max)
                    .map_err(|_| BrokerError::Rejected)?;
            }
            requalify()?;
            encode_canonical(&record, MAX_APPEAL_FINANCE_CHECKPOINT_FRAME_BYTES_V1)
        }
        (slot, OPERATION_APPEAL_FINANCE_CHECKPOINT_COMPARE_AND_SWAP_V1)
            if slot == appeal_checkpoint_slot =>
        {
            let compare = decode_canonical::<AppealFinanceCheckpointCompareAndSwapWireV1>(
                &request.payload,
                MAX_APPEAL_FINANCE_CHECKPOINT_FRAME_BYTES_V1,
            )?;
            let checkpoint_max =
                required_binding_value!(&request.binding, appeal_finance_checkpoint_max_bytes);
            compare
                .next
                .validate(checkpoint_max)
                .map_err(|_| BrokerError::Rejected)?;
            let checkpoint = broker_backend!(state, appeal_finance_checkpoint);
            let current = checkpoint
                .load_latest()
                .map_err(|_| BrokerError::Unavailable)?;
            if current.as_ref().map(|record| record.revision) != compare.expected_revision {
                return Err(BrokerError::Conflict);
            }
            let monotonic = match current.as_ref() {
                None => {
                    compare.expected_revision.is_none() && compare.next.checkpoint_sequence == 1
                }
                Some(record) => record
                    .checkpoint_sequence
                    .checked_add(1)
                    .is_some_and(|sequence| sequence == compare.next.checkpoint_sequence),
            };
            if !monotonic {
                return Err(BrokerError::Rejected);
            }
            checkpoint
                .compare_and_swap_latest(compare.expected_revision, &compare.next)
                .map_err(|error| {
                    match error {
                    sorafs_node::appeal_finance_transaction_forwarder::
                        AppealFinanceCheckpointExternalError::Unavailable => {
                            BrokerError::Unavailable
                        }
                    sorafs_node::appeal_finance_transaction_forwarder::
                        AppealFinanceCheckpointExternalError::Rejected => {
                            BrokerError::Rejected
                        }
                    sorafs_node::appeal_finance_transaction_forwarder::
                        AppealFinanceCheckpointExternalError::Ambiguous => {
                            BrokerError::Ambiguous
                        }
                }
                })?;
            if checkpoint
                .load_latest()
                .map_err(|_| BrokerError::Ambiguous)?
                .as_ref()
                != Some(&compare.next)
            {
                return Err(BrokerError::Ambiguous);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&(), MAX_STREAM_TOKEN_FRAME_BYTES_V1)
        }
        (slot, OPERATION_POTR_SIGN_V1)
            if slot == potr_gateway_slot || slot == potr_provider_slot =>
        {
            let sign = decode_canonical::<PotrSignRequestWireV1>(
                &request.payload,
                MAX_POTR_FRAME_BYTES_V1,
            )?;
            let runtime = required_binding_ref!(&request.binding, potr_runtime_binding);
            validate_potr_signing_payload(
                &sign.payload,
                runtime.baseline_admission_policy.provider_id,
            )?;
            let (role, algorithm, signature) = if slot == potr_gateway_slot {
                let signer = broker_backend!(state, potr_gateway_signer);
                let public_key = signer.public_key().map_err(|_| BrokerError::Unavailable)?;
                if sign.expected_public_key.as_slice() != public_key.as_slice() {
                    return Err(BrokerError::BindingMismatch);
                }
                (
                    "gateway",
                    sorafs_manifest::potr::PotrSignatureAlgorithm::Ed25519,
                    signer.sign(&sign.payload).map_err(|error| match error {
                        iroha_torii::sorafs::PotrSignerServiceError::Unavailable => {
                            BrokerError::Unavailable
                        }
                        iroha_torii::sorafs::PotrSignerServiceError::Refused => {
                            BrokerError::Rejected
                        }
                    })?,
                )
            } else {
                let signer = broker_backend!(state, potr_provider_signer);
                let public_key = signer.public_key().map_err(|_| BrokerError::Unavailable)?;
                if sign.expected_public_key != public_key {
                    return Err(BrokerError::BindingMismatch);
                }
                (
                    "provider",
                    sorafs_manifest::potr::PotrSignatureAlgorithm::MlDsa65,
                    signer.sign(&sign.payload).map_err(|error| match error {
                        iroha_torii::sorafs::PotrSignerServiceError::Unavailable => {
                            BrokerError::Unavailable
                        }
                        iroha_torii::sorafs::PotrSignerServiceError::Refused => {
                            BrokerError::Rejected
                        }
                    })?,
                )
            };
            if signature.is_empty() || signature.len() > MAX_POTR_SIGNATURE_BYTES_V1 {
                return Err(BrokerError::Rejected);
            }
            sorafs_manifest::potr::PotrSignatureV1 {
                algorithm,
                public_key: sign.expected_public_key.clone(),
                signature: signature.clone(),
            }
            .verify(role, &sign.payload)
            .map_err(|_| BrokerError::Rejected)?;
            requalify()?;
            encode_canonical(
                &VariableSignatureResultWireV1 { signature },
                MAX_POTR_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_GATEWAY_ACME_ORDER_CERTIFICATE_V1) if slot == gateway_acme_slot => {
            let wire = decode_canonical::<GatewayAcmeOrderRequestWireV1>(
                &request.payload,
                MAX_GATEWAY_ACME_FRAME_BYTES_V1,
            )?;
            validate_gateway_acme_order(&wire)?;
            let order = iroha_torii::sorafs::gateway::CertificateOrder {
                hostnames: wire.hostnames,
                account_email: wire.account_email,
                directory_url: wire.directory_url,
                dns_provider_id: wire.dns_provider_id,
                challenge: iroha_torii::sorafs::gateway::ChallengeProfile {
                    dns01: wire.dns01,
                    tls_alpn_01: wire.tls_alpn_01,
                },
            };
            let outcome =
                match broker_backend!(state, gateway_acme_client).order_certificate(&order) {
                    Ok(bundle) => GatewayAcmeOrderOutcomeWireV1 {
                        outcome: 0,
                        certificate_pem: bundle.certificate_pem.clone(),
                        private_key_pem: bundle.private_key_pem.clone(),
                        ech_config: bundle.ech_config.clone(),
                        not_after: Some(
                            SystemTimeWireV1::from_system_time(bundle.not_after)
                                .map_err(|_| BrokerError::Ambiguous)?,
                        ),
                        retry_after: None,
                    },
                    Err(iroha_torii::sorafs::gateway::AcmeClientError::Rejected) => {
                        GatewayAcmeOrderOutcomeWireV1 {
                            outcome: 1,
                            certificate_pem: String::new(),
                            private_key_pem: String::new(),
                            ech_config: None,
                            not_after: None,
                            retry_after: None,
                        }
                    }
                    Err(iroha_torii::sorafs::gateway::AcmeClientError::Temporary {
                        retry_after,
                    }) => GatewayAcmeOrderOutcomeWireV1 {
                        outcome: 2,
                        certificate_pem: String::new(),
                        private_key_pem: String::new(),
                        ech_config: None,
                        not_after: None,
                        retry_after: retry_after.map(DurationWireV1::from_duration),
                    },
                    Err(iroha_torii::sorafs::gateway::AcmeClientError::Transport) => {
                        GatewayAcmeOrderOutcomeWireV1 {
                            outcome: 3,
                            certificate_pem: String::new(),
                            private_key_pem: String::new(),
                            ech_config: None,
                            not_after: None,
                            retry_after: None,
                        }
                    }
                };
            validate_gateway_acme_outcome(&outcome).map_err(|_| BrokerError::Ambiguous)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&outcome, MAX_GATEWAY_ACME_FRAME_BYTES_V1)
                .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_GATEWAY_COMPLIANCE_RESOLVE_V1) if slot == gateway_compliance_slot => {
            let wire = decode_canonical::<GatewayComplianceResolveRequestWireV1>(
                &request.payload,
                128 * 1024,
            )?;
            let timeout = validate_gateway_compliance_resolve_request(&wire)?;
            let outcome = match broker_backend!(state, gateway_compliance_feed_transport)
                .resolve(&wire.hostname, timeout)
            {
                Ok(addresses) => {
                    let addresses = addresses
                        .into_iter()
                        .map(IpAddressWireV1::from)
                        .collect::<Vec<_>>();
                    let outcome = GatewayComplianceResolveOutcomeWireV1 {
                        outcome: 0,
                        addresses,
                        found: 0,
                        maximum: 0,
                    };
                    validate_gateway_compliance_resolve_outcome(&outcome)?;
                    outcome
                }
                Err(error) => {
                    let (outcome, found, maximum) = gateway_compliance_error_wire(&error);
                    GatewayComplianceResolveOutcomeWireV1 {
                        outcome,
                        addresses: Vec::new(),
                        found,
                        maximum,
                    }
                }
            };
            requalify()?;
            encode_canonical(&outcome, 128 * 1024)
        }
        (slot, OPERATION_GATEWAY_COMPLIANCE_FETCH_V1) if slot == gateway_compliance_slot => {
            let wire = decode_canonical::<GatewayComplianceFetchRequestWireV1>(
                &request.payload,
                MAX_GATEWAY_COMPLIANCE_FRAME_BYTES_V1,
            )?;
            let (url, pinned_addresses, connect_timeout, total_timeout, max_encoded_bytes) =
                validate_gateway_compliance_fetch_request(&wire)?;
            let fetch = iroha_torii::sorafs::gateway::GatewayComplianceFetchRequest {
                url,
                pinned_addresses,
                connect_timeout,
                total_timeout,
                max_encoded_bytes,
            };
            let outcome = match broker_backend!(state, gateway_compliance_feed_transport)
                .fetch(&fetch)
            {
                Ok(response) => GatewayComplianceFetchOutcomeWireV1 {
                    outcome: 0,
                    status: response.status,
                    redirect_location: response.redirect_location,
                    connected_address: Some(IpAddressWireV1::from(
                        response.connected_address,
                    )),
                    peer_spki_sha256: response.peer_spki_sha256,
                    content_encoding: match response.content_encoding {
                        iroha_torii::sorafs::gateway::
                            GatewayComplianceContentEncoding::Identity => 0,
                        iroha_torii::sorafs::gateway::
                            GatewayComplianceContentEncoding::Gzip => 1,
                        iroha_torii::sorafs::gateway::
                            GatewayComplianceContentEncoding::Zstd => 2,
                    },
                    body: response.body,
                    elapsed: Some(DurationWireV1::from_duration(response.elapsed)),
                    found: 0,
                    maximum: 0,
                },
                Err(error) => {
                    let (outcome, found, maximum) =
                        gateway_compliance_error_wire(&error);
                    GatewayComplianceFetchOutcomeWireV1 {
                        outcome,
                        status: 0,
                        redirect_location: None,
                        connected_address: None,
                        peer_spki_sha256: [0; 32],
                        content_encoding: 0,
                        body: Vec::new(),
                        elapsed: None,
                        found,
                        maximum,
                    }
                }
            };
            validate_gateway_compliance_fetch_outcome(&outcome, &wire)?;
            requalify()?;
            encode_canonical(&outcome, MAX_GATEWAY_COMPLIANCE_FRAME_BYTES_V1)
        }
        (slot, OPERATION_POP_RUNTIME_OPEN_V1) if slot == pop_registry_slot => {
            if pop_session.providers.is_some() {
                return Err(BrokerError::Rejected);
            }
            let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
            let requested = decode_canonical::<PopCredentialRuntimeBindingWireV1>(
                &request.payload,
                MAX_POP_RUNTIME_FRAME_BYTES_V1,
            )?;
            if &requested != exact {
                return Err(BrokerError::BindingMismatch);
            }
            let bindings = pop_runtime_bindings_from_wire(&request.binding)?;
            let registry = broker_backend!(state, pop_credential_provider_registry);
            let providers_result = registry.resolve(&bindings);
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            let providers = providers_result.map_err(|error| {
                match error {
                iroha_torii::sorafs::pop_api::
                    PopCredentialRuntimeProviderRegistryErrorV1::Unavailable => {
                    BrokerError::Unavailable
                }
                iroha_torii::sorafs::pop_api::
                    PopCredentialRuntimeProviderRegistryErrorV1::StaleOrRevoked => {
                    BrokerError::StaleOrRevoked
                }
                iroha_torii::sorafs::pop_api::
                    PopCredentialRuntimeProviderRegistryErrorV1::RejectedBindings => {
                    BrokerError::Rejected
                }
            }
            })?;
            if providers.issuer_signer.key_id() != exact.issuer_signer_handle
                || providers.issuer_signer.public_key() != exact.issuer_public_key
                || providers.enrollment_recipient.key_id() != exact.enrollment_recipient_key_id
                || providers.enrollment_recipient.public_key_digest()
                    != exact.enrollment_recipient_public_key_digest
                || providers.wallet_recipient.key_id() != exact.wallet_recipient_key_id
                || providers.wallet_recipient.public_key_digest()
                    != exact.wallet_recipient_public_key_digest
                || providers.wallet_key_wrapper.active_key_id() != exact.wallet_wrapping_key_id
                || !iroha_config::parameters::is_production_runtime_handle(
                    providers.issuer_signer.key_id(),
                )
                || !iroha_config::parameters::is_production_runtime_handle(
                    providers.enrollment_recipient.key_id(),
                )
                || !iroha_config::parameters::is_production_runtime_handle(
                    providers.wallet_recipient.key_id(),
                )
                || !iroha_config::parameters::is_production_runtime_handle(
                    providers.wallet_key_wrapper.active_key_id(),
                )
            {
                return Err(BrokerError::Ambiguous);
            }
            let outcome = PopRuntimeOpenResultWireV1 {
                issuer_signer_handle: providers.issuer_signer.key_id().to_owned(),
                issuer_public_key: providers.issuer_signer.public_key(),
                enrollment_recipient_key_id: providers.enrollment_recipient.key_id().to_owned(),
                enrollment_recipient_public_key_digest: providers
                    .enrollment_recipient
                    .public_key_digest(),
                wallet_recipient_key_id: providers.wallet_recipient.key_id().to_owned(),
                wallet_recipient_public_key_digest: providers.wallet_recipient.public_key_digest(),
                wallet_wrapping_key_id: providers.wallet_key_wrapper.active_key_id().to_owned(),
            };
            validate_pop_open_result(&outcome, exact).map_err(|_| BrokerError::Ambiguous)?;
            let encoded = encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
                .map_err(|_| BrokerError::Ambiguous)?;
            pop_session.providers = Some(providers);
            Ok(encoded)
        }
        (
            slot,
            operation @ (OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1
            | OPERATION_POP_WALLET_RECIPIENT_OPEN_V1),
        ) if slot == pop_registry_slot => {
            let wire = decode_canonical::<PopRecipientOpenRequestWireV1>(
                &request.payload,
                MAX_POP_RUNTIME_FRAME_BYTES_V1,
            )?;
            validate_pop_recipient_open_request(&wire, operation)?;
            let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
            let providers = pop_session
                .providers
                .as_ref()
                .ok_or(BrokerError::Rejected)?;
            let opened = if operation == OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1 {
                if providers.enrollment_recipient.key_id() != exact.enrollment_recipient_key_id
                    || providers.enrollment_recipient.public_key_digest()
                        != exact.enrollment_recipient_public_key_digest
                {
                    return Err(BrokerError::StaleOrRevoked);
                }
                providers
                    .enrollment_recipient
                    .open_enrollment(&wire.encrypted_payload, &wire.aad)
            } else {
                if providers.wallet_recipient.key_id() != exact.wallet_recipient_key_id
                    || providers.wallet_recipient.public_key_digest()
                        != exact.wallet_recipient_public_key_digest
                {
                    return Err(BrokerError::StaleOrRevoked);
                }
                providers
                    .wallet_recipient
                    .open_wallet_delivery(&wire.encrypted_payload, &wire.aad)
            };
            requalify()?;
            if operation == OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1 {
                if providers.enrollment_recipient.key_id() != exact.enrollment_recipient_key_id
                    || providers.enrollment_recipient.public_key_digest()
                        != exact.enrollment_recipient_public_key_digest
                {
                    return Err(BrokerError::StaleOrRevoked);
                }
            } else if providers.wallet_recipient.key_id() != exact.wallet_recipient_key_id
                || providers.wallet_recipient.public_key_digest()
                    != exact.wallet_recipient_public_key_digest
            {
                return Err(BrokerError::StaleOrRevoked);
            }
            let plaintext = opened.map_err(|error| match error {
                sorafs_node::pop_credentials::PopRecipientOpenErrorV1::Unavailable => {
                    BrokerError::Unavailable
                }
                sorafs_node::pop_credentials::PopRecipientOpenErrorV1::Rejected => {
                    BrokerError::Rejected
                }
            })?;
            let outcome = PopRecipientOpenResultWireV1 { plaintext };
            validate_pop_recipient_open_result(&outcome, operation)?;
            encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_POP_ISSUER_SIGN_V1) if slot == pop_registry_slot => {
            let wire = decode_canonical::<PopIssuerSignRequestWireV1>(
                &request.payload,
                MAX_POP_RUNTIME_FRAME_BYTES_V1,
            )?;
            let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
            let providers = pop_session
                .providers
                .as_ref()
                .ok_or(BrokerError::Rejected)?;
            if providers.issuer_signer.key_id() != exact.issuer_signer_handle
                || providers.issuer_signer.public_key() != exact.issuer_public_key
            {
                return Err(BrokerError::StaleOrRevoked);
            }
            let purpose =
                sorafs_node::pop_credentials::PopIssuerSigningPurposeV1::try_from_wire_id(
                    wire.purpose,
                )
                .ok_or(BrokerError::Rejected)?;
            if wire.digest == [0; 32] {
                return Err(BrokerError::Rejected);
            }
            let signature_result = providers.issuer_signer.sign_digest(purpose, wire.digest);
            requalify()?;
            if providers.issuer_signer.key_id() != exact.issuer_signer_handle
                || providers.issuer_signer.public_key() != exact.issuer_public_key
            {
                return Err(BrokerError::StaleOrRevoked);
            }
            let signature = signature_result.map_err(|_| BrokerError::Rejected)?;
            verify_evidence_viewer_ed25519_signature(
                exact.issuer_public_key,
                signature,
                &wire.digest,
            )?;
            encode_canonical(
                &PopIssuerSignResultWireV1 { signature },
                MAX_POP_RUNTIME_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_POP_AUTHENTICATE_V1) if slot == pop_registry_slot => {
            let wire = decode_canonical::<PopAuthenticateRequestWireV1>(
                &request.payload,
                MAX_POP_RUNTIME_FRAME_BYTES_V1,
            )?;
            let action = pop_action_from_wire(wire.action)?;
            let providers = pop_session
                .providers
                .as_ref()
                .ok_or(BrokerError::Rejected)?;
            let principal_result = providers.authenticator.authenticate(
                &wire.opaque_credential,
                action,
                wire.request_binding,
                wire.now_epoch,
            );
            requalify()?;
            let principal = principal_result.map_err(|_| BrokerError::Rejected)?;
            let outcome = PopAuthenticatedPrincipalWireV1 {
                principal_digest: principal.principal_digest,
                expires_at_epoch: principal.expires_at_epoch,
                caller_signed_transaction: matches!(
                    principal.request_authority,
                    sorafs_node::pop_credentials::PopRequestAuthorityV1::CallerSignedTransaction
                ),
            };
            validate_pop_principal(outcome, &wire)?;
            encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_POP_REGISTRY_SUBMIT_V1) if slot == pop_registry_slot => {
            let wire = decode_canonical::<PopRegistrySubmitRequestWireV1>(
                &request.payload,
                MAX_POP_REGISTRY_OPERATION_BYTES_V1,
            )?;
            let providers = pop_session
                .providers
                .as_ref()
                .ok_or(BrokerError::Rejected)?;
            let submit_result = providers
                .registry_submitter
                .submit(wire.idempotency_key, &wire.operation);
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            submit_result.map_err(|_| BrokerError::Rejected)?;
            encode_canonical(&(), MAX_POP_RUNTIME_FRAME_BYTES_V1)
                .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_POP_REGISTRY_NEXT_V1) if slot == pop_registry_slot => {
            let wire = decode_canonical::<PopRegistryNextRequestWireV1>(
                &request.payload,
                MAX_POP_RUNTIME_FRAME_BYTES_V1,
            )?;
            let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
            let providers = pop_session
                .providers
                .as_ref()
                .ok_or(BrokerError::Rejected)?;
            let projection_result = providers.registry_reader.next_after(wire.cursor);
            requalify()?;
            let projection = projection_result.map_err(|_| BrokerError::Unavailable)?;
            if let Some(projection) = projection.as_ref() {
                validate_pop_projection(projection, exact)?;
            }
            encode_canonical(
                &PopRegistryNextResultWireV1 { projection },
                MAX_POP_PROJECTION_BYTES_V1,
            )
        }
        (slot, OPERATION_POP_ISSUANCE_DRAFT_V1) if slot == pop_registry_slot => {
            let wire = decode_canonical::<PopIssuanceDraftRequestWireV1>(
                &request.payload,
                MAX_POP_RUNTIME_FRAME_BYTES_V1,
            )?;
            let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
            let providers = pop_session
                .providers
                .as_ref()
                .ok_or(BrokerError::Rejected)?;
            let draft_result = providers
                .issuance_draft_provider
                .resolve(wire.request_id, wire.now_epoch);
            requalify()?;
            let draft = draft_result.map_err(|_| BrokerError::Unavailable)?;
            let outcome = PopIssuanceDraftResultWireV1 {
                request_id: draft.request_id,
                credential: draft.credential.clone(),
                commitment_root: draft.commitment_root.clone(),
                revocation_list: draft.revocation_list.clone(),
                witness: PopMembershipWitnessWireV1::from_witness(&draft.witness),
            };
            validate_pop_draft(&outcome, wire, exact)?;
            encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_POP_WALLET_WRAP_DEK_V1) if slot == pop_registry_slot => {
            let wire = decode_canonical::<PopWalletWrapDekRequestWireV1>(
                &request.payload,
                MAX_POP_RUNTIME_FRAME_BYTES_V1,
            )?;
            let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
            let providers = pop_session
                .providers
                .as_ref()
                .ok_or(BrokerError::Rejected)?;
            if providers.wallet_key_wrapper.active_key_id() != exact.wallet_wrapping_key_id {
                return Err(BrokerError::StaleOrRevoked);
            }
            let wrapped_result = providers
                .wallet_key_wrapper
                .wrap_dek(wire.context, &wire.dek);
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            if providers.wallet_key_wrapper.active_key_id() != exact.wallet_wrapping_key_id {
                return Err(BrokerError::Ambiguous);
            }
            let wrapped_dek = wrapped_result.map_err(|_| BrokerError::Rejected)?;
            if wrapped_dek.is_empty() || wrapped_dek.len() > MAX_POP_WRAPPED_DEK_BYTES_V1 {
                return Err(BrokerError::Ambiguous);
            }
            encode_canonical(
                &PopWalletWrapDekResultWireV1 { wrapped_dek },
                MAX_POP_RUNTIME_FRAME_BYTES_V1,
            )
            .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_POP_WALLET_UNWRAP_DEK_V1) if slot == pop_registry_slot => {
            let wire = decode_canonical::<PopWalletUnwrapDekRequestWireV1>(
                &request.payload,
                MAX_POP_RUNTIME_FRAME_BYTES_V1,
            )?;
            let exact = required_binding_ref!(&request.binding, pop_credential_runtime_binding);
            let providers = pop_session
                .providers
                .as_ref()
                .ok_or(BrokerError::Rejected)?;
            if providers.wallet_key_wrapper.active_key_id() != exact.wallet_wrapping_key_id {
                return Err(BrokerError::StaleOrRevoked);
            }
            let dek_result = providers.wallet_key_wrapper.unwrap_dek(
                &wire.key_id,
                wire.context,
                &wire.wrapped_dek,
            );
            requalify()?;
            if providers.wallet_key_wrapper.active_key_id() != exact.wallet_wrapping_key_id {
                return Err(BrokerError::StaleOrRevoked);
            }
            let dek = dek_result.map_err(|_| BrokerError::Rejected)?;
            if dek == [0; 32] {
                return Err(BrokerError::Rejected);
            }
            encode_canonical(
                &PopWalletUnwrapDekResultWireV1 { dek },
                MAX_POP_RUNTIME_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_POP_WALLET_WITNESS_V1) if slot == pop_registry_slot => {
            let wire = decode_canonical::<PopWalletWitnessRequestWireV1>(
                &request.payload,
                MAX_POP_PROJECTION_BYTES_V1,
            )?;
            let providers = pop_session
                .providers
                .as_ref()
                .ok_or(BrokerError::Rejected)?;
            let witness_result = providers
                .wallet_witness_provider
                .resolve(wire.credential_commitment, &wire.projection);
            requalify()?;
            let witness = witness_result.map_err(|_| BrokerError::Unavailable)?;
            let outcome = PopMembershipWitnessWireV1::from_witness(&witness);
            validate_pop_witness_wire(&outcome)?;
            encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_POP_FINALIZED_TIME_V1) if slot == pop_registry_slot => {
            decode_canonical::<()>(&request.payload, MAX_POP_RUNTIME_FRAME_BYTES_V1)?;
            let providers = pop_session
                .providers
                .as_ref()
                .ok_or(BrokerError::Rejected)?;
            let sample_result = providers.finalized_time_provider.sample();
            requalify()?;
            let sample = sample_result.map_err(|_| BrokerError::Unavailable)?;
            let outcome = PopFinalizedTimeResultWireV1 {
                finalized_block_height: sample.finalized_block_height,
                finalized_block_hash: sample.finalized_block_hash,
                finalized_epoch: sample.finalized_epoch,
                observed_epoch: sample.observed_epoch,
            };
            validate_pop_finalized_time(outcome)?;
            encode_canonical(&outcome, MAX_POP_RUNTIME_FRAME_BYTES_V1)
        }
        (slot, OPERATION_POR_REPLAY_ARCHIVE_READINESS_V1) if slot == por_replay_archive_slot => {
            decode_canonical::<()>(
                &request.payload,
                MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1,
            )?;
            broker_backend!(state, por_finalized_replay_archive)
                .check_readiness()
                .map_err(|error| match error {
                    sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable => {
                        BrokerError::Unavailable
                    }
                    sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                })?;
            requalify()?;
            encode_canonical(&(), MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_POR_REPLAY_ARCHIVE_CURRENT_HEAD_V1) if slot == por_replay_archive_slot => {
            decode_canonical::<()>(
                &request.payload,
                MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1,
            )?;
            let exact = por_replay_archive_exact_binding(&request.binding)?;
            let head = broker_backend!(state, por_finalized_replay_archive)
                .current_head()
                .map_err(|error| match error {
                    sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable => {
                        BrokerError::Unavailable
                    }
                    sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                })?;
            if let Some(head) = head {
                validate_por_replay_archive_receipt(&head, exact)?;
            }
            requalify()?;
            encode_canonical(&head, MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
        }
        (slot, OPERATION_POR_REPLAY_ARCHIVE_APPEND_V1) if slot == por_replay_archive_slot => {
            let append = decode_canonical::<PorReplayArchiveAppendRequestWireV1>(
                &request.payload,
                MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1,
            )?;
            let record = validate_por_replay_archive_append_request(&append)?;
            let exact = por_replay_archive_exact_binding(&request.binding)?;
            let (_, configured_bounds) =
                por_replay_archive_configured_proof_bounds(&request.binding)?;
            let archive = broker_backend!(state, por_finalized_replay_archive);
            let receipt = archive
                .append(&record, append.expected_previous_head)
                .map_err(|error| match error {
                    sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable => {
                        BrokerError::Ambiguous
                    }
                    sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                })?;
            receipt
                .validate_record(exact, &record, Some(append.expected_previous_head))
                .map_err(|_| BrokerError::Ambiguous)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            let head = archive
                .current_head()
                .map_err(|_| BrokerError::Ambiguous)?
                .ok_or(BrokerError::Ambiguous)?;
            validate_por_replay_archive_receipt(&head, exact)
                .map_err(|_| BrokerError::Ambiguous)?;
            if head != receipt {
                if head.reputation_sequence() <= receipt.reputation_sequence() {
                    return Err(BrokerError::Ambiguous);
                }
                let readback = archive
                    .lookup(record.challenge_id(), head, configured_bounds)
                    .map_err(|_| BrokerError::Ambiguous)?;
                match readback {
                    sorafs_node::PorFinalizedReplayArchiveLookupV1::Found(readback) => {
                        if readback.record != record || readback.receipt != receipt {
                            return Err(BrokerError::Ambiguous);
                        }
                        readback
                            .validate_at_checkpoint(exact, head, configured_bounds)
                            .map_err(|_| BrokerError::Ambiguous)?;
                    }
                    sorafs_node::PorFinalizedReplayArchiveLookupV1::Absent(_) => {
                        return Err(BrokerError::Ambiguous);
                    }
                }
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&receipt, MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
                .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_POR_REPLAY_ARCHIVE_LOOKUP_V1) if slot == por_replay_archive_slot => {
            let lookup = decode_canonical::<PorReplayArchiveLookupRequestWireV1>(
                &request.payload,
                MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1,
            )?;
            let bounds = validate_por_replay_archive_lookup_request(&lookup, &request.binding)?;
            let exact = por_replay_archive_exact_binding(&request.binding)?;
            let outcome = broker_backend!(state, por_finalized_replay_archive)
                .lookup(lookup.challenge_id, lookup.expected_checkpoint_head, bounds)
                .map_err(|error| match error {
                    sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable => {
                        BrokerError::Unavailable
                    }
                    sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                })?;
            let outcome = por_replay_archive_lookup_to_wire(outcome, &lookup, exact, bounds)?;
            requalify()?;
            encode_canonical(&outcome, MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1)
        }
        (slot, OPERATION_NATIVE_TRANSACTION_SIGN_V1)
            if slot == moderation_transaction_signer_slot =>
        {
            let payload = decode_native_transaction_payload(&request.payload)?;
            ensure_transaction_session_network(&payload, &state.network_id)?;
            let signed = sign_moderation_transaction(state, &payload)?;
            requalify()?;
            encode_canonical(&signed, MAX_NATIVE_SIGNED_TRANSACTION_BYTES_V1)
        }
        (slot, OPERATION_MODERATION_HANDOFF_DELIVER_ONCE_V1)
            if slot == moderation_settlement_handoff_slot
                || slot == moderation_publication_handoff_slot =>
        {
            let wire = decode_canonical::<ModerationDurableHandoffRequestWireV1>(
                &request.payload,
                MAX_MODERATION_HANDOFF_FRAME_BYTES_V1,
            )?;
            let handoff =
                validate_moderation_handoff_request(&wire, slot, Some(&state.network_id))?;
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
        (slot, OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_HEAD_PUBLISH_V1)
            if slot == moderation_publication_handoff_slot =>
        {
            let wire = decode_canonical::<
                ModerationPanelNotificationArchiveHeadPublishRequestWireV1,
            >(&request.payload, MAX_MODERATION_HANDOFF_FRAME_BYTES_V1)?;
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
        (slot, OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_HEAD_READ_V1)
            if slot == moderation_publication_handoff_slot =>
        {
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
        (slot, OPERATION_MODERATION_PANEL_NOTIFICATION_DELIVER_ONCE_V1)
            if slot == moderation_panel_notification_slot =>
        {
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
        (slot, OPERATION_NATIVE_TRANSACTION_SIGN_V1)
            if native_transaction_signer_role_for_slot(slot).is_some() =>
        {
            let payload = decode_native_transaction_payload(&request.payload)?;
            ensure_transaction_session_network(&payload, &state.network_id)?;
            let signed = sign_native_transaction(state, &request.binding, payload)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&signed, MAX_NATIVE_SIGNED_TRANSACTION_BYTES_V1)
                .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_NATIVE_TRANSACTION_SIGN_V1) if slot == soracloud_runtime_signer_slot => {
            let payload = decode_native_transaction_payload(&request.payload)?;
            ensure_transaction_session_network(&payload, &state.network_id)?;
            let backend = qualified_soracloud_runtime_signer(state, &request.binding)?;
            let transaction = backend
                .sign_transaction(payload)
                .map_err(map_soracloud_runtime_signing_error)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&transaction, MAX_NATIVE_SIGNED_TRANSACTION_BYTES_V1)
                .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_SORACLOUD_PROVENANCE_SIGN_V1) if slot == soracloud_runtime_signer_slot => {
            let request_payload = decode_canonical::<SoracloudProvenanceSignRequestWireV1>(
                &request.payload,
                MAX_NATIVE_TRANSACTION_FRAME_BYTES_V1,
            )?;
            let purpose =
                iroha_data_model::soracloud::SoracloudRuntimeProvenancePurposeV1::try_from_wire_id(
                    request_payload.purpose,
                )
                .map_err(|_| BrokerError::Rejected)?;
            iroha_data_model::soracloud::validate_soracloud_runtime_provenance_preimage_v1(
                purpose,
                &request_payload.preimage,
            )
            .map_err(|_| BrokerError::Rejected)?;
            let signer = qualified_soracloud_runtime_signer(state, &request.binding)?;
            let signature = signer
                .sign_provenance(purpose, &request_payload.preimage)
                .map_err(map_soracloud_runtime_signing_error)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&signature, MAX_NATIVE_TRANSACTION_FRAME_BYTES_V1)
                .map_err(|_| BrokerError::Ambiguous)
        }
        (slot, OPERATION_SIGN_V1) if slot == governance_signer_slot => {
            let sign = decode_canonical::<PurposeSignRequestWireV1>(
                &request.payload,
                MAX_OPERATION_FRAME_BYTES_V1,
            )?;
            let purpose = validate_governance_purpose_signing_request(&sign, &request.binding)?;
            let signer = broker_backend!(state, governance_dag_signer);
            let signature = signer
                .sign(purpose, &sign.payload)
                .map_err(|_| BrokerError::Rejected)?;
            if signature == [0; 64] {
                return Err(BrokerError::Rejected);
            }
            requalify()?;
            encode_canonical(
                &SignResultWireV1 { signature },
                MAX_OPERATION_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_GOVERNANCE_REQUEST_AUTHENTICATE_V1)
            if slot == governance_ipfs_auth_slot || slot == governance_head_auth_slot =>
        {
            let wire = decode_canonical::<GovernanceRequestAuthRequestWireV1>(
                &request.payload,
                MAX_GOVERNANCE_REQUEST_AUTH_FRAME_BYTES_V1,
            )?;
            let ingress =
                governance_request_ingress_binding_from_provider_binding(&request.binding)?;
            let descriptor = governance_request_auth_from_wire(&wire, ingress.max_body_bytes())?;
            let (authenticator, expected_scope) = if slot == governance_ipfs_auth_slot {
                (
                    state.backends.governance_dag_ipfs_authenticator.as_ref(),
                    sorafs_node::GovernanceDagAuthenticationScope::Ipfs,
                )
            } else {
                (
                    state.backends.governance_dag_head_authenticator.as_ref(),
                    sorafs_node::GovernanceDagAuthenticationScope::SignedHead,
                )
            };
            if descriptor.scope() != expected_scope || descriptor.scope() != ingress.scope() {
                return Err(BrokerError::BindingMismatch);
            }
            let authenticator = authenticator.ok_or(BrokerError::BindingMismatch)?;
            let envelope = authenticator
                .authenticate(&descriptor)
                .map_err(|_| BrokerError::Unavailable)?;
            let envelope = validate_governance_request_auth_envelope(
                &descriptor,
                governance_request_auth_result_to_wire(&envelope),
                ingress.public_key(),
            )?;
            requalify()?;
            encode_canonical(
                &governance_request_auth_result_to_wire(&envelope),
                MAX_GOVERNANCE_REQUEST_AUTH_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_SEALED_LOAD_V1) if slot == governance_checkpoint_slot => {
            let load = decode_canonical::<SealedLoadRequestWireV1>(
                &request.payload,
                MAX_OPERATION_FRAME_BYTES_V1,
            )?;
            let sealed_slot = sealed_slot_from_wire(load.slot)?;
            let store = broker_backend!(state, governance_dag_checkpoint_store);
            let record = store
                .load(sealed_slot)
                .map_err(|_| BrokerError::Unavailable)?
                .map(|record| SealedRecordWireV1 {
                    generation: record.generation,
                    revision: record.revision,
                    payload: record.payload,
                });
            if let Some(record) = record.as_ref() {
                validate_sealed_record_fields(
                    sealed_slot,
                    record.generation,
                    record.revision,
                    &record.payload,
                )
                .map_err(|_| BrokerError::Protocol)?;
            }
            requalify()?;
            encode_canonical(&record, MAX_OPERATION_FRAME_BYTES_V1)
        }
        (slot, OPERATION_SEALED_COMPARE_AND_SWAP_V1) if slot == governance_checkpoint_slot => {
            let compare = decode_canonical::<SealedCompareAndSwapRequestWireV1>(
                &request.payload,
                MAX_OPERATION_FRAME_BYTES_V1,
            )?;
            let sealed_slot = sealed_slot_from_wire(compare.slot)?;
            let next = sorafs_node::GovernanceDagSealedStateRecord {
                generation: compare.next.generation,
                revision: compare.next.revision,
                payload: compare.next.payload,
            };
            let store = broker_backend!(state, governance_dag_checkpoint_store);
            let current = store
                .load(sealed_slot)
                .map_err(|_| BrokerError::Unavailable)?;
            validate_sealed_successor(
                sealed_slot,
                current.as_ref(),
                compare.expected_revision,
                &next,
            )?;
            store
                .compare_and_swap(sealed_slot, compare.expected_revision, next.clone())
                .map_err(|_| BrokerError::Ambiguous)?;
            let readback = store
                .load(sealed_slot)
                .map_err(|_| BrokerError::Ambiguous)?;
            if readback.as_ref() != Some(&next) {
                return Err(BrokerError::Ambiguous);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
        }
        (slot, OPERATION_SEALED_DELETE_V1) if slot == governance_checkpoint_slot => {
            let delete = decode_canonical::<SealedDeleteRequestWireV1>(
                &request.payload,
                MAX_OPERATION_FRAME_BYTES_V1,
            )?;
            let sealed_slot = sealed_slot_from_wire(delete.slot)?;
            validate_sealed_delete(sealed_slot, delete.expected_revision)?;
            let store = broker_backend!(state, governance_dag_checkpoint_store);
            let current = store
                .load(sealed_slot)
                .map_err(|_| BrokerError::Unavailable)?;
            if let Some(current) = current.as_ref() {
                validate_sealed_record_fields(
                    sealed_slot,
                    current.generation,
                    current.revision,
                    &current.payload,
                )
                .map_err(|_| BrokerError::Protocol)?;
            }
            if current.as_ref().map(|record| record.revision) != Some(delete.expected_revision) {
                return Err(BrokerError::Conflict);
            }
            store
                .delete(sealed_slot, delete.expected_revision)
                .map_err(|_| BrokerError::Ambiguous)?;
            if store
                .load(sealed_slot)
                .map_err(|_| BrokerError::Ambiguous)?
                .is_some()
            {
                return Err(BrokerError::Ambiguous);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
        }
        (slot, OPERATION_PROVIDER_INGEST_RESOLVER_READINESS_V1)
            if slot == provider_resolver_slot =>
        {
            let resolver = broker_backend!(state, provider_ingest_signer_resolver);
            resolver.check_readiness().map_err(|error| match error {
                sorafs_node::ProviderIngestCompletionSignerResolverErrorV1::Unavailable => {
                    BrokerError::Unavailable
                }
                sorafs_node::ProviderIngestCompletionSignerResolverErrorV1::Rejected => {
                    BrokerError::Rejected
                }
            })?;
            requalify()?;
            encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
        }
        (slot, OPERATION_PROVIDER_INGEST_RESOLVE_SIGNER_V1) if slot == provider_resolver_slot => {
            let resolve = decode_canonical::<ProviderIngestResolveSignerRequestWireV1>(
                &request.payload,
                MAX_OPERATION_FRAME_BYTES_V1,
            )?;
            let context = provider_ingest_signer_context_from_wire(&resolve.context)?;
            let expected = provider_ingest_expected_signer_binding(&request.binding)?;
            if !expected
                .qualification
                .matches_authority(&context.provider_owner)
                || expected.qualification.signer_policy != context.signer_policy
            {
                return Err(BrokerError::BindingMismatch);
            }
            let signer = resolved_provider_signer(state, context.clone())?;
            if let Some(signer) = &signer {
                validate_resolved_provider_signer(
                    signer.as_ref(),
                    &expected,
                    &context.provider_owner,
                )?;
            }
            requalify()?;
            encode_canonical(
                &ProviderIngestResolveSignerResultWireV1 {
                    eligible: signer.is_some(),
                },
                MAX_OPERATION_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_PROVIDER_INGEST_SIGN_V1) if slot == provider_signer_slot => {
            let (context, expected, payload) = decode_provider_ingest_sign_operation(request)?;
            let max_signed = usize::try_from(required_binding_value!(
                &request.binding,
                provider_ingest_max_signed_transaction_bytes
            ))
            .map_err(|_| BrokerError::Rejected)?;
            ensure_provider_ingest_completion_payload(&payload, &context, &state.network_id)?;
            let backend =
                resolved_provider_signer(state, context.clone())?.ok_or(BrokerError::Rejected)?;
            validate_resolved_provider_signer(
                backend.as_ref(),
                &expected,
                &context.provider_owner,
            )?;
            let transaction =
                block_on_provider_future(backend.sign(payload.clone()))?.map_err(|error| {
                    match error {
                        sorafs_node::ProviderIngestCompletionSignerErrorV1::Unavailable => {
                            BrokerError::Unavailable
                        }
                        sorafs_node::ProviderIngestCompletionSignerErrorV1::Rejected => {
                            BrokerError::Rejected
                        }
                    }
                })?;
            validate_resolved_provider_signer(
                backend.as_ref(),
                &expected,
                &context.provider_owner,
            )?;
            if transaction.payload() != &payload {
                return Err(BrokerError::Rejected);
            }
            ensure_provider_ingest_completion_transaction(
                &transaction,
                &context,
                &state.network_id,
            )?;
            requalify()?;
            let signed_transaction = encode_canonical(&transaction, max_signed)?;
            encode_canonical(
                &ProviderIngestSignResultWireV1 { signed_transaction },
                MAX_OPERATION_FRAME_BYTES_V1,
            )
        }
        (slot, OPERATION_PROVIDER_INGEST_CHECKPOINT_LOAD_V1)
            if slot == provider_checkpoint_slot =>
        {
            let store = broker_backend!(state, provider_ingest_checkpoint_store);
            let max_bytes =
                required_binding_value!(&request.binding, provider_ingest_checkpoint_max_bytes);
            let record = store.load_latest().map_err(|error| match error {
                sorafs_node::ProviderIngestCheckpointExternalErrorV1::Unavailable => {
                    BrokerError::Unavailable
                }
                sorafs_node::ProviderIngestCheckpointExternalErrorV1::Rejected => {
                    BrokerError::Rejected
                }
                sorafs_node::ProviderIngestCheckpointExternalErrorV1::Ambiguous => {
                    BrokerError::Protocol
                }
            })?;
            let record = record
                .map(|record| {
                    record
                        .to_canonical_bytes(max_bytes)
                        .map_err(|_| BrokerError::Protocol)
                })
                .transpose()?;
            requalify()?;
            encode_canonical(&record, MAX_OPERATION_FRAME_BYTES_V1)
        }
        (slot, OPERATION_PROVIDER_INGEST_CHECKPOINT_COMPARE_AND_SWAP_V1)
            if slot == provider_checkpoint_slot =>
        {
            let compare = decode_canonical::<ProviderIngestCheckpointCompareAndSwapRequestWireV1>(
                &request.payload,
                MAX_OPERATION_FRAME_BYTES_V1,
            )?;
            let max_bytes =
                required_binding_value!(&request.binding, provider_ingest_checkpoint_max_bytes);
            let max_bytes_limit = usize::try_from(max_bytes).map_err(|_| BrokerError::Rejected)?;
            reserve_external_canonical_decode(compare.next_record.len(), max_bytes_limit)?;
            let next = sorafs_node::ProviderIngestSealedCheckpointRecordV1::from_canonical_bytes(
                &compare.next_record,
                max_bytes,
            )
            .map_err(|_| BrokerError::Rejected)?;
            let store = broker_backend!(state, provider_ingest_checkpoint_store);
            let current = store.load_latest().map_err(|error| match error {
                sorafs_node::ProviderIngestCheckpointExternalErrorV1::Unavailable => {
                    BrokerError::Unavailable
                }
                sorafs_node::ProviderIngestCheckpointExternalErrorV1::Rejected => {
                    BrokerError::Rejected
                }
                sorafs_node::ProviderIngestCheckpointExternalErrorV1::Ambiguous => {
                    BrokerError::Protocol
                }
            })?;
            let monotonic = match &current {
                None => {
                    compare.expected_revision.is_none()
                        && next.checkpoint_sequence == 1
                        && next.predecessor_revision.is_none()
                        && next.predecessor_checkpoint_digest.is_none()
                }
                Some(previous) => {
                    compare.expected_revision == Some(previous.revision)
                        && previous
                            .checkpoint_sequence
                            .checked_add(1)
                            .is_some_and(|sequence| sequence == next.checkpoint_sequence)
                        && next.predecessor_revision == Some(previous.revision)
                        && next.predecessor_checkpoint_digest == Some(previous.checkpoint_digest)
                }
            };
            if !monotonic {
                return Err(BrokerError::Rejected);
            }
            store
                .compare_and_swap_latest(compare.expected_revision, &next)
                .map_err(|error| match error {
                    sorafs_node::ProviderIngestCheckpointExternalErrorV1::Unavailable
                    | sorafs_node::ProviderIngestCheckpointExternalErrorV1::Ambiguous => {
                        BrokerError::Ambiguous
                    }
                    sorafs_node::ProviderIngestCheckpointExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                })?;
            let readback = store.load_latest().map_err(|_| BrokerError::Ambiguous)?;
            if readback.as_ref() != Some(&next) {
                return Err(BrokerError::Ambiguous);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
        }
        (slot, OPERATION_PROVIDER_INGEST_RETENTION_LOAD_V1) if slot == provider_retention_slot => {
            let load = decode_canonical::<ProviderIngestRetentionLoadRequestWireV1>(
                &request.payload,
                MAX_OPERATION_FRAME_BYTES_V1,
            )?;
            if state.network_id != load.network_id {
                return Err(BrokerError::BindingMismatch);
            }
            let authority = broker_backend!(state, provider_ingest_retention_authority);
            let record = authority.load_latest(&load.network_id).map_err(|error| {
                match error {
                iroha_core::query::provider_ingest_finalized::
                    ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Unavailable => BrokerError::Unavailable,
                iroha_core::query::provider_ingest_finalized::
                    ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Rejected => BrokerError::Rejected,
                iroha_core::query::provider_ingest_finalized::
                    ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Ambiguous => BrokerError::Protocol,
            }
            })?;
            let record = record
                .map(|record| {
                    record
                        .to_canonical_bytes()
                        .map_err(|_| BrokerError::Protocol)
                })
                .transpose()?;
            requalify()?;
            encode_canonical(&record, MAX_OPERATION_FRAME_BYTES_V1)
        }
        (slot, OPERATION_PROVIDER_INGEST_RETENTION_COMPARE_AND_SWAP_V1)
            if slot == provider_retention_slot =>
        {
            let compare = decode_canonical::<ProviderIngestRetentionCompareAndSwapRequestWireV1>(
                &request.payload,
                MAX_OPERATION_FRAME_BYTES_V1,
            )?;
            if state.network_id != compare.network_id {
                return Err(BrokerError::BindingMismatch);
            }
            reserve_external_canonical_decode(
                compare.next_record.len(),
                MAX_PROVIDER_INGEST_RETENTION_APPROVAL_BYTES_V1,
            )?;
            let next = iroha_core::query::provider_ingest_finalized::
                ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::
                    from_canonical_bytes(&compare.next_record)
                    .map_err(|_| BrokerError::Rejected)?;
            let authority = broker_backend!(state, provider_ingest_retention_authority);
            let current = authority
                .load_latest(&compare.network_id)
                .map_err(|error| {
                    match error {
                iroha_core::query::provider_ingest_finalized::
                    ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Unavailable => BrokerError::Unavailable,
                iroha_core::query::provider_ingest_finalized::
                    ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Rejected => BrokerError::Rejected,
                iroha_core::query::provider_ingest_finalized::
                    ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Ambiguous => BrokerError::Protocol,
            }
                })?;
            let monotonic = match &current {
                None => {
                    compare.expected_revision.is_none()
                        && next.sequence() == 1
                        && next.predecessor_revision().is_none()
                        && next.predecessor_checkpoint_digest().is_none()
                }
                Some(previous) => {
                    compare.expected_revision == Some(previous.revision())
                        && previous
                            .sequence()
                            .checked_add(1)
                            .is_some_and(|sequence| sequence == next.sequence())
                        && next.predecessor_revision() == Some(previous.revision())
                        && next.predecessor_checkpoint_digest()
                            == Some(previous.proposal().checkpoint_digest())
                }
            };
            if !monotonic {
                return Err(BrokerError::Rejected);
            }
            authority
                .compare_and_swap_latest(&compare.network_id, compare.expected_revision, &next)
                .map_err(|error| {
                    match error {
                    iroha_core::query::provider_ingest_finalized::
                        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
                        Unavailable
                    | iroha_core::query::provider_ingest_finalized::
                        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
                        Ambiguous => BrokerError::Ambiguous,
                    iroha_core::query::provider_ingest_finalized::
                        ProviderIngestFinalizedArchiveRetentionAuthorityExternalErrorV1::
                        Rejected => BrokerError::Rejected,
                }
                })?;
            let readback = authority
                .load_latest(&compare.network_id)
                .map_err(|_| BrokerError::Ambiguous)?;
            if readback.as_ref() != Some(&next) {
                return Err(BrokerError::Ambiguous);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
        }
        (slot, OPERATION_REPUTATION_RETENTION_LOAD_V1) if slot == reputation_retention_slot => {
            let load = decode_canonical::<ReputationRetentionLoadRequestWireV1>(
                &request.payload,
                MAX_REPUTATION_RETENTION_FRAME_BYTES_V1,
            )?;
            if state.network_id != load.network_id {
                return Err(BrokerError::BindingMismatch);
            }
            let authority =
                broker_backend!(state, reputation_finalized_archive_retention_authority);
            let record = authority.load_latest(&load.network_id).map_err(|error| {
                match error {
                iroha_core::query::reputation_finalized::
                    ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Unavailable => BrokerError::Unavailable,
                iroha_core::query::reputation_finalized::
                    ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Rejected => BrokerError::Rejected,
                iroha_core::query::reputation_finalized::
                    ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Ambiguous => BrokerError::Protocol,
            }
            })?;
            let record = record
                .map(|record| {
                    let bytes = record
                        .to_canonical_bytes()
                        .map_err(|_| BrokerError::Protocol)?;
                    if bytes.is_empty() || bytes.len() > MAX_REPUTATION_RETENTION_APPROVAL_BYTES_V1
                    {
                        return Err(BrokerError::Protocol);
                    }
                    Ok(bytes)
                })
                .transpose()?;
            requalify()?;
            encode_canonical(&record, MAX_REPUTATION_RETENTION_FRAME_BYTES_V1)
        }
        (slot, OPERATION_REPUTATION_RETENTION_COMPARE_AND_SWAP_V1)
            if slot == reputation_retention_slot =>
        {
            let compare = decode_canonical::<ReputationRetentionCompareAndSwapRequestWireV1>(
                &request.payload,
                MAX_REPUTATION_RETENTION_FRAME_BYTES_V1,
            )?;
            if state.network_id != compare.network_id
                || compare.expected_revision == Some([0; 32])
                || compare.next_record.is_empty()
                || compare.next_record.len() > MAX_REPUTATION_RETENTION_APPROVAL_BYTES_V1
            {
                return Err(BrokerError::BindingMismatch);
            }
            reserve_external_canonical_decode(
                compare.next_record.len(),
                MAX_REPUTATION_RETENTION_APPROVAL_BYTES_V1,
            )?;
            let next = iroha_core::query::reputation_finalized::
                ReputationFinalizedArchiveRetentionApprovalRecordV1::
                    from_canonical_bytes(&compare.next_record)
                    .map_err(|_| BrokerError::Rejected)?;
            let expected_qualification = qualification_from_binding(&request.binding)?;
            let next_qualification = next.authority_qualification();
            if next_qualification.revision() != expected_qualification.revision
                || next_qualification.policy_digest() != expected_qualification.policy_digest
                || next.predecessor_revision() != compare.expected_revision
            {
                return Err(BrokerError::BindingMismatch);
            }
            let authority =
                broker_backend!(state, reputation_finalized_archive_retention_authority);
            let current = authority
                .load_latest(&compare.network_id)
                .map_err(|error| {
                    match error {
                iroha_core::query::reputation_finalized::
                    ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Unavailable => BrokerError::Unavailable,
                iroha_core::query::reputation_finalized::
                    ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Rejected => BrokerError::Rejected,
                iroha_core::query::reputation_finalized::
                    ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
                    Ambiguous => BrokerError::Protocol,
            }
                })?;
            let monotonic = match &current {
                None => {
                    compare.expected_revision.is_none()
                        && next.sequence() == 1
                        && next.predecessor_revision().is_none()
                        && next.predecessor_checkpoint_digest().is_none()
                }
                Some(previous) => {
                    previous.authority_qualification() == next_qualification
                        && compare.expected_revision == Some(previous.revision())
                        && previous
                            .sequence()
                            .checked_add(1)
                            .is_some_and(|sequence| sequence == next.sequence())
                        && next.predecessor_revision() == Some(previous.revision())
                        && next.predecessor_checkpoint_digest()
                            == Some(previous.proposal().checkpoint_digest())
                }
            };
            if !monotonic {
                return Err(BrokerError::Rejected);
            }
            authority
                .compare_and_swap_latest(&compare.network_id, compare.expected_revision, &next)
                .map_err(|error| {
                    match error {
                    iroha_core::query::reputation_finalized::
                        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
                        Unavailable
                    | iroha_core::query::reputation_finalized::
                        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
                        Ambiguous => BrokerError::Ambiguous,
                    iroha_core::query::reputation_finalized::
                        ReputationFinalizedArchiveRetentionAuthorityExternalErrorV1::
                        Rejected => BrokerError::Rejected,
                }
                })?;
            let readback = authority
                .load_latest(&compare.network_id)
                .map_err(|_| BrokerError::Ambiguous)?;
            if readback.as_ref() != Some(&next) {
                return Err(BrokerError::Ambiguous);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&(), MAX_REPUTATION_RETENTION_FRAME_BYTES_V1)
        }
        (slot, OPERATION_EVIDENCE_VIEWER_ISSUE_CHALLENGE_V1) if slot == evidence_webauthn_slot => {
            let issue = decode_canonical::<EvidenceViewerIssueChallengeRequestWireV1>(
                &request.payload,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )?;
            let secret = broker_backend!(state, evidence_viewer_webauthn)
                .issue_challenge(issue.binding_digest, issue.expires_at_unix_ms)
                .map_err(|error| match error {
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                        BrokerError::Ambiguous
                    }
                })?;
            let result = EvidenceViewerSecretResultWireV1 {
                secret: secret.expose().as_bytes().to_vec(),
            };
            validate_evidence_viewer_secret(&result.secret)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&result, MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
        }
        (slot, OPERATION_EVIDENCE_VIEWER_VERIFY_AND_CONSUME_V1)
            if slot == evidence_webauthn_slot =>
        {
            let verify = decode_canonical::<EvidenceViewerVerifyAndConsumeRequestWireV1>(
                &request.payload,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )?;
            let configured =
                required_binding_ref!(&request.binding, evidence_viewer_webauthn_binding);
            validate_evidence_viewer_verify_and_consume_wire(&verify, configured)?;
            let challenge = validate_evidence_viewer_secret(&verify.challenge)?;
            let result = broker_backend!(state, evidence_viewer_webauthn)
                .verify_and_consume(
                    challenge,
                    &verify.assertion,
                    verify.binding_digest,
                    &verify.rp_id,
                    &verify.allowed_origins,
                    verify.now_unix_ms,
                )
                .map_err(|error| match error {
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                        BrokerError::Ambiguous
                    }
                })?;
            if result.attestation_digest == [0; 32] || result.credential_id_digest == [0; 32] {
                return Err(BrokerError::Rejected);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(
                &EvidenceViewerWebAuthnResultWireV1 {
                    attestation_digest: result.attestation_digest,
                    credential_id_digest: result.credential_id_digest,
                    authenticator_counter: result.authenticator_counter,
                },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
        }
        (slot, OPERATION_EVIDENCE_VIEWER_GRANT_ISSUE_V1) if slot == evidence_grants_slot => {
            let issue = decode_canonical::<EvidenceViewerGrantIssueRequestWireV1>(
                &request.payload,
                MAX_EVIDENCE_VIEWER_CLAIMS_BYTES_V1,
            )?;
            let secret = broker_backend!(state, evidence_viewer_grants)
                .issue(&issue.claims)
                .map_err(|error| match error {
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                        BrokerError::Ambiguous
                    }
                })?;
            let result = EvidenceViewerSecretResultWireV1 {
                secret: secret.expose().as_bytes().to_vec(),
            };
            validate_evidence_viewer_secret(&result.secret)?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&result, MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
        }
        (slot, OPERATION_EVIDENCE_VIEWER_GRANT_VERIFY_V1) if slot == evidence_grants_slot => {
            let verify = decode_canonical::<EvidenceViewerGrantVerifyRequestWireV1>(
                &request.payload,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )?;
            let token = validate_evidence_viewer_secret(&verify.token)?;
            broker_backend!(state, evidence_viewer_grants)
                .verify(token, &verify.claims, verify.now_unix_ms)
                .map_err(|error| match error {
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                        BrokerError::Unavailable
                    }
                })?;
            requalify()?;
            encode_canonical(&(), MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
        }
        (slot, OPERATION_EVIDENCE_VIEWER_GRANT_REVOKE_V1) if slot == evidence_grants_slot => {
            let revoke = decode_canonical::<EvidenceViewerGrantRevokeRequestWireV1>(
                &request.payload,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )?;
            broker_backend!(state, evidence_viewer_grants)
                .revoke(revoke.token_digest)
                .map_err(|error| match error {
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                        BrokerError::Ambiguous
                    }
                })?;
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(&(), MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
        }
        (slot, OPERATION_EVIDENCE_VIEWER_RECEIPT_SIGN_V1)
            if slot == evidence_receipt_signer_slot =>
        {
            let sign = decode_canonical::<PurposeSignRequestWireV1>(
                &request.payload,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )?;
            let purpose = validate_evidence_purpose_signing_request(&sign, &request.binding)?;
            let signer = broker_backend!(state, evidence_viewer_receipt_signer);
            let signature = signer
                .sign(purpose, &sign.payload)
                .map_err(|error| match error {
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                        BrokerError::Unavailable
                    }
                })?;
            let public_key = required_binding_value!(
                &request.binding,
                evidence_viewer_receipt_signer_public_key
            );
            verify_evidence_viewer_ed25519_signature(public_key, signature, &sign.payload)?;
            requalify()?;
            encode_canonical(
                &SignResultWireV1 { signature },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
        }
        (slot, OPERATION_EVIDENCE_VIEWER_ERASE_V1) if slot == evidence_erasure_slot => {
            let erase = decode_canonical::<EvidenceViewerEraseRequestWireV1>(
                &request.payload,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )?;
            let commit_digest = broker_backend!(state, evidence_viewer_erasure)
                .erase(
                    erase.operation_id,
                    erase.quarantine_id,
                    erase.object_id,
                    erase.evidence_digest,
                )
                .map_err(|error| match error {
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                        BrokerError::Ambiguous
                    }
                })?;
            if commit_digest == [0; 32] {
                return Err(BrokerError::Rejected);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(
                &EvidenceViewerEraseResultWireV1 { commit_digest },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
        }
        (slot, OPERATION_EVIDENCE_VIEWER_CHECKPOINT_LOAD_V1)
            if slot == evidence_checkpoint_slot =>
        {
            let store = broker_backend!(state, evidence_viewer_checkpoint_store);
            let record = store.load_latest().map_err(|error| {
                match error {
                    sorafs_node::evidence_viewer::
                        EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::
                        EvidenceViewerCheckpointStoreExternalErrorV1::Ambiguous => {
                            BrokerError::Unavailable
                        }
                    sorafs_node::evidence_viewer::
                        EvidenceViewerCheckpointStoreExternalErrorV1::Rejected => {
                            BrokerError::Rejected
                        }
                }
            })?;
            let record = record
                .map(|record| {
                    let bytes = encode_canonical(
                        &record,
                        evidence_viewer_checkpoint_record_limit(&request.binding)?,
                    )?;
                    decode_evidence_viewer_checkpoint_record(&bytes, &request.binding)?;
                    Ok(bytes)
                })
                .transpose()?;
            requalify()?;
            encode_canonical(&record, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
        }
        (slot, OPERATION_EVIDENCE_VIEWER_CHECKPOINT_COMPARE_AND_SWAP_V1)
            if slot == evidence_checkpoint_slot =>
        {
            let compare = decode_canonical::<EvidenceViewerCheckpointCompareAndSwapRequestWireV1>(
                &request.payload,
                MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
            )?;
            let next =
                decode_evidence_viewer_checkpoint_record(&compare.next_record, &request.binding)?;
            let store = broker_backend!(state, evidence_viewer_checkpoint_store);
            let current = store.load_latest().map_err(|error| {
                match error {
                sorafs_node::evidence_viewer::
                    EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable
                | sorafs_node::evidence_viewer::
                    EvidenceViewerCheckpointStoreExternalErrorV1::Ambiguous => {
                    BrokerError::Unavailable
                }
                sorafs_node::evidence_viewer::
                    EvidenceViewerCheckpointStoreExternalErrorV1::Rejected => {
                    BrokerError::Rejected
                }
            }
            })?;
            if let Some(current) = current.as_ref() {
                let current_bytes = encode_canonical(
                    current,
                    evidence_viewer_checkpoint_record_limit(&request.binding)?,
                )?;
                decode_evidence_viewer_checkpoint_record(&current_bytes, &request.binding)
                    .map_err(|_| BrokerError::Protocol)?;
            }
            validate_evidence_viewer_checkpoint_successor(
                current.as_ref(),
                compare.expected_revision,
                &next,
            )?;
            store
                .compare_and_swap_latest(compare.expected_revision, &next)
                .map_err(|error| {
                    match error {
                    sorafs_node::evidence_viewer::
                        EvidenceViewerCheckpointStoreExternalErrorV1::Rejected => {
                        BrokerError::Conflict
                    }
                    sorafs_node::evidence_viewer::
                        EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::
                        EvidenceViewerCheckpointStoreExternalErrorV1::Ambiguous => {
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
        (slot, OPERATION_MODERATION_CHECKPOINT_LOAD_V1) if slot == moderation_checkpoint_slot => {
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
                    decode_moderation_checkpoint_record(
                        &bytes,
                        &request.binding,
                        Some(&state.network_id),
                    )?;
                    Ok(bytes)
                })
                .transpose()?;
            requalify()?;
            encode_canonical(&record, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
        }
        (slot, OPERATION_MODERATION_CHECKPOINT_COMPARE_AND_SWAP_V1)
            if slot == moderation_checkpoint_slot =>
        {
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
                decode_moderation_checkpoint_record(
                    &bytes,
                    &request.binding,
                    Some(&state.network_id),
                )
                .map_err(|_| BrokerError::Protocol)?;
            }
            validate_moderation_checkpoint_successor(
                current.as_ref(),
                compare.expected_revision,
                &next,
            )?;
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
        (slot, OPERATION_MODERATION_PANEL_NOTIFICATION_SOURCE_ATTEST_V1)
            if slot == moderation_checkpoint_slot =>
        {
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
        (slot, OPERATION_EVIDENCE_VIEWER_ARCHIVE_INSTALL_V1) if slot == evidence_archive_slot => {
            let install = decode_canonical::<EvidenceViewerArchiveInstallRequestWireV1>(
                &request.payload,
                MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
            )?;
            let archive = broker_backend!(state, evidence_viewer_compaction_archive);
            let signature = archive
                .install(
                    install.operation_id,
                    install.receipt_message,
                    &install.canonical_artifact,
                )
                .map_err(|error| match error {
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                        BrokerError::Ambiguous
                    }
                })?;
            let public_key =
                required_binding_value!(&request.binding, evidence_viewer_archive_public_key);
            verify_evidence_viewer_ed25519_signature(
                public_key,
                signature,
                &install.receipt_message,
            )?;
            let readback = archive
                .read(install.operation_id)
                .map_err(|_| BrokerError::Ambiguous)?
                .ok_or(BrokerError::Ambiguous)?;
            if readback.canonical_artifact != install.canonical_artifact
                || readback.signature != signature
            {
                return Err(BrokerError::Ambiguous);
            }
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            encode_canonical(
                &SignResultWireV1 { signature },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
        }
        (slot, OPERATION_EVIDENCE_VIEWER_ARCHIVE_READ_V1) if slot == evidence_archive_slot => {
            let read = decode_canonical::<EvidenceViewerArchiveReadRequestWireV1>(
                &request.payload,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )?;
            let max_bytes = usize::try_from(required_binding_value!(
                &request.binding,
                evidence_viewer_archive_max_bytes
            ))
            .map_err(|_| BrokerError::Rejected)?;
            let readback = broker_backend!(state, evidence_viewer_compaction_archive)
                .read(read.operation_id)
                .map_err(|error| match error {
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                    sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Backpressure => {
                        BrokerError::Unavailable
                    }
                })?
                .map(|readback| {
                    if readback.canonical_artifact.is_empty()
                        || readback.canonical_artifact.len() > max_bytes
                        || readback.signature == [0; 64]
                    {
                        return Err(BrokerError::Rejected);
                    }
                    Ok(EvidenceViewerArchiveReadbackWireV1 {
                        canonical_artifact: readback.canonical_artifact,
                        signature: readback.signature,
                    })
                })
                .transpose()?;
            requalify()?;
            encode_canonical(&readback, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
        }
        (slot, OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_INSTALL_V1)
            if slot == moderation_panel_notification_archive_slot =>
        {
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
            let validated =
                validate_moderation_panel_notification_archive_artifact_at_broker_boundary(
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
            if readback.canonical_artifact != install.canonical_artifact
                || readback.signature != signature
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
        (slot, OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_READ_V1)
            if slot == moderation_panel_notification_archive_slot =>
        {
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
        (slot, OPERATION_EVIDENCE_VIEWER_TRANSPARENCY_LOAD_V1)
            if slot == evidence_transparency_publisher_slot =>
        {
            let publisher = broker_backend!(state, evidence_viewer_transparency_publisher);
            let head = publisher.load_head().map_err(|error| {
                match error {
                    sorafs_node::evidence_viewer::transparency_producer::
                        EvidenceViewerTransparencyPublisherExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::transparency_producer::
                        EvidenceViewerTransparencyPublisherExternalErrorV1::Backpressure
                    | sorafs_node::evidence_viewer::transparency_producer::
                        EvidenceViewerTransparencyPublisherExternalErrorV1::Ambiguous => {
                        BrokerError::Unavailable
                    }
                    sorafs_node::evidence_viewer::transparency_producer::
                        EvidenceViewerTransparencyPublisherExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                }
            })?;
            if let Some(head) = head.as_ref() {
                validate_evidence_viewer_transparency_head_body(&head.body, &request.binding)?;
                if head.signature == [0; 64] || head.head_digest == [0; 32] {
                    return Err(BrokerError::Rejected);
                }
            }
            requalify()?;
            encode_canonical(&head, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
        }
        (slot, OPERATION_EVIDENCE_VIEWER_TRANSPARENCY_COMPARE_AND_PUBLISH_V1)
            if slot == evidence_transparency_publisher_slot =>
        {
            let body = decode_canonical::<
                sorafs_node::evidence_viewer::transparency_producer::
                    EvidenceViewerTransparencyHeadBodyV1,
            >(
                &request.payload, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1
            )?;
            validate_evidence_viewer_transparency_head_body(&body, &request.binding)?;
            let publisher = broker_backend!(state, evidence_viewer_transparency_publisher);
            let publish_result = publisher.compare_and_publish(&body);
            requalify().map_err(|_| BrokerError::Ambiguous)?;
            publish_result.map_err(|error| {
                match error {
                    sorafs_node::evidence_viewer::transparency_producer::
                        EvidenceViewerTransparencyPublisherExternalErrorV1::Rejected => {
                        BrokerError::Rejected
                    }
                    sorafs_node::evidence_viewer::transparency_producer::
                        EvidenceViewerTransparencyPublisherExternalErrorV1::Unavailable
                    | sorafs_node::evidence_viewer::transparency_producer::
                        EvidenceViewerTransparencyPublisherExternalErrorV1::Backpressure => {
                        BrokerError::Unavailable
                    }
                    sorafs_node::evidence_viewer::transparency_producer::
                        EvidenceViewerTransparencyPublisherExternalErrorV1::Ambiguous => {
                        BrokerError::Ambiguous
                    }
                }
            })?;
            encode_canonical(&(), MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
        }
        _ => Err(BrokerError::BindingMismatch),
    };
    result.map(ScrubbedBytes::new)
}
#[cfg(test)]
fn dispatch_server_operation(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<ScrubbedBytes, BrokerError> {
    dispatch_server_operation_with_session(state, &mut PopBrokerServerSessionV1::default(), request)
}
