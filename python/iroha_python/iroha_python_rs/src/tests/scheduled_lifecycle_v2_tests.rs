//! Certified compact lifecycle consumption through compiled native contract calls.
//! Runtime/artifact setup is explicit fixture setup, not rollout qualification.

use super::*;
use iroha_data_model::{
    HasMetadata,
    block::execution_output::{
        TriggerLifecycleActionCodecV1, TriggerLifecycleChangeV1, TriggerLifecycleEffectsV1,
        TriggerLifecycleOriginV1,
    },
    transaction::{TransactionBuilder, executable::ContractInvocation},
};

struct V2Fixture {
    fixture: Fixture,
    activation: String,
}

impl V2Fixture {
    fn new(reject: bool, touching: bool) -> Self {
        Self::with_recovery(reject, touching, false)
    }

    fn with_recovery(reject: bool, touching: bool, recover: bool) -> Self {
        Self::with_panel(reject, touching, recover, None)
    }

    fn with_panel(
        reject: bool,
        touching: bool,
        recover: bool,
        panel: Option<
            iroha_data_model::parameter::runtime_emergency_panel::RuntimeEmergencyPanelV1,
        >,
    ) -> Self {
        Self::with_runtime(reject, touching, recover, panel, None)
    }

    fn with_runtime(
        reject: bool,
        touching: bool,
        recover: bool,
        panel: Option<
            iroha_data_model::parameter::runtime_emergency_panel::RuntimeEmergencyPanelV1,
        >,
        selection: Option<&iroha_data_model::runtime::definition::RootSelectionV1>,
    ) -> Self {
        Self::with_runtime_progress(reject, touching, recover, panel, selection, None)
    }

