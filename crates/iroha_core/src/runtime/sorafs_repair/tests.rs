//! The exceptional successor stays bounded, immutable and exclusive to its deployment.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    Level,
    isi::{InstructionBox, Log},
    runtime::definition::{DefinitionDescriptorV1, DefinitionId},
};

use super::*;
use crate::{
    runtime::catalog::{ReleaseCatalog, additions},
    smartcontracts::isi::sorafs_bootstrap::tests::Fixture,
    state::StorageReadOnly as _,
};

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x25; 32])))
}

fn baseline_definitions(
    catalog: &ReleaseCatalog,
) -> BTreeMap<DefinitionId, DefinitionDescriptorV1> {
    catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect()
}

#[test]
fn repair_successor_preserves_baseline_and_does_not_activate_other_additions() {
    let catalog = super::super::release_catalog().expect("release catalog");
    let published = baseline_definitions(catalog);
    let original = catalog.baseline.clone();
    let change = catalog
        .sorafs_repair_activation(&original, &published, network(), 42, [0x26; 32])
        .expect("closed repair successor");
    let applied = change
        .apply(&original, &published)
        .expect("apply exact successor");
    let successor = &applied.selection;
    assert_eq!(
        catalog.baseline, original,
        "drafting does not mutate the predecessor"
    );
    assert_eq!(change.predecessor, original.id().expect("predecessor id"));
    assert_eq!(change.successor, successor.id().expect("successor id"));
    assert_ne!(successor.kernel, original.kernel);
    assert_eq!(successor.commitment_profile, original.commitment_profile);
    assert_eq!(successor.services_profile, original.services_profile);
    assert_eq!(successor.installed_storage, original.installed_storage);
    assert_eq!(successor.restrictions, original.restrictions);
    assert_eq!(successor.completion_closures, original.completion_closures);
    assert!(
        original
            .active
            .iter()
            .all(|id| successor.active.contains(id))
    );
    assert!(
        original
            .bindings
            .iter()
            .all(|binding| successor.bindings.contains(binding))
    );
    assert_eq!(successor.active.len(), original.active.len() + 2);
    assert_eq!(successor.bindings.len(), original.bindings.len() + 1);
    assert_eq!(
        change.publish.len(),
        3,
        "one interface, implementation and new kernel"
    );
    assert_eq!(catalog.unsupported(successor), None);
    let repair = catalog
        .additions
        .iter()
        .find(|a| a.module == additions::SORAFS_REPAIR.module)
        .unwrap();
    for other in catalog
        .additions
        .iter()
        .filter(|a| a.module != repair.module)
    {
        for id in other.definitions() {
            assert!(!successor.active.contains(&id));
            assert!(!successor.installed_storage.contains(&id));
            assert!(!applied.published.contains_key(&id));
        }
    }
    let mut all = published.clone();
    all.extend(applied.published);
    for (id, definition) in published {
        assert_eq!(
            all.get(&id),
            Some(&definition),
            "old definitions remain byte-identical"
        );
    }
    successor
        .check_closure(&all)
        .expect("complete successor closure");
}

#[test]
fn repair_implementation_depends_only_on_frozen_baseline_and_its_own_interface() {
    let catalog = super::super::release_catalog().expect("catalog");
    let repair = catalog
        .additions
        .iter()
        .find(|a| a.module == additions::SORAFS_REPAIR.module)
        .unwrap();
    assert!(repair.storage.is_empty());
    assert!(repair.rebinds.is_empty());
    assert_eq!(repair.interfaces.len(), 1);
    let DefinitionDescriptorV1::Implementation(implementation) =
        &catalog.definitions[&repair.implementation]
    else {
        panic!("native repair implementation");
    };
    let baseline_storage: BTreeSet<_> =
        catalog.baseline.installed_storage.iter().copied().collect();
    assert_eq!(
        implementation
            .reads
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        baseline_storage
    );
    assert_eq!(
        implementation
            .writes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        baseline_storage
    );
    assert_eq!(
        implementation
            .scans
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        baseline_storage
    );
    let interfaces: BTreeSet<_> = catalog
        .baseline
        .bindings
        .iter()
        .map(|b| b.interface)
        .chain(repair.interfaces.values().copied())
        .collect();
    assert_eq!(
        implementation
            .invokes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        interfaces
    );
    let pin = include_str!("../../../../../specs/runtime_baseline_catalog.json");
    let id = format!("\"selection\": \"{}\"", catalog.baseline_id);
    assert!(
        pin.contains(&id),
        "the original checked-in baseline selection remains unchanged"
    );
}

#[test]
fn repair_cannot_repeat_or_replace_a_different_kernel() {
    let catalog = super::super::release_catalog().expect("catalog");
    let published = baseline_definitions(catalog);
    let change = catalog
        .sorafs_repair_activation(&catalog.baseline, &published, network(), 42, [0x26; 32])
        .expect("repair");
    let applied = change.apply(&catalog.baseline, &published).expect("apply");
    let mut after = published.clone();
    after.extend(applied.published);
    assert!(
        catalog
            .sorafs_repair_activation(&applied.selection, &after, network(), 43, [0x26; 32])
            .is_err()
    );
    let mut unrelated = catalog.baseline.clone();
    unrelated.kernel = DefinitionId([0xFF; 32]);
    assert!(
        catalog
            .sorafs_repair_activation(&unrelated, &published, network(), 42, [0x26; 32])
            .is_err()
    );
    assert!(
        catalog
            .sorafs_repair_activation(&catalog.baseline, &after, network(), 42, [0x26; 32])
            .is_err(),
        "an already-published repair kernel is not repurposed"
    );
}

