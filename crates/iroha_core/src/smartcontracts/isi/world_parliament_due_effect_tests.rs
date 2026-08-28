// Exact-due automatic-enactment coverage for the remaining typed Parliament effects.

fn assert_exact_due_parliament_effect_enacted(
    state_transaction: &StateTransaction<'_, '_>,
    fixture: &DueParliamentCertificateFixture,
) {
    assert_eq!(
        state_transaction.block_height(),
        PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
        "the effect must execute at the certificate's exact due height",
    );
    let attempt = state_transaction
        .world
        .parliament_attempts
        .get(&fixture.governance_attempt_id)
        .expect("terminal exact-due Parliament attempt");
    assert_eq!(attempt.attempt().status, GovernanceAttemptStatusV1::Enacted);
    assert_eq!(
        attempt.terminal_height(),
        Some(PARLIAMENT_DUE_CERTIFICATE_HEIGHT)
    );
    assert_eq!(
        attempt
            .certificate()
            .map(|certificate| certificate.enact_at_height),
        Some(PARLIAMENT_DUE_CERTIFICATE_HEIGHT)
    );
    let proposal = state_transaction
        .world
        .governance_proposals
        .get(&fixture.proposal_id)
        .expect("enacted exact-due governance proposal");
    assert_eq!(
        proposal.status,
        crate::state::GovernanceProposalStatus::Enacted
    );
    assert!(
        state_transaction
            .world
            .internal_event_buf
            .iter()
            .any(|event| matches!(
                event.as_ref(),
                iroha_data_model::events::data::DataEvent::Governance(
                    GovernanceEvent::ProposalEnacted(enacted)
                ) if enacted.id == fixture.proposal_id
            )),
        "automatic success must emit the typed ProposalEnacted event",
    );
    assert_automatic_parliament_execution_event(
        state_transaction,
        fixture,
        gov::ParliamentAutomaticExecutionOutcomeV1::Enacted,
    );
}

#[test]
fn parliament_runtime_upgrade_enacts_at_the_exact_due_height() {
    let state = blank_test_state();
    let block = new_dummy_block_at_height(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).expect("due height is nonzero"),
    );
    let mut state_block = state.block(block.as_ref().header());
    let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
        name: "exact-due-runtime-upgrade".to_owned(),
        description: "activate the fixed first-release ABI at the certified height".to_owned(),
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        added_syscalls: Vec::new(),
        added_pointer_types: Vec::new(),
        start_height: PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
        end_height: PARLIAMENT_DUE_CERTIFICATE_HEIGHT + 1,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let upgrade_id = manifest.id();
    let fixture = {
        let mut seed = state_block.transaction();
        let fixture = seed_due_parliament_certificate(
            &mut seed,
            ProposalKind::RuntimeUpgrade(RuntimeUpgradeProposal {
                manifest: manifest.clone(),
            }),
        );
        seed.apply();
        fixture
    };

    let mut execution = state_block.transaction();
    execution.world.internal_event_buf.clear();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .expect("execute exact-due runtime-upgrade certificate"),
        DueParliamentCertificateExecutionV1::Applied
    );
    let record = execution
        .world
        .runtime_upgrades
        .get(&upgrade_id)
        .expect("runtime-upgrade effect record");
    assert_eq!(record.manifest, manifest);
    assert_eq!(
        record.status,
        iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(
            PARLIAMENT_DUE_CERTIFICATE_HEIGHT
        )
    );
    assert_eq!(&record.proposer, &*ALICE_ID);
    assert_eq!(record.created_height, PARLIAMENT_DUE_CERTIFICATE_HEIGHT);
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
}

#[test]
fn parliament_sorafs_provider_owner_enacts_at_the_exact_due_height() {
    use iroha_data_model::isi::sorafs::{
        EstablishSorafsProviderOwnerV1, SorafsProviderGovernanceActionV1,
    };
    use iroha_executor_data_model::permission::sorafs::CanOperateSorafsRepair;

    let state = blank_test_state();
    let block = new_dummy_block_at_height(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).expect("due height is nonzero"),
    );
    let mut state_block = state.block(block.as_ref().header());
    let provider_id = iroha_data_model::sorafs::capacity::ProviderId::new([0xA8; 32]);
    let fixture = {
        let mut seed = state_block.transaction();
        bootstrap_alice_account(&mut seed);
        let fixture = seed_due_parliament_certificate(
            &mut seed,
            ProposalKind::SorafsProviderGovernance(SorafsProviderGovernanceProposal {
                action: Box::new(SorafsProviderGovernanceActionV1::Establish(
                    EstablishSorafsProviderOwnerV1 {
                        provider_id,
                        owner: ALICE_ID.clone(),
                    },
                )),
            }),
        );
        seed.apply();
        fixture
    };

    let mut execution = state_block.transaction();
    execution.world.internal_event_buf.clear();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .expect("execute exact-due SoraFS provider certificate"),
        DueParliamentCertificateExecutionV1::Applied
    );
    assert_eq!(
        execution.world.provider_owners.get(&provider_id),
        Some(&*ALICE_ID)
    );
    let repair_permission = Permission::from(CanOperateSorafsRepair { provider_id });
    assert!(
        execution
            .world
            .account_permissions
            .get(&ALICE_ID)
            .is_some_and(|permissions| permissions.contains(&repair_permission)),
        "provider enactment must install its typed repair-worker authorization",
    );
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
}