    fn with_runtime_progress(
        reject: bool,
        touching: bool,
        recover: bool,
        panel: Option<
            iroha_data_model::parameter::runtime_emergency_panel::RuntimeEmergencyPanelV1,
        >,
        selection: Option<&iroha_data_model::runtime::definition::RootSelectionV1>,
        progress: Option<u32>,
    ) -> Self {
        pyo3::Python::initialize();
        let key = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
        let authority = AccountId::new(key.public_key().clone());
        let world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
        let mut config = TestChainConfig::new(world, 1_000);
        if let Some(panel) = panel {
            // Runtime instructions require the universal root's Parliament
            // module. Author it before genesis policy derivation, together with
            // the emergency panel; never patch a checkpoint's committed State.
            let nexus = config.nexus.get_or_insert_with(Default::default);
            nexus.governance.default_module = Some("parliament".into());
            nexus
                .governance
                .modules
                .entry("parliament".into())
                .or_default()
                .module_type = Some("parliament".into());
            config
                .genesis_parameters
                .push(iroha_data_model::parameter::Parameter::Custom(
                    panel.to_custom_parameter().unwrap(),
                ));
        }
        let mut chain = CertifiedTestChain::start(config).unwrap();
        if let Some(selection) = selection {
            chain.select_runtime_for_testing(1_001, selection);
        } else {
            chain.select_lifecycle_for_testing(1_001);
        }
        let (program, manifest) = kotodama_lang::compiler::Compiler::new()
            .compile_source_with_manifest(
                r#"
seiyaku LifecycleObservation {
    permission Run;
    error enum Failure { Rejected = 1, }
    state StateMap<int, int> Calls;
    kotoage fn unrelated() authorize(Run) { Calls[1] = Calls.get(1).unwrap_or(0) + 1; }
    kotoage fn callback() authorize(Run) { Calls[1] = Calls.get(1).unwrap_or(0) + 10; }
    kotoage fn reject() authorize(Run) { require(false, Failure::Rejected); }
    kotoage fn enable_retry() authorize(Run) { Calls[2] = 1; }
    kotoage fn retry_callback() authorize(Run) {
        require(Calls.get(2).unwrap_or(0) == 1, Failure::Rejected);
        Calls[1] = Calls.get(1).unwrap_or(0) + 10;
    }
    kotoage fn touch() authorize(Run) {
        ledger::trigger::set_enabled(trigger: Name::parse("fixture"), enabled: false);
    }
}
"#,
            )
            .expect("compile actual lifecycle consumer contract");
        let code_hash = ivm::contract_code_hash(&program);
        let address =
            chain.install_lifecycle_contract_for_testing(1_001, &key, 83, program, manifest);
        let call = |entrypoint: &str| ContractInvocation {
            contract_address: address.clone(),
            expected_code_hash: code_hash,
            entrypoint: entrypoint.into(),
            arguments: None,
        };
        chain.commit_at(1_001, vec![]);
        let checkpoint = build_checkpoint(&chain.state().view(), 2).unwrap();
        let registration_ms = chain.committed(2).block_time_ms() + 1;
        let mut action = TriggerAction::new(
            Executable::Batch(
                vec![ExecutableBatchItem::ContractCall(call(if recover {
                    "retry_callback"
                } else if reject {
                    "reject"
                } else {
                    "callback"
                }))]
                .into(),
            ),
            Repeats::Exactly(1),
            authority.clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(
                TimeSchedule::starting_at(Duration::from_millis(registration_ms + 5))
                    .with_period(Duration::from_millis(5)),
            )),
        )
        .unwrap()
        .with_retry_policy(TimeTriggerRetryPolicy {
            max_retries: NonZeroU32::new(1).unwrap(),
            retry_after_ms: NonZeroU64::new(5).unwrap(),
        })
        .unwrap();
        if let Some(attempts) = progress {
            let mut metadata = action.metadata().clone();
            metadata.insert(
                "native_progress_attempts".parse().unwrap(),
                Json::new(attempts),
            );
            action = action.with_metadata(metadata);
        }
        let mut builder = TransactionBuilder::new(
            chain.network_id(),
            authority.clone(),
            FeePaymentIntent::authority(vec![], NonZeroU64::new(1_000_000)),
        );
        builder.set_creation_time(Duration::from_millis(registration_ms - 1));
        let original = builder
            .with_executable_batch([
                ExecutableBatchItem::ContractCall(call("unrelated")),
                ExecutableBatchItem::Instruction(
                    Register::trigger(Trigger::new("fixture".parse().unwrap(), action)).into(),
                ),
            ])
            .sign(key.private_key());
        assert_eq!(
            chain.commit_at(registration_ms, vec![original.clone()]),
            vec![true]
        );
        let activation = chain
            .state()
            .lifecycle_activation_witness_for_testing(&chain.committed(3))
            .unwrap();
        let proof_at = |chain: &CertifiedTestChain, height| {
            let witness: json::Value = json::from_json(
                &chain
                    .state()
                    .lifecycle_activation_witness_for_testing(&chain.committed(height))
                    .unwrap(),
            )
            .unwrap();
            witness["runtime_proof"].clone()
        };
        let mut continuation_proofs = Vec::new();
        let mut builder = TransactionBuilder::new(
            chain.network_id(),
            authority,
            FeePaymentIntent::authority(vec![], NonZeroU64::new(1_000_000)),
        );
        builder.set_creation_time(Duration::from_millis(registration_ms));
        let unrelated = if touching {
            // Native contract effects act as the contract subject, so the
            // trigger owner must delegate this exact metadata permission.
            let permission = iroha_data_model::permission::Permission::new(
                "CanModifyTriggerMetadata".to_owned(),
                iroha_primitives::json::Json::new(norito::json!({"trigger": "fixture"})),
            );
            builder.with_executable_batch([
                ExecutableBatchItem::Instruction(
                    iroha_data_model::isi::Grant::account_permission(
                        permission,
                        address.subject_id(),
                    )
                    .into(),
                ),
                ExecutableBatchItem::ContractCall(call("touch")),
            ])
        } else {
            builder.with_executable(Executable::ContractCall(call("unrelated")))
        }
        .sign(key.private_key());
        let accepted = chain.commit_at(registration_ms + 1, vec![unrelated]);
        assert_eq!(
            accepted,
            vec![true],
            "contract-window transaction: {:?}",
            chain.committed(4).block().execution_outputs()
        );
        continuation_proofs.push(proof_at(&chain, 4));
        chain.commit_at(registration_ms + 6, vec![]);
        continuation_proofs.push(proof_at(&chain, 5));
        if recover {
            let mut builder = TransactionBuilder::new(
                chain.network_id(),
                AccountId::new(key.public_key().clone()),
                FeePaymentIntent::authority(vec![], NonZeroU64::new(1_000_000)),
            );
            builder.set_creation_time(Duration::from_millis(registration_ms + 6));
            let enable = builder
                .with_executable(Executable::ContractCall(call("enable_retry")))
                .sign(key.private_key());
            assert_eq!(
                chain.commit_at(registration_ms + 7, vec![enable]),
                vec![true]
            );
            continuation_proofs.push(proof_at(&chain, 6));
            chain.commit_at(registration_ms + 11, vec![]);
            continuation_proofs.push(proof_at(&chain, 7));
        } else if reject && !touching {
            chain.commit_at(registration_ms + 11, vec![]);
            continuation_proofs.push(proof_at(&chain, 6));
        }
        let mut activation: json::Value = json::from_json(&activation).unwrap();
        activation.as_object_mut().unwrap().insert(
            "continuation_proofs".into(),
            json::to_value(&continuation_proofs).unwrap(),
        );
        let activation = json::to_json(&activation).unwrap();
        let last = if recover {
            7
        } else if reject && !touching {
            6
        } else {
            5
        };
        let proofs = (2..=last)
            .map(|height| build_proof(&chain.state().view(), height).unwrap())
            .collect();
        Self {
            fixture: Fixture {
                chain,
                original,
                checkpoint,
                proofs,
            },
            activation,
        }
    }

    fn verify(&self, activation: Option<&str>, count: usize) -> PyResult<json::Value> {
        let f = &self.fixture;
        // Positive prefix queries retain exactly the branches in that prefix.
        // Never trim caller-mutated witnesses used by the refusal tests.
        let prefix_witness =
            if activation == Some(self.activation.as_str()) && count < f.proofs.len() {
                let mut value: json::Value = json::from_json(&self.activation).unwrap();
                let json::Value::Array(cuts) = value
                    .as_object_mut()
                    .unwrap()
                    .get_mut("continuation_proofs")
                    .unwrap()
                else {
                    unreachable!()
                };
                cuts.truncate(count.saturating_sub(2));
                Some(json::to_json(&value).unwrap())
            } else {
                None
            };
        let activation = prefix_witness.as_deref().or(activation);
        let (projection, _) = authenticate_with_activation(
            &f.original.encode_versioned(),
            "fixture",
            &json::to_json(&f.proofs[..count].to_vec()).unwrap(),
            f.chain.network_id(),
            f.checkpoint.chain_id(),
            &f.checkpoint.encode_canonical().unwrap(),
            activation,
        )?;
        json::from_json(&projection).map_err(refusal)
    }
}

