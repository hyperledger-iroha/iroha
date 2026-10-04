//! Complete protocol fixture using the current compiler and ordinary signed native execution.
//! The tiny contract exercises typed native outcomes; it is not a BOI business-model fixture.

use super::*;
use iroha_data_model::{
    isi::{
        Grant, Log, SetAssetHoldingLimit, SetKeyValue, TransferAssetBatchEntry,
        smart_contract_code::{
            CommitContractDeployment, RegisterSmartContractBytes, RegisterSmartContractCode,
        },
    },
    smart_contract::{ContractAddress, ContractAlias},
    transaction::{
        FeePaymentIntent, SignedTransaction, TransactionBuilder, executable::ContractInvocation,
    },
};
use iroha_model_base::{domain::DomainId, state_path::StatePath};
use std::collections::BTreeSet;
use std::time::Duration;

const SOURCE: &str = r#"
seiyaku PrivateCounterFixture {
  state int private_counter_fixture_effect;
  error enum CounterFixtureError {
    WalletLimitExceeded = 1, BelowMinimum = 2, IncomingDisabled = 3,
    NotPermitted = 4, InsufficientBalance = 5,
  }
  hajimari() { private_counter_fixture_effect = 0; }
  kotoage fn run() authorize("CanInvokeContractEntrypoint") {
    private_counter_fixture_effect = 1;
  }
  kotoage fn wallet_limit() authorize("CanInvokeContractEntrypoint") {
    require(false, CounterFixtureError::WalletLimitExceeded);
  }
  kotoage fn below_minimum() authorize("CanInvokeContractEntrypoint") {
    require(false, CounterFixtureError::BelowMinimum);
  }
  kotoage fn incoming_disabled() authorize("CanInvokeContractEntrypoint") {
    require(false, CounterFixtureError::IncomingDisabled);
  }
  kotoage fn not_permitted() authorize("CanInvokeContractEntrypoint") {
    require(false, CounterFixtureError::NotPermitted);
  }
  kotoage fn insufficient_balance() authorize("CanInvokeContractEntrypoint") {
    require(false, CounterFixtureError::InsufficientBalance);
  }
}
"#;

const SUCCESS_SOURCE: &str = r#"
seiyaku PrivateCounterSuccessfulFixture {
  state int private_counter_success_fixture_effect;
  hajimari() { private_counter_success_fixture_effect = 0; }
  kotoage fn run() authorize("CanInvokeContractEntrypoint") {
    private_counter_success_fixture_effect = 1;
  }
}
"#;

/// Complete independently authored expectations and actual sealed native receipt manifest.
pub(crate) struct PublishedPrivateCounterFixture {
    pub(crate) native: PrivateCounterTestChain,
    pub(crate) policy: PrivateCountersPolicyV1,
    pub(crate) manifest: PrivateCountersManifestV1,
}

impl PublishedPrivateCounterFixture {
    pub(crate) fn request(&self, reader: usize) -> SignedPrivateCountersRequestV1 {
        let now = self
            .native
            .chain
            .committed(self.native.chain.height())
            .block_time_ms();
        let mut payload = request_at_tip(&self.native, reader, now).payload;
        payload.purpose = self.policy.purpose;
        payload.policy_hash = self.policy.commitment().unwrap();
        payload.manifest_hash = self.manifest.commitment().unwrap();
        payload.try_sign(&self.native.readers[reader]).unwrap()
    }
}

#[derive(Clone, Copy)]
enum Control {
    Genuine,
    SubstituteLog,
    OmitReceipt,
    WrongErrorBinding,
    WrongSuccessArtifact,
}

/// Shared whole genuine private fixture for installed member-service positive controls.
pub(crate) fn published_interactions_fixture_v1() -> PublishedPrivateCounterFixture {
    published(Control::Genuine)
}

fn original_metadata(binding: &CounterRunBindingV1, action: &CompiledAction) -> Metadata {
    let CounterRunBindingV1::Interactions {
        run_id,
        run_namespace,
        definition_id,
        definition_hash,
        bindings_hash,
    } = binding
    else {
        unreachable!()
    };
    let mut metadata = Metadata::default();
    for (name, value) in [
        ("boiw_v1_run_id", run_id.as_str()),
        ("boiw_v1_namespace", run_namespace.as_str()),
        ("boiw_v1_definition_id", definition_id.as_str()),
        ("boiw_v1_checkpoint_id", action.original_case),
        ("boiw_v1_interaction_id", action.original_action),
        ("boiw_v1_operation_kind", action.operation),
        ("boiw_v1_movement_category", category_name(action.category)),
        (
            "boiw_v1_rejection_category",
            rejection_name(action.rejection),
        ),
        (
            "boiw_v1_psp_scope",
            if action.party == CounterPartyV1::None {
                "operator"
            } else {
                party_name(action.party)
            },
        ),
    ] {
        metadata.insert(name.parse().unwrap(), Json::new(value));
    }
    metadata.insert(
        "boiw_v1_definition_digest".parse().unwrap(),
        Json::new(hex::encode(definition_hash)),
    );
    metadata.insert(
        "boiw_v1_binding_digest".parse().unwrap(),
        Json::new(hex::encode(bindings_hash)),
    );
    metadata
}

