// Provider-ingest runtime binding and capacity regressions.

#[test]
fn provider_ingest_catalog_projects_independent_source_and_resolver_qualifications() {
    let mut config = default_runtime_config();
    configure_provider_ingest_runtime(&mut config);

    let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
        .expect("project provider-ingest provider bindings");
    let source = bindings
        .iter()
        .find(|binding| {
            binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource
        })
        .expect("source-pool binding");
    assert_eq!(
        source.handle(),
        "network://sorafs/provider-ingest/source-primary"
    );
    assert_eq!(source.revision(), Some(5));
    assert_eq!(source.policy_digest(), Some([0xB1; 32]));
    assert_eq!(
        source.provider_ingest_source_limits(),
        Some(ProviderIngestSourceLimitsV1 {
            operation_timeout_ms: 30_000,
            max_content_bytes: config.torii.sorafs_storage.max_capacity_bytes.0,
            max_source_providers: 1_024,
            max_concurrent_streams: u32::try_from(config.torii.sorafs_storage.max_parallel_fetches)
                .expect("configured parallel-fetch bound fits u32"),
        })
    );

    let resolver = bindings
        .iter()
        .find(|binding| {
            binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver
        })
        .expect("signer-resolver binding");
    assert_eq!(
        resolver.handle(),
        "hsm://sorafs/provider-ingest/resolver-primary"
    );
    assert_eq!(resolver.revision(), Some(6));
    assert_eq!(resolver.policy_digest(), Some([0xB2; 32]));
    assert_ne!(source.revision(), resolver.revision());
    assert_ne!(source.policy_digest(), resolver.policy_digest());

    let signer = bindings
        .iter()
        .find(|binding| {
            binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSigner
        })
        .expect("leaf completion-signer binding");
    assert_eq!(
        signer.handle(),
        "pkcs11://sorafs/provider-ingest/signer-primary"
    );
    assert_eq!(signer.revision(), Some(3));
    assert_eq!(signer.policy_digest(), Some([0xA2; 32]));
    assert_ne!(resolver.revision(), signer.revision());
    assert_ne!(resolver.policy_digest(), signer.policy_digest());
    assert_eq!(
        resolver.provider_ingest_signer_binding(),
        signer.provider_ingest_signer_binding(),
        "resolver and leaf roles pin one exact algorithm/key/policy binding"
    );
    let detailed = signer
        .provider_ingest_signer_binding()
        .expect("exact completion-signer binding");
    assert_eq!(detailed.qualification.adapter_revision, 3);
    assert_eq!(
        detailed.qualification.signer_policy.policy_digest,
        [0xA2; 32]
    );
    assert_eq!(
        signer.provider_ingest_max_signed_transaction_bytes(),
        Some(1024 * 1024)
    );
    let checkpoint = bindings
        .iter()
        .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore)
        .expect("checkpoint-store binding");
    assert_eq!(
        checkpoint.provider_ingest_checkpoint_max_bytes(),
        Some(160 * 1024 * 1024)
    );

    let mut excessive_streams = config;
    excessive_streams.torii.sorafs_storage.max_parallel_fetches =
        usize::try_from(MAX_PROVIDER_INGEST_SOURCE_STREAMS_V1).expect("stream ceiling fits usize")
            + 1;
    assert!(matches!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&excessive_streams),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource
        ))
    ));
}

#[test]
fn provider_ingest_catalog_accepts_enabled_default_outbox_capacity() {
    let mut config = default_runtime_config();
    configure_provider_ingest_runtime(&mut config);
    let ingest = config
        .torii
        .sorafs_storage
        .provider_ingest_runtime
        .as_mut()
        .expect("configured provider ingest");
    ingest.outbox = iroha_config::parameters::actual::SorafsProviderIngestOutbox {
        max_active_entries: provider_ingest_outbox_defaults::MAX_ACTIVE_ENTRIES,
        max_terminal_entries: provider_ingest_outbox_defaults::MAX_TERMINAL_ENTRIES,
        max_attempts: provider_ingest_outbox_defaults::MAX_ATTEMPTS,
        checkpoint_max_bytes: provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES,
        checkpoint_operation_timeout_ms:
            provider_ingest_outbox_defaults::CHECKPOINT_OPERATION_TIMEOUT_MS,
        source_lease_ttl_ms: provider_ingest_outbox_defaults::SOURCE_LEASE_TTL_MS,
        retry_base_delay_ms: provider_ingest_outbox_defaults::RETRY_BASE_DELAY_MS,
        retry_max_delay_ms: provider_ingest_outbox_defaults::RETRY_MAX_DELAY_MS,
        terminal_retention_blocks: provider_ingest_outbox_defaults::TERMINAL_RETENTION_BLOCKS,
        max_signed_transaction_bytes: provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES,
        max_status_page_size: provider_ingest_outbox_defaults::MAX_STATUS_PAGE_SIZE,
    };

    let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
        .expect("enabled default provider-ingest outbox must project");
    assert_eq!(
        bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore
            })
            .and_then(IrohaRuntimeProviderBindingV1::provider_ingest_checkpoint_max_bytes),
        Some(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES.0)
    );
    assert_eq!(
        bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSigner
            })
            .and_then(IrohaRuntimeProviderBindingV1::provider_ingest_max_signed_transaction_bytes),
        Some(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES.0)
    );
}

