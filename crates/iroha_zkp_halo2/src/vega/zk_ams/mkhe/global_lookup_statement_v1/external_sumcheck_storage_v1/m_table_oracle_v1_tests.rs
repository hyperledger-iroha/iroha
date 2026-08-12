use super::*;

const TOPOLOGY_KAT_V1: [u8; 32] =
    hex_literal::hex!("2d1dcc86a7c58d99a729df30b5c48d3082cea1e4706068eedf6c6ea5aea567a6");
const PARENT_STORAGE_MANIFEST_KAT_V1: [u8; 32] =
    hex_literal::hex!("8396538b1b6ddceb293269d64b0a989c994dd602eb8ef4de311cec4de02cabb2");

fn scalar_chunk_v1(values: &[u64]) -> ConfidentialSpoolChunkV1 {
    let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(SLOT_PLAINTEXT_BYTES_V1).unwrap();
    for (lane, value) in values.iter().copied().enumerate() {
        chunk.as_mut_slice_v1()[lane * 32..lane * 32 + 32]
            .copy_from_slice(&Scalar::from_u64(value).to_be_bytes());
    }
    chunk
}

fn manual_mapping_v1(completed_plane_rounds: u8) -> [u8; 32] {
    let values = 32_768_u64 >> completed_plane_rounds;
    let slots = values.div_ceil(256);
    let file = slots * 8_208;
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.phase23.global-lookup.m-table.mapping\0");
    hash.update(&[1, 15]);
    hash.update(&TOPOLOGY_KAT_V1);
    hash.update(&[completed_plane_rounds]);
    for value in [values, slots, 256, 8_192, 8_208, file] {
        hash.update(&value.to_be_bytes());
    }
    let language = b"M(y)=canonical-u32-multiplicity-before-z;sum(M)=520486912;bits=y0..y14-little-endian;initial-width=32768;slot=floor(index/256);lane=index%256;canonical-T256-scalar-big-endian-32;coordinate-rounds3..13=M-unchanged;plane-rounds14..28=M-low+r*(M-high-M-low);final-unused-lanes-zero";
    hash.update(&(language.len() as u16).to_be_bytes());
    hash.update(language);
    for slot in 0..slots {
        hash.update(&slot.to_be_bytes());
        hash.update(&(slot * 256).to_be_bytes());
        hash.update(&((values - slot * 256).min(256) as u16).to_be_bytes());
    }
    hash.finalize()
}

