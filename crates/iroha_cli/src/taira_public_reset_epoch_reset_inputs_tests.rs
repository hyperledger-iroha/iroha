use super::*;

/// Structural plan fixture only. Production prepare() derives the entire public
/// view from held native source/artifact/config inputs before calling build_plan.
fn fixture() -> (
    PrepareEpochSupervisorPlan,
    super::super::super::InventoryV1,
    public_inputs::PublicInputsV1,
) {
    let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
    let inventory = super::super::super::sample_inventory_fixture();
    let old = &inventory.epoch_supervisor;
    let policy: NativePolicyV1 = json::from_slice(&old.policy_bytes).unwrap();
    let args = PrepareEpochSupervisorPlan {
        intent: "/public/intent.json".into(),
        local: inputs::ResetContextInputs {
            public_inputs: "/public/genesis".into(),
            runtime_client_config: "/custody/runtime.toml".into(),
            maintenance_admin_config: "/custody/administrator.toml".into(),
            validator_client_config: (0..4)
                .map(|index| PathBuf::from(format!("/custody/client{index}.toml")))
                .collect(),
            onboarding_token: "/custody/onboarding-token".into(),
            validator_operator_key: "/custody/http-operator.key".into(),
            inrou_stage_dir: None,
            validator_unit: (0..4)
                .map(|index| PathBuf::from(format!("/public/validator{index}.service")))
                .collect(),
            edge_unit: "/public/edge.service".into(),
            known_hosts: "/public/known-hosts".into(),
        },
        host_slug: old.host_slug.clone(),
        authorization: OngoingAuthorization::UntilStopped,
        payment_asset: policy.intent.payment_asset,
        transaction_fee_maximum: policy.intent.transaction_fee_maximum,
        first_epoch: policy.intent.first_epoch,
        batch_epochs: u16::try_from(policy.intent.batch_epochs).unwrap(),
        operation_timeout_ms: policy.intent.operation_timeout_ms,
        provision_timeout_ms: policy.provision_timeout_ms,
        timeout_ms: old.timeout_ms,
        epoch_seed_source: old
            .original_seed_sources
            .iter()
            .map(|s| PathBuf::from(&s.path))
            .collect(),
        prior_state: PriorState::Absent,
        prior_plan: None,
        output_dir: "/public/new-output".into(),
    };
    let public = public_inputs::PublicInputsV1 {
        schema: "iroha.taira.public-reset.public-inputs.v1".into(),
        network_id: inventory
            .maintenance_admin_identity
            .network_id
            .parse()
            .unwrap(),
        genesis_hash: inventory.next_genesis_hash.clone(),
        signed_genesis_sha256: "a".repeat(64),
        raw_manifest_sha256: "b".repeat(64),
        genesis_public_key: inventory.beacon_bootstrap.genesis_public_key.clone(),
        canary_public_key: AccountId::parse_encoded(
            &inventory.canary_onboarding_request.account_id,
        )
        .unwrap()
        .try_signatory()
        .unwrap()
        .clone(),
        canary_onboarding_request: inventory.canary_onboarding_request.clone(),
    };
    (args, inventory, public)
}

fn view<'a>(
    inventory: &'a super::super::super::InventoryV1,
    public: &'a public_inputs::PublicInputsV1,
) -> PlanContext<'a> {
    PlanContext {
        revision: &inventory.revision,
        validators: &inventory.validators,
        validator_clients: &inventory.validator_clients,
        operator_public_key: &inventory.operator_public_key,
        maintenance_admin_identity: &inventory.maintenance_admin_identity,
        maintenance_admin_config_sha256: &inventory.maintenance_admin_config_sha256,
        http_operator_key_sha256: &inventory.epoch_supervisor.http_operator_key_sha256,
        public,
        observation_trust_bytes: &inventory.epoch_supervisor.observation_trust_bytes,
    }
}

