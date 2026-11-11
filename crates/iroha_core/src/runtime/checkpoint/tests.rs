//! The committed checkpoint records: which root reads them, what `SetParameter` admits, and
//! what a node checks against its own history before it signs one.

use std::collections::BTreeMap;

use iroha_crypto::Algorithm;
use iroha_data_model::runtime::definition::SelectionId;

use super::*;
use crate::runtime::{catalog::additions::RUNTIME_LIFECYCLE, release_catalog};

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x11; 32])))
}

fn keys() -> Vec<KeyPair> {
    (1..=4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect()
}

fn committee(keys: &[KeyPair]) -> Vec<PublicKey> {
    keys.iter().map(|key| key.public_key().clone()).collect()
}

/// A node's history: the block hash, committed time, certified roots and committee per height.
#[derive(Default)]
struct Store {
    blocks: BTreeMap<u64, ([u8; 32], u64)>,
    world_roots: BTreeMap<u64, [u8; 32]>,
    parent_world_roots: BTreeMap<u64, [u8; 32]>,
    committees: BTreeMap<u64, Vec<PublicKey>>,
}

impl History for Store {
    fn block_hash(&self, height: u64) -> Option<[u8; 32]> {
        self.blocks.get(&height).map(|(hash, _)| *hash)
    }
    fn committed_ms(&self, height: u64) -> Option<u64> {
        self.blocks.get(&height).map(|(_, ms)| *ms)
    }
    fn world_root(&self, height: u64) -> Option<[u8; 32]> {
        self.world_roots.get(&height).copied()
    }
    fn parent_world_root(&self, height: u64) -> Option<[u8; 32]> {
        self.parent_world_roots.get(&height).copied()
    }
    fn committee(&self, height: u64) -> Option<Vec<PublicKey>> {
        self.committees.get(&height).cloned()
    }
}

/// The adoption record of a network that adopted at height 40, and the store that agrees.
fn adoption(keys: &[KeyPair]) -> (AdoptionRecordV1, Store, UpgradeCheckpointV1) {
    let committee = committee(keys);
    let record = AdoptionRecordV1 {
        network: network(),
        height: 40,
        block_hash: [1; 32],
        predecessor_world_root: [2; 32],
        imported_state_root: [3; 32],
        committee: committee_digest(&committee).unwrap(),
        selection: release_catalog().unwrap().baseline_id,
        authorization: [0; 32],
    };
    let store = Store {
        blocks: BTreeMap::from([(40, ([1; 32], 160_000)), (41, ([4; 32], 164_000))]),
        world_roots: BTreeMap::from([(40, [2; 32])]),
        parent_world_roots: BTreeMap::from([(41, [3; 32])]),
        committees: BTreeMap::from([(40, committee)]),
    };
    let configured = UpgradeCheckpointV1 {
        height: 40,
        block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([1; 32])),
    };
    (record, store, configured)
}

#[test]
fn a_node_signs_an_anchor_only_if_its_store_agrees_with_every_field() {
    let keys = keys();
    let committee = committee(&keys);
    let (record, store, configured) = adoption(&keys);
    let sign =
        |record: &AdoptionRecordV1, store: &Store, configured: Option<&UpgradeCheckpointV1>| {
            attest_anchor(store, &network(), configured, &committee, record, &keys[0])
        };
    let signed = sign(&record, &store, Some(&configured)).expect("the store agrees");
    assert!(signed.verify_for(&record).is_ok());
    assert_eq!(signed.attestation.validator, *keys[0].public_key());

    let outsider = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
    assert_eq!(
        attest_anchor(
            &store,
            &network(),
            Some(&configured),
            &committee,
            &record,
            &outsider
        ),
        Err(RecordAttestationError::NotInCommittee)
    );
    let elsewhere =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x12; 32])));
    assert_eq!(
        attest_anchor(
            &store,
            &elsewhere,
            Some(&configured),
            &committee,
            &record,
            &keys[0]
        ),
        Err(RecordAttestationError::Network)
    );
    // The anchor is the checkpoint the node adopted at, which no governance record authorized.
    assert_eq!(
        sign(&record, &store, None),
        Err(RecordAttestationError::NotAdopted)
    );
    let mut another_checkpoint = configured;
    another_checkpoint.height = 39;
    assert_eq!(
        sign(&record, &store, Some(&another_checkpoint)),
        Err(RecordAttestationError::NotAdopted)
    );
    let mut authorized = record;
    authorized.authorization = [7; 32];
    assert_eq!(
        sign(&authorized, &store, Some(&configured)),
        Err(RecordAttestationError::NotAdopted)
    );

    // Every field the committed State cannot decide is checked against the node's history.
    let differs = |change: &dyn Fn(&mut AdoptionRecordV1)| {
        let mut changed = record;
        change(&mut changed);
        matches!(
            sign(&changed, &store, Some(&configured)),
            Err(RecordAttestationError::History(_))
        )
    };
    assert!(differs(&|record| record.predecessor_world_root = [0xee; 32]));
    assert!(differs(&|record| record.imported_state_root = [0xee; 32]));
    assert!(differs(&|record| record.committee = [0xee; 32]));
    assert!(differs(&|record| record.selection = SelectionId([0xee; 32])));
    let mut other_block = store;
    other_block.blocks.insert(40, ([0xdd; 32], 160_000));
    assert!(matches!(
        sign(&record, &other_block, Some(&configured)),
        Err(RecordAttestationError::History(_))
    ));
}

