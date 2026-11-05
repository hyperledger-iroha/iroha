//! Merged-release qualification: actual selected definitions, unanimous support and cutovers.

use std::collections::BTreeMap;

use mv::storage::StorageReadOnly;

use iroha_data_model::{
    NetworkId,
    runtime::lifecycle::{
        ChangeSetRecordV1, ChangeSetStatusV1, ChangeSetV1, RuntimeNoticeV1, SupportAttestationV1,
        committee_digest,
    },
};

use super::*;
use crate::{
    state::{World, WorldReadOnly},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig, fixture_keys},
};

fn next(
    time: bool,
    network: NetworkId,
    height: u64,
    previous: &RootSelectionV1,
    published: &BTreeMap<DefinitionId, DefinitionDescriptorV1>,
) -> ChangeSetV1 {
    if time {
        lifecycle_activation::from_selection(
            release_catalog().unwrap().clone(),
            previous,
            published,
            network,
            height,
            [0x81; 32],
        )
        .unwrap()
    } else {
        ironwood::activation_change_set_for_selection(
            network, height, [0x82; 32], previous, published,
        )
        .unwrap()
    }
}

/// Only the already-approved record is fixture setup. Admission, readiness, block execution,
/// publication and the all-validator activation gate remain the production path.
fn schedule(chain: &CertifiedTestChain, change: ChangeSetV1, time_ms: u64) {
    let committee: Vec<_> = chain
        .validators()
        .iter()
        .map(|(peer, _)| peer.public_key().clone())
        .collect();
    let notice = RuntimeNoticeV1 {
        blocks: 1,
        ms: 0,
        approval_cushion_ms: 0,
    };
    let view = chain.state().view();
    let id = change_set::admit(
        &change,
        &chain.network_id(),
        chain.height(),
        notice,
        view.world().runtime_selection(),
        &BTreeMap::new(),
        &change_set::PublishedDefinitions(view.world().runtime_definitions()),
    )
    .unwrap();
    drop(view);
    let mut record = ChangeSetRecordV1 {
        change_set: change.clone(),
        status: ChangeSetStatusV1::Scheduled,
        proposed_ms: 0,
        approved_height: chain.height(),
        approved_ms: 0,
        notice,
        attestations: Vec::new(),
    };
    for key in fixture_keys() {
        let signed = SupportAttestationV1 {
            network: change.network,
            root: change.root,
            change_set: id,
            predecessor: change.predecessor,
            successor: change.successor,
            committee: committee_digest(&committee).unwrap(),
            tested_checkpoint: chain.height(),
            valid_from: chain.height(),
            valid_until: change.effective_height + 1,
            release: [0x83; 32],
            validator: key.public_key().clone(),
        }
        .sign(key.private_key())
        .unwrap();
        change_set::record_attestation(&mut record, signed);
    }
    assert!(change_set::ready(
        &record,
        id,
        change.effective_height,
        time_ms,
        &committee
    ));
    chain.setup_world_at(time_ms, |tx| {
        tx.world.runtime_change_sets.insert(id, record);
    });
}

#[test]
fn merged_both_orders_activate_through_committed_selection_and_unanimous_support() {
    let catalog = release_catalog().unwrap();
    let mut final_selections = Vec::new();
    for time_first in [false, true] {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::default(), 1_000)).unwrap();
        let mut published: BTreeMap<_, _> = catalog
            .baseline_definitions
            .iter()
            .map(|id| (*id, catalog.definitions[id].clone()))
            .collect();
        let mut previous = catalog.baseline.clone();
        for (step, time) in [time_first, !time_first].into_iter().enumerate() {
            let height = chain.height() + 2;
            let time_ms = 2_000 + step as u64 * 2_000;
            let change = next(time, chain.network_id(), height, &previous, &published);
            let applied = change.apply(&previous, &published).unwrap();
            assert!(catalog.unsupported(&applied.selection).is_none());
            schedule(&chain, change.clone(), time_ms);
            chain.commit_at(time_ms, Vec::new());
            assert_eq!(chain.height(), height - 1);
            assert_eq!(chain.state().view().world().runtime_selection(), &previous);
            chain.commit_at(time_ms + 1_000, Vec::new());
            let view = chain.state().view();
            assert_eq!(view.world().runtime_selection(), &applied.selection);
            assert_eq!(
                view.world()
                    .runtime_change_sets()
                    .get(&change.id().unwrap())
                    .unwrap()
                    .status,
                ChangeSetStatusV1::Activated(height)
            );
            assert_eq!(
                trigger_lifecycle_enabled(view.world().runtime_selection()).unwrap(),
                time || step == 1
            );
            assert_eq!(
                ironwood::enabled(view.world().runtime_selection()),
                !time || step == 1
            );
            for id in &catalog.baseline_definitions {
                assert_eq!(
                    view.world().runtime_definitions().get(id),
                    Some(&catalog.definitions[id]),
                    "published predecessor definitions must remain byte-identical"
                );
            }
            previous = applied.selection;
            published.extend(applied.published);
        }
        final_selections.push(previous);
    }
    assert_eq!(final_selections[0], final_selections[1]);
}

fn supported_pairs_source() -> String {
    let catalog = release_catalog().unwrap();
    let mut builder = catalog.clone();
    let combined = builder
        .lifecycle_successor(&catalog.ironwood.as_ref().unwrap().selection)
        .unwrap();
    let mut source = String::from(
        "// Generated from the compiled M8 catalog by regenerate_merged_lifecycle_supported_pairs.\n\
         // Changing these exact pairs requires requalification of activation and continuation.\n\
         #[rustfmt::skip]\n\
         const SUPPORTED_LIFECYCLE_PAIRS: [([u8; 32], [u8; 32]); 2] = [\n",
    );
    for selection in [&catalog.lifecycle, &combined] {
        let implementations: std::collections::BTreeSet<_> = selection
            .bindings
            .iter()
            .map(|binding| binding.implementation)
            .collect();
        assert_eq!(implementations.len(), 1);
        source.push_str(&format!(
            "    ({:?}, {:?}),\n",
            selection.services_profile.0,
            implementations.first().unwrap().0
        ));
    }
    source.push_str("];\n");
    source
}

const PAIRS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../python/iroha_python/iroha_python_rs/src/scheduled_lifecycle_supported_pairs.rs"
);

#[test]
fn merged_sdk_pairs_are_exact_compiled_catalog_identities() {
    assert_eq!(
        std::fs::read_to_string(PAIRS).unwrap(),
        supported_pairs_source()
    );
}

#[test]
#[ignore = "explicit maintenance capture of supported merged SDK identity pairs"]
fn regenerate_merged_lifecycle_supported_pairs() {
    std::fs::write(PAIRS, supported_pairs_source()).unwrap();
}
