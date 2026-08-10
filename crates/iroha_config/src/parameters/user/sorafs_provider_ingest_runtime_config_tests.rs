//! Focused parser tests for the SoraFS provider-ingest runtime policy.

use super::*;

fn provider_id() -> ProviderId {
    ProviderId::new([0x51; 32])
}

fn completion_signer_public_key_hex() -> String {
    let key =
        KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519).expect("completion signer key");
    hex::encode(key.public_key().to_bytes().1)
}

fn valid_config() -> SorafsProviderIngestRuntimeConfig {
    SorafsProviderIngestRuntimeConfig {
        enabled: true,
        authenticated_source_fetch_handle: Some("https-pinned-source-pool:eu-1".to_owned()),
        authenticated_source_fetch_revision: Some(5),
        authenticated_source_fetch_policy_digest_hex: Some("b1".repeat(32)),
        completion_signer_resolver_handle: Some(
            "resolver://sorafs/provider-ingest/primary".to_owned(),
        ),
        completion_signer_resolver_revision: Some(6),
        completion_signer_resolver_policy_digest_hex: Some("b2".repeat(32)),
        completion_signer_handle: Some(
            "software://sorafs/provider-ingest/signer-primary".to_owned(),
        ),
        completion_signer_adapter_revision: Some(3),
        completion_signer_policy_id_hex: Some("a1".repeat(32)),
        completion_signer_policy_revision: Some(1),
        completion_signer_policy_predecessor_digest_hex: None,
        completion_signer_policy_digest_hex: Some("a2".repeat(32)),
        completion_signer_algorithm: Some(Algorithm::Ed25519),
        completion_signer_public_key_hex: Some(completion_signer_public_key_hex()),
        checkpoint_store_handle: Some(
            "sealed://sorafs/provider-ingest/checkpoint-primary".to_owned(),
        ),
        checkpoint_store_revision: Some(7),
        checkpoint_store_policy_digest_hex: Some("a7".repeat(32)),
        ..SorafsProviderIngestRuntimeConfig::default()
    }
}

fn large_valid_outbox_config() -> SorafsProviderIngestRuntimeConfig {
    use defaults::sorafs::storage::provider_ingest_runtime::outbox;

    let mut config = valid_config();
    config.max_source_jobs_per_tick = 1;
    config.outbox.max_active_entries = 1;
    config.outbox.max_terminal_entries = 1;
    config.outbox.checkpoint_max_bytes = Bytes(outbox::CHECKPOINT_MAX_BYTES_LIMIT);
    config.outbox.max_signed_transaction_bytes = Bytes(outbox::MAX_SIGNED_TRANSACTION_BYTES_LIMIT);
    config
}

#[test]
fn disabled_default_is_inert() {
    let mut emitter = Emitter::new();
    assert!(
        SorafsProviderIngestRuntimeConfig::default()
            .parse(false, None, &mut emitter)
            .is_none()
    );
    assert!(emitter.into_result().is_ok());
}