#[test]
fn a_node_derives_the_adoption_record_from_its_own_store() {
    let keys = keys();
    let committee = committee(&keys);
    let (record, store, configured) = adoption(&keys);
    assert_eq!(
        adoption_record(&store, &network(), Some(&configured)),
        Ok(record),
        "every field comes from the store, the configuration and the release"
    );
    // What a node derives it also signs.
    assert!(
        attest_anchor(
            &store,
            &network(),
            Some(&configured),
            &committee,
            &adoption_record(&store, &network(), Some(&configured)).unwrap(),
            &keys[2]
        )
        .is_ok()
    );
    assert_eq!(
        adoption_record(&store, &network(), None),
        Err(RecordAttestationError::NotAdopted)
    );
    let mut another_block = configured;
    another_block.block_hash = HashOf::from_untyped_unchecked(Hash::prehashed([9; 32]));
    assert!(matches!(
        adoption_record(&store, &network(), Some(&another_block)),
        Err(RecordAttestationError::History(_))
    ));
    // A store that does not hold the first foundation block states no record.
    let without_successor = Store {
        blocks: store.blocks.clone(),
        world_roots: store.world_roots.clone(),
        parent_world_roots: BTreeMap::new(),
        committees: store.committees.clone(),
    };
    assert!(matches!(
        adoption_record(&without_successor, &network(), Some(&configured)),
        Err(RecordAttestationError::History(_))
    ));
}

#[test]
fn a_node_signs_a_floor_only_for_a_checkpoint_it_holds() {
    let keys = keys();
    let committee = committee(&keys);
    let (_, store, _) = adoption(&keys);
    let floor = StartupFloorV1 {
        network: network(),
        height: 41,
        block_hash: [4; 32],
        committed_ms: 164_000,
    };
    let sign = |floor: &StartupFloorV1, holds: bool| {
        attest_floor(&store, &network(), &committee, holds, floor, &keys[1])
    };
    let signed = sign(&floor, true).expect("the store agrees and holds the checkpoint");
    assert!(signed.verify_for(&floor).is_ok());
    assert!(matches!(
        sign(&floor, false),
        Err(RecordAttestationError::History(_))
    ));
    let mut another_time = floor;
    another_time.committed_ms += 1;
    assert!(matches!(
        sign(&another_time, true),
        Err(RecordAttestationError::History(_))
    ));
    let mut another_block = floor;
    another_block.block_hash = [5; 32];
    assert!(matches!(
        sign(&another_block, true),
        Err(RecordAttestationError::History(_))
    ));
    let mut unknown = floor;
    unknown.height = 99;
    assert!(matches!(
        sign(&unknown, true),
        Err(RecordAttestationError::History(_))
    ));
}

fn policy() -> RuntimeCheckpointPolicyV1 {
    RuntimeCheckpointPolicyV1 {
        cadence_ms: 15 * 60 * 1000,
        retain_all_ms: DAY_MS,
        retain_spacing_ms: DAY_MS,
        recovery_window_ms: 7 * DAY_MS,
        floor_notice_ms: DAY_MS,
    }
}

fn blank_state() -> crate::state::State {
    crate::state::State::new_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    )
}

fn header(height: u64, committed_ms: u64) -> iroha_data_model::block::BlockHeader {
    iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(height).expect("height"),
        None,
        None,
        committed_ms,
        0,
    )
}

