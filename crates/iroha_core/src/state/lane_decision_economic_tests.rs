// Actual finalized admission + native 3-of-4 Decisions feed disposable economics.
// These tests do not publish the scratch overlay or acknowledge reducer Apply.

#[derive(Clone, Copy)]
enum NativeEconomicCase {
    Transfer(u32),
    TransferAfterParent(u32, u64),
    AtomicBatchTransfer,
    IndependentBatchTransfer,
    RegisterAssetDefinition,
    RuntimeEffect(AutonomousRuntimeEffectFixture),
    CallbackTransfer,
    BadSignature,
    ExpiredAtAdmission,
    MissingRuntimeGas,
    Reveal(u8),
    RevealExpired,
    OrderedReveal(u32),
}

struct NativeEconomicFixture {
    native: LaneContextVerifiedFixture<Box<State>>,
    source: AssetId,
    destination: AssetId,
}

fn native_economic_fixture(
    cases: &[NativeEconomicCase],
    atomic_group: bool,
) -> Box<NativeEconomicFixture> {
    native_economic_fixture_with_genesis_layout(cases, atomic_group, None)
}

fn native_economic_fixture_with_genesis_layout(
    cases: &[NativeEconomicCase],
    atomic_group: bool,
    genesis_layout: Option<DataAvailabilityLayout>,
) -> Box<NativeEconomicFixture> {
    native_economic_fixture_with_fee_policy(cases, atomic_group, genesis_layout, None)
}

fn native_economic_fixture_with_fee_policy(
    cases: &[NativeEconomicCase],
    atomic_group: bool,
    genesis_layout: Option<DataAvailabilityLayout>,
    direct_fee: Option<NativeEconomicDirectFee>,
) -> Box<NativeEconomicFixture> {
    native_economic_fixture_with_world_initializer(
        cases,
        atomic_group,
        genesis_layout,
        direct_fee,
        |_| {},
    )
}

// Keep the World/State construction frame separate from every nested block
// acquisition. A lexical scope alone does not reduce debug-build stack frames.
// The returned Box is the original State; no snapshot, clone or replay is used.
struct NativeEconomicStateSetup {
    state: Box<State>,
    kura: Arc<Kura>,
    genesis: SignedBlock,
    nexus: iroha_config::parameters::actual::Nexus,
    configured_catalog_hash: Hash,
    source_key: KeyPair,
    source_account: AccountId,
    destination_account: AccountId,
    source: AssetId,
    destination: AssetId,
    fee_asset: AssetId,
    // Fixture input consumed before transaction signing and first admission.
    fee_intent: Option<iroha_data_model::transaction::FeePaymentIntent>,
    // Real governed policy instructions, executed before first admission.
    genesis_instructions: Vec<InstructionBox>,
}

// Do not inline the large constructor into a caller that acquires WorldBlock.
#[inline(never)]
fn native_economic_state_setup(
    direct_fee: Option<NativeEconomicDirectFee>,
    initialize_world: impl FnOnce(&mut World),
) -> NativeEconomicStateSetup {
    let source_key = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
    let destination_key = KeyPair::try_from_seed(vec![0x72; 32], Algorithm::Ed25519).unwrap();
    let source_account = AccountId::new(source_key.public_key().clone());
    let destination_account = AccountId::new(destination_key.public_key().clone());
    let domain_id = DomainId::try_new("native-economics", "universal").unwrap();
    let definition_id =
        AssetDefinitionId::derive_from_components(domain_id.clone(), "coin".parse().unwrap());
    let source = AssetId::new(definition_id.clone(), source_account.clone());
    let destination = AssetId::new(definition_id.clone(), destination_account.clone());
    let mut definition = AssetDefinition::numeric(
        definition_id,
        "native coin",
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&source_account);
    definition.total_quantity = Quantity::from(100u32);
    let mut domains = vec![Domain::new(domain_id).build(&source_account)];
    let mut definitions = vec![definition];
    let mut balances = vec![Asset::new(source.clone(), 100u32)];
    let fee_asset = native_economic_direct_fee_asset(&source);
    if let Some(policy) = direct_fee {
        let fee_domain = DomainId::parse_fully_qualified("universal.universal").unwrap();
        domains.push(Domain::new(fee_domain.clone()).build(&source_account));
        let mut fee_definition = AssetDefinition::numeric(
            fee_asset.definition().clone(),
            "native fee XOR",
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(fee_domain),
        )
        .build(&source_account);
        fee_definition.total_quantity = Quantity::from(policy.funding);
        definitions.push(fee_definition);
        balances.push(Asset::new(fee_asset.clone(), policy.funding));
    }
    let mut world = World::with_assets(
        domains,
        [
            Account::new(source_account.clone()).build(&source_account),
            Account::new(destination_account.clone()).build(&destination_account),
        ],
        definitions,
        balances,
        [],
    );
    initialize_world(&mut world);
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    // These balance/order/rollback controls use an explicit zero-charge policy,
    // as the existing autonomous transfer fixture does. Empty signed fee limits
    // are valid only because the actual configured charge is zero.
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    if direct_fee.is_some() {
        // Configure the real policy before genesis, admission and native signing.
        // The complete input authorizes this asset and exact maximum in its
        // signed intent; execution still runs every production fee check.
        nexus.fees.settlement_mode =
            iroha_config::parameters::actual::NexusFeeSettlementMode::Direct;
        nexus.fees.fee_asset_id = fee_asset.definition().canonical_address();
        nexus.fees.base_fee = Quantity::from(2u32);
        nexus.fees.per_instruction_fee = Quantity::from(3u32);
    }
    nexus.lane_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(1),
                alias: "economic-secondary".into(),
                ..LaneConfig::default()
            },
        ],
    )
    .unwrap();
    // Establish the real immutable startup baseline before any durable block.
    // Historical replay then reinstalls static policy through the same guarded
    // boundary without inventing catalog authority after genesis.
    nexus.configured_lane_catalog = nexus.lane_catalog.clone();
    let configured_catalog_hash = iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(
        &nexus.configured_lane_catalog,
    );
    let genesis = empty_global_block_after(None);
    let kura = Kura::new_temporary_with_configured_lane_catalog(
        &strict_kura_config_for_testing(std::path::PathBuf::new()),
        &iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.configured_lane_catalog),
        &nexus.configured_lane_catalog,
    )
    .expect("authenticate the actual configured catalog before opening fixture State");
    let state = Box::new(
        State::try_new_with_chain_and_network_id_with_default_telemetry(
            world,
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            (*DEFAULT_TEST_CHAIN_ID).clone(),
            NetworkId::from_genesis_hash(genesis.hash()),
        )
        .expect("construct State before authenticating its configured primary geometry"),
    );
    NativeEconomicStateSetup {
        state,
        kura,
        genesis,
        nexus,
        configured_catalog_hash,
        source_key,
        source_account,
        destination_account,
        source,
        destination,
        fee_asset,
        fee_intent: None,
        genesis_instructions: Vec::new(),
    }
}

