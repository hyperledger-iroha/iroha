//! Scalable external-node range and failure tests.

use super::super::{MAX_NORITO_RANGE_PROOF_BYTES, MAX_NORITO_RANGE_ROWS};
use super::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct Store {
    nodes: BTreeMap<(u8, u64), Hash>,
    writes_remaining: Option<usize>,
}

impl NoritoKeyRangeNodeStoreV1 for Store {
    fn put(&mut self, level: u8, index: u64, hash: Hash) -> Result<(), NoritoKeyRangeError> {
        if let Some(remaining) = &mut self.writes_remaining {
            if *remaining == 0 {
                return Err(NoritoKeyRangeError::NodeStore);
            }
            *remaining -= 1;
        }
        if self.nodes.insert((level, index), hash).is_some() {
            return Err(NoritoKeyRangeError::NodeStore);
        }
        Ok(())
    }

    fn get(&self, level: u8, index: u64) -> Result<Hash, NoritoKeyRangeError> {
        self.nodes
            .get(&(level, index))
            .copied()
            .ok_or(NoritoKeyRangeError::NodeStore)
    }
}

fn source(count: u32) -> Vec<(Vec<u8>, Vec<u8>)> {
    (0..count)
        .map(|index| {
            (
                (index * 2).to_be_bytes().to_vec(),
                index.to_le_bytes().to_vec(),
            )
        })
        .collect()
}

fn frames(rows: &[(Vec<u8>, Vec<u8>)]) -> impl Iterator<Item = (&[u8], &[u8])> {
    rows.iter()
        .map(|(key, value)| (key.as_slice(), value.as_slice()))
}

fn schema() -> Hash {
    Hash::new(b"external-range-test-schema")
}

const DOMAIN: &[u8] = b"world.accounts";

fn verify<'a>(
    proof: &'a NoritoKeyRangeExternalProofV1,
    root: &Hash,
    start: &[u8],
    end: &[u8],
) -> Result<VerifiedNoritoKeyRangeExternalV1<'a>, NoritoKeyRangeError> {
    proof.verify(NoritoKeyRangeVerifyRequestV1 {
        expected_root: root,
        schema_hash: &schema(),
        domain: DOMAIN,
        start,
        end,
        max_rows: MAX_NORITO_RANGE_ROWS,
        max_bytes: MAX_NORITO_RANGE_PROOF_BYTES,
    })
}

#[test]
fn table_beyond_scoped_cap_has_bounded_complete_tail_and_empty_range_proofs() {
    let rows = source(65_537);
    let mut store = Store::default();
    let tree =
        NoritoKeyRangeExternalV1::from_sorted(schema(), DOMAIN, frames(&rows), &mut store).unwrap();
    assert_eq!(tree.len(), 65_537);
    assert!(tree.len() > super::super::MAX_NORITO_TREE_ENTRIES as u64);
    assert!(store.nodes.len() < rows.len() * 3);

    let start = (65_534_u32 * 2).to_be_bytes();
    let end = (65_537_u32 * 2).to_be_bytes();
    let proof = tree
        .prove_range(&store, frames(&rows), &start, &end, 3, 16_384)
        .unwrap();
    assert_eq!(proof.entry_count(), tree.len());
    let verified = verify(&proof, &tree.root(), &start, &end).unwrap();
    assert_eq!(verified.len(), 3);
    assert_eq!(
        verified.rows().collect::<Vec<_>>(),
        rows[65_534..]
            .iter()
            .map(|(key, value)| (key.as_slice(), value.as_slice()))
            .collect::<Vec<_>>()
    );

    let empty_start = (201_u32).to_be_bytes();
    let empty_end = (202_u32).to_be_bytes();
    let empty = tree
        .prove_range(&store, frames(&rows), &empty_start, &empty_end, 0, 16_384)
        .unwrap();
    assert!(
        verify(&empty, &tree.root(), &empty_start, &empty_end)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn external_range_rejects_forged_rows_neighbors_roots_and_resource_excess() {
    let rows = source(9);
    let mut store = Store::default();
    let tree =
        NoritoKeyRangeExternalV1::from_sorted(schema(), DOMAIN, frames(&rows), &mut store).unwrap();
    let start = (4_u32).to_be_bytes();
    let end = (10_u32).to_be_bytes();
    let proof = tree
        .prove_range(&store, frames(&rows), &start, &end, 3, 16_384)
        .unwrap();
    assert_eq!(verify(&proof, &tree.root(), &start, &end).unwrap().len(), 3);

    let mut omitted = proof.clone();
    omitted.rows.remove(1);
    assert_eq!(
        verify(&omitted, &tree.root(), &start, &end).err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );
    let mut forged = proof.clone();
    forged.rows[0].value[0] ^= 1;
    assert_eq!(
        verify(&forged, &tree.root(), &start, &end).err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );
    let mut missing_neighbor = proof.clone();
    missing_neighbor.before = None;
    assert_eq!(
        verify(&missing_neighbor, &tree.root(), &start, &end).err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );
    let mut absent_sibling = proof.clone();
    absent_sibling.rows[0].siblings[0] = None;
    assert_eq!(
        verify(&absent_sibling, &tree.root(), &start, &end).err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );
    assert_eq!(
        verify(&proof, &Hash::new(b"wrong-root"), &start, &end).err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );
    assert_eq!(
        proof
            .verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &tree.root(),
                schema_hash: &Hash::new(b"wrong-schema"),
                domain: DOMAIN,
                start: &start,
                end: &end,
                max_rows: 3,
                max_bytes: 16_384
            })
            .err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );
    assert_eq!(
        proof
            .verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &tree.root(),
                schema_hash: &schema(),
                domain: b"world.assets",
                start: &start,
                end: &end,
                max_rows: 3,
                max_bytes: 16_384
            })
            .err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );
    assert_eq!(
        tree.prove_range(&store, frames(&rows), &start, &end, 2, 16_384)
            .err(),
        Some(NoritoKeyRangeError::Capacity)
    );
    assert_eq!(
        tree.prove_range(&store, frames(&rows), &start, &end, 3, 16)
            .err(),
        Some(NoritoKeyRangeError::Capacity)
    );
}