#[test]
fn the_policy_is_read_and_admitted_only_with_the_lifecycle_state() {
    let catalog = release_catalog().unwrap();
    let committed = |policy: CustomParameter| {
        let mut parameters = Parameters::default();
        parameters.set_parameter(iroha_data_model::parameter::Parameter::Custom(policy));
        parameters
    };
    let well_formed = policy().to_custom_parameter().unwrap();
    let malformed = CustomParameter::new(
        RuntimeCheckpointPolicyV1::parameter_id().clone(),
        iroha_primitives::json::Json::new(7_u64),
    );
    let unrelated = CustomParameter::new(
        "another_policy".parse().unwrap(),
        iroha_primitives::json::Json::new(7_u64),
    );
    assert_eq!(
        committed_policy(&catalog.baseline, &committed(well_formed.clone())),
        None
    );

    // The frozen baseline validates nothing under this identity.
    let state = blank_state();
    let mut block = state.block(header(1, 1_000));
    let stx = block.transaction();
    assert_eq!(admit_parameter(&stx, &malformed), Ok(()));
    drop(stx);
    drop(block);

    let state = blank_state();
    crate::runtime::activate_additions_for_testing(&state.world, RUNTIME_LIFECYCLE.module)
        .expect("activate the lifecycle addition");
    let mut block = state.block(header(1, 1_000));
    let stx = block.transaction();
    let selection = stx.world.runtime_selection.get();
    assert_eq!(
        committed_policy(selection, &committed(well_formed.clone())),
        Some(policy())
    );
    assert_eq!(
        committed_policy(selection, &committed(malformed.clone())),
        None,
        "a value the baseline stored is judged when it is read"
    );
    assert!(admit_parameter(&stx, &malformed).is_err());
    assert_eq!(admit_parameter(&stx, &well_formed), Ok(()));
    assert_eq!(admit_parameter(&stx, &unrelated), Ok(()));
}

/// A State that has committed nine blocks, each four seconds after the last, whose root has
/// activated the lifecycle State and whose committee holds `keys`; with the hash of each block
/// by height.
fn chain(keys: &[KeyPair]) -> (crate::state::State, BTreeMap<u64, [u8; 32]>) {
    let mut state = blank_state();
    crate::runtime::activate_additions_for_testing(&state.world, RUNTIME_LIFECYCLE.module)
        .expect("activate the lifecycle addition");
    {
        let mut topology = state.commit_topology.block();
        topology.clear();
        topology.extend(
            committee(keys)
                .into_iter()
                .map(iroha_model_base::peer::PeerId::new),
        );
        topology.commit();
    }
    let mut hashes = BTreeMap::new();
    for height in 1..=9_u8 {
        let hash = HashOf::from_untyped_unchecked(Hash::prehashed([height; 32]));
        state.push_block_hash_for_testing(hash);
        let bytes: &[u8; 32] = hash.as_ref();
        hashes.insert(u64::from(height), *bytes);
    }
    state.assume_da_indexes_hydrated_for_testing();
    (state, hashes)
}

/// The anchor of `network` at height 4 of [`chain`], with the statement of every key.
fn chain_anchor(
    keys: &[KeyPair],
    network: NetworkId,
    hashes: &BTreeMap<u64, [u8; 32]>,
) -> RuntimeAdoptionAnchorV1 {
    let (mut record, _, _) = adoption(keys);
    record.network = network;
    record.height = 4;
    record.block_hash = hashes[&4];
    RuntimeAdoptionAnchorV1 {
        attestations: keys
            .iter()
            .map(|key| {
                RuntimeAdoptionAnchorV1::statement(&record, key.public_key().clone())
                    .unwrap()
                    .sign(key.private_key())
                    .unwrap()
            })
            .collect(),
        record,
    }
}

fn floor_statements(
    keys: &[KeyPair],
    floor: &StartupFloorV1,
) -> Vec<SignedStartupFloorAttestationV1> {
    keys.iter()
        .map(|key| {
            RuntimeStartupFloorV1::statement(floor, key.public_key().clone())
                .unwrap()
                .sign(key.private_key())
                .unwrap()
        })
        .collect()
}

fn boundary(height: u64, hashes: &BTreeMap<u64, [u8; 32]>) -> Option<UpgradeCheckpointV1> {
    Some(UpgradeCheckpointV1 {
        height,
        block_hash: HashOf::from_untyped_unchecked(Hash::prehashed(hashes[&height])),
    })
}

