//! Join original parent and registered-child publications to actual native retirement writes.
//!
//! Parent approval uses the producer tests' explicit scheduled-record setup corridor. Child
//! approval uses its signed genesis administrators. Activation, validator support, startup
//! floors, retirement writes, registration, anchors and original State cuts use their real
//! owners. This tests the evidence join, not Parliament approval or source-removed replay.

use super::*;
use crate::runtime::{
    catalog::additions,
    change_set::{PublishedDefinitions, record_attestation},
    child_admin::certified_tests as child_fixture,
};
use crate::sumeragi::test_chain::fixture_keys;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::{AccountAddress, AccountId},
    isi::{
        InstructionBox,
        child_runtime::AttestChildRuntimeSupportV1,
        private_dataspace::{AnchorPrivateDataspace, RegisterPrivateDataspace},
        runtime_boundary::RaiseRuntimeStartupFloorV1,
        runtime_retirement::RetireRuntimeDefinitionV1,
    },
    parameter::{Parameter, RuntimeCheckpointPolicyV1},
    runtime::{
        checkpoint::{StartupFloorAttestationV1, StartupFloorV1},
        child_admin::ChildRuntimeAdminActionV1,
        lifecycle::{
            ChangeSetRecordV1, ChangeSetStatusV1, ChangeSetV1, RestrictionKindV1, RuntimeNoticeV1,
            SelectionTransitionV1, SupportAttestationV1, committee_digest,
        },
    },
    sns::{NameControllerV1, NameRecordV1},
};
use iroha_model_base::metadata::Metadata;
use norito::codec::Encode as _;
use std::collections::BTreeSet;

mod rehearsal;

const ALIAS: &str = "retirement-child";

fn payer() -> KeyPair {
    KeyPair::from_seed(vec![0xCE; 32], Algorithm::Ed25519)
}

fn checkpoint_policy() -> Parameter {
    Parameter::Custom(
        RuntimeCheckpointPolicyV1 {
            cadence_ms: 1,
            retain_all_ms: 10_000,
            retain_spacing_ms: 0,
            recovery_window_ms: 1,
            floor_notice_ms: 0,
        }
        .to_custom_parameter()
        .unwrap(),
    )
}

fn parent_config() -> TestChainConfig {
    let mut world = World::new();
    crate::runtime::activate_additions_for_testing(&world, additions::RUNTIME_LIFECYCLE.module)
        .unwrap();
    let owner = AccountId::new(payer().public_key().clone());
    let lease = NameRecordV1::new(
        crate::sns::selector_for_dataspace_alias(ALIAS).unwrap(),
        owner.clone(),
        vec![NameControllerV1::account(
            &AccountAddress::from_account_id(&owner).unwrap(),
        )],
        0,
        0,
        1_000_000,
        2_000_000,
        3_000_000,
        Metadata::default(),
    );
    world.smart_contract_state_mut_for_testing().insert(
        crate::sns::record_storage_key(&lease.selector),
        lease.encode(),
    );
    let mut config = TestChainConfig::new(world, 1_000);
    config.clock_bootstrap_in_genesis = true;
    config.genesis_parameters.extend([
        checkpoint_policy(),
        Parameter::Custom(
            PrivateDataspaceAdmissionPolicy {
                max_registered_roots: 1,
                max_roots_per_owner: 1,
            }
            .into_custom_parameter()
            .unwrap(),
        ),
    ]);
    config
}

fn next_time(chain: &CertifiedTestChain) -> u64 {
    1_000 + chain.height() * 1_000
}

fn submit(chain: &mut CertifiedTestChain, instructions: Vec<InstructionBox>) {
    let at = next_time(chain);
    let transaction = chain.sign(&payer(), instructions, at - 1);
    assert_eq!(chain.commit_at(at, vec![transaction]), [true]);
}

fn advance(chain: &mut CertifiedTestChain) {
    // An empty supplied list makes this fixture sign real clock work; no empty block is made.
    chain.commit_at(next_time(chain), Vec::new());
}

