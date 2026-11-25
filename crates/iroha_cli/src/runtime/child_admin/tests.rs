//! Offline plan/signature assembly and exact sole-instruction relay regressions.

use super::*;
use iroha::{
    crypto::{Algorithm, Hash, HashOf, KeyPair},
    data_model::{
        NetworkId,
        parameter::runtime_upgrade_policy::RuntimeUpgradePolicyV1,
        runtime::{
            child_admin::ChildRootIdentityV1,
            definition::SelectionId,
            lifecycle::{RootScopeV1, RuntimeNoticeV1},
        },
        transaction::Executable,
    },
};
use iroha_model_base::topology::DataSpaceId;

#[derive(Default)]
struct Captured {
    config: Option<iroha::config::Config>,
    stdin: bool,
    printed: Option<String>,
    submitted: Option<Executable>,
}

impl RunContext for Captured {
    fn config(&self) -> &iroha::config::Config {
        self.config
            .as_ref()
            .expect("offline commands never read config")
    }
    fn has_client_config(&self) -> bool {
        self.config.is_some()
    }
    fn transaction_metadata(&self) -> Option<&iroha_model_base::metadata::Metadata> {
        None
    }
    fn input_instructions(&self) -> bool {
        self.stdin
    }
    fn output_instructions(&self) -> bool {
        false
    }
    fn i18n(&self) -> &iroha_i18n::Localizer {
        panic!("commands only print data")
    }
    fn print_data<T: norito::json::JsonSerialize + ?Sized>(&mut self, data: &T) -> Result<()> {
        assert!(self.printed.replace(norito::json::to_json(data)?).is_none());
        Ok(())
    }
    fn println(&mut self, _: impl std::fmt::Display) -> Result<()> {
        panic!("commands only print data")
    }
    fn finish(&mut self, instructions: impl Into<Executable>) -> Result<()> {
        assert!(self.submitted.replace(instructions.into()).is_none());
        Ok(())
    }
}

fn fixture() -> (
    ChildRuntimeAdminStateV1,
    Vec<KeyPair>,
    iroha::config::Config,
) {
    let config = crate::fallback_config();
    let mut root = ChildRootIdentityV1 {
        network: config.network_id,
        parent_network: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"reviewed-child-parent",
        ))),
        dataspace: DataSpaceId::new((1 << 40) + 41),
        instance: [0; 32],
    };
    root.instance = root
        .scope()
        .instance_id(
            &iroha_core::sumeragi::crypto::BlsCrypto::new(),
            config.network_id,
            config.chain.as_str(),
        )
        .unwrap()
        .0;
    let mut keys: Vec<_> = (41..=43)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect();
    keys.sort_by(|a, b| a.public_key().cmp(b.public_key()));
    let notice = RuntimeNoticeV1 {
        blocks: 2,
        ms: 0,
        approval_cushion_ms: 0,
    };
    (
        ChildRuntimeAdminStateV1 {
            root,
            policy: ChildRuntimeAdminPolicyV1 {
                revision: 1,
                admins: keys.iter().map(|key| key.public_key().clone()).collect(),
                threshold: 2,
                notice: RuntimeUpgradePolicyV1 {
                    activation: notice,
                    disable: notice,
                },
                provenance: None,
            },
            next_nonce: 4,
        },
        keys,
        config,
    )
}

fn save<T: norito::json::JsonSerialize>(
    directory: &std::path::Path,
    name: &str,
    value: &T,
) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, norito::json::to_json(value).unwrap()).unwrap();
    path
}

fn plan(state: &ChildRuntimeAdminStateV1) -> ChildRuntimeAdminPlanV1 {
    ChildRuntimeAdminPlanV1 {
        root: state.root,
        policy_id: state.policy.id().unwrap(),
        policy_revision: state.policy.revision,
        nonce: state.next_nonce,
        valid_from: 5,
        valid_until: 10,
        action: ChildRuntimeAdminActionV1::Cancel(ChangeSetId([9; 32])),
    }
}