fn native_economic_fixture_with_world_initializer(
    cases: &[NativeEconomicCase],
    atomic_group: bool,
    genesis_layout: Option<DataAvailabilityLayout>,
    direct_fee: Option<NativeEconomicDirectFee>,
    initialize_world: impl FnOnce(&mut World),
) -> Box<NativeEconomicFixture> {
    native_economic_fixture_with_initializers(
        cases,
        atomic_group,
        genesis_layout,
        direct_fee,
        initialize_world,
        |_, _| {},
    )
}

fn native_economic_fixture_with_initializers(
    cases: &[NativeEconomicCase],
    atomic_group: bool,
    genesis_layout: Option<DataAvailabilityLayout>,
    direct_fee: Option<NativeEconomicDirectFee>,
    initialize_world: impl FnOnce(&mut World),
    before_admission: impl FnOnce(&State, &SignedBlock),
) -> Box<NativeEconomicFixture> {
    let mut setup = native_economic_state_setup(direct_fee, initialize_world);
    if cases
        .iter()
        .any(|case| matches!(case, NativeEconomicCase::RuntimeEffect(_)))
    {
        setup.nexus.staking.restricted_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;
    }
    native_economic_fixture_from_state_with_initializer(
        cases,
        atomic_group,
        genesis_layout,
        direct_fee,
        setup,
        before_admission,
    )
}

fn native_economic_fixture_from_state(
    cases: &[NativeEconomicCase],
    atomic_group: bool,
    genesis_layout: Option<DataAvailabilityLayout>,
    direct_fee: Option<NativeEconomicDirectFee>,
    setup: NativeEconomicStateSetup,
) -> Box<NativeEconomicFixture> {
    native_economic_fixture_from_state_with_initializer(
        cases,
        atomic_group,
        genesis_layout,
        direct_fee,
        setup,
        |_, _| {},
    )
}

// Keep policy execution and its transaction frame out of the large admission
// constructor. Every instruction uses the actual ISI implementation, including
// permission checks and canonical authority-policy history creation.
#[inline(never)]
fn native_economic_commit_genesis_overlay(
    state: &State,
    genesis: &SignedBlock,
    instructions: Vec<InstructionBox>,
) {
    use crate::smartcontracts::Execute as _;

    let mut block = state.block(genesis.header());
    if !instructions.is_empty() {
        let mut transaction = block.transaction();
        for instruction in instructions {
            instruction
                .execute(
                    &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID,
                    &mut transaction,
                )
                .expect("execute governed archive prerequisites before Native admission");
        }
        transaction.apply();
    }
    block
        .commit_world_overlay_for_testing()
        .expect("seed actual genesis policies and asset incarnations before metadata publication");
}