#[test]
fn parliament_musubi_action_authorization_enacts_at_the_exact_due_height() {
    use iroha_data_model::musubi::{
        MusubiGovernanceDecisionV1, MusubiPackageIdV1, MusubiPackageScopeV1,
        MusubiParliamentActionV1, MusubiRecoverPackageOwnersV1,
    };

    let state = blank_test_state();
    let block = new_dummy_block_at_height(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).expect("due height is nonzero"),
    );
    let mut state_block = state.block(block.as_ref().header());
    let package = MusubiPackageIdV1::new(
        DataSpaceId::new(7),
        MusubiPackageScopeV1::DataspaceRoot,
        "exact-due-recovery"
            .parse()
            .expect("canonical Musubi package name"),
    );
    let action = MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
        package: package.clone(),
        owners: vec![ALICE_ID.clone()],
        expected_revision: 1,
    });
    action.validate().expect("valid exact Musubi action");
    let fixture = {
        let mut seed = state_block.transaction();
        let fixture = seed_due_parliament_certificate(
            &mut seed,
            ProposalKind::MusubiRegistryGovernance(action.clone()),
        );
        seed.apply();
        fixture
    };

    let mut execution = state_block.transaction();
    execution.world.internal_event_buf.clear();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .expect("execute exact-due Musubi authorization certificate"),
        DueParliamentCertificateExecutionV1::Applied
    );
    let proposal = execution
        .world
        .governance_proposals
        .get(&fixture.proposal_id)
        .expect("enacted Musubi authorization proposal");
    assert!(matches!(
        &proposal.kind,
        ProposalKind::MusubiRegistryGovernance(enacted) if enacted == &action
    ));
    assert!(
        execution.world.musubi_packages.get(&package).is_none(),
        "enactment authorizes the delayed Musubi mutation without bypassing its target ISI",
    );
    assert!(
        execution
            .world
            .musubi_governance_decisions
            .get(&fixture.proposal_id)
            .is_none(),
        "the enacted authorization must remain unconsumed until the delayed target ISI",
    );
    let decision = MusubiGovernanceDecisionV1 {
        decision_id: fixture.proposal_id,
        action_digest: action.action_digest(),
        enacted_at_height: PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
        execute_after_height: PARLIAMENT_DUE_CERTIFICATE_HEIGHT
            .checked_add(execution.gov.min_enactment_delay.max(1))
            .expect("Musubi execution boundary does not overflow"),
    };
    decision
        .validate()
        .expect("typed delayed Musubi authorization is valid");
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
}

#[test]
fn parliament_validation_fee_policy_enacts_at_the_exact_due_height() {
    use iroha_data_model::validation_fee::{
        VALIDATION_FEE_DS_SCALE, VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS,
        VALIDATION_FEE_POLICY_SCHEMA_VERSION, ValidationFeeChargingMode, ValidationFeePolicyV1,
    };

    let state = blank_test_state();
    let block = new_dummy_block_at_height(
        NonZeroU64::new(PARLIAMENT_DUE_CERTIFICATE_HEIGHT).expect("due height is nonzero"),
    );
    let mut state_block = state.block(block.as_ref().header());
    let (fixture, policy) = {
        let mut seed = state_block.transaction();
        bootstrap_alice_account(&mut seed);
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("validation-fee", "universal")
                .expect("validation-fee fixture domain"),
            "exact_due_ds"
                .parse()
                .expect("validation-fee fixture asset name"),
        );
        Register::asset_definition(AssetDefinition::new(
            asset_definition_id.clone(),
            "exact-due validation fee DS".to_owned(),
            NumericSpec::fractional(u32::from(VALIDATION_FEE_DS_SCALE)),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .execute(&ALICE_ID, &mut seed)
        .expect("register exact-scale validation-fee asset");
        let policy = ValidationFeePolicyV1 {
            schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
            network_id: seed.network_id.clone(),
            policy_version: 1,
            previous_policy_hash: None,
            ds_asset_id: asset_definition_id,
            ds_scale: VALIDATION_FEE_DS_SCALE,
            fee: Quantity::zero(),
            treasury_account_id: ALICE_ID.clone(),
            charging_mode: ValidationFeeChargingMode::Disabled,
            effective_from_height: PARLIAMENT_DUE_CERTIFICATE_HEIGHT
                + VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS,
            expires_after_height: None,
            exemption_classes: Vec::new(),
            treasury_payout_binding: None,
        };
        validate_validation_fee_policy_proposal(&policy, &seed)
            .expect("valid exact-due validation-fee policy preflight");
        let fixture = seed_due_parliament_certificate(
            &mut seed,
            ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
                proposal_operator: ALICE_ID.clone(),
                policy: policy.clone(),
                payout_lifecycle_proposal_id: None,
            }),
        );
        seed.apply();
        (fixture, policy)
    };

    let mut execution = state_block.transaction();
    execution.world.internal_event_buf.clear();
    assert_eq!(
        execute_due_parliament_certificate_v1(fixture.governance_attempt_id, &mut execution)
            .expect("execute exact-due validation-fee policy certificate"),
        DueParliamentCertificateExecutionV1::Applied
    );
    let registry = validation_fee_policy_registry(&execution)
        .expect("read validation-fee registry")
        .expect("automatic enactment installs validation-fee registry");
    assert_eq!(registry.registered_policies.len(), 1);
    let entry = registry
        .head()
        .expect("validation-fee registry has its enacted head");
    assert_eq!(entry.policy, policy);
    assert_eq!(entry.payout_lifecycle, None);
    let proposal = execution
        .world
        .governance_proposals
        .get(&fixture.proposal_id)
        .expect("retained validation-fee policy proposal");
    let authorization = validation_fee_parliament_authorization(
        fixture.proposal_id,
        proposal,
        &fixture.certificate,
        PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
    )
    .expect("derive exact typed validation-fee authorization");
    assert_eq!(entry.parliament_authorization, authorization);
    assert_eq!(&authorization.proposal_operator, &*ALICE_ID);
    assert_eq!(authorization.proposal_fingerprint, fixture.proposal_id);
    assert_eq!(
        authorization.governance_certificate_id,
        GovernanceCertificateId::derive_v1(&fixture.certificate)
    );
    assert_eq!(authorization.invariant_error(), None);
    assert_exact_due_parliament_effect_enacted(&execution, &fixture);
}