#[test]
fn lifecycle_v2_merged_both_orders_authenticate_registration_and_continuation() {
    let selections = activation::merged_selections_for_testing();
    assert_eq!(selections[0], selections[1]);
    for selection in selections {
        // The production builders derive each order above. Runtime installation is
        // explicit fixture setup; every following contract call, witness and QC is real.
        let f = V2Fixture::with_runtime(false, false, false, None, Some(&selection));
        let pending = f.verify(Some(&f.activation), 3).unwrap();
        assert_eq!(pending["terminal"].as_str(), Some("pending"));
        let result = f
            .verify(Some(&f.activation), f.fixture.proofs.len())
            .unwrap();
        assert_eq!(result["terminal"].as_str(), Some("applied"));
        assert_eq!(result["outcomes"].as_array().unwrap().len(), 1);
        assert_eq!(
            result["outcomes"][0]["output"]["kind"].as_str(),
            Some("time_with_lifecycle")
        );
        let witness: json::Value = json::from_json(&f.activation).unwrap();
        let expected = json::to_value(&selection.id().unwrap()).unwrap();
        assert_eq!(witness["runtime_proof"]["execution_selection"], expected);
        let continuations = witness["continuation_proofs"].as_array().unwrap();
        assert_eq!(continuations.len(), 2);
        for proof in continuations {
            assert_eq!(proof["execution_selection"], expected);
            assert_eq!(proof["post_selection"], expected);
        }
        // A successful composed callback must still refuse a substituted exact
        // standalone implementation, even though its lifecycle closure is identical.
        let catalog = iroha_core::runtime::release_catalog().unwrap();
        let mut forged = witness;
        forged.as_object_mut().unwrap().insert(
            "implementation".into(),
            json::to_value(&catalog.definitions[&catalog.lifecycle.bindings[0].implementation])
                .unwrap(),
        );
        assert!(
            f.verify(
                Some(&json::to_json(&forged).unwrap()),
                f.fixture.proofs.len()
            )
            .is_err()
        );
    }
}

#[test]
fn lifecycle_v2_certified_batch_registration_and_unrelated_contract_are_accepted() {
    let f = V2Fixture::new(false, false);
    let result = f
        .verify(Some(&f.activation), f.fixture.proofs.len())
        .unwrap();
    assert_eq!(result["terminal"].as_str(), Some("applied"));
    let outcomes = result["outcomes"].as_array().unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(
        outcomes[0]["output"]["kind"].as_str(),
        Some("time_with_lifecycle")
    );
    let detail = outcomes[0]["output"]["detail"].as_object().unwrap();
    assert_eq!(
        detail.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["execution", "trigger_lifecycle"]
    );
    assert!(detail["execution"].get("result").is_some());
    let registration = decode_framed_signed_block(&f.fixture.proofs[1].block_wire).unwrap();
    let effect = registration
        .execution_outputs()
        .iter()
        .flat_map(|row| row.require_trigger_lifecycle().unwrap().effects())
        .find(|effect| effect.trigger_id.to_string() == "fixture")
        .unwrap();
    assert_eq!(
        effect.origin,
        TriggerLifecycleOriginV1::DirectNetworkInstruction { item_index: 1 }
    );
    let TriggerLifecycleChangeV1::Insert { after } = &effect.change else {
        panic!("registration Insert")
    };
    assert_eq!(
        after.action.codec,
        TriggerLifecycleActionCodecV1::NoritoCanonicalV1
    );
    assert_eq!(after.action.bytes.len(), 72);
    assert_eq!(after.action.persistent_hash(), after.action_hash);
    let pending = f.verify(Some(&f.activation), 3).unwrap();
    assert_eq!(pending["terminal"].as_str(), Some("pending"));
}

#[test]
fn lifecycle_v2_contract_touch_is_refused_even_when_the_callback_never_fires() {
    let f = V2Fixture::new(false, true);
    let error = f
        .verify(Some(&f.activation), f.fixture.proofs.len())
        .unwrap_err();
    assert!(error.to_string().contains("lifecycle changed"), "{error}");
}

#[test]
fn lifecycle_v2_failed_callbacks_authenticate_retry_then_exhaustion_policy() {
    let f = V2Fixture::new(true, false);
    let retrying = f.verify(Some(&f.activation), 4).unwrap();
    assert_eq!(retrying["terminal"].as_str(), Some("retrying"));
    let result = f
        .verify(Some(&f.activation), f.fixture.proofs.len())
        .unwrap();
    assert_eq!(result["terminal"].as_str(), Some("exhausted"));
    let outcomes = result["outcomes"].as_array().unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(
        outcomes
            .iter()
            .all(|row| row["disposition"].as_str() == Some("rejected"))
    );
    assert!(outcomes.iter().all(|row| {
        row["output"]["detail"]["trigger_lifecycle"]["detail"]["effects"]
            .as_array()
            .is_some_and(|effects| !effects.is_empty())
    }));
}

