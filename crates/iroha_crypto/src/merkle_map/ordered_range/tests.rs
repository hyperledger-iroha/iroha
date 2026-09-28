//! Mutation and boundary tests for raw canonical Norito-key range witnesses.

use super::*;

fn schema() -> Hash {
    Hash::new(b"ordered-range-test-schema")
}

fn fixture() -> NoritoKeyRangeTreeV1 {
    NoritoKeyRangeTreeV1::from_sorted(
        schema(),
        b"state:table:accounts:v1",
        [
            (b"a".as_slice(), b"value-a".as_slice()),
            (b"c".as_slice(), b"value-c".as_slice()),
            (b"e".as_slice(), b"value-e".as_slice()),
            (b"g".as_slice(), b"value-g".as_slice()),
        ],
    )
    .unwrap()
}

fn verify<'a>(
    proof: &'a NoritoKeyRangeProofV1,
    root: &Hash,
    start: &[u8],
    end: &[u8],
) -> Result<VerifiedNoritoKeyRangeV1<'a>, NoritoKeyRangeError> {
    proof.verify(NoritoKeyRangeVerifyRequestV1 {
        expected_root: root,
        schema_hash: &schema(),
        domain: b"state:table:accounts:v1",
        start,
        end,
        max_rows: MAX_NORITO_RANGE_ROWS,
        max_bytes: MAX_NORITO_RANGE_PROOF_BYTES,
    })
}

#[test]
fn empty_full_value_range_requires_the_complete_proof_header_budget() {
    let tree = NoritoKeyRangeTreeV1::from_sorted(
        schema(),
        b"state:table:accounts:v1",
        std::iter::empty::<(&[u8], &[u8])>(),
    )
    .unwrap();
    let proof = tree.prove_range(b"a", b"z", 0, 12).unwrap();
    for maximum in [0, 11, 12] {
        let built = tree.prove_range(b"a", b"z", 0, maximum);
        let verified = proof.verify(NoritoKeyRangeVerifyRequestV1 {
            expected_root: &tree.root(),
            schema_hash: &schema(),
            domain: b"state:table:accounts:v1",
            start: b"a",
            end: b"z",
            max_rows: 0,
            max_bytes: maximum,
        });
        if maximum < 12 {
            assert!(matches!(built, Err(NoritoKeyRangeError::Capacity)));
            assert!(matches!(verified, Err(NoritoKeyRangeError::Capacity)));
        } else {
            assert!(built.is_ok());
            assert!(verified.unwrap().is_empty());
        }
    }
}

#[test]
fn trusted_builder_rows_expose_the_exact_retained_canonical_bytes() {
    let tree = fixture();
    assert_eq!(tree.rows().len(), tree.len());
    assert_eq!(
        tree.rows().collect::<Vec<_>>(),
        vec![
            (b"a".as_slice(), b"value-a".as_slice()),
            (b"c".as_slice(), b"value-c".as_slice()),
            (b"e".as_slice(), b"value-e".as_slice()),
            (b"g".as_slice(), b"value-g".as_slice()),
        ]
    );
}

