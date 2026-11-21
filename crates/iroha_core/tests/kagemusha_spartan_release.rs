//! The public release catalog publishes the future module without changing founded roots.
use iroha_core::runtime::release_catalog;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    isi::kagemusha_wallet::{
        KagemushaWalletLedgerV1,
        spartan::{
            ActivateKagemushaSpartanV1 as Activate, InstallKagemushaSpartanProgramsV1 as Install,
            UnloadKagemushaSpartanV1 as Unload,
        },
    },
};

#[test]
fn future_wallet_storage_and_interfaces_require_a_governed_root_change() {
    let catalog = release_catalog().unwrap();
    let baseline = &catalog.baseline;
    let table = catalog.world_storage["world.kagemusha_spartan_programs"];
    assert!(!baseline.installed_storage.contains(&table));
    assert!(!baseline.bindings.iter().any(|b| {
        [Activate::WIRE_ID, Install::WIRE_ID, Unload::WIRE_ID].contains(&b.wire_id.as_str())
    }));
    let network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x35; 32])));
    use iroha_data_model::runtime::definition::{RestrictionEntryV1, RestrictionV1};
    let mut modules = std::collections::BTreeSet::new();
    for mut before in [baseline, &catalog.lifecycle].map(Clone::clone) {
        let predecessor = before
            .bindings
            .iter()
            .find(|b| b.wire_id == KagemushaWalletLedgerV1::WIRE_ID)
            .unwrap()
            .implementation;
        before.restrictions.push(RestrictionEntryV1 {
            definition: predecessor,
            restriction: RestrictionV1::AdmissionClosed(25),
        });
        let draft = |selection: &_| {
            catalog.activation(
                "iroha.kagemusha.spartan.v1",
                selection,
                &catalog.definitions,
                network,
                55,
                [5; 32],
            )
        };
        let successor = draft(&before)
            .unwrap()
            .apply(&before, &catalog.definitions)
            .unwrap()
            .selection;
        assert!(successor.installed_storage.contains(&table));
        let ledger = successor
            .bindings
            .iter()
            .find(|b| b.wire_id == KagemushaWalletLedgerV1::WIRE_ID)
            .unwrap()
            .implementation;
        assert!(
            modules.insert(ledger),
            "each ledger profile has its own authenticated replacement"
        );
        assert_eq!(
            catalog.native_module(&ledger),
            Some("iroha.kagemusha.spartan.v1")
        );
        assert!(successor.restrictions.contains(&RestrictionEntryV1 {
            definition: ledger,
            restriction: RestrictionV1::AdmissionClosed(55),
        }));
        for wire in [Activate::WIRE_ID, Install::WIRE_ID, Unload::WIRE_ID] {
            assert!(
                successor
                    .bindings
                    .iter()
                    .any(|b| b.wire_id == wire && b.implementation == ledger)
            );
        }
        before.restrictions.last_mut().unwrap().restriction = RestrictionV1::AdmissionClosed(56);
        assert!(draft(&before).is_err());
    }
}