#[test]
fn offline_prepare_binds_schedule_cancel_and_rotation_to_exact_review_context() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _, _) = fixture();
    let state_file = save(dir.path(), "state.json", &state);
    let change = ChangeSetV1 {
        network: state.root.network,
        root: RootScopeV1::DataSpace(state.root.dataspace),
        predecessor: SelectionId([1; 32]),
        successor: SelectionId([2; 32]),
        effective_height: 9,
        publish: vec![],
        transitions: vec![],
        qualification: [3; 32],
    };
    let change_file = save(dir.path(), "change.json", &change);
    let mut next = state.policy.clone();
    next.revision += 1;
    next.threshold = 3;
    let next_file = save(dir.path(), "next.json", &next);
    for (action, expected) in [
        (
            PrepareAction::Schedule {
                change_set: change_file.clone(),
                endorsements: vec![],
            },
            ChildRuntimeAdminActionV1::Schedule {
                change_set: change.clone(),
                provenance: vec![],
            },
        ),
        (
            PrepareAction::Cancel {
                id: hex::encode([9; 32]),
            },
            ChildRuntimeAdminActionV1::Cancel(ChangeSetId([9; 32])),
        ),
        (
            PrepareAction::Rotate { policy: next_file },
            ChildRuntimeAdminActionV1::RotatePolicy(next),
        ),
    ] {
        let mut context = Captured::default();
        PrepareArgs {
            state: StateArgs {
                state: state_file.clone(),
            },
            valid_from: 5,
            valid_until: 10,
            action,
        }
        .run(&mut context)
        .unwrap();
        let prepared: ChildRuntimeAdminPlanV1 =
            norito::json::from_str(context.printed.as_ref().unwrap()).unwrap();
        assert_eq!(
            prepared,
            ChildRuntimeAdminPlanV1 {
                action: expected,
                ..plan(&state)
            }
        );
        assert!(context.submitted.is_none());
    }
    let foreign = ChangeSetV1 {
        root: RootScopeV1::Universal,
        ..change
    };
    save(dir.path(), "change.json", &foreign);
    let mut context = Captured::default();
    assert!(
        PrepareArgs {
            state: StateArgs { state: state_file },
            valid_from: 5,
            valid_until: 10,
            action: PrepareAction::Schedule {
                change_set: change_file,
                endorsements: vec![]
            },
        }
        .run(&mut context)
        .is_err()
    );
    assert!(context.printed.is_none());
}

#[test]
fn offline_sign_assemble_and_relay_preserve_the_exact_certificate_without_key_output() {
    let dir = tempfile::tempdir().unwrap();
    let (state, keys, config) = fixture();
    let state_file = save(dir.path(), "state.json", &state);
    let plan = plan(&state);
    let plan_file = save(dir.path(), "plan.json", &plan);
    let mut signatures = Vec::new();
    for (index, key) in keys[..2].iter().enumerate() {
        let key_text = iroha::crypto::ExposedPrivateKey(key.private_key().clone())
            .try_to_multihash_string()
            .unwrap();
        let key_file = dir.path().join(format!("key-{index}"));
        std::fs::write(&key_file, &key_text).unwrap();
        let mut context = Captured::default();
        SignArgs {
            state: StateArgs {
                state: state_file.clone(),
            },
            plan: plan_file.clone(),
            admin_key_file: key_file,
        }
        .run(&mut context)
        .unwrap();
        let printed = context.printed.unwrap();
        assert!(!printed.contains(&key_text));
        let signature: ChildAdminSignatureV1 = norito::json::from_str(&printed).unwrap();
        assert_eq!(signature.signer, *key.public_key());
        assert!(context.submitted.is_none());
        signatures.push(save(dir.path(), &format!("sig-{index}.json"), &signature));
    }
    signatures.reverse();
    let mut context = Captured::default();
    AssembleArgs {
        state: StateArgs {
            state: state_file.clone(),
        },
        plan: plan_file,
        signatures,
        at_height: 5,
    }
    .run(&mut context)
    .unwrap();
    let certificate: ChildRuntimeAdminCertificateV1 =
        norito::json::from_str(context.printed.as_ref().unwrap()).unwrap();
    assert_eq!(certificate.plan, plan);
    certificate.verify_against(&state, 9).unwrap();
    assert_eq!(
        ChildRuntimeAdminCertificateV1::from_bytes(&certificate.to_bytes().unwrap()).unwrap(),
        certificate
    );
    assert!(context.submitted.is_none());
    let file = save(dir.path(), "certificate.json", &certificate);
    let mut context = Captured {
        config: Some(config),
        ..Captured::default()
    };
    SubmitArgs {
        state: StateArgs { state: state_file },
        certificate: file,
        at_height: 5,
    }
    .run(&mut context)
    .unwrap();
    assert_eq!(
        context.submitted,
        Some(Executable::Instructions(
            vec![InstructionBox::from(AuthorizeChildRuntimeActionV1 {
                certificate
            })]
            .into()
        ))
    );
}