#[test]
fn reset_producer_derives_exact_policy_unit_custody_and_update_binding() {
    let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
    let (args, context, public) = fixture();
    let plan = build_plan(&args, &view(&context, &public), None).unwrap();
    let second = build_plan(&args, &view(&context, &public), None).unwrap();
    assert_eq!(json::to_vec(&plan).unwrap(), json::to_vec(&second).unwrap());
    assert_eq!(plan.policy_sha256, sha256_hex(&plan.policy_bytes));
    assert_eq!(plan.unit_sha256, sha256_hex(&plan.unit_bytes));
    let policy: NativePolicyV1 = json::from_slice(&plan.policy_bytes).unwrap();
    assert_eq!(policy.intent.authorization, "until_stopped");
    assert_eq!(policy.intent.first_epoch, args.first_epoch);
    let cli = Path::new(&policy.kagami.path).with_file_name("iroha");
    assert_eq!(
        plan.unit_bytes,
        epoch_supervisor::render_unit(&plan, cli.to_str().unwrap()).unwrap()
    );
    let custody: SeedCustodyV1 = json::from_slice(&plan.custody_bytes).unwrap();
    for (index, row) in custody.seeds.iter().enumerate() {
        assert_eq!(
            Path::new(&row.path),
            epoch_seed_custody::destination(public.network_id, index).unwrap()
        );
        assert_eq!(row.validator, plan.original_seed_sources[index].validator);
    }
    let binding = epoch_generation::binding_from_plan(&plan).unwrap();
    let restored = epoch_generation::as_plan(
        &binding,
        plan.admin_config_sha256.clone(),
        plan.http_operator_key_sha256.clone(),
        "absent",
    )
    .unwrap();
    assert_eq!(restored.unit_bytes, plan.unit_bytes);
    assert_eq!(restored.policy_bytes, plan.policy_bytes);
}

#[test]
fn reset_producer_rejects_implicit_prior_and_invalid_ongoing_bounds() {
    let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
    let (mut args, mut context, public) = fixture();
    args.prior_state = PriorState::Running;
    assert!(build_plan(&args, &view(&context, &public), None).is_err());
    args.prior_state = PriorState::Absent;
    assert!(build_plan(&args, &view(&context, &public), Some(b"{}".to_vec())).is_err());
    args.first_epoch = u64::MAX;
    assert!(build_plan(&args, &view(&context, &public), None).is_err());
    args.first_epoch = 1;
    args.batch_epochs = 1;
    assert!(build_plan(&args, &view(&context, &public), None).is_err());
    args.batch_epochs = 2;
    context.epoch_supervisor.http_operator_key_sha256 = "0".repeat(64);
    assert!(build_plan(&args, &view(&context, &public), None).is_err());
}

#[test]
fn reset_producer_rejects_unmapped_sources_and_admin_genesis_substitution() {
    let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
    let (mut args, mut context, public) = fixture();
    args.epoch_seed_source[1] = args.epoch_seed_source[0].clone();
    assert!(build_plan(&args, &view(&context, &public), None).is_err());
    let (args, _, _) = fixture();
    context.maintenance_admin_identity.genesis_hash = "a".repeat(64);
    assert!(build_plan(&args, &view(&context, &public), None).is_err());
    let (_, mut context, _) = fixture();
    context.maintenance_admin_identity.torii_origin = "https://unselected.example/".into();
    assert!(build_plan(&args, &view(&context, &public), None).is_err());
}