#[test]
fn parliament_validation_fee_payout_lifecycle_enacts_at_the_exact_due_height() {
    let ds_asset_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "paynet").expect("payout fixture asset domain"),
        "fee_token".parse().expect("payout fixture asset name"),
    );
    crate::validation_fee::tests::with_validation_fee_payout_state_at_height(
        PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
        |execution, deployer, code, code_hash| {
            let mut wrapper = crate::validation_fee::tests::activate_bound_payout_runtime(
                execution,
                deployer,
                code,
                code_hash,
                0,
                ds_asset_id.clone(),
                "exact_due_validation_fee_wrapper",
            );
            let pool = crate::validation_fee::tests::activate_bound_payout_runtime(
                execution,
                deployer,
                code,
                code_hash,
                1,
                ds_asset_id,
                "exact_due_validation_fee_pool",
            );
            wrapper.binding.pool_vault_account_id = pool.binding.treasury_account_id.clone();
            let binding = wrapper.binding;
            assert_eq!(binding.invariant_error(), None);
            validate_validation_fee_payout_lifecycle_runtime_before_effect_install(
                &binding, execution,
            )
            .expect("exact payout topology is vacant before enactment");
            let fixture = seed_due_parliament_certificate(
                execution,
                ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                    proposal_operator: ALICE_ID.clone(),
                    payout_binding: binding.clone(),
                }),
            );
            execution.world.internal_event_buf.clear();

            assert_eq!(
                execute_due_parliament_certificate_v1(fixture.governance_attempt_id, execution,)
                    .expect("execute exact-due validation-fee payout certificate"),
                DueParliamentCertificateExecutionV1::Applied
            );
            validate_validation_fee_payout_lifecycle_runtime(&binding, execution)
                .expect("automatic enactment installs the exact protected payout topology");
            let pool_contract_address = execution
                .world
                .contract_subject_addresses
                .get(&binding.pool_vault_account_id)
                .cloned()
                .expect("bound payout pool contract address");
            for (permission, required_holder, permission_label) in
                validation_fee_runtime_permissions(&binding, &pool_contract_address)
            {
                assert!(
                    execution
                        .world
                        .account_permissions
                        .get(&required_holder)
                        .is_some_and(|permissions| permissions.contains(&permission)),
                    "automatic enactment must install {permission_label}",
                );
            }
            let proposal = execution
                .world
                .governance_proposals
                .get(&fixture.proposal_id)
                .expect("retained validation-fee payout proposal");
            assert!(matches!(
                &proposal.kind,
                ProposalKind::ValidationFeePayoutLifecycle(payload)
                    if payload.payout_binding == binding
            ));
            let authorization = validation_fee_parliament_authorization(
                fixture.proposal_id,
                proposal,
                &fixture.certificate,
                PARLIAMENT_DUE_CERTIFICATE_HEIGHT,
            )
            .expect("derive exact payout-lifecycle Parliament authorization");
            assert_eq!(&authorization.proposal_operator, &*ALICE_ID);
            assert_eq!(authorization.proposal_fingerprint, fixture.proposal_id);
            assert_eq!(
                authorization.governance_certificate_id,
                GovernanceCertificateId::derive_v1(&fixture.certificate)
            );
            assert_eq!(authorization.invariant_error(), None);
            assert_exact_due_parliament_effect_enacted(execution, &fixture);
        },
    );
}