#[test]
fn signing_refuses_each_foreign_or_stale_binding_and_nonmember_keys() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _, _) = fixture();
    let original = plan(&state);
    let mut variants = Vec::new();
    let mut changed = original.clone();
    changed.root.network = state.root.parent_network;
    variants.push(changed);
    let mut changed = original.clone();
    changed.root.parent_network = state.root.network;
    variants.push(changed);
    let mut changed = original.clone();
    changed.root.dataspace = DataSpaceId::new(9);
    variants.push(changed);
    let mut changed = original.clone();
    changed.root.instance[0] ^= 1;
    variants.push(changed);
    let mut changed = original.clone();
    changed.policy_id[0] ^= 1;
    variants.push(changed);
    let mut changed = original.clone();
    changed.policy_revision += 1;
    variants.push(changed);
    let mut changed = original.clone();
    changed.nonce += 1;
    variants.push(changed);
    let state_file = save(dir.path(), "state.json", &state);
    for changed in variants {
        let mut context = Captured::default();
        let error = SignArgs {
            state: StateArgs {
                state: state_file.clone(),
            },
            plan: save(dir.path(), "plan.json", &changed),
            admin_key_file: dir.path().join("absent-key"),
        }
        .run(&mut context)
        .unwrap_err();
        assert!(error.to_string().contains("reviewed root"), "{error}");
        assert!(context.printed.is_none());
    }
    let outsider = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519);
    let key_file = dir.path().join("outsider");
    std::fs::write(
        &key_file,
        iroha::crypto::ExposedPrivateKey(outsider.private_key().clone())
            .try_to_multihash_string()
            .unwrap(),
    )
    .unwrap();
    let mut context = Captured::default();
    assert!(
        SignArgs {
            state: StateArgs { state: state_file },
            plan: save(dir.path(), "plan.json", &original),
            admin_key_file: key_file,
        }
        .run(&mut context)
        .unwrap_err()
        .to_string()
        .contains("not in")
    );
    let mut exhausted = state;
    exhausted.next_nonce = u64::MAX;
    assert!(check_plan(&exhausted, &plan(&exhausted)).is_err());
}