#[test]
fn enabled_policy_parses_without_credentials() {
    let provider_id = provider_id();
    let mut emitter = Emitter::new();
    let parsed = valid_config()
        .parse(true, Some(&provider_id), &mut emitter)
        .expect("enabled provider-ingest policy");
    assert!(emitter.into_result().is_ok());
    assert_eq!(parsed.scan_interval_ms, 1_000);
    assert_eq!(parsed.max_page_rows, 64);
    assert_eq!(parsed.max_pages_per_tick, 4);
    assert_eq!(parsed.max_source_providers, 1_024);
    assert_eq!(parsed.authenticated_source_fetch_revision, 5);
    assert_eq!(parsed.authenticated_source_fetch_policy_digest, [0xB1; 32]);
    assert_eq!(parsed.completion_signer_resolver_revision, 6);
    assert_eq!(parsed.completion_signer_resolver_policy_digest, [0xB2; 32]);
    assert_eq!(
        parsed.finalized_archive.relative_root,
        PathBuf::from("provider-ingest-finalized-archive-v1")
    );
    assert_eq!(parsed.finalized_archive.max_record_bytes, 128 * 1024 * 1024);
    assert_eq!(parsed.finalized_archive.max_archive_entries, 1_000_000);
    assert_eq!(
        parsed.finalized_archive.max_total_bytes,
        64 * 1024 * 1024 * 1024
    );
    assert_eq!(parsed.finalized_archive.max_providers_per_anchor, 1_024);
    assert_eq!(parsed.finalized_archive.max_orders_per_provider, 256);
    assert_eq!(parsed.finalized_archive.max_total_orders_per_anchor, 256);
    assert_eq!(parsed.finalized_archive.max_page_rows, 64);
    assert_eq!(parsed.finalized_archive.max_kura_tip_lag_blocks, 2);
    assert_eq!(parsed.completion_signer_adapter_revision, 3);
    assert_eq!(
        parsed.completion_signer_policy,
        ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [0xA1; 32],
            revision: 1,
            predecessor_digest: None,
            policy_digest: [0xA2; 32],
        }
    );
    assert_eq!(parsed.completion_signer_algorithm, Algorithm::Ed25519);
    assert_eq!(
        hex::encode(parsed.completion_signer_public_key.to_bytes().1),
        completion_signer_public_key_hex()
    );
    assert_eq!(parsed.checkpoint_store_revision, 7);
    assert_eq!(parsed.checkpoint_store_policy_digest, [0xA7; 32]);
    assert_eq!(parsed.outbox.max_status_page_size, 256);
    assert_eq!(parsed.outbox.max_active_entries, 128);
    assert_eq!(parsed.outbox.checkpoint_max_bytes.0, 160 * 1024 * 1024);
    assert_eq!(parsed.outbox.checkpoint_operation_timeout_ms, 30_000);
}

#[test]
fn defaults_and_actual_projection_respect_provider_broker_limits() {
    use defaults::sorafs::storage::provider_ingest_runtime::outbox;

    assert_eq!(outbox::CHECKPOINT_MAX_BYTES_LIMIT, 192 * 1024 * 1024);
    assert_eq!(outbox::MAX_SIGNED_TRANSACTION_BYTES_LIMIT, 64 * 1024 * 1024);
    assert_eq!(outbox::MAX_SIGNED_TRANSACTION_BYTES_MIN, 64 * 1024);
    assert!(
        outbox::MAX_SIGNED_TRANSACTION_BYTES_MIN
            > outbox::SIGNED_TRANSACTION_ENVELOPE_RESERVE_BYTES_V1
    );
    assert!(outbox::CHECKPOINT_MAX_BYTES.0 <= outbox::CHECKPOINT_MAX_BYTES_LIMIT);
    assert!(outbox::MAX_SIGNED_TRANSACTION_BYTES.0 >= outbox::MAX_SIGNED_TRANSACTION_BYTES_MIN);
    assert!(outbox::MAX_SIGNED_TRANSACTION_BYTES.0 <= outbox::MAX_SIGNED_TRANSACTION_BYTES_LIMIT);

    let mut emitter = Emitter::new();
    let parsed = large_valid_outbox_config()
        .parse(true, Some(&provider_id()), &mut emitter)
        .expect("large but checkpoint-compatible policy must project");
    assert!(emitter.into_result().is_ok());
    assert_eq!(
        parsed.outbox.checkpoint_max_bytes.0,
        outbox::CHECKPOINT_MAX_BYTES_LIMIT
    );
    assert_eq!(
        parsed.outbox.max_signed_transaction_bytes.0,
        outbox::MAX_SIGNED_TRANSACTION_BYTES_LIMIT
    );
}