#[test]
fn lifecycle_v2_requires_registration_cut_activation_and_rejects_substitutions() {
    let f = V2Fixture::new(false, false);
    let count = f.fixture.proofs.len();
    assert!(
        f.verify(None, count)
            .unwrap_err()
            .to_string()
            .contains("activation witness")
    );
    for field in ["selection", "services", "implementation", "runtime_proof"] {
        let mut witness: json::Value = json::from_json(&f.activation).unwrap();
        let object = witness.as_object_mut().unwrap();
        object.insert(field.into(), json::Value::Null);
        assert!(
            f.verify(Some(&json::to_json(&witness).unwrap()), count)
                .is_err(),
            "{field}"
        );
    }
    for mutation in ["missing", "extra", "replayed", "reordered"] {
        let mut witness: json::Value = json::from_json(&f.activation).unwrap();
        let registration_cut = witness["runtime_proof"].clone();
        let json::Value::Array(cuts) = witness
            .as_object_mut()
            .unwrap()
            .get_mut("continuation_proofs")
            .unwrap()
        else {
            unreachable!()
        };
        match mutation {
            "missing" => {
                cuts.pop();
            }
            "extra" => cuts.push(cuts.last().unwrap().clone()),
            "replayed" => cuts[0] = registration_cut,
            "reordered" => cuts.swap(0, 1),
            _ => unreachable!(),
        }
        assert!(
            f.verify(Some(&json::to_json(&witness).unwrap()), count)
                .is_err(),
            "{mutation}"
        );
    }
    let mut witness: json::Value = json::from_json(&f.activation).unwrap();
    witness
        .as_object_mut()
        .unwrap()
        .get_mut("selection")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert(
            "services_profile".into(),
            json::to_value(&iroha_data_model::runtime::definition::DefinitionId(
                [7; 32],
            ))
            .unwrap(),
        );
    assert!(
        f.verify(Some(&json::to_json(&witness).unwrap()), count)
            .is_err()
    );
    let later = f
        .fixture
        .chain
        .state()
        .lifecycle_activation_witness_for_testing(&f.fixture.chain.committed(5))
        .unwrap();
    assert!(
        f.verify(Some(&later), count).is_err(),
        "a later native branch is not original activation evidence"
    );
}

#[test]
fn lifecycle_v2_composed_native_descriptors_still_require_certified_selection_and_root() {
    use iroha_data_model::runtime::definition::{
        DefinitionDescriptorV1, ImplementationCodeV1, RootSelectionV1,
    };
    let f = V2Fixture::new(false, false);
    let count = f.fixture.proofs.len();
    assert_eq!(
        f.verify(Some(&f.activation), count).unwrap()["terminal"].as_str(),
        Some("applied")
    );
    let mut witness: json::Value = json::from_json(&f.activation).unwrap();
    let mut selection: RootSelectionV1 = json::from_value(witness["selection"].clone()).unwrap();
    let mut services: DefinitionDescriptorV1 =
        json::from_value(witness["services"].clone()).unwrap();
    let mut implementation: DefinitionDescriptorV1 =
        json::from_value(witness["implementation"].clone()).unwrap();
    let original = implementation.id().unwrap();
    let DefinitionDescriptorV1::ServicesProfile(profile) = &mut services else {
        unreachable!()
    };
    profile
        .services
        .iter_mut()
        .find(|service| service.name == "authorization")
        .unwrap()
        .reference_closure = Hash::new(b"composed authorization fixture").into();
    selection.services_profile = services.id().unwrap();
    let DefinitionDescriptorV1::Implementation(bundle) = &mut implementation else {
        unreachable!()
    };
    bundle.services_profile = selection.services_profile;
    bundle.code = ImplementationCodeV1::Native {
        module: "iroha.lifecycle.composition.test.v1".into(),
        reference_closure: Hash::new(b"composed native bundle fixture").into(),
    };
    let replacement = implementation.id().unwrap();
    for active in &mut selection.active {
        if *active == original {
            *active = replacement;
        }
    }
    selection.active.sort_unstable();
    for binding in &mut selection.bindings {
        assert_eq!(binding.implementation, original);
        binding.implementation = replacement;
    }
    let object = witness.as_object_mut().unwrap();
    object.insert("selection".into(), json::to_value(&selection).unwrap());
    object.insert("services".into(), json::to_value(&services).unwrap());
    object.insert(
        "implementation".into(),
        json::to_value(&implementation).unwrap(),
    );
    // Coherent joins and the unchanged lifecycle closure do not qualify an
    // arbitrary native descriptor pair.
    let error = f
        .verify(Some(&json::to_json(&witness).unwrap()), count)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("registration did not select the supported lifecycle runtime"),
        "{error}"
    );

    // The real compiled Ironwood+Time pair passes identity recognition, but it
    // cannot replace the Time-only selection authenticated by the original cuts.
    let catalog = iroha_core::runtime::release_catalog().unwrap();
    let selection = activation::merged_selections_for_testing()[0].clone();
    let selected = selection.id().unwrap();
    assert_ne!(selected, catalog.lifecycle_id);
    let mut witness: json::Value = json::from_json(&f.activation).unwrap();
    let object = witness.as_object_mut().unwrap();
    object.insert("selection".into(), json::to_value(&selection).unwrap());
    object.insert(
        "services".into(),
        json::to_value(&catalog.definitions[&selection.services_profile]).unwrap(),
    );
    object.insert(
        "implementation".into(),
        json::to_value(&catalog.definitions[&selection.bindings[0].implementation]).unwrap(),
    );
    let supported = activation::ActivationWitness::parse(Some(&json::to_json(&witness).unwrap()))
        .unwrap()
        .unwrap();
    assert_eq!(supported.selected_identity().unwrap(), selected);
    let error = f
        .verify(Some(&json::to_json(&witness).unwrap()), count)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("runtime selection differs at certified cut"),
        "{error}"
    );
    // Rewriting both claimed selections cannot forge the certified native key.
    let object = witness.as_object_mut().unwrap();
    let proof = object
        .get_mut("runtime_proof")
        .unwrap()
        .as_object_mut()
        .unwrap();
    proof.insert(
        "execution_selection".into(),
        json::to_value(&selected).unwrap(),
    );
    proof.insert("post_selection".into(), json::to_value(&selected).unwrap());
    let json::Value::Array(cuts) = object.get_mut("continuation_proofs").unwrap() else {
        unreachable!()
    };
    for cut in cuts {
        let proof = cut.as_object_mut().unwrap();
        proof.insert(
            "execution_selection".into(),
            json::to_value(&selected).unwrap(),
        );
        proof.insert("post_selection".into(), json::to_value(&selected).unwrap());
    }
    let error = f
        .verify(Some(&json::to_json(&witness).unwrap()), count)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("runtime root differs at certified cut"),
        "{error}"
    );
}

