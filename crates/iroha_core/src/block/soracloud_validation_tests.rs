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
                // Ordered-mailbox receipts are submitted with an unassigned sequence; the
                // ledger assigns the authoritative Soracloud sequence atomically on apply.
                emitted_sequence: 0,
                mailbox_message_id: Some(request.mailbox_message.message_id),
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                execution_host: None,
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
) -> (iroha_model_base::name::Name, Hash) {
    let service_name: iroha_model_base::name::Name = "portal".parse().expect("valid service name");
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
                allow_state_writes: false,
                allow_model_inference: false,
                allow_model_training: false,
            },
            resources: SoraResourceLimitsV1 {
                cpu_millis: NonZeroU32::new(500).expect("nonzero cpu"),
                memory_bytes: NonZeroU64::new(16 * 1024 * 1024).expect("nonzero memory"),
                ephemeral_storage_bytes: NonZeroU64::new(16 * 1024 * 1024)
                    .expect("nonzero storage"),
                max_open_files_per_process: NonZeroU32::new(256).expect("nonzero files"),
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
            placement_targets: BTreeSet::new(),
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
            active_service_version: service_version.clone(),
            health_status: SoraServiceHealthStatusV1::Healthy,
            load_factor_bps: 77,
            materialized_bundle_hash: bundle_hash,
        },
    );
    let mut message = SoraServiceMailboxMessageV1 {
        schema_version: iroha_data_model::soracloud::SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
        message_id: Hash::prehashed([0; Hash::LENGTH]),
        from_service: service_name.clone(),
        from_service_version: service_version.clone(),
        from_handler: "update".parse().expect("valid from handler"),
        to_service: service_name.clone(),
        to_service_version: service_version,
        to_handler: "update".parse().expect("valid to handler"),
        payload_bytes: b"portal-mailbox-payload".to_vec(),
        payload_commitment: Hash::new(b"portal-mailbox-payload"),
        delivery_delay_blocks: 0,
        enqueue_sequence: 1,
        enqueue_height: 1,
        available_after_height: 1,
        expires_at_height: 16,
    };
    message.message_id =
        iroha_data_model::soracloud::derive_soracloud_mailbox_message_id_v1(&message);
    let message_id = message.message_id;
    world
        .soracloud_mailbox_messages_mut_for_testing()
        .insert(message_id, message);
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
#[test]
fn validate_and_record_transactions_never_executes_local_soracloud_mailbox_runtime() {
    let mut world = World::new();
    let (service_name, message_id) = seed_soracloud_mailbox_fixture(&mut world, Vec::new());
    let mut chain = crate::sumeragi::test_chain::CertifiedTestChain::start(
        crate::sumeragi::test_chain::TestChainConfig::new(world, 1_000),
    )
    .expect("original native genesis for mailbox isolation");
    let runtime = CountingSoracloudRuntime::default();
    chain
        .state()
        .set_soracloud_runtime(Some(Arc::new(runtime.clone())));
    let audit_sequence_before = mailbox_audit_sequence(chain.state());
    chain.commit(Vec::new());
    assert_eq!(
        mailbox_audit_sequence(chain.state()),
        audit_sequence_before,
        "local runtime output cannot advance the consensus audit sequence"
    );
    let state = chain.state();
    let view = state.view();
    let world = view.world();
    let runtime_state = world
        .soracloud_service_runtime()
        .get(&service_name)
        .expect("seeded runtime state remains available");
    assert_eq!(runtime.ordered_mailbox_call_count(), 0);
    assert_eq!(runtime_state.load_factor_bps, 77);
    assert!(world.soracloud_runtime_receipts().is_empty());
    assert!(
        world
            .soracloud_mailbox_messages()
            .get(&message_id)
            .is_some()
    );
}

#[test]
fn validate_and_record_transactions_ignores_local_soracloud_mailbox_state_mutations() {
    let mut world = World::new();
    let binding_name: iroha_model_base::name::Name = "vault".parse().expect("valid binding name");
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
    let mut chain = crate::sumeragi::test_chain::CertifiedTestChain::start(
        crate::sumeragi::test_chain::TestChainConfig::new(world, 1_000),
    )
    .expect("original native genesis for mailbox isolation");
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
    chain
        .state()
        .set_soracloud_runtime(Some(Arc::new(runtime.clone())));
    let audit_sequence_before = mailbox_audit_sequence(chain.state());
    chain.commit(Vec::new());
    assert_eq!(
        mailbox_audit_sequence(chain.state()),
        audit_sequence_before,
        "local runtime output cannot advance the consensus audit sequence"
    );
    let state = chain.state();
    let view = state.view();
    let world = view.world();
    let runtime_state = world
        .soracloud_service_runtime()
        .get(&service_name)
        .expect("seeded runtime state remains available");
    assert_eq!(runtime.ordered_mailbox_call_count(), 0);
    assert_eq!(runtime_state.load_factor_bps, 77);
    assert!(world.soracloud_runtime_receipts().is_empty());
    assert!(
        world
            .soracloud_mailbox_messages()
            .get(&message_id)
            .is_some()
    );
    assert!(
        world
            .soracloud_service_state_entries()
            .get(&(
                service_name.as_ref().to_owned(),
                binding_name.as_ref().to_owned(),
                state_key,
            ))
            .is_none(),
        "local runtime mutations must never alter consensus state"
    );
}

fn mailbox_audit_sequence(state: &State) -> u64 {
    let header = state
        .view()
        .latest_block()
        .expect("original block read attempt")
        .unwrap()
        .header();
    let mut overlay = state.block(header);
    crate::smartcontracts::isi::soracloud::next_soracloud_audit_sequence(&overlay.transaction())
        .expect("fixture audit sequence")
}