#[test]
fn broker_incompatible_outbox_limits_fail_closed() {
    use defaults::sorafs::storage::provider_ingest_runtime::outbox;

    let mut oversized_checkpoint = large_valid_outbox_config();
    oversized_checkpoint.outbox.checkpoint_max_bytes =
        Bytes(outbox::CHECKPOINT_MAX_BYTES_LIMIT + 1);
    let mut emitter = Emitter::new();
    let _projected = oversized_checkpoint.parse(true, Some(&provider_id()), &mut emitter);
    assert!(
        emitter.into_result().is_err(),
        "checkpoint bytes above the stock broker ceiling must fail"
    );

    let mut oversized_transaction = large_valid_outbox_config();
    oversized_transaction.outbox.max_signed_transaction_bytes =
        Bytes(outbox::MAX_SIGNED_TRANSACTION_BYTES_LIMIT + 1);
    let mut emitter = Emitter::new();
    let _projected = oversized_transaction.parse(true, Some(&provider_id()), &mut emitter);
    assert!(
        emitter.into_result().is_err(),
        "signed transaction bytes above the stock broker ceiling must fail"
    );

    let mut below_envelope_reserve = large_valid_outbox_config();
    below_envelope_reserve.outbox.max_signed_transaction_bytes =
        Bytes(outbox::MAX_SIGNED_TRANSACTION_BYTES_MIN - 1);
    let mut emitter = Emitter::new();
    let _projected = below_envelope_reserve.parse(true, Some(&provider_id()), &mut emitter);
    assert!(
        emitter.into_result().is_err(),
        "a signed transaction limit without room beyond the envelope reserve must fail"
    );

    let mut exact_minimum = large_valid_outbox_config();
    exact_minimum.outbox.max_active_entries = 1;
    exact_minimum.outbox.max_terminal_entries = 1;
    exact_minimum.outbox.max_signed_transaction_bytes =
        Bytes(outbox::MAX_SIGNED_TRANSACTION_BYTES_MIN);
    let mut emitter = Emitter::new();
    let projected = exact_minimum
        .parse(true, Some(&provider_id()), &mut emitter)
        .expect("exact signed-transaction minimum must project");
    assert!(emitter.into_result().is_ok());
    assert_eq!(
        projected.outbox.max_signed_transaction_bytes.0,
        outbox::MAX_SIGNED_TRANSACTION_BYTES_MIN
    );

    let mut legacy_128_mib_ceiling = large_valid_outbox_config();
    legacy_128_mib_ceiling.outbox.max_signed_transaction_bytes = Bytes(128 * 1024 * 1024);
    let mut emitter = Emitter::new();
    let _projected = legacy_128_mib_ceiling.parse(true, Some(&provider_id()), &mut emitter);
    assert!(
        emitter.into_result().is_err(),
        "one retained expected payload plus one signed transaction at the 128 MiB ceiling cannot fit the 192 MiB checkpoint ceiling"
    );
}