fn reviewed_aliases() -> (
    ImplementationCodeV1,
    BTreeMap<DefinitionId, DefinitionDescriptorV1>,
) {
    let catalog = crate::runtime::release_catalog().unwrap();
    let owner = catalog
        .definitions
        .values()
        .filter_map(code)
        .find(|code| {
            matches!(code, ImplementationCodeV1::Native { module, .. }
            if module == additions::ACCOUNT_SIGNING_KEYS.module)
        })
        .unwrap()
        .clone();
    let aliases: BTreeMap<_, _> = catalog
        .definitions
        .iter()
        .filter(|(_, descriptor)| code(descriptor) == Some(&owner))
        .map(|(id, descriptor)| (*id, descriptor.clone()))
        .collect();
    assert!(!aliases.is_empty());
    (owner, aliases)
}

fn retirement_change(
    chain: &CertifiedTestChain,
    aliases: &BTreeMap<DefinitionId, DefinitionDescriptorV1>,
) -> ChangeSetV1 {
    let view = chain.state().view();
    let selected = view.world().runtime_selection();
    let mut change = crate::runtime::draft_addition(
        view.world(),
        chain.network_id(),
        additions::RUNTIME_RETIREMENT.module,
        chain.height() + 4,
        [0x6A; 32],
    )
    .unwrap();
    let mut published: BTreeMap<_, _> = change
        .publish
        .drain(..)
        .map(|descriptor| (descriptor.id().unwrap(), descriptor))
        .collect();
    for (id, descriptor) in aliases {
        assert!(
            !selected.active.contains(id),
            "the reviewed native owner is unused"
        );
        if view.world().runtime_definitions().get(id).is_none() {
            published.insert(*id, descriptor.clone());
        }
        // These explicit ordered exclusions are approved with the change. Merely publishing
        // inactive code is insufficient; the producer retains this exact governed intent.
        change.transitions.extend([
            SelectionTransitionV1::Restrict {
                definition: *id,
                restriction: RestrictionKindV1::Disabled,
            },
            SelectionTransitionV1::Deactivate(*id),
        ]);
    }
    change.publish = published.into_values().collect();
    change
        .apply(
            selected,
            &PublishedDefinitions(view.world().runtime_definitions()),
        )
        .expect("inactive exclusions preserve the independently drafted successor");
    change
}

fn activate_parent(chain: &mut CertifiedTestChain, change: ChangeSetV1) {
    let committee: Vec<_> = fixture_keys()
        .into_iter()
        .map(|key| key.public_key().clone())
        .collect();
    let mut record = ChangeSetRecordV1 {
        change_set: change.clone(),
        status: ChangeSetStatusV1::Scheduled,
        proposed_ms: 0,
        approved_height: 1,
        approved_ms: 0,
        notice: RuntimeNoticeV1 {
            blocks: 1,
            ms: 0,
            approval_cushion_ms: 0,
        },
        attestations: Vec::new(),
    };
    for validator in fixture_keys() {
        record_attestation(
            &mut record,
            SupportAttestationV1 {
                network: chain.network_id(),
                root: change.root,
                change_set: change.id().unwrap(),
                predecessor: change.predecessor,
                successor: change.successor,
                committee: committee_digest(&committee).unwrap(),
                tested_checkpoint: 1,
                valid_from: 0,
                valid_until: u64::MAX,
                release: [0x6B; 32],
                validator: validator.public_key().clone(),
            }
            .sign(validator.private_key())
            .unwrap(),
        );
    }
    // Same explicit upstream-approval fixture corridor as producer/tests.rs. No selection,
    // census, receipt, certificate or execution result is manufactured here.
    chain.setup_world_at(next_time(chain), |tx| {
        tx.world
            .runtime_change_sets
            .insert(change.id().unwrap(), record);
    });
    while chain.height() < change.effective_height {
        advance(chain);
    }
    assert_eq!(
        chain.state().world_view().runtime_selection().id().unwrap(),
        change.successor
    );
}

