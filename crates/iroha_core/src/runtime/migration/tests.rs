//! One migration end to end at the level the kernel drives it: three governed change sets, the
//! bounded stages between them, and the write barrier at the end of every block.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    runtime::lifecycle::{
        ChangeSetRecordV1, ChangeSetStatusV1, RuntimeNoticeV1, SupportAttestationV1,
        committee_digest,
    },
};

use super::*;
use crate::{
    runtime::{
        catalog::{
            MigrationStep,
            additions::test_additions::MIGRATED,
            migrations::test_migrations::{FROM, MODULE, SEQUENCES, TO, successor},
            test_catalog,
        },
        change_set::{PublishedDefinitions, activate_due},
    },
    state::{World, WorldReadOnly as _},
};

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x11; 32])))
}

fn validators() -> Vec<KeyPair> {
    (1..=4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect()
}

fn committee() -> Vec<PublicKey> {
    validators()
        .iter()
        .map(|pair| pair.public_key().clone())
        .collect()
}

fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![0x40 + seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

/// The accounts of the test rows, in the key order the stages copy them.
fn accounts() -> Vec<AccountId> {
    let mut accounts: Vec<AccountId> = (1..=5).map(account).collect();
    accounts.sort();
    accounts
}

/// Install the migration's old storage on the root of `world`, as an earlier change set did.
fn install_old_storage(world: &World) {
    let catalog = release_catalog().unwrap();
    let storage = catalog.world_storage[FROM];
    let mut block = world.block();
    block
        .runtime_definitions
        .insert(storage, catalog.definitions[&storage].clone());
    let selection = block.runtime_selection.get_mut();
    selection.installed_storage.push(storage);
    selection.installed_storage.sort_unstable();
    block.commit();
}

/// A World of a release that adds the `MIGRATED` storage and the lifecycle addition and
/// compiles the `SEQUENCES` migration, with the lifecycle State active, the old storage
/// installed and five old rows.
fn world() -> World {
    let world = World::default();
    crate::runtime::activate_additions_for_testing(&world, RUNTIME_LIFECYCLE.module)
        .expect("activate the lifecycle addition");
    install_old_storage(&world);
    let mut block = world.block();
    for (index, account) in accounts().into_iter().enumerate() {
        block
            .tx_sequences
            .insert(account, u64::try_from(index).unwrap() + 1);
    }
    block.commit();
    world
}

/// Record `change` as scheduled and attested by every validator, so it activates at its height.
fn schedule(world: &World, change: &ChangeSetV1) {
    let committee = committee();
    let attestations = validators()
        .iter()
        .map(|pair| {
            SupportAttestationV1 {
                network: change.network,
                root: change.root,
                change_set: change.id().unwrap(),
                predecessor: change.predecessor,
                successor: change.successor,
                committee: committee_digest(&committee).unwrap(),
                tested_checkpoint: 1,
                valid_from: 0,
                valid_until: u64::MAX,
                release: [0x44; 32],
                validator: pair.public_key().clone(),
            }
            .sign(pair.private_key())
            .unwrap()
        })
        .collect::<Vec<_>>();
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
    for attestation in attestations {
        crate::runtime::change_set::record_attestation(&mut record, attestation);
    }
    let mut block = world.block();
    block
        .runtime_change_sets
        .insert(change.id().unwrap(), record);
    block.commit();
}

/// Draft and schedule one step of the migration at `height`.
fn schedule_step(world: &World, step: MigrationStep, height: u64) -> ChangeSetV1 {
    let view = world.view();
    let change = draft_step(&view, network(), MODULE, step, height, [0x55; 32])
        .unwrap_or_else(|error| panic!("draft {step:?}: {error}"));
    drop(view);
    schedule(world, &change);
    change
}

/// Execute the block at `height` as the kernel does: the height's change sets and migration
/// stages in the block-start transaction, then `writes` as the block's own execution, then the
/// write barrier as the block's last World write.
fn run_block(world: &World, height: u64, writes: impl FnOnce(&mut crate::state::WorldBlock<'_>)) {
    let mut block = world.block();
    let mut transaction = block
        .transaction_without_telemetry(iroha_config::parameters::actual::LaneConfig::default(), 0);
    activate_due(&mut transaction, height, 1_000, &committee()).expect("block start");
    run_stages(&mut transaction, height);
    transaction.apply();
    writes(&mut block);
    barrier(&mut block, height);
    block.commit();
}

fn migration_id(world: &World) -> DefinitionId {
    let view = world.view();
    release_catalog()
        .unwrap()
        .migration_definition(MODULE, view.runtime_selection())
        .expect("compiled migration")
        .0
}

fn record(world: &World) -> MigrationRecordV1 {
    let id = migration_id(world);
    world
        .view()
        .runtime_migrations()
        .get(&id)
        .cloned()
        .expect("migration record")
}

fn status_of(world: &World, change: &ChangeSetV1) -> ChangeSetStatusV1 {
    world
        .view()
        .runtime_change_sets()
        .get(&change.id().unwrap())
        .expect("recorded change set")
        .status
}

/// Old rows and successor rows, each sorted by key.
fn rows(world: &World) -> (Vec<(AccountId, u64)>, Vec<(Hash, bool)>) {
    let view = world.view();
    (
        view.tx_sequences()
            .iter()
            .map(|(account, sequence)| (account.clone(), *sequence))
            .collect(),
        view.viral_bonus_paid()
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect(),
    )
}

/// The successor rows an exact copy of `old` holds, sorted by key.
fn copied(old: &[(AccountId, u64)]) -> Vec<(Hash, bool)> {
    let mut copied: Vec<(Hash, bool)> = old
        .iter()
        .map(|(account, sequence)| successor(account, *sequence).unwrap())
        .collect();
    copied.sort();
    copied
}

fn with_migration<T>(body: impl FnOnce() -> T) -> T {
    test_catalog::with_release(&[MIGRATED, RUNTIME_LIFECYCLE], &[SEQUENCES], body)
}

#[test]
fn a_migration_copies_cuts_over_drains_and_retires_its_old_storage() {
    with_migration(|| {
        let world = world();
        let accounts = accounts();
        let catalog = release_catalog().unwrap();
        let (from, to) = (catalog.world_storage[FROM], catalog.world_storage[TO]);
        let installs = |world: &World, storage| {
            world
                .view()
                .runtime_selection()
                .installed_storage
                .binary_search(&storage)
                .is_ok()
        };
        assert!(installs(&world, from) && !installs(&world, to));

        // Start: the successor is installed and every block copies two rows.
        let start = schedule_step(&world, MigrationStep::Start, 10);
        run_block(&world, 10, |_| {});
        assert_eq!(status_of(&world, &start), ChangeSetStatusV1::Activated(10));
        assert!(installs(&world, to));
        let staged = record(&world);
        assert_eq!(
            (staged.status, staged.started_at, staged.rows, staged.stages),
            (MigrationStatusV1::Running, 10, 2, 1)
        );
        let (old, new) = rows(&world);
        assert_eq!(new, copied(&old[..2]), "the first two rows are staged");

        // The barrier mirrors what the block writes at or before the cursor: a changed row and
        // a removed row. A row after the cursor waits for its stage.
        run_block(&world, 11, |block| {
            block.tx_sequences.insert(accounts[0].clone(), 100);
            block.tx_sequences.remove(accounts[1].clone());
            block.tx_sequences.insert(accounts[4].clone(), 7);
        });
        let (old, new) = rows(&world);
        assert_eq!(old.len(), 4);
        assert_eq!(new, copied(&old[..3]), "rows up to the cursor are exact");
        assert_eq!(record(&world).rows, 4);

        // A cutover before the copy is complete lapses.
        let early = schedule_step(&world, MigrationStep::CutOver, 12);
        run_block(&world, 12, |_| {});
        assert_eq!(status_of(&world, &early), ChangeSetStatusV1::Lapsed(12));
        let complete = record(&world);
        assert_eq!(complete.status, MigrationStatusV1::Complete(12));
        assert_eq!(complete.rows, 5);
        let (old, new) = rows(&world);
        assert_eq!(new, copied(&old), "the successor is an exact copy");

        // The barrier keeps holding until the cutover.
        run_block(&world, 13, |block| {
            block.tx_sequences.insert(accounts[2].clone(), 8);
        });
        let (old, new) = rows(&world);
        assert_eq!(new, copied(&old));
        assert_eq!(record(&world).status, MigrationStatusV1::Complete(12));

        // A retirement before the old storage is drained lapses.
        let hasty = schedule_step(&world, MigrationStep::Retire, 14);
        run_block(&world, 14, |_| {});
        assert_eq!(status_of(&world, &hasty), ChangeSetStatusV1::Lapsed(14));
        assert!(installs(&world, from));

        // Cutover: the successor is authoritative and the old rows drain, two a block.
        let cutover = schedule_step(&world, MigrationStep::CutOver, 15);
        run_block(&world, 15, |_| {});
        assert_eq!(
            status_of(&world, &cutover),
            ChangeSetStatusV1::Activated(15)
        );
        assert_eq!(record(&world).status, MigrationStatusV1::CutOver(15));
        let successor_rows = rows(&world).1;
        run_block(&world, 16, |_| {});
        assert_eq!(rows(&world).0.len(), 2);
        assert_eq!(record(&world).status, MigrationStatusV1::CutOver(15));
        run_block(&world, 17, |_| {});
        assert!(rows(&world).0.is_empty());
        assert_eq!(record(&world).status, MigrationStatusV1::Drained(17));
        assert_eq!(
            rows(&world).1,
            successor_rows,
            "draining leaves the successor alone"
        );

        // Retirement: the empty old storage leaves the selection with the migration.
        let migration = migration_id(&world);
        let retire = schedule_step(&world, MigrationStep::Retire, 18);
        run_block(&world, 18, |_| {});
        assert_eq!(status_of(&world, &retire), ChangeSetStatusV1::Activated(18));
        assert!(!installs(&world, from) && installs(&world, to));
        let view = world.view();
        assert!(
            view.runtime_selection()
                .active
                .binary_search(&migration)
                .is_err()
        );
        assert_eq!(
            view.runtime_migrations()
                .get(&migration)
                .map(|record| record.status),
            Some(MigrationStatusV1::Drained(17)),
            "the record stays as the retirement receipt"
        );
        assert_eq!(
            crate::runtime::uninstalled_world_members(view.runtime_selection())
                .unwrap()
                .members,
            ["tx_sequences"],
            "the retired field leaves snapshots"
        );
    });
}

#[test]
fn a_restriction_suspends_a_migration_and_a_stale_write_stops_its_drain() {
    with_migration(|| {
        let world = world();
        let accounts = accounts();
        schedule_step(&world, MigrationStep::Start, 10);
        run_block(&world, 10, |_| {});
        assert_eq!(record(&world).rows, 2);

        // Closing the migration's admission suspends its stages; the barrier keeps holding.
        let migration = migration_id(&world);
        let restrict = |world: &World, restriction: Option<RestrictionV1>| {
            let mut block = world.block();
            let selection = block.runtime_selection.get_mut();
            selection
                .restrictions
                .retain(|entry| entry.definition != migration);
            if let Some(restriction) = restriction {
                selection.restrictions.push(
                    iroha_data_model::runtime::definition::RestrictionEntryV1 {
                        definition: migration,
                        restriction,
                    },
                );
                selection.restrictions.sort_by_key(|entry| entry.definition);
            }
            block.commit();
        };
        restrict(&world, Some(RestrictionV1::AdmissionClosed(11)));
        run_block(&world, 11, |block| {
            block.tx_sequences.insert(accounts[0].clone(), 50);
        });
        let suspended = record(&world);
        assert_eq!((suspended.rows, suspended.stages), (2, 1));
        let (old, new) = rows(&world);
        assert_eq!(new, copied(&old[..2]), "the barrier holds while suspended");

        // Lifting the restriction resumes from the committed cursor.
        restrict(&world, None);
        run_block(&world, 12, |_| {});
        run_block(&world, 13, |_| {});
        assert_eq!(record(&world).status, MigrationStatusV1::Complete(13));

        // After the cutover a write that still reaches the old storage stops the drain.
        restrict(&world, Some(RestrictionV1::CompletionOnly(14)));
        run_block(&world, 14, |_| {});
        assert_eq!(record(&world).status, MigrationStatusV1::CutOver(14));
        run_block(&world, 15, |block| {
            block.tx_sequences.insert(accounts[4].clone(), 9);
        });
        assert_eq!(record(&world).status, MigrationStatusV1::StaleWrite(15));
        let remaining = rows(&world).0.len();
        run_block(&world, 16, |_| {});
        assert_eq!(
            rows(&world).0.len(),
            remaining,
            "a stale write stops the drain"
        );
    });
}

#[test]
fn a_failing_stage_faults_the_migration_without_writing() {
    with_migration(|| {
        let world = world();
        let accounts = accounts();
        // A row no stage accepts lies in the second batch.
        let mut block = world.block();
        block.tx_sequences.insert(accounts[3].clone(), u64::MAX);
        block.commit();
        schedule_step(&world, MigrationStep::Start, 10);
        run_block(&world, 10, |_| {});
        let staged = rows(&world).1;
        run_block(&world, 11, |_| {});
        let faulted = record(&world);
        assert_eq!(faulted.status, MigrationStatusV1::Faulted(11));
        assert_eq!((faulted.rows, faulted.stages), (2, 1));
        assert_eq!(rows(&world).1, staged, "the failing stage wrote nothing");
        run_block(&world, 12, |block| {
            block.tx_sequences.insert(accounts[0].clone(), 3);
        });
        assert_eq!(record(&world).status, MigrationStatusV1::Faulted(11));
        let (old, new) = rows(&world);
        assert_eq!(
            new,
            copied(&old[..2]),
            "the barrier of a faulted migration keeps the staged rows exact"
        );
    });
}

#[test]
fn a_root_without_the_lifecycle_state_admits_no_migration() {
    with_migration(|| {
        let world = World::default();
        install_old_storage(&world);
        let view = world.view();
        let catalog = release_catalog().unwrap();
        assert!(!lifecycle_installed(view.runtime_selection()));
        // The draft is closed over the published definitions, but the root refuses its effect.
        let change = draft_step(&view, network(), MODULE, MigrationStep::Start, 10, [0; 32])
            .expect("a structurally valid start");
        let applied = change
            .apply(
                view.runtime_selection(),
                &PublishedDefinitions(view.runtime_definitions()),
            )
            .expect("the start applies to the baseline");
        let reason = unsupported_transition(
            &change,
            false,
            view.runtime_selection(),
            view.runtime_selection(),
            &applied.selection,
            &catalog.definitions,
        )
        .expect("a root without the engine refuses a migration");
        assert!(
            reason.contains("unimplemented migration execution"),
            "{reason}"
        );
    });
}

#[test]
fn the_change_sets_of_a_migration_are_structurally_bound() {
    with_migration(|| {
        let world = world();
        let catalog = release_catalog().unwrap();
        schedule_step(&world, MigrationStep::Start, 10);
        run_block(&world, 10, |_| {});
        let view = world.view();
        let current = view.runtime_selection().clone();
        let (migration, descriptor) = catalog.migration_definition(MODULE, &current).unwrap();
        let unsupported = |transitions: Vec<SelectionTransitionV1>| {
            let change = ChangeSetV1 {
                network: network(),
                root: iroha_data_model::runtime::lifecycle::RootScopeV1::Universal,
                predecessor: current.id().unwrap(),
                successor: current.id().unwrap(),
                effective_height: 20,
                publish: Vec::new(),
                transitions,
                qualification: [0; 32],
            };
            unsupported_transition(
                &change,
                false,
                &current,
                &current,
                &current,
                &catalog.definitions,
            )
        };
        let from = descriptor.from[0];
        // Storage is retired only with its migration, and a migration is deactivated only by
        // retiring its old storage or by a replacement.
        assert!(
            unsupported(vec![SelectionTransitionV1::RetireStorage(from)])
                .is_some_and(|reason| reason.contains("outside the retirement"))
        );
        assert!(
            unsupported(vec![SelectionTransitionV1::Deactivate(migration)])
                .is_some_and(|reason| reason.contains("deactivated only"))
        );
        assert_eq!(
            unsupported(vec![
                SelectionTransitionV1::RetireStorage(from),
                SelectionTransitionV1::Deactivate(migration),
            ]),
            None
        );
        assert_eq!(
            unsupported(vec![SelectionTransitionV1::Restrict {
                definition: migration,
                restriction: RestrictionKindV1::CompletionOnly,
            }]),
            None,
            "the cutover is structurally an ordinary restriction"
        );
        // An identity cutover's retirements belong to no migration.
        let identity = ChangeSetV1 {
            network: network(),
            root: iroha_data_model::runtime::lifecycle::RootScopeV1::Universal,
            predecessor: current.id().unwrap(),
            successor: current.id().unwrap(),
            effective_height: 20,
            publish: Vec::new(),
            transitions: vec![SelectionTransitionV1::RetireStorage(from)],
            qualification: [0; 32],
        };
        assert_eq!(
            unsupported_transition(
                &identity,
                true,
                &current,
                &current,
                &current,
                &catalog.definitions
            ),
            None
        );
    });
}
