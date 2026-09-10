// Actual coordinator argv through the real CLI parser and child action validator.
// Disposable descriptors only: this test never launches a process or contacts a host.

#[cfg(unix)]
#[test]
fn coordinator_write_canary_argv_passes_child_validation_for_all_core_actions() {
    use clap::Parser as _;

    struct NoChildProcess;
    impl ProcessRunner for NoChildProcess {
        fn run(&mut self, _spec: &ProcessSpec) -> Result<ProcessOutput> {
            panic!("the argv contract must not launch any process");
        }
    }

    let directory = super::super::private_custody_test_dir("taira-canary-argv-");
    let mut admitted = admitted_reset_fixture();
    admitted.inventory.qualification_scope = super::super::QualificationScopeV1::CoreTestnet;
    admitted.authorization.claims.qualification_scope = admitted.inventory.qualification_scope;
    let _chain = iroha::data_model::account::address::ChainDiscriminantGuard::enter(
        admitted.inventory.chain_discriminant,
    );
    let config_path = directory.path().join("client.toml");
    fs::write(
        &config_path,
        client_config_bytes_for_inventory(&admitted.inventory),
    )
    .expect("public client fixture");
    fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600))
        .expect("private client fixture mode");
    let token_path = directory.path().join("onboarding.token");
    fs::write(&token_path, b"FIXTURE-TOKEN-ONLY-NOT-A-RUNTIME-SECRET")
        .expect("disposable onboarding fixture");
    fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600))
        .expect("private token fixture mode");

    // The basic argv builder does not consume the retained Inrou stage. Populate
    // its identity from the same public inventory without preparing any VM data.
    let canary = &admitted.inventory.inrou_canary;
    let stage_identity = crate::soracloud::TairaInrouStageIdentity {
        service_name: canary.service_name.clone(),
        service_version: canary.service_version.clone(),
        route_host: canary.route_host.clone(),
        route_path_prefix: canary.route_path_prefix.clone(),
        healthcheck_path: canary.healthcheck_path.clone(),
        stage_mode: "deploy".to_owned(),
        bundle_hash: canary.bundle_hash.clone(),
        bundle_content_cid: canary.bundle_content_cid.clone(),
        bundle_manifest_digest_hex: canary.bundle_manifest_digest_hex.clone(),
        guest_content_cid: canary.guest_content_cid.clone(),
        guest_manifest_digest_hex: canary.guest_manifest_digest_hex.clone(),
        discovery_payload_dir: canary.discovery_payload_dir.clone(),
        discovery_document_hash: canary.discovery_document_hash.clone(),
        discovery_content_cid: canary.discovery_content_cid.clone(),
        discovery_manifest_digest_hex: canary.discovery_manifest_digest_hex.clone(),
        public_discovery_url: canary.public_discovery_url.clone(),
        public_discovery_cid_host_url: canary.public_discovery_cid_host_url.clone(),
        deployment_bundle_hash: canary.deployment_bundle_hash.clone(),
        container_manifest_hash: canary.container_manifest_hash.clone(),
        service_manifest_hash: canary.service_manifest_hash.clone(),
        placement_targets: inventory_inrou_placement_targets(&admitted.inventory)
            .expect("public placement fixture"),
    };
    let transport = OpenSshTransport {
        admitted: &admitted,
        runtime: RuntimeCustody {
            client_config: pin_owner_private_file(&config_path, "client fixture")
                .expect("pin client fixture"),
            validator_client_configs: Vec::new(),
            validator_operator_key: None,
            onboarding_token: Some(
                pin_owner_private_file(&token_path, "token fixture").expect("pin token fixture"),
            ),
            inrou_stage_dir: directory.path().join("unused-inrou-stage"),
            snapshot_stage_files: Vec::new(),
            stage_identity,
            fee_args: super::super::PublicResetApply::fee_args(&admitted.inventory)
                .expect("actual parent fee arguments"),
        },
        closure: LocalArtifactClosure {
            files: BTreeMap::new(),
        },
        local_receipt_root: directory.path().to_owned(),
        runner: NoChildProcess,
    };
    let expected_policy = inventory_faucet_policy(&admitted.inventory).expect("signed policy");
    let phases = [
        "pre_edge",
        "restart-wave-1",
        "restart-wave-2",
        "restart-wave-3",
        "restart-wave-4",
        "post_edge",
    ];
    let mut census = BTreeSet::new();
    for phase in phases {
        for kind in admitted.inventory.qualification_scope.canary_kinds() {
            for action in [
                WriteCanaryChildAction::Prepare,
                WriteCanaryChildAction::Submit,
                WriteCanaryChildAction::Recover,
            ] {
                let context = format!("{phase}/{kind}/{action:?}");
                assert!(census.insert(context.clone()), "duplicate argv case");
                let idempotency_key = child_mutation_idempotency_key(
                    &admitted.inventory.authorization_nonce,
                    phase,
                    kind,
                );
                let (mut args, mut inherited) = transport
                    .write_canary_base_args(phase, kind, &idempotency_key, 120, action)
                    .unwrap_or_else(|error| panic!("{context}: parent argv failed: {error:#}"));
                let envelope = tempfile::tempfile().expect("disposable envelope descriptor");
                let envelope_fd = u32::try_from(envelope.as_raw_fd()).expect("envelope FD");
                action.append_envelope_args(&mut args, envelope.as_raw_fd());
                inherited.push(envelope);
                let prerequisite_fd = if action == WriteCanaryChildAction::Prepare
                    && mutation_predecessor_kind(kind, phase).is_some()
                {
                    let predecessor =
                        tempfile::tempfile().expect("disposable predecessor descriptor");
                    let fd = u32::try_from(predecessor.as_raw_fd()).expect("predecessor FD");
                    args.extend(["--prerequisite-envelope-fd".into(), fd.to_string().into()]);
                    inherited.push(predecessor);
                    Some(fd)
                } else {
                    None
                };
                // The common owned process launcher adds this global flag.
                args.insert(0, "--machine".into());
                let parsed = crate::Args::try_parse_from(
                    std::iter::once(OsString::from("iroha")).chain(args),
                )
                .unwrap_or_else(|error| panic!("{context}: actual child parser failed: {error}"));
                assert!(parsed.machine);
                assert!(parsed.config.is_none());
                assert_eq!(
                    parsed.config_source_path.as_deref(),
                    Some(config_path.as_path())
                );
                let config_fd = parsed.config_fd.expect("inherited client descriptor");
                assert_eq!(config_fd, u32::try_from(inherited[0].as_raw_fd()).unwrap());
                let (config, _) = crate::client_config::load_inherited(config_fd, &config_path)
                    .expect("actual child config loader");
                validate_client_network_identity(&config, &admitted.inventory)
                    .expect("exact Taira chain, discriminant and genesis");
                assert_eq!(
                    parsed.fee_payment.selection().expect("actual fee parser"),
                    iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None)
                );
                let crate::Command::Taira(crate::taira::Command::WriteCanary(child)) =
                    parsed.command
                else {
                    panic!("{context}: wrong parsed child command");
                };
                let expected_operation = match *kind {
                    "onboarding" => crate::taira::WriteCanaryOperation::Onboarding,
                    "faucet" => crate::taira::WriteCanaryOperation::Faucet,
                    "write_canary" => crate::taira::WriteCanaryOperation::FinalCanary,
                    _ => panic!("unexpected core canary kind"),
                };
                assert_eq!(child.operation, expected_operation, "{context}");
                assert_eq!(
                    child.public_root,
                    mutation_probe_root(&admitted.inventory, phase).unwrap(),
                    "{context}"
                );
                assert_eq!(
                    child.authorization_sha256, admitted.authorization_sha256,
                    "{context}"
                );
                assert_eq!(
                    child.authorization_nonce, admitted.inventory.authorization_nonce,
                    "{context}"
                );
                assert_eq!(child.mutation_phase, phase, "{context}");
                assert_eq!(child.idempotency_key, idempotency_key, "{context}");
                assert_eq!(
                    child.execution_expires_at_unix_ms,
                    admitted.authorization.claims.execution_expires_at_unix_ms,
                    "{context}"
                );
                assert_eq!(child.timeout_secs, 120);
                assert!(child.json);
                assert_eq!(
                    child.prepare_envelope,
                    action == WriteCanaryChildAction::Prepare
                );
                assert_eq!(
                    child.prepared_output_fd,
                    (action == WriteCanaryChildAction::Prepare).then_some(envelope_fd)
                );
                assert_eq!(
                    child.submit_prepared_envelope_fd,
                    (action == WriteCanaryChildAction::Submit).then_some(envelope_fd)
                );
                assert_eq!(
                    child.recover_prepared_envelope_fd,
                    (action == WriteCanaryChildAction::Recover).then_some(envelope_fd)
                );
                assert_eq!(child.prerequisite_envelope_fd, prerequisite_fd, "{context}");
                assert!(child.onboarding_token_file.is_none());
                let token_required =
                    *kind == "onboarding" && action != WriteCanaryChildAction::Recover;
                assert_eq!(
                    child.onboarding_token_fd.is_some(),
                    token_required,
                    "{context}"
                );
                for fd in [
                    Some(config_fd),
                    Some(envelope_fd),
                    prerequisite_fd,
                    child.onboarding_token_fd,
                ]
                .into_iter()
                .flatten()
                {
                    assert!(
                        inherited
                            .iter()
                            .any(|file| file.as_raw_fd() == i32::try_from(fd).unwrap()),
                        "{context}: parsed descriptor lacks parent custody"
                    );
                }
                assert_eq!(
                    inherited.len(),
                    2 + usize::from(token_required) + usize::from(prerequisite_fd.is_some())
                );
                let (current, predecessor) = crate::taira::validate_parent_write_canary_for_test(
                    &child,
                )
                .unwrap_or_else(|error| {
                    panic!("{context}: child action/policy validation failed: {error:#}")
                });
                assert_eq!(
                    current.as_ref(),
                    (*kind == "faucet").then_some(&expected_policy),
                    "{context}"
                );
                assert_eq!(
                    predecessor.as_ref(),
                    (*kind == "write_canary" && action == WriteCanaryChildAction::Prepare)
                        .then_some(&expected_policy),
                    "{context}"
                );
            }
        }
    }
    assert_eq!(
        census.len(),
        54,
        "six phases, three core operations and three actions"
    );
}