#[test]
fn production_boundary_rejects_a_fully_certified_foreign_network() {
    let fixture = Fixture::new();
    let certificate = fixture.certificate(&fixture.payload());
    let view = fixture.chain.state().view();
    let tip = view
        .native_execution_tip()
        .expect("authenticated fixture parent");
    let height = tip.height() + 1;
    assert!(
        validate_fixture_boundary(
            &certificate,
            view.world(),
            Some(tip),
            &view.chain_id,
            view.network_id,
            height,
            tip.creation_time_ms(),
            &certificate.plan.committee
        )
        .is_ok(),
        "the complete common certificate validation must pass before testing the deployment pin"
    );
    let error = validate_boundary(
        &certificate,
        view.world(),
        Some(tip),
        &view.chain_id,
        view.network_id,
        height,
        tip.creation_time_ms(),
        &certificate.plan.committee,
    )
    .expect_err("synthetic chain has no Taira exception authority");
    assert!(matches!(error, ValidationFail::NotPermitted(reason)
        if reason.contains("this exact Taira network only")));
}

#[test]
fn only_the_single_direct_typed_instruction_is_a_repair_entrypoint() {
    let fixture = Fixture::new();
    let repair: InstructionBox = ApplySorafsRepairV1 {
        certificate: fixture.certificate(&fixture.payload()),
    }
    .into();
    let log: InstructionBox = Log::new(Level::INFO, "ordinary instruction".into()).into();
    assert!(sole_repair(&Executable::Instructions(vec![repair.clone()].into())).is_some());
    for instructions in [
        vec![],
        vec![log.clone()],
        vec![log.clone(), repair.clone()],
        vec![repair, log],
    ] {
        assert!(sole_repair(&Executable::Instructions(instructions.into())).is_none());
    }
}

#[test]
fn unsuccessful_exception_keeps_the_exact_opaque_predecessor_rejection() {
    use crate::smartcontracts::isi::sorafs_provider_admission::test_fixture::key;
    use iroha_data_model::{block::BlockHeader, isi::OpaqueInstructionV1};

    let fixture = Fixture::new();
    let valid = fixture.certificate(&fixture.payload());
    let mut malformed = valid.clone();
    malformed.plan.version = 0;
    let mut wrong_signer = valid.clone();
    wrong_signer.signatures[3] = wrong_signer.signatures[0].clone();
    for certificate in [valid, malformed, wrong_signer] {
        let repair = ApplySorafsRepairV1 { certificate };
        let frame = norito::encode_canonical(&repair).expect("typed repair frame");
        let opaque = Executable::Instructions(
            vec![
                OpaqueInstructionV1::new(ApplySorafsRepairV1::WIRE_ID, frame)
                    .expect("well-formed new wire")
                    .into(),
            ]
            .into(),
        );
        let state = std::sync::Arc::clone(fixture.chain.state());
        let view = state.view();
        let tip = view
            .native_execution_tip()
            .expect("genuine certified parent");
        let time = tip.creation_time_ms() + 1;
        let source = fixture.chain.sign(&key(6), [repair.into()], time);
        let header = BlockHeader::new(
            std::num::NonZeroU64::new(tip.height() + 1).expect("next height"),
            view.latest_block_hash(),
            None,
            time,
            0,
        );
        drop(view);
        let mut block = state.block(header);
        let mut tx = block.transaction_for_fastpq_testing(Hash::from(source.hash_as_entrypoint()));
        let before = tx.world.runtime_selection().clone();
        let definitions: Vec<_> = tx
            .world
            .runtime_definitions()
            .iter()
            .map(|(id, definition)| (*id, definition.clone()))
            .collect();
        let old_rejection =
            super::super::envelope::admit_envelopes(&before, tx.block_height(), &opaque)
                .expect_err("a predecessor release keeps this envelope opaque and inactive");
        assert_eq!(
            super::super::envelope::admit_envelopes(
                &before,
                tx.block_height(),
                source.instructions()
            ),
            Err(old_rejection.clone()),
            "known and opaque unbound envelopes have identical results"
        );
        prepare_signed_boundary(&mut tx, &source).expect("failed qualification falls through");
        assert_eq!(tx.world.runtime_selection(), &before);
        assert_eq!(tx.sorafs_repair_entrypoint, None);
        assert_eq!(
            tx.world
                .runtime_definitions()
                .iter()
                .map(|(id, definition)| (*id, definition.clone()))
                .collect::<Vec<_>>(),
            definitions
        );
        assert_eq!(
            super::super::envelope::admit_envelopes(
                tx.world.runtime_selection(),
                tx.block_height(),
                source.instructions()
            ),
            Err(old_rejection),
            "failed qualification must not change the predecessor's committed rejection"
        );
        assert!(!tx.is_execution_deferred());
    }
}