#[test]
fn reset_producer_requires_explicit_until_stopped_cli_intent() {
    let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
    use clap::{Parser, ValueEnum as _};
    #[derive(Parser)]
    struct Command {
        #[command(flatten)]
        args: PrepareEpochSupervisorPlan,
    }
    let (args, _, _) = fixture();
    let command = vec![
        "producer".into(),
        "--intent".into(),
        "/public/intent.json".into(),
        "--public-inputs".into(),
        "/public/genesis".into(),
        "--runtime-client-config".into(),
        "/custody/runtime.toml".into(),
        "--maintenance-admin-config".into(),
        "/custody/administrator.toml".into(),
        "--validator-client-config".into(),
        "/custody/client0.toml".into(),
        "/custody/client1.toml".into(),
        "/custody/client2.toml".into(),
        "/custody/client3.toml".into(),
        "--onboarding-token".into(),
        "/custody/onboarding-token".into(),
        "--validator-operator-key".into(),
        "/custody/http-operator.key".into(),
        "--validator-unit".into(),
        "/public/validator0.service".into(),
        "/public/validator1.service".into(),
        "/public/validator2.service".into(),
        "/public/validator3.service".into(),
        "--edge-unit".into(),
        "/public/edge.service".into(),
        "--known-hosts".into(),
        "/public/known-hosts".into(),
        "--host-slug".into(),
        args.host_slug,
        "--payment-asset".into(),
        args.payment_asset.to_string(),
        "--transaction-fee-maximum".into(),
        args.transaction_fee_maximum.to_string(),
        "--first-epoch".into(),
        "1".into(),
        "--batch-epochs".into(),
        "2".into(),
        "--operation-timeout-ms".into(),
        "1000".into(),
        "--provision-timeout-ms".into(),
        "1000".into(),
        "--timeout-ms".into(),
        "1000".into(),
        "--epoch-seed-source".into(),
        "/original/0.seed".into(),
        "/original/1.seed".into(),
        "/original/2.seed".into(),
        "/original/3.seed".into(),
        "--prior-state".into(),
        "absent".into(),
        "--output-dir".into(),
        "/public/new-output".into(),
    ];
    assert!(Command::try_parse_from(&command).is_err());
    let mut explicit = command;
    explicit.extend(["--authorization".into(), "until-stopped".into()]);
    assert!(Command::try_parse_from(&explicit).is_ok());
    // The clean first invocation supplies native paths, never a computed hash,
    // prior trust export or plan-dependent inventory draft.
    for removed in [
        "--inventory-draft",
        "--observation-trust",
        "--http-operator-key-sha256",
    ] {
        let mut rejected = explicit.clone();
        rejected.extend([removed.into(), "/public/forbidden-input".into()]);
        assert!(Command::try_parse_from(rejected).is_err());
    }
    for required in [
        "--maintenance-admin-config",
        "--validator-operator-key",
        "--public-inputs",
    ] {
        let mut missing = explicit.clone();
        let index = missing.iter().position(|arg| arg == required).unwrap();
        missing.drain(index..index + 2);
        assert!(Command::try_parse_from(missing).is_err());
    }
    assert!(OngoingAuthorization::from_str("until-reset-expiry", false).is_err());
}

#[test]
fn reset_producer_publication_is_atomic_and_never_replaces() {
    let _guard = ChainDiscriminantGuard::enter(super::super::super::CHAIN_DISCRIMINANT);
    let (args, context, public) = fixture();
    let plan = build_plan(&args, &view(&context, &public), None).unwrap();
    let directory = super::super::super::private_custody_test_dir("epoch-public-inputs-");
    let output = directory.path().join("new-inputs");
    publish(&output, &plan).unwrap();
    assert_eq!(fs::read_dir(&output).unwrap().count(), 3);
    let expected_plan = json::to_vec(&plan).unwrap();
    assert_eq!(
        fs::read(output.join("observation-trust.json")).unwrap(),
        plan.observation_trust_bytes
    );
    let expected_binding =
        json::to_vec(&epoch_generation::binding_from_plan(&plan).unwrap()).unwrap();
    assert_eq!(
        fs::read(output.join("supervisor-plan.json")).unwrap(),
        expected_plan
    );
    assert_eq!(
        fs::read(output.join("supervisor-binding.json")).unwrap(),
        expected_binding
    );
    assert!(publish(&output, &plan).is_err());
    assert_eq!(
        fs::read(output.join("supervisor-plan.json")).unwrap(),
        expected_plan
    );
    assert_eq!(
        fs::read(output.join("supervisor-binding.json")).unwrap(),
        expected_binding
    );
    let invalid_output = directory.path().join("must-stay-absent");
    let mut invalid = plan;
    invalid.policy_sha256 = "0".repeat(64);
    assert!(publish(&invalid_output, &invalid).is_err());
    assert!(!invalid_output.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}