#[test]
fn lifecycle_v2_predecessor_execution_has_no_anchor_and_substitutions_name_the_failed_join() {
    let mut f = V2Fixture::new(false, false);
    let count = f.fixture.proofs.len();
    for (field, expected) in [
        ("height", "runtime height differs at certified cut"),
        (
            "post_selection",
            "runtime selection differs at certified cut",
        ),
        ("siblings", "runtime root differs at certified cut"),
    ] {
        let mut witness: json::Value = json::from_json(&f.activation).unwrap();
        let branch = witness
            .as_object_mut()
            .unwrap()
            .get_mut("runtime_proof")
            .unwrap()
            .as_object_mut()
            .unwrap();
        branch.insert(
            field.into(),
            match field {
                "height" => json::to_value(&999_u64).unwrap(),
                "post_selection" => {
                    json::to_value(&iroha_data_model::runtime::definition::SelectionId([7; 32]))
                        .unwrap()
                }
                _ => json::Value::Array(Vec::new()),
            },
        );
        let error = f
            .verify(Some(&json::to_json(&witness).unwrap()), count)
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{field}: {error}");
    }
    let next = f.fixture.chain.committed(5).block_time_ms() + 1;
    f.fixture
        .chain
        .select_lifecycle_predecessor_for_testing(next);
    f.fixture.chain.commit_at(next, vec![]);
    let certified = build_proof(&f.fixture.chain.state().view(), 6).unwrap();
    let block = decode_framed_signed_block(&certified.block_wire).unwrap();
    assert!(
        block
            .execution_outputs()
            .iter()
            .all(|row| row.trigger_lifecycle().is_none())
    );
    let error = f
        .fixture
        .chain
        .state()
        .lifecycle_activation_witness_for_testing(&f.fixture.chain.committed(6))
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("missing native lifecycle runtime anchor"),
        "{error}"
    );
    f.fixture.proofs.push(certified);
    // A forged same-height branch with the previous selected identity fails the
    // certified ordinary-write root join, independently of height/selection checks.
    let mut witness: json::Value = json::from_json(&f.activation).unwrap();
    let json::Value::Array(cuts) = witness
        .as_object_mut()
        .unwrap()
        .get_mut("continuation_proofs")
        .unwrap()
    else {
        panic!("continuation array")
    };
    let mut forged = cuts.last().unwrap().clone();
    forged
        .as_object_mut()
        .unwrap()
        .insert("height".into(), json::to_value(&6_u64).unwrap());
    cuts.push(forged);
    let error = f
        .verify(
            Some(&json::to_json(&witness).unwrap()),
            f.fixture.proofs.len(),
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("runtime root differs at certified cut"),
        "{error}"
    );
}