#[test]
fn builder_and_range_follow_raw_norito_bytes_not_key_hash_order() {
    let mut keys: Vec<Vec<u8>> = (0_u8..=64)
        .map(|value| norito::to_bytes(&value).unwrap())
        .collect();
    keys.sort();
    keys.dedup();
    let pair = keys
        .windows(2)
        .find(|pair| digest_frame(KEY_DOMAIN, &pair[0]) > digest_frame(KEY_DOMAIN, &pair[1]))
        .expect("at least one canonical Norito key pair has inverted hash order");
    let tree = NoritoKeyRangeTreeV1::from_sorted(
        schema(),
        b"state:table:accounts:v1",
        [
            (pair[0].as_slice(), b"left".as_slice()),
            (pair[1].as_slice(), b"right".as_slice()),
        ],
    )
    .unwrap();
    let proof = tree
        .prove_range(&pair[0], &pair[1], 1, MAX_NORITO_RANGE_PROOF_BYTES)
        .unwrap();
    let verified = verify(&proof, &tree.root(), &pair[0], &pair[1]).unwrap();
    assert_eq!(
        verified.rows().collect::<Vec<_>>(),
        vec![(pair[0].as_slice(), b"left".as_slice())]
    );
    assert_eq!(tree.len(), 2);
    assert_eq!(proof.entry_count(), 2);

    let changed_value = NoritoKeyRangeTreeV1::from_sorted(
        schema(),
        b"state:table:accounts:v1",
        [
            (pair[0].as_slice(), b"other".as_slice()),
            (pair[1].as_slice(), b"right".as_slice()),
        ],
    )
    .unwrap();
    let changed_domain = NoritoKeyRangeTreeV1::from_sorted(
        schema(),
        b"state:table:assets:v1",
        [
            (pair[0].as_slice(), b"left".as_slice()),
            (pair[1].as_slice(), b"right".as_slice()),
        ],
    )
    .unwrap();
    let changed_schema = NoritoKeyRangeTreeV1::from_sorted(
        Hash::new(b"other-schema"),
        b"state:table:accounts:v1",
        [
            (pair[0].as_slice(), b"left".as_slice()),
            (pair[1].as_slice(), b"right".as_slice()),
        ],
    )
    .unwrap();
    let changed_count = NoritoKeyRangeTreeV1::from_sorted(
        schema(),
        b"state:table:accounts:v1",
        [(pair[0].as_slice(), b"left".as_slice())],
    )
    .unwrap();
    for changed in [changed_value, changed_domain, changed_schema, changed_count] {
        assert_ne!(tree.root(), changed.root());
    }
}

