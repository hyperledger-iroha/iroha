// Actual finalized admission + native 3-of-4 Decisions feed disposable economics.
// These tests do not publish the scratch overlay or acknowledge reducer Apply.

#[derive(Clone, Copy)]
enum NativeEconomicCase {
    Transfer(u32),
    RegisterAssetDefinition,
    BadSignature,
    ExpiredAtAdmission,
    MissingRuntimeGas,
    Reveal(u8),
    RevealExpired,
    OrderedReveal(u32),
}

struct NativeEconomicFixture {
    native: LaneContextVerifiedFixture,
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

fn native_economic_fixture_with_world_initializer(
    cases: &[NativeEconomicCase],
    atomic_group: bool,
    genesis_layout: Option<DataAvailabilityLayout>,
    direct_fee: Option<NativeEconomicDirectFee>,
    initialize_world: impl FnOnce(&mut World),
) -> Box<NativeEconomicFixture> {
    use iroha_data_model::transaction::signed::{
        SealedTransactionCommitmentPayload, SignedSealedTransactionCommitment,
    };
    assert!(!cases.is_empty() && cases.len() <= 2);
    assert!(!atomic_group || cases.len() == 1);
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
    let mut state = State::try_new_with_chain_and_network_id_with_default_telemetry(
        world,
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        (*DEFAULT_TEST_CHAIN_ID).clone(),
        NetworkId::from_genesis_hash(genesis.hash()),
    )
    .expect("construct State before authenticating its configured primary geometry");
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
    configure_commit_topology_preserving_world_peers(&state, 1);
    // The fixture starts with live asset definitions before executing genesis
    // instructions. Seed their real genesis incarnations while the parent history
    // is still empty, using the production finalizer at this exact header.
    // The generic metadata helper publishes the hash before commit, so it cannot
    // retrospectively supply this genesis-only authority.
    state
        .block(genesis.header())
        .commit_world_overlay_for_testing()
        .expect("seed actual genesis asset incarnations before metadata publication");
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
    let fee_intent = direct_fee.map_or_else(
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
    );
    for (index, case) in cases.iter().enumerate() {
        let mut builder =
            TransactionBuilder::new(state.network_id, source_account.clone(), fee_intent.clone());
        builder.set_creation_time(Duration::from_millis(1 + index as u64));
        builder.set_ttl(Duration::from_secs(1));
        let amount = match case {
            NativeEconomicCase::Transfer(amount) | NativeEconomicCase::OrderedReveal(amount) => {
                *amount
            }
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
            } else {
                vec![
                    Transfer::asset_quantity(source.clone(), amount, destination_account.clone())
                        .into(),
                ]
            };
        let mut transaction = builder
            .with_instructions(instructions)
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            )
            .sign(source_key.private_key());
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
        let (binding, control) = queue_plan_admission_certificate_for_entrypoint_state_test(
            &state,
            plan,
            &validators,
            parent.header().height().get(),
            0x81 + index as u8,
            &entrypoint,
        );
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
    let opening = if let Some(layout) = genesis_layout {
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
            );
            kura.store_v2_finality_artifact(&artifact).unwrap();
            previous = Some(artifact);
        }
        let previous = previous.unwrap();
        crate::sumeragi::v2_context::build_successor_height_context(
            &previous,
            previous.height_context.nexus_amx_context_hash,
            None,
        )
        .unwrap()
    } else {
        lane_opening_context_for_state_test(&state)
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
    let before=crate::snapshot::canonical_state_snapshot_hash(state);
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
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state),before,"successful scratch execution is not global publication");
    let (repeated,again)=state.preexecute_lane_decision_groups(header,&groups).unwrap();
    assert_eq!(again[0].result,executions[0].result);assert_eq!(again[0].settlement_commitment,executions[0].settlement_commitment);assert_eq!(again[0].fastpq_transcripts,executions[0].fastpq_transcripts);
    drop(repeated);assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state),before);
}

