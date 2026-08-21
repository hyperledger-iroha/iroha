#[derive(Clone, Default)]
struct CountingSoracloudRuntime {
    ordered_mailbox_calls: Arc<parking_lot::Mutex<Vec<Hash>>>,
    state_mutations: Vec<SoracloudDeterministicStateMutation>,
}
impl CountingSoracloudRuntime {
    fn ordered_mailbox_call_count(&self) -> usize {
        self.ordered_mailbox_calls.lock().len()
    }
    fn with_state_mutations(state_mutations: Vec<SoracloudDeterministicStateMutation>) -> Self {
        Self {
            ordered_mailbox_calls: Arc::default(),
            state_mutations,
        }
    }
}
impl SoracloudRuntimeReadHandle for CountingSoracloudRuntime {
    fn snapshot(&self) -> SoracloudRuntimeSnapshot {
        SoracloudRuntimeSnapshot::default()
    }
    fn state_dir(&self) -> PathBuf {
        PathBuf::from("/tmp/iroha-soracloud-runtime-test")
    }
}
impl SoracloudRuntime for CountingSoracloudRuntime {
    fn execute_local_read(
        &self,
        _request: SoracloudLocalReadRequest,
    ) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
        Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "local reads are not used in this test runtime",
        ))
    }
    fn execute_ordered_mailbox(
        &self,
        request: SoracloudOrderedMailboxExecutionRequest,
    ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError> {
        self.ordered_mailbox_calls
            .lock()
            .push(request.mailbox_message.message_id);
        Ok(SoracloudOrderedMailboxExecutionResult {
            state_mutations: self.state_mutations.clone(),
            outbound_mailbox_messages: Vec::new(),
            response_bytes: Vec::new(),
            content_type: None,
            runtime_state: Some(SoraServiceRuntimeStateV1 {
                schema_version: iroha_data_model::soracloud::SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
                service_name: request.deployment.service_name.clone(),
                active_service_version: request.deployment.current_service_version.clone(),
                health_status: SoraServiceHealthStatusV1::Healthy,
                load_factor_bps: 111,
                materialized_bundle_hash: request.bundle.container.bundle_hash,
                rollout_handle: request
                    .deployment
                    .active_rollout
                    .as_ref()
                    .map(|rollout| rollout.rollout_handle.clone()),
                pending_mailbox_message_count: request
                    .authoritative_pending_mailbox_messages
                    .saturating_sub(1),
                last_receipt_id: None,
            }),
            runtime_receipt: SoraRuntimeReceiptV1 {
                schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id: Hash::new(
                    format!(
                        "test-receipt:{}:{}",
                        request.deployment.service_name, request.mailbox_message.message_id
                    )
                    .as_bytes(),
                ),
                service_name: request.deployment.service_name,
                service_version: request.deployment.current_service_version,
                handler_name: request.mailbox_message.to_handler.clone(),
                handler_class: request
                    .handler
                    .as_ref()
                    .map(|handler| handler.class)
                    .unwrap_or(SoraServiceHandlerClassV1::Update),
                request_commitment: request.mailbox_message.payload_commitment,
                result_commitment: Hash::new(
                    format!("test-result:{}", request.mailbox_message.message_id).as_bytes(),
                ),
                certified_by: SoraCertifiedResponsePolicyV1::None,
                emitted_sequence: request.execution_sequence,
                mailbox_message_id: Some(request.mailbox_message.message_id),
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                placement_id: None,
                selected_validator_account_id: None,
                selected_peer_id: None,
            },
        })
    }
    fn execute_apartment(
        &self,
        _request: SoracloudApartmentExecutionRequest,
    ) -> Result<SoracloudApartmentExecutionResult, SoracloudRuntimeExecutionError> {
        Err(SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "apartments are not used in this test runtime",
        ))
    }
}
fn seed_soracloud_mailbox_fixture(
    world: &mut World,
    state_bindings: Vec<SoraStateBindingV1>,
) -> (iroha_data_model::name::Name, Hash) {
    let service_name: iroha_data_model::name::Name = "portal".parse().expect("valid service name");
    let service_version = "2026.1".to_string();
    let bundle_hash = Hash::new(b"bundle:portal:2026.1");
    let bundle = SoraDeploymentBundleV1 {
        schema_version: iroha_data_model::soracloud::SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container: SoraContainerManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_CONTAINER_MANIFEST_VERSION_V1,
            runtime: SoraContainerRuntimeV1::Ivm,
            bundle_hash,
            bundle_path: "/bundles/portal.ivm".to_string(),
            entrypoint: "main".to_string(),
            args: Vec::new(),
            env: std::collections::BTreeMap::new(),
            inrou: None,
            required_config_names: Vec::new(),
            required_secret_names: Vec::new(),
            config_exports: Vec::new(),
            capabilities: SoraCapabilityPolicyV1 {
                network: SoraNetworkPolicyV1::Isolated,
                allow_wallet_signing: false,
                allow_state_writes: false,
                allow_model_inference: false,
                allow_model_training: false,
            },
            resources: SoraResourceLimitsV1 {
                cpu_millis: NonZeroU32::new(500).expect("nonzero cpu"),
                memory_bytes: NonZeroU64::new(16 * 1024 * 1024).expect("nonzero memory"),
                ephemeral_storage_bytes: NonZeroU64::new(16 * 1024 * 1024)
                    .expect("nonzero storage"),
                max_open_files: NonZeroU32::new(256).expect("nonzero files"),
                max_tasks: NonZeroU16::new(16).expect("nonzero tasks"),
            },
            lifecycle: SoraLifecycleHooksV1 {
                start_grace_secs: NonZeroU32::new(5).expect("nonzero start grace"),
                stop_grace_secs: NonZeroU32::new(5).expect("nonzero stop grace"),
                healthcheck_path: Some("/health".to_string()),
            },
        },
        service: SoraServiceManifestV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_MANIFEST_VERSION_V1,
            service_name: service_name.clone(),
            service_version: service_version.clone(),
            execution_plane:
                iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::DeterministicService,
            container: SoraContainerManifestRefV1 {
                manifest_hash: Hash::new(b"container-manifest:portal"),
                expected_schema_version:
                    iroha_data_model::soracloud::SORA_CONTAINER_MANIFEST_VERSION_V1,
            },
            replicas: NonZeroU16::new(1).expect("nonzero replicas"),
            route: None,
            rollout: SoraRolloutPolicyV1 {
                canary_percent: 0,
                max_unavailable_replicas: 0,
                health_window_secs: NonZeroU32::new(30).expect("nonzero health window"),
                automatic_rollback_failures: NonZeroU32::new(1).expect("nonzero rollback"),
            },
            economics: iroha_data_model::soracloud::SoraHttpServiceEconomicsV1::default(),
            state_bindings,
            lease_volumes: Vec::new(),
            handlers: vec![SoraServiceHandlerV1 {
                handler_name: "update".parse().expect("valid handler name"),
                class: SoraServiceHandlerClassV1::Update,
                entrypoint: "apply_update".to_string(),
                route_path: Some("/update".to_string()),
                certified_response: SoraCertifiedResponsePolicyV1::None,
                mailbox: Some(SoraMailboxContractV1 {
                    queue_name: "updates".parse().expect("valid queue name"),
                    max_pending_messages: NonZeroU32::new(1_024).expect("nonzero pending limit"),
                    max_message_bytes: NonZeroU64::new(65_536).expect("nonzero message limit"),
                    retention_blocks: NonZeroU32::new(1_440).expect("nonzero retention"),
                }),
            }],
            artifacts: Vec::new(),
        },
    };
    world.soracloud_service_revisions_mut_for_testing().insert(
        (service_name.as_ref().to_owned(), service_version.clone()),
        bundle.clone(),
    );
    world
        .soracloud_service_deployments_mut_for_testing()
        .insert(
            service_name.clone(),
            SoraServiceDeploymentStateV1 {
                schema_version:
                    iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                service_name: service_name.clone(),
                current_service_version: service_version.clone(),
                current_service_manifest_hash: Hash::new(b"service-manifest:portal"),
                current_container_manifest_hash: Hash::new(b"container-manifest:portal"),
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
                service_lease: None,
                lease_volume_states: Vec::new(),
            },
        );
    world.soracloud_service_runtime_mut_for_testing().insert(
        service_name.clone(),
        SoraServiceRuntimeStateV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
            service_name: service_name.clone(),
            active_service_version: service_version,
            health_status: SoraServiceHealthStatusV1::Healthy,
            load_factor_bps: 77,
            materialized_bundle_hash: bundle_hash,
            rollout_handle: None,
            pending_mailbox_message_count: 1,
            last_receipt_id: None,
        },
    );
    let message_id = Hash::new(b"portal-mailbox-message");
    world.soracloud_mailbox_messages_mut_for_testing().insert(
        message_id,
        SoraServiceMailboxMessageV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
            message_id,
            from_service: service_name.clone(),
            from_handler: "update".parse().expect("valid from handler"),
            to_service: service_name.clone(),
            to_handler: "update".parse().expect("valid to handler"),
            payload_bytes: b"portal-mailbox-payload".to_vec(),
            payload_commitment: Hash::new(b"portal-mailbox-payload"),
            enqueue_sequence: 1,
            available_after_sequence: 1,
            expires_at_sequence: Some(16),
        },
    );
    (service_name, message_id)
}
#[test]
fn try_sign_adds_verifiable_signature_and_clears_verified_flag() {
    let key_pairs = core::iter::repeat_with(|| {
        crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal)
    })
    .take(2)
    .collect::<Vec<_>>();
    let topology = test_topology_with_keys(&key_pairs);
    let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
    block.mark_signatures_verified();
    assert!(block.signatures_verified_for_tests());
    block
        .try_sign(&key_pairs[1], &topology)
        .expect("checked valid-block signing succeeds");
    let signature = block
        .as_ref()
        .signatures()
        .find(|signature| signature.index() == 1)
        .expect("signature for requested validator is present");
    signature
        .signature()
        .verify_hash(key_pairs[1].public_key(), block.as_ref().hash())
        .expect("checked valid-block signature verifies");
    assert!(!block.signatures_verified_for_tests());
}
fn sccp_transfer_payload() -> iroha_sccp::SccpPayloadV1 {
    iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
        nonce: 1,
        route_revision: 1,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        asset_id: b"xor".to_vec(),
        amount: 10,
        sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        sender: b"bridge@sora".to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        recipient: vec![0x22; 20],
        route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        route_id: b"nexus:eth:xor".to_vec(),
    })
}
fn sccp_chain_id() -> ChainId {
    "00000000-0000-0000-0000-000000000000"
        .parse()
        .expect("valid chain id")
}
fn sccp_state_with_account(account_id: &AccountId) -> State {
    let domain_id = DomainId::try_new("sccp", "universal").expect("domain id");
    let domain = Domain::new(domain_id).build(account_id);
    let account = Account::new(account_id.clone()).build(account_id);
    let world = World::with([domain], [account], []);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    State::new_with_chain(world, kura, query_handle, sccp_chain_id())
}
fn sccp_accepted_transaction_with_record_count(
    account_id: AccountId,
    keypair: &KeyPair,
    record_count: usize,
) -> AcceptedTransaction<'static> {
    let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&sccp_transfer_payload())
        .expect("valid SCCP block fixture payload encodes");
    let overlay = core::iter::repeat_with(|| {
        InstructionBox::from(crate::bridge::test_record_sccp_message(
            payload_bytes.clone(),
        ))
    })
    .take(record_count)
    .collect::<Vec<_>>();
    sccp_accepted_transaction_with_overlay(account_id, keypair, overlay)
}
fn sccp_accepted_transaction_with_overlay(
    account_id: AccountId,
    keypair: &KeyPair,
    overlay: Vec<InstructionBox>,
) -> AcceptedTransaction<'static> {
    let mut bytecode = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: ivm::ivm_mode::ZK,
        vector_length: 0,
        max_cycles: 1,
        abi_version: 1,
    }
    .encode();
    bytecode.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let tx = TransactionBuilder::new(
        deterministic_test_network_id(0x04),
        account_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(Executable::IvmProved(IvmProved {
        bytecode: IvmBytecode::from_compiled(bytecode),
        overlay: overlay.into(),
        events_commitment: Hash::new(b"events"),
        gas_policy_commitment: Hash::new(b"gas"),
    }))
    .sign(keypair.private_key());
    AcceptedTransaction::new_unchecked(Cow::Owned(tx))
}
fn sccp_accepted_transaction() -> AcceptedTransaction<'static> {
    let (account_id, keypair) = gen_account_in("sccp");
    sccp_accepted_transaction_with_record_count(account_id, &keypair, 1)
}
fn signed_sccp_block(root: Option<[u8; 32]>) -> SignedBlock {
    let leader = crate::block::checked_keypair();
    BlockBuilder::new(vec![sccp_accepted_transaction()])
        .chain(0, None)
        .with_sccp_commitment_root(root)
        .sign(leader.private_key())
        .unpack(|_| {})
        .into()
}
fn set_single_sccp_transaction_result(
    block: &mut SignedBlock,
    result: iroha_data_model::transaction::TransactionResultInner,
) {
    let hashes = block
        .external_transactions()
        .map(|transaction| transaction.hash_as_entrypoint())
        .collect::<Vec<_>>();
    block
        .set_transaction_results_with_transcripts(
            Vec::new(),
            &hashes,
            vec![result],
            std::collections::BTreeMap::new(),
            Vec::new(),
            AxtPolicySnapshot::default(),
        )
        .expect("SCCP test block entrypoint hashes should match");
}
#[test]
fn sccp_commitment_root_validation_accepts_matching_root() {
    let mut block = signed_sccp_block(None);
    set_single_sccp_transaction_result(
        &mut block,
        Ok(iroha_data_model::transaction::DataTriggerSequence::default()),
    );
    let messages = crate::bridge::collect_sccp_messages_from_signed_block(&block);
    let root = crate::bridge::sccp_commitment_root_from_messages(&messages);
    block.set_sccp_commitment_root(root);
    ValidBlock::validate_sccp_commitment_root(&block)
        .expect("matching SCCP commitment root should validate");
}
#[test]
fn sccp_commitment_root_validation_rejects_wrong_root() {
    let mut block = signed_sccp_block(Some([0xAA; 32]));
    set_single_sccp_transaction_result(
        &mut block,
        Ok(iroha_data_model::transaction::DataTriggerSequence::default()),
    );
    let err = ValidBlock::validate_sccp_commitment_root(&block)
        .expect_err("wrong SCCP commitment root should reject");
    assert!(matches!(
        err,
        BlockValidationError::SccpCommitmentRootMismatch {
            actual: Some(_),
            ..
        }
    ));
}
#[test]
fn sccp_commitment_root_validation_rejects_resultless_root() {
    let block = signed_sccp_block(Some([0xAA; 32]));
    let err = ValidBlock::validate_sccp_commitment_root(&block)
        .expect_err("SCCP commitment root without committed results should reject");
    assert!(matches!(
        err,
        BlockValidationError::SccpCommitmentRootMismatch {
            expected: None,
            actual: Some(_),
        }
    ));
}
#[test]
fn sccp_commitment_root_validation_rejects_short_result_vector() {
    let (plain_account, plain_keypair) = gen_account_in("sccp");
    let plain_tx = TransactionBuilder::new(
        deterministic_test_network_id(0x04),
        plain_account,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(plain_keypair.private_key());
    let plain_hash = plain_tx.hash_as_entrypoint();
    let sccp_entrypoint = sccp_accepted_transaction().entrypoint().clone();
    let accepted_plain = AcceptedTransaction::new_unchecked(Cow::Owned(plain_tx.clone()));
    let leader = crate::block::checked_keypair();
    let mut block: SignedBlock = BlockBuilder::new(vec![accepted_plain])
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {})
        .into();
    block
        .set_transaction_results(
            Vec::new(),
            &[plain_hash],
            vec![Ok(
                iroha_data_model::transaction::DataTriggerSequence::default(),
            )],
        )
        .expect("single plain transaction result should attach");
    block.set_external_entrypoints(vec![
        iroha_data_model::transaction::TransactionEntrypoint::External(plain_tx),
        sccp_entrypoint,
    ]);
    let err = ValidBlock::validate_sccp_commitment_root(&block).expect_err(
        "SCCP validation must reject external SCCP entrypoints without committed results",
    );
    assert!(matches!(
        err,
        BlockValidationError::SccpTransactionResultCountMismatch {
            external_entrypoints: 2,
            results: 1,
        }
    ));
}
#[test]
fn sccp_commitment_root_validation_rejects_invalid_successful_record_payload() {
    let (account_id, keypair) = gen_account_in("sccp");
    let accepted = sccp_accepted_transaction_with_overlay(
        account_id,
        &keypair,
        vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(b"not a canonical SCCP payload".to_vec()),
        )],
    );
    let leader = crate::block::checked_keypair();
    let mut block: SignedBlock = BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {})
        .into();
    set_single_sccp_transaction_result(
        &mut block,
        Ok(iroha_data_model::transaction::DataTriggerSequence::default()),
    );
    let err = ValidBlock::validate_sccp_commitment_root(&block)
        .expect_err("successful invalid SCCP record payload must not be hidden by an empty root");
    assert!(matches!(
        err,
        BlockValidationError::SccpInvalidOutboundRecord {
            tx_index: 0,
            instruction_index: 0,
            reason
        } if reason.contains("payload is invalid")
    ));
}
#[test]
fn sccp_commitment_root_validation_rejects_root_without_messages() {
    let leader = crate::block::checked_keypair();
    let mut block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .with_sccp_commitment_root(Some([0xBB; 32]))
        .sign(leader.private_key())
        .unpack(|_| {})
        .into();
    block
        .set_transaction_results(Vec::new(), &[], Vec::new())
        .expect("empty block result fixture should be valid");
    let err = ValidBlock::validate_sccp_commitment_root(&block)
        .expect_err("root without SCCP messages should reject");
    assert!(matches!(
        err,
        BlockValidationError::SccpCommitmentRootMismatch {
            expected: None,
            actual: Some(_),
        }
    ));
}
#[test]
fn sccp_commitment_root_validation_rejects_missing_root_after_successful_result() {
    let mut block = signed_sccp_block(None);
    set_single_sccp_transaction_result(
        &mut block,
        Ok(iroha_data_model::transaction::DataTriggerSequence::default()),
    );
    let err = ValidBlock::validate_sccp_commitment_root(&block)
        .expect_err("successful SCCP result must be signed with the matching root");
    assert!(matches!(
        err,
        BlockValidationError::SccpCommitmentRootMismatch {
            expected: Some(_),
            actual: None,
        }
    ));
}
#[test]
fn sccp_commitment_root_change_invalidates_block_signature() {
    let leader = crate::block::checked_keypair();
    let mut block: SignedBlock = BlockBuilder::new(vec![sccp_accepted_transaction()])
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {})
        .into();
    let signed_hash = block.hash();
    block
        .signatures()
        .next()
        .expect("signed SCCP test block should have a leader signature")
        .signature()
        .verify_hash(leader.public_key(), signed_hash)
        .expect("leader signature should verify before SCCP root change");
    set_single_sccp_transaction_result(
        &mut block,
        Ok(iroha_data_model::transaction::DataTriggerSequence::default()),
    );
    let messages = crate::bridge::collect_sccp_messages_from_signed_block(&block);
    let root = crate::bridge::sccp_commitment_root_from_messages(&messages)
        .expect("successful SCCP result should have a commitment root");
    block.set_sccp_commitment_root(Some(root));
    assert!(block.header().sccp_commitment_root().is_some());
    assert_ne!(
        signed_hash,
        block.hash(),
        "SCCP root changes must change the signed block hash"
    );
    assert!(
        block
            .signatures()
            .next()
            .expect("signed SCCP test block should retain its leader signature")
            .signature()
            .verify_hash(leader.public_key(), block.hash())
            .is_err(),
        "leader signature must not verify after the SCCP root changes"
    );
}
#[test]
fn sccp_commitment_root_validation_rejects_root_after_failed_result() {
    let mut block = signed_sccp_block(Some([0xAA; 32]));
    set_single_sccp_transaction_result(
        &mut block,
        Err(
            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::NotPermitted(
                    "failed SCCP transaction fixture".to_owned(),
                ),
            ),
        ),
    );
    let err = ValidBlock::validate_sccp_commitment_root(&block)
        .expect_err("failed SCCP result must not keep a signed commitment root");
    assert!(matches!(
        err,
        BlockValidationError::SccpCommitmentRootMismatch {
            expected: None,
            actual: Some(_),
        }
    ));
}
#[test]
fn sccp_commitment_root_validation_rejects_deduped_root_for_successful_duplicates() {
    let (account_id, keypair) = gen_account_in("sccp");
    let accepted = sccp_accepted_transaction_with_record_count(account_id, &keypair, 2);
    let candidate_messages =
        crate::bridge::collect_sccp_messages_from_accepted_transactions(&[accepted.clone()]);
    assert_eq!(
        candidate_messages.len(),
        1,
        "proposal SCCP collection deduplicates outbound replay keys"
    );
    let deduplicated_root = crate::bridge::sccp_commitment_root_from_messages(&candidate_messages)
        .expect("deduplicated candidate root");
    let leader = crate::block::checked_keypair();
    let mut block: SignedBlock = BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .with_sccp_commitment_root(Some(deduplicated_root))
        .sign(leader.private_key())
        .unpack(|_| {})
        .into();
    set_single_sccp_transaction_result(
        &mut block,
        Ok(iroha_data_model::transaction::DataTriggerSequence::default()),
    );
    let err = ValidBlock::validate_sccp_commitment_root(&block)
        .expect_err("successful duplicate SCCP records must not validate with a deduplicated root");
    assert!(matches!(
        err,
        BlockValidationError::SccpDuplicateOutboundMessage { .. }
    ));
}
#[test]
fn sccp_commitment_root_validation_rejects_duplicate_inclusive_root() {
    let (account_id, keypair) = gen_account_in("sccp");
    let accepted = sccp_accepted_transaction_with_record_count(account_id, &keypair, 2);
    let leader = crate::block::checked_keypair();
    let duplicate_commitment = crate::bridge::test_sccp_hub_commitment(&sccp_transfer_payload());
    let duplicate_inclusive_root =
        iroha_sccp::commitment_merkle_root(&[duplicate_commitment.clone(), duplicate_commitment])
            .expect("duplicate-inclusive candidate root");
    let mut block: SignedBlock = BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .with_sccp_commitment_root(Some(duplicate_inclusive_root))
        .sign(leader.private_key())
        .unpack(|_| {})
        .into();
    set_single_sccp_transaction_result(
        &mut block,
        Ok(iroha_data_model::transaction::DataTriggerSequence::default()),
    );
    let err = ValidBlock::validate_sccp_commitment_root(&block).expect_err(
        "successful duplicate SCCP records must not validate with a duplicate-inclusive root",
    );
    assert!(matches!(
        err,
        BlockValidationError::SccpDuplicateOutboundMessage { .. }
    ));
}
#[test]
fn validate_and_record_transactions_rejects_sccp_root_after_rejected_record_tx() {
    let (account_id, keypair) = gen_account_in("sccp");
    let state = sccp_state_with_account(&account_id);
    let accepted = sccp_accepted_transaction_with_record_count(account_id, &keypair, 1);
    let candidate_messages =
        crate::bridge::collect_sccp_messages_from_accepted_transactions(&[accepted.clone()]);
    let candidate_root = crate::bridge::sccp_commitment_root_from_messages(&candidate_messages)
        .expect("candidate SCCP root");
    let key = crate::bridge::test_sccp_outbound_message_key(&sccp_transfer_payload());
    let leader = crate::block::checked_keypair();
    let new_block = BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .with_sccp_commitment_root(Some(candidate_root))
        .sign(leader.private_key())
        .unpack(|_| {});
    assert_eq!(
        new_block.header().sccp_commitment_root(),
        Some(candidate_root)
    );
    let mut state_block = state.block(new_block.header());
    let mut signed_block: SignedBlock = new_block.into();
    let err = ValidBlock::validate_and_record_transactions(
        &mut signed_block,
        &mut state_block,
        None,
        false,
    )
    .expect_err("failed SCCP record must reject the signed root instead of rewriting it");
    assert!(
        signed_block.error(0).is_some(),
        "invalid SCCP proved record should reject the transaction"
    );
    assert_eq!(
        signed_block.header().sccp_commitment_root(),
        Some(candidate_root)
    );
    assert!(
        state_block
            .world
            .sccp_outbound_pending_messages
            .get(&key)
            .is_none()
    );
    assert!(matches!(
        err,
        BlockValidationError::SccpCommitmentRootMismatch {
            expected: None,
            actual: Some(_),
        }
    ));
}
#[test]
fn sccp_commitment_root_after_execution_omits_rejected_record_tx() {
    let (account_id, keypair) = gen_account_in("sccp");
    let state = sccp_state_with_account(&account_id);
    let accepted = sccp_accepted_transaction_with_record_count(account_id, &keypair, 1);
    let candidate_messages =
        crate::bridge::collect_sccp_messages_from_accepted_transactions(&[accepted.clone()]);
    assert!(
        crate::bridge::sccp_commitment_root_from_messages(&candidate_messages).is_some(),
        "the pre-execution candidate includes the SCCP record"
    );
    let key = crate::bridge::test_sccp_outbound_message_key(&sccp_transfer_payload());
    let leader = crate::block::checked_keypair();
    let new_block = BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(new_block.header());
    let signed_block: SignedBlock = new_block.into();
    let root = ValidBlock::sccp_commitment_root_after_execution(signed_block, &mut state_block)
        .expect("failed SCCP record should still derive a post-execution root");
    assert_eq!(
        root, None,
        "rejected SCCP records must be omitted from the signed root"
    );
    assert!(
        state_block
            .world
            .sccp_outbound_pending_messages
            .get(&key)
            .is_none(),
        "rejected SCCP records must not persist outbound messages"
    );
}
#[test]
fn validate_and_record_transactions_rejects_sccp_root_after_duplicate_overlay_records() {
    let (account_id, keypair) = gen_account_in("sccp");
    let state = sccp_state_with_account(&account_id);
    let accepted = sccp_accepted_transaction_with_record_count(account_id, &keypair, 2);
    let candidate_messages =
        crate::bridge::collect_sccp_messages_from_accepted_transactions(&[accepted.clone()]);
    assert_eq!(
        candidate_messages.len(),
        1,
        "pre-execution SCCP root collection deduplicates outbound keys"
    );
    let candidate_root = crate::bridge::sccp_commitment_root_from_messages(&candidate_messages)
        .expect("candidate SCCP root");
    let key = crate::bridge::test_sccp_outbound_message_key(&sccp_transfer_payload());
    let leader = crate::block::checked_keypair();
    let new_block = BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .with_sccp_commitment_root(Some(candidate_root))
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(new_block.header());
    let mut signed_block: SignedBlock = new_block.into();
    let err = ValidBlock::validate_and_record_transactions(
        &mut signed_block,
        &mut state_block,
        None,
        false,
    )
    .expect_err("duplicate SCCP overlay records must reject the transaction and signed root");
    assert!(
        signed_block.error(0).is_some(),
        "duplicate SCCP proved records should reject the transaction"
    );
    assert_eq!(
        signed_block.header().sccp_commitment_root(),
        Some(candidate_root)
    );
    assert!(
        state_block
            .world
            .sccp_outbound_pending_messages
            .get(&key)
            .is_none()
    );
    assert!(matches!(
        err,
        BlockValidationError::SccpCommitmentRootMismatch {
            expected: None,
            actual: Some(_),
        }
    ));
}
#[test]
fn validate_and_record_transactions_executes_soracloud_mailbox_runtime_once() {
    let mut world = World::new();
    let (service_name, message_id) = seed_soracloud_mailbox_fixture(&mut world, Vec::new());
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);
    let runtime = CountingSoracloudRuntime::default();
    state.set_soracloud_runtime(Some(Arc::new(runtime.clone())));
    let leader = crate::block::checked_keypair();
    let block = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(block.header);
    let _valid = block.validate_and_record_transactions(&mut state_block);
    state_block.commit().expect("commit first mailbox block");
    {
        let view = state.view();
        let world = view.world();
        let runtime_state = world
            .soracloud_service_runtime()
            .get(&service_name)
            .expect("runtime state after execution");
        let receipt = world
            .soracloud_runtime_receipts()
            .iter()
            .next()
            .map(|(_receipt_id, receipt)| receipt.clone())
            .expect("runtime receipt recorded");
        assert_eq!(runtime.ordered_mailbox_call_count(), 1);
        assert_eq!(runtime_state.pending_mailbox_message_count, 0);
        assert_eq!(runtime_state.last_receipt_id, Some(receipt.receipt_id));
        assert_eq!(receipt.mailbox_message_id, Some(message_id));
    }
    let follow_up_header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut follow_up_state_block = state.block(follow_up_header);
    let follow_up_transaction = follow_up_state_block.transaction();
    assert!(
        collect_ready_soracloud_mailbox_messages(&follow_up_transaction).is_empty(),
        "mailbox receipts must suppress re-delivery on later blocks"
    );
    let view = state.view();
    let world = view.world();
    assert_eq!(runtime.ordered_mailbox_call_count(), 1);
    assert_eq!(world.soracloud_runtime_receipts().iter().count(), 1);
}