// Only pointer-sized State custody remains live while the actual genesis,
// admission and finality producers acquire their original nested writers.
#[inline(never)]
fn native_economic_fixture_from_state_with_initializer(
    cases: &[NativeEconomicCase],
    atomic_group: bool,
    genesis_layout: Option<DataAvailabilityLayout>,
    direct_fee: Option<NativeEconomicDirectFee>,
    setup: NativeEconomicStateSetup,
    before_admission: impl FnOnce(&State, &SignedBlock),
) -> Box<NativeEconomicFixture> {
    use iroha_data_model::transaction::signed::{
        SealedTransactionCommitmentPayload, SignedSealedTransactionCommitment,
    };
    assert!(!cases.is_empty() && cases.len() <= 2);
    assert!(!atomic_group || cases.len() == 1);
    let NativeEconomicStateSetup {
        mut state,
        kura,
        genesis,
        nexus,
        configured_catalog_hash,
        source_key,
        source_account,
        destination_account,
        source,
        destination,
        fee_asset,
        fee_intent,
        genesis_instructions,
    } = setup;
    // Convenience State fixtures install default markers eagerly. Follow the
    // actual startup order here, then apply only the usual test runtime limits.
    state.configure_test_runtime_defaults();
    state.install_pre_genesis_nexus_for_testing(nexus);
    assert_eq!(
        kura.configured_lane_catalog_baseline().unwrap(),
        Some(configured_catalog_hash),
    );
    let (ids, validators) = bls_accounts_in("validators", 4);
    seed_consensus_keys_with_pops(&state, &validators);
    install_lane_manifest_registry(
        &state,
        &[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL, ids.clone()),
            (LaneId::new(1), DataSpaceId::UNIVERSAL, ids),
        ],
    );
    if cases
        .iter()
        .any(|case| matches!(case, NativeEconomicCase::RuntimeEffect(_)))
    {
        install_native_runtime_startup_registry(&state, &validators);
    }
    // The State prefix and its frozen global finality must use the same exact
    // four-validator roster. A random one-member metadata topology cannot be
    // authenticated as the first context of a restored snapshot.
    let mut global_validators = (0xD3_u8..=0xD6)
        .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    global_validators.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    set_commit_topology_from_keypairs(&state, &global_validators);
    seed_consensus_keys_with_pops(&state, &global_validators);
    // The fixture starts with live asset definitions before executing genesis
    // instructions. Seed their real genesis incarnations while the parent history
    // is still empty, using the production finalizer at this exact header.
    // The generic metadata helper publishes the hash before commit, so it cannot
    // retrospectively supply this genesis-only authority.
    native_economic_commit_genesis_overlay(&state, &genesis, genesis_instructions);
    let expected_incarnation = AxtAssetIncarnationV1::derive(
        &state.network_id,
        source.definition(),
        &genesis.hash(),
        &Hash::new_from_chunks(&[
            b"iroha:axt:genesis-asset-incarnation:v1\0",
            genesis.hash().as_ref(),
        ]),
        u64::try_from(
            state
                .world
                .asset_definitions
                .view()
                .iter()
                .position(|(id, _)| id == source.definition())
                .unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        state
            .world
            .axt_asset_incarnations
            .view()
            .get(source.definition()),
        Some(&expected_incarnation),
        "the economic source uses its actual network/header genesis token",
    );
    if direct_fee.is_some() {
        let expected_fee_incarnation = AxtAssetIncarnationV1::derive(
            &state.network_id,
            fee_asset.definition(),
            &genesis.hash(),
            &Hash::new_from_chunks(&[
                b"iroha:axt:genesis-asset-incarnation:v1\0",
                genesis.hash().as_ref(),
            ]),
            u64::try_from(
                state
                    .world
                    .asset_definitions
                    .view()
                    .iter()
                    .position(|(id, _)| id == fee_asset.definition())
                    .unwrap(),
            )
            .unwrap(),
        );
        assert_eq!(
            state
                .world
                .axt_asset_incarnations
                .view()
                .get(fee_asset.definition()),
            Some(&expected_fee_incarnation),
            "the funded fee asset uses its actual genesis incarnation too",
        );
    }
    kura.store_block(Arc::new(genesis.clone())).unwrap();
    commit_block_metadata_with_genesis_checkpoint_to_state(&state, &genesis);
    let parent = advance_queue_plan_fixture_to_beacon_parent(&state, genesis);
    let primary = crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let secondary = crate::queue::RoutingDecision::new(LaneId::new(1), DataSpaceId::UNIVERSAL);
    let mut controls = Vec::new();
    let mut first_binding = None;
    let mut shared_commitment = None;
    let mut first_ordered_reveal = None;
    let mut first_timed_entrypoint = None;
    let fee_intent = fee_intent.unwrap_or_else(|| {
        direct_fee.map_or_else(
            || iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            |policy| {
                iroha_data_model::transaction::FeePaymentIntent::authority(
                    vec![iroha_data_model::transaction::FeeChargeLimit::new(
                        iroha_data_model::transaction::FeeChargeKind::Nexus,
                        fee_asset.definition().clone(),
                        Quantity::from(policy.signed_max),
                    )],
                    None,
                )
            },
        )
    });
    for (index, case) in cases.iter().enumerate() {
        let mut builder =
            TransactionBuilder::new(state.network_id, source_account.clone(), fee_intent.clone());
        let creation_time_ms = match case {
            NativeEconomicCase::TransferAfterParent(_, milliseconds) => parent
                .header()
                .creation_time_ms
                .checked_add(*milliseconds)
                .unwrap(),
            _ => 1 + index as u64,
        };
        builder.set_creation_time(Duration::from_millis(creation_time_ms));
        builder.set_ttl(Duration::from_secs(1));
        let amount = match case {
            NativeEconomicCase::Transfer(amount)
            | NativeEconomicCase::TransferAfterParent(amount, _)
            | NativeEconomicCase::OrderedReveal(amount) => *amount,
            NativeEconomicCase::Reveal(variant) => 25 + u32::from(*variant),
            _ => 25,
        };
        let instructions: Vec<iroha_data_model::isi::InstructionBox> =
            if matches!(case, NativeEconomicCase::RegisterAssetDefinition) {
                let domain = DomainId::try_new("native-economics", "universal").unwrap();
                let id = AssetDefinitionId::derive_from_components(
                    domain.clone(),
                    "registered".parse().unwrap(),
                );
                vec![
                    Register::asset_definition(AssetDefinition::numeric(
                        id,
                        "native registered",
                        iroha_data_model::asset::AssetBalancePolicy::Global,
                        Some(domain),
                    ))
                    .into(),
                ]
            } else if matches!(
                case,
                NativeEconomicCase::AtomicBatchTransfer
                    | NativeEconomicCase::IndependentBatchTransfer
            ) {
                let second_recipient = AccountId::new(
                    KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519)
                        .expect("deterministic native batch second recipient")
                        .public_key()
                        .clone(),
                );
                let entries = vec![
                    TransferAssetBatchEntry::with_leg_id(
                        "autonomous-batch-leg-a",
                        source_account.clone(),
                        destination_account.clone(),
                        source.definition().clone(),
                        3_u32,
                    ),
                    TransferAssetBatchEntry::with_leg_id(
                        "autonomous-batch-leg-b",
                        source_account.clone(),
                        second_recipient,
                        source.definition().clone(),
                        4_u32,
                    ),
                ];
                vec![if matches!(case, NativeEconomicCase::AtomicBatchTransfer) {
                    TransferAssetBatch::new(entries).into()
                } else {
                    TransferAssetBatch::independent(entries).into()
                }]
            } else if matches!(case, NativeEconomicCase::CallbackTransfer) {
                vec![ExecuteTrigger::new("native_sized_callback".parse().unwrap()).into()]
            } else {
                vec![
                    Transfer::asset_quantity(source.clone(), amount, destination_account.clone())
                        .into(),
                ]
            };
        let signed_builder = builder
            .with_instructions(instructions)
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            );
        let mut transaction = signed_builder.clone().sign(source_key.private_key());
        if matches!(case, NativeEconomicCase::TransferAfterParent(..)) {
            if let Some(first) = first_timed_entrypoint {
                transaction = (1..=256)
                    .find_map(|nonce| {
                        let mut builder = signed_builder.clone();
                        builder.set_nonce(std::num::NonZeroU32::new(nonce).unwrap());
                        let signed = builder.sign(source_key.private_key());
                        (first < TransactionEntrypoint::External(signed.clone()).hash())
                            .then_some(signed)
                    })
                    .expect("bounded canonical hash order for the clock-prefix fixture");
            } else {
                // Select a deterministic small first identity before admission;
                // a fixed unsearched hash can leave no greater second identity
                // inside the bounded fixture nonce range.
                transaction = (1..=256)
                    .map(|nonce| {
                        let mut builder = signed_builder.clone();
                        builder.set_nonce(std::num::NonZeroU32::new(nonce).unwrap());
                        builder.sign(source_key.private_key())
                    })
                    .min_by_key(|signed| signed.hash())
                    .unwrap();
                first_timed_entrypoint =
                    Some(TransactionEntrypoint::External(transaction.clone()).hash());
            }
        }
        if matches!(case, NativeEconomicCase::BadSignature) {
            // A well-formed Ed25519 signature over another message reaches the
            // real stateless signature owner; it is not a decoder-shape failure.
            let signature = Signature::try_new(
                source_key.private_key(),
                b"wrong signed transaction preimage",
            )
            .unwrap();
            transaction.set_signature(iroha_data_model::transaction::signed::TransactionSignature(
                iroha_crypto::SignatureOf::from_signature(signature),
            ));
            assert!(transaction.verify_signature().is_err());
        }
        if matches!(
            case,
            NativeEconomicCase::ExpiredAtAdmission | NativeEconomicCase::MissingRuntimeGas
        ) {
            let mut builder = TransactionBuilder::new(
                state.network_id,
                source_account.clone(),
                fee_intent.clone(),
            );
            builder.set_creation_time(Duration::from_millis(1));
            builder.set_ttl(if matches!(case, NativeEconomicCase::ExpiredAtAdmission) {
                Duration::from_millis(1)
            } else {
                Duration::from_secs(1)
            });
            builder = builder.with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            );
            transaction = if matches!(case, NativeEconomicCase::MissingRuntimeGas) {
                builder
                    .with_executable(iroha_data_model::transaction::Executable::Ivm(
                        iroha_data_model::transaction::IvmBytecode::from_compiled(vec![0u8; 16]),
                    ))
                    .sign(source_key.private_key())
            } else {
                builder
                    .with_instructions([Transfer::asset_quantity(
                        source.clone(),
                        25u32,
                        destination_account.clone(),
                    )])
                    .sign(source_key.private_key())
            };
        }
        let entrypoint = match case {
            NativeEconomicCase::RuntimeEffect(effect) => autonomous_runtime_effect_entrypoint(
                &state,
                &validators,
                0x81 + index as u8,
                *effect,
            ),
            NativeEconomicCase::Reveal(_)
            | NativeEconomicCase::RevealExpired
            | NativeEconomicCase::OrderedReveal(_) => {
                let mut salt = [0x79; 32];
                let height = parent.header().height().get();
                let deadline = if matches!(case, NativeEconomicCase::RevealExpired) {
                    height + 1
                } else {
                    height + 100
                };
                if matches!(case, NativeEconomicCase::OrderedReveal(_)) && index == 1 {
                    // Force canonical first-admission hash order to oppose real
                    // prior commitment height order; the outcome detects an
                    // executor that accidentally runs the source vector directly.
                    salt=(0u8..=255).map(|byte|[byte;32]).find(|salt| {
                        let hash=iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(&state.network_id,&transaction,*salt,deadline);
                        let entrypoint=TransactionEntrypoint::SealedReveal(iroha_data_model::transaction::signed::SealedTransactionReveal::new(hash,transaction.clone(),*salt));
                        Some(entrypoint.hash()) < first_ordered_reveal
                    }).expect("bounded deterministic opposite admission order");
                }
                let expected =
                    iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
                        &state.network_id,
                        &transaction,
                        salt,
                        deadline,
                    );
                let commitment = if matches!(case, NativeEconomicCase::OrderedReveal(_)) {
                    expected
                } else {
                    *shared_commitment.get_or_insert(expected)
                };
                if index == 0 || matches!(case, NativeEconomicCase::OrderedReveal(_)) {
                    let signed = SignedSealedTransactionCommitment::sign(
                        SealedTransactionCommitmentPayload {
                            network_id: state.network_id,
                            authority: source_account.clone(),
                            commitment,
                            reveal_after_height: height + 1,
                            reveal_deadline_height: deadline,
                            nonce: None,
                        },
                        source_key.private_key(),
                    );
                    // Seed this fixture's pre-state through the real commitment
                    // executor. This helper deliberately claims no finalized
                    // global carrier for this isolated initial-world mutation.
                    let seed_header =
                        if matches!(case, NativeEconomicCase::OrderedReveal(_)) && index == 0 {
                            BlockHeader::new(
                                NonZeroU64::new(height - 1).unwrap(),
                                None,
                                None,
                                parent.header().creation_time_ms,
                                0,
                            )
                        } else {
                            parent.header()
                        };
                    let mut seed = state.merge_preexecution_block(seed_header);
                    let accepted = crate::tx::AcceptedTransaction::accept_entrypoint_at_time(
                        TransactionEntrypoint::SealedCommitment(signed),
                        &state.network_id,
                        seed.world.parameters().sumeragi().max_clock_drift(),
                        seed.world.parameters().transaction(),
                        seed.crypto.as_ref(),
                        parent.header().creation_time(),
                    )
                    .unwrap();
                    seed.validate_transaction_with_entrypoint_index_and_routing_context(
                        accepted,
                        &mut crate::smartcontracts::ivm::cache::IvmCache::new(),
                        0,
                        primary,
                    )
                    .1
                    .unwrap();
                    seed.commit_world_overlay_for_testing().unwrap();
                }
                let reveal = TransactionEntrypoint::SealedReveal(
                    iroha_data_model::transaction::signed::SealedTransactionReveal::new(
                        commitment,
                        transaction,
                        salt,
                    ),
                );
                if matches!(case, NativeEconomicCase::OrderedReveal(_)) && index == 0 {
                    first_ordered_reveal = Some(reveal.hash());
                }
                reveal
            }
            _ => TransactionEntrypoint::External(transaction),
        };
        let plan = if atomic_group {
            crate::queue::RoutingPlan::native_amx(
                primary,
                vec![
                    crate::queue::RouteLeg::new(primary, crate::queue::RouteLegRole::Participant),
                    crate::queue::RouteLeg::new(secondary, crate::queue::RouteLegRole::Participant),
                ],
            )
        } else {
            crate::queue::RoutingPlan::single(if index == 0 { primary } else { secondary })
        };
        let (mut binding, mut control) = queue_plan_admission_certificate_for_entrypoint_state_test(
            &state,
            plan.clone(),
            &validators,
            parent.header().height().get(),
            0x81 + index as u8,
            &entrypoint,
        );
        if matches!(case, NativeEconomicCase::TransferAfterParent(..)) {
            // Authenticate the actual fresh admission time before the first
            // carrier, instead of changing an already verified input.
            binding = crate::torii_proxy::new_queue_plan_admission_binding(
                &state.network_id,
                &entrypoint,
                &plan,
                binding.admission_context.clone(),
                creation_time_ms,
            )
            .unwrap();
            control = queue_plan_admission_certificate_bytes_for_state_test(
                &entrypoint,
                &binding,
                &validators,
            );
        }
        first_binding.get_or_insert(binding);
        controls.push(control);
    }
    controls.sort_by_key(|control| {
        norito::decode_canonical::<iroha_data_model::block::lane_admission::LaneAdmittedInputV1>(
            control,
        )
        .unwrap()
        .certificate
        .binding
        .registry_key()
    });
    let mut block = empty_global_block_after(Some(&parent));
    let mut execution = block.execution_context().cloned().unwrap_or_default();
    execution.queue_plan_admissions = controls.clone();
    block.set_execution_context(Some(execution));
    // These are admission controls only: their signed complete inputs are not
    // yet Network execution sources. Replacing proposal controls deliberately
    // invalidates any older results, so bind an explicit empty structural result
    // to the final proposal before the admission/finality metadata fixture uses it.
    assert_eq!(block.network_entrypoint_count(), 0);
    block
        .set_execution_outputs(
            Vec::new(),
            0,
            BTreeMap::new(),
            Vec::new(),
            AxtPolicySnapshot::default(),
            Default::default(),
            Vec::new(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
        .expect("admission-only fixture binds its exact empty Network result");
    let carrier_key = merge_carrier_finality_fixture_keypair();
    block
        .replace_signatures(BTreeSet::from([
            iroha_data_model::block::BlockSignature::new(
                0,
                iroha_crypto::SignatureOf::from_hash(carrier_key.private_key(), block.hash()),
            ),
        ]))
        .unwrap();
    block.validate_proposal_commitments().unwrap();
    block.validate_execution_result_structure().unwrap();
    // Install caller-owned parent fixtures before the admission carrier's
    // authenticated witness and finality. The applying batch is prepared only
    // after this exact committed prefix; no certified base is rewritten.
    before_admission(&state, &block);
    let opening = {
        let layout = genesis_layout.unwrap_or(DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        });
        let policy = crate::sumeragi::v2_recovery::committed_execution_policy_hash(&state).unwrap();
        // Choose geometry before any signed finality or frozen lane instance.
        // Native historical recovery tests exercise the real enclosing batch,
        // whose carrier is larger than this helper's original 4 KiB default.
        let mut previous = None;
        for height in 1..=state.committed_height() {
            assert!(kura.v2_finality_artifact(height as u64).unwrap().is_none());
            let committed = kura.get_block(NonZeroUsize::new(height).unwrap()).unwrap();
            let artifact = merge_carrier_finality_artifact_with_genesis_layout(
                &committed,
                previous.as_ref(),
                state.network_id,
                layout,
                policy,
            );
            let receipt = kura.store_v2_finality_artifact(&artifact).unwrap();
            assert_eq!(receipt.height(), artifact.height);
            assert_eq!(receipt.block_hash(), committed.hash());
            previous = Some(artifact);
        }
        let previous = previous.unwrap();
        crate::sumeragi::v2_context::build_successor_height_context(
            &previous,
            previous.height_context.nexus_amx_context_hash,
            None,
        )
        .unwrap()
    };
    let mut overlay = state
        .block_with_queue_plan_admissions(block.header(), &controls)
        .unwrap();
    overlay
        .finalize_lane_consensus_contexts(&block, Some(&opening))
        .unwrap();
    let mut witness = ExecWitness::default();
    overlay
        .capture_lane_consensus_contexts(&mut witness)
        .unwrap();
    overlay
        .stage_autoscale_sample_record_for_count(&block, 0)
        .expect("admission-only fixture retains its exact runtime predecessor");
    overlay.block_hashes.push(block.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &block);
    overlay.commit().unwrap();
    kura.store_block(Arc::new(block.clone())).unwrap();
    let (artifact, receipt) =
        stage_lane_context_fixture_finality(&state, &block, opening.clone(), witness.clone());
    kura.promote_kagemusha_finality_sidecar(&artifact, &receipt)
        .unwrap();
    Box::new(NativeEconomicFixture {
        native: LaneContextVerifiedFixture {
            state,
            validators,
            binding: first_binding.unwrap(),
            block,
            opening,
            witness,
        },
        source,
        destination,
    })
}