#[test]
fn lifecycle_v2_refuses_a_genuine_certified_selection_change() {
    use iroha_data_model::{
        isi::runtime_upgrade::ContainRuntimeV1,
        parameter::runtime_emergency_panel::RuntimeEmergencyPanelV1,
        runtime::lifecycle::{ContainmentKindV1, ContainmentV1, EndorsementV1, RootScopeV1},
        sumeragi_finality::lifecycle_runtime::LifecycleRuntimeProofV1,
    };
    let primary = KeyPair::from_seed(vec![0x71; 32], Algorithm::Ed25519);
    let backup = KeyPair::from_seed(vec![0x72; 32], Algorithm::Ed25519);
    let panel = RuntimeEmergencyPanelV1 {
        primary: vec![primary.public_key().clone()],
        backup: vec![backup.public_key().clone()],
        threshold: 1,
        term_until_ms: 1_000_000,
        min_lease_ms: 1,
        max_lease_ms: 10_000,
    };
    // The panel is committed by signed genesis. This later signed containment
    // changes selection through ordinary native execution and certification.
    let mut f = V2Fixture::with_panel(false, false, false, Some(panel));
    let next = f.fixture.chain.committed(5).block_time_ms() + 1;
    let catalog = iroha_core::runtime::release_catalog().unwrap();
    let frozen = catalog.lifecycle.id().unwrap();
    let target = catalog
        .lifecycle
        .bindings
        .iter()
        .find(|binding| binding.wire_id == "iroha.log")
        .unwrap()
        .interface;
    let containment = ContainmentV1 {
        network: f.fixture.chain.network_id(),
        root: RootScopeV1::Universal,
        incident: [0x73; 32],
        targets: vec![target],
        kind: ContainmentKindV1::AdmissionClosed,
        lease_until_ms: next + 1_000,
    };
    let endorsement = EndorsementV1::sign(
        containment.id().unwrap(),
        next,
        primary.public_key().clone(),
        primary.private_key(),
    )
    .unwrap();
    let key = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
    let tx = f.fixture.chain.sign(
        &key,
        [ContainRuntimeV1 {
            containment,
            endorsements: vec![endorsement],
        }
        .into()],
        next - 1,
    );
    assert_eq!(f.fixture.chain.commit_at(next, vec![tx]), vec![true]);
    let certified = build_proof(&f.fixture.chain.state().view(), 6).unwrap();
    let native: json::Value = json::from_json(
        &f.fixture
            .chain
            .state()
            .lifecycle_activation_witness_for_testing(&f.fixture.chain.committed(6))
            .unwrap(),
    )
    .unwrap();
    let branch: LifecycleRuntimeProofV1 =
        json::from_value(native["runtime_proof"].clone()).unwrap();
    assert_eq!(branch.execution_selection, frozen);
    assert_ne!(
        branch.post_selection, frozen,
        "actual execution changed the final committed selection"
    );
    let decoded = certified
        .decode_checked(iroha_data_model::sumeragi_finality::CertifiedResultScope::TairaGlobal)
        .unwrap();
    assert_eq!(
        branch.computed_root(f.fixture.chain.network_id()).unwrap(),
        decoded.execution().ordinary_writes_root,
        "the genuine unequal selections are certified"
    );
    f.fixture.proofs.push(certified);
    let mut witness: json::Value = json::from_json(&f.activation).unwrap();
    let json::Value::Array(cuts) = witness
        .as_object_mut()
        .unwrap()
        .get_mut("continuation_proofs")
        .unwrap()
    else {
        unreachable!()
    };
    cuts.push(native["runtime_proof"].clone());
    let error = f
        .verify(
            Some(&json::to_json(&witness).unwrap()),
            f.fixture.proofs.len(),
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("runtime selection differs at certified cut"),
        "{error}"
    );
}

#[test]
fn lifecycle_v2_certified_receipt_projection_and_inclusion_keep_the_original_wrapper() {
    use iroha_data_model::query::CommittedTransaction;
    let f = V2Fixture::new(false, false);
    let block = decode_framed_signed_block(&f.fixture.proofs[1].block_wire).unwrap();
    let input_index = block
        .network_entrypoints()
        .position(|input| input.hash() == f.fixture.original.hash_as_entrypoint())
        .unwrap();
    let (output_index, _) = block.network_output_at(input_index as u32).unwrap();
    let output = block.execution_outputs()[output_index as usize].clone();
    assert!(matches!(output, ExecutionOutputV1::NetworkWithLifecycle(_)));
    let committed = CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: block.network_entrypoint_at(input_index).unwrap().hash(),
        entrypoint_proof: block.network_input_proof(input_index as u32).unwrap(),
        entrypoint: block.network_entrypoint_at(input_index).unwrap().clone(),
        output_hash: HashOf::new(&output),
        output_proof: block.output_proof(output_index).unwrap(),
        output,
    };
    use iroha_data_model::query::{
        QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryResponse,
    };
    let response = norito::to_bytes(&QueryResponse::Iterable(QueryOutput {
        batch: QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::CommittedTransaction(
            vec![committed.clone()],
        )),
        remaining_items: Some(0),
        has_more: false,
        continue_cursor: None,
    }))
    .unwrap();
    let proofs_json = json::to_json(&f.fixture.proofs[..2].to_vec()).unwrap();
    let (authenticated, page) =
        crate::committed_transaction_verification::authenticate_committed_transaction(
            committed.entrypoint_hash,
            &response,
            &proofs_json,
            f.fixture.chain.network_id(),
            f.fixture.checkpoint.chain_id(),
            &f.fixture.checkpoint.encode_canonical().unwrap(),
        )
        .unwrap();
    assert_eq!(authenticated, committed);
    assert!(committed.verify_inclusion_in_block(page.tip().block()));
    let projected =
        crate::project_selected_execution_context(page.tip().block(), &committed).unwrap();
    assert_eq!(projected["input_index"].as_u64(), Some(input_index as u64));
    pyo3::Python::attach(|py| {
        let (projection, _) = crate::project_authenticated_committed_transaction(
            py,
            committed.clone(),
            page.clone(),
            &PyNetworkId {
                inner: f.fixture.chain.network_id(),
            },
            753,
        )
        .unwrap();
        let result: json::Value = json::from_json(&projection).unwrap();
        assert_eq!(
            result["output_hash"].as_str(),
            Some(hex_encode(committed.output_hash.as_ref()).as_str())
        );
        assert_eq!(
            result["committed_transaction"]["output"]["kind"].as_str(),
            Some("network_with_lifecycle")
        );
    });
    let mut stripped = committed.clone();
    stripped.output = ExecutionOutputV1::Network(stripped.output.network().unwrap().clone());
    stripped.output_hash = HashOf::new(&stripped.output);
    assert!(!stripped.verify_inclusion_in_block(page.tip().block()));
}