fn manual_manifest_v1() -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.phase23.global-lookup.m-table-oracle.manifest\0");
    hash.update(&[1, 15]);
    hash.update(&TOPOLOGY_KAT_V1);
    hash.update(&PARENT_STORAGE_MANIFEST_KAT_V1);
    for value in [
        32_768_u64,
        128,
        1_050_624,
        2_101_248,
        11_556_864,
        6_517_152,
        20_175_264,
        1_932,
        270,
        32_767,
        5_380_245_504,
        24_576,
    ] {
        hash.update(&value.to_be_bytes());
    }
    for language in [
        b"M(y)=canonical-u32-multiplicity-before-z;sum(M)=520486912;bits=y0..y14-little-endian;initial-width=32768;slot=floor(index/256);lane=index%256;canonical-T256-scalar-big-endian-32;coordinate-rounds3..13=M-unchanged;plane-rounds14..28=M-low+r*(M-high-M-low);final-unused-lanes-zero".as_slice(),
        b"for-round-k=3..28,prefix=r0..r(k-1),suffix-boolean:chi=eq(rho,x);S=MLE(plane<31768);E0=prod-c(1-c);Qz=MLE_t((z-t)^-1);V=(z-A)U;F=alpha*chi*(V-S)+lambda*(U-E0*M*Qz)+mu*(E0*M-S);evaluate-current-line-at-t=0,1,2,3;sum-over-suffix;interpolate-cubic;require-g(0)+g(1)=base-claim;mask-Z=aT^3+bT^2+cT+(carry-a-b-c)/2;wire=(masked-constant,masked-quadratic,masked-cubic)-canonical-le;derive-nonzero-r-only-after-wire;fold-A,U,and-M-only-for-plane-rounds;final=(A*,U*,M*,R*=F(r),Z*=mask-carry)".as_slice(),
        b"M-initial=write+seal-read;coordinate-rounds=11-authenticated-M-full-reads;plane-round=evaluator-M-read+fold-M-read+next-write+seal-read;combined-file-peak=AU-exact-peak+live-initial-M;derived-context-chains-prior-lineage+parent-snapshot+generation;Qz-and-S-derived-not-stored;OS-page-cache,allocator,stack,AAD,cipher-state,handles-excluded".as_slice(),
    ] {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    hash.update(&[1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    hash.finalize()
}

fn manual_context_v1(public: [u8; 32], completed_plane_rounds: u8, lineage: [u8; 32]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.phase23.global-lookup.m-table.context\0");
    hash.update(&[1, 15]);
    hash.update(&manual_manifest_v1());
    hash.update(&public);
    hash.update(&manual_mapping_v1(completed_plane_rounds));
    hash.update(&lineage);
    hash.update(&[completed_plane_rounds]);
    hash.finalize()
}

#[test]
fn independent_literal_frames_pin_release_tiny_manifest_and_contexts() {
    let release = m_descriptor_v1(0).unwrap();
    let tiny = m_descriptor_v1(13).unwrap();
    assert_eq!(release.mapping_digest, manual_mapping_v1(0));
    assert_eq!(tiny.mapping_digest, manual_mapping_v1(13));
    assert_eq!(m_manifest_digest_v1().unwrap(), manual_manifest_v1());
    let public = [0x5a; 32];
    let release_context = m_context_v1(public, &release, [0; 32]).unwrap();
    let tiny_context = m_context_v1(public, &tiny, [0x44; 32]).unwrap();
    assert_eq!(release_context, manual_context_v1(public, 0, [0; 32]));
    assert_eq!(tiny_context, manual_context_v1(public, 13, [0x44; 32]));
    assert_eq!(
        release.mapping_digest,
        hex_literal::hex!("d2eaa96ca9d3f6c91ed8541baac25374c29e36f4edc7b31175abeec93c065c7b")
    );
    assert_eq!(
        tiny.mapping_digest,
        hex_literal::hex!("a1348ec28699eff102c29a09f1b1c139c3e2ff4473e065d5f543a32285af6639")
    );
    assert_eq!(
        m_manifest_digest_v1().unwrap(),
        hex_literal::hex!("0aa2484f79f2b441042a68b3afb4fa6a1e0e7ca47edd50dc69af80f9f6210ef4")
    );
    assert_eq!(
        release_context,
        hex_literal::hex!("cb4b0ecc55fd05377545d9d96cd80cecdeb8dbe2efe2e53997b08be3e026fadf")
    );
    assert_eq!(
        tiny_context,
        hex_literal::hex!("b4231366278a86d7ae0868443bc43ddc973bc926969c7096464de2af03765e08")
    );
}

#[test]
fn release_geometry_accounting_and_false_gates_are_exact() {
    let release = m_descriptor_v1(0).unwrap();
    assert_eq!(release.value_count, 32_768);
    assert_eq!(release.slot_count, 128);
    assert_eq!(release.file_bytes, 1_050_624);
    assert_eq!(M_TOTAL_IO_BYTES_V1, 20_175_264);
    assert_eq!(M_AUTHENTICATED_READS_V1, 1_932);
    assert_eq!(M_NEXT_WRITE_AND_SEAL_RECORDS_V1, 270);
    assert_eq!(M_SCALAR_FOLDS_V1, 32_767);
    assert_eq!(COMBINED_AU_M_PEAK_FILE_BYTES_V1, 5_380_245_504);
    assert_eq!(M_NAMED_CHUNK_HEAP_BYTES_V1, 24_576);
    assert!(AUTHENTICATED_M_TABLE_COMPLETE_V1);
    assert!(REAL_GLOBAL_CUBIC_ORACLE_COMPLETE_V1);
    for gate in [
        PREFIX_THREE_ROUNDS_WIRED_V1,
        SHARED_TRANSCRIPT_WIRED_V1,
        MASK_COMMITMENT_OPENING_WIRED_V1,
        COMMITTED_MLE_OPENINGS_WIRED_V1,
        PROOF_VERIFIED_V1,
        ZERO_KNOWLEDGE_ACCEPTED_V1,
        RECEIPT_ACCEPTED_V1,
        RSS_QUALIFIED_V1,
        RELEASE_READY_V1,
    ] {
        assert!(!gate);
    }
}

#[test]
fn canonical_u32_sum_is_authenticated_and_invalid_input_poisoned() {
    let directory = tempfile::tempdir().unwrap();
    let mut writer = begin_m_table_v1(
        [0x21; 32],
        MProducerSealV1::TestOnly {
            directory: directory.path().to_path_buf(),
        },
    )
    .unwrap();
    writer
        .push_next_slot_v1(scalar_chunk_v1(&[ACTIVE_LOOKUP_VALUES_V1]))
        .unwrap();
    for _ in 1..M_INITIAL_SLOTS_V1 {
        writer.push_next_slot_v1(scalar_chunk_v1(&[])).unwrap();
    }
    let mut table = writer.seal_v1().unwrap();
    let first = table.read_slot_v1(0).unwrap();
    assert_eq!(
        decode_scalar_be_v1(&first.as_slice_v1()[..32]).unwrap(),
        Scalar::from_u64(ACTIVE_LOOKUP_VALUES_V1)
    );

    let mut poisoned = begin_m_table_v1(
        [0x22; 32],
        MProducerSealV1::TestOnly {
            directory: directory.path().to_path_buf(),
        },
    )
    .unwrap();
    let mut non_u32 = scalar_chunk_v1(&[1]);
    non_u32.as_mut_slice_v1()[0] = 1;
    assert_eq!(
        poisoned.push_next_slot_v1(non_u32),
        Err(MOracleErrorV1::Encoding)
    );
    assert_eq!(
        poisoned.push_next_slot_v1(scalar_chunk_v1(&[])),
        Err(MOracleErrorV1::Order)
    );
}

fn tiny_pair_v1(directory: &Path, public: [u8; 32]) -> ExternalTablePairV1 {
    let mut writer = begin_initial_pair_v1(
        public,
        InitialProducerSealV1::TestOnly {
            directory: directory.to_path_buf(),
            completed_rounds: 27,
        },
    )
    .unwrap();
    writer
        .push_candidate_slot_v1(0, scalar_chunk_v1(&[1, 4, 9, 16]))
        .unwrap();
    writer
        .push_inverse_slot_v1(0, scalar_chunk_v1(&[2, 3, 5, 8]))
        .unwrap();
    writer.seal_v1().unwrap()
}

fn tiny_m_v1(directory: &Path, public: [u8; 32]) -> MTableV1 {
    let descriptor = m_descriptor_v1(13).unwrap();
    let mut writer =
        MWriterV1::create_v1(directory, descriptor, public, [0x44; 32], false).unwrap();
    writer
        .push_next_slot_v1(scalar_chunk_v1(&[7, 11, 13, 17]))
        .unwrap();
    writer.seal_v1().unwrap()
}

fn fold_four_v1(values: [u64; 4], first: Scalar, second: Scalar) -> Scalar {
    let left = Scalar::from_u64(values[0])
        + first * (Scalar::from_u64(values[1]) - Scalar::from_u64(values[0]));
    let right = Scalar::from_u64(values[2])
        + first * (Scalar::from_u64(values[3]) - Scalar::from_u64(values[2]));
    left + second * (right - left)
}

#[test]
fn tiny_real_oracle_recomputes_cubic_and_folds_a_u_m_to_exact_endpoints() {
    let directory = tempfile::tempdir().unwrap();
    let public = [0x31; 32];
    let mut pair = tiny_pair_v1(directory.path(), public);
    let mut multiplicity = tiny_m_v1(directory.path(), public);
    let axes = OracleAxesV1 {
        z: Scalar::from_u64(40_000),
        rho: core::array::from_fn(|index| Scalar::from_u64((index % 7 + 2) as u64)),
        alpha: Scalar::from_u64(3),
        lambda: Scalar::from_u64(5),
        mu: Scalar::from_u64(7),
    };
    let mut point = [Scalar::zero(); 29];
    for (index, value) in point[..27].iter_mut().enumerate() {
        *value = Scalar::from_u64((index % 5 + 2) as u64);
    }
    let first_evaluations =
        evaluate_round_polynomial_v1(27, &axes, &point, &mut pair, &mut multiplicity).unwrap();
    let base_claim = first_evaluations[0] + first_evaluations[1];
    let oracle = begin_global_cubic_oracle_v1(OraclePrefixSealV1::TestOnly {
        pair,
        multiplicity,
        axes,
        point,
        base_claim,
        mask_carry: Scalar::zero(),
        masks: MaskCoefficientsV1([[Scalar::zero(); 3]; MASK_ROUNDS_V1]),
    })
    .unwrap();
    let evaluated = oracle.evaluate_next_v1().unwrap();
    let first_coefficients = interpolate_cubic_v1(first_evaluations).unwrap();
    for (encoded, expected) in evaluated.message_v1().chunks_exact(32).zip([
        first_coefficients[0],
        first_coefficients[2],
        first_coefficients[3],
    ]) {
        assert_eq!(
            Scalar::from_le_bytes_exact(encoded.try_into().unwrap()).unwrap(),
            expected
        );
    }
    let first = Scalar::from_u64(2);
    let second = Scalar::from_u64(3);
    let oracle = match evaluated
        .derive_and_fold_v1(
            OracleTranscriptSealV1::TestOnly { challenge: first },
            FoldSinkSealV1::TestOnly {
                directory: directory.path().to_path_buf(),
            },
        )
        .unwrap()
    {
        OracleTransitionV1::Continue(oracle) => oracle,
        OracleTransitionV1::Complete(_) => panic!("one round early"),
    };
    let evaluated = oracle.evaluate_next_v1().unwrap();
    let complete = match evaluated
        .derive_and_fold_v1(
            OracleTranscriptSealV1::TestOnly { challenge: second },
            FoldSinkSealV1::TestOnly {
                directory: directory.path().to_path_buf(),
            },
        )
        .unwrap()
    {
        OracleTransitionV1::Complete(complete) => complete,
        OracleTransitionV1::Continue(_) => panic!("missing completion"),
    };
    assert_eq!(
        complete.candidate,
        fold_four_v1([1, 4, 9, 16], first, second)
    );
    assert_eq!(complete.inverse, fold_four_v1([2, 3, 5, 8], first, second));
    assert_eq!(
        complete.multiplicity,
        fold_four_v1([7, 11, 13, 17], first, second)
    );
    assert_eq!(complete.mask_carry, Scalar::zero());
    assert_eq!(complete.point[27], first);
    assert_eq!(complete.point[28], second);
    assert_eq!(
        complete.relation,
        endpoint_relation_v1(
            &axes,
            &complete.point,
            complete.candidate,
            complete.inverse,
            complete.multiplicity,
        )
        .unwrap()
    );
}

#[test]
fn interpolation_mask_and_lineage_hostile_mutations_diverge() {
    let coefficients = [
        Scalar::from_u64(2),
        Scalar::from_u64(3),
        Scalar::from_u64(5),
        Scalar::from_u64(7),
    ];
    let evaluations =
        [0_u64, 1, 2, 3].map(|point| evaluate_cubic_v1(coefficients, Scalar::from_u64(point)));
    assert_eq!(interpolate_cubic_v1(evaluations).unwrap(), coefficients);
    assert_ne!(manual_mapping_v1(0), manual_mapping_v1(1));
    assert_ne!(
        manual_context_v1([0x5a; 32], 13, [0x44; 32]),
        manual_context_v1([0x5a; 32], 13, [0x45; 32])
    );
    assert!(m_fold_lineage_v1([0; 32], [1; 32], 1).is_err());
    assert!(m_fold_lineage_v1([1; 32], [0; 32], 1).is_err());
    assert!(m_fold_lineage_v1([1; 32], [2; 32], 0).is_err());

    let mut axes = OracleAxesV1 {
        z: Scalar::from_u64(40_000),
        rho: [Scalar::one(); 29],
        alpha: Scalar::one(),
        lambda: Scalar::one(),
        mu: Scalar::one(),
    };
    let mut point = [Scalar::zero(); 29];
    point[..3].fill(Scalar::one());
    assert!(validate_axes_and_prefix_v1(&axes, &point, 3).is_ok());
    axes.z = Scalar::from_u64(17);
    assert_eq!(
        validate_axes_and_prefix_v1(&axes, &point, 3),
        Err(MOracleErrorV1::Context)
    );
    axes.z = Scalar::from_u64(40_000);
    axes.alpha = Scalar::zero();
    assert_eq!(
        validate_axes_and_prefix_v1(&axes, &point, 3),
        Err(MOracleErrorV1::Context)
    );
    axes.alpha = Scalar::one();
    point[3] = Scalar::one();
    assert_eq!(
        validate_axes_and_prefix_v1(&axes, &point, 3),
        Err(MOracleErrorV1::Order)
    );
}

#[test]
fn source_guards_freeze_real_oracle_privacy_and_budgets() {
    let source = include_str!("m_table_oracle_v1.rs");
    assert!(!source.contains("pub struct"));
    assert!(!source.contains("pub fn"));
    assert!(!source.contains("Vec<Scalar>"));
    assert!(!source.contains("relation_rhs"));
    assert!(!source.contains("FTable"));
    assert!(!source.contains("Serialize"));
    assert!(!source.contains("Deserialize"));
    assert!(source.contains("committed_m_opening: Infallible"));
    assert!(source.contains("prefix_three_rounds: Infallible"));
    assert!(source.contains("shared_transcript: Infallible"));
    assert!(source.contains("evaluate_round_polynomial_v1"));
    assert!(source.contains("fold-A,U,and-M-only-for-plane-rounds"));
    assert!(source.lines().count() <= 1_200);
    assert!(include_str!("m_table_oracle_v1_tests.rs").lines().count() <= 500);
}