fn native_economic_groups(
    fixture: &NativeEconomicFixture,
) -> Vec<super::VerifiedLaneDecisionGroupV1> {
    let native = &fixture.native;
    let state = &native.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let mut seen = BTreeSet::new();
    let mut groups = Vec::new();
    for lane in observed.contexts() {
        if !seen.insert(lane.frozen().admitted_binding_hash) {
            continue;
        }
        let FirstLaneAdmittedInputReadV1::Ready(source) =
            state.first_lane_admitted_input(&observed, lane).unwrap()
        else {
            panic!("first carrier");
        };
        let LaneInputBodyPreparationV1::Ready(body) = state
            .prepare_lane_input_body(&observed, lane, &source)
            .unwrap()
        else {
            panic!("all exact route heads");
        };
        let decisions = body
            .payload()
            .descriptor
            .slots
            .iter()
            .map(|slot| {
                let lane = observed
                    .contexts()
                    .iter()
                    .find(|lane| Hash::from(lane.instance_id().0) == slot.instance_id)
                    .unwrap();
                sign_native_group_decision_for_test(lane, &native.validators, &body, 0, 1)
            })
            .collect::<Vec<_>>();
        let super::LaneDecisionGroupPreparationV1::Ready(group) = state
            .prepare_lane_decision_group(&observed, lane, &source, &decisions)
            .unwrap()
        else {
            panic!("native Commit group");
        };
        groups.push(group);
    }
    groups.sort_by_key(|group| group.body().payload().descriptor.admission_priority);
    groups
}