state_test! { sync native_economic_executor_rejection_settles_exact_heads_without_charging_or_mutating_assets
    for case in [NativeEconomicCase::Transfer(101),NativeEconomicCase::BadSignature,NativeEconomicCase::ExpiredAtAdmission,NativeEconomicCase::MissingRuntimeGas,NativeEconomicCase::RevealExpired] {
        let fixture=native_economic_fixture(&[case],true);let state=&fixture.native.state;let groups=native_economic_groups(&fixture);
        let before=crate::snapshot::canonical_state_snapshot_hash(state);let header=empty_global_block_after(Some(&fixture.native.block)).header();
        let (overlay,executions)=state.preexecute_lane_decision_groups(header,&groups).unwrap();
        assert!(executions[0].result.is_err());assert_native_economic_terminal(&overlay,&groups[0],header.height().get());
        assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0,Quantity::from(100u32));assert!(overlay.world.assets.get(&fixture.destination).is_none());
        assert!(executions[0].authenticated_signed_replay_alias.is_none());
        assert!(executions[0].settlement_commitment.receipts.is_empty());assert!(executions[0].settlement_commitment.nexus_fee_receipts.is_empty());assert!(executions[0].fastpq_transcripts.is_empty());
        if !matches!(case,NativeEconomicCase::Transfer(_)|NativeEconomicCase::RevealExpired) {assert_eq!(overlay.gas_used_in_block,0,"non-executable input never invokes the stateful fee/execution path");}
        drop(overlay);assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state),before);
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
    set_native_economic_gas_limit(state,maximum);let before=crate::snapshot::canonical_state_snapshot_hash(state);
    let header=empty_global_block_after(Some(&fixture.native.block)).header();
    assert!(matches!(state.preexecute_lane_decision_groups(header,&groups),Err(MergeLedgerCommitError::ExecutionBatchFull{fitting_prefix:1,..})));
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state),before,"aggregate refusal commits no economics/pending/frontier changes");
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
    let before=crate::snapshot::canonical_state_snapshot_hash(state);
    let error=match state.preexecute_lane_decision_groups(empty_global_block_after(Some(&fixture.native.block)).header(),&groups) {Err(error)=>error,Ok(_)=>panic!("duplicate sealed commitment must fail before economics")};
    assert!(error.to_string().contains("repeats a sealed commitment"),"{error}");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state),before);
}

state_test! { sync native_economic_executor_sealed_alias_uses_authenticated_preblock_record_after_removal
    let fixture=native_economic_fixture(&[NativeEconomicCase::Reveal(0)],true);
    let state=&fixture.native.state;let groups=native_economic_groups(&fixture);
    let entrypoint=&groups[0].body().payload().input.entrypoint;
    let header=empty_global_block_after(Some(&fixture.native.block)).header();
    let before=crate::snapshot::canonical_state_snapshot_hash(state);
    let (overlay,outputs)=state.preexecute_lane_decision_groups(header,&groups).unwrap();
    assert!(outputs[0].result.is_ok(),"{:?}",outputs[0].result);
    let alias=crate::tx::exact_signed_transaction_hash(entrypoint).unwrap();
    assert_eq!(outputs[0].authenticated_signed_replay_alias,Some(Hash::from(alias)));
    assert_eq!(crate::tx::authenticated_signed_replay_alias(&overlay,entrypoint).map(Hash::from),Some(Hash::from(alias)),"successful removal does not erase pre-block authentication");
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0,Quantity::from(75u32));
    assert_native_economic_terminal(&overlay,&groups[0],header.height().get());
    drop(overlay);assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state),before);
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
    let before=crate::snapshot::canonical_state_snapshot_hash(state);
    let (overlay,outputs)=state.preexecute_lane_decision_groups(header,&groups).unwrap();
    assert!(outputs[0].result.is_err(),"newer 40-unit reveal must execute after earlier 80-unit reveal");
    assert!(outputs[1].result.is_ok(),"earlier commitment wins the actual insufficient-balance conflict");
    assert_eq!(outputs[0].source,groups[0].to_wire());assert_eq!(outputs[1].source,groups[1].to_wire());
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0,Quantity::from(20u32));
    assert_eq!(overlay.world.assets.get(&fixture.destination).unwrap().0,Quantity::from(80u32));
    for group in &groups {assert_native_economic_terminal(&overlay,group,header.height().get());}
    assert!(outputs.iter().all(|output|output.authenticated_signed_replay_alias.is_some()),"both exact commitments authenticate aliases even when stateful execution rejects one");
    drop(overlay);assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state),before);
}