#[test]
fn lifecycle_v2_rejects_wrong_registration_position_mixed_coverage_and_extra_touches() {
    use iroha_data_model::{
        block::execution_output::TriggerLifecycleEffectsV1,
        parameter::execution_output::ExecutionOutputPolicyV1,
    };
    let f = V2Fixture::new(false, false);
    let original = &f.fixture.original;
    let selected = original_trigger(original, &"fixture".parse().unwrap()).unwrap();
    let registration = decode_framed_signed_block(&f.fixture.proofs[1].block_wire).unwrap();
    let registered = (
        registration.header().height().get(),
        registration.header().creation_time_ms,
    );
    for mutation in ["position", "mixed", "hash", "unavailable"] {
        let mut rows = registration.execution_outputs().to_vec();
        let row = rows
            .iter_mut()
            .find(|row| {
                row.network().is_some_and(|network| {
                    registration
                        .network_entrypoint_at(network.input_index as usize)
                        .unwrap()
                        .hash()
                        == original.hash_as_entrypoint()
                })
            })
            .unwrap();
        let ExecutionOutputV1::NetworkWithLifecycle(covered) = row else {
            panic!("V2 registration")
        };
        let TriggerLifecycleEffectsV1::Complete { effects } = &mut covered.trigger_lifecycle;
        let effect = effects
            .iter_mut()
            .find(|effect| effect.trigger_id == *selected.id())
            .unwrap();
        match mutation {
            "position" => {
                effect.origin = TriggerLifecycleOriginV1::DirectNetworkInstruction { item_index: 0 }
            }
            "mixed" => *row = ExecutionOutputV1::Network(covered.execution.clone()),
            "hash" | "unavailable" => {
                let TriggerLifecycleChangeV1::Insert { after } = &mut effect.change else {
                    panic!("Insert")
                };
                if mutation == "hash" {
                    after.action.bytes[0] ^= 1;
                } else {
                    after.action.bytes =
                        iroha_data_model::block::execution_output::lifecycle_unavailable_frame(1)
                            .unwrap()
                            .to_vec();
                    after.registered_at_height = None;
                    after.action_hash = None;
                }
            }
            _ => unreachable!(),
        }
        let mut changed = registration.canonical_resultless_proposal().unwrap();
        let installed = changed.set_execution_outputs(
            rows,
            0,
            Default::default(),
            vec![],
            Default::default(),
            Default::default(),
            &ExecutionOutputPolicyV1::bootstrap().limits(),
        );
        if installed.is_ok() {
            assert!(
                lifecycle_trace::check_block(
                    &changed,
                    original.hash_as_entrypoint(),
                    original,
                    &selected,
                    registered,
                    &mut lifecycle_trace::frame_budget()
                )
                .is_err(),
                "{mutation}"
            );
        }
    }
    let fired = decode_framed_signed_block(&f.fixture.proofs[3].block_wire).unwrap();
    let actual = fired
        .execution_outputs()
        .iter()
        .find(|row| time_row(row).is_some())
        .unwrap();
    for mutation in ["missing", "extra", "origin"] {
        let mut row = actual.clone();
        let ExecutionOutputV1::TimeWithLifecycle(covered) = &mut row else {
            panic!("V2 Time")
        };
        let TriggerLifecycleEffectsV1::Complete { effects } = &mut covered.trigger_lifecycle;
        match mutation {
            "missing" => {
                effects.pop();
            }
            "extra" => {
                effects.push(effects.last().unwrap().clone());
            }
            "origin" => {
                effects[0].origin = TriggerLifecycleOriginV1::Derived {
                    callback_index: None,
                }
            }
            _ => unreachable!(),
        }
        assert!(
            lifecycle_trace::check_occurrence(
                &row,
                &selected,
                registered,
                None,
                fired.header().creation_time_ms,
                "applied",
                None,
            )
            .is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn lifecycle_v2_compact_references_have_a_separate_hash_work_bound() {
    pyo3::Python::initialize();
    let mut work = LifecycleHashWork::new(1024 * 1024);
    for _ in 0..36 {
        work.charge(7).unwrap();
    }
    assert!(work.charge(7).is_err());
    assert!(work.charge(1).is_err());
    let mut overflow = LifecycleHashWork::new(usize::MAX);
    assert!(overflow.charge(7).is_err());
}

#[test]
fn lifecycle_v2_retry_success_clears_only_the_actual_retry_state() {
    let f = V2Fixture::with_recovery(true, false, true);
    let result = f
        .verify(Some(&f.activation), f.fixture.proofs.len())
        .unwrap();
    assert_eq!(result["terminal"].as_str(), Some("applied"));
    let outcomes = result["outcomes"].as_array().unwrap();
    assert_eq!(outcomes.len(), 2);
    assert_eq!(outcomes[0]["disposition"].as_str(), Some("rejected"));
    assert_eq!(outcomes[1]["disposition"].as_str(), Some("applied"));
}

#[test]
fn lifecycle_v2_explicit_rollout_policy_requires_every_certified_row_to_match_height() {
    let f = V2Fixture::new(false, false);
    let fixture = &f.fixture;
    let policy = |height: Option<u64>, network: NetworkId, committed: u64| {
        let active = height.is_some_and(|h| committed >= h);
        let fingerprint = Hash::new(b"independently pinned rollout");
        json::to_json(&norito::json!({
            "network_id": network,
            "v2_activation_height": height,
            "committed_height": committed,
            "active": active,
            "consensus_config_fingerprint": fingerprint,
        }))
        .unwrap()
    };
    let verify = |policy: &str| {
        authenticate_with_activation_policy(
            &fixture.original.encode_versioned(),
            "fixture",
            &json::to_json(&fixture.proofs).unwrap(),
            fixture.chain.network_id(),
            fixture.checkpoint.chain_id(),
            &fixture.checkpoint.encode_canonical().unwrap(),
            Some(&f.activation),
            Some(policy),
        )
    };
    verify(&policy(Some(2), fixture.chain.network_id(), 100)).unwrap();
    for height in [None, Some(4), Some(100)] {
        let error = verify(&policy(height, fixture.chain.network_id(), 100)).unwrap_err();
        assert!(error.to_string().contains("version contradicts"), "{error}");
    }
    let error = verify(&policy(Some(2), fixture.chain.network_id(), 2)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("predates the certified proof tip")
    );
    let mut value: json::Value =
        json::from_json(&policy(Some(2), fixture.chain.network_id(), 100)).unwrap();
    // The fixture's actual canonical NetworkId remains unchanged in every proof.
    let other: NetworkId =
        json::from_json(&json::to_json(&Hash::new(b"another network")).unwrap()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("network_id".into(), json::to_value(&other).unwrap());
    let error = verify(&json::to_json(&value).unwrap()).unwrap_err();
    assert!(error.to_string().contains("belongs to another network"));
}

#[test]
fn lifecycle_v2_authenticates_native_progress_success_retry_and_exhaustion() {
    use iroha_data_model::block::execution_output::{
        TimeExecutionOutputV2, TriggerLifecyclePolicyV1,
    };
    for (reject, recover, terminal) in [
        (false, false, "applied"),
        (true, false, "exhausted"),
        (true, true, "applied"),
    ] {
        let f = V2Fixture::with_runtime_progress(reject, false, recover, None, None, Some(3));
        let result = f
            .verify(Some(&f.activation), f.fixture.proofs.len())
            .unwrap();
        assert_eq!(result["terminal"].as_str(), Some(terminal));
        let original = &f.fixture.original;
        let selected = original_trigger(original, &"fixture".parse().unwrap()).unwrap();
        assert_eq!(
            native_progress_declaration(selected.action()).unwrap(),
            Some(3)
        );
        let registration = decode_framed_signed_block(&f.fixture.proofs[1].block_wire).unwrap();
        let registered = (
            registration.header().height().get(),
            registration.header().creation_time_ms,
        );
        let fired = decode_framed_signed_block(&f.fixture.proofs[3].block_wire).unwrap();
        let actual = fired
            .execution_outputs()
            .iter()
            .find(|row| time_row(row).is_some())
            .unwrap();
        let disposition = scheduled_disposition(actual).unwrap();
        // A guessed counter fails even when the original Action and retry tuple are correct.
        assert!(
            lifecycle_trace::check_occurrence(
                actual,
                &selected,
                registered,
                None,
                fired.header().creation_time_ms,
                disposition,
                Some(2)
            )
            .is_err()
        );
        if !reject {
            // Decoder-only negative controls for a healthy-overflow terminal: retain precisely
            // the real debit effect, without claiming this constructed row has finality.
            let debit = actual
                .require_trigger_lifecycle()
                .unwrap()
                .effects()
                .iter()
                .find(|effect| {
                    matches!(
                        effect.origin,
                        TriggerLifecycleOriginV1::InvocationPolicy {
                            policy: TriggerLifecyclePolicyV1::ConsumeTimeProgress,
                            ..
                        }
                    )
                })
                .unwrap()
                .clone();
            let ExecutionOutputV1::Time(execution) = ExecutionOutputV1::time_output_limit_rejection(
                time_row(actual).unwrap().invocation.clone(),
            ) else {
                unreachable!()
            };
            let mut terminal = ExecutionOutputV1::TimeWithLifecycle(TimeExecutionOutputV2 {
                execution,
                trigger_lifecycle: TriggerLifecycleEffectsV1::Complete {
                    effects: vec![debit],
                },
            });
            lifecycle_trace::check_occurrence(
                &terminal,
                &selected,
                registered,
                None,
                fired.header().creation_time_ms,
                "output_limit",
                Some(3),
            )
            .unwrap();
            let ExecutionOutputV1::TimeWithLifecycle(row) = &mut terminal else {
                unreachable!()
            };
            row.trigger_lifecycle = TriggerLifecycleEffectsV1::Complete { effects: vec![] };
            assert!(
                lifecycle_trace::check_occurrence(
                    &terminal,
                    &selected,
                    registered,
                    None,
                    fired.header().creation_time_ms,
                    "output_limit",
                    Some(3)
                )
                .is_err()
            );
        }
    }
}