fn assert_native_economic_terminal(
    overlay: &StateBlock<'_>,
    group: &super::VerifiedLaneDecisionGroupV1,
    actual_height: u64,
) {
    let payload = group.body().payload();
    assert!(
        State::pending_queue_plan_binding_for_execution(
            overlay,
            &payload.input.entrypoint,
            &payload.input.routing_plan().unwrap(),
            actual_height
        )
        .unwrap()
        .is_none()
    );
    for slot in &payload.descriptor.slots {
        assert_eq!(
            State::canonical_merged_lane_frontier_with_anchor_from_world(
                &overlay.world,
                slot.route.lane_id,
                slot.route.dataspace_id,
                slot.lane_incarnation
            )
            .unwrap(),
            (
                slot.lane_height,
                Some(payload.descriptor.canonical_hash().unwrap()),
                actual_height
            )
        );
    }
}

state_test! { sync native_economic_executor_transfers_once_across_shared_roles_and_drops_atomically
    let fixture=native_economic_fixture(&[NativeEconomicCase::Transfer(25)],true);
    let state=&fixture.native.state;let groups=native_economic_groups(&fixture);
    assert_eq!(groups.len(),1);assert_eq!(groups[0].body().payload().descriptor.slots.len(),2);
    let before=crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let header=empty_global_block_after(Some(&fixture.native.block)).header();
    let (overlay,executions)=state.preexecute_lane_decision_groups(header,&groups).unwrap();
    assert_eq!(executions.len(),1);assert!(executions[0].result.is_ok(),"{:?}",executions[0].result);
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0,Quantity::from(75u32));
    assert_eq!(overlay.world.assets.get(&fixture.destination).unwrap().0,Quantity::from(25u32));
    assert_native_economic_terminal(&overlay,&groups[0],header.height().get());
    assert_eq!(executions[0].settlement_commitment.tx_count,1,"coordinator plus participant role cannot execute twice");
    assert!(executions[0].settlement_commitment.native_amx_receipts.is_empty());
    assert_native_fastpq_retained_for_test(&overlay,&executions);
    for transcript in &executions[0].fastpq_transcripts {assert_eq!(transcript.entry_hash,Hash::from(groups[0].body().payload().input.entrypoint.hash()));}
    drop(overlay);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before,"successful scratch execution is not global publication");
    let (repeated,again)=state.preexecute_lane_decision_groups(header,&groups).unwrap();
    assert_eq!(again[0].result,executions[0].result);assert_eq!(again[0].settlement_commitment,executions[0].settlement_commitment);assert_eq!(again[0].fastpq_transcripts,executions[0].fastpq_transcripts);
    drop(repeated);assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before);
}