#[test]
fn complete_interior_rows_require_immediate_boundary_adjacency() {
    let tree = fixture();
    let proof = tree
        .prove_range(b"b", b"g", 2, MAX_NORITO_RANGE_PROOF_BYTES)
        .unwrap();
    let verified = verify(&proof, &tree.root(), b"b", b"g").unwrap();
    assert_eq!(
        verified.rows().collect::<Vec<_>>(),
        vec![
            (b"c".as_slice(), b"value-c".as_slice()),
            (b"e".as_slice(), b"value-e".as_slice())
        ]
    );

    let mut omitted_first = proof.clone();
    omitted_first.rows.remove(0);
    assert_eq!(
        verify(&omitted_first, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );

    let mut omitted_last = proof.clone();
    omitted_last.rows.pop();
    assert_eq!(
        verify(&omitted_last, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );

    let mut reordered = proof.clone();
    reordered.rows.swap(0, 1);
    assert_eq!(
        verify(&reordered, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );

    let mut duplicated = proof.clone();
    duplicated.rows.insert(1, duplicated.rows[0].clone());
    assert_eq!(
        verify(&duplicated, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );

    let mut missing_before = proof.clone();
    missing_before.before = None;
    assert_eq!(
        verify(&missing_before, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );

    let mut missing_after = proof.clone();
    missing_after.after = None;
    assert_eq!(
        verify(&missing_after, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );

    assert_eq!(
        verify(&proof, &tree.root(), b"b", b"e").err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );
    assert_eq!(
        verify(&proof, &tree.root(), b"a", b"h").err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );
}

#[test]
fn empty_intervals_and_table_edges_are_authenticated() {
    let tree = fixture();
    for (start, end) in [
        (b"b".as_slice(), b"c".as_slice()),
        (b"0".as_slice(), b"a".as_slice()),
        (b"x".as_slice(), b"z".as_slice()),
    ] {
        let proof = tree
            .prove_range(start, end, 0, MAX_NORITO_RANGE_PROOF_BYTES)
            .unwrap();
        let verified = verify(&proof, &tree.root(), start, end).unwrap();
        assert!(verified.is_empty());
        assert_eq!(verified.len(), 0);
    }

    let empty = NoritoKeyRangeTreeV1::from_sorted(
        schema(),
        b"state:table:accounts:v1",
        std::iter::empty::<(&[u8], &[u8])>(),
    )
    .unwrap();
    assert!(empty.is_empty());
    let proof = empty.prove_range(b"a", b"z", 0, 12).unwrap();
    assert!(
        verify(&proof, &empty.root(), b"a", b"z")
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        verify(&proof, &tree.root(), b"a", b"z").err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );

    let mut fabricated = proof;
    fabricated.entries = 1;
    assert_eq!(
        verify(&fabricated, &empty.root(), b"a", b"z").err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );
}

#[test]
fn forged_path_value_key_index_root_schema_and_domain_reject() {
    let tree = fixture();
    let proof = tree
        .prove_range(b"b", b"g", 2, MAX_NORITO_RANGE_PROOF_BYTES)
        .unwrap();
    let wrong_root = Hash::new(b"wrong finalized root");
    assert_eq!(
        verify(&proof, &wrong_root, b"b", b"g").err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );
    assert_eq!(
        proof
            .verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &tree.root(),
                schema_hash: &Hash::new(b"wrong schema"),
                domain: b"state:table:accounts:v1",
                start: b"b",
                end: b"g",
                max_rows: 2,
                max_bytes: MAX_NORITO_RANGE_PROOF_BYTES
            })
            .err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );
    assert_eq!(
        proof
            .verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &tree.root(),
                schema_hash: &schema(),
                domain: b"state:table:assets:v1",
                start: b"b",
                end: b"g",
                max_rows: 2,
                max_bytes: MAX_NORITO_RANGE_PROOF_BYTES
            })
            .err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );

    let mut wrong_value = proof.clone();
    wrong_value.rows[0].value[0] ^= 1;
    assert_eq!(
        verify(&wrong_value, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );

    let mut wrong_key = proof.clone();
    wrong_key.rows[0].key = b"d".to_vec();
    assert_eq!(
        verify(&wrong_key, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );

    let mut wrong_index = proof.clone();
    wrong_index.rows[0].index += 1;
    assert_eq!(
        verify(&wrong_index, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );

    let mut wrong_path = proof.clone();
    wrong_path.rows[0].siblings[0] = Hash::new(b"forged sibling");
    assert_eq!(
        verify(&wrong_path, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );

    let mut short_path = proof.clone();
    short_path.rows[0].siblings.pop();
    assert_eq!(
        verify(&short_path, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::InvalidProof)
    );

    let mut wrong_boundary = proof.clone();
    wrong_boundary.before.as_mut().unwrap().value_digest = Hash::new(b"forged boundary value");
    assert_eq!(
        verify(&wrong_boundary, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );

    let mut wrong_boundary_index = proof.clone();
    wrong_boundary_index.before.as_mut().unwrap().index = 2;
    assert!(verify(&wrong_boundary_index, &tree.root(), b"b", b"g").is_err());

    let mut wrong_successor_path = proof;
    wrong_successor_path.after.as_mut().unwrap().siblings[0] = Hash::new(b"forged successor path");
    assert_eq!(
        verify(&wrong_successor_path, &tree.root(), b"b", b"g").err(),
        Some(NoritoKeyRangeError::RootMismatch)
    );
}

fn assert_builder_order_and_frame_limits(domain: &[u8]) {
    assert_eq!(
        NoritoKeyRangeTreeV1::from_sorted(
            schema(),
            domain,
            [
                (b"b".as_slice(), b"1".as_slice()),
                (b"a".as_slice(), b"2".as_slice())
            ]
        )
        .err(),
        Some(NoritoKeyRangeError::UnsortedKeys)
    );
    assert_eq!(
        NoritoKeyRangeTreeV1::from_sorted(
            schema(),
            domain,
            [
                (b"a".as_slice(), b"1".as_slice()),
                (b"a".as_slice(), b"2".as_slice())
            ]
        )
        .err(),
        Some(NoritoKeyRangeError::UnsortedKeys)
    );
    assert_eq!(
        NoritoKeyRangeTreeV1::from_sorted(schema(), b"", std::iter::empty::<(&[u8], &[u8])>())
            .err(),
        Some(NoritoKeyRangeError::InvalidDomain)
    );
    let oversized_key = vec![0; MAX_NORITO_KEY_BYTES + 1];
    assert_eq!(
        NoritoKeyRangeTreeV1::from_sorted(
            schema(),
            domain,
            [(oversized_key.as_slice(), b"1".as_slice())]
        )
        .err(),
        Some(NoritoKeyRangeError::Capacity)
    );
    let oversized_value = vec![0; MAX_NORITO_VALUE_BYTES + 1];
    assert_eq!(
        NoritoKeyRangeTreeV1::from_sorted(
            schema(),
            domain,
            [(b"a".as_slice(), oversized_value.as_slice())]
        )
        .err(),
        Some(NoritoKeyRangeError::Capacity)
    );
}

#[test]
fn builder_order_frame_and_proof_resource_limits_are_fail_closed() {
    let domain = b"state:table:accounts:v1";
    assert_builder_order_and_frame_limits(domain);

    let tree = fixture();
    assert_eq!(
        tree.prove_range(b"e", b"e", 2, MAX_NORITO_RANGE_PROOF_BYTES)
            .err(),
        Some(NoritoKeyRangeError::InvalidBounds)
    );
    assert_eq!(
        tree.prove_range(b"b", b"g", 1, MAX_NORITO_RANGE_PROOF_BYTES)
            .err(),
        Some(NoritoKeyRangeError::Capacity)
    );
    assert_eq!(
        tree.prove_range(b"b", b"g", 2, 32).err(),
        Some(NoritoKeyRangeError::Capacity)
    );
    assert_eq!(
        tree.prove_range(
            b"b",
            b"g",
            MAX_NORITO_RANGE_ROWS + 1,
            MAX_NORITO_RANGE_PROOF_BYTES
        )
        .err(),
        Some(NoritoKeyRangeError::InvalidLimit)
    );
    let proof = tree
        .prove_range(b"b", b"g", 2, MAX_NORITO_RANGE_PROOF_BYTES)
        .unwrap();
    assert_eq!(
        proof
            .verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &tree.root(),
                schema_hash: &schema(),
                domain,
                start: b"b",
                end: b"g",
                max_rows: 1,
                max_bytes: MAX_NORITO_RANGE_PROOF_BYTES
            })
            .err(),
        Some(NoritoKeyRangeError::Capacity)
    );
    assert_eq!(
        proof
            .verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &tree.root(),
                schema_hash: &schema(),
                domain,
                start: b"b",
                end: b"g",
                max_rows: 2,
                max_bytes: 32
            })
            .err(),
        Some(NoritoKeyRangeError::Capacity)
    );
    assert_eq!(
        proof
            .verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &tree.root(),
                schema_hash: &schema(),
                domain,
                start: b"g",
                end: b"b",
                max_rows: 2,
                max_bytes: MAX_NORITO_RANGE_PROOF_BYTES
            })
            .err(),
        Some(NoritoKeyRangeError::InvalidBounds)
    );
    let mut bogus = Vec::<u8>::new();
    assert_eq!(
        reserve_exact(&mut bogus, usize::MAX),
        Err(NoritoKeyRangeError::Allocation)
    );
}

#[test]
fn retained_builder_entries_grow_geometrically_within_the_admitted_ceiling() {
    let mut entries = Vec::<u8>::new();
    let mut growths = 0;
    for _ in 0..MAX_NORITO_TREE_ENTRIES {
        if entries.len() == entries.capacity() {
            growths += 1;
        }
        reserve_next_entry(&mut entries, MAX_NORITO_TREE_ENTRIES).unwrap();
        entries.push(0);
    }
    assert!(growths <= 17, "amortized growth avoids per-row copying");
    assert_eq!(
        reserve_next_entry(&mut entries, MAX_NORITO_TREE_ENTRIES),
        Err(NoritoKeyRangeError::Capacity)
    );
}