fn activate_child(chain: &mut CertifiedTestChain, change: ChangeSetV1) {
    let action = child_fixture::action(
        chain,
        ChildRuntimeAdminActionV1::Schedule {
            change_set: change.clone(),
            provenance: Vec::new(),
        },
    );
    submit(chain, vec![action]);
    let committee: Vec<_> = chain
        .validators()
        .iter()
        .map(|(peer, _)| peer.public_key().clone())
        .collect();
    let at = next_time(chain);
    let transactions = fixture_keys()
        .iter()
        .map(|validator| {
            let attestation = crate::runtime::attestation::sign_support(
                chain.state().view().world(),
                &committee,
                chain.height(),
                change.id().unwrap(),
                validator,
                [0x6C; 32],
            )
            .unwrap();
            chain.sign(
                &payer(),
                [AttestChildRuntimeSupportV1 { attestation }.into()],
                at - 1,
            )
        })
        .collect();
    assert_eq!(chain.commit_at(at, transactions), [true; 4]);
    while chain.height() < change.effective_height {
        advance(chain);
    }
    assert_eq!(
        chain.state().world_view().runtime_selection().id().unwrap(),
        change.successor
    );
}

fn retire(
    chain: &mut CertifiedTestChain,
    aliases: &BTreeMap<DefinitionId, DefinitionDescriptorV1>,
) {
    retire_with_checkpoint(chain, aliases, |_| {});
}

fn retire_with_checkpoint(
    chain: &mut CertifiedTestChain,
    aliases: &BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    checkpoint: impl FnOnce(&CertifiedTestChain),
) {
    advance(chain); // The floor must exceed every alias's conservative first-census height.
    checkpoint(chain);
    let block = chain.committed(chain.height());
    let floor = StartupFloorV1 {
        network: chain.network_id(),
        height: chain.height(),
        block_hash: *block.block_hash().as_ref(),
        committed_ms: block.block_time_ms(),
    };
    let mut attestations: Vec<_> = fixture_keys()
        .iter()
        .map(|key| {
            StartupFloorAttestationV1 {
                floor: floor.id().unwrap(),
                validator: key.public_key().clone(),
            }
            .sign(key.private_key())
            .unwrap()
        })
        .collect();
    attestations
        .sort_by(|left, right| left.attestation.validator.cmp(&right.attestation.validator));
    submit(
        chain,
        vec![
            RaiseRuntimeStartupFloorV1 {
                floor,
                attestations,
            }
            .into(),
        ],
    );
    submit(
        chain,
        aliases
            .keys()
            .map(|id| RetireRuntimeDefinitionV1 { definition: *id }.into())
            .collect(),
    );
    let view = chain.state().view();
    for id in aliases.keys() {
        let proof = crate::runtime::retirement::receipt_proof(&view, *id)
            .unwrap()
            .expect("original native retirement write is archived");
        assert_eq!(proof.receipt.receipt_height, chain.height());
        assert!(proof.receipt.last_required.unwrap() < proof.receipt.startup_floor.height);
    }
}

