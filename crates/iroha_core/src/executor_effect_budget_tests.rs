/// Actual signed root work admits complete instruction groups before dispatch.
mod effect_budget {
    use super::*;
    use crate::executor::Executor;

    const GAS: u64 = 50_000_000;

    fn fixture() -> State {
        state_for_testing(World::with(
            [],
            [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
            [],
        ))
    }

    fn writes() -> Vec<InstructionBox> {
        ["effect_first", "effect_second"]
            .into_iter()
            .map(|key| {
                SetKeyValue::account(ALICE_ID.clone(), key.parse().unwrap(), Json::new(true)).into()
            })
            .collect()
    }

    fn signed(state: &State, executable: Executable) -> SignedTransaction {
        TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(GAS)),
        )
        .with_executable(executable)
        .sign(ALICE_KEYPAIR.private_key())
    }

    fn assert_limit(error: &ValidationFail, expected: &str) {
        assert!(
            matches!(error, ValidationFail::NotPermitted(reason) if reason == expected),
            "{error:?}"
        );
    }

    #[test]
    fn plain_instruction_group_checks_exact_count_before_first_effect() {
        let mut cache = IvmCache::new();
        for cap in [0, 2, 1] {
            let state = fixture();
            let source = signed(&state, Executable::Instructions(writes().into()));
            let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
            let fragments = block.committed_fragment_count();
            let mut tx = block.transaction();
            tx.pipeline.overlay_max_instructions = cap;
            tx.pipeline.overlay_max_bytes = 0;
            let result =
                Executor::Initial.execute_transaction(&mut tx, &ALICE_ID, source, &mut cache);
            if cap == 1 {
                assert_limit(
                    &result.unwrap_err(),
                    "overlay exceeds max instructions: 2 > 1",
                );
                assert!(tx.execution_effect_limit_exceeded());
                assert_eq!(
                    tx.last_tx_gas_used, 0,
                    "preflight must precede authored work"
                );
                for key in ["effect_first", "effect_second"] {
                    assert!(
                        tx.world
                            .account(&ALICE_ID)
                            .unwrap()
                            .metadata()
                            .get(key)
                            .is_none()
                    );
                }
                drop(tx);
                assert_eq!(block.committed_fragment_count(), fragments);
            } else {
                result.unwrap();
                assert!(!tx.execution_effect_limit_exceeded());
                tx.apply();
                assert_eq!(block.committed_fragment_count(), fragments + 1);
            }
            for key in ["effect_first", "effect_second"] {
                assert_eq!(
                    block
                        .world
                        .account(&ALICE_ID)
                        .unwrap()
                        .metadata()
                        .get(key)
                        .is_some(),
                    cap != 1
                );
            }
        }
    }

    #[test]
    fn plain_instruction_group_checks_exact_bare_bytes_before_first_effect() {
        let instructions = writes();
        let exact: u64 = instructions
            .iter()
            .map(|instruction| u64::try_from(instruction.encode().len()).unwrap())
            .sum();
        assert!(exact > 1);
        let mut cache = IvmCache::new();
        for cap in [0, exact, exact - 1] {
            let state = fixture();
            let source = signed(
                &state,
                Executable::Instructions(instructions.clone().into()),
            );
            let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
            let fragments = block.committed_fragment_count();
            let mut tx = block.transaction();
            tx.pipeline.overlay_max_instructions = 0;
            tx.pipeline.overlay_max_bytes = cap;
            let result =
                Executor::Initial.execute_transaction(&mut tx, &ALICE_ID, source, &mut cache);
            if cap == exact - 1 {
                assert_limit(
                    &result.unwrap_err(),
                    &format!("overlay exceeds max bytes: more than {cap}"),
                );
                assert!(tx.execution_effect_limit_exceeded());
                assert_eq!(tx.last_tx_gas_used, 0);
                for key in ["effect_first", "effect_second"] {
                    assert!(
                        tx.world
                            .account(&ALICE_ID)
                            .unwrap()
                            .metadata()
                            .get(key)
                            .is_none()
                    );
                }
                drop(tx);
                assert_eq!(block.committed_fragment_count(), fragments);
            } else {
                result.unwrap();
                assert!(!tx.execution_effect_limit_exceeded());
                tx.apply();
                assert_eq!(block.committed_fragment_count(), fragments + 1);
            }
            for key in ["effect_first", "effect_second"] {
                assert_eq!(
                    block
                        .world
                        .account(&ALICE_ID)
                        .unwrap()
                        .metadata()
                        .get(key)
                        .is_some(),
                    cap != exact - 1
                );
            }
        }
    }

    fn contract_fixture() -> (State, Vec<u8>, ContractAddress, Hash) {
        let (program, manifest) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(r#"
seiyaku ActualEffectGroups {
  kotoage fn first() authorize("CanInvokeContractEntrypoint") {
    ledger::account::set_detail(account: context::authority(), key: Name::parse("effect_first"), value: Json::parse("true"));
  }
  kotoage fn second() authorize("CanInvokeContractEntrypoint") {
    ledger::account::set_detail(account: context::authority(), key: Name::parse("effect_second"), value: Json::parse("true"));
  }
  kotoage fn pair() authorize("CanInvokeContractEntrypoint") {
    ledger::account::set_detail(account: context::authority(), key: Name::parse("effect_first"), value: Json::parse("true"));
    ledger::account::set_detail(account: context::authority(), key: Name::parse("effect_second"), value: Json::parse("true"));
  }
}
"#).expect("compile genuine effect-producing contract");
        let hash = ivm::contract_code_hash(&program);
        let address = ContractAddress::derive(
            &executor_test_network_id(b"actual-effect-admission-161"),
            &ALICE_ID,
            161,
            DataSpaceId::UNIVERSAL,
        )
        .unwrap();
        let mut world = World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
        world.contract_code.insert(hash, program.clone());
        world
            .contract_manifests
            .insert(hash, manifest.signed(&ALICE_KEYPAIR));
        bind_executor_test_contract(&mut world, &address, &ALICE_ID, hash);
        (state_for_testing(world), program, address, hash)
    }

    fn grant_entrypoints(block: &mut crate::state::StateBlock<'_>, address: &ContractAddress) {
        let mut setup = block.transaction();
        for entrypoint in ["first", "second", "pair"] {
            let permission: Permission = iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
                contract: address.clone(),
                entrypoint: entrypoint.to_owned(),
            }.into();
            Grant::account_permission(permission, ALICE_ID.clone())
                .execute(&ALICE_ID, &mut setup)
                .unwrap();
        }
        setup.apply();
    }

    #[test]
    fn bound_raw_artifact_group_rejects_before_first_effect() {
        // The same cache is reused across fresh signed attempts and worlds.
        let mut cache = IvmCache::new();
        let mut successful_gas = None;
        for cap in [2, 1] {
            let (state, program, address, hash) = contract_fixture();
            let mut metadata = Metadata::default();
            for (key, value) in [
                ("contract_address", address.to_string()),
                ("contract_code_hash", hash.to_string()),
                ("contract_entrypoint", "pair".to_owned()),
            ] {
                metadata.insert(key.parse().unwrap(), Json::new(value));
            }
            let source = TransactionBuilder::new(
                state.network_id,
                ALICE_ID.clone(),
                FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(GAS)),
            )
            .with_metadata(metadata)
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
            .sign(ALICE_KEYPAIR.private_key());
            let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
            grant_entrypoints(&mut block, &address);
            let fragments = block.committed_fragment_count();
            let mut tx = block.transaction();
            tx.pipeline.overlay_max_instructions = cap;
            tx.pipeline.overlay_max_bytes = 0;
            let result =
                Executor::Initial.execute_transaction(&mut tx, &ALICE_ID, source, &mut cache);
            assert!(
                tx.last_tx_gas_used > 0,
                "actual VM must finish before artifact admission"
            );
            if cap == 1 {
                assert_limit(
                    &result.unwrap_err(),
                    "overlay exceeds max instructions: 2 > 1",
                );
                assert!(tx.execution_effect_limit_exceeded());
                assert_eq!(Some(tx.last_tx_gas_used), successful_gas);
                for key in ["effect_first", "effect_second"] {
                    assert!(
                        tx.world
                            .account(&ALICE_ID)
                            .unwrap()
                            .metadata()
                            .get(key)
                            .is_none()
                    );
                }
                drop(tx);
                assert_eq!(block.committed_fragment_count(), fragments);
            } else {
                result.unwrap();
                assert!(!tx.execution_effect_limit_exceeded());
                successful_gas = Some(tx.last_tx_gas_used);
                tx.apply();
                assert_eq!(block.committed_fragment_count(), fragments + 1);
            }
            for key in ["effect_first", "effect_second"] {
                assert_eq!(
                    block
                        .world
                        .account(&ALICE_ID)
                        .unwrap()
                        .metadata()
                        .get(key)
                        .is_some(),
                    cap == 2
                );
            }
        }
    }

    #[test]
    fn mixed_batch_cumulative_artifact_denial_precedes_second_group_effect() {
        let mut cache = IvmCache::new();
        let mut successful_gas = None;
        for cap in [3, 2] {
            let (state, _program, address, hash) = contract_fixture();
            let mut items = vec![ExecutableBatchItem::Instruction(
                SetKeyValue::account(
                    ALICE_ID.clone(),
                    "effect_authored".parse().unwrap(),
                    Json::new(true),
                )
                .into(),
            )];
            items.extend(["first", "second"].into_iter().map(|entrypoint| {
                ExecutableBatchItem::ContractCall(ContractInvocation {
                    contract_address: address.clone(),
                    expected_code_hash: hash,
                    entrypoint: entrypoint.to_owned(),
                    arguments: None,
                })
            }));
            let source = signed(&state, Executable::Batch(items.into()));
            let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
            grant_entrypoints(&mut block, &address);
            let fragments = block.committed_fragment_count();
            let mut tx = block.transaction();
            tx.pipeline.overlay_max_instructions = cap;
            tx.pipeline.overlay_max_bytes = 0;
            let result =
                Executor::Initial.execute_transaction(&mut tx, &ALICE_ID, source, &mut cache);
            assert!(tx.last_tx_gas_used > 0);
            assert!(
                tx.world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get("effect_authored")
                    .is_some()
            );
            assert!(
                tx.world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get("effect_first")
                    .is_some(),
                "first admitted group executes before the next contract runs"
            );
            if cap == 2 {
                assert_limit(
                    &result.unwrap_err(),
                    "overlay exceeds max instructions: 3 > 2",
                );
                assert!(tx.execution_effect_limit_exceeded());
                assert_eq!(
                    Some(tx.last_tx_gas_used),
                    successful_gas,
                    "both actual VMs complete even when the second artifact group cannot apply"
                );
                assert!(
                    tx.world
                        .account(&ALICE_ID)
                        .unwrap()
                        .metadata()
                        .get("effect_second")
                        .is_none(),
                    "cumulative denial must precede the second group's first effect"
                );
                drop(tx);
                assert_eq!(block.committed_fragment_count(), fragments);
            } else {
                result.unwrap();
                assert!(!tx.execution_effect_limit_exceeded());
                successful_gas = Some(tx.last_tx_gas_used);
                tx.apply();
                assert_eq!(block.committed_fragment_count(), fragments + 1);
            }
            for key in ["effect_authored", "effect_first", "effect_second"] {
                assert_eq!(
                    block
                        .world
                        .account(&ALICE_ID)
                        .unwrap()
                        .metadata()
                        .get(key)
                        .is_some(),
                    cap == 3,
                    "discarding a rejected mixed batch rolls back its earlier admitted group"
                );
            }
        }
    }

    #[test]
    fn signed_mixed_batch_contract_runs_share_one_exact_cycle_allowance() {
        let mut cache = IvmCache::new();
        let mut segment_cycles = Vec::new();
        let mut segment_gas = Vec::new();
        let mut successful_batch_gas = None;
        for case in 0..5 {
            let (state, _program, address, hash) = contract_fixture();
            let call = |entrypoint: &str| ContractInvocation {
                contract_address: address.clone(), expected_code_hash: hash,
                entrypoint: entrypoint.to_owned(), arguments: None,
            };
            let executable = if case < 2 {
                Executable::ContractCall(call(if case == 0 { "first" } else { "second" }))
            } else {
                Executable::Batch(vec![
                    ExecutableBatchItem::Instruction(SetKeyValue::account(
                        ALICE_ID.clone(), "effect_authored".parse().unwrap(), Json::new(true),
                    ).into()),
                    ExecutableBatchItem::ContractCall(call("first")),
                    ExecutableBatchItem::ContractCall(call("second")),
                ].into())
            };
            let total: u64 = segment_cycles.iter().sum();
            let cap = match case { 0..=2 => 1_000_000, 3 => total, _ => total - 1 };
            let mut metadata = iroha_model_base::metadata::Metadata::default();
            metadata.insert(crate::tx::QUARANTINE_METADATA_KEY.parse().unwrap(), Json::new(true));
            let source = TransactionBuilder::new(
                state.network_id, ALICE_ID.clone(),
                FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(GAS)),
            ).with_metadata(metadata).with_executable(executable).sign(ALICE_KEYPAIR.private_key());
            let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
            grant_entrypoints(&mut block, &address);
            let fragments = block.committed_fragment_count();
            let mut tx = block.transaction();
            tx.pipeline.quarantine_tx_max_cycles = cap;
            tx.pipeline.overlay_max_instructions = 0;
            tx.pipeline.overlay_max_bytes = 0;
            let result = Executor::Initial.execute_transaction(&mut tx, &ALICE_ID, source, &mut cache);
            let cycles = tx.completed_execution_cycles_for_tests().unwrap();
            assert!(!tx.execution_effect_limit_exceeded());
            if case < 2 {
                result.unwrap();
                assert!(cycles > 1);
                segment_cycles.push(cycles);
                segment_gas.push(tx.last_tx_gas_used);
                drop(tx);
                continue;
            }
            assert!(tx.world.account(&ALICE_ID).unwrap().metadata().get("effect_authored").is_some());
            assert!(tx.world.account(&ALICE_ID).unwrap().metadata().get("effect_first").is_some());
            if case == 4 {
                assert_limit(&result.unwrap_err(), &format!("quarantine cycle budget exceeded: {cap}"));
                assert_eq!(cycles, cap);
                assert!(tx.last_tx_gas_used > segment_gas[0]);
                assert!(Some(tx.last_tx_gas_used) < successful_batch_gas);
                assert!(tx.world.account(&ALICE_ID).unwrap().metadata().get("effect_second").is_none());
                assert!(!tx.execution_effects_allow_apply());
                drop(tx);
            } else {
                result.unwrap();
                assert_eq!(cycles, total, "Batch shares precisely the two actual VM runs");
                if case == 2 { successful_batch_gas = Some(tx.last_tx_gas_used); }
                assert_eq!(Some(tx.last_tx_gas_used), successful_batch_gas);
                tx.apply();
            }
            assert_eq!(block.committed_fragment_count(), fragments + usize::from(case != 4));
            for key in ["effect_authored", "effect_first", "effect_second"] {
                assert_eq!(block.world.account(&ALICE_ID).unwrap().metadata().get(key).is_some(), case != 4,
                    "rejected Batch rolls back its earlier authored and contract effects");
            }
        }
    }
}
