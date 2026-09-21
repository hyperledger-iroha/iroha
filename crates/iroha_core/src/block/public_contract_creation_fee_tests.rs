mod public_contract_creation_fees {
    //! Ordinary public artifact stages require funded, exact fee intents before state mutation.
    use super::*;

    #[test]
    fn public_contract_artifact_stages_pay_fees_without_management_grants() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("fee status lock");
        let (artifact, manifest) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(
                "seiyaku PublicQuote { view fn quote(int count) -> int { return count * 10; } }",
            )
            .expect("valid immutable artifact");
        let code_hash = manifest.code_hash.expect("artifact hash");
        for parallel_apply in [false, true] {
            for (funded, bounded) in [(true, true), (true, false), (false, true)] {
                let (developer, key) = gen_account_in("public-builder");
                let (sink, _) = gen_account_in("fee-sink");
                let domain_id = DomainId::try_new("fees", "universal").expect("domain");
                let fee_id = AssetDefinitionId::derive_from_components(
                    domain_id.clone(),
                    "xor".parse().unwrap(),
                );
                let payer_asset = AssetId::of(fee_id.clone(), developer.clone());
                let sink_asset = AssetId::of(fee_id.clone(), sink.clone());
                let initial = Quantity::from(if funded { 10_u32 } else { 0_u32 });
                let world = test_world_with_assets(
                    [Domain::new(domain_id).build(&developer)],
                    [
                        Account::new(developer.clone()).build(&developer),
                        Account::new(sink.clone()).build(&sink),
                    ],
                    [AssetDefinition::numeric(
                        fee_id.clone(),
                        "xor".to_owned(),
                        iroha_data_model::asset::AssetBalancePolicy::Global,
                        None,
                    )
                    .build(&developer)],
                    [
                        Asset::new(payer_asset.clone(), initial.clone()),
                        Asset::new(sink_asset.clone(), Quantity::zero()),
                    ],
                    [],
                );
                let mut state = State::new_with_chain(
                    world,
                    Kura::blank_kura_for_testing(),
                    LiveQueryStore::start_test(),
                    ChainId::from("public-contract-fees"),
                );
                install_test_lane_manifests(&state);
                let mut pipeline = state.pipeline.clone();
                pipeline.parallel_overlay = true;
                pipeline.parallel_apply = parallel_apply;
                pipeline.workers = 2;
                state.set_pipeline(pipeline);
                let fees = &mut state.nexus.get_mut().fees;
                fees.base_fee = Quantity::from(1_u32);
                fees.per_byte_fee = Quantity::zero();
                fees.per_instruction_fee = Quantity::zero();
                fees.per_gas_unit_fee = Quantity::zero();
                fees.fee_asset_id = fee_id.to_string();
                fees.fee_sink_account_id = sink.to_string();
                let leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
                let genesis: SignedBlock =
                    ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
                        header.set_height(nonzero!(1_u64));
                    })
                    .into();
                finalize_test_genesis_assets(&state, &genesis);
                let address = iroha_data_model::smart_contract::ContractAddress::derive(
                    &state.network_id,
                    &developer,
                    0,
                    DataSpaceId::UNIVERSAL,
                )
                .unwrap();
                let mut metadata = Metadata::default();
                for name in ["contract_address", "gov_contract_address"] {
                    metadata.insert(name.parse().unwrap(), Json::new(address.to_string()));
                }
                let limits = if bounded {
                    vec![iroha_data_model::transaction::FeeChargeLimit::new(
                        iroha_data_model::transaction::FeeChargeKind::Nexus,
                        fee_id,
                        Quantity::from(1_u32),
                    )]
                } else {
                    Vec::new()
                };
                let intent =
                    iroha_data_model::transaction::FeePaymentIntent::authority(limits, None);
                let stages: Vec<Vec<InstructionBox>> = vec![
                    vec![
                        iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk { code_hash, total_size: artifact.len() as u64, chunk_index: 0, chunk_count: 1, chunk: artifact.clone() }.into(),
                        iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload { code_hash, total_size: artifact.len() as u64, chunk_count: 1 }.into(),
                    ],
                    vec![iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode { manifest: manifest.clone().try_signed(&key).unwrap() }.into()],
                ];
                let transactions = stages
                    .into_iter()
                    .enumerate()
                    .map(|(index, instructions)| {
                        let (_, time) =
                            TimeSource::new_mock(Duration::from_millis(10 + index as u64));
                        let tx = TransactionBuilder::new_with_time_source(
                            state.network_id,
                            developer.clone(),
                            &time,
                            intent.clone(),
                        )
                        .with_metadata(metadata.clone())
                        .with_instructions(instructions)
                        .sign(key.private_key());
                        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
                    })
                    .collect();
                let (_, time) = TimeSource::new_mock(Duration::from_millis(20));
                let block = BlockBuilder::new_with_time_source(transactions, time)
                    .chain(1, Some(&genesis))
                    .sign(leader.private_key())
                    .unpack(|_| {});
                let mut state_block = state.block(block.header());
                let valid = block
                    .validate_and_record_transactions(&mut state_block)
                    .unpack(|_| {});
                assert_eq!(valid.as_ref().network_entrypoint_count(), 2);
                assert_eq!(valid.as_ref().execution_outputs().len(), 2);
                let errors = valid
                    .as_ref()
                    .failed_outputs()
                    .map(|(index, error)| format!("{index}: {error:?}"))
                    .collect::<Vec<_>>();
                if funded && bounded {
                    assert!(
                        errors.is_empty(),
                        "ordinary paid artifact creation failed (parallel_apply={parallel_apply}): {errors:?}"
                    );
                    assert_eq!(
                        state_block.world.contract_code().get(&code_hash),
                        Some(&artifact)
                    );
                    assert!(
                        state_block
                            .world
                            .contract_manifests()
                            .get(&code_hash)
                            .is_some()
                    );
                    assert_eq!(
                        state_block.world.assets().get(&payer_asset).unwrap().0,
                        Quantity::from(8_u32)
                    );
                    assert_eq!(
                        state_block
                            .world
                            .asset_total_amount(payer_asset.definition())
                            .unwrap(),
                        Quantity::from(8_u32),
                        "both paid native stages burn one unit of supply"
                    );
                    assert_eq!(
                        state_block.world.assets().get(&sink_asset).unwrap().0,
                        Quantity::zero(),
                        "native Nexus transaction fees are burned, not credited to a sink"
                    );
                } else {
                    assert_eq!(
                        errors.len(),
                        2,
                        "both stages must fail fee admission: {errors:?}"
                    );
                    assert!(state_block.world.contract_code().get(&code_hash).is_none());
                    assert!(
                        state_block
                            .world
                            .contract_manifests()
                            .get(&code_hash)
                            .is_none()
                    );
                    assert!(state_block.world.contract_code_uploads().is_empty());
                    assert_eq!(
                        state_block.world.assets().get(&payer_asset).unwrap().0,
                        initial
                    );
                    assert_eq!(
                        state_block.world.assets().get(&sink_asset).unwrap().0,
                        Quantity::zero()
                    );
                }
            }
        }
    }
}