#[test]
fn failed_staging_and_changed_sources_never_yield_accepted_proofs() {
    let rows = source(5);
    let mut failed = Store {
        writes_remaining: Some(2),
        ..Store::default()
    };
    assert_eq!(
        NoritoKeyRangeExternalV1::from_sorted(schema(), DOMAIN, frames(&rows), &mut failed).err(),
        Some(NoritoKeyRangeError::NodeStore)
    );
    let mut store = Store::default();
    let tree =
        NoritoKeyRangeExternalV1::from_sorted(schema(), DOMAIN, frames(&rows), &mut store).unwrap();
    let mut changed = rows.clone();
    changed[2].1[0] ^= 1;
    let start = (4_u32).to_be_bytes();
    let end = (6_u32).to_be_bytes();
    let forged = tree
        .prove_range(&store, frames(&changed), &start, &end, 1, 16_384)
        .unwrap();
    assert_eq!(
        verify(&forged, &tree.root(), &start, &end).err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );
    changed.swap(1, 2);
    assert_eq!(
        tree.prove_range(&store, frames(&changed), &start, &end, 1, 16_384)
            .err(),
        Some(NoritoKeyRangeError::UnsortedKeys)
    );
}

#[test]
fn exact_lookup_inclusion_absence_and_maximum_key_are_authenticated() {
    let maximal = vec![0xA5; MAX_NORITO_KEY_BYTES];
    let rows = vec![
        (b"a".to_vec(), b"first".to_vec()),
        (maximal.clone(), b"last".to_vec()),
    ];
    let mut store = Store::default();
    let tree =
        NoritoKeyRangeExternalV1::from_sorted(schema(), DOMAIN, frames(&rows), &mut store).unwrap();
    let present = tree
        .prove_lookup(&store, frames(&rows), &maximal, 16_384)
        .unwrap();
    assert_eq!(
        present
            .verify_lookup(&tree.root(), &schema(), DOMAIN, &maximal, 16_384)
            .unwrap(),
        Some(b"last".as_slice())
    );
    let absent = tree
        .prove_lookup(&store, frames(&rows), b"b", 16_384)
        .unwrap();
    assert_eq!(
        absent
            .verify_lookup(&tree.root(), &schema(), DOMAIN, b"b", 16_384)
            .unwrap(),
        None
    );
    // The same immediate neighbors legitimately prove every missing key in
    // their gap, while they cannot turn a present key into an absence proof.
    assert_eq!(
        absent
            .verify_lookup(&tree.root(), &schema(), DOMAIN, b"c", 16_384)
            .unwrap(),
        None
    );
    assert!(
        absent
            .verify_lookup(&tree.root(), &schema(), DOMAIN, b"a", 16_384)
            .is_err()
    );
}

#[test]
fn empty_table_proof_obeys_header_budget_and_table_binding() {
    let rows: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut store = Store::default();
    let tree =
        NoritoKeyRangeExternalV1::from_sorted(schema(), DOMAIN, frames(&rows), &mut store).unwrap();
    assert!(tree.is_empty());
    let proof = tree
        .prove_range(&store, frames(&rows), b"a", b"b", 0, 16)
        .unwrap();
    assert!(
        proof
            .verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &tree.root(),
                schema_hash: &schema(),
                domain: DOMAIN,
                start: b"a",
                end: b"b",
                max_rows: 0,
                max_bytes: 16
            })
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        tree.prove_range(&store, frames(&rows), b"a", b"b", 0, 15)
            .err(),
        Some(NoritoKeyRangeError::Capacity)
    );
    assert_eq!(
        proof
            .verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &tree.root(),
                schema_hash: &schema(),
                domain: DOMAIN,
                start: b"a",
                end: b"b",
                max_rows: 0,
                max_bytes: 15
            })
            .err(),
        Some(NoritoKeyRangeError::Capacity)
    );
    assert_eq!(
        proof
            .verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &tree.root(),
                schema_hash: &schema(),
                domain: b"world.assets",
                start: b"a",
                end: b"b",
                max_rows: 0,
                max_bytes: 16
            })
            .err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );
}

#[test]
fn external_row_budget_counts_u64_indices() {
    let rows = vec![(b"a".to_vec(), b"v".to_vec())];
    let mut store = Store::default();
    let tree =
        NoritoKeyRangeExternalV1::from_sorted(schema(), DOMAIN, frames(&rows), &mut store).unwrap();
    let exact = EXTERNAL_PROOF_HEADER_BYTES + EXTERNAL_ROW_HEADER_BYTES + 2;
    assert_eq!(
        tree.prove_lookup(&store, frames(&rows), b"a", exact - 1)
            .err(),
        Some(NoritoKeyRangeError::Capacity)
    );
    let proof = tree
        .prove_lookup(&store, frames(&rows), b"a", exact)
        .unwrap();
    assert_eq!(
        proof
            .verify_lookup(&tree.root(), &schema(), DOMAIN, b"a", exact)
            .unwrap(),
        Some(b"v".as_slice())
    );
    assert_eq!(
        proof
            .verify_lookup(&tree.root(), &schema(), DOMAIN, b"a", exact - 1)
            .err(),
        Some(NoritoKeyRangeError::Capacity)
    );
}