fn signed_executable(
    native: &PrivateCounterTestChain,
    executable: Executable,
    metadata: Metadata,
    now: u64,
) -> SignedTransaction {
    let authority = AccountId::new(native.policy_key.public_key().clone());
    let mut builder = TransactionBuilder::new(
        native.chain.network_id(),
        authority,
        FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(100_000)),
    )
    .with_executable(executable)
    .with_metadata(metadata);
    builder.set_creation_time(Duration::from_millis(now));
    let view = native.chain.state().view();
    let draft = builder.clone().sign(native.policy_key.private_key());
    let quote = crate::executor::quote_nexus_fee_admission_draft(
        view.world(),
        view.nexus(),
        view.pipeline(),
        draft.payload(),
        now,
        native.chain.height() + 1,
        Some(native.scope.dataspace_id()),
    )
    .expect("actual private signed executable pays the original finite fee policy");
    builder
        .with_fee_payment_intent(quote.recommended_intent)
        .sign(native.policy_key.private_key())
}

fn selector(rejection: CounterRejectionV1) -> &'static str {
    match rejection {
        CounterRejectionV1::None => "run",
        CounterRejectionV1::WalletLimitExceeded => "wallet_limit",
        CounterRejectionV1::BelowMinimum => "below_minimum",
        CounterRejectionV1::IncomingDisabled => "incoming_disabled",
        CounterRejectionV1::NotPermitted => "not_permitted",
        CounterRejectionV1::InsufficientBalance => "insufficient_balance",
        _ => panic!("not in the selected current interaction plan"),
    }
}