#[test]
fn assembly_refuses_duplicates_insufficient_foreign_wrong_plan_and_expired_signatures() {
    let dir = tempfile::tempdir().unwrap();
    let (state, keys, _) = fixture();
    let plan = plan(&state);
    let state_file = save(dir.path(), "state.json", &state);
    let plan_file = save(dir.path(), "plan.json", &plan);
    let first = ChildAdminSignatureV1::sign(&plan, &keys[0]).unwrap();
    let second = ChildAdminSignatureV1::sign(&plan, &keys[1]).unwrap();
    let outsider =
        ChildAdminSignatureV1::sign(&plan, &KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519))
            .unwrap();
    let mut different = plan.clone();
    different.valid_until += 1;
    let wrong = ChildAdminSignatureV1::sign(&different, &keys[2]).unwrap();
    for (signatures, height) in [
        (vec![first.clone()], 5),
        (vec![first.clone(), first.clone()], 5),
        (vec![first.clone(), second.clone(), outsider], 5),
        (vec![first.clone(), second.clone(), wrong], 5),
        (vec![first.clone(), second.clone()], 4),
        (vec![first, second], 10),
    ] {
        let signatures = signatures
            .iter()
            .enumerate()
            .map(|(index, signature)| save(dir.path(), &format!("sig-{index}.json"), signature))
            .collect();
        let mut context = Captured::default();
        assert!(
            AssembleArgs {
                state: StateArgs {
                    state: state_file.clone()
                },
                plan: plan_file.clone(),
                signatures,
                at_height: height,
            }
            .run(&mut context)
            .is_err()
        );
        assert!(context.printed.is_none());
        assert!(context.submitted.is_none());
    }
}

#[test]
fn relay_refuses_foreign_config_instance_missing_config_expiry_and_instruction_composition() {
    let dir = tempfile::tempdir().unwrap();
    let (state, keys, config) = fixture();
    let plan = plan(&state);
    let certificate = ChildRuntimeAdminCertificateV1 {
        signatures: keys[..2]
            .iter()
            .map(|key| ChildAdminSignatureV1::sign(&plan, key).unwrap())
            .collect(),
        plan,
    };
    let state_file = save(dir.path(), "state.json", &state);
    let certificate_file = save(dir.path(), "certificate.json", &certificate);
    let mut foreign = config.clone();
    foreign.network_id = state.root.parent_network;
    let mut other_chain = config.clone();
    other_chain.chain = "another-child-instance".into();
    let mut wrong_signer = config.clone();
    wrong_signer.key_pair = KeyPair::from_seed(vec![71; 32], Algorithm::Secp256k1);
    for (config, stdin, height) in [
        (Some(foreign), false, 5),
        (Some(other_chain), false, 5),
        (None, false, 5),
        (Some(config.clone()), true, 5),
        (Some(config), false, 10),
        (Some(wrong_signer), false, 5),
    ] {
        let mut context = Captured {
            config,
            stdin,
            ..Captured::default()
        };
        assert!(
            SubmitArgs {
                state: StateArgs {
                    state: state_file.clone()
                },
                certificate: certificate_file.clone(),
                at_height: height
            }
            .run(&mut context)
            .is_err()
        );
        assert!(context.submitted.is_none());
    }
}

#[test]
fn only_offline_child_commands_allow_missing_client_configuration() {
    use clap::Parser as _;
    for (tail, offline) in [
        (
            vec![
                "prepare",
                "--state",
                "state.json",
                "--valid-from",
                "5",
                "--valid-until",
                "10",
                "cancel",
                "--id",
                "09",
            ],
            true,
        ),
        (
            vec![
                "sign",
                "--state",
                "state.json",
                "--plan",
                "plan.json",
                "--admin-key-file",
                "key",
            ],
            true,
        ),
        (
            vec![
                "assemble",
                "--state",
                "state.json",
                "--plan",
                "plan.json",
                "--signature",
                "sig.json",
                "--at-height",
                "5",
            ],
            true,
        ),
        (
            vec![
                "submit",
                "--state",
                "state.json",
                "--certificate",
                "cert.json",
                "--at-height",
                "5",
            ],
            false,
        ),
    ] {
        let mut args = vec!["iroha", "ops", "runtime", "child-admin"];
        args.extend(tail);
        let args = crate::Args::try_parse_from(args).unwrap();
        assert_eq!(args.command.allows_fallback_config(), offline);
        assert_eq!(
            args.command.allows_fallback_config_in_machine_mode(),
            offline
        );
    }
    let args = crate::Args::try_parse_from(["iroha", "ops", "runtime", "upgrade", "list"]).unwrap();
    assert!(!args.command.allows_fallback_config());
}