state_test! { sync native_economic_execution_retains_its_owner_across_manifest_cache_refresh
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let header = empty_global_block_after(Some(&fixture.native.block)).header();
    let (baseline, expected) = state.preexecute_lane_decision_groups(header, &groups).unwrap();
    drop(baseline);
    let generation = state.state_view_generation();
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (overlay, actual) = state.with_native_lane_execution(header, &groups, |overlay, executions| {
        state.install_lane_manifests(&overlay.lane_manifests);
        assert_ne!(state.state_view_generation(), generation,
            "exercise the real cache publication, not a manually changed counter");
        Ok(executions)
    }).expect("cache refresh cannot replace the retained execution owner");
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(&expected) {
        assert_eq!(actual.result, expected.result);
        assert_eq!(actual.settlement_commitment, expected.settlement_commitment);
        assert_eq!(actual.fastpq_transcripts, expected.fastpq_transcripts);
    }
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    assert_eq!(overlay.world.assets.get(&fixture.destination).unwrap().0, Quantity::from(25u32));
    assert_native_economic_terminal(&overlay, &groups[0], header.height().get());
    assert_native_fastpq_retained_for_test(&overlay, &actual);
    drop(overlay);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before,
        "a successful scratch transition still cannot publish global State");
}

state_test! { sync native_economic_executor_rejection_settles_exact_heads_without_charging_or_mutating_assets
    for case in [NativeEconomicCase::Transfer(101),NativeEconomicCase::BadSignature,NativeEconomicCase::ExpiredAtAdmission,NativeEconomicCase::MissingRuntimeGas,NativeEconomicCase::RevealExpired] {
        let fixture=native_economic_fixture(&[case],true);let state=&fixture.native.state;let groups=native_economic_groups(&fixture);
        let before=crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");let header=empty_global_block_after(Some(&fixture.native.block)).header();
        let (overlay,executions)=state.preexecute_lane_decision_groups(header,&groups).unwrap();
        assert!(executions[0].result.is_err());assert_native_economic_terminal(&overlay,&groups[0],header.height().get());
        assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0,Quantity::from(100u32));assert!(overlay.world.assets.get(&fixture.destination).is_none());
        assert!(executions[0].authenticated_signed_replay_alias.is_none());
        assert!(executions[0].settlement_commitment.receipts.is_empty());assert!(executions[0].settlement_commitment.nexus_fee_receipts.is_empty());assert!(executions[0].fastpq_transcripts.is_empty());
        if !matches!(case,NativeEconomicCase::Transfer(_)|NativeEconomicCase::RevealExpired) {assert_eq!(overlay.gas_used_in_block,0,"non-executable input never invokes the stateful fee/execution path");}
        drop(overlay);assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before);
    }
}

