// Lexically included by `zk_x509::stark::tests` to preserve the existing libtest paths.
fn test_stark_digest_v1(seed: u8) -> GoldilocksDigest384V1 {
    GoldilocksDigest384V1::new([u64::from(seed); 6]).expect("test digest is canonical")
}

fn mutate_stark_digest_v1(digest: &mut GoldilocksDigest384V1) {
    let mut words = digest.words();
    words[0] = F::canonical(words[0])
        .expect("digest lane is canonical")
        .add(F::ONE)
        .value();
    *digest = GoldilocksDigest384V1::new(words).expect("mutated test digest is canonical");
}

#[test]
fn deterministic_projection_proof_roundtrips_and_has_a_protocol_kat() {
    let (statement, _, proof) = projection_fixture();
    verify_zk_x509_projection_segmented_stark_v1(statement, proof).expect("valid projection proof");
    assert!(proof.len() <= ZK_X509_MAX_PROOF_BYTES_V1 as usize);
    assert_eq!(&proof[..4], &PROOF_MAGIC_V1);
    let decoded = decode_projection_fixture();
    assert_eq!(
        exact_encoded_aggregate_proof_bytes_v1(&decoded, &projection_aggregate_layout())
            .expect("exact wire size"),
        proof.len()
    );
    assert!(
        proof.len()
            <= maximum_encoded_aggregate_proof_bytes_v1(&projection_aggregate_layout())
                .expect("maximum wire size")
    );
    assert_eq!(
        encode_zk_x509_segmented_stark_proof_v1(&decoded, &projection_aggregate_layout())
            .expect("canonical re-encode"),
        *proof
    );
    let indices = decoded
        .queries
        .iter()
        .map(|query| query.index)
        .collect::<BTreeSet<_>>();
    assert_eq!(indices.len(), QUERY_COUNT);
    let digest: [u8; 32] = Sha256::digest(proof).into();
    assert_eq!(
        hex::encode(digest),
        "bd7e1827553dcaed27a5b18a4e29a9d5db948aa3a8ad7ace3427e98ee87d279b",
        "update only when the canonical projection proof protocol intentionally changes"
    );
}
#[test]
fn aggregate_proof_shape_rejects_empty_duplicate_and_reordered_group_material() {
    let mut changed = decode_fixture();
    changed.trace_groups.clear();
    assert_rejected(&changed);
    changed = decode_fixture();
    let duplicate_group = changed.trace_groups[0].clone();
    changed.trace_groups.push(duplicate_group);
    assert_rejected(&changed);
    let mut layout = fixture_aggregate_layout();
    layout.registered_segments.clear();
    assert!(layout.validate().is_err());
}
#[test]
fn deterministic_proof_roundtrips_and_has_unique_post_grinding_queries() {
    let (statement, proof) = fixture();
    verify_zk_x509_io_segmented_stark_v1(statement, proof).expect("valid proof");
    assert!(
        ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1
            .windows(b"sha256".len())
            .any(|window| window == b"sha256")
    );
    assert!(
        !ZK_X509_SEGMENTED_STARK_DESCRIPTOR_V1
            .windows(b"poseidon".len())
            .any(|window| window == b"poseidon")
    );
    assert!(proof.len() <= ZK_X509_MAX_PROOF_BYTES_V1 as usize);
    assert_eq!(&proof[..4], &PROOF_MAGIC_V1);
    let decoded = decode_fixture();
    assert_eq!(
        exact_encoded_aggregate_proof_bytes_v1(&decoded, &fixture_aggregate_layout())
            .expect("exact wire size"),
        proof.len()
    );
    assert!(
        proof.len()
            <= maximum_encoded_aggregate_proof_bytes_v1(&fixture_aggregate_layout())
                .expect("maximum wire size")
    );
    assert_eq!(
        encode_zk_x509_segmented_stark_proof_v1(&decoded, &fixture_aggregate_layout())
            .expect("canonical re-encode"),
        *proof
    );
    let indices = decoded
        .queries
        .iter()
        .map(|query| query.index)
        .collect::<BTreeSet<_>>();
    assert_eq!(indices.len(), QUERY_COUNT);
    let digest: [u8; 32] = Sha256::digest(proof).into();
    assert_eq!(
        hex::encode(digest),
        "edb5e8a1e839059f449a818f53eaac0598f6c634ca2e0767a211ad70c4f08bce",
        "update only when the canonical proof protocol intentionally changes"
    );
}
#[test]
fn witness_and_entropy_failures_are_rejected_before_emission() {
    let statement = fixture_statement();
    let mut changed = fixture_witnesses();
    changed[0].consumer_values[0][0] ^= 1;
    let mut rng = StdRng::from_seed([7; 32]);
    assert!(matches!(
        prove_zk_x509_io_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
        Err(ZkX509StarkErrorV1::IoWitness)
    ));
    let mut changed = fixture_witnesses();
    changed[1]
        .declaration
        .public_value
        .as_mut()
        .expect("public")[0] ^= 1;
    assert!(matches!(
        prove_zk_x509_io_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
        Err(ZkX509StarkErrorV1::WitnessStatementMismatch)
    ));
    assert!(matches!(
        prove_zk_x509_io_segmented_stark_v1_with_rng(
            &statement,
            &fixture_witnesses(),
            &mut MaxValueRng
        ),
        Err(ZkX509StarkErrorV1::RandomnessUnavailable)
    ));
}
#[test]
fn exact_wire_rejects_truncation_trailing_magic_version_and_counts() {
    let (statement, proof) = fixture();
    assert!(matches!(
        verify_zk_x509_io_segmented_stark_v1(statement, &[]),
        Err(ZkX509StarkErrorV1::MalformedProof)
    ));
    for length in [1, 4, 7, proof.len() / 4, proof.len() / 2, proof.len() - 1] {
        assert!(
            verify_zk_x509_io_segmented_stark_v1(statement, &proof[..length]).is_err(),
            "prefix length {length} must reject"
        );
    }
    let mut trailing = proof.clone();
    trailing.push(0);
    assert!(verify_zk_x509_io_segmented_stark_v1(statement, &trailing).is_err());
    let mut wrong_magic = proof.clone();
    wrong_magic[0] ^= 1;
    assert!(matches!(
        verify_zk_x509_io_segmented_stark_v1(statement, &wrong_magic),
        Err(ZkX509StarkErrorV1::MalformedProof)
    ));
    let mut wrong_version = proof.clone();
    wrong_version[5] ^= 1;
    assert!(matches!(
        verify_zk_x509_io_segmented_stark_v1(statement, &wrong_version),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let mut wrong_segments = proof.clone();
    wrong_segments[7] ^= 1;
    assert!(matches!(
        verify_zk_x509_io_segmented_stark_v1(statement, &wrong_segments),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    assert!(matches!(
        verify_zk_x509_io_segmented_stark_v1(
            statement,
            &vec![0_u8; ZK_X509_MAX_PROOF_BYTES_V1 as usize + 1]
        ),
        Err(ZkX509StarkErrorV1::ProofTooLarge)
    ));
}
#[test]
fn every_committed_column_family_and_opening_family_is_bound() {
    let mut changed = decode_fixture();
    mutate_stark_digest_v1(&mut changed.trace_groups[0].base_root);
    assert_rejected(&changed);
    changed = decode_fixture();
    mutate_stark_digest_v1(&mut changed.trace_groups[0].aux_root);
    assert_rejected(&changed);
    for lane in 0..SECURITY_LANES {
        changed = decode_fixture();
        mutate_stark_digest_v1(&mut changed.composition_roots[lane]);
        assert_rejected(&changed);
        changed = decode_fixture();
        mutate_stark_digest_v1(&mut changed.fri_lanes[lane].roots[0]);
        assert_rejected(&changed);
        changed = decode_fixture();
        changed.queries[0].composition_values[lane][0][0] ^= 1;
        assert_rejected(&changed);
        changed = decode_fixture();
        mutate_stark_digest_v1(&mut changed.composition_frontiers[lane][0]);
        assert_rejected(&changed);
        changed = decode_fixture();
        changed.queries[0].fri_lanes[lane].rounds[0].low[0] ^= 1;
        assert_rejected(&changed);
        changed = decode_fixture();
        changed.queries[0].fri_lanes[lane].rounds[0].high[0] ^= 1;
        assert_rejected(&changed);
        changed = decode_fixture();
        mutate_stark_digest_v1(&mut changed.fri_lanes[lane].round_frontiers[0][0]);
        assert_rejected(&changed);
    }
    for column in 0..IO_BASE_WIDTH {
        changed = decode_fixture();
        changed.queries[0].trace_groups[0].base_current[column] ^= 1;
        assert_rejected(&changed);
        changed = decode_fixture();
        changed.queries[0].trace_groups[0].base_next[column] ^= 1;
        assert_rejected(&changed);
    }
    for column in 0..IO_AUX_WIDTH {
        changed = decode_fixture();
        changed.queries[0].trace_groups[0].aux_current[column] ^= 1;
        assert_rejected(&changed);
        changed = decode_fixture();
        changed.queries[0].trace_groups[0].aux_next[column] ^= 1;
        assert_rejected(&changed);
    }
    changed = decode_fixture();
    mutate_stark_digest_v1(&mut changed.trace_groups[0].base_frontier[0]);
    assert_rejected(&changed);
    changed = decode_fixture();
    mutate_stark_digest_v1(&mut changed.trace_groups[0].aux_frontier[0]);
    assert_rejected(&changed);
}
#[test]
fn every_deep_value_order_omission_and_replay_is_rejected() {
    let canonical = decode_fixture();
    for column in 0..canonical.deep.trace_groups[0].base_current.len() {
        let mut changed = canonical.clone();
        changed.deep.trace_groups[0].base_current[column][0] ^= 1;
        assert_rejected(&changed);
        let mut changed = canonical.clone();
        changed.deep.trace_groups[0].base_next[column][0] ^= 1;
        assert_rejected(&changed);
    }
    for column in 0..canonical.deep.trace_groups[0].aux_current.len() {
        let mut changed = canonical.clone();
        changed.deep.trace_groups[0].aux_current[column][0] ^= 1;
        assert_rejected(&changed);
        let mut changed = canonical.clone();
        changed.deep.trace_groups[0].aux_next[column][0] ^= 1;
        assert_rejected(&changed);
    }
    for lane in 0..canonical.deep.composition_values.len() {
        for chunk in 0..canonical.deep.composition_values[lane].len() {
            let mut changed = canonical.clone();
            changed.deep.composition_values[lane][chunk][0] ^= 1;
            assert_rejected(&changed);
        }
    }
    let mut reordered = canonical.clone();
    let group = &mut reordered.deep.trace_groups[0];
    core::mem::swap(&mut group.base_current, &mut group.base_next);
    assert_rejected(&reordered);
    let mut reordered = canonical.clone();
    let group = &mut reordered.deep.trace_groups[0];
    core::mem::swap(&mut group.aux_current, &mut group.aux_next);
    assert_rejected(&reordered);
    let mut omitted = canonical.clone();
    omitted.deep.trace_groups[0].base_current.pop();
    assert!(matches!(
        encode_zk_x509_segmented_stark_proof_v1(&omitted, &fixture_aggregate_layout()),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let mut duplicated = canonical.clone();
    let duplicate = duplicated.deep.trace_groups[0].aux_next[0];
    duplicated.deep.trace_groups[0].aux_next.push(duplicate);
    assert!(matches!(
        encode_zk_x509_segmented_stark_proof_v1(&duplicated, &fixture_aggregate_layout()),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let mut replay_rng = StdRng::from_seed([0xC7; 32]);
    let replay_bytes = prove_zk_x509_io_segmented_stark_v1_with_rng(
        &fixture().0,
        &fixture_witnesses(),
        &mut replay_rng,
    )
    .expect("independent masked proof");
    let replay =
        decode_zk_x509_segmented_stark_proof_v1(&replay_bytes, &fixture_aggregate_layout())
            .expect("independent proof decoding");
    let mut spliced = canonical;
    spliced.deep = replay.deep;
    assert_rejected(&spliced);
}
#[test]
fn every_projection_committed_column_and_opening_family_is_bound() {
    let mut changed = decode_projection_fixture();
    mutate_stark_digest_v1(&mut changed.trace_groups[0].base_root);
    assert_projection_rejected(&changed);
    changed = decode_projection_fixture();
    mutate_stark_digest_v1(&mut changed.trace_groups[0].aux_root);
    assert_projection_rejected(&changed);
    for lane in 0..SECURITY_LANES {
        changed = decode_projection_fixture();
        mutate_stark_digest_v1(&mut changed.composition_roots[lane]);
        assert_projection_rejected(&changed);
        changed = decode_projection_fixture();
        mutate_stark_digest_v1(&mut changed.fri_lanes[lane].roots[0]);
        assert_projection_rejected(&changed);
        changed = decode_projection_fixture();
        changed.queries[0].composition_values[lane][0][0] ^= 1;
        assert_projection_rejected(&changed);
        changed = decode_projection_fixture();
        mutate_stark_digest_v1(&mut changed.composition_frontiers[lane][0]);
        assert_projection_rejected(&changed);
        changed = decode_projection_fixture();
        changed.queries[0].fri_lanes[lane].rounds[0].low[0] ^= 1;
        assert_projection_rejected(&changed);
        changed = decode_projection_fixture();
        changed.queries[0].fri_lanes[lane].rounds[0].high[0] ^= 1;
        assert_projection_rejected(&changed);
        changed = decode_projection_fixture();
        mutate_stark_digest_v1(&mut changed.fri_lanes[lane].round_frontiers[0][0]);
        assert_projection_rejected(&changed);
    }
    for column in 0..ZK_X509_PROJECTION_BASE_WIDTH_V1 {
        changed = decode_projection_fixture();
        changed.queries[0].trace_groups[0].base_current[column] ^= 1;
        assert_projection_rejected(&changed);
        changed = decode_projection_fixture();
        changed.queries[0].trace_groups[0].base_next[column] ^= 1;
        assert_projection_rejected(&changed);
    }
    for column in 0..ZK_X509_PROJECTION_AUX_WIDTH_V1 {
        changed = decode_projection_fixture();
        changed.queries[0].trace_groups[0].aux_current[column] ^= 1;
        assert_projection_rejected(&changed);
        changed = decode_projection_fixture();
        changed.queries[0].trace_groups[0].aux_next[column] ^= 1;
        assert_projection_rejected(&changed);
    }
    changed = decode_projection_fixture();
    mutate_stark_digest_v1(&mut changed.trace_groups[0].base_frontier[0]);
    assert_projection_rejected(&changed);
    changed = decode_projection_fixture();
    mutate_stark_digest_v1(&mut changed.trace_groups[0].aux_frontier[0]);
    assert_projection_rejected(&changed);
}
#[test]
fn invalid_projection_witnesses_and_entropy_fail_before_emission() {
    let (statement, witness) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
    let mut rng = StdRng::from_seed([0x31; 32]);
    let mut changed = witness.clone();
    changed.chain_spki_der[0][0] ^= 1;
    assert!(matches!(
        prove_zk_x509_projection_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
        Err(ZkX509StarkErrorV1::ProjectionWitness)
    ));
    let mut changed = witness.clone();
    changed.leaf_serial[0] ^= 1;
    assert!(matches!(
        prove_zk_x509_projection_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
        Err(ZkX509StarkErrorV1::ProjectionWitness)
    ));
    let mut changed = witness.clone();
    changed.disclosed_attribute_values[0][0] ^= 1;
    assert!(matches!(
        prove_zk_x509_projection_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
        Err(ZkX509StarkErrorV1::ProjectionWitness)
    ));
    let mut changed = witness.clone();
    changed.attribute_salts[0][0] ^= 1;
    assert!(matches!(
        prove_zk_x509_projection_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
        Err(ZkX509StarkErrorV1::ProjectionWitness)
    ));
    let mut changed = witness.clone();
    changed.chain_spki_der.pop();
    assert!(matches!(
        prove_zk_x509_projection_segmented_stark_v1_with_rng(&statement, &changed, &mut rng),
        Err(ZkX509StarkErrorV1::ProjectionWitness)
    ));
    assert!(matches!(
        prove_zk_x509_projection_segmented_stark_v1_with_rng(
            &statement,
            &witness,
            &mut MaxValueRng
        ),
        Err(ZkX509StarkErrorV1::RandomnessUnavailable)
    ));
}
#[test]
fn multiproof_frontiers_reject_nonminimal_duplicate_reordered_and_superfluous_siblings() {
    let mut changed = decode_fixture();
    changed.trace_groups[0].base_frontier.pop();
    assert_rejected(&changed);
    changed = decode_fixture();
    let duplicate = changed.trace_groups[0].base_frontier[0];
    changed.trace_groups[0].base_frontier.push(duplicate);
    assert_rejected(&changed);
    changed = decode_fixture();
    changed.trace_groups[0].base_frontier[1] = changed.trace_groups[0].base_frontier[0];
    assert_rejected(&changed);
    changed = decode_fixture();
    changed.trace_groups[0].base_frontier.swap(0, 1);
    assert_rejected(&changed);
    changed = decode_fixture();
    let superfluous = changed.trace_groups[0].aux_frontier[0];
    changed.trace_groups[0].aux_frontier.insert(0, superfluous);
    assert_rejected(&changed);
    changed = decode_fixture();
    changed.composition_frontiers[0].swap(0, 1);
    assert_rejected(&changed);
    changed = decode_fixture();
    changed.fri_lanes[0].round_frontiers[0].swap(0, 1);
    assert_rejected(&changed);
}
#[test]
fn query_reordering_duplication_grinding_and_composition_mutations_reject() {
    let mut changed = decode_fixture();
    changed.queries.swap(0, 1);
    assert_rejected(&changed);
    changed = decode_fixture();
    changed.queries[1] = changed.queries[0].clone();
    assert_rejected(&changed);
    changed = decode_fixture();
    changed.grinding_nonce ^= 1;
    assert_rejected(&changed);
    changed = decode_fixture();
    mutate_stark_digest_v1(&mut changed.composition_roots[0]);
    assert_rejected(&changed);
    changed = decode_fixture();
    mutate_stark_digest_v1(&mut changed.fri_lanes[0].roots[0]);
    assert_rejected(&changed);
}
#[test]
fn noncanonical_fields_and_terminal_high_degree_reject() {
    let mut changed = decode_fixture();
    changed.queries[0].trace_groups[0].base_current[0] =
        crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
    assert!(matches!(
        encode_zk_x509_segmented_stark_proof_v1(&changed, &fixture_aggregate_layout()),
        Err(ZkX509StarkErrorV1::NonCanonicalField)
    ));
    changed = decode_fixture();
    changed.deep.trace_groups[0].base_current[0][0] =
        crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
    assert!(matches!(
        encode_zk_x509_segmented_stark_proof_v1(&changed, &fixture_aggregate_layout()),
        Err(ZkX509StarkErrorV1::NonCanonicalField)
    ));
    changed = decode_fixture();
    changed.fri_lanes[0].terminal_values[TERMINAL_SIZE - 1][0] ^= 1;
    let terminal =
        aggregate::canonical_fp4_fields_v1(&changed.fri_lanes[0].terminal_values, TERMINAL_SIZE)
            .expect("canonical mutation");
    let tree =
        fri_tree_v1(0, fixture_aggregate_layout().fri_rounds(), &terminal).expect("terminal tree");
    changed.fri_lanes[0].roots[fixture_aggregate_layout().fri_rounds()] = tree.root();
    let bytes = encode_zk_x509_segmented_stark_proof_v1(&changed, &fixture_aggregate_layout())
        .expect("encode high-degree terminal");
    assert!(matches!(
        verify_zk_x509_io_segmented_stark_v1(&fixture().0, &bytes),
        Err(ZkX509StarkErrorV1::FriDegree)
    ));
}
#[test]
fn every_public_topology_and_public_byte_change_rejects_replay() {
    let proof = &fixture().1;
    let mut declarations = fixture().0.declarations.clone();
    declarations[1].public_value.as_mut().expect("public")[0] ^= 1;
    let statement = ZkX509IoStarkStatementV1::new(declarations).expect("valid changed public");
    assert!(verify_zk_x509_io_segmented_stark_v1(&statement, proof).is_err());
    let mut declarations = fixture().0.declarations.clone();
    declarations[0].producer.instance += 1;
    let statement = ZkX509IoStarkStatementV1::new(declarations).expect("valid changed producer");
    assert!(verify_zk_x509_io_segmented_stark_v1(&statement, proof).is_err());
    let mut declarations = fixture().0.declarations.clone();
    declarations[0].consumers[0].instance += 1;
    let statement = ZkX509IoStarkStatementV1::new(declarations).expect("valid changed consumer");
    assert!(verify_zk_x509_io_segmented_stark_v1(&statement, proof).is_err());
}
#[test]
fn every_projection_public_field_and_output_rejects_proof_replay() {
    let (baseline, _, proof) = projection_fixture();
    let mut mutations = Vec::<(&str, IrohaZkX509StarkP256StatementV1)>::new();
    let mut changed = baseline.clone();
    changed.context.network_id = NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x92; 32])),
    );
    mutations.push(("context.network_id", changed));
    let mut changed = baseline.clone();
    changed.context.action_index += 1;
    mutations.push(("context.action_index", changed));
    let mut changed = baseline.clone();
    changed.context.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([0x81; 32]);
    mutations.push(("context.transaction_intent_digest", changed));
    let mut changed = baseline.clone();
    changed.context.parameter_id = PrivacyParameterIdV1::new([0x82; 32]);
    mutations.push(("context.parameter_id", changed));
    let mut changed = baseline.clone();
    changed.context.parameter_digest = PrivacyParameterDigestV1::new([0x83; 32]);
    mutations.push(("context.parameter_digest", changed));
    let mut changed = baseline.clone();
    changed.context.verifier_digest = PrivacyVerifierDigestV1::new([0x84; 32]);
    mutations.push(("context.verifier_digest", changed));
    let mut changed = baseline.clone();
    changed.context.statement_schema_digest = PrivacyStatementSchemaDigestV1::new([0x85; 32]);
    mutations.push(("context.statement_schema_digest", changed));
    let mut changed = baseline.clone();
    changed.context.engine_manifest_digest = PrivacyEngineManifestDigestV1::new([0x86; 32]);
    mutations.push(("context.engine_manifest_digest", changed));
    let mut changed = baseline.clone();
    changed.trust_anchor_id = PrivacyIssuerIdV1::new([0x87; 32]);
    mutations.push(("trust_anchor_id", changed));
    let mut changed = baseline.clone();
    changed.certificate_policy_id = PrivacyPolicyIdV1::new([0x88; 32]);
    mutations.push(("certificate_policy_id", changed));
    let mut changed = baseline.clone();
    changed.trust_anchor_record_digest = PrivacyZkX509TrustAnchorRecordDigestV1::new([0x89; 32]);
    mutations.push(("trust_anchor_record_digest", changed));
    let mut changed = baseline.clone();
    changed.trust_anchor_record_epoch += 1;
    mutations.push(("trust_anchor_record_epoch", changed));
    let mut changed = baseline.clone();
    changed.certificate_policy_record_digest =
        PrivacyZkX509CertificatePolicyRecordDigestV1::new([0x8A; 32]);
    mutations.push(("certificate_policy_record_digest", changed));
    let mut changed = baseline.clone();
    changed.certificate_policy_record_epoch += 1;
    mutations.push(("certificate_policy_record_epoch", changed));
    let mut changed = baseline.clone();
    changed.crl_record_digest = PrivacyZkX509CrlRecordDigestV1::new([0x8B; 32]);
    mutations.push(("crl_record_digest", changed));
    let mut changed = baseline.clone();
    changed.crl_record_epoch += 1;
    mutations.push(("crl_record_epoch", changed));
    let mut changed = baseline.clone();
    changed.subject_public_key_digest = PrivacyCertificateKeyDigestV1::new([0x8C; 32]);
    mutations.push(("subject_public_key_digest", changed));
    let mut changed = baseline.clone();
    changed.ca_membership_root = PrivacyRootV1::new([0x8D; 32]);
    mutations.push(("ca_membership_root", changed));
    let mut changed = baseline.clone();
    changed.ca_membership_root_epoch += 1;
    mutations.push(("ca_membership_root_epoch", changed));
    let mut changed = baseline.clone();
    changed.key_usage.digital_signature =
        (!changed.key_usage.digital_signature.is_required()).into();
    mutations.push(("key_usage.digital_signature", changed));
    let mut changed = baseline.clone();
    changed.key_usage.content_commitment =
        (!changed.key_usage.content_commitment.is_required()).into();
    mutations.push(("key_usage.content_commitment", changed));
    let mut changed = baseline.clone();
    changed.key_usage.key_encipherment = (!changed.key_usage.key_encipherment.is_required()).into();
    mutations.push(("key_usage.key_encipherment", changed));
    let mut changed = baseline.clone();
    changed.key_usage.key_agreement = (!changed.key_usage.key_agreement.is_required()).into();
    mutations.push(("key_usage.key_agreement", changed));
    let mut changed = baseline.clone();
    changed
        .extended_key_usages
        .insert(1, PrivacyX509ExtendedKeyUsageV1::DocumentSigning);
    mutations.push(("extended_key_usages", changed));
    for disclosure in 0..baseline.disclosed_attributes.len() {
        let mut changed = baseline.clone();
        changed.disclosed_attributes[disclosure].index = match disclosure {
            0 => 1,
            _ => 2,
        };
        mutations.push(("disclosed_attributes.index", changed));
        let mut changed = baseline.clone();
        changed.disclosed_attributes[disclosure].attribute_digest =
            PrivacyAttributeDigestV1::new([0x90 + disclosure as u8; 32]);
        mutations.push(("disclosed_attributes.attribute_digest", changed));
    }
    let mut changed = baseline.clone();
    changed.disclosed_attributes.clear();
    mutations.push(("disclosed_attributes.length", changed));
    let mut changed = baseline.clone();
    changed.presentation_not_before_unix_seconds += 1;
    mutations.push(("presentation_not_before_unix_seconds", changed));
    let mut changed = baseline.clone();
    changed.presentation_not_after_unix_seconds -= 1;
    mutations.push(("presentation_not_after_unix_seconds", changed));
    let mut changed = baseline.clone();
    changed.wallet_account = crate::privacy_engines::zk_x509::projection_air::tests::account(0xA1);
    mutations.push(("wallet_account", changed));
    let mut changed = baseline.clone();
    changed.wallet_challenge = PrivacyChallengeV1::new([0x96; 32]);
    mutations.push(("wallet_challenge", changed));
    let mut changed = baseline.clone();
    changed.certificate_nullifier = PrivacyNullifierV1::new([0x97; 32]);
    mutations.push(("certificate_nullifier", changed));
    for (field, statement) in mutations {
        assert!(
            verify_zk_x509_projection_segmented_stark_v1(&statement, proof).is_err(),
            "replaying a proof after mutating {field} must reject"
        );
    }
}
#[test]
fn compact_ca_registration_is_single_fixed_capacity_and_fail_closed() {
    assert_eq!(
        checked_compact_ca_degree_capacity_v1(
            ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
            ZK_X509_CA_FRI_LDE_LOG2_V1,
            ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
        )
        .expect("canonical compact-CA capacity"),
        (1_171, 511)
    );
    assert_eq!(
        checked_compact_ca_degree_capacity_v1(
            ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
            ZK_X509_CA_FRI_LDE_LOG2_V1,
            2,
        )
        .expect("lower-degree compact-CA capacity"),
        (738, 511)
    );
    for (trace_log2, lde_log2, constraint_degree) in [
        (
            ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1 - 1,
            ZK_X509_CA_FRI_LDE_LOG2_V1,
            ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
        ),
        (
            ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1 + 1,
            ZK_X509_CA_FRI_LDE_LOG2_V1,
            ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
        ),
        (
            ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
            ZK_X509_CA_FRI_LDE_LOG2_V1 - 1,
            ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
        ),
        (
            ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
            ZK_X509_CA_FRI_LDE_LOG2_V1 + 1,
            ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
        ),
        (
            ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
            ZK_X509_CA_FRI_LDE_LOG2_V1,
            1,
        ),
        (
            ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
            ZK_X509_CA_FRI_LDE_LOG2_V1,
            4,
        ),
    ] {
        assert!(
            checked_compact_ca_degree_capacity_v1(trace_log2, lde_log2, constraint_degree,)
                .is_err(),
            "adjacent compact-CA geometry ({trace_log2}, {lde_log2}, \
                 {constraint_degree}) must fail closed"
        );
    }
    assert!(
        checked_segment_degree_capacity_v1(
            ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
            ZK_X509_CA_FRI_LDE_LOG2_V1,
            ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
        )
        .is_err(),
        "compact-CA must not be accepted under the MAIN mask and terminal profile"
    );
    let layout = accumulator_aggregate_layout();
    layout
        .validate_accumulator_registration_v1()
        .expect("canonical compact-CA registration");
    assert_eq!(layout.parameters_v1(), CA_AGGREGATE_PARAMETERS_V1);
    assert_eq!(layout.common_lde_log2, ZK_X509_CA_FRI_LDE_LOG2_V1);
    assert_eq!(layout.fri_rounds(), 5);
    assert_eq!(layout.trace_groups.len(), 1);
    assert_eq!(layout.registered_segments.len(), 1);
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::CaAccumulator, 0)
        .expect("compact-CA registration");
    assert_eq!(registration.segment.active_rows, 13);
    assert_eq!(registration.segment.trace_log2, 7);
    assert_eq!(registration.segment.lde_log2, ZK_X509_CA_FRI_LDE_LOG2_V1);
    assert_eq!(registration.segment.base_width, 695);
    assert_eq!(registration.segment.aux_width, 128);
    assert_eq!(registration.segment.fixed_width, 80);
    assert_eq!(registration.segment.constraint_count, 1_379);
    assert_eq!(registration.segment.constraint_degree, 3);
    assert_eq!(registration.column_chunks, ZK_X509_CA_ACCUMULATOR_CHUNKS_V1);
    assert_eq!(
        layout.trace_groups[0].column_chunks,
        ZK_X509_CA_ACCUMULATOR_CHUNKS_V1
    );
    let mut mutations = Vec::new();
    let mut changed = layout.clone();
    changed.registered_segments[0].segment.instance = 1;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.registered_segments[0].segment.active_rows = 12;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.registered_segments[0].segment.physical_chunks += 1;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.common_lde_log2 -= 1;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.common_lde_log2 += 1;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.registered_segments[0].segment.lde_log2 -= 1;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.registered_segments[0].segment.lde_log2 += 1;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.registered_segments[0].segment.constraint_degree = 4;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.registered_segments[0].segment.trace_log2 += 1;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.registered_segments[0].column_chunks += 1;
    changed.trace_groups[0].column_chunks += 1;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.registered_segments[0].segment.adapter = SegmentAdapterIdV1::Projection;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed
        .registered_segments
        .push(changed.registered_segments[0]);
    mutations.push(changed);
    for (index, mutation) in mutations.iter().enumerate() {
        assert!(
            mutation.validate_accumulator_registration_v1().is_err(),
            "compact-CA mutation {index} must fail closed"
        );
    }
}
#[test]
fn main_provider_registry_is_exactly_six_closed_ordered_groups() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    {
        let mut a = MockMainTraceGroupSourceV1::default();
        let mut b = MockMainTraceGroupSourceV1::default();
        let mut c = MockMainTraceGroupSourceV1::default();
        let mut d = MockMainTraceGroupSourceV1::default();
        let mut e = MockMainTraceGroupSourceV1::default();
        assert!(matches!(
            MainTraceProviderSetV1::new_v1(
                &layout,
                vec![
                    MainTraceGroupProviderV1::TestLog5(&mut a),
                    MainTraceGroupProviderV1::TestLog8(&mut b),
                    MainTraceGroupProviderV1::Log15(&mut c),
                    MainTraceGroupProviderV1::TestLog16(&mut d),
                    MainTraceGroupProviderV1::Log18(&mut e),
                ],
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
    }
    {
        let mut a = MockMainTraceGroupSourceV1::default();
        let mut b = MockMainTraceGroupSourceV1::default();
        let mut c = MockMainTraceGroupSourceV1::default();
        let mut d = MockMainTraceGroupSourceV1::default();
        let mut e = MockMainTraceGroupSourceV1::default();
        let mut f = MockMainTraceGroupSourceV1::default();
        assert!(matches!(
            MainTraceProviderSetV1::new_v1(
                &layout,
                vec![
                    MainTraceGroupProviderV1::TestLog8(&mut a),
                    MainTraceGroupProviderV1::TestLog5(&mut b),
                    MainTraceGroupProviderV1::Log15(&mut c),
                    MainTraceGroupProviderV1::TestLog16(&mut d),
                    MainTraceGroupProviderV1::Log18(&mut e),
                    MainTraceGroupProviderV1::Log19(&mut f),
                ],
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
    }
    {
        let mut a = MockMainTraceGroupSourceV1::default();
        let mut b = MockMainTraceGroupSourceV1::default();
        let mut c = MockMainTraceGroupSourceV1::default();
        let mut d = MockMainTraceGroupSourceV1::default();
        let mut e = MockMainTraceGroupSourceV1::default();
        let mut f = MockMainTraceGroupSourceV1::default();
        assert!(matches!(
            MainTraceProviderSetV1::new_v1(
                &layout,
                vec![
                    MainTraceGroupProviderV1::TestLog5(&mut a),
                    MainTraceGroupProviderV1::TestLog5(&mut b),
                    MainTraceGroupProviderV1::Log15(&mut c),
                    MainTraceGroupProviderV1::TestLog16(&mut d),
                    MainTraceGroupProviderV1::Log18(&mut e),
                    MainTraceGroupProviderV1::Log19(&mut f),
                ],
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
    }
    {
        let mut a = MockMainTraceGroupSourceV1::default();
        let mut b = MockMainTraceGroupSourceV1::default();
        let mut c = MockMainTraceGroupSourceV1::default();
        let mut d = MockMainTraceGroupSourceV1::default();
        let mut e = MockMainTraceGroupSourceV1::default();
        let mut f = MockMainTraceGroupSourceV1::default();
        let mut g = MockMainTraceGroupSourceV1::default();
        assert!(matches!(
            MainTraceProviderSetV1::new_v1(
                &layout,
                vec![
                    MainTraceGroupProviderV1::TestLog5(&mut a),
                    MainTraceGroupProviderV1::TestLog8(&mut b),
                    MainTraceGroupProviderV1::Log15(&mut c),
                    MainTraceGroupProviderV1::TestLog16(&mut d),
                    MainTraceGroupProviderV1::Log18(&mut e),
                    MainTraceGroupProviderV1::Log19(&mut f),
                    MainTraceGroupProviderV1::Log19(&mut g),
                ],
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
    }
    {
        let mut changed = layout.clone();
        changed.common_lde_log2 -= 1;
        let mut a = MockMainTraceGroupSourceV1::default();
        let mut b = MockMainTraceGroupSourceV1::default();
        let mut c = MockMainTraceGroupSourceV1::default();
        let mut d = MockMainTraceGroupSourceV1::default();
        let mut e = MockMainTraceGroupSourceV1::default();
        let mut f = MockMainTraceGroupSourceV1::default();
        assert!(matches!(
            MainTraceProviderSetV1::new_v1(
                &changed,
                mock_main_group_providers_v1([&mut a, &mut b, &mut c, &mut d, &mut e, &mut f,]),
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
    }
    {
        let mut a = MockMainTraceGroupSourceV1::default();
        let mut b = MockMainTraceGroupSourceV1::default();
        let mut c = MockMainTraceGroupSourceV1::default();
        let mut d = MockMainTraceGroupSourceV1::default();
        let mut e = MockMainTraceGroupSourceV1::default();
        let mut f = MockMainTraceGroupSourceV1::default();
        assert!(matches!(
            MainOpenedProviderSetV1::new_v1(
                &layout,
                vec![
                    MainOpenedGroupProviderV1::TestLog5(&mut a),
                    MainOpenedGroupProviderV1::TestLog8(&mut b),
                    MainOpenedGroupProviderV1::TestLog16(&mut c),
                    MainOpenedGroupProviderV1::TestLog15(&mut d),
                    MainOpenedGroupProviderV1::TestLog18(&mut e),
                    MainOpenedGroupProviderV1::TestLog19(&mut f),
                ],
            ),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
    }
}
#[test]
fn main_provider_routes_every_group_column_through_verifier_owned_slices() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let mut log5 = MockMainTraceGroupSourceV1::default();
    let mut log8 = MockMainTraceGroupSourceV1::default();
    let mut log15 = MockMainTraceGroupSourceV1::default();
    let mut log16 = MockMainTraceGroupSourceV1::default();
    let mut log18 = MockMainTraceGroupSourceV1::default();
    let mut log19 = MockMainTraceGroupSourceV1::default();
    let mut providers = MainTraceProviderSetV1::new_v1(
        &layout,
        mock_main_group_providers_v1([
            &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
        ]),
    )
    .expect("exact MAIN providers");
    for group_index in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
        for kind in [MainTraceColumnKindV1::Base, MainTraceColumnKindV1::Aux] {
            let width = match kind {
                MainTraceColumnKindV1::Base => layout.trace_groups[group_index].base_width,
                MainTraceColumnKindV1::Aux => layout.trace_groups[group_index].aux_width,
            };
            if width == 0 {
                assert!(
                    providers
                        .native_group_column_v1(group_index, kind, 0)
                        .is_err()
                );
                continue;
            }
            for column_index in [0, width - 1] {
                let (registration, local_column) = providers
                    .registered_column_v1(group_index, kind, column_index)
                    .expect("verifier-owned slice");
                let column = providers
                    .native_group_column_v1(group_index, kind, column_index)
                    .expect("routed native column");
                assert_eq!(column.len(), registration.segment.trace_size());
                assert_eq!(
                    column[0],
                    mock_main_column_value_v1(
                        registration,
                        local_column,
                        0,
                        matches!(kind, MainTraceColumnKindV1::Aux),
                    )
                );
                assert_eq!(
                    *column.last().expect("nonempty native column"),
                    mock_main_column_value_v1(
                        registration,
                        local_column,
                        registration.segment.trace_size() - 1,
                        matches!(kind, MainTraceColumnKindV1::Aux),
                    )
                );
            }
        }
    }
    assert!(
        providers
            .native_group_column_v1(FULL_PROFILE_TRACE_GROUPS_V1, MainTraceColumnKindV1::Base, 0,)
            .is_err()
    );
    assert!(
        providers
            .native_group_column_v1(
                0,
                MainTraceColumnKindV1::Base,
                layout.trace_groups[0].base_width,
            )
            .is_err()
    );
    drop(providers);
    log19.short_base_column = true;
    let mut providers = MainTraceProviderSetV1::new_v1(
        &layout,
        mock_main_group_providers_v1([
            &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
        ]),
    )
    .expect("short-column adversary registry");
    assert!(
        providers
            .native_group_column_v1(5, MainTraceColumnKindV1::Base, 0)
            .is_err()
    );
    drop(providers);
    log19.short_base_column = false;
    log19.noncanonical_aux_column = true;
    let mut providers = MainTraceProviderSetV1::new_v1(
        &layout,
        mock_main_group_providers_v1([
            &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
        ]),
    )
    .expect("noncanonical-column adversary registry");
    assert!(
        providers
            .native_group_column_v1(5, MainTraceColumnKindV1::Aux, 0)
            .is_err()
    );
}
#[test]
fn main_projection_source_matches_every_direct_native_column() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::Projection, 0)
        .expect("projection registration");
    let (statement, witness) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
    let trace = build_zk_x509_projection_trace_v1(&statement, &witness).expect("projection trace");
    let post_base = projection_provider_post_base_v1(&statement);
    let direct_aux =
        build_zk_x509_projection_aux_trace_v1(&trace.base, &trace.fixed, post_base.projection())
            .expect("direct projection aux");
    let mut projection = MainProjectionTraceGroupSourceV1::for_main_v1(&layout, &statement, &trace)
        .expect("production projection source");
    assert!(
        projection.native_aux_column_v1(registration, 0).is_err(),
        "auxiliary columns must remain unavailable before challenge binding"
    );
    for local_column in 0..ZK_X509_PROJECTION_BASE_WIDTH_V1 {
        assert_eq!(
            projection
                .native_base_column_v1(registration, local_column)
                .expect("production base column"),
            trace
                .base
                .rows
                .iter()
                .map(|row| row[local_column])
                .collect::<Vec<_>>(),
            "base column {local_column}"
        );
    }
    projection
        .bind_challenges_v1(post_base)
        .expect("bind projection challenges");
    for local_column in 0..ZK_X509_PROJECTION_AUX_WIDTH_V1 {
        assert_eq!(
            projection
                .native_aux_column_v1(registration, local_column)
                .expect("production aux column"),
            direct_aux
                .rows
                .iter()
                .map(|row| row[local_column])
                .collect::<Vec<_>>(),
            "aux column {local_column}"
        );
    }
    assert!(projection.bind_challenges_v1(post_base).is_err());
    assert!(
        projection
            .native_base_column_v1(registration, ZK_X509_PROJECTION_BASE_WIDTH_V1)
            .is_err()
    );
    assert!(
        projection
            .native_aux_column_v1(registration, ZK_X509_PROJECTION_AUX_WIDTH_V1)
            .is_err()
    );
    let io_registration = layout
        .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
        .expect("I/O registration");
    assert!(
        projection
            .native_base_column_v1(io_registration, 0)
            .is_err()
    );
    let mut log5 = MockMainTraceGroupSourceV1::default();
    let mut log8 = MockMainTraceGroupSourceV1::default();
    let mut log16 = MockMainTraceGroupSourceV1::default();
    let mut log18 = MockMainTraceGroupSourceV1::default();
    let mut log19 = MockMainTraceGroupSourceV1::default();
    let mut providers = MainTraceProviderSetV1::new_v1(
        &layout,
        vec![
            MainTraceGroupProviderV1::TestLog5(&mut log5),
            MainTraceGroupProviderV1::TestLog8(&mut log8),
            MainTraceGroupProviderV1::Log15(&mut projection),
            MainTraceGroupProviderV1::TestLog16(&mut log16),
            MainTraceGroupProviderV1::Log18(&mut log18),
            MainTraceGroupProviderV1::Log19(&mut log19),
        ],
    )
    .expect("production projection registry");
    for local_column in 0..ZK_X509_PROJECTION_BASE_WIDTH_V1 {
        assert_eq!(
            providers
                .native_group_column_v1(
                    registration.trace_group,
                    MainTraceColumnKindV1::Base,
                    registration.base_start + local_column,
                )
                .expect("routed production base column"),
            trace
                .base
                .rows
                .iter()
                .map(|row| row[local_column])
                .collect::<Vec<_>>()
        );
    }
    for local_column in 0..ZK_X509_PROJECTION_AUX_WIDTH_V1 {
        assert_eq!(
            providers
                .native_group_column_v1(
                    registration.trace_group,
                    MainTraceColumnKindV1::Aux,
                    registration.aux_start + local_column,
                )
                .expect("routed production aux column"),
            direct_aux
                .rows
                .iter()
                .map(|row| row[local_column])
                .collect::<Vec<_>>()
        );
    }
    drop(providers);
    assert!(
        projection
            .aux
            .as_ref()
            .is_some_and(|aux| { aux.rows.iter().flatten().any(|value| *value != F::ZERO) }),
        "the source holds challenge-bound auxiliary witness material"
    );
    projection.zeroize_private_buffers_v1();
    assert!(
        projection.aux.is_none(),
        "challenge-bound auxiliary copies are zeroized before release"
    );
    let mut changed_trace = trace.clone();
    changed_trace.fixed.rows.swap(0, 1);
    assert!(
        MainProjectionTraceGroupSourceV1::for_main_v1(&layout, &statement, &changed_trace).is_err(),
        "prover-native fixed material must equal the verifier compiler"
    );
}
#[test]
fn main_projection_fixed_rows_and_residues_match_direct_common_domain_adapter() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::Projection, 0)
        .expect("projection registration");
    let (statement, _) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
    let post_base = projection_provider_post_base_v1(&statement);
    let challenges = post_base.projection();
    let fixed_rows = compile_zk_x509_projection_stark_fixed_rows_v1(&statement)
        .expect("verifier projection fixed rows");
    let mut fixed_coefficients =
        transpose_array_rows_v1(&fixed_rows).expect("fixed native columns");
    let trace_root = goldilocks_primitive_root_v1(registration.segment.trace_log2)
        .expect("projection native root");
    for coefficients in &mut fixed_coefficients {
        goldilocks_ifft_v1(coefficients, trace_root).expect("fixed interpolation");
    }
    let common_root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common LDE root");
    let evaluate_fixed = |index: usize| {
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(index as u128));
        fixed_coefficients
            .iter()
            .map(|coefficients| {
                coefficients
                    .iter()
                    .rev()
                    .fold(F::ZERO, |value, coefficient| value.mul(x).add(*coefficient))
            })
            .collect::<Vec<_>>()
    };
    let mut source =
        MainProjectionVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
            .expect("verifier-safe projection source");
    let next_stride = layout.trace_groups[registration.trace_group]
        .next_stride(layout.common_lde_log2)
        .expect("projection next stride");
    for query_index in [
        0,
        1,
        (1_usize << registration.segment.lde_log2) - 1,
        layout.common_lde_size() - 1,
    ] {
        let next_query_index = (query_index + next_stride) % layout.common_lde_size();
        let fixed = source
            .verifier_fixed_opening_v1(registration, query_index, next_query_index)
            .expect("sampled projection fixed rows");
        let expected_current = evaluate_fixed(query_index);
        let expected_next = evaluate_fixed(next_query_index);
        assert_eq!(fixed.current.as_slice(), expected_current.as_slice());
        assert_eq!(fixed.next.as_slice(), expected_next.as_slice());
        let base_current = (0..registration.segment.base_width)
            .map(|column| F(u64::try_from(column + 3).expect("small base")))
            .collect::<Vec<_>>();
        let base_next = (0..registration.segment.base_width)
            .map(|column| F(u64::try_from(column + 37).expect("small base")))
            .collect::<Vec<_>>();
        let aux_current = (0..registration.segment.aux_width)
            .map(|column| F(u64::try_from(column + 71).expect("small aux")))
            .collect::<Vec<_>>();
        let aux_next = (0..registration.segment.aux_width)
            .map(|column| F(u64::try_from(column + 109).expect("small aux")))
            .collect::<Vec<_>>();
        let opening = RegisteredOpenedRowsV1 {
            base_current: &base_current,
            base_next: &base_next,
            aux_current: &aux_current,
            aux_next: &aux_next,
        };
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(query_index as u128));
        let production = source
            .constraint_residues_v1(registration, query_index, next_query_index, x, opening)
            .expect("production projection residues");
        let direct = projection_constraint_residues_v1(
            &base_current,
            &base_next,
            &aux_current,
            &aux_next,
            &fixed.current,
            challenges,
        )
        .expect("direct projection residues");
        assert_eq!(production, direct);
        assert!(
            source
                .constraint_residues_v1(
                    registration,
                    query_index,
                    next_query_index,
                    x.add(F::ONE),
                    opening,
                )
                .is_err(),
            "query coordinate and common-domain point cannot diverge"
        );
    }
    assert!(
        source
            .verifier_fixed_opening_v1(registration, layout.common_lde_size(), 0,)
            .is_err()
    );
    assert!(
        source
            .verifier_fixed_opening_v1(registration, 0, next_stride + 1)
            .is_err(),
        "the verifier-derived next stride cannot be caller-selected"
    );
    let io_registration = layout
        .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
        .expect("I/O registration");
    assert!(
        source
            .verifier_fixed_opening_v1(io_registration, 0, next_stride)
            .is_err()
    );
    let wrap_query = layout.common_lde_size() - next_stride;
    let wrap_fixed = source
        .verifier_fixed_opening_v1(registration, wrap_query, 0)
        .expect("wrapped verifier fixed rows");
    assert_eq!(wrap_fixed.next_query_index, 0);
    let expected_wrapped_next = evaluate_fixed(0);
    assert_eq!(wrap_fixed.next.as_slice(), expected_wrapped_next.as_slice());
    let base = vec![F::ZERO; registration.segment.base_width];
    let aux = vec![F::ZERO; registration.segment.aux_width];
    let opening = RegisteredOpenedRowsV1 {
        base_current: &base,
        base_next: &base,
        aux_current: &aux,
        aux_next: &aux,
    };
    let x = F(GOLDILOCKS_GENERATOR_V1);
    let cache_len = source.fixed_openings.len();
    let unseen_query = 73;
    let unseen_next = unseen_query + next_stride;
    assert!(
        source
            .constraint_residues_v1(registration, unseen_query, unseen_next, x, opening,)
            .is_err(),
        "a mismatched common-domain point is rejected before fixed sampling"
    );
    let malformed_query = unseen_query + 1;
    let malformed_next = malformed_query + next_stride;
    let malformed_x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(malformed_query as u128));
    let short_base = vec![F::ZERO; registration.segment.base_width - 1];
    let malformed_opening = RegisteredOpenedRowsV1 {
        base_current: &short_base,
        base_next: &base,
        aux_current: &aux,
        aux_next: &aux,
    };
    assert!(
        source
            .constraint_residues_v1(
                registration,
                malformed_query,
                malformed_next,
                malformed_x,
                malformed_opening,
            )
            .is_err(),
        "malformed opened-row widths are rejected before fixed sampling"
    );
    assert!(
        source
            .constraint_residues_v1(io_registration, 0, next_stride, x, opening,)
            .is_err(),
        "the combined production route rejects a caller-spliced registration"
    );
    assert!(
        source
            .constraint_residues_v1(registration, 0, next_stride + 1, x, opening,)
            .is_err(),
        "the combined production route derives, rather than accepts, the next query"
    );
    assert!(
        source
            .constraint_residues_v1(registration, layout.common_lde_size(), 0, x, opening,)
            .is_err(),
        "the combined production route rejects an out-of-domain query"
    );
    assert_eq!(
        source.fixed_openings.len(),
        cache_len,
        "invalid points, openings, registrations, and queries cannot mutate the verifier cache"
    );
    for mutate_next in [false, true] {
        for column in 0..ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1 {
            let mut changed_fixed = source
                .verifier_fixed_opening_v1(registration, 0, next_stride)
                .expect("opaque fixed rows");
            let row = if mutate_next {
                &mut changed_fixed.next
            } else {
                &mut changed_fixed.current
            };
            row[column] = row[column].add(F::ONE);
            assert!(
                source
                    .constraint_residues_from_fixed_opening_v1(
                        registration,
                        0,
                        next_stride,
                        x,
                        opening,
                        &changed_fixed,
                    )
                    .is_err(),
                "{} fixed column {column} substitution must fail closed",
                if mutate_next { "next" } else { "current" }
            );
        }
    }
    let mut changed_registration = source
        .verifier_fixed_opening_v1(registration, 0, next_stride)
        .expect("opaque fixed rows");
    changed_registration.registration = io_registration;
    assert!(
        source
            .constraint_residues_from_fixed_opening_v1(
                registration,
                0,
                next_stride,
                x,
                opening,
                &changed_registration,
            )
            .is_err(),
        "a fixed opening is registration-tagged"
    );
    let mut changed_query = source
        .verifier_fixed_opening_v1(registration, 0, next_stride)
        .expect("opaque fixed rows");
    changed_query.query_index = 1;
    assert!(
        source
            .constraint_residues_from_fixed_opening_v1(
                registration,
                0,
                next_stride,
                x,
                opening,
                &changed_query,
            )
            .is_err(),
        "a fixed opening is query-tagged"
    );
}
#[test]
fn main_projection_verifier_cache_reuses_caps_and_rejects_the_117th_opening() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::Projection, 0)
        .expect("projection registration");
    let (statement, _) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
    let post_base = projection_provider_post_base_v1(&statement);
    let mut source =
        MainProjectionVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
            .expect("verifier projection source");
    let next_stride = layout.trace_groups[registration.trace_group]
        .next_stride(layout.common_lde_log2)
        .expect("projection next stride");
    let first = source
        .verifier_fixed_opening_v1(registration, 0, next_stride)
        .expect("first two sampled openings");
    assert_eq!(source.fixed_openings.len(), 2);
    let repeated = source
        .verifier_fixed_opening_v1(registration, 0, next_stride)
        .expect("cache reuse");
    assert_eq!(source.fixed_openings.len(), 2);
    assert_eq!(first.current, repeated.current);
    assert_eq!(first.next, repeated.next);
    let mut candidate = 1_usize;
    while source.fixed_openings.len() < VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 {
        if candidate != next_stride * 2 {
            source
                .fixed_openings
                .entry(candidate)
                .or_insert([F::ZERO; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1]);
        }
        candidate += 1;
    }
    assert_eq!(
        source.fixed_openings.len(),
        VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1
    );
    assert!(source.fixed_openings.contains_key(&next_stride));
    assert!(!source.fixed_openings.contains_key(&(next_stride * 2)));
    let base = vec![F::ZERO; registration.segment.base_width];
    let aux = vec![F::ZERO; registration.segment.aux_width];
    let opening = RegisteredOpenedRowsV1 {
        base_current: &base,
        base_next: &base,
        aux_current: &aux,
        aux_next: &aux,
    };
    let common_root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
    let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(next_stride as u128));
    assert!(
        source
            .constraint_residues_v1(registration, next_stride, next_stride * 2, x, opening,)
            .is_err(),
        "one new index beyond the exact 116-opening bound must fail before sampling"
    );
    assert_eq!(
        source.fixed_openings.len(),
        VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1,
        "a rejected opening cannot mutate the verifier cache"
    );
}
#[test]
fn main_projection_concrete_verifier_routes_through_the_closed_provider_set() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::Projection, 0)
        .expect("projection registration");
    let (statement, _) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
    let post_base = projection_provider_post_base_v1(&statement);
    let mut projection =
        MainProjectionVerifierConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
            .expect("concrete projection verifier");
    let mut log5 = MockMainTraceGroupSourceV1::default();
    let mut log8 = MockMainTraceGroupSourceV1::default();
    let mut log16 = MockMainTraceGroupSourceV1::default();
    let mut log18 = MockMainTraceGroupSourceV1::default();
    let mut log19 = MockMainTraceGroupSourceV1::default();
    let next_stride = layout.trace_groups[registration.trace_group]
        .next_stride(layout.common_lde_log2)
        .expect("projection next stride");
    let query_index = 17;
    let next_query_index = query_index + next_stride;
    let common_root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).expect("MAIN common root");
    let x = F(GOLDILOCKS_GENERATOR_V1).mul(common_root.pow(query_index as u128));
    let base = vec![F::ZERO; registration.segment.base_width];
    let aux = vec![F::ZERO; registration.segment.aux_width];
    let opening = RegisteredOpenedRowsV1 {
        base_current: &base,
        base_next: &base,
        aux_current: &aux,
        aux_next: &aux,
    };
    let mut providers = MainOpenedProviderSetV1::new_v1(
        &layout,
        vec![
            MainOpenedGroupProviderV1::TestLog5(&mut log5),
            MainOpenedGroupProviderV1::TestLog8(&mut log8),
            MainOpenedGroupProviderV1::Projection(&mut projection),
            MainOpenedGroupProviderV1::TestLog16(&mut log16),
            MainOpenedGroupProviderV1::TestLog18(&mut log18),
            MainOpenedGroupProviderV1::TestLog19(&mut log19),
        ],
    )
    .expect("closed provider set with concrete projection verifier");
    let residues = providers
        .registered_constraint_residues_v1(registration, query_index, next_query_index, x, opening)
        .expect("projection routed through concrete verifier variant");
    assert_eq!(residues.len(), registration.segment.constraint_count);
    drop(providers);
    assert_eq!(
        projection.fixed_openings.len(),
        2,
        "provider-set routing must mint verifier-owned fixed openings"
    );
    let mut forged = registration;
    forged.base_start += 1;
    let mut providers = MainOpenedProviderSetV1::new_v1(
        &layout,
        vec![
            MainOpenedGroupProviderV1::TestLog5(&mut log5),
            MainOpenedGroupProviderV1::TestLog8(&mut log8),
            MainOpenedGroupProviderV1::Projection(&mut projection),
            MainOpenedGroupProviderV1::TestLog16(&mut log16),
            MainOpenedGroupProviderV1::TestLog18(&mut log18),
            MainOpenedGroupProviderV1::TestLog19(&mut log19),
        ],
    )
    .expect("closed provider set");
    assert!(
        providers
            .registered_constraint_residues_v1(forged, query_index, next_query_index, x, opening,)
            .is_err(),
        "caller-spliced registration slices fail before provider dispatch"
    );
}
#[test]
fn main_projection_prover_streams_fixed_polynomials_without_verifier_cache() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::Projection, 0)
        .expect("projection registration");
    let (statement, _) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
    let post_base = projection_provider_post_base_v1(&statement);
    let source =
        MainProjectionProverConstraintSourceV1::for_main_v1(&layout, &statement, post_base)
            .expect("streaming projection prover source");
    let fixed_rows =
        compile_zk_x509_projection_stark_fixed_rows_v1(&statement).expect("projection fixed rows");
    let mut expected = transpose_array_rows_v1(&fixed_rows).expect("fixed columns");
    let trace_root = goldilocks_primitive_root_v1(registration.segment.trace_log2)
        .expect("projection native root");
    for coefficients in &mut expected {
        goldilocks_ifft_v1(coefficients, trace_root).expect("fixed interpolation");
    }
    let mut seen = 0_usize;
    source
        .stream_fixed_polynomials_v1(|column, coefficients| {
            assert_eq!(coefficients, expected[column].as_slice());
            seen += 1;
            Ok(())
        })
        .expect("one-at-a-time fixed polynomial stream");
    assert_eq!(seen, ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1);
    let base = vec![F::ZERO; registration.segment.base_width];
    let aux = vec![F::ZERO; registration.segment.aux_width];
    let opening = RegisteredOpenedRowsV1 {
        base_current: &base,
        base_next: &base,
        aux_current: &aux,
        aux_next: &aux,
    };
    let fixed = fixed_rows[0];
    let alphas = vec![E::ONE; registration.segment.constraint_count];
    for _ in 0..=VERIFIER_GENERATED_FIXED_MAX_SAMPLED_OPENINGS_V1 {
        source
            .composition_value_v1(
                registration,
                F(GOLDILOCKS_GENERATOR_V1),
                opening,
                &fixed,
                &alphas,
            )
            .expect("prover composition is independent of verifier cache capacity");
    }
    let mut copied = copied_array_column_v1(&fixed_rows, 0).expect("zeroizing copied fixed column");
    assert!(copied.iter().any(|value| *value != F::ZERO));
    copied.zeroize_private_v1();
    assert!(copied.is_empty());
}
#[test]
fn main_composition_and_opened_evaluator_share_one_exact_checked_path() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let trace_groups = mock_main_opened_groups_v1(&layout);
    let alphas = mock_main_alphas_v1(&layout);
    let mixes = mock_main_mixes_v1(&layout);
    let composition_chunks = (0..COMPOSITION_DEGREE_CHUNKS)
        .map(|index| E::from_base(F(u64::try_from(index + 71).expect("small chunk"))))
        .collect::<Vec<_>>();
    let query_index = 123_456;
    let lane = 0;
    let mut log5 = MockMainTraceGroupSourceV1::default();
    let mut log8 = MockMainTraceGroupSourceV1::default();
    let mut log15 = MockMainTraceGroupSourceV1::default();
    let mut log16 = MockMainTraceGroupSourceV1::default();
    let mut log18 = MockMainTraceGroupSourceV1::default();
    let mut log19 = MockMainTraceGroupSourceV1::default();
    let mut providers = MainOpenedProviderSetV1::new_v1(
        &layout,
        mock_main_opened_group_providers_v1([
            &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
        ]),
    )
    .expect("exact MAIN providers");
    let direct =
        main_opened_composition_value_v1(&mut providers, query_index, lane, &trace_groups, &alphas)
            .expect("direct MAIN row composition");
    let lde_root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).expect("canonical MAIN LDE root");
    let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(query_index as u128));
    let independently_expected = layout
        .registered_segments
        .iter()
        .copied()
        .enumerate()
        .try_fold(E::ZERO, |sum, (index, registration)| {
            let opening = registered_opened_rows_v1(&layout, registration, &trace_groups)
                .expect("registered opening slice");
            let next_stride = layout.trace_groups[registration.trace_group]
                .next_stride(layout.common_lde_log2)
                .expect("registered next stride");
            let next_query_index = (query_index + next_stride) % layout.common_lde_size();
            let fixed = MainFixedOpenedRowsV1 {
                current: mock_main_fixed_row_v1(registration, query_index),
                next: mock_main_fixed_row_v1(registration, next_query_index),
            };
            let residue = mock_main_residue_value_v1(registration, query_index, x, opening, &fixed);
            accumulator_quotient_value_v1(
                registration.segment,
                x,
                &vec![residue; registration.segment.constraint_count],
                &alphas[index][lane],
            )
            .map(|value| sum.add(value))
        })
        .expect("independent registered quotient sum");
    assert_eq!(direct, independently_expected);
    let evaluated = {
        let mut evaluator = MainOpenedRowEvaluatorV1 {
            providers: &mut providers,
            alphas: &alphas,
            mixes: &mixes,
        };
        aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
            &mut evaluator,
            query_index,
            lane,
            &trace_groups,
            &composition_chunks,
        )
        .expect("MAIN opened evaluation")
    };
    assert_eq!(evaluated.composition, direct);
    let mut expected_fri = E::ZERO;
    for (group, lanes) in trace_groups.iter().zip(&mixes) {
        let mix = &lanes[lane];
        expected_fri = expected_fri.add(
            group
                .base_current
                .iter()
                .zip(&mix.base)
                .fold(E::ZERO, |sum, (value, coefficient)| {
                    sum.add(coefficient.mul_base(*value))
                }),
        );
        expected_fri = expected_fri.add(
            group
                .aux_current
                .iter()
                .zip(&mix.aux)
                .fold(E::ZERO, |sum, (value, coefficient)| {
                    sum.add(coefficient.mul_base(*value))
                }),
        );
    }
    expected_fri = expected_fri.add(
        mix_opened_composition_chunks_v1(&composition_chunks, &mixes[0][lane])
            .expect("composition mix"),
    );
    assert_eq!(evaluated.fri_base, expected_fri);
    let mut changed_rows = trace_groups.clone();
    changed_rows[0].base_current[0] = changed_rows[0].base_current[0].add(F::ONE);
    assert_ne!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &changed_rows,
                &alphas,
            )
            .expect("semantic row mutation"),
            direct
        );
    let mut short_rows = trace_groups.clone();
    short_rows[0].base_current.pop();
    assert!(
        main_opened_composition_value_v1(&mut providers, query_index, lane, &short_rows, &alphas,)
            .is_err()
    );
    let mut noncanonical_rows = trace_groups.clone();
    noncanonical_rows[0].base_current[0] =
        F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
    assert!(
        main_opened_composition_value_v1(
            &mut providers,
            query_index,
            lane,
            &noncanonical_rows,
            &alphas,
        )
        .is_err()
    );
    assert!(
        main_opened_composition_value_v1(
            &mut providers,
            query_index,
            lane,
            &trace_groups[..FULL_PROFILE_TRACE_GROUPS_V1 - 1],
            &alphas,
        )
        .is_err()
    );
    assert!(
        main_opened_composition_value_v1(
            &mut providers,
            layout.common_lde_size(),
            lane,
            &trace_groups,
            &alphas,
        )
        .is_err()
    );
    assert!(
        main_opened_composition_value_v1(
            &mut providers,
            query_index,
            SECURITY_LANES,
            &trace_groups,
            &alphas,
        )
        .is_err()
    );
    let mut short_alphas = alphas.clone();
    short_alphas[0][lane].pop();
    assert!(
        main_opened_composition_value_v1(
            &mut providers,
            query_index,
            lane,
            &trace_groups,
            &short_alphas,
        )
        .is_err()
    );
    let mut short_alpha_lanes = alphas.clone();
    short_alpha_lanes[0].pop();
    assert!(
        main_opened_composition_value_v1(
            &mut providers,
            query_index,
            lane,
            &trace_groups,
            &short_alpha_lanes,
        )
        .is_err()
    );
    let mut short_mix = mixes.clone();
    short_mix[0][lane].base.pop();
    let mut evaluator = MainOpenedRowEvaluatorV1 {
        providers: &mut providers,
        alphas: &alphas,
        mixes: &short_mix,
    };
    assert!(
        aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
            &mut evaluator,
            query_index,
            lane,
            &trace_groups,
            &composition_chunks,
        )
        .is_err()
    );
    drop(evaluator);
    let mut short_mix_lanes = mixes.clone();
    short_mix_lanes[0].pop();
    let mut evaluator = MainOpenedRowEvaluatorV1 {
        providers: &mut providers,
        alphas: &alphas,
        mixes: &short_mix_lanes,
    };
    assert!(
        aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
            &mut evaluator,
            query_index,
            lane,
            &trace_groups,
            &composition_chunks,
        )
        .is_err()
    );
    drop(evaluator);
    let mut malformed_layout = layout.clone();
    malformed_layout.trace_groups.clear();
    assert!(validate_main_fri_mixes_v1(&malformed_layout, &[]).is_err());
    let mut inconsistent_mix = mixes.clone();
    inconsistent_mix[1][lane].composition[0] = inconsistent_mix[1][lane].composition[0].add(E::ONE);
    let mut evaluator = MainOpenedRowEvaluatorV1 {
        providers: &mut providers,
        alphas: &alphas,
        mixes: &inconsistent_mix,
    };
    assert!(
        aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
            &mut evaluator,
            query_index,
            lane,
            &trace_groups,
            &composition_chunks,
        )
        .is_err()
    );
    drop(evaluator);
    let mut evaluator = MainOpenedRowEvaluatorV1 {
        providers: &mut providers,
        alphas: &alphas,
        mixes: &mixes,
    };
    assert!(
        aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
            &mut evaluator,
            query_index,
            lane,
            &trace_groups,
            &composition_chunks[..COMPOSITION_DEGREE_CHUNKS - 1],
        )
        .is_err()
    );
    drop(evaluator);
    drop(providers);
    log5.short_residues = true;
    let mut providers = MainOpenedProviderSetV1::new_v1(
        &layout,
        mock_main_opened_group_providers_v1([
            &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
        ]),
    )
    .expect("short-residue adversary registry");
    assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &trace_groups,
                &alphas,
            )
            .is_err()
        );
    drop(providers);
    log5.short_residues = false;
    log5.noncanonical_residues = true;
    let mut providers = MainOpenedProviderSetV1::new_v1(
        &layout,
        mock_main_opened_group_providers_v1([
            &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
        ]),
    )
    .expect("noncanonical-residue adversary registry");
    assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &trace_groups,
                &alphas,
            )
            .is_err()
        );
    drop(providers);
    log5.noncanonical_residues = false;
    log5.short_fixed_row = true;
    let mut providers = MainOpenedProviderSetV1::new_v1(
        &layout,
        mock_main_opened_group_providers_v1([
            &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
        ]),
    )
    .expect("short-fixed-row adversary registry");
    assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &trace_groups,
                &alphas,
            )
            .is_err()
        );
    drop(providers);
    log5.short_fixed_row = false;
    log5.noncanonical_fixed_row = true;
    let mut providers = MainOpenedProviderSetV1::new_v1(
        &layout,
        mock_main_opened_group_providers_v1([
            &mut log5, &mut log8, &mut log15, &mut log16, &mut log18, &mut log19,
        ]),
    )
    .expect("noncanonical-fixed-row adversary registry");
    assert!(
            main_opened_composition_value_v1(
                &mut providers,
                query_index,
                lane,
                &trace_groups,
                &alphas,
            )
            .is_err()
        );
}
#[test]
fn main_base_commitment_session_mints_pre_aux_only_after_canonical_six_group_chronology() {
    fn root(group: usize) -> GoldilocksDigest384V1 {
        test_stark_digest_v1(u8::try_from(0x41 + group).expect("six groups"))
    }
    let mut session = main_base_commitment_session_fixture_v1();
    let streamed = aggregate::StreamingRowCommitmentResultV1 {
        commitment: aggregate::StreamingMerkleCommitmentV1 {
            root: root(0),
            frontier: Vec::new(),
        },
        opened_rows: std::collections::BTreeMap::new(),
    };
    session
        .accept_streaming_base_commitment_v1(0, &streamed)
        .expect("derive log5 root from streamed commitment");
    for (group, native_log) in MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1
        .into_iter()
        .enumerate()
        .skip(1)
    {
        session
            .accept_base_root_v1(group, native_log, root(group))
            .expect("canonical ordered base root");
    }
    assert_eq!(session.next_group, FULL_PROFILE_TRACE_GROUPS_V1);
    let pre_aux = session
        .finish_pre_aux_v1()
        .expect("completed session mints pre-aux state");
    assert_eq!(pre_aux.consensus_context_digest_for_test_v1(), [0xB1; 32]);
    assert_eq!(
        pre_aux.main_profile_digest_for_test_v1(),
        TEST_COMPILED_PROFILE_DIGEST_V1
    );
    assert_eq!(
        pre_aux.main_base_roots_for_test_v1(),
        core::array::from_fn(|index| root(index))
    );
    derive_zk_x509_credential_pre_aux_binding_v1(
        pre_aux,
        test_stark_digest_v1(0xC1),
        test_stark_digest_v1(0xD1),
        test_stark_digest_v1(0xE1),
    )
    .expect("X5B1 begins only from session-minted MAIN pre-aux");
}
#[test]
fn main_base_commitment_session_rejects_omission_reorder_duplicate_wrong_log_zero_and_excess() {
    fn root(group: usize) -> GoldilocksDigest384V1 {
        test_stark_digest_v1(u8::try_from(group + 1).expect("six groups"))
    }
    for omitted_after in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
        let mut session = main_base_commitment_session_fixture_v1();
        for (group, native_log) in MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1
            .into_iter()
            .enumerate()
            .take(omitted_after)
        {
            session
                .accept_base_root_v1(group, native_log, root(group))
                .expect("canonical prefix");
        }
        assert!(
            session.complete_v1().is_err(),
            "a {omitted_after}-root prefix must not complete"
        );
    }
    let mut zero = main_base_commitment_session_fixture_v1();
    assert!(matches!(
        zero.accept_base_root_v1(0, 5, GoldilocksDigest384V1::default()),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert_eq!(zero.next_group, 0);
    assert_eq!(
        zero.roots,
        [GoldilocksDigest384V1::default(); ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1]
    );
    assert_eq!(
        zero.recorded,
        [false; ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1]
    );
    let zero_streamed = aggregate::StreamingRowCommitmentResultV1 {
        commitment: aggregate::StreamingMerkleCommitmentV1 {
            root: GoldilocksDigest384V1::default(),
            frontier: Vec::new(),
        },
        opened_rows: std::collections::BTreeMap::new(),
    };
    assert!(matches!(
        zero.accept_streaming_base_commitment_v1(0, &zero_streamed),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert_eq!(zero.next_group, 0);
    let mut session = main_base_commitment_session_fixture_v1();
    assert!(matches!(
        session.accept_base_root_v1(1, 8, root(1)),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert_eq!(session.next_group, 0);
    assert!(matches!(
        session.accept_base_root_v1(0, 8, root(0)),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert_eq!(session.next_group, 0);
    session
        .accept_base_root_v1(0, 5, root(0))
        .expect("canonical log5 root");
    assert!(matches!(
        session.accept_base_root_v1(0, 5, root(0)),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert!(matches!(
        session.accept_base_root_v1(2, 15, root(2)),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert_eq!(session.next_group, 1);
    for (group, native_log) in MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1
        .into_iter()
        .enumerate()
        .skip(1)
    {
        session
            .accept_base_root_v1(group, native_log, root(group))
            .expect("remaining canonical roots");
    }
    assert!(matches!(
        session.accept_base_root_v1(FULL_PROFILE_TRACE_GROUPS_V1, 20, test_stark_digest_v1(0xFF),),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    session
        .complete_v1()
        .expect("exactly six roots complete once");
}
#[test]
fn main_base_commitment_session_rejects_wrong_layout_profile_count_and_internal_state_tampering() {
    fn group(root: GoldilocksDigest384V1) -> TraceGroupProofV1 {
        TraceGroupProofV1 {
            base_root: root,
            aux_root: GoldilocksDigest384V1::default(),
            base_frontier: Vec::new(),
            aux_frontier: Vec::new(),
        }
    }
    let unpinned_profile = unpinned_main_verifier_profile_fixture_v1();
    let canonical_layout =
        AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    assert!(matches!(
        ZkX509MainBaseCommitmentSessionV1::new_v1(&canonical_layout, [0xB1; 32], unpinned_profile,),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let isolated =
        AggregateProofLayoutV1::for_segments(&[SegmentLayoutV1::for_full_io().expect("full I/O")])
            .expect("isolated I/O layout");
    assert!(matches!(
        ZkX509MainBaseCommitmentSessionV1::new_after_profile_validation_v1(
            &isolated,
            [0xB1; 32],
            TEST_COMPILED_PROFILE_DIGEST_V1,
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let mut wrong_layout = layout.clone();
    wrong_layout.trace_groups.swap(0, 1);
    assert!(matches!(
        ZkX509MainBaseCommitmentSessionV1::new_after_profile_validation_v1(
            &wrong_layout,
            [0xB1; 32],
            TEST_COMPILED_PROFILE_DIGEST_V1,
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    assert!(matches!(
        ZkX509MainBaseCommitmentSessionV1::new_after_profile_validation_v1(
            &layout,
            [0_u8; 32],
            TEST_COMPILED_PROFILE_DIGEST_V1,
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    assert!(matches!(
        ZkX509MainBaseCommitmentSessionV1::new_after_profile_validation_v1(
            &layout, [0xB1; 32], [0_u8; 32],
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let canonical_groups = (0..FULL_PROFILE_TRACE_GROUPS_V1)
        .map(|index| {
            group(test_stark_digest_v1(
                u8::try_from(index + 1).expect("six groups"),
            ))
        })
        .collect::<Vec<_>>();
    for wrong_count in [
        FULL_PROFILE_TRACE_GROUPS_V1 - 1,
        FULL_PROFILE_TRACE_GROUPS_V1 + 1,
    ] {
        let mut groups = canonical_groups.clone();
        groups.resize(wrong_count, group(test_stark_digest_v1(0xFF)));
        let mut session = main_base_commitment_session_fixture_v1();
        assert!(matches!(
            session.accept_decoded_base_groups_v1(&groups),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(session.next_group, 0);
    }
    for zero_at in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
        let mut groups = canonical_groups.clone();
        groups[zero_at].base_root = GoldilocksDigest384V1::default();
        let mut session = main_base_commitment_session_fixture_v1();
        assert!(matches!(
            session.accept_decoded_base_groups_v1(&groups),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(session.next_group, 0);
        assert_eq!(
            session.roots,
            [GoldilocksDigest384V1::default(); ZK_X509_CREDENTIAL_MAIN_BASE_ROOT_COUNT_V1],
            "decoded zero sentinel at group {zero_at} must fail transactionally"
        );
    }
    let mut session = main_base_commitment_session_fixture_v1();
    session
        .accept_decoded_base_groups_v1(&canonical_groups)
        .expect("exact decoded six-group roots");
    session
        .finish_pre_aux_v1()
        .expect("decoded canonical roots mint pre-aux");
    let mut partial = main_base_commitment_session_fixture_v1();
    partial
        .accept_base_root_v1(0, 5, test_stark_digest_v1(0x11))
        .expect("first root");
    assert!(matches!(
        partial.accept_decoded_base_groups_v1(&canonical_groups),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    let mut corrupted = main_base_commitment_session_fixture_v1();
    corrupted.recorded[0] = true;
    assert!(matches!(
        corrupted.accept_base_root_v1(0, 5, test_stark_digest_v1(0x11)),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let mut corrupted = main_base_commitment_session_fixture_v1();
    corrupted.roots[0] = test_stark_digest_v1(0x11);
    assert!(matches!(
        corrupted.accept_base_root_v1(0, 5, test_stark_digest_v1(0x22)),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let mut corrupted = main_base_commitment_session_fixture_v1();
    corrupted.next_group = 1;
    assert!(matches!(
        corrupted.complete_v1(),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let mut corrupted = main_base_commitment_session_fixture_v1();
    corrupted.recorded[0] = true;
    corrupted.next_group = 1;
    assert!(matches!(
        corrupted.complete_v1(),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let mut corrupted = main_base_commitment_session_fixture_v1();
    corrupted.consensus_context_digest = [0_u8; 32];
    assert!(matches!(
        corrupted.accept_base_root_v1(0, 5, test_stark_digest_v1(0x11)),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let mut corrupted = main_base_commitment_session_fixture_v1();
    corrupted.main_profile_digest = [0_u8; 32];
    assert!(matches!(
        corrupted.accept_base_root_v1(0, 5, test_stark_digest_v1(0x11)),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let mut corrupted = main_base_commitment_session_fixture_v1();
    corrupted.layout.trace_groups.swap(0, 1);
    assert!(matches!(
        corrupted.accept_base_root_v1(0, 5, test_stark_digest_v1(0x11)),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
}
#[test]
fn main_trace_phase_root_recorder_rejects_every_reorder_duplicate_omission_zero_and_excess() {
    fn commitment(seed: u8) -> aggregate::StreamingRowCommitmentResultV1 {
        aggregate::StreamingRowCommitmentResultV1 {
            commitment: aggregate::StreamingMerkleCommitmentV1 {
                root: if seed == 0 {
                    GoldilocksDigest384V1::default()
                } else {
                    test_stark_digest_v1(seed)
                },
                frontier: Vec::new(),
            },
            opened_rows: BTreeMap::new(),
        }
    }
    let mut groups = Vec::new();
    assert!(matches!(
        record_main_group_commitment_v1(
            1,
            MainTraceColumnKindV1::Base,
            &commitment(2),
            &mut groups,
        ),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert!(
        groups.is_empty(),
        "a rejected base reorder is transactional"
    );
    assert!(matches!(
        record_main_group_commitment_v1(
            0,
            MainTraceColumnKindV1::Base,
            &commitment(0),
            &mut groups,
        ),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert!(
        groups.is_empty(),
        "the zero-root sentinel cannot mutate state"
    );
    record_main_group_commitment_v1(0, MainTraceColumnKindV1::Base, &commitment(1), &mut groups)
        .expect("first canonical base root");
    assert!(matches!(
        record_main_group_commitment_v1(
            0,
            MainTraceColumnKindV1::Base,
            &commitment(7),
            &mut groups,
        ),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert!(matches!(
        record_main_group_commitment_v1(
            0,
            MainTraceColumnKindV1::Aux,
            &commitment(0x41),
            &mut groups,
        ),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert_eq!(groups.len(), 1, "aux cannot start before all six bases");
    assert_eq!(groups[0].aux_root, GoldilocksDigest384V1::default());
    for group in 1..FULL_PROFILE_TRACE_GROUPS_V1 {
        record_main_group_commitment_v1(
            group,
            MainTraceColumnKindV1::Base,
            &commitment(u8::try_from(group + 1).expect("six roots")),
            &mut groups,
        )
        .expect("remaining canonical base roots");
    }
    let base_snapshot = groups.clone();
    assert!(matches!(
        record_main_group_commitment_v1(
            1,
            MainTraceColumnKindV1::Aux,
            &commitment(0x42),
            &mut groups,
        ),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert_eq!(
        groups, base_snapshot,
        "a rejected aux reorder is transactional"
    );
    assert!(matches!(
        record_main_group_commitment_v1(0, MainTraceColumnKindV1::Aux, &commitment(0), &mut groups,),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
    assert_eq!(groups, base_snapshot, "a zero aux root is transactional");
    for group in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
        record_main_group_commitment_v1(
            group,
            MainTraceColumnKindV1::Aux,
            &commitment(u8::try_from(0x41 + group).expect("six aux roots")),
            &mut groups,
        )
        .expect("canonical auxiliary root");
        assert!(
            groups[..=group]
                .iter()
                .all(|recorded| recorded.aux_root != GoldilocksDigest384V1::default())
        );
        assert!(
            groups[group + 1..]
                .iter()
                .all(|pending| pending.aux_root == GoldilocksDigest384V1::default())
        );
    }
    let complete = groups.clone();
    for hostile_group in [0, FULL_PROFILE_TRACE_GROUPS_V1] {
        assert!(matches!(
            record_main_group_commitment_v1(
                hostile_group,
                MainTraceColumnKindV1::Aux,
                &commitment(0xF1),
                &mut groups,
            ),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
        assert_eq!(groups, complete);
    }
}
fn tiny_authenticated_main_polynomial_fixture_v1(
    seed: u8,
) -> aggregate::MaskedTracePolynomialSetV1 {
    let mut rng = StdRng::from_seed([seed; 32]);
    let (_, polynomials) = aggregate::commit_masked_trace_polynomial_columns_v1(
        ZK_X509_DIGEST_CONTEXT_V1,
        b"iroha:test:zk-x509:main-scratch-leaf:v1",
        b"iroha:test:zk-x509:main-scratch-node:v1",
        usize::from(seed),
        2,
        4,
        1,
        1,
        &[],
        &mut rng,
        |_| Ok(vec![F(u64::from(seed)); 4]),
    )
    .expect("tiny authenticated masked polynomials");
    polynomials
}
#[test]
fn main_polynomial_set_fails_closed_on_count_shape_and_phase_lifecycle() {
    assert!(core::mem::needs_drop::<MainTracePolynomialSetV1>());
    assert!(core::mem::needs_drop::<
        ZkX509MainAwaitingCredentialBindingV1<'static>,
    >());
    assert!(core::mem::needs_drop::<ZkX509MainCompositionPhaseV1<'static>>());
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    for hostile_count in [
        FULL_PROFILE_TRACE_GROUPS_V1 - 1,
        FULL_PROFILE_TRACE_GROUPS_V1 + 1,
    ] {
        let polynomials = (0..hostile_count)
            .map(|index| {
                tiny_authenticated_main_polynomial_fixture_v1(
                    u8::try_from(index + 1).expect("small hostile count"),
                )
            })
            .collect();
        assert!(matches!(
            MainTracePolynomialSetV1::from_ordered_v1(
                &layout,
                MainTraceColumnKindV1::Base,
                polynomials,
            ),
            Err(ZkX509StarkErrorV1::TranscriptMismatch)
        ));
    }
    // Six authenticated polynomial sets are still rejected when any
    // native/commitment-domain shape is not the verifier-fixed MAIN shape.
    // The consuming constructor owns the vector, so every retained secret
    // coefficient is zeroized on this failure path.
    let wrong_shape = (0..FULL_PROFILE_TRACE_GROUPS_V1)
        .map(|index| {
            tiny_authenticated_main_polynomial_fixture_v1(
                u8::try_from(index + 0x21).expect("six fixtures"),
            )
        })
        .collect();
    assert!(matches!(
        MainTracePolynomialSetV1::from_ordered_v1(&layout, MainTraceColumnKindV1::Aux, wrong_shape,),
        Err(ZkX509StarkErrorV1::TranscriptMismatch)
    ));
}
#[test]
fn main_phase_source_has_no_root_only_or_reconstruction_commit_path() {
    let source = include_str!("main_aggregate.rs");
    let helper_start = source
        .find("fn commit_main_trace_group_v1")
        .expect("MAIN commitment helper");
    let helper_end = source[helper_start..]
        .find("fn main_trace_group_root_v1")
        .map(|offset| helper_start + offset)
        .expect("MAIN commitment helper end");
    let helper = &source[helper_start..helper_end];
    assert!(
        helper.contains("commit_masked_trace_polynomial_columns_v1"),
        "MAIN commitments must retain the authenticated masked polynomials"
    );
    assert!(
        !helper.contains("commit_masked_trace_columns_v1(")
            && !helper.contains("commit_masked_trace_columns_retaining_encrypted_scratch_v1")
            && !helper.contains("spill_replayed_masked_trace_columns_v1")
            && !helper.contains("replay_masked_trace_columns_via_encrypted_scratch_v1"),
        "MAIN must not discard its committed polynomials or reconstruct them from native witness"
    );
    let phase_start = source
        .find("pub(crate) fn commit_zk_x509_main_base_phase_v1_with_rng")
        .expect("typed MAIN phase one");
    let phase_end = source[phase_start..]
        .find("/// Exact six-provider registry")
        .map(|offset| phase_start + offset)
        .expect("typed MAIN phase end");
    let phases = &source[phase_start..phase_end];
    assert_eq!(
        phases.matches("base_polynomials.push(polynomials)").count(),
        FULL_PROFILE_TRACE_GROUPS_V1,
        "phase one must retain exactly six base polynomial sets"
    );
    assert_eq!(
        phases.matches("aux_polynomials.push(polynomials)").count(),
        FULL_PROFILE_TRACE_GROUPS_V1,
        "phase two must retain exactly six auxiliary polynomial sets"
    );
    let provenance = phases
        .find("matches_main_pre_aux_v1")
        .expect("provenance check");
    let absorption = phases
        .find("absorb_zk_x509_credential_pre_aux_binding_v1")
        .expect("local binding absorption");
    let child_bind = phases
        .find("projection.bind_challenges_v1")
        .expect("child bind");
    assert!(
        provenance < absorption && provenance < child_bind,
        "cross-phase provenance must fail before transcript or child mutation"
    );
}
fn empty_main_composition_chunks_v1() -> Vec<Vec<Vec<E>>> {
    (0..SECURITY_LANES)
        .map(|_| (0..COMPOSITION_DEGREE_CHUNKS).map(|_| Vec::new()).collect())
        .collect()
}
#[test]
fn main_coefficient_accumulator_is_bounded_transactional_and_order_independent() {
    let coefficient_cap = 8;
    let value = |value| E::from_base(F(value));
    let mut first = empty_main_composition_chunks_v1();
    first[0][0] = vec![value(1), value(2), value(3)];
    first[0][2] = vec![value(9)];
    let mut second = empty_main_composition_chunks_v1();
    second[0][0] = vec![value(4), value(5)];
    second[0][1] = vec![value(7), value(8), value(9), value(10)];
    second[0][2] = vec![value(9).neg()];
    let mut first_then_second = empty_main_composition_chunks_v1();
    add_main_composition_coefficient_chunks_v1(&mut first_then_second, &first, coefficient_cap)
        .expect("first contribution");
    add_main_composition_coefficient_chunks_v1(&mut first_then_second, &second, coefficient_cap)
        .expect("second contribution");
    let mut second_then_first = empty_main_composition_chunks_v1();
    add_main_composition_coefficient_chunks_v1(&mut second_then_first, &second, coefficient_cap)
        .expect("second contribution first");
    add_main_composition_coefficient_chunks_v1(&mut second_then_first, &first, coefficient_cap)
        .expect("first contribution second");
    assert_eq!(first_then_second, second_then_first);
    assert!(
        first_then_second[0][2].is_empty(),
        "exact cancellation must remove the retained tail"
    );
    let canonical = first_then_second.clone();
    let mut wrong_lanes = second.clone();
    wrong_lanes.pop();
    assert!(matches!(
        add_main_composition_coefficient_chunks_v1(
            &mut first_then_second,
            &wrong_lanes,
            coefficient_cap,
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    assert_eq!(first_then_second, canonical);
    let mut wrong_chunks = second.clone();
    wrong_chunks[0].pop();
    assert!(matches!(
        add_main_composition_coefficient_chunks_v1(
            &mut first_then_second,
            &wrong_chunks,
            coefficient_cap,
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    assert_eq!(first_then_second, canonical);
    let mut oversized = second.clone();
    oversized[0][0].resize(coefficient_cap + 1, E::ONE);
    assert!(matches!(
        add_main_composition_coefficient_chunks_v1(
            &mut first_then_second,
            &oversized,
            coefficient_cap,
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    assert_eq!(first_then_second, canonical);
    assert!(matches!(
        add_main_composition_coefficient_chunks_v1(&mut first_then_second, &second, 0,),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    assert_eq!(first_then_second, canonical);
}
#[test]
fn composition_chunk_split_rejects_hidden_high_degree_coefficients() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let shared = layout.as_shared().expect("shared MAIN layout");
    let value = |value| E::from_base(F(value));
    let mut coefficients = vec![value(1), value(2), value(3), E::ZERO, E::ZERO, E::ZERO];
    let chunks =
        composition_coefficient_chunks_v1(&coefficients, 2, &shared).expect("zero high tail");
    assert_eq!(chunks.len(), COMPOSITION_DEGREE_CHUNKS);
    assert_eq!(chunks[0], vec![value(1), value(2), value(3)]);
    coefficients[5] = value(1);
    assert!(matches!(
        composition_coefficient_chunks_v1(&coefficients, 2, &shared),
        Err(ZkX509StarkErrorV1::ConstraintOpening)
    ));
    assert!(matches!(
        composition_coefficient_chunks_v1(&coefficients, usize::MAX, &shared),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
}
#[test]
fn main_finish_verifier_and_consensus_source_use_only_the_closed_release_path() {
    let source = include_str!("main_aggregate.rs");
    let finish_start = source
        .find("pub(crate) fn finish_v1_with_rng")
        .expect("MAIN finish");
    let finish_end = source[finish_start..]
        .find("/// Exact six-provider registry")
        .map(|offset| finish_start + offset)
        .expect("MAIN finish end");
    let finish = &source[finish_start..finish_end];
    assert!(finish.contains("replay_masked_trace_polynomial_columns_v1"));
    assert!(finish.contains("self.composition_material_v1()"));
    let material_start = source
        .find("fn composition_material_v1(&self)")
        .expect("retained composition material");
    let material_end = source[material_start..]
        .find("pub(super) fn record_main_group_commitment_v1")
        .map(|offset| material_start + offset)
        .expect("retained composition material end");
    assert!(
        source[material_start..material_end]
            .contains("main_composition_material_from_polynomials_v1")
    );
    assert!(!finish.contains("verify_opened_query_relations_with_deep_v1"));
    for forbidden in [
        "spill_replayed_masked_trace_columns_v1",
        "replay_masked_trace_columns_via_encrypted_scratch_v1",
        "EncryptedFieldMatrixScratch",
        "native_base_column_v1",
        "native_aux_column_v1",
    ] {
        assert!(
            !finish.contains(forbidden),
            "MAIN finish must not use {forbidden}"
        );
    }
    let verifier_start = source
        .find("pub(crate) fn verify_zk_x509_main_aggregate_stark_v1")
        .expect("complete MAIN verifier");
    let verifier = &source[verifier_start..];
    let grinding = verifier
        .find("verify_grinding_nonce_v1")
        .expect("grinding verification");
    let queries = verifier
        .find("let expected_indices = query_indices_v1")
        .expect("post-grinding queries");
    let fixed = verifier
        .find("derive_zk_x509_main_fixed_openings_after_grinding_v1")
        .expect("verifier-derived fixed openings");
    assert!(grinding < queries && queries < fixed);
    assert_eq!(
        verifier.matches("MainOpenedGroupProviderV1::").count(),
        FULL_PROFILE_TRACE_GROUPS_V1
    );
    assert!(verifier.contains("verify_opened_query_relations_with_deep_v1"));
    let engine = include_str!("../engine.rs");
    let engine_production = &engine[..engine
        .find("#[cfg(test)]")
        .expect("engine production/test boundary")];
    assert!(engine_production.contains("verify_zk_x509_main_aggregate_stark_v1"));
    assert!(engine_production.contains("ca_accumulator_subproof_binding_from_proof_v1"));
    assert!(engine_production.contains("validate_cross_subproof_binding_v1"));
    assert!(!engine_production.contains("ConsensusVerifierUnavailable"));
}
#[test]
fn main_local_transcript_separates_binding_before_base_after_aux_and_wrong_outer_root() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    let base_roots = (0..FULL_PROFILE_TRACE_GROUPS_V1)
        .map(|index| TraceGroupProofV1 {
            base_root: test_stark_digest_v1(u8::try_from(index + 1).expect("six roots")),
            aux_root: test_stark_digest_v1(u8::try_from(index + 0x41).expect("six roots")),
            base_frontier: Vec::new(),
            aux_frontier: Vec::new(),
        })
        .collect::<Vec<_>>();
    let pre_aux = ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
        [0x81; 32],
        TEST_COMPILED_PROFILE_DIGEST_V1,
        base_roots
            .iter()
            .map(|group| group.base_root)
            .collect::<Vec<_>>()
            .try_into()
            .expect("exact six MAIN roots"),
    );
    let binding = derive_zk_x509_credential_pre_aux_binding_v1(
        pre_aux,
        test_stark_digest_v1(0x91),
        test_stark_digest_v1(0xA1),
        test_stark_digest_v1(0xB1),
    )
    .expect("canonical outer binding");
    let mut changed_pre_aux = pre_aux;
    mutate_stark_digest_v1(&mut changed_pre_aux.main_base_roots_mut_for_test_v1()[5]);
    let changed_binding = derive_zk_x509_credential_pre_aux_binding_v1(
        changed_pre_aux,
        test_stark_digest_v1(0x91),
        test_stark_digest_v1(0xA1),
        test_stark_digest_v1(0xB1),
    )
    .expect("binding under a hostile log19 root");
    let claims = main_log19_terminal_claims_fixture_v1();
    let alpha = |order: u8, binding: ZkX509CredentialPreAuxBindingV1| {
        let mut transcript = new_main_transcript_after_profile_validation_v1(
            &[0x81; 32],
            TEST_COMPILED_PROFILE_DIGEST_V1,
        )
        .expect("test MAIN transcript");
        absorb_aggregate_layout_v1(&mut transcript, MAIN_LAYOUT_DOMAIN_V1, &layout)
            .expect("MAIN layout");
        if order == 1 {
            absorb_zk_x509_credential_pre_aux_binding_v1(&mut transcript, binding)
                .expect("premature binding still frames distinctly");
        }
        aggregate::absorb_base_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &base_roots)
            .expect("ordered base roots");
        if order == 0 {
            absorb_zk_x509_credential_pre_aux_binding_v1(&mut transcript, binding)
                .expect("canonical binding");
        }
        aggregate::absorb_aux_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &base_roots)
            .expect("ordered aux roots");
        if order == 2 {
            absorb_zk_x509_credential_pre_aux_binding_v1(&mut transcript, binding)
                .expect("late binding still frames distinctly");
        }
        absorb_zk_x509_main_terminal_claims_v1(&mut transcript, claims).expect("terminal claims");
        derive_constraint_alphas_v1(&mut transcript, &layout).expect("alphas")[0][0][0]
    };
    let canonical = alpha(0, binding);
    assert_ne!(
        canonical,
        alpha(1, binding),
        "binding-before-base must separate"
    );
    assert_ne!(
        canonical,
        alpha(2, binding),
        "binding-after-aux must separate"
    );
    assert_ne!(
        canonical,
        alpha(0, changed_binding),
        "changing only the outer-bound log19 root must separate"
    );
}
#[test]
fn full_profile_layout_is_constant_exact_and_rejects_registration_splices() {
    let sha_layout = AggregateProofLayoutV1::for_equal_log_buckets_v1(
        &canonical_sha_segment_layouts_v1().expect("canonical SHA slices"),
    )
    .expect("canonical SHA-only aggregate layout");
    let sha_shared_layout = sha_layout.as_shared().expect("shared SHA layout");
    assert_eq!(
            maximum_encoded_aggregate_proof_bytes_v1(&sha_layout)
                .expect("maximum SHA-only X5S1 size")
                .checked_sub(
                    aggregate::exact_deep_opening_bytes_v1(
                        AGGREGATE_PARAMETERS_V1,
                        &sha_shared_layout,
                    )
                    .expect("exact SHA DEEP bytes"),
                )
                .expect("SHA pre-DEEP size"),
            ZK_X509_SHA_MAX_ENCODED_PROOF_BYTES_V1,
        );
    let layout = AggregateProofLayoutV1::for_full_profile_v1()
        .expect("canonical fixed-capacity X5S1 layout");
    layout
        .validate_exact_full_profile_registration_v1()
        .expect("exact full registration");
    assert_eq!(
        layout.registered_segments.len(),
        FULL_PROFILE_LOGICAL_REGISTRATIONS_V1
    );
    assert_eq!(layout.trace_groups.len(), FULL_PROFILE_TRACE_GROUPS_V1);
    assert_eq!(
        layout
            .trace_groups
            .iter()
            .map(|group| group.column_chunks)
            .sum::<usize>(),
        FULL_PROFILE_PHYSICAL_CHUNKS_V1
    );
    assert_eq!(layout.common_lde_log2, ZK_X509_MAIN_COMMON_LDE_LOG2_V1);
    assert_eq!(
        layout
            .trace_groups
            .iter()
            .map(|group| group.native_trace_log2)
            .collect::<Vec<_>>(),
        vec![5, 8, 15, 16, 18, 19]
    );
    let maximum_encoded_bytes =
        maximum_encoded_aggregate_proof_bytes_v1(&layout).expect("maximum X5S1 size");
    let main_deep_bytes = aggregate::exact_deep_opening_bytes_v1(
        AGGREGATE_PARAMETERS_V1,
        &layout.as_shared().expect("shared full-profile layout"),
    )
    .expect("exact main DEEP bytes");
    assert_eq!(
        maximum_encoded_bytes
            .checked_sub(main_deep_bytes)
            .expect("main pre-DEEP size"),
        usize::try_from(ZK_X509_MAIN_PRE_DEEP_MAXIMUM_BYTES_V1)
            .expect("profile proof bound fits usize")
    );
    assert!(
        maximum_encoded_bytes
            <= usize::try_from(ZK_X509_MAX_PROOF_BYTES_V1).expect("consensus proof cap fits usize")
    );
    for signature in 0..P256_SIGNATURE_COUNT_V1 {
        let arithmetic = layout
            .registered_segment(
                SegmentAdapterIdV1::P256Arithmetic,
                p256_instance_v1(signature, 0).expect("instance"),
            )
            .expect("one arithmetic registration per signature");
        assert_eq!(
            p256_instance_parts_v1(arithmetic.segment.instance),
            Some((signature, 0))
        );
    }
    let mut mutations = Vec::new();
    let mut changed = layout.clone();
    changed.registered_segments.remove(0);
    mutations.push(changed);
    let mut changed = layout.clone();
    changed
        .registered_segments
        .push(changed.registered_segments[0]);
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.registered_segments.swap(0, 1);
    mutations.push(changed);
    let mut changed = layout.clone();
    let sha = changed
        .registered_segments
        .iter()
        .position(|registration| registration.segment.adapter == SegmentAdapterIdV1::Sha256CallBus)
        .expect("SHA registration");
    changed.registered_segments[sha].segment.instance = 4;
    mutations.push(changed);
    let mut changed = layout.clone();
    let der = changed
        .registered_segments
        .iter()
        .position(|registration| registration.segment.adapter == SegmentAdapterIdV1::StrictDer)
        .expect("DER registration");
    changed.registered_segments[der].segment.active_rows -= 1;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.trace_groups[5].column_chunks -= 1;
    mutations.push(changed);
    let mut changed = layout.clone();
    changed.common_lde_log2 = 22;
    mutations.push(changed);
    for (index, mutation) in mutations.iter().enumerate() {
        assert!(
            mutation
                .validate_exact_full_profile_registration_v1()
                .is_err(),
            "full-layout splice {index} must fail closed"
        );
    }
}
#[test]
fn projection_challenge_labels_are_explicit_and_pairwise_distinct() {
    let labels = ZK_X509_PROJECTION_CHALLENGE_LABELS_V1
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), ZK_X509_PROJECTION_COPY_LANES_V1 * 7);
    let unique = labels.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique.len(), labels.len());
}