#[test]
fn provider_ingest_catalog_rejects_broker_incompatible_outbox_limits() {
    let mut exact = default_runtime_config();
    configure_provider_ingest_runtime(&mut exact);
    let exact_ingest = exact
        .torii
        .sorafs_storage
        .provider_ingest_runtime
        .as_mut()
        .expect("configured provider ingest");
    exact_ingest.max_source_jobs_per_tick = 1;
    let exact_outbox = &mut exact_ingest.outbox;
    exact_outbox.max_active_entries = 1;
    exact_outbox.max_terminal_entries = 1;
    exact_outbox.checkpoint_max_bytes =
        Bytes(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT);
    exact_outbox.max_signed_transaction_bytes =
        Bytes(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_LIMIT);
    let exact_bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&exact)
        .expect("stock broker ceilings must project");
    assert_eq!(
        exact_bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore
            })
            .and_then(IrohaRuntimeProviderBindingV1::provider_ingest_checkpoint_max_bytes),
        Some(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT)
    );
    assert_eq!(
        exact_bindings
            .iter()
            .find(|binding| {
                binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSigner
            })
            .and_then(IrohaRuntimeProviderBindingV1::provider_ingest_max_signed_transaction_bytes),
        Some(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_LIMIT)
    );

    let mut oversized_checkpoint = default_runtime_config();
    configure_provider_ingest_runtime(&mut oversized_checkpoint);
    oversized_checkpoint
        .torii
        .sorafs_storage
        .provider_ingest_runtime
        .as_mut()
        .expect("configured provider ingest")
        .outbox
        .checkpoint_max_bytes =
        Bytes(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT + 1);
    assert_eq!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&oversized_checkpoint),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore,
        ))
    );

    let mut oversized_transaction = default_runtime_config();
    configure_provider_ingest_runtime(&mut oversized_transaction);
    oversized_transaction
        .torii
        .sorafs_storage
        .provider_ingest_runtime
        .as_mut()
        .expect("configured provider ingest")
        .outbox
        .max_signed_transaction_bytes =
        Bytes(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_LIMIT + 1);
    assert_eq!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&oversized_transaction),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver,
        ))
    );

    let mut legacy_128_mib_transaction = default_runtime_config();
    configure_provider_ingest_runtime(&mut legacy_128_mib_transaction);
    let legacy_ingest = legacy_128_mib_transaction
        .torii
        .sorafs_storage
        .provider_ingest_runtime
        .as_mut()
        .expect("configured provider ingest");
    legacy_ingest.max_source_jobs_per_tick = 1;
    legacy_ingest.outbox.max_active_entries = 1;
    legacy_ingest.outbox.max_terminal_entries = 1;
    legacy_ingest.outbox.checkpoint_max_bytes =
        Bytes(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT);
    legacy_ingest.outbox.max_signed_transaction_bytes = Bytes(128 * 1024 * 1024);
    assert_eq!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&legacy_128_mib_transaction),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver,
        )),
        "the former 128 MiB transaction limit must not project under a 192 MiB checkpoint"
    );

    let mut below_envelope_reserve = default_runtime_config();
    configure_provider_ingest_runtime(&mut below_envelope_reserve);
    below_envelope_reserve
        .torii
        .sorafs_storage
        .provider_ingest_runtime
        .as_mut()
        .expect("configured provider ingest")
        .outbox
        .max_signed_transaction_bytes =
        Bytes(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_MIN - 1);
    assert_eq!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&below_envelope_reserve),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver,
        ))
    );

    let mut exact_minimum = default_runtime_config();
    configure_provider_ingest_runtime(&mut exact_minimum);
    exact_minimum
        .torii
        .sorafs_storage
        .provider_ingest_runtime
        .as_mut()
        .expect("configured provider ingest")
        .outbox
        .max_signed_transaction_bytes =
        Bytes(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_MIN);
    IrohaRuntimeProviderBindingsV1::try_from_config(&exact_minimum)
        .expect("exact signed-transaction minimum must project");

    let mut impossible_aggregate = default_runtime_config();
    configure_provider_ingest_runtime(&mut impossible_aggregate);
    impossible_aggregate
        .torii
        .sorafs_storage
        .provider_ingest_runtime
        .as_mut()
        .expect("configured provider ingest")
        .outbox
        .max_active_entries = 128;
    assert_eq!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&impossible_aggregate),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore,
        )),
        "an aggregate outbox capacity above its checkpoint bound must fail projection"
    );
}
