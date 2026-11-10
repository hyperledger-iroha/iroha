//! Focused construction, incremental-write and selected-state-read comparison.
//! Run with `cargo bench -p iroha_data_model --bench keyed_state`.
use std::{hint::black_box, time::Instant};

use iroha_allocation::AllocationBudget;
use iroha_crypto::Hash;
use iroha_data_model::sumeragi_finality::{
    WORLD_STATE_ACCUMULATOR_LANES_V1, WorldStateElementKindV1, WorldStateSnapshotEntryV1,
    WorldStateSnapshotV1,
    keyed_state::{KeyedStateBuilderV1, KeyedStateTreeV1, StateKeyV1},
    world_state_element_v1, world_state_path_hash_v1, world_state_root_from_accumulator_v1,
};

const TABLE: &str = "world.assets";
type Lanes = [u16; WORLD_STATE_ACCUMULATOR_LANES_V1];
type Rows = Vec<(StateKeyV1, [u8; 32])>;

fn rows(n: usize) -> Rows {
    let mut rows: Rows = (0..n)
        .map(|i| {
            let raw = blake3::hash(&(i as u64).to_le_bytes());
            (
                StateKeyV1 {
                    table: TABLE.into(),
                    key: raw.as_bytes().to_vec(),
                },
                *blake3::hash(raw.as_bytes()).as_bytes(),
            )
        })
        .collect();
    rows.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    rows
}

// Inputs to both constructions are the same already encoded raw bytes. The
// LtHash algorithm hashes these bytes with the native bare Norito length suffix.
fn old_hash(bytes: &[u8]) -> Hash {
    Hash::new_from_chunks(&[
        b"iroha:world-net-delta:value:bare-v1\0",
        bytes,
        &(bytes.len() as u64).to_le_bytes(),
    ])
}

fn adjust(lanes: &mut Lanes, path: &Hash, key: &Hash, value: &[u8], insert: bool) {
    let element = world_state_element_v1(path, Some(key), &old_hash(value));
    for (lane, pair) in lanes.iter_mut().zip(element.as_chunks::<2>().0) {
        let n = u16::from_le_bytes(*pair);
        *lane = if insert {
            lane.wrapping_add(n)
        } else {
            lane.wrapping_sub(n)
        };
    }
}

fn old_build(rows: &Rows, path: &Hash) -> Lanes {
    let mut lanes = [0u16; WORLD_STATE_ACCUMULATOR_LANES_V1];
    for (key, value) in rows {
        adjust(&mut lanes, path, &old_hash(&key.key), value, true);
    }
    lanes
}
fn new_build(rows: &Rows, schema: &Hash, pool: &AllocationBudget) -> KeyedStateTreeV1 {
    let mut builder = KeyedStateBuilderV1::new(*schema.as_ref(), pool);
    for (key, val) in rows {
        builder.push(key, val).unwrap();
    }
    builder.finish().unwrap()
}
fn old_snapshot(rows: &Rows, schema: Hash) -> WorldStateSnapshotV1 {
    let mut snap = WorldStateSnapshotV1 {
        schema_hash: schema,
        entries: rows
            .iter()
            .map(|(key, val)| WorldStateSnapshotEntryV1 {
                field_id: TABLE.into(),
                kind: WorldStateElementKindV1::Table,
                key_hash: Some(old_hash(&key.key)),
                value_hash: old_hash(val),
            })
            .collect(),
    };
    snap.entries.sort_unstable_by_key(|e| e.key_hash);
    snap
}
fn median(mut f: impl FnMut(), repeats: usize) -> u128 {
    let mut runs = [0u128; 9];
    f();
    for sample in &mut runs {
        let start = Instant::now();
        for _ in 0..repeats {
            f();
        }
        *sample = start.elapsed().as_nanos() / repeats as u128;
    }
    runs.sort_unstable();
    runs[4]
}