fn published(control: Control) -> PublishedPrivateCounterFixture {
    let (code, manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(SOURCE)
        .expect("current native compiler emits the tiny exact test contract");
    let code_hash = ivm::contract_code_hash(&code);
    assert_eq!(manifest.code_hash, Some(code_hash));
    let artifact_hash = external_artifact_sha256(&code);
    let (success_code, success_manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(SUCCESS_SOURCE)
        .expect("current compiler emits the successful contract without declared errors");
    // Current Kotodama registers these canonical built-in schemas even when the source
    // declares no error enum. Their presence grants no nominal counter-rejection mapping.
    let builtin_errors = vec![
        ivm::error_types::list_error_type(),
        ivm::error_types::numeric_error_type(),
    ];
    assert_eq!(
        success_manifest.error_types.as_deref(),
        Some(builtin_errors.as_slice()),
        "successful source has only the exact compiler-owned built-in error schemas"
    );
    let success_code_hash = ivm::contract_code_hash(&success_code);
    assert_eq!(success_manifest.code_hash, Some(success_code_hash));
    let success_artifact_hash = external_artifact_sha256(&success_code);
    // These nominal identities come from the independently compiled source manifest,
    // before any action is submitted; no rejection receipt supplies an expectation.
    let errors = manifest
        .error_types
        .as_ref()
        .expect("declared and compiler-owned typed errors");
    assert_eq!(errors.len(), builtin_errors.len() + 1);
    for builtin in &builtin_errors {
        assert!(errors.contains(builtin));
    }
    let counter_error = errors
        .iter()
        .find(|error| error.identity == "PrivateCounterFixture::CounterFixtureError")
        .expect("exact source-declared counter error identity")
        .clone();
    assert_eq!(
        counter_error
            .variants
            .iter()
            .map(|variant| (variant.name.as_str(), variant.code))
            .collect::<Vec<_>>(),
        [
            ("WalletLimitExceeded", 1),
            ("BelowMinimum", 2),
            ("IncomingDisabled", 3),
            ("NotPermitted", 4),
            ("InsufficientBalance", 5),
        ],
        "only independently source-declared variants authorize these counter categories"
    );
    let ds = DataSpaceId::new(u64::MAX - 15);
    let key = KeyPair::from_seed(vec![0xCE; 32], Algorithm::Ed25519);
    let authority = AccountId::new(key.public_key().clone());
    let alias_permission = iroha_executor_data_model::permission::account::CanManageAccountAlias {
        scope:
            iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Dataspace(
                ds,
            ),
    };
    let domain_alias_permission =
        iroha_executor_data_model::permission::account::CanManageAccountAlias {
            scope:
                iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Domain(
                    DomainId::try_new("app", "private-counter-test").unwrap(),
                ),
        };
    let artifact_id = ContractArtifactId::new(ds, code_hash);
    let success_artifact_id = ContractArtifactId::new(ds, success_code_hash);
    let mut native = private_chain_with_genesis(vec![
        RegisterSmartContractBytes { artifact_id, code }.into(),
        RegisterSmartContractCode {
            artifact_id,
            manifest: manifest.signed(&key),
        }
        .into(),
        Grant::account_permission(alias_permission, authority.clone()).into(),
        Grant::account_permission(domain_alias_permission, authority.clone()).into(),
        RegisterSmartContractBytes {
            artifact_id: success_artifact_id,
            code: success_code,
        }
        .into(),
        RegisterSmartContractCode {
            artifact_id: success_artifact_id,
            manifest: success_manifest.signed(&key),
        }
        .into(),
    ]);
    let address = ContractAddress::derive(&native.chain.network_id(), &authority, 0, ds).unwrap();
    let deploy = native.chain.sign(
        &key,
        [CommitContractDeployment {
            expected_deploy_nonce: 0,
            contract_address: address.clone(),
            code_hash,
            contract_alias: ContractAlias::from_components(
                "counter-fixture",
                Some("app"),
                "private-counter-test",
            )
            .unwrap(),
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        }
        .into()],
        2_000,
    );
    let success_address =
        ContractAddress::derive(&native.chain.network_id(), &authority, 1, ds).unwrap();
    let success_deploy = native.chain.sign(
        &key,
        [CommitContractDeployment {
            expected_deploy_nonce: 1,
            contract_address: success_address.clone(),
            code_hash: success_code_hash,
            contract_alias: ContractAlias::from_components(
                "counter-success-fixture",
                Some("app"),
                "private-counter-test",
            )
            .unwrap(),
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        }
        .into()],
        2_000,
    );
    let deployments = native.chain.commit_at(2_000, vec![deploy, success_deploy]);
    let deployment_block = native.chain.committed(2);
    let deployment_results: Vec<_> = (0..2)
        .map(|index| {
            deployment_block
                .block()
                .network_output_at(index)
                .expect("original deployment output")
                .1
                .result
                .as_ref()
                .map(|_| ())
        })
        .collect();
    assert_eq!(
        deployments,
        vec![true, true],
        "genuine deployments establish pending hajimari: {deployment_results:?}"
    );
    // Invocation authority is an exact owner-authored token, independent of alias management.
    // Deployment schedules each hook; only the genuine top-level call may complete it.
    let mut invocation_grants = Vec::new();
    for (contract, selectors) in [
        (
            &address,
            &[
                "hajimari",
                "run",
                "wallet_limit",
                "below_minimum",
                "incoming_disabled",
                "not_permitted",
                "insufficient_balance",
            ][..],
        ),
        (&success_address, &["hajimari", "run"][..]),
    ] {
        for selector in selectors {
            invocation_grants.push(
                Grant::account_permission(
                    iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
                        contract: contract.clone(),
                        entrypoint: (*selector).to_owned(),
                    },
                    authority.clone(),
                )
                .into(),
            );
        }
    }
    let grant = native.chain.sign(&key, invocation_grants, 3_000);
    let granted = native.chain.commit_at(3_000, vec![grant]);
    let grant_block = native.chain.committed(native.chain.height());
    let original_grant_result = grant_block
        .block()
        .network_output_at(0)
        .expect("original owner-authored invocation grant output")
        .1
        .result
        .as_ref()
        .map(|_| ());
    assert_eq!(
        granted,
        vec![true],
        "original lifecycle owner grants exact invocation capabilities: {original_grant_result:?}"
    );
    let hooks = [(&address, code_hash), (&success_address, success_code_hash)]
        .into_iter()
        .map(|(contract, expected_code_hash)| {
            signed_executable(
                &native,
                Executable::ContractCall(ContractInvocation {
                    contract_address: contract.clone(),
                    expected_code_hash,
                    entrypoint: "hajimari".into(),
                    arguments: None,
                }),
                Metadata::default(),
                4_000,
            )
        })
        .collect();
    let hook_results = native.chain.commit_at(4_000, hooks);
    let hook_block = native.chain.committed(native.chain.height());
    let original_hook_results: Vec<_> = (0..2)
        .map(|index| {
            hook_block
                .block()
                .network_output_at(index)
                .expect("original hajimari output")
                .1
                .result
                .as_ref()
                .map(|_| ())
        })
        .collect();
    assert_eq!(
        hook_results,
        vec![true, true],
        "genuine pinned hajimari calls execute: {original_hook_results:?}"
    );
    let success_effect_path: StatePath = format!(
        "sc/{}/private_counter_success_fixture_effect",
        hex::encode(Hash::new(success_address.to_string().as_bytes()).as_ref())
    )
    .parse()
    .unwrap();
    let initial_success_effect = native
        .chain
        .state()
        .view()
        .world()
        .smart_contract_state()
        .get(&success_effect_path)
        .expect("actual hajimari initializes address-scoped state")
        .clone();
    let limited = AccountId::new(native.readers[2].public_key().clone());
    let limit = native.chain.sign(
        &native.policy_key,
        [SetAssetHoldingLimit::new(
            limited.clone(),
            native.fee_asset.clone(),
            Some(0_u32.into()),
        )
        .into()],
        5_000,
    );
    let limited_result = native.chain.commit_at(5_000, vec![limit]);
    let limit_block = native.chain.committed(native.chain.height());
    let original_limit_result = limit_block
        .block()
        .network_output_at(0)
        .expect("actual native holding policy output exists")
        .1
        .result
        .as_ref()
        .map(|_| ());
    assert_eq!(
        limited_result,
        vec![true],
        "asset owner commits native holding policy: {original_limit_result:?}"
    );
    let limit_view = native.chain.state().view();
    let limited_account = limit_view.world().account(&limited).unwrap();
    let controls =
        crate::smartcontracts::isi::asset::isi::load_asset_transfer_control_store_from_account(
            &limited,
            limited_account.metadata(),
        )
        .expect("actual committed native holding policy remains canonical");
    assert_eq!(
        controls
            .find(&native.fee_asset)
            .expect("actual fee-asset holding policy is present")
            .holding_limit,
        Some(0_u32.into()),
        "the original asset owner sets the intended zero holding cap"
    );
    drop(limit_view);

    let run_binding = CounterRunBindingV1::Interactions {
        run_id: "current-native-fixture-run".into(),
        run_namespace: "current-native-fixture".into(),
        definition_id: "boi-walkthrough-v1".into(),
        definition_hash: [1; 32],
        bindings_hash: [2; 32],
    };
    let selected: Vec<_> = actions(CounterPurposeV1::WalkthroughInteractions)
        .iter()
        .filter(|action| action.action_id != 12)
        .collect();
    let mut expected_executables = Vec::new();
    let mut authored_executables = Vec::new();
    for action in &selected {
        let executable = if action.independent_batch {
            Executable::Instructions(
                vec![
                    TransferAssetBatch::independent(vec![
                        TransferAssetBatchEntry::with_leg_id(
                            "passing",
                            authority.clone(),
                            AccountId::new(native.readers[1].public_key().clone()),
                            native.fee_asset.clone(),
                            10_u32,
                        ),
                        TransferAssetBatchEntry::with_leg_id(
                            "holding-rejected",
                            authority.clone(),
                            limited.clone(),
                            native.fee_asset.clone(),
                            10_u32,
                        ),
                    ])
                    .into(),
                ]
                .into(),
            )
        } else {
            Executable::ContractCall(ContractInvocation {
                contract_address: if action.result == CounterResultV1::Applied {
                    success_address.clone()
                } else {
                    address.clone()
                },
                expected_code_hash: if action.result == CounterResultV1::Applied {
                    success_code_hash
                } else {
                    code_hash
                },
                entrypoint: selector(action.rejection).into(),
                arguments: None,
            })
        };
        // Capture the authored native executable before even building/submitting the receipt.
        expected_executables.push(CounterExecutableBindingV1 {
            action_id: action.action_id,
            authority: authority.clone(),
            executable_hash: HashOf::<Executable>::try_new(&executable).unwrap(),
        });
        let submitted = if matches!(control, Control::SubstituteLog) && action.action_id == 9 {
            Executable::Instructions(
                vec![
                    Log::new(
                        iroha_logger::Level::DEBUG,
                        "same signed classification, substituted executable".to_owned(),
                    )
                    .into(),
                ]
                .into(),
            )
        } else {
            executable
        };
        authored_executables.push(submitted);
    }
    // Keep every release-owned executable binding before the first action submission.
    // The actual committed genesis source/output policy bounds each complete carrier;
    // a semantic run may span several carriers without changing that immutable capacity.
    let mut authored_executables = authored_executables.into_iter();
    let mut entries = Vec::new();
    let mut action_offset = 0_usize;
    while action_offset < selected.len() {
        let parent = native.chain.committed(native.chain.height());
        let (carrier_capacity, now_ms) = {
            let view = native.chain.state().view();
            let block_parameters = view.world().parameters().block();
            let source_capacity = block_parameters
                .fastpq_source()
                .maximum_network_inputs(block_parameters.execution_output())
                .expect("actual committed source/output capacity is finite and valid");
            let carrier_capacity = usize::try_from(
                u64::from(source_capacity).min(block_parameters.max_transactions().get()),
            )
            .expect("actual carrier capacity fits the fixture host");
            assert!(
                carrier_capacity > 0,
                "original policy admits an action carrier"
            );
            let next_height = native.chain.height().checked_add(1).unwrap();
            let cadence_ms = view
                .world()
                .consensus_schedule()
                .ready(next_height)
                .expect("actual successor schedule owns the action carrier cadence")
                .params
                .block_time_ms;
            let now_ms = parent
                .block_time_ms()
                .checked_add(cadence_ms)
                .expect("original carrier time fits u64")
                .max(6_000);
            (carrier_capacity, now_ms)
        };
        let action_end = action_offset
            .checked_add(carrier_capacity)
            .unwrap()
            .min(selected.len());
        let carrier_actions = &selected[action_offset..action_end];
        let mut transactions = Vec::new();
        let mut receipt_identities = Vec::new();
        for action in carrier_actions {
            // Quote and sign against this carrier's actual pre-state and successor height.
            // Receipt identities belong to these exact signed originals before submission.
            let submitted = authored_executables
                .next()
                .expect("every selected action retains its authored executable");
            let transaction = signed_executable(
                &native,
                submitted,
                original_metadata(&run_binding, action),
                now_ms,
            );
            let entrypoint = TransactionEntrypoint::External(transaction.clone());
            receipt_identities.push((entrypoint.hash(), action.semantic()));
            transactions.push(transaction);
        }
        let results = native.chain.commit_at(now_ms, transactions);
        let action_block = native.chain.committed(native.chain.height());
        assert_eq!(
            action_block.block().network_entrypoint_count(),
            carrier_actions.len(),
            "the original carrier contains exactly its submitted action inputs"
        );
        let original_action_results: Vec<_> = (0..u32::try_from(carrier_actions.len()).unwrap())
            .map(|index| {
                action_block
                    .block()
                    .network_output_at(index)
                    .expect("original selected action output")
                    .1
                    .result
                    .as_ref()
                    .map(|_| ())
            })
            .collect();
        assert_eq!(
            results,
            carrier_actions
                .iter()
                .map(|action| action.result == CounterResultV1::Applied)
                .collect::<Vec<_>>(),
            "only genuine current runtime outcomes establish terminal statuses: {original_action_results:?}"
        );
        for (index, (entrypoint_hash, semantic)) in receipt_identities.into_iter().enumerate() {
            let original_input = action_block
                .block()
                .network_entrypoints()
                .nth(index)
                .expect("actual carrier retains the original selected input");
            assert_eq!(
                original_input.hash(),
                entrypoint_hash,
                "manifest identity matches the actual committed signed original"
            );
            entries.push(CounterManifestEntryV1 {
                entrypoint_hash,
                block_height: action_block.height(),
                authority: authority.clone(),
                semantic,
            });
        }
        action_offset = action_end;
    }
    assert!(authored_executables.next().is_none());
    assert_eq!(entries.len(), selected.len());
    let first_action_height = entries
        .iter()
        .map(|entry| entry.block_height)
        .min()
        .expect("the complete current semantic run has actual action carriers");
    let final_success_effect = native
        .chain
        .state()
        .view()
        .world()
        .smart_contract_state()
        .get(&success_effect_path)
        .expect("actual successful address-scoped state remains initialized")
        .clone();
    assert_ne!(
        final_success_effect, initial_success_effect,
        "successful original runtime calls change their native address-scoped state"
    );
    let mut contract_errors: Vec<_> = counter_error
        .variants
        .iter()
        .map(|variant| CounterContractErrorV1 {
            contract_address: address.clone(),
            contract: "PrivateCounterFixture".into(),
            error_type: counter_error.identity.clone(),
            schema_hash: counter_error.schema_hash(),
            name: variant.name.clone(),
            code: variant.code,
            code_hash,
            artifact_hash,
            rejection: match variant.code {
                1 => CounterRejectionV1::WalletLimitExceeded,
                2 => CounterRejectionV1::BelowMinimum,
                3 => CounterRejectionV1::IncomingDisabled,
                4 => CounterRejectionV1::NotPermitted,
                5 => CounterRejectionV1::InsufficientBalance,
                _ => panic!("not a declared fixture error"),
            },
        })
        .collect();
    if matches!(control, Control::WrongErrorBinding) {
        contract_errors[0].name = "WrongDeclaredVariant".into();
    }
    contract_errors.sort_by(|left, right| {
        (
            &left.contract_address,
            &left.contract,
            &left.error_type,
            left.schema_hash,
            &left.name,
            left.code,
        )
            .cmp(&(
                &right.contract_address,
                &right.contract,
                &right.error_type,
                right.schema_hash,
                &right.name,
                right.code,
            ))
    });
    let ids: Vec<_> = selected.iter().map(|action| action.action_id).collect();
    let semantics =
        compiled_private_counter_plan_v1(CounterPurposeV1::WalkthroughInteractions, &ids).unwrap();
    let mut contracts = vec![
        artifact_hash,
        if matches!(control, Control::WrongSuccessArtifact) {
            [7; 32]
        } else {
            success_artifact_hash
        },
    ];
    contracts.sort_unstable();
    let policy = PrivateCountersPolicyV1 {
        version: 1,
        network_id: native.chain.network_id(),
        scope: native.scope,
        purpose: CounterPurposeV1::WalkthroughInteractions,
        run_id: run_binding.commitment().unwrap(),
        run_binding,
        readers: native
            .readers
            .iter()
            .map(|reader| AccountId::new(reader.public_key().clone()))
            .collect(),
        authorities: vec![authority.clone()],
        plan_hash: counter_plan_commitment_v1(&semantics, &expected_executables).unwrap(),
        expected_executables,
        contracts,
        contract_errors,
        first_height: first_action_height,
        last_height: MAX_PRIVATE_COUNTER_HEIGHT_V1,
        limits: CounterLimitsV1 {
            max_entries: 30,
            max_groups: 64,
            max_carrier_work: 10_000,
            max_total_work: 10_000,
            max_source_bytes: 16 * 1024 * 1024,
            max_retained_bytes: 16 * 1024 * 1024,
            max_time_to_live_ms: 60_000,
            max_clock_skew_ms: 5_000,
            max_signature_age_ms: 60_000,
        },
    };
    assert_eq!(policy.contract_errors.len(), 5);
    assert!(
        policy
            .contract_errors
            .iter()
            .all(|binding| binding.contract_address != success_address),
        "successful pinned contract receives no invented nominal counter-error mapping"
    );
    policy.validate().unwrap();
    if matches!(control, Control::OmitReceipt) {
        entries.pop();
    }
    entries.sort_by_key(|entry| entry.entrypoint_hash);
    let manifest = PrivateCountersManifestV1 {
        version: 1,
        network_id: policy.network_id,
        scope: policy.scope,
        purpose: policy.purpose,
        run_id: policy.run_id,
        policy_hash: policy.commitment().unwrap(),
        entries,
    };
    // A separate fixed-owner sealing carrier publishes both originals; it is not a semantic action.
    let seal = native.chain.sign(
        &key,
        [
            SetKeyValue::account(
                authority.clone(),
                PRIVATE_COUNTER_POLICY_METADATA_KEY_V1.parse().unwrap(),
                Json::new(hex::encode(policy.encode_canonical().unwrap())),
            )
            .into(),
            SetKeyValue::account(
                authority,
                PRIVATE_COUNTER_MANIFEST_METADATA_KEY_V1.parse().unwrap(),
                Json::new(hex::encode(manifest.encode_canonical().unwrap())),
            )
            .into(),
        ],
        7_000,
    );
    let sealing_results = native.chain.commit_at(7_000, vec![seal]);
    let sealing_block = native.chain.committed(native.chain.height());
    let original_sealing_result = sealing_block
        .block()
        .network_output_at(0)
        .expect("actual owner sealing output exists")
        .1
        .result
        .as_ref()
        .map(|_| ());
    assert_eq!(
        sealing_results,
        vec![true],
        "actual owner sealing carrier executes: {original_sealing_result:?}"
    );
    native.chain.commit_at(8_000, vec![]);
    PublishedPrivateCounterFixture {
        native,
        policy,
        manifest,
    }
}

#[test]
fn genuine_complete30_native_originals_compute_operator_and_both_psp_counters() {
    let fixture = published_interactions_fixture_v1();
    assert_eq!(fixture.policy.contracts.len(), 2);
    assert_eq!(
        fixture
            .policy
            .contract_errors
            .iter()
            .map(|binding| &binding.contract_address)
            .collect::<BTreeSet<_>>()
            .len(),
        1,
        "successful error-free code is independently pinned without invented rejection mappings"
    );
    let owner = AccountId::new(fixture.native.policy_key.public_key().clone());
    for reader in 0..3 {
        let request = fixture.request(reader);
        let budget = AllocationBudget::new(64 * 1024 * 1024);
        let claim = compute_private_transaction_counters_v1(
            fixture.native.chain.state(),
            &owner,
            &request,
            request.payload.creation_time_ms,
            &budget,
        )
        .expect("complete genuine current native counters");
        let admitted_party = match reader {
            0 => None,
            1 => Some(CounterPartyV1::Psp1),
            2 => Some(CounterPartyV1::Psp2),
            _ => unreachable!(),
        };
        let expected = compiled_plan::INTERACTIONS
            .iter()
            .filter(|action| {
                action.action_id != 12 && admitted_party.is_none_or(|party| action.party == party)
            })
            .count() as u64;
        assert_eq!(
            claim.groups.iter().map(|group| group.count).sum::<u64>(),
            expected
        );
        assert!(
            claim
                .groups
                .iter()
                .all(|group| group.key.party == CounterPartyV1::None)
        );
        assert_eq!(
            budget.reserved_bytes(),
            COUNTER_AUXILIARY_BYTES,
            "computed claim keeps its original charge until payload destruction"
        );
        assert!(budget.peak_reserved_bytes() <= budget.limit_bytes());
        if reader == 0 {
            assert_eq!(expected, 30);
            assert_eq!(
                claim
                    .groups
                    .iter()
                    .filter(|group| group.key.result == CounterResultV1::Rejected)
                    .map(|group| group.count)
                    .sum::<u64>(),
                6
            );
            assert_eq!(
                claim
                    .groups
                    .iter()
                    .filter(|group| group.key.category == CounterCategoryV1::BatchLeg)
                    .map(|group| group.count)
                    .sum::<u64>(),
                1
            );
        }
        let (claim, original_allocation) = claim.into_parts();
        assert!(original_allocation.belongs_to(&budget));
        drop(claim);
        assert_eq!(
            budget.reserved_bytes(),
            COUNTER_AUXILIARY_BYTES,
            "installed member retains the original charge after moving its claim"
        );
        drop(original_allocation);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn same_signed_metadata_substituted_log_cannot_count_as_authored_issuance() {
    assert_refused(published(Control::SubstituteLog));
}

#[test]
fn complete_genuine_counter_plan_refuses_capacity_before_history_projection() {
    use crate::smartcontracts::isi::tx::{
        canonical_network_projection_calls_for_test,
        reset_canonical_network_projection_calls_for_test,
    };
    let fixture = published_interactions_fixture_v1();
    let owner = AccountId::new(fixture.native.policy_key.public_key().clone());
    let request = fixture.request(0);
    let graph_bytes = usize::try_from(fixture.policy.limits.max_source_bytes).unwrap()
        + usize::try_from(fixture.policy.limits.max_retained_bytes).unwrap()
        + COUNTER_CURRENT_FRAME_BYTES;
    let operation_budget = AllocationBudget::new(COUNTER_AUXILIARY_BYTES + graph_bytes - 1);
    reset_canonical_network_projection_calls_for_test();
    assert!(matches!(
        compute_private_transaction_counters_v1(
            fixture.native.chain.state(),
            &owner,
            &request,
            request.payload.creation_time_ms,
            &operation_budget,
        ),
        Err(PrivateCountersErrorV1::Bounds)
    ));
    assert_eq!(
        canonical_network_projection_calls_for_test(),
        0,
        "no partial native history row may be projected before complete graph admission"
    );
    assert_eq!(
        operation_budget.reserved_bytes(),
        0,
        "refused graph admission releases its auxiliary owner"
    );
    assert_eq!(
        operation_budget.peak_reserved_bytes(),
        COUNTER_AUXILIARY_BYTES,
        "the complete original graph reservation refuses atomically"
    );
}

#[test]
fn complete_native_policy_refuses_one_omitted_actual_receipt() {
    assert_refused(published(Control::OmitReceipt));
}

#[test]
fn genuine_rejection_cannot_match_changed_nominal_error_binding() {
    assert_refused(published(Control::WrongErrorBinding));
}

#[test]
fn successful_original_code_refuses_a_foreign_released_artifact_pin() {
    assert_refused(published(Control::WrongSuccessArtifact));
}

fn assert_refused(fixture: PublishedPrivateCounterFixture) {
    let owner = AccountId::new(fixture.native.policy_key.public_key().clone());
    let request = fixture.request(0);
    assert!(matches!(
        compute_private_transaction_counters_v1(
            fixture.native.chain.state(),
            &owner,
            &request,
            request.payload.creation_time_ms,
            &AllocationBudget::new(64 * 1024 * 1024)
        ),
        Err(PrivateCountersErrorV1::Context)
    ));
}

#[test]
fn complete_published_native_plan_refuses_re_signed_foreign_cut() {
    let fixture = published_interactions_fixture_v1();
    let owner = AccountId::new(fixture.native.policy_key.public_key().clone());
    let mut payload = fixture.request(0).payload;
    payload.cut.world_root = Hash::new(b"another current cut");
    let request = payload.try_sign(&fixture.native.readers[0]).unwrap();
    assert!(matches!(
        compute_private_transaction_counters_v1(
            fixture.native.chain.state(),
            &owner,
            &request,
            request.payload.creation_time_ms,
            &AllocationBudget::new(64 * 1024 * 1024)
        ),
        Err(PrivateCountersErrorV1::Context)
    ));
}

#[test]
fn genuine_signed_genesis_counter_source_sequences_preserve_all_finite_limits() {
    use norito::core::{DecodeAttemptErrorKind, DecodeResourceError};

    fn original_refusal(original: &[u8], limits: norito::DecodeLimits) -> DecodeResourceError {
        let error = norito::core::with_decode_limits_scope(limits, || {
            iroha_data_model::block::decode_framed_signed_block(original)
        })
        .expect_err("the genuine unchanged source must retain its exact caller refusal");
        assert_eq!(error.kind(), DecodeAttemptErrorKind::EnclosingLimit);
        error
            .into_error()
            .decode_resource_error()
            .expect("the original native decoder retains the exact resource dimension")
    }

    let fixture = published_interactions_fixture_v1();
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let auxiliary = budget.try_reserve_bytes(COUNTER_AUXILIARY_BYTES).unwrap();
    let allocations = CounterGraphAllocations::admit(&fixture.policy.limits, &budget).unwrap();
    let profile = counter_source_decode_limits_v1(allocations.source.remaining_bytes());
    let view = fixture.native.chain.state().view();
    let genesis = fixture.native.chain.committed(1);
    // The fixture oracle performs native history decoding; admit it before the tested families.
    let expected_prefix = [
        fixture.native.chain.committed(1),
        fixture.native.chain.committed(2),
        fixture.native.chain.committed(3),
        fixture.native.chain.committed(4),
        fixture.native.chain.committed(5),
    ]
    .map(|expected| (expected.height(), expected.block_hash(), expected));
    let expected_tip_hash = fixture
        .native
        .chain
        .committed(fixture.native.chain.height())
        .block_hash();
    let source = view
        .kura()
        .native_frame_read(1, genesis.block_hash())
        .unwrap()
        .expect("the actual signed-genesis canonical slot is retained");
    let wire_len = source.wire_len();
    assert!(wire_len <= fixture.policy.limits.max_source_bytes);
    let original = source
        .read(wire_len, &budget)
        .unwrap()
        .expect("the same physical signed-genesis original remains available");
    assert!(original.belongs_to(&budget));

    let old_profile = norito::DecodeLimits::new(
        1024,
        profile.max_field_bytes(),
        profile.max_total_elements(),
        profile.max_total_allocated_bytes(),
        profile.max_nesting_depth(),
    );
    assert_eq!(
        original_refusal(&original, old_profile),
        DecodeResourceError::SequenceLengthExceeded {
            length: 2484,
            limit: 1024,
        },
        "the genuine recorded failure is a source sequence, not an output group limit"
    );
    let (decoded, usage) = norito::core::with_decode_limits_measured(profile, || {
        iroha_data_model::block::decode_framed_signed_block(&original)
    });
    let decoded = decoded.expect("the corrected finite profile admits this exact native source");
    assert_eq!(&decoded, genesis.block().as_ref());
    assert!(usage.total_elements() <= profile.max_total_elements());
    assert!(usage.total_allocated_bytes() <= allocations.source.remaining_bytes());
    drop(decoded);

    // Compare the genuine contiguous original bodies without adding an oracle decode
    // inside this family. This direct prefix alone is narrower than the archive operation:
    // the production reader acquires its requested tip before verifying the preceding gaps.
    let decode_original_prefix = || {
        for (height, expected_hash, committed) in &expected_prefix {
            let source = view
                .kura()
                .native_frame_read(*height, *expected_hash)
                .unwrap()
                .expect("each genuine contiguous original remains retained");
            let wire_len = source.wire_len();
            assert!(wire_len <= fixture.policy.limits.max_source_bytes);
            let original = source
                .read(wire_len, &budget)
                .unwrap()
                .expect("each original frame uses the same physical operation pool");
            assert!(original.belongs_to(&budget));
            let decoded = iroha_data_model::block::decode_framed_signed_block(&original)?;
            assert_eq!(&decoded, committed.block().as_ref());
        }
        Ok::<_, norito::core::DecodeAttemptError>(())
    };
    let (prefix_decodes, prefix_usage) =
        norito::core::with_decode_limits_measured(profile, decode_original_prefix);
    prefix_decodes.expect("the genuine H1 through H5 originals fit the prepaid source envelope");
    assert!(prefix_usage.total_elements() <= profile.max_total_elements());
    assert!(prefix_usage.total_elements() <= prefix_usage.total_allocated_bytes());
    assert!(prefix_usage.total_allocated_bytes() <= allocations.source.remaining_bytes());

    // Exercise the exact production archive sequence under one cumulative source family:
    // authenticate signed genesis, request the actual tip, then verify its complete prefix.
    // The reader's native target/genesis/gap ordering must supply the measured work itself.
    let singular_limits = crate::smartcontracts::isi::query::SingularQueryOutputLimits::new(
        allocations.frame.remaining_bytes() as u64,
        allocations.retained.remaining_bytes() as u64,
    );
    let authenticate_original_archive = || {
        crate::smartcontracts::isi::query::with_retained_singular_query_limits(
            singular_limits,
            || {
                let archive = CertifiedArchiveView::new_with_budget(&view, view.kura(), &budget)?;
                assert_eq!(archive.tip_height(), fixture.native.chain.height());
                let tip = archive.block(archive.tip_height())?;
                assert_eq!(tip.committed().block_hash(), expected_tip_hash);
                archive.verify_unchanged()?;
                Ok::<_, crate::query::archive_finality::ArchiveFinalityError>(())
            },
        )
    };
    let (archive_read, archive_usage) =
        norito::core::with_decode_limits_measured(profile, authenticate_original_archive);
    eprintln!(
        "genuine counter source usage: direct_prefix_elements={} archive_elements={} archive_allocated_bytes={}",
        prefix_usage.total_elements(),
        archive_usage.total_elements(),
        archive_usage.total_allocated_bytes(),
    );
    archive_read.expect("the prepaid profile authenticates the complete original archive");
    assert!(archive_usage.total_elements() > 131_072);
    assert!(archive_usage.total_elements() <= profile.max_total_elements());
    assert!(archive_usage.total_elements() <= archive_usage.total_allocated_bytes());
    assert!(archive_usage.total_allocated_bytes() <= allocations.source.remaining_bytes());

    let old_total_profile = norito::DecodeLimits::new(
        profile.max_sequence_elements(),
        profile.max_field_bytes(),
        131_072,
        profile.max_total_allocated_bytes(),
        profile.max_nesting_depth(),
    );
    let (archive_refusal, narrower_usage) =
        norito::core::with_decode_limits_measured(old_total_profile, authenticate_original_archive);
    eprintln!(
        "genuine counter narrower source usage: archive_elements={} archive_allocated_bytes={}",
        narrower_usage.total_elements(),
        narrower_usage.total_allocated_bytes(),
    );
    let archive_refusal = archive_refusal
        .expect_err("the same original archive retains the narrower cumulative refusal");
    assert!(
        matches!(
            &archive_refusal,
            crate::query::archive_finality::ArchiveFinalityError::Deferred(original)
                if original.reason() == ivm::error::ExecutionDeferral::ActiveMemoryCapacity
                    && original.allocation_refusal().is_none()
        ),
        "the native archive retains its original caller-limit deferral: {archive_refusal:?}"
    );
    assert!(narrower_usage.total_elements() <= old_total_profile.max_total_elements());
    assert!(narrower_usage.total_elements() < archive_usage.total_elements());
    assert!(narrower_usage.total_allocated_bytes() <= allocations.source.remaining_bytes());

    let fewer_elements = usage.total_elements().checked_sub(1).unwrap();
    assert!(matches!(
        original_refusal(
            &original,
            norito::DecodeLimits::new(
                profile.max_sequence_elements(),
                profile.max_field_bytes(),
                fewer_elements,
                profile.max_total_allocated_bytes(),
                profile.max_nesting_depth(),
            ),
        ),
        DecodeResourceError::TotalElementsExceeded { limit, .. }
            if limit == fewer_elements as u64
    ));
    let fewer_bytes = usage.total_allocated_bytes().checked_sub(1).unwrap();
    assert!(matches!(
        original_refusal(
            &original,
            norito::DecodeLimits::new(
                profile.max_sequence_elements(),
                profile.max_field_bytes(),
                profile.max_total_elements(),
                fewer_bytes,
                profile.max_nesting_depth(),
            ),
        ),
        DecodeResourceError::TotalAllocationExceeded { limit, .. }
            if limit == fewer_bytes as u64
    ));
    assert!(matches!(
        original_refusal(
            &original,
            norito::DecodeLimits::new(
                profile.max_sequence_elements(),
                0,
                profile.max_total_elements(),
                profile.max_total_allocated_bytes(),
                profile.max_nesting_depth(),
            ),
        ),
        DecodeResourceError::ArchiveLengthExceeded { limit: 0, .. }
            | DecodeResourceError::FieldLengthExceeded { limit: 0, .. }
    ));
    assert!(matches!(
        original_refusal(
            &original,
            norito::DecodeLimits::new(
                profile.max_sequence_elements(),
                profile.max_field_bytes(),
                profile.max_total_elements(),
                profile.max_total_allocated_bytes(),
                0,
            ),
        ),
        DecodeResourceError::NestingDepthExceeded { limit: 0, .. }
    ));
    assert!(budget.peak_reserved_bytes() <= budget.limit_bytes());
    drop(original);
    drop(allocations);
    drop(auxiliary);
    assert_eq!(budget.reserved_bytes(), 0);
}