#[test]
fn enabled_policy_requires_storage_provider_and_production_handles() {
    let mut config = valid_config();
    config.authenticated_source_fetch_handle = Some("mock-source.test".to_owned());
    config.authenticated_source_fetch_revision = Some(0);
    config.authenticated_source_fetch_policy_digest_hex = Some("00".repeat(32));
    config.completion_signer_resolver_handle = None;
    config.completion_signer_resolver_revision = None;
    config.completion_signer_resolver_policy_digest_hex = Some("B2".repeat(32));
    config.checkpoint_store_handle = Some("sealed.sorafs.test".to_owned());
    config.checkpoint_store_revision = Some(0);
    config.checkpoint_store_policy_digest_hex = Some("A7".repeat(32));

    let mut emitter = Emitter::new();
    assert!(config.parse(false, None, &mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn enabled_policy_rejects_credential_or_uri_parameter_handles() {
    for rejected in [
        "https://operator:secret@host",
        "https://host/source?token=secret",
        "https://host/source#fragment",
        "https://host/%73ource",
        "resolver://sorafs/provider-ingest/dummy",
    ] {
        let mut config = valid_config();
        config.authenticated_source_fetch_handle = Some(rejected.to_owned());
        let mut emitter = Emitter::new();
        assert!(
            config
                .parse(true, Some(&provider_id()), &mut emitter)
                .is_none(),
            "{rejected:?} must not reach the actual configuration"
        );
        assert!(emitter.into_result().is_err());
    }
}

#[test]
fn enabled_policy_rejects_stale_or_noncanonical_completion_signer_binding() {
    let mut config = valid_config();
    config.completion_signer_handle = Some("software://sorafs/provider-ingest/test".to_owned());
    config.completion_signer_adapter_revision = Some(0);
    config.completion_signer_policy_id_hex = Some("00".repeat(32));
    config.completion_signer_policy_revision = Some(2);
    config.completion_signer_policy_predecessor_digest_hex = None;
    config.completion_signer_policy_digest_hex = Some("A2".repeat(32));
    config.completion_signer_algorithm = Some(Algorithm::Secp256k1);
    config.completion_signer_public_key_hex =
        Some(completion_signer_public_key_hex().to_uppercase());

    let mut emitter = Emitter::new();
    assert!(
        config
            .parse(true, Some(&provider_id()), &mut emitter)
            .is_none()
    );
    assert!(emitter.into_result().is_err());
}

#[test]
fn enabled_policy_rejects_zero_provider_identity() {
    let zero_provider_id = ProviderId::new([0; 32]);
    let mut emitter = Emitter::new();
    assert!(
        valid_config()
            .parse(true, Some(&zero_provider_id), &mut emitter)
            .is_some()
    );
    assert!(emitter.into_result().is_err());
}

#[test]
fn unsafe_resource_timing_and_capacity_bounds_fail_closed() {
    use defaults::sorafs::storage::provider_ingest_runtime::outbox;

    let provider_id = provider_id();
    let mut config = valid_config();
    config.scan_interval_ms = 0;
    config.max_page_rows = 1_001;
    config.max_pages_per_tick = 4_097;
    config.max_source_jobs_per_tick = 4_097;
    config.max_source_providers = 1_025;
    config.source_operation_timeout_ms = 24 * 60 * 60 * 1_000 + 1;
    config.source_lease_renew_interval_ms = 60_000;
    config.signer_timeout_ms = 0;
    config.ingress_timeout_ms = 0;
    config.completion_transaction_ttl_ms = 0;
    config.finalized_archive.relative_root = PathBuf::from("../archive");
    config.finalized_archive.max_record_bytes = Bytes(1024 * 1024 * 1024 + 1);
    config.finalized_archive.max_archive_entries = 1_000_001;
    config.finalized_archive.max_total_bytes = Bytes(1024 * 1024 * 1024 * 1024 + 1);
    config.finalized_archive.max_providers_per_anchor = 1_025;
    config.finalized_archive.max_orders_per_provider = 65_537;
    config.finalized_archive.max_total_orders_per_anchor = 65_537;
    config.finalized_archive.max_page_rows = 1_001;
    config.finalized_archive.max_kura_tip_lag_blocks = 10_001;
    config.outbox.max_active_entries = 0;
    config.outbox.max_terminal_entries = 65_537;
    config.outbox.max_attempts = 65;
    config.outbox.checkpoint_max_bytes = Bytes(outbox::CHECKPOINT_MAX_BYTES_LIMIT + 1);
    config.outbox.checkpoint_operation_timeout_ms = 24 * 60 * 60 * 1_000 + 1;
    config.outbox.source_lease_ttl_ms = 60_000;
    config.outbox.retry_base_delay_ms = 2_000;
    config.outbox.retry_max_delay_ms = 1_000;
    config.outbox.terminal_retention_blocks = 10_000_001;
    config.outbox.max_signed_transaction_bytes =
        Bytes(outbox::MAX_SIGNED_TRANSACTION_BYTES_LIMIT + 1);
    config.outbox.max_status_page_size = 1_001;

    let mut emitter = Emitter::new();
    assert!(
        config
            .parse(true, Some(&provider_id), &mut emitter)
            .is_none()
    );
    assert!(emitter.into_result().is_err());
}

#[test]
fn checkpoint_operation_deadline_must_be_nonzero_and_bounded() {
    for invalid_timeout_ms in [0, 24 * 60 * 60 * 1_000 + 1] {
        let mut config = valid_config();
        config.outbox.checkpoint_operation_timeout_ms = invalid_timeout_ms;
        let mut emitter = Emitter::new();
        let _actual = config.parse(true, Some(&provider_id()), &mut emitter);
        assert!(
            emitter.into_result().is_err(),
            "{invalid_timeout_ms}ms must not reach a runnable configuration"
        );
    }
}

#[test]
fn checked_aggregate_capacities_fail_closed() {
    let provider_id = provider_id();
    let mut config = valid_config();
    config.max_page_rows = 1_000;
    config.max_pages_per_tick = 4_096;
    config.finalized_archive.max_page_rows = 1_000;
    config.finalized_archive.max_record_bytes = Bytes(1);
    config.max_source_jobs_per_tick = 16;
    config.outbox.max_active_entries = 1;
    config.outbox.max_terminal_entries = 1;
    config.outbox.checkpoint_max_bytes = Bytes(1);
    config.outbox.max_signed_transaction_bytes = Bytes(1);

    let mut emitter = Emitter::new();
    assert!(
        config
            .parse(true, Some(&provider_id), &mut emitter)
            .is_none()
    );
    assert!(emitter.into_result().is_err());
}

#[test]
fn archive_policy_allows_zero_lag_but_rejects_absolute_or_dot_roots() {
    let provider_id = provider_id();
    let mut zero_lag = valid_config();
    zero_lag.finalized_archive.max_kura_tip_lag_blocks = 0;
    let mut emitter = Emitter::new();
    let parsed = zero_lag
        .parse(true, Some(&provider_id), &mut emitter)
        .expect("zero-lag archive policy");
    assert_eq!(parsed.finalized_archive.max_kura_tip_lag_blocks, 0);
    assert!(emitter.into_result().is_ok());

    for relative_root in [
        PathBuf::from("."),
        PathBuf::from("archive/../substituted"),
        std::env::current_dir()
            .expect("current directory")
            .join("absolute-archive"),
    ] {
        let mut config = valid_config();
        config.finalized_archive.relative_root = relative_root;
        let mut emitter = Emitter::new();
        assert!(
            config
                .parse(true, Some(&provider_id), &mut emitter)
                .is_none()
        );
        assert!(emitter.into_result().is_err());
    }
}

#[test]
fn worst_case_outbox_transactions_must_fit_checkpoint() {
    let provider_id = provider_id();
    let mut config = valid_config();
    config.outbox.max_active_entries = 4_096;

    let mut emitter = Emitter::new();
    assert!(
        config
            .parse(true, Some(&provider_id), &mut emitter)
            .is_some()
    );
    assert!(emitter.into_result().is_err());
}

#[test]
fn disabled_policy_rejects_stale_runtime_bindings() {
    let mut config = SorafsProviderIngestRuntimeConfig::default();
    config.authenticated_source_fetch_handle =
        Some("network.sorafs.authenticated-source.primary".to_owned());
    config.completion_signer_resolver_policy_digest_hex = Some("b2".repeat(32));
    config.checkpoint_store_revision = Some(1);
    let mut emitter = Emitter::new();
    assert!(config.parse(false, None, &mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn disabled_policy_rejects_each_top_level_provider_qualification_field() {
    let mutations: [fn(&mut SorafsProviderIngestRuntimeConfig); 4] = [
        |config| config.authenticated_source_fetch_revision = Some(5),
        |config| {
            config.authenticated_source_fetch_policy_digest_hex = Some("b1".repeat(32));
        },
        |config| config.completion_signer_resolver_revision = Some(6),
        |config| {
            config.completion_signer_resolver_policy_digest_hex = Some("b2".repeat(32));
        },
    ];
    for mutate in mutations {
        let mut config = SorafsProviderIngestRuntimeConfig::default();
        mutate(&mut config);
        let mut emitter = Emitter::new();
        assert!(config.parse(false, None, &mut emitter).is_none());
        assert!(emitter.into_result().is_err());
    }
}
