//! Typed source rights, preflight denial and immutable migration access identities.

use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::account::AccountId;

use super::*;
use crate::{
    runtime::{
        ReleaseCatalog,
        catalog::{
            additions::{RUNTIME_LIFECYCLE, test_additions::MIGRATED},
            migrations::{
                MigrationAddition,
                test_migrations::{FROM, SEQUENCES, TO},
            },
        },
    },
    state::World,
};

fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

#[test]
fn point_and_scan_capabilities_read_the_original_overlay_with_distinct_rights() {
    let world = World::default();
    let mut block = world.block();
    let mut keys = [account(1), account(2), account(3)];
    keys.sort();
    for (index, key) in keys.iter().enumerate() {
        block.tx_sequences.insert(key.clone(), index as u64 + 1);
    }
    let mut tx = block.transaction_without_telemetry(Default::default(), 0);
    tx.tx_sequences.insert(keys[1].clone(), 20);
    let point = Source::<PointOnly, _, _> {
        storage: &tx.tx_sequences,
        access: PhantomData,
    };
    assert_eq!(point.get(&keys[1]), Some(&20));
    assert_eq!(point.get(&account(4)), None);
    let scan = Source::<ScanOnly, _, _> {
        storage: &tx.tx_sequences,
        access: PhantomData,
    };
    assert_eq!(
        scan.scan_after(Some(&keys[0]), 1).collect::<Vec<_>>(),
        vec![(&keys[1], &20)],
    );
    assert_eq!(scan.scan_after(None, 0).count(), 0);
    assert_eq!(scan.scan_after(Some(&keys[2]), 2).count(), 0);
}

#[test]
fn adapter_denies_wrong_roles_undeclared_fields_and_multitable_shapes_before_dispatch() {
    for (from, to) in [
        (&["world.accounts"][..], &[TO][..]),
        (&[FROM][..], &["world.domains"][..]),
        (&[TO][..], &[FROM][..]),
        (&[FROM, "world.accounts"][..], &[TO][..]),
    ] {
        assert!(!SEQUENCES.code.matches_storage(from, to));
    }

    // No adapter or callbacks are made executable under another declaration. In particular,
    // there is no late `open` that could fail after stage or barrier writes have started.
    for declaration in [
        MigrationAddition {
            from: &["world.accounts"],
            ..SEQUENCES
        },
        MigrationAddition {
            to: &["world.domains"],
            ..SEQUENCES
        },
        MigrationAddition {
            from: &[TO],
            to: &[FROM],
            ..SEQUENCES
        },
        MigrationAddition {
            from: &[FROM, "world.accounts"],
            ..SEQUENCES
        },
    ] {
        let error = ReleaseCatalog::build_with(&[MIGRATED, RUNTIME_LIFECYCLE], &[declaration])
            .expect_err("unsupported binding must never reach a callback");
        assert!(
            error
                .to_string()
                .contains("registered typed migration adapter")
        );
    }
}

fn no_stage<A>(
    _: StageStorage<'_, '_, A, AccountId, u64, Hash, bool>,
    cursor: &[u8],
    _: u32,
) -> Result<StageOutcome, String> {
    Ok(StageOutcome {
        cursor: cursor.to_vec(),
        rows: 0,
        done: true,
    })
}

fn no_barrier(_: BarrierStorage<'_, '_, AccountId, u64, Hash, bool>, _: &[u8]) {}
fn no_drain(source: SourceDrain<'_, '_, AccountId, u64>, _: u32) -> DrainOutcome {
    DrainOutcome {
        rows: 0,
        empty: source.is_empty(),
    }
}
fn no_stale(source: SourceChanges<'_, '_, AccountId, u64>) -> bool {
    source.touched_entries().any(|entry| entry.after.is_some())
}

static POINT_PROGRAM: TableMigration<PointOnly, AccountId, u64, Hash, bool> =
    SEQUENCE_TABLES.migration(no_stage, no_barrier, no_drain, no_stale);
static SCAN_PROGRAM: TableMigration<ScanOnly, AccountId, u64, Hash, bool> =
    SEQUENCE_TABLES.migration(no_stage, no_barrier, no_drain, no_stale);

#[test]
fn access_rights_change_only_new_migration_identity_and_preserve_existing_descriptors() {
    let additions = &[MIGRATED, RUNTIME_LIFECYCLE];
    let without = ReleaseCatalog::build_with(additions, &[]).unwrap();
    let point = ReleaseCatalog::build_with(
        additions,
        &[MigrationAddition {
            code: &POINT_PROGRAM,
            ..SEQUENCES
        }],
    )
    .unwrap();
    let scan = ReleaseCatalog::build_with(
        additions,
        &[MigrationAddition {
            code: &SCAN_PROGRAM,
            ..SEQUENCES
        }],
    )
    .unwrap();
    assert_eq!(without.baseline_id, point.baseline_id);
    assert_eq!(without.baseline_id, scan.baseline_id);
    assert_eq!(without.lifecycle_id, point.lifecycle_id);
    assert_eq!(without.lifecycle_id, scan.lifecycle_id);
    for (id, descriptor) in &without.definitions {
        assert_eq!(descriptor, &point.definitions[id]);
        assert_eq!(descriptor, &scan.definitions[id]);
    }
    let point_id = point
        .migration_definition(SEQUENCES.module, &point.lifecycle)
        .unwrap()
        .0;
    let scan_id = scan
        .migration_definition(SEQUENCES.module, &scan.lifecycle)
        .unwrap()
        .0;
    assert_ne!(
        point_id, scan_id,
        "scan authority is part of the code identity"
    );
    assert_ne!(
        POINT_PROGRAM.access_contract(),
        SCAN_PROGRAM.access_contract()
    );
}
