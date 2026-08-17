// Provider-ingest, reputation, governance, and native-signer registry tests.
//
// Included by `runtime_provider_registry::tests` to preserve exact libtest paths.
fn test_network_id(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(
        HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(Hash::prehashed(
            [seed; Hash::LENGTH],
        )),
    )
}
fn configure_governance_service(config: &mut Config) {
    let service = &mut config.torii.sorafs_storage.governance_dag_service;
    service.enabled = true;
    service.head_mode = "signed_http".to_owned();
    service.ipfs_api_url = Some(GOVERNANCE_IPFS_ENDPOINT.to_owned());
    service.signed_head_url = Some(GOVERNANCE_HEAD_ENDPOINT.to_owned());
    service.ipns_name = None;
    service.ipns_key_name = None;
    service.ipfs_authenticator_handle = Some(GOVERNANCE_IPFS_HANDLE.to_owned());
    service.ipfs_authenticator_revision = Some(GOVERNANCE_QUALIFICATION.revision);
    service.ipfs_authenticator_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);
    service.ipfs_request_auth_public_key = Some(governance_auth_public_key(GOVERNANCE_IPFS_HANDLE));
    service.checkpoint_store_handle = Some(GOVERNANCE_CHECKPOINT_HANDLE.to_owned());
    service.checkpoint_store_revision = Some(GOVERNANCE_QUALIFICATION.revision);
    service.checkpoint_store_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);
    service.head_authenticator_handle = Some(GOVERNANCE_HEAD_HANDLE.to_owned());
    service.head_authenticator_revision = Some(GOVERNANCE_QUALIFICATION.revision);
    service.head_authenticator_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);
    service.head_request_auth_public_key = Some(governance_auth_public_key(GOVERNANCE_HEAD_HANDLE));
}
fn configure_governance_ipns_service(config: &mut Config) {
    configure_governance_service(config);
    let service = &mut config.torii.sorafs_storage.governance_dag_service;
    service.head_mode = "ipns".to_owned();
    service.signed_head_url = None;
    service.ipns_name = Some("k51qzi5uqu5dtest".to_owned());
    service.ipns_key_name = Some("governance-head".to_owned());
    service.head_authenticator_handle = None;
    service.head_authenticator_revision = None;
    service.head_authenticator_policy_digest = None;
    service.head_request_auth_public_key = None;
}
fn governance_service_view(
    head_mode: &str,
) -> iroha_config::parameters::actual::SorafsGovernanceDagServiceView {
    let mut config = default_runtime_config();
    if head_mode == "ipns" {
        configure_governance_ipns_service(&mut config);
    } else {
        configure_governance_service(&mut config);
        config.torii.sorafs_storage.governance_dag_service.head_mode = head_mode.to_owned();
    }
    let service = config.torii.sorafs_storage.governance_dag_service;
    iroha_config::parameters::actual::SorafsGovernanceDagServiceView {
        source_dir: None,
        producer_publisher_peer_id: None,
        producer_signer_handle: None,
        producer_signer_revision: None,
        producer_signer_policy_digest: None,
        producer_publisher_public_key_hex: None,
        service,
    }
}
fn governance_auth_ingress_binding_for_config(
    config: &Config,
    handle: &'static str,
) -> sorafs_node::GovernanceDagRequestIngressBindingV1 {
    let service = &config.torii.sorafs_storage.governance_dag_service;
    let (scope, endpoint, max_body_bytes) = if handle == GOVERNANCE_IPFS_HANDLE {
        (
            sorafs_node::GovernanceDagAuthenticationScope::Ipfs,
            service
                .ipfs_api_url
                .as_deref()
                .expect("configured IPFS URL"),
            sorafs_node::governance_service::authenticated_ipfs_wire_body_max_bytes(
                service.max_request_bytes.0,
            )
            .expect("configured IPFS wire bound"),
        )
    } else {
        (
            sorafs_node::GovernanceDagAuthenticationScope::SignedHead,
            service
                .signed_head_url
                .as_deref()
                .expect("configured signed-head URL"),
            service.max_request_bytes.0,
        )
    };
    let endpoint_binding =
        sorafs_node::governance_dag_request_ingress_endpoint_binding_v1(scope, endpoint)
            .expect("configured test endpoint binding");
    sorafs_node::GovernanceDagRequestIngressBindingV1::try_new(
        scope,
        endpoint_binding,
        governance_auth_public_key(handle),
        max_body_bytes,
        service.request_auth_max_envelope_lifetime_secs,
        service.request_auth_max_future_skew_secs,
    )
    .expect("configured test ingress binding")
}
fn governance_service_dependencies(config: &Config, include_head: bool) -> IrohaRuntimeDeps {
    let dependencies = IrohaRuntimeDeps::default()
        .with_sorafs_governance_dag_ipfs_authenticator(Arc::new(GovernanceAuthenticator::new(
            GOVERNANCE_IPFS_HANDLE,
            governance_auth_ingress_binding_for_config(config, GOVERNANCE_IPFS_HANDLE),
        )))
        .with_sorafs_governance_dag_checkpoint_store(
            Arc::new(GovernanceCheckpointStore::default()),
        );
    if include_head {
        dependencies.with_sorafs_governance_dag_head_authenticator(Arc::new(
            GovernanceAuthenticator::new(
                GOVERNANCE_HEAD_HANDLE,
                governance_auth_ingress_binding_for_config(config, GOVERNANCE_HEAD_HANDLE),
            ),
        ))
    } else {
        dependencies
    }
}
#[test]
fn provider_ingest_catalog_projects_retention_authority_binding() {
    let mut config = default_runtime_config();
    configure_provider_ingest_runtime(&mut config);
    let ingest = config
        .torii
        .sorafs_storage
        .provider_ingest_runtime
        .as_mut()
        .expect("configured provider ingest");
    ingest.finalized_archive.retention_authority = Some(
        iroha_config::parameters::actual::SorafsProviderIngestFinalizedArchiveRetentionAuthority {
            handle: "sealed://sorafs/provider-ingest/retention-primary".to_owned(),
            revision: 9,
            policy_digest: [0xC9; 32],
        },
    );
    let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
        .expect("project provider-ingest retention binding");
    let retention = bindings
        .iter()
        .find(|binding| {
            binding.slot() == IrohaRuntimeProviderSlotV1::ProviderIngestRetentionAuthority
        })
        .expect("retention-authority binding");
    assert_eq!(
        retention.handle(),
        "sealed://sorafs/provider-ingest/retention-primary"
    );
    assert_eq!(retention.revision(), Some(9));
    assert_eq!(retention.policy_digest(), Some([0xC9; 32]));
}
#[test]
fn reputation_catalog_projects_exact_retention_authority_binding() {
    let mut config = default_runtime_config();
    configure_reputation_runtime(&mut config);
    let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
        .expect("project reputation retention binding");
    let retention = bindings
        .iter()
        .find(|binding| {
            binding.slot()
                == IrohaRuntimeProviderSlotV1::ReputationFinalizedArchiveRetentionAuthority
        })
        .expect("reputation retention-authority binding");
    assert_eq!(
        retention.handle(),
        "sealed://sorafs/reputation/retention-primary"
    );
    assert_eq!(retention.revision(), Some(9));
    assert_eq!(retention.policy_digest(), Some([0xC9; 32]));
    for (slot, handle, revision, policy_digest) in [
        (
            IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint,
            "sealed://sorafs/reputation/journal-primary",
            1,
            [0x60; 32],
        ),
        (
            IrohaRuntimeProviderSlotV1::ReputationJournalTransactionSubmitter,
            "queue://sorafs/reputation/journal-primary",
            11,
            [0x61; 32],
        ),
        (
            IrohaRuntimeProviderSlotV1::ReputationThresholdSigner,
            "software://sorafs/reputation/threshold-primary",
            12,
            [0x62; 32],
        ),
        (
            IrohaRuntimeProviderSlotV1::ReputationGovernanceDag,
            "dag://sorafs/reputation/publisher-primary",
            13,
            [0x63; 32],
        ),
    ] {
        let binding = bindings
            .iter()
            .find(|binding| binding.slot() == slot)
            .expect("reputation runtime provider binding");
        assert_eq!(binding.handle(), handle);
        assert_eq!(binding.revision(), Some(revision));
        assert_eq!(binding.policy_digest(), Some(policy_digest));
    }
    let mut dormant = config;
    dormant
        .torii
        .sorafs_storage
        .reputation_runtime
        .as_mut()
        .expect("configured reputation runtime")
        .finalized_archive_retention_authority = None;
    assert!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&dormant)
            .expect("project dormant reputation runtime")
            .iter()
            .all(|binding| {
                binding.slot()
                    != IrohaRuntimeProviderSlotV1::ReputationFinalizedArchiveRetentionAuthority
            })
    );
}
#[test]
fn reputation_catalog_rejects_zero_public_qualification_bindings() {
    for mutation in 0..8 {
        let mut config = default_runtime_config();
        configure_reputation_runtime(&mut config);
        let reputation = config
            .torii
            .sorafs_storage
            .reputation_runtime
            .as_mut()
            .expect("configured reputation runtime");
        match mutation {
            0 => reputation.journal_checkpoint_provider_revision = 0,
            1 => reputation.journal_checkpoint_provider_policy_digest = [0; 32],
            2 => reputation.journal_transaction_submitter_revision = 0,
            3 => reputation.journal_transaction_submitter_policy_digest = [0; 32],
            4 => reputation.threshold_signer_revision = 0,
            5 => reputation.threshold_signer_policy_digest = [0; 32],
            6 => reputation.governance_dag_revision = 0,
            7 => reputation.governance_dag_policy_digest = [0; 32],
            _ => unreachable!(),
        }
        assert!(matches!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&config),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(_))
        ));
    }
}
fn reputation_checkpoint_request() -> IrohaRuntimeProviderBindingsV1 {
    IrohaRuntimeProviderBindingsV1 {
        chain_id: "reputation-checkpoint-registry-test".to_owned(),
        network_id: test_network_id(0xA5),
        bindings: vec![
            IrohaRuntimeProviderBindingV1::try_new(
                IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint,
                REPUTATION_CHECKPOINT_HANDLE,
                Some(REPUTATION_CHECKPOINT_QUALIFICATION.revision()),
                Some(REPUTATION_CHECKPOINT_QUALIFICATION.policy_digest()),
            )
            .expect("valid reputation checkpoint binding"),
        ],
    }
}
fn resolve_reputation_checkpoint(
    requested: &IrohaRuntimeProviderBindingsV1,
    provider: ReputationJournalCheckpointProvider,
) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
    let registry = FixedRegistry(
        IrohaRuntimeDeps::default()
            .with_sorafs_reputation_journal_checkpoint_provider(Arc::new(provider)),
    );
    resolve_runtime_deps_from_bindings(requested, Some(&registry))
}
#[test]
fn reputation_checkpoint_resolution_is_exactly_scoped_and_qualified() {
    let requested = reputation_checkpoint_request();
    assert!(matches!(
        resolve_runtime_deps_from_bindings(&requested, Some(&EmptyRegistry)),
        Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
    ));
    assert!(
        resolve_reputation_checkpoint(&requested, ReputationJournalCheckpointProvider::exact(),)
            .is_ok()
    );
    let unrequested = IrohaRuntimeProviderBindingsV1 {
        chain_id: "reputation-checkpoint-registry-test".to_owned(),
        network_id: test_network_id(0xA5),
        bindings: Vec::new(),
    };
    assert!(matches!(
        resolve_reputation_checkpoint(&unrequested, ReputationJournalCheckpointProvider::exact(),),
        Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
    ));
    let mut substituted = ReputationJournalCheckpointProvider::exact();
    substituted.handle = "sealed://sorafs/reputation/substituted-checkpoint";
    assert!(matches!(
        resolve_reputation_checkpoint(&requested, substituted),
        Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
    ));
    let mut stale = ReputationJournalCheckpointProvider::exact();
    stale.first = sorafs_node::reputation::runtime::ReputationRuntimeProviderQualificationV1::new(
        REPUTATION_CHECKPOINT_QUALIFICATION.revision(),
        [0xA5; 32],
    );
    assert!(matches!(
        resolve_reputation_checkpoint(&requested, stale),
        Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
    ));
    let mut drifting = ReputationJournalCheckpointProvider::exact();
    drifting.later = Some(
        sorafs_node::reputation::runtime::ReputationRuntimeProviderQualificationV1::new(
            REPUTATION_CHECKPOINT_QUALIFICATION.revision(),
            [0xA6; 32],
        ),
    );
    assert!(matches!(
        resolve_reputation_checkpoint(&requested, drifting),
        Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
    ));
    let mut unavailable = ReputationJournalCheckpointProvider::exact();
    unavailable.load_error = Some(
        sorafs_node::reputation::runtime::ReputationJournalCheckpointExternalErrorV1::Unavailable,
    );
    assert!(matches!(
        resolve_reputation_checkpoint(&requested, unavailable),
        Err(IrohaRuntimeProviderRegistryErrorV1::Unavailable)
    ));
    let mut ambiguous = ReputationJournalCheckpointProvider::exact();
    ambiguous.load_error = Some(
        sorafs_node::reputation::runtime::ReputationJournalCheckpointExternalErrorV1::Ambiguous,
    );
    assert!(matches!(
        resolve_reputation_checkpoint(&requested, ambiguous),
        Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
    ));
}
#[test]
fn reputation_checkpoint_binding_rejects_non_v1_profile_and_test_handle() {
    for mutation in 0..2 {
        let mut config = default_runtime_config();
        configure_reputation_runtime(&mut config);
        let reputation = config
            .torii
            .sorafs_storage
            .reputation_runtime
            .as_mut()
            .expect("configured reputation runtime");
        if mutation == 0 {
            reputation.journal_checkpoint_provider_revision = 2;
        } else {
            reputation.journal_checkpoint_provider_handle =
                "sealed://sorafs/reputation/test-checkpoint".to_owned();
        }
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&config),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint,
            ))
        );
    }
}
#[test]
fn reputation_retention_projection_rejects_test_marked_and_stale_bindings() {
    for mutation in 0..3 {
        let mut config = default_runtime_config();
        configure_reputation_runtime(&mut config);
        let retention = config
            .torii
            .sorafs_storage
            .reputation_runtime
            .as_mut()
            .expect("configured reputation runtime")
            .finalized_archive_retention_authority
            .as_mut()
            .expect("configured reputation retention authority");
        match mutation {
            0 => {
                retention.handle = "sealed://sorafs/reputation/test-retention".to_owned();
            }
            1 => retention.revision = 0,
            2 => retention.policy_digest = [0; 32],
            _ => unreachable!(),
        }
        assert_eq!(
            IrohaRuntimeProviderBindingsV1::try_from_config(&config),
            Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::ReputationFinalizedArchiveRetentionAuthority,
            ))
        );
    }
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the test audits the complete public Governance DAG service projection"
)]
fn governance_service_catalog_projects_only_exact_public_provider_bindings() {
    let mut config = default_runtime_config();
    configure_governance_service(&mut config);
    let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
        .expect("project Governance DAG service provider bindings");
    let expected = [
        (
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
            GOVERNANCE_IPFS_HANDLE,
        ),
        (
            IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
            GOVERNANCE_HEAD_HANDLE,
        ),
        (
            IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
            GOVERNANCE_CHECKPOINT_HANDLE,
        ),
    ];
    for (slot, handle) in expected {
        let binding = bindings
            .iter()
            .find(|binding| binding.slot() == slot)
            .expect("projected Governance DAG service role");
        assert_eq!(binding.handle(), handle);
        assert_eq!(binding.revision(), Some(GOVERNANCE_QUALIFICATION.revision));
        assert_eq!(
            binding.policy_digest(),
            Some(GOVERNANCE_QUALIFICATION.policy_digest)
        );
        if matches!(
            slot,
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
                | IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator
        ) {
            let ingress = binding
                .governance_request_ingress_binding()
                .expect("request-auth roles carry an exact ingress binding");
            assert_eq!(ingress.public_key(), governance_auth_public_key(handle));
            let service = &config.torii.sorafs_storage.governance_dag_service;
            let (scope, endpoint, expected_body_bytes) =
                if slot == IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator {
                    let scope = sorafs_node::GovernanceDagAuthenticationScope::Ipfs;
                    (
                        scope,
                        service
                            .ipfs_api_url
                            .as_deref()
                            .expect("configured IPFS URL"),
                        sorafs_node::governance_service::authenticated_ipfs_wire_body_max_bytes(
                            service.max_request_bytes.0,
                        )
                        .expect("configured IPFS wire bound"),
                    )
                } else {
                    (
                        sorafs_node::GovernanceDagAuthenticationScope::SignedHead,
                        service
                            .signed_head_url
                            .as_deref()
                            .expect("configured signed-head URL"),
                        service.max_request_bytes.0,
                    )
                };
            assert_eq!(ingress.scope(), scope);
            assert_eq!(
                ingress.endpoint_binding(),
                sorafs_node::governance_dag_request_ingress_endpoint_binding_v1(scope, endpoint)
                    .expect("configured Governance ingress endpoint")
            );
            assert_eq!(ingress.max_body_bytes(), expected_body_bytes);
            assert_eq!(
                ingress.max_envelope_lifetime_secs(),
                service.request_auth_max_envelope_lifetime_secs
            );
            assert_eq!(
                ingress.max_future_skew_secs(),
                service.request_auth_max_future_skew_secs
            );
        } else {
            assert_eq!(binding.governance_request_ingress_binding(), None);
        }
    }
    let mut ipns_config = default_runtime_config();
    configure_governance_ipns_service(&mut ipns_config);
    let ipns = IrohaRuntimeProviderBindingsV1::try_from_config(&ipns_config)
        .expect("project IPNS Governance DAG service provider bindings");
    assert_eq!(
        ipns.iter()
            .filter(|binding| {
                matches!(
                    binding.slot(),
                    IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
                        | IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator
                        | IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
                )
            })
            .map(IrohaRuntimeProviderBindingV1::slot)
            .collect::<Vec<_>>(),
        vec![
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
            IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
        ]
    );
}
#[test]
fn standalone_governance_service_projection_is_exact_and_mode_scoped() {
    let chain_id = iroha_data_model::ChainId::from("governance-service-projection");
    let network_id = test_network_id(0xA5);
    let signed_head_view = governance_service_view("signed_http");
    let request_max = signed_head_view.service.max_request_bytes.0;
    let signed_head = IrohaRuntimeProviderBindingsV1::try_from_governance_dag_service_view(
        &chain_id,
        network_id,
        &signed_head_view,
    )
    .expect("project signed-head standalone service bindings");
    assert_eq!(signed_head.chain_id(), chain_id.to_string());
    assert_eq!(signed_head.network_id(), &network_id);
    assert_eq!(
        signed_head
            .iter()
            .map(IrohaRuntimeProviderBindingV1::slot)
            .collect::<Vec<_>>(),
        vec![
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
            IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
            IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
        ]
    );
    let ipfs_ingress = signed_head
        .iter()
        .find(|binding| {
            binding.slot() == IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
        })
        .and_then(IrohaRuntimeProviderBindingV1::governance_request_ingress_binding)
        .expect("standalone IPFS ingress binding");
    assert_eq!(
        ipfs_ingress.max_body_bytes(),
        sorafs_node::governance_service::authenticated_ipfs_wire_body_max_bytes(request_max)
            .expect("test IPFS wire bound")
    );
    let head = signed_head
        .iter()
        .find(|binding| {
            binding.slot() == IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator
        })
        .and_then(IrohaRuntimeProviderBindingV1::governance_request_ingress_binding)
        .expect("standalone signed-head ingress binding");
    assert_eq!(head.max_body_bytes(), request_max);
    assert!(
        signed_head
            .iter()
            .all(|binding| { binding.slot() != IrohaRuntimeProviderSlotV1::GovernanceDagSigner })
    );
    let ipns_bindings = IrohaRuntimeProviderBindingsV1::try_from_governance_dag_service_view(
        &chain_id,
        network_id,
        &governance_service_view("ipns"),
    )
    .expect("project IPNS standalone service bindings");
    assert_eq!(ipns_bindings.network_id(), &network_id);
    assert_eq!(
        ipns_bindings
            .iter()
            .map(IrohaRuntimeProviderBindingV1::slot)
            .collect::<Vec<_>>(),
        vec![
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
            IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
        ]
    );
}
#[test]
fn governance_request_ingress_binding_rejects_zero_public_bound() {
    assert_eq!(
        sorafs_node::GovernanceDagRequestIngressBindingV1::try_new(
            sorafs_node::GovernanceDagAuthenticationScope::Ipfs,
            [0x51; 32],
            governance_auth_public_key(GOVERNANCE_IPFS_HANDLE),
            0,
            30,
            5,
        ),
        Err(sorafs_node::GovernanceDagRequestIngressQualificationErrorV1::InvalidRequestBodyLimit)
    );
}
#[test]
fn standalone_governance_service_view_projection_rejects_invalid_public_bindings() {
    let chain_id = iroha_data_model::ChainId::from("governance-service-projection");
    let assert_invalid =
        |view: &iroha_config::parameters::actual::SorafsGovernanceDagServiceView, slot| {
            assert_eq!(
                IrohaRuntimeProviderBindingsV1::try_from_governance_dag_service_view(
                    &chain_id,
                    test_network_id(0xA5),
                    view,
                ),
                Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))
            );
        };
    let mut disabled = governance_service_view("signed_http");
    disabled.service.enabled = false;
    assert_invalid(
        &disabled,
        IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
    );
    let mut missing_ipfs = governance_service_view("signed_http");
    missing_ipfs.service.ipfs_authenticator_handle = None;
    assert_invalid(
        &missing_ipfs,
        IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
    );
    let mut missing_head = governance_service_view("signed_http");
    missing_head.service.head_authenticator_handle = None;
    assert_invalid(
        &missing_head,
        IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
    );
    let mut missing_checkpoint = governance_service_view("signed_http");
    missing_checkpoint.service.checkpoint_store_handle = None;
    assert_invalid(
        &missing_checkpoint,
        IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
    );
    let mut zero_qualified = governance_service_view("signed_http");
    zero_qualified.service.checkpoint_store_revision = Some(0);
    assert_invalid(
        &zero_qualified,
        IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
    );
    let mut zero_bound = governance_service_view("signed_http");
    zero_bound.service.max_request_bytes = Bytes(0);
    assert_invalid(
        &zero_bound,
        IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
    );
    let mut test_marked = governance_service_view("signed_http");
    test_marked.service.head_authenticator_handle = Some("vault://governance/test-head".to_owned());
    assert_invalid(
        &test_marked,
        IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
    );
    let mut invalid_mode = governance_service_view("signed_http");
    invalid_mode.service.head_mode = "compatibility".to_owned();
    assert_invalid(
        &invalid_mode,
        IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
    );
    let mut ipns_with_head_binding = governance_service_view("signed_http");
    ipns_with_head_binding.service.head_mode = "ipns".to_owned();
    assert_invalid(
        &ipns_with_head_binding,
        IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
    );
}
#[test]
fn governance_producer_catalog_projects_store_while_public_service_is_disabled() {
    let mut config = default_runtime_config();
    configure_governance_producer(&mut config);
    let bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
        .expect("project signed local Governance DAG producer bindings");
    let signer = bindings
        .iter()
        .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::GovernanceDagSigner)
        .expect("producer signer binding");
    assert_eq!(signer.handle(), GOVERNANCE_SIGNER_HANDLE);
    assert_eq!(signer.revision(), Some(GOVERNANCE_QUALIFICATION.revision));
    assert_eq!(
        signer.policy_digest(),
        Some(GOVERNANCE_QUALIFICATION.policy_digest)
    );
    assert_eq!(
        signer.governance_dag_publisher_peer_id(),
        Some(GOVERNANCE_PUBLISHER_PEER_ID.as_bytes())
    );
    assert_eq!(
        signer.governance_dag_publisher_public_key(),
        Some(governance_signer_public_key(0x73))
    );
    let checkpoint_store = bindings
        .iter()
        .find(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore)
        .expect("producer checkpoint-store binding");
    assert_eq!(checkpoint_store.handle(), GOVERNANCE_CHECKPOINT_HANDLE);
    assert_eq!(
        checkpoint_store.revision(),
        Some(GOVERNANCE_QUALIFICATION.revision)
    );
    assert_eq!(
        checkpoint_store.policy_digest(),
        Some(GOVERNANCE_QUALIFICATION.policy_digest)
    );
    assert!(bindings.iter().all(|binding| {
        !matches!(
            binding.slot(),
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
                | IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator
        )
    }));
}
#[test]
fn governance_signer_resolution_rejects_substitution_staleness_and_drift() {
    let mut config = default_runtime_config();
    configure_governance_producer(&mut config);
    let mut requested = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
        .expect("project Governance DAG signer binding");
    requested
        .bindings
        .retain(|binding| binding.slot() == IrohaRuntimeProviderSlotV1::GovernanceDagSigner);
    assert_eq!(requested.bindings.len(), 1);
    let resolve = |signer: GovernanceSigner| {
        let registry = FixedRegistry(
            IrohaRuntimeDeps::default().with_sorafs_governance_dag_signer(Arc::new(signer)),
        );
        resolve_runtime_deps_from_bindings(&requested, Some(&registry))
    };
    let resolved =
        resolve(GovernanceSigner::exact()).expect("exact Governance DAG signer must resolve");
    assert!(resolved.sorafs_governance_dag_signer.is_some());
    let mut substituted_peer = GovernanceSigner::exact();
    substituted_peer.publisher_peer_id = b"12D3KooWGovernanceProducerSubstitute".to_vec();
    assert!(matches!(
        resolve(substituted_peer),
        Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
    ));
    let mut substituted_key = GovernanceSigner::exact();
    substituted_key.key_pair = governance_signer_keypair(0x74);
    assert!(matches!(
        resolve(substituted_key),
        Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
    ));
    let mut lying_key = GovernanceSigner::exact();
    lying_key.signing_key_pair = Some(governance_signer_keypair(0x74));
    assert!(matches!(
        resolve(lying_key),
        Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
    ));
    let mut failed_sign = GovernanceSigner::exact();
    failed_sign.sign_error = true;
    assert!(matches!(
        resolve(failed_sign),
        Err(IrohaRuntimeProviderRegistryErrorV1::Unavailable)
    ));
    let mut failed_sign_with_drift = GovernanceSigner::exact();
    failed_sign_with_drift.sign_error = true;
    failed_sign_with_drift.later_public_key = Some(governance_signer_public_key(0x75));
    assert!(matches!(
        resolve(failed_sign_with_drift),
        Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
    ));
    let mut substituted_handle = GovernanceSigner::exact();
    substituted_handle.handle = "software://sorafs/governance-dag/secondary";
    assert!(matches!(
        resolve(substituted_handle),
        Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
    ));
    let mut stale = GovernanceSigner::exact();
    stale.first_qualification = sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
        GOVERNANCE_QUALIFICATION.revision + 1,
        GOVERNANCE_QUALIFICATION.policy_digest,
    );
    assert!(matches!(
        resolve(stale),
        Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
    ));
    let mut qualification_drift = GovernanceSigner::exact();
    qualification_drift.later_qualification = Some(
        sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
            GOVERNANCE_QUALIFICATION.revision + 1,
            GOVERNANCE_QUALIFICATION.policy_digest,
        ),
    );
    assert!(matches!(
        resolve(qualification_drift),
        Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
    ));
    let mut handle_drift = GovernanceSigner::exact();
    handle_drift.later_handle = Some("software://sorafs/governance-dag/secondary");
    assert!(matches!(
        resolve(handle_drift),
        Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
    ));
    let mut peer_drift = GovernanceSigner::exact();
    peer_drift.later_publisher_peer_id = Some(b"12D3KooWGovernanceProducerRotated".to_vec());
    assert!(matches!(
        resolve(peer_drift),
        Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
    ));
    let mut key_drift = GovernanceSigner::exact();
    key_drift.later_public_key = Some(governance_signer_public_key(0x75));
    assert!(matches!(
        resolve(key_drift),
        Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
    ));
}
#[test]
fn governance_signer_catalog_rejects_unqualified_manual_actual_config() {
    let mut config = default_runtime_config();
    configure_governance_producer(&mut config);
    let storage = &mut config.torii.sorafs_storage;
    storage.governance_dag_signer_revision = None;
    storage.governance_dag_signer_policy_digest = None;
    assert_eq!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&config),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::GovernanceDagSigner,
        ))
    );
    let mut missing_peer = default_runtime_config();
    configure_governance_producer(&mut missing_peer);
    missing_peer
        .torii
        .sorafs_storage
        .governance_dag_publisher_peer_id = None;
    assert_eq!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&missing_peer),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::GovernanceDagSigner,
        ))
    );
    let mut invalid_key = default_runtime_config();
    configure_governance_producer(&mut invalid_key);
    invalid_key
        .torii
        .sorafs_storage
        .governance_dag_publisher_public_key_hex = Some("00".repeat(32));
    assert_eq!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&invalid_key),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::GovernanceDagSigner,
        ))
    );
    let mut missing_directory = default_runtime_config();
    configure_governance_producer(&mut missing_directory);
    missing_directory.torii.sorafs_storage.governance_dag_dir = None;
    assert_eq!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&missing_directory),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::GovernanceDagSigner,
        ))
    );
    let mut disabled_storage = default_runtime_config();
    configure_governance_producer(&mut disabled_storage);
    disabled_storage.torii.sorafs_storage.enabled = false;
    assert_eq!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&disabled_storage),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::GovernanceDagSigner,
        ))
    );
}
#[test]
fn governance_producer_catalog_rejects_missing_partial_and_dormant_store_bindings() {
    for (label, handle, revision, policy_digest) in [
        ("missing", None, None, None),
        (
            "handle only",
            Some(GOVERNANCE_CHECKPOINT_HANDLE.to_owned()),
            None,
            None,
        ),
        (
            "missing policy",
            Some(GOVERNANCE_CHECKPOINT_HANDLE.to_owned()),
            Some(GOVERNANCE_QUALIFICATION.revision),
            None,
        ),
    ] {
        let mut config = default_runtime_config();
        configure_governance_producer(&mut config);
        let service = &mut config.torii.sorafs_storage.governance_dag_service;
        service.checkpoint_store_handle = handle;
        service.checkpoint_store_revision = revision;
        service.checkpoint_store_policy_digest = policy_digest;
        assert!(
            matches!(
                IrohaRuntimeProviderBindingsV1::try_from_config(&config),
                Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                    IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
                ))
            ),
            "{label} producer store binding must fail"
        );
    }
    let mut dormant = default_runtime_config();
    let service = &mut dormant.torii.sorafs_storage.governance_dag_service;
    service.checkpoint_store_handle = Some(GOVERNANCE_CHECKPOINT_HANDLE.to_owned());
    service.checkpoint_store_revision = Some(GOVERNANCE_QUALIFICATION.revision);
    service.checkpoint_store_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);
    assert!(matches!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&dormant),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
        ))
    ));
}
#[test]
fn governance_producer_catalog_rejects_disabled_service_authentication_bindings() {
    let mut config = default_runtime_config();
    configure_governance_producer(&mut config);
    let service = &mut config.torii.sorafs_storage.governance_dag_service;
    service.ipfs_authenticator_handle = Some(GOVERNANCE_IPFS_HANDLE.to_owned());
    service.ipfs_authenticator_revision = Some(GOVERNANCE_QUALIFICATION.revision);
    service.ipfs_authenticator_policy_digest = Some(GOVERNANCE_QUALIFICATION.policy_digest);
    assert!(matches!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&config),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
        ))
    ));
}
#[test]
fn governance_service_catalog_rejects_incomplete_or_test_marked_bindings() {
    let mut incomplete = default_runtime_config();
    incomplete
        .torii
        .sorafs_storage
        .governance_dag_service
        .enabled = true;
    assert!(matches!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&incomplete),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
        ))
    ));
    let mut test_marked = default_runtime_config();
    configure_governance_service(&mut test_marked);
    test_marked
        .torii
        .sorafs_storage
        .governance_dag_service
        .checkpoint_store_handle = Some("kms://governance/checkpoint-test".to_owned());
    assert!(matches!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&test_marked),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
        ))
    ));
}
#[test]
fn governance_service_resolution_rejects_missing_and_unrequested_adapters() {
    let mut signed_http = default_runtime_config();
    configure_governance_service(&mut signed_http);
    let mut signed_http_bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&signed_http)
        .expect("project signed-head Governance DAG bindings");
    signed_http_bindings.bindings.retain(|binding| {
        matches!(
            binding.slot(),
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
                | IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator
                | IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
        )
    });
    let missing_head = FixedRegistry(governance_service_dependencies(&signed_http, false));
    assert!(matches!(
        resolve_runtime_deps_from_bindings(&signed_http_bindings, Some(&missing_head)),
        Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
    ));
    let complete = FixedRegistry(governance_service_dependencies(&signed_http, true));
    resolve_runtime_deps_from_bindings(&signed_http_bindings, Some(&complete))
        .expect("resolve the complete signed-head adapter set");
    let mut substituted_dependencies = governance_service_dependencies(&signed_http, true);
    substituted_dependencies.sorafs_governance_dag_ipfs_authenticator = Some(Arc::new(
        GovernanceAuthenticator::new(
            GOVERNANCE_IPFS_HANDLE,
            governance_auth_ingress_binding_for_config(&signed_http, GOVERNANCE_IPFS_HANDLE),
        )
        .with_key_from(GOVERNANCE_HEAD_HANDLE),
    ));
    let substituted = FixedRegistry(substituted_dependencies);
    assert!(matches!(
        resolve_runtime_deps_from_bindings(&signed_http_bindings, Some(&substituted)),
        Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
    ));
    let mut ipns = default_runtime_config();
    configure_governance_ipns_service(&mut ipns);
    let mut ipns_bindings = IrohaRuntimeProviderBindingsV1::try_from_config(&ipns)
        .expect("project IPNS Governance DAG bindings");
    ipns_bindings.bindings.retain(|binding| {
        matches!(
            binding.slot(),
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator
                | IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator
                | IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore
        )
    });
    let complete_ipns = FixedRegistry(governance_service_dependencies(&ipns, false));
    resolve_runtime_deps_from_bindings(&ipns_bindings, Some(&complete_ipns))
        .expect("resolve the complete IPNS adapter set");
    let unexpected_head = FixedRegistry(governance_service_dependencies(&signed_http, true));
    assert!(matches!(
        resolve_runtime_deps_from_bindings(&ipns_bindings, Some(&unexpected_head)),
        Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
    ));
}
#[test]
fn native_signer_config_projection_preserves_every_public_identity_field() {
    let proof = ProofOutcomeTestSigner::new();
    let repair = RepairTestSigner::new();
    let reserve = ReserveTestSigner::new();
    let orderbook = OrderbookTestSigner::new();
    let expected = [
        (
            IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
            proof.expected_binding(),
        ),
        (
            IrohaRuntimeProviderSlotV1::RepairTransactionSigner,
            repair.expected_binding(),
        ),
        (
            IrohaRuntimeProviderSlotV1::ReserveTransactionSigner,
            reserve.expected_binding(),
        ),
        (
            IrohaRuntimeProviderSlotV1::OrderbookTransactionSigner,
            orderbook.expected_binding(),
        ),
    ];
    let mut config = default_runtime_config();
    let configured = &mut config.torii.sorafs_storage.native_transaction_signers;
    configured.proof_outcome = Some(actual_native_signer_binding(&expected[0].1));
    configured.repair = Some(actual_native_signer_binding(&expected[1].1));
    configured.reserve = Some(actual_native_signer_binding(&expected[2].1));
    configured.orderbook = Some(actual_native_signer_binding(&expected[3].1));
    let projected = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
        .expect("project exact native signer bindings");
    for (slot, exact) in expected {
        let binding = projected
            .iter()
            .find(|binding| binding.slot() == slot)
            .expect("projected native signer role");
        assert_eq!(binding.handle(), exact.handle());
        assert_eq!(binding.revision(), Some(exact.qualification().revision()));
        assert_eq!(
            binding.policy_digest(),
            Some(exact.qualification().policy_digest())
        );
        assert_eq!(binding.native_signer_binding(), Some(&exact));
        assert_eq!(
            binding.native_signer_algorithm(),
            exact.public_key().try_algorithm().ok()
        );
    }
}
#[test]
fn native_signer_config_projection_rejects_algorithm_and_authority_substitution() {
    let provider = ProofOutcomeTestSigner::new();
    let exact = provider.expected_binding();
    let mut config = default_runtime_config();
    let mut substituted = actual_native_signer_binding(&exact);
    substituted.algorithm = iroha_crypto::Algorithm::Secp256k1;
    config
        .torii
        .sorafs_storage
        .native_transaction_signers
        .proof_outcome = Some(substituted);
    assert!(matches!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&config),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner
        ))
    ));
    let other =
        iroha_crypto::KeyPair::try_from_seed(vec![0xA1; 32], iroha_crypto::Algorithm::Ed25519)
            .expect("derive substituted authority");
    let mut substituted = actual_native_signer_binding(&exact);
    substituted.authority = iroha_data_model::account::AccountId::new(other.public_key().clone());
    config
        .torii
        .sorafs_storage
        .native_transaction_signers
        .proof_outcome = Some(substituted);
    assert!(matches!(
        IrohaRuntimeProviderBindingsV1::try_from_config(&config),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner
        ))
    ));
}
#[test]
fn native_signer_slot_rejects_role_confusion_in_public_binding() {
    let proof = ProofOutcomeTestSigner::new();
    assert!(matches!(
        IrohaRuntimeProviderBindingV1::try_new_native_signer(
            IrohaRuntimeProviderSlotV1::RepairTransactionSigner,
            proof.expected_binding(),
        ),
        Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::RepairTransactionSigner
        ))
    ));
}
#[test]
fn registry_qualifies_all_four_native_signers_before_forwarding() {
    let proof = Arc::new(ProofOutcomeTestSigner::new());
    let repair = Arc::new(RepairTestSigner::new());
    let reserve = Arc::new(ReserveTestSigner::new());
    let orderbook = Arc::new(OrderbookTestSigner::new());
    let expected = [
        (
            IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
            proof.expected_binding(),
        ),
        (
            IrohaRuntimeProviderSlotV1::RepairTransactionSigner,
            repair.expected_binding(),
        ),
        (
            IrohaRuntimeProviderSlotV1::ReserveTransactionSigner,
            reserve.expected_binding(),
        ),
        (
            IrohaRuntimeProviderSlotV1::OrderbookTransactionSigner,
            orderbook.expected_binding(),
        ),
    ];
    let bindings = native_signer_catalog(expected.clone());
    let registry = FixedRegistry(
        IrohaRuntimeDeps::default()
            .with_sorafs_proof_outcome_signer(proof)
            .with_sorafs_repair_transaction_signer(repair)
            .with_sorafs_reserve_transaction_signer(reserve)
            .with_sorafs_orderbook_transaction_signer(orderbook),
    );
    let resolved = resolve_runtime_deps_from_bindings(&bindings, Some(&registry))
        .expect("qualify all native signers");
    let observed = [
        observed_native_signer_binding(
            resolved
                .sorafs_proof_outcome_signer
                .as_ref()
                .expect("qualified proof signer")
                .as_ref(),
        ),
        observed_native_signer_binding(
            resolved
                .sorafs_repair_transaction_signer
                .as_ref()
                .expect("qualified repair signer")
                .as_ref(),
        ),
        observed_native_signer_binding(
            resolved
                .sorafs_reserve_transaction_signer
                .as_ref()
                .expect("qualified reserve signer")
                .as_ref(),
        ),
        observed_native_signer_binding(
            resolved
                .sorafs_orderbook_transaction_signer
                .as_ref()
                .expect("qualified orderbook signer")
                .as_ref(),
        ),
    ];
    assert_eq!(
        observed,
        expected.map(|(_, binding)| binding),
        "qualified facades must expose only their immutable config bindings"
    );
}
#[test]
fn registry_rejects_missing_role_confused_substituted_and_stale_native_signers() {
    let good = Arc::new(ProofOutcomeTestSigner::new());
    let exact = good.expected_binding();
    let exact_catalog = native_signer_catalog([(
        IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
        exact.clone(),
    )]);
    assert!(matches!(
        resolve_runtime_deps_from_bindings(&exact_catalog, Some(&EmptyRegistry)),
        Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)
    ));
    let confused = Arc::new(RoleConfusedProofOutcomeSigner(ProofOutcomeTestSigner::new()));
    let confused_registry =
        FixedRegistry(IrohaRuntimeDeps::default().with_sorafs_proof_outcome_signer(confused));
    assert!(matches!(
        resolve_runtime_deps_from_bindings(&exact_catalog, Some(&confused_registry)),
        Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
    ));
    let substituted = iroha_torii::SorafsNativeTransactionSignerBindingV1::try_new(
        iroha_torii::SorafsNativeTransactionSignerRoleV1::ProofOutcome,
        "software://sorafs/proof-outcome/secondary",
        exact.authority().clone(),
        exact.public_key().clone(),
        exact.qualification(),
    )
    .expect("valid substituted config binding");
    let substituted_catalog = native_signer_catalog([(
        IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
        substituted,
    )]);
    let good_registry =
        FixedRegistry(IrohaRuntimeDeps::default().with_sorafs_proof_outcome_signer(good.clone()));
    assert!(matches!(
        resolve_runtime_deps_from_bindings(&substituted_catalog, Some(&good_registry)),
        Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
    ));
    let stale = iroha_torii::SorafsNativeTransactionSignerBindingV1::try_new(
        iroha_torii::SorafsNativeTransactionSignerRoleV1::ProofOutcome,
        exact.handle(),
        exact.authority().clone(),
        exact.public_key().clone(),
        iroha_torii::SorafsNativeTransactionSignerQualificationV1::new(
            exact.qualification().revision() + 1,
            exact.qualification().policy_digest(),
        ),
    )
    .expect("valid stale config binding");
    let stale_catalog = native_signer_catalog([(
        IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
        stale,
    )]);
    let good_registry =
        FixedRegistry(IrohaRuntimeDeps::default().with_sorafs_proof_outcome_signer(good));
    assert!(matches!(
        resolve_runtime_deps_from_bindings(&stale_catalog, Some(&good_registry)),
        Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
    ));
}
#[test]
fn unrequested_native_signers_are_rejected_individually() {
    let proof_provider = Arc::new(ProofOutcomeTestSigner::new());
    let proof_binding = proof_provider.expected_binding();
    let proof_signer = iroha_torii::qualify_sorafs_proof_outcome_transaction_signer_v1(
        proof_binding,
        proof_provider,
    )
    .expect("qualify proof-outcome test signer");
    let repair_provider = Arc::new(RepairTestSigner::new());
    let repair_binding = repair_provider.expected_binding();
    let repair_signer =
        iroha_torii::qualify_sorafs_repair_transaction_signer_v1(repair_binding, repair_provider)
            .expect("qualify repair test signer");
    let reserve_provider = Arc::new(ReserveTestSigner::new());
    let reserve_binding = reserve_provider.expected_binding();
    let reserve_signer = iroha_torii::qualify_sorafs_reserve_transaction_signer_v1(
        reserve_binding,
        reserve_provider,
    )
    .expect("qualify reserve test signer");
    let orderbook_provider = Arc::new(OrderbookTestSigner::new());
    let orderbook_binding = orderbook_provider.expected_binding();
    let orderbook_signer = iroha_torii::qualify_sorafs_orderbook_transaction_signer_v1(
        orderbook_binding,
        orderbook_provider,
    )
    .expect("qualify orderbook test signer");
    let unrequested_dependencies = [
        IrohaRuntimeDeps::default().with_sorafs_proof_outcome_signer(proof_signer),
        IrohaRuntimeDeps::default().with_sorafs_repair_transaction_signer(repair_signer),
        IrohaRuntimeDeps::default().with_sorafs_reserve_transaction_signer(reserve_signer),
        IrohaRuntimeDeps::default().with_sorafs_orderbook_transaction_signer(orderbook_signer),
    ];
    let empty_bindings = IrohaRuntimeProviderBindingsV1 {
        chain_id: "production-chain".to_owned(),
        network_id: test_network_id(0xA5),
        bindings: Vec::new(),
    };
    for dependencies in unrequested_dependencies {
        let registry = FixedRegistry(dependencies);
        assert!(matches!(
            resolve_runtime_deps_from_bindings(&empty_bindings, Some(&registry)),
            Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders)
        ));
    }
}