fn set_native_economic_gas_limit(state: &State, limit: u64) {
    let mut parameters = state.world.parameters.block();
    parameters.set_parameter(iroha_data_model::parameter::Parameter::Custom(
        iroha_data_model::parameter::CustomParameter::new(
            iroha_data_model::parameter::CustomParameterId::new(
                "ivm_gas_limit_per_block".parse().unwrap(),
            ),
            iroha_primitives::json::Json::new(limit),
        ),
    ));
    parameters.commit();
}

state_test! { sync native_economic_executor_distinguishes_single_oversize_from_aggregate_batch_full
    let fixture=native_economic_fixture(&[NativeEconomicCase::Transfer(25),NativeEconomicCase::Transfer(25)],false);
    let state=&fixture.native.state;let groups=native_economic_groups(&fixture);assert_eq!(groups.len(),2);
    let costs=groups.iter().map(|group|crate::queue::Queue::compute_proposal_gas_cost(&crate::tx::AcceptedTransaction::new_unchecked_entrypoint(std::borrow::Cow::Borrowed(&group.body().payload().input.entrypoint))).unwrap()).collect::<Vec<_>>();
    let maximum=*costs.iter().max().unwrap();assert!(maximum>1);
    // This fixture changes real governed execution policy after admission; the
    // immutable native source remains the same. No signing uses this raw edit.
    set_native_economic_gas_limit(state,maximum);let before=crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let header=empty_global_block_after(Some(&fixture.native.block)).header();
    assert!(matches!(state.preexecute_lane_decision_groups(header,&groups),Err(MergeLedgerCommitError::ExecutionBatchFull{fitting_prefix:1,..})));
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before,"aggregate refusal commits no economics/pending/frontier changes");
    let (prefix,output)=state.preexecute_lane_decision_groups(header,&groups[..1]).unwrap();assert!(output[0].result.is_ok());
    assert_native_economic_terminal(&prefix,&groups[0],header.height().get());
    let later=&groups[1].body().payload().input;
    assert!(State::pending_queue_plan_binding_for_execution(prefix.as_ref(),&later.entrypoint,&later.routing_plan().unwrap(),header.height().get()).unwrap().is_some(),"unselected owner stays pending");drop(prefix);
    set_native_economic_gas_limit(state,1);
    let (rejected,output)=state.preexecute_lane_decision_groups(header,&groups).unwrap();assert!(output.iter().all(|execution|execution.result.is_err()));assert_eq!(rejected.gas_used_in_block,0);
    for group in &groups {assert_native_economic_terminal(&rejected,group,header.height().get());}
}

state_test! { sync native_economic_executor_refuses_duplicate_sealed_commitments_before_any_execution
    let fixture=native_economic_fixture(&[NativeEconomicCase::Reveal(0),NativeEconomicCase::Reveal(1)],false);
    let state=&fixture.native.state;let groups=native_economic_groups(&fixture);
    assert_eq!(groups.len(),2);assert_ne!(groups[0].body().payload().input.entrypoint.hash(),groups[1].body().payload().input.entrypoint.hash());
    let before=crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let error=match state.preexecute_lane_decision_groups(empty_global_block_after(Some(&fixture.native.block)).header(),&groups) {Err(error)=>error,Ok(_)=>panic!("duplicate sealed commitment must fail before economics")};
    assert!(error.to_string().contains("repeats a sealed commitment"),"{error}");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before);
}

state_test! { sync native_economic_executor_sealed_alias_uses_authenticated_preblock_record_after_removal
    let fixture=native_economic_fixture(&[NativeEconomicCase::Reveal(0)],true);
    let state=&fixture.native.state;let groups=native_economic_groups(&fixture);
    let entrypoint=&groups[0].body().payload().input.entrypoint;
    let header=empty_global_block_after(Some(&fixture.native.block)).header();
    let before=crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let (overlay,outputs)=state.preexecute_lane_decision_groups(header,&groups).unwrap();
    assert!(outputs[0].result.is_ok(),"{:?}",outputs[0].result);
    let alias=crate::tx::exact_signed_transaction_hash(entrypoint).unwrap();
    assert_eq!(outputs[0].authenticated_signed_replay_alias,Some(Hash::from(alias)));
    assert_eq!(crate::tx::authenticated_signed_replay_alias(&overlay,entrypoint).map(Hash::from),Some(Hash::from(alias)),"successful removal does not erase pre-block authentication");
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0,Quantity::from(75u32));
    assert_native_economic_terminal(&overlay,&groups[0],header.height().get());
    drop(overlay);assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before);
}