fn report(n: usize) {
    let rows = rows(n);
    let schema = Hash::new(b"keyed-state-bench-schema");
    let path = world_state_path_hash_v1(TABLE, WorldStateElementKindV1::Table).unwrap();
    let pool = AllocationBudget::new(512 * 1024 * 1024);
    let tree = new_build(&rows, &schema, &pool);
    let old = old_build(&rows, &path);
    let retained = pool.reserved_bytes();
    let cold_old = median(
        || {
            let lanes = old_build(&rows, &path);
            black_box(world_state_root_from_accumulator_v1(
                schema, n as u64, &lanes,
            ));
        },
        1,
    );
    let cold_keyed = median(
        || {
            black_box(new_build(&rows, &schema, &pool).root());
        },
        1,
    );
    let mut seq = 0;
    let mut changes = |name: &str, remove: bool| {
        let old_ns = median(
            || {
                seq = (seq + 7919) % n;
                let (key, original) = &rows[seq];
                let mut lanes = old;
                let id = old_hash(&key.key);
                adjust(&mut lanes, &path, &id, original, false);
                if !remove {
                    adjust(&mut lanes, &path, &id, &[71; 32], true);
                }
                black_box(world_state_root_from_accumulator_v1(
                    schema,
                    (n - usize::from(remove)) as u64,
                    &lanes,
                ));
            },
            256,
        );
        let new_ns = median(
            || {
                seq = (seq + 7919) % n;
                let mut changed = tree.clone();
                changed
                    .replace(&rows[seq].0, (!remove).then_some(&[71; 32][..]))
                    .unwrap();
                black_box(changed.root());
            },
            256,
        );
        println!("{n} {name}: LtHash={old_ns} ns, keyed={new_ns} ns");
    };
    println!(
        "{n} cold build+drop: LtHash={cold_old} ns, keyed={cold_keyed} ns; keyed retained bytes={retained}"
    );
    changes("update", false);
    changes("delete", true);
    let selected = [n / 7, n / 3, n / 2, 2 * n / 3, n - 1];
    let mut old_wire = 0;
    let mut new_wire = 0;
    let old_proof = median(
        || {
            let snapshot = old_snapshot(&rows, schema);
            black_box(snapshot.root().unwrap());
            old_wire = black_box(norito::to_bytes(&snapshot).unwrap()).len();
        },
        1,
    );
    let new_proof = median(
        || {
            new_wire = 0;
            for i in selected {
                let key = &rows[i].0;
                let proof = tree.prove(key).unwrap();
                new_wire += norito::to_bytes(&proof).unwrap().len()
                    + key.table.len()
                    + key.key.len()
                    + rows[i].1.len();
            }
            black_box(new_wire);
        },
        64,
    );
    println!(
        "{n} same-cut read, five keys: full snapshot build/check/encode={old_proof} ns, {old_wire} bytes (excludes five original values); keyed prove+encode={new_proof} ns, {new_wire} witness+key/value bytes"
    );
    // Verification starts from already decoded originals and an independently
    // supplied root. For the full snapshot this measures the necessary root
    // check only, so it excludes clone and subsequent typed target lookups.
    let snapshot = old_snapshot(&rows, schema);
    let old_root = world_state_root_from_accumulator_v1(schema, n as u64, &old);
    let root = tree.root();
    let proofs = selected.map(|i| tree.prove(&rows[i].0).unwrap());
    let old_verify = median(
        || assert_eq!(black_box(snapshot.root().unwrap()), old_root),
        1,
    );
    let new_verify = median(
        || {
            for (&i, proof) in selected.iter().zip(&proofs) {
                assert!(black_box(proof.verify(
                    schema.as_ref(),
                    &root,
                    &rows[i].0,
                    Some(&rows[i].1)
                )));
            }
        },
        128,
    );
    println!(
        "{n} decoded proof verification: full World root={old_verify} ns; five exact keyed statements={new_verify} ns"
    );
}
fn main() {
    for n in [1_024, 16_384] {
        report(n);
    }
}
