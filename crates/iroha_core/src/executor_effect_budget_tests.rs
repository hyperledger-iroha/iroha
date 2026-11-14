/// Actual signed root work admits complete instruction groups before dispatch.
mod effect_budget {
    use super::*;
    use crate::executor::Executor;

    const GAS: u64 = 50_000_000;

    fn fixture() -> State {
        state_after_genesis(World::with(
            [],
            [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
            [],
        ))
    }

    fn next_block(state: &State) -> crate::state::StateBlock<'_> {
        state.block(BlockHeader::new(
            core::num::NonZeroU64::new(state.committed_height() as u64 + 1)
                .expect("post-genesis fixture height"),
            state.view().latest_block_hash(),
            None,
            0,
            0,
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
            let mut block = next_block(&state);
            let fragments = block.committed_fragment_count();
            let mut tx =
                block.transaction_for_fastpq_testing(Hash::from(source.hash_as_entrypoint()));
            tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            tx.pipeline.overlay_max_instructions = cap;
            tx.pipeline.overlay_max_bytes = 0;
            let result = Executor::Initial
                .execute_transaction(&mut tx, &ALICE_ID, &source, &mut cache)
                .map_err(crate::execution_attempt::expect_completed_rejection);
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
            let mut block = next_block(&state);
            let fragments = block.committed_fragment_count();
            let mut tx =
                block.transaction_for_fastpq_testing(Hash::from(source.hash_as_entrypoint()));
            tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            tx.pipeline.overlay_max_instructions = 0;
            tx.pipeline.overlay_max_bytes = cap;
            let result = Executor::Initial
                .execute_transaction(&mut tx, &ALICE_ID, &source, &mut cache)
                .map_err(crate::execution_attempt::expect_completed_rejection);
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
        let manifest_signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
        let (program, manifest) = kotodama_lang::compiler::Compiler::new()
            .compile_source_with_manifest(r#"
seiyaku ActualEffectGroups { permission CanInvokeContractEntrypoint;
  kotoage fn first() authorize(CanInvokeContractEntrypoint) {
    ledger::account::set_metadata(account: context::authority(), key: Name::parse("effect_first"), value: Json::parse("true"));
  }
  kotoage fn second() authorize(CanInvokeContractEntrypoint) {
    ledger::account::set_metadata(account: context::authority(), key: Name::parse("effect_second"), value: Json::parse("true"));
  }
  kotoage fn pair() authorize(CanInvokeContractEntrypoint) {
    ledger::account::set_metadata(account: context::authority(), key: Name::parse("effect_first"), value: Json::parse("true"));
    ledger::account::set_metadata(account: context::authority(), key: Name::parse("effect_second"), value: Json::parse("true"));
  }
}
"#).expect("compile genuine effect-producing contract");
        let hash = ivm::contract_code_hash(&program);
        let state = fixture();
        let address =
            ContractAddress::derive(&state.network_id, &ALICE_ID, 161, DataSpaceId::UNIVERSAL)
                .unwrap();
        let mut block = next_block(&state);
        let mut setup = block.transaction();
        setup.world.contract_code.insert(
            iroha_data_model::smart_contract::ContractArtifactId::new(
                address.dataspace_id().unwrap(),
                hash,
            ),
            program.clone(),
        );
        setup.world.contract_manifests.insert(
            iroha_data_model::smart_contract::ContractArtifactId::new(
                address.dataspace_id().unwrap(),
                hash,
            ),
            manifest
                .try_signed(
                    manifest_signing.context(),
                    manifest_signing.max_frame_bytes(),
                    &ALICE_KEYPAIR,
                )
                .expect("sign bounded fixture manifest"),
        );
        setup.world.accounts.insert(
            address.subject_id(),
            iroha_data_model::account::AccountValue::new(
                iroha_data_model::account::AccountDetails::default(),
            ),
        );
        setup.world.contract_instances.insert(address.clone(), hash);
        setup
            .world
            .contract_subject_addresses
            .insert(address.subject_id(), address.clone());
        setup.world.contract_subject_bindings.insert(
            address.clone(),
            crate::smartcontracts::code::ContractSubjectBinding::new_direct(
                &address,
                ALICE_ID.clone(),
            )
            .with_active_code_hash(hash),
        );
        // Contract execution retains its own subject; its authored writes to the caller's
        // account need the exact account-scoped grant even though the caller owns it.
        setup.world.account_permissions.insert(
            address.subject_id(),
            BTreeSet::from([executor_permission::account::CanModifyAccountMetadata {
                account: ALICE_ID.clone(),
            }
            .into()]),
        );
        setup.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("bound contract fixture World setup");
        (state, program, address, hash)
    }

    fn grant_entrypoints(block: &mut crate::state::StateBlock<'_>, address: &ContractAddress) {
        let mut setup = block.transaction();
        let permission: Permission =
            iroha_executor_data_model::permission::smart_contract::CanUseContractPermission {
                contract: address.clone(),
                permission: "CanInvokeContractEntrypoint".parse().unwrap(),
            }
            .into();
        Grant::account_permission(permission, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut setup)
            .unwrap();
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
            let mut block = next_block(&state);
            grant_entrypoints(&mut block, &address);
            let fragments = block.committed_fragment_count();
            let mut tx =
                block.transaction_for_fastpq_testing(Hash::from(source.hash_as_entrypoint()));
            tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            tx.pipeline.overlay_max_instructions = cap;
            tx.pipeline.overlay_max_bytes = 0;
            let result = Executor::Initial
                .execute_transaction(&mut tx, &ALICE_ID, &source, &mut cache)
                .map_err(crate::execution_attempt::expect_completed_rejection);
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
            let mut block = next_block(&state);
            grant_entrypoints(&mut block, &address);
            let fragments = block.committed_fragment_count();
            let mut tx =
                block.transaction_for_fastpq_testing(Hash::from(source.hash_as_entrypoint()));
            tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            tx.pipeline.overlay_max_instructions = cap;
            tx.pipeline.overlay_max_bytes = 0;
            let result = Executor::Initial
                .execute_transaction(&mut tx, &ALICE_ID, &source, &mut cache)
                .map_err(crate::execution_attempt::expect_completed_rejection);
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

    /// Smallest gas cap in `charged..=sufficient` under which `completes` holds.
    ///
    /// Completion is monotone in the cap, `sufficient` is known to complete, and no cap below
    /// the gas a completed run is charged can.
    pub(super) fn minimal_sufficient_gas_cap(
        mut completes: impl FnMut(u64) -> bool,
        charged: u64,
        sufficient: u64,
    ) -> u64 {
        let (mut low, mut high) = (charged, sufficient);
        while low < high {
            let middle = low + (high - low) / 2;
            if completes(middle) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    }

    #[test]
    fn minimal_sufficient_gas_cap_finds_the_first_completing_cap() {
        for boundary in [5, 6, 37, 99, 100] {
            let mut probes = 0;
            assert_eq!(
                minimal_sufficient_gas_cap(
                    |cap| {
                        probes += 1;
                        cap >= boundary
                    },
                    5,
                    100
                ),
                boundary
            );
            assert!(probes <= 7, "bisection over 96 caps");
        }
        assert_eq!(minimal_sufficient_gas_cap(|_| unreachable!(), 9, 9), 9);
    }

    /// Outcome of one quarantine root executed in a disposable transaction of a fresh block.
    struct QuarantineAttempt {
        result: Result<(), ValidationFail>,
        gas_used: u64,
        /// Presence of `effect_authored`, `effect_first` and `effect_second` before rollback.
        effects: [bool; 3],
    }

    const EFFECT_KEYS: [&str; 3] = ["effect_authored", "effect_first", "effect_second"];

    /// Execute `source` under the quarantine gas `cap` and discard the transaction.
    fn attempt_quarantine_root(
        state: &State,
        address: &ContractAddress,
        source: &SignedTransaction,
        cache: &mut IvmCache,
        cap: u64,
    ) -> QuarantineAttempt {
        let mut block = next_block(state);
        grant_entrypoints(&mut block, address);
        let mut tx = block.transaction_for_fastpq_testing(Hash::from(source.hash_as_entrypoint()));
        tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        tx.pipeline.quarantine_tx_max_gas = cap;
        tx.pipeline.overlay_max_instructions = 0;
        tx.pipeline.overlay_max_bytes = 0;
        let result = Executor::Initial
            .execute_transaction(&mut tx, &ALICE_ID, source, cache)
            .map(|_| ())
            .map_err(crate::execution_attempt::expect_completed_rejection);
        assert!(!tx.execution_effect_limit_exceeded());
        let effects = EFFECT_KEYS.map(|key| {
            tx.world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get(key)
                .is_some()
        });
        QuarantineAttempt {
            result,
            gas_used: tx.last_tx_gas_used,
            effects,
        }
    }

    #[test]
    fn signed_mixed_batch_contract_runs_share_one_quarantine_gas_cap() {
        let mut cache = IvmCache::new();
        let authored: InstructionBox = SetKeyValue::account(
            ALICE_ID.clone(),
            "effect_authored".parse().unwrap(),
            Json::new(true),
        )
        .into();
        let authored_gas = isi_gas::meter_instructions(
            std::slice::from_ref(&authored),
            &crate::gas::DEFAULT_ZK_GAS,
        )
        .expect("gas quote must complete for this fixture");
        assert!(authored_gas > 0);
        let (state, _program, address, hash) = contract_fixture();
        let call = |entrypoint: &str| ContractInvocation {
            contract_address: address.clone(),
            expected_code_hash: hash,
            entrypoint: entrypoint.to_owned(),
            arguments: None,
        };
        let quarantined = |executable: Executable| {
            let mut metadata = iroha_model_base::metadata::Metadata::default();
            metadata.insert(
                crate::tx::QUARANTINE_METADATA_KEY.parse().unwrap(),
                Json::new(true),
            );
            TransactionBuilder::new(
                state.network_id,
                ALICE_ID.clone(),
                FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(GAS)),
            )
            .with_metadata(metadata)
            .with_executable(executable)
            .sign(ALICE_KEYPAIR.private_key())
        };
        // The cap is a gas limit: a reserved syscall quote must be affordable before its unused
        // part is refunded, so the smallest sufficient cap can exceed the gas finally charged.
        let [(first_gas, first_minimal), (second_gas, second_minimal)] =
            ["first", "second"].map(|entrypoint| {
                let source = quarantined(Executable::ContractCall(call(entrypoint)));
                let uncapped = attempt_quarantine_root(&state, &address, &source, &mut cache, GAS);
                uncapped.result.unwrap();
                assert!(uncapped.gas_used > 0);
                let minimal = minimal_sufficient_gas_cap(
                    |cap| {
                        attempt_quarantine_root(&state, &address, &source, &mut cache, cap)
                            .result
                            .is_ok()
                    },
                    uncapped.gas_used,
                    GAS,
                );
                assert!(minimal >= uncapped.gas_used);
                (uncapped.gas_used, minimal)
            });
        let source = quarantined(Executable::Batch(
            vec![
                ExecutableBatchItem::Instruction(authored.clone()),
                ExecutableBatchItem::ContractCall(call("first")),
                ExecutableBatchItem::ContractCall(call("second")),
            ]
            .into(),
        ));
        let total = authored_gas + first_gas + second_gas;
        let uncapped = attempt_quarantine_root(&state, &address, &source, &mut cache, GAS);
        uncapped.result.unwrap();
        assert_eq!(
            uncapped.gas_used, total,
            "successful batch adds only the authored native instruction gas to its two VM runs"
        );
        assert_eq!(uncapped.effects, [true; 3]);
        // The quarantine cap narrows the one signed gas limit that the authored instruction and
        // both contract runs of the batch draw from: each run meets what the earlier work left.
        let minimal = minimal_sufficient_gas_cap(
            |cap| {
                attempt_quarantine_root(&state, &address, &source, &mut cache, cap)
                    .result
                    .is_ok()
            },
            total,
            GAS,
        );
        let measured = format!(
            "minimal={minimal} total={total} authored={authored_gas} first=({first_gas}, {first_minimal}) second=({second_gas}, {second_minimal})"
        );
        assert_eq!(
            minimal,
            authored_gas + first_minimal.max(first_gas + second_minimal),
            "{measured}"
        );
        // One allowance, not one per run: the last insufficient cap covers either run alone, and
        // the first run alone after the authored instruction.
        assert!(
            minimal - 1 >= authored_gas + first_minimal.max(second_minimal),
            "{measured}"
        );
        for cap in [minimal, minimal - 1] {
            let completes = cap == minimal;
            let mut block = next_block(&state);
            grant_entrypoints(&mut block, &address);
            let fragments = block.committed_fragment_count();
            let mut tx =
                block.transaction_for_fastpq_testing(Hash::from(source.hash_as_entrypoint()));
            tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            tx.pipeline.quarantine_tx_max_gas = cap;
            tx.pipeline.overlay_max_instructions = 0;
            tx.pipeline.overlay_max_bytes = 0;
            let result = Executor::Initial
                .execute_transaction(&mut tx, &ALICE_ID, &source, &mut cache)
                .map(|_| ())
                .map_err(crate::execution_attempt::expect_completed_rejection);
            assert!(!tx.execution_effect_limit_exceeded());
            let effects = EFFECT_KEYS.map(|key| {
                tx.world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get(key)
                    .is_some()
            });
            // The authored instruction and the first run complete under either cap.
            assert_eq!(effects, [true, true, completes], "{measured}");
            if completes {
                result.unwrap();
                assert_eq!(tx.last_tx_gas_used, total);
                tx.apply();
            } else {
                let error = result.unwrap_err();
                assert!(
                    matches!(&error, ValidationFail::IvmFault(fault)
                        if fault.kind == iroha_data_model::executor::fault::IvmFaultKindV1::OutOfGas),
                    "{error:?}"
                );
                // The second run exhausts the capped allowance after the authored
                // instruction and the first run; its consumed work stays charged.
                assert!(tx.last_tx_gas_used > authored_gas + first_gas);
                assert!(tx.last_tx_gas_used <= cap);
                drop(tx);
            }
            assert_eq!(
                block.committed_fragment_count(),
                fragments + usize::from(completes)
            );
            for key in EFFECT_KEYS {
                assert_eq!(
                    block
                        .world
                        .account(&ALICE_ID)
                        .unwrap()
                        .metadata()
                        .get(key)
                        .is_some(),
                    completes,
                    "rejected Batch rolls back its earlier authored and contract effects"
                );
            }
        }
    }
}