state_test! { sync native_economic_executor_preserves_sealed_commitment_order_and_original_result_slots
    let fixture=native_economic_fixture(&[NativeEconomicCase::OrderedReveal(80),NativeEconomicCase::OrderedReveal(40)],false);
    let state=&fixture.native.state;let groups=native_economic_groups(&fixture);assert_eq!(groups.len(),2);
    let header=empty_global_block_after(Some(&fixture.native.block)).header();
    {
        let overlay=state.merge_preexecution_block(header);
        let keys=groups.iter().map(|group|match &group.body().payload().input.entrypoint {
            TransactionEntrypoint::SealedReveal(reveal)=>crate::tx::sealed_reveal_execution_key(&overlay,reveal),
            _=>panic!("sealed fixture"),
        }).collect::<Vec<_>>();
        assert!(keys[0]>keys[1],"actual earlier commitment is later in canonical admission source order");
    }
    let before=crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let (overlay,outputs)=state.preexecute_lane_decision_groups(header,&groups).unwrap();
    assert!(outputs[0].result.is_err(),"newer 40-unit reveal must execute after earlier 80-unit reveal");
    assert!(outputs[1].result.is_ok(),"earlier commitment wins the actual insufficient-balance conflict");
    assert_eq!(outputs[0].source,groups[0].to_wire());assert_eq!(outputs[1].source,groups[1].to_wire());
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0,Quantity::from(20u32));
    assert_eq!(overlay.world.assets.get(&fixture.destination).unwrap().0,Quantity::from(80u32));
    for group in &groups {assert_native_economic_terminal(&overlay,group,header.height().get());}
    assert!(outputs.iter().all(|output|output.authenticated_signed_replay_alias.is_some()),"both exact commitments authenticate aliases even when stateful execution rejects one");
    drop(overlay);assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before);
}

state_test! { sync native_common_owner_output_limit_rolls_back_callback_but_settles_exact_heads
    use iroha_data_model::block::execution_output::ExecutionOutputV1;
    let fixture = native_economic_fixture_with_world_initializer(
        &[NativeEconomicCase::CallbackTransfer], true, None, None, |world| {
            let source_key = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
            let destination_key = KeyPair::try_from_seed(vec![0x72; 32], Algorithm::Ed25519).unwrap();
            let authority = AccountId::new(source_key.public_key().clone());
            let destination = AccountId::new(destination_key.public_key().clone());
            let domain = DomainId::try_new("native-economics", "universal").unwrap();
            let definition = AssetDefinitionId::derive_from_components(domain, "coin".parse().unwrap());
            let trigger_id: TriggerId = "native_sized_callback".parse().unwrap();
            let mut metadata = iroha_model_base::metadata::Metadata::default();
            metadata.insert("__registered_block_height".parse::<Name>().unwrap(), Json::new(0u64));
            let trigger = Trigger::new(trigger_id.clone(), Action::new(
                vec![
                    InstructionBox::from(Transfer::asset_quantity(AssetId::new(definition, authority.clone()), 25u32, destination)),
                    InstructionBox::from(Log::new(Level::INFO, "x".repeat(64 * 1024))),
                ],
                Repeats::Exactly(1), authority.clone(), ExecuteTriggerEventFilter::new().for_trigger(trigger_id).under_authority(authority),
            ).unwrap().with_metadata(metadata));
            let mut block = world.triggers.block();
            let mut transaction = block.transaction();
            assert!(transaction.add_by_call_trigger(trigger.try_into().unwrap()).unwrap());
            transaction.apply();
            block.commit();
        },
    );
    let state = &fixture.native.state;
    // Exercise an applying output ceiling after the admitted input has already
    // been authenticated. No carrier or result is signed by this fixture edit.
    {
        let mut parameters = state.world.parameters.block();
        let mut policy = parameters.get().block().execution_output();
        policy.max_output_bytes = 16 * 1024;
        policy.validate().unwrap();
        parameters.get_mut().set_parameter(iroha_data_model::parameter::Parameter::Block(
            iroha_data_model::parameter::BlockParameter::ExecutionOutput(policy),
        ));
        parameters.commit();
    }
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let groups = native_economic_groups(&fixture);
    let header = empty_global_block_after(Some(&fixture.native.block)).header();
    let (overlay, executions) = state.preexecute_lane_decision_groups(header, &groups).unwrap();
    let rows = overlay.retained_execution_outputs_for_test().unwrap();
    assert!(matches!(rows, [ExecutionOutputV1::Network(_)]));
    assert!(rows[0].is_output_limit_rejection());
    assert_eq!(executions[0].result, *rows[0].result());
    assert!(rows[0].completions().is_empty());
    assert!(executions[0].fastpq_transcripts.is_empty());
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0, Quantity::from(100u32));
    assert!(overlay.world.assets.get(&fixture.destination).is_none());
    assert!(overlay.world.triggers.by_call_triggers().get(&"native_sized_callback".parse().unwrap()).is_some());
    assert_native_economic_terminal(&overlay, &groups[0], header.height().get());
    assert!(overlay.gas_used_in_block > 0, "actual attempted work is retained despite business rollback");
    drop(overlay);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
}

state_test! { sync native_admission_fixture_binds_final_controls_results_signature_and_body
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let state = &fixture.native.state;
    let block = &fixture.native.block;
    assert_eq!(block.execution_context().unwrap().queue_plan_admissions.len(), 1);
    assert_eq!(block.network_entrypoint_count(), 0, "admission does not execute its input");
    assert!(block.has_results());
    assert!(block.execution_outputs().is_empty());
    block.validate_proposal_commitments().unwrap();
    block.validate_execution_result_structure().unwrap();
    let key = merge_carrier_finality_fixture_keypair();
    block.signatures().next().unwrap().signature().verify_hash(key.public_key(), block.hash()).unwrap();
    let retained = state.kura.get_block(NonZeroUsize::new(block.header().height().get() as usize).unwrap()).unwrap();
    assert_eq!(retained.encode_wire().unwrap(), block.encode_wire().unwrap());
    assert_eq!(state.world.assets.view().get(&fixture.source).unwrap().0, Quantity::from(100u32));
    assert!(state.world.assets.view().get(&fixture.destination).is_none());
    let groups = native_economic_groups(&fixture);
    let input = &groups[0].body().payload().input;
    assert!(State::pending_queue_plan_binding_for_execution(
        &state.view(), &input.entrypoint, &input.routing_plan().unwrap(),
        block.header().height().get() + 1,
    ).unwrap().is_some(), "the exact admitted input remains pending economic execution");
}