#[test]
fn a_root_ratifies_its_anchor_once_and_raises_its_floor_above_it() {
    let keys = keys();
    let (state, hashes) = chain(&keys);
    // The executing block is the tenth, committed thirty days after the ninth.
    let now = 30 * DAY_MS;
    let mut block = state.block(header(10, now));
    let mut stx = block.transaction();
    let network = stx.network_id;
    assert_eq!(committed_anchor(&stx.world), None);
    assert_eq!(history_boundary(&stx.world, now), None);

    // The anchor names this network and a committed block, and every validator signed it.
    let anchor = chain_anchor(&keys, network, &hashes);
    let foreign = chain_anchor(&keys, self::network(), &hashes);
    let reason = ratify_anchor(&mut stx, foreign).expect_err("the anchor of another network");
    assert!(reason.to_string().contains("another network"), "{reason}");
    let mut ahead = anchor.clone();
    ahead.record.height = 10;
    let reason = ratify_anchor(&mut stx, ahead).expect_err("the executing block is not committed");
    assert!(reason.to_string().contains("not committed"), "{reason}");
    let mut quorum = anchor.clone();
    quorum.attestations.pop();
    assert!(ratify_anchor(&mut stx, quorum).is_err());
    assert_eq!(committed_anchor(&stx.world), None);

    ratify_anchor(&mut stx, anchor.clone()).expect("ratify");
    assert_eq!(committed_anchor(&stx.world), Some(anchor.clone()));
    assert_eq!(history_boundary(&stx.world, now), boundary(4, &hashes));
    let reason = ratify_anchor(&mut stx, anchor.clone()).expect_err("ratified once");
    assert!(reason.to_string().contains("never changes"), "{reason}");

    // No floor rises without a committed policy.
    let floor_at = |height: u64| StartupFloorV1 {
        network,
        height,
        block_hash: hashes[&height],
        committed_ms: height * 4_000,
    };
    let first = floor_at(6);
    let reason = raise_floor(&mut stx, first, floor_statements(&keys, &first))
        .expect_err("no policy is committed");
    assert!(reason.to_string().contains("policy"), "{reason}");
    stx.world
        .set_parameter_for_testing(iroha_data_model::parameter::Parameter::Custom(
            policy().to_custom_parameter().unwrap(),
        ))
        .expect("commit the checkpoint policy");

    // The floor lies above the anchor and comes into force the policy's notice later.
    let at_the_anchor = floor_at(4);
    assert!(
        raise_floor(
            &mut stx,
            at_the_anchor,
            floor_statements(&keys, &at_the_anchor)
        )
        .is_err()
    );
    raise_floor(&mut stx, first, floor_statements(&keys, &first)).expect("raise");
    let raised = latest_floor(&stx.world).expect("the raised floor");
    assert_eq!((raised.floor, raised.effective_ms), (first, now + DAY_MS));
    assert_eq!(
        history_boundary(&stx.world, now + DAY_MS - 1),
        boundary(4, &hashes),
        "the anchor stays the boundary through the notice"
    );
    assert_eq!(
        history_boundary(&stx.world, now + DAY_MS),
        boundary(6, &hashes)
    );
    // While it waits out its notice the root raises no other floor.
    let second = floor_at(8);
    let reason = raise_floor(&mut stx, second, floor_statements(&keys, &second))
        .expect_err("the first floor is pending");
    assert!(reason.to_string().contains("waiting"), "{reason}");
    assert_eq!(
        stx.world
            .runtime_boundaries
            .iter()
            .map(|(height, _)| *height)
            .collect::<Vec<_>>(),
        [4, 6]
    );
    stx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit the World overlay");

    // Once the first floor is in force the root raises the next, and a third supersedes the
    // first: State keeps the anchor, the floor in force and the floor waiting out its notice.
    let later = now + 2 * DAY_MS;
    let mut block = state.block(header(10, later));
    let mut stx = block.transaction();
    let below = floor_at(5);
    assert!(raise_floor(&mut stx, below, floor_statements(&keys, &below)).is_err());
    raise_floor(&mut stx, second, floor_statements(&keys, &second)).expect("raise again");
    assert_eq!(history_boundary(&stx.world, later), boundary(6, &hashes));
    assert_eq!(
        history_boundary(&stx.world, later + DAY_MS),
        boundary(8, &hashes)
    );
    stx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit the World overlay");

    let last = later + 2 * DAY_MS;
    let mut block = state.block(header(10, last));
    let mut stx = block.transaction();
    let third = floor_at(9);
    raise_floor(&mut stx, third, floor_statements(&keys, &third)).expect("raise a third time");
    assert_eq!(
        stx.world
            .runtime_boundaries
            .iter()
            .map(|(height, _)| *height)
            .collect::<Vec<_>>(),
        [4, 8, 9],
        "the superseded floor left State"
    );
    assert_eq!(committed_anchor(&stx.world), Some(anchor));
}