fn anchor_instruction(
    child: &CertifiedTestChain,
    registration: &PrivateDataspaceRegistration,
    height: u64,
) -> InstructionBox {
    AnchorPrivateDataspace {
        dataspace_id: crate::sns::dataspace_id_for_sns_alias(ALIAS).unwrap(),
        anchor: norito::encode_canonical(
            &PrivateDataspaceAnchor::from_certificate(
                registration,
                child
                    .committed(height)
                    .block()
                    .commit_certificate()
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    }
    .into()
}

#[test]
fn original_parent_and_registered_child_receipts_join_every_reviewed_native_alias() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(verify_certified_join)
        .unwrap()
        .join()
        .unwrap();
}

fn verify_certified_join() {
    let (owner, aliases) = reviewed_aliases();
    let mut parent = CertifiedTestChain::start(parent_config()).unwrap();
    let home = crate::sns::dataspace_id_for_sns_alias(ALIAS).unwrap();
    let mut config = child_fixture::config_for_root(parent.network_id(), home);
    config.genesis_parameters.push(checkpoint_policy());
    let mut child = CertifiedTestChain::start(config).unwrap();
    let parent_change = retirement_change(&parent, &aliases);
    let child_change = retirement_change(&child, &aliases);
    activate_parent(&mut parent, parent_change);
    activate_child(&mut child, child_change);
    retire(&mut parent, &aliases);
    retire(&mut child, &aliases);

    let registration = PrivateDataspaceRegistration::new(
        child.root_scope(),
        child.state().query_view().chain_id().clone(),
        child.network_id(),
        child.committed(1).result().0,
        authenticated_genesis(child.genesis())
            .unwrap()
            .into_parts()
            .0,
    )
    .unwrap();
    let mut instructions = vec![
        RegisterPrivateDataspace {
            alias: ALIAS.into(),
            expected_ownership_generation: 1,
            registration: norito::encode_canonical(&registration).unwrap(),
        }
        .into(),
    ];
    instructions.extend(
        (2..=child.height()).map(|height| anchor_instruction(&child, &registration, height)),
    );
    submit(&mut parent, instructions);

    let (directory, manifest, request) = source_fixture(owner);
    let sources = source_ownership::check_sources(
        directory.path(),
        &manifest,
        source_ownership::manifest_digest(&manifest).unwrap(),
        &request,
        SourceReadLimits {
            max_file_bytes: 4096,
            max_total_bytes: 4096,
        },
    )
    .unwrap();
    let parent_state = parent.state().clone();
    let child_state = child.state().clone();
    let parent_cut = parent_state.runtime_retirement_cut().unwrap();
    let child_cuts = [child_state.runtime_retirement_cut().unwrap()];
    let parent_root = original_root(parent_cut.view()).unwrap();
    let child_root = original_root(child_cuts[0].view()).unwrap();
    let proofs: BTreeMap<_, _> = aliases
        .keys()
        .map(|id| {
            (
                *id,
                [&parent, &child].map(|chain| {
                    crate::runtime::retirement::receipt_proof(&chain.state().view(), *id)
                        .unwrap()
                        .unwrap()
                }),
            )
        })
        .collect();
    let index = |root| {
        if root == parent_root {
            0
        } else {
            assert_eq!(
                root, child_root,
                "proof lookup cannot substitute its own root"
            );
            1
        }
    };
    let read = |id, root| {
        proofs
            .get(&id)
            .map(|receipts| receipts[index(root)].clone())
    };
    let mut visited = BTreeSet::new();
    let evidence = verify_source_candidate(&sources, &parent_cut, &child_cuts, &mut |id, root| {
        assert!(
            visited.insert((id, index(root))),
            "every root/alias is visited once"
        );
        Ok(read(id, root))
    })
    .unwrap();
    assert_eq!(evidence.owners(), 1);
    assert_eq!(evidence.aliases(), aliases.len());
    assert_eq!(evidence.roots(), 2);
    assert_eq!(visited.len(), aliases.len() * 2);
    evidence.recheck_sources().unwrap();

    let missing = (*aliases.keys().next().unwrap(), child_root);
    assert!(
        verify_source_candidate(&sources, &parent_cut, &child_cuts, &mut |id, root| {
            Ok(((id, root) != missing).then(|| read(id, root)).flatten())
        })
        .is_err(),
        "one missing child alias must refuse the complete join"
    );
    assert!(
        verify_source_candidate(&sources, &parent_cut, &[], &mut |_, _| {
            panic!("missing registered root refuses before receipt lookup")
        })
        .is_err()
    );

    advance(&mut child);
    let new_child_cuts = [child_state.runtime_retirement_cut().unwrap()];
    assert!(
        verify_source_candidate(&sources, &parent_cut, &new_child_cuts, &mut |_, _| {
            panic!("unanchored child cut refuses before receipt lookup")
        })
        .is_err()
    );
    submit(
        &mut parent,
        vec![anchor_instruction(&child, &registration, child.height())],
    );
    let new_parent_cut = parent_state.runtime_retirement_cut().unwrap();
    assert!(
        verify_source_candidate(&sources, &new_parent_cut, &child_cuts, &mut |_, _| {
            panic!("stale child cut refuses before receipt lookup")
        })
        .is_err()
    );
    verify_source_candidate(
        &sources,
        &new_parent_cut,
        &new_child_cuts,
        &mut |id, root| Ok(read(id, root)),
    )
    .expect("the actual newly anchored cuts retain their original receipt carriers");

    std::fs::write(directory.path().join("handler.rs"), b"substituted").unwrap();
    assert!(matches!(
        verify_source_candidate(&sources, &new_parent_cut, &new_child_cuts, &mut |_, _| {
            panic!("changed source refuses before receipt acquisition")
        }),
        Err(CandidateError::Sources(
            source_ownership::Error::PreimageChanged(_)
        ))
    ));
}