#[test]
fn validate_and_record_transactions_persists_soracloud_mailbox_state_mutations() {
    let mut world = World::new();
    let binding_name: iroha_data_model::name::Name = "vault".parse().expect("valid binding name");
    let state_key = "/state/private/patient-1".to_string();
    let payload = b"portal-runtime-state-payload".to_vec();
    let payload_commitment = Hash::new(&payload);
    let (service_name, message_id) = seed_soracloud_mailbox_fixture(
        &mut world,
        vec![SoraStateBindingV1 {
            schema_version: SORA_STATE_BINDING_VERSION_V1,
            binding_name: binding_name.clone(),
            scope: iroha_data_model::soracloud::SoraStateScopeV1::ServiceState,
            mutability: SoraStateMutabilityV1::ReadWrite,
            encryption: SoraStateEncryptionV1::Plaintext,
            key_prefix: "/state/private".to_string(),
            max_item_bytes: NonZeroU64::new(512).expect("nonzero item bytes"),
            max_total_bytes: NonZeroU64::new(2_048).expect("nonzero total bytes"),
        }],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);
    let runtime =
        CountingSoracloudRuntime::with_state_mutations(vec![SoracloudDeterministicStateMutation {
            binding_name: binding_name.to_string(),
            state_key: state_key.clone(),
            operation: SoraStateMutationOperationV1::Upsert,
            encryption: SoraStateEncryptionV1::Plaintext,
            payload_bytes: Some(u64::try_from(payload.len()).expect("payload length")),
            payload: Some(payload),
            payload_commitment: Some(payload_commitment),
        }]);
    state.set_soracloud_runtime(Some(Arc::new(runtime.clone())));
    let leader = crate::block::checked_keypair();
    let block = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(block.header);
    let _valid = block.validate_and_record_transactions(&mut state_block);
    state_block.commit().expect("commit mailbox state block");
    let receipt = {
        let view = state.view();
        let world = view.world();
        let runtime_state = world
            .soracloud_service_runtime()
            .get(&service_name)
            .expect("runtime state after state mutation execution");
        let receipt = world
            .soracloud_runtime_receipts()
            .iter()
            .next()
            .map(|(_receipt_id, receipt)| receipt.clone())
            .expect("runtime receipt recorded");
        let entry = world
            .soracloud_service_state_entries()
            .get(&(
                service_name.as_ref().to_owned(),
                binding_name.as_ref().to_owned(),
                state_key.clone(),
            ))
            .expect("mailbox-driven service state entry");
        assert_eq!(runtime.ordered_mailbox_call_count(), 1);
        assert_eq!(runtime_state.pending_mailbox_message_count, 0);
        assert_eq!(runtime_state.last_receipt_id, Some(receipt.receipt_id));
        assert_eq!(receipt.mailbox_message_id, Some(message_id));
        assert_eq!(entry.encryption, SoraStateEncryptionV1::Plaintext);
        assert_eq!(entry.payload_bytes.get(), 28);
        assert_eq!(entry.payload_commitment, payload_commitment);
        assert_eq!(entry.governance_tx_hash, receipt.receipt_id);
        assert_eq!(entry.last_update_sequence, receipt.emitted_sequence);
        assert_eq!(
            entry.source_action,
            SoraServiceLifecycleActionV1::StateMutation
        );
        receipt
    };
    let follow_up_header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut follow_up_state_block = state.block(follow_up_header);
    let follow_up_transaction = follow_up_state_block.transaction();
    assert_eq!(
        crate::smartcontracts::isi::soracloud::next_soracloud_audit_sequence(
            &follow_up_transaction
        ),
        receipt.emitted_sequence.saturating_add(1),
        "runtime receipts must advance the shared Soracloud execution sequence"
    );
    assert!(
        collect_ready_soracloud_mailbox_messages(&follow_up_transaction).is_empty(),
        "mailbox receipts must suppress re-delivery after state mutation write-back"
    );
}
