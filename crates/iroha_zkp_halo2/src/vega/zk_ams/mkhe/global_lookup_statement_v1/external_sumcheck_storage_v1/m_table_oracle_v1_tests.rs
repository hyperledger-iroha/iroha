use super::*;
const TOPOLOGY_KAT_V1: [u8; 32] =
    hex_literal::hex!("3af9a6ad67383c32b06bb5d95a05863b8cb0b3338660177bc2a92e1bbf40b4ab");
const PARENT_STORAGE_MANIFEST_KAT_V1: [u8; 32] =
    hex_literal::hex!("32f5dfeb2ba549c07d37c06f6ef10ae6fb66c5bff745046560cc00b690d4573b");
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
        hex_literal::hex!("dac62360b74b1c5d5c1578443c80520c977ceaef5853f409dee44237198a3f46")
    );
    assert_eq!(
        tiny.mapping_digest,
        hex_literal::hex!("7f8cf7961710f3a6e8be9f208b9511654eb2c96200c6c762c845216e7378ae94")
    );
    assert_eq!(
        m_manifest_digest_v1().unwrap(),
        hex_literal::hex!("60081ce451f48984142423e69c21465eebbc66511f0693fdd041d93cb97b2a23")
    );
    assert_eq!(
        release_context,
        hex_literal::hex!("11d9aec8b2c6b404079cb580cbb673735461137dd822e5c70748b0b1cadd4ab9")
    );
    assert_eq!(
        tiny_context,
        hex_literal::hex!("3b0e275c2d2667c5bc1b10443ba243d00921a3dc8d6f5112f132e95b2bd04ba8")
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
    const {
        assert!(AUTHENTICATED_M_TABLE_COMPLETE_V1);
        assert!(REAL_GLOBAL_CUBIC_ORACLE_COMPLETE_V1);
    };
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
    let directory = crate::testing::TestDirectory::new("m-table-canonical-sum");
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
fn round12_pair_v1(directory: &Path, public: [u8; 32]) -> ExternalTablePairV1 {
    let mut writer = begin_initial_pair_v1(
        public,
        InitialProducerSealV1::TestOnly {
            directory: directory.to_path_buf(),
            completed_rounds: 12,
        },
    )
    .unwrap();
    for slot in 0..512 {
        writer
            .push_candidate_slot_v1(slot, scalar_chunk_v1(&[3; 256]))
            .unwrap();
    }
    for slot in 0..512 {
        writer
            .push_inverse_slot_v1(slot, scalar_chunk_v1(&[5; 256]))
            .unwrap();
    }
    writer.seal_v1().unwrap()
}
fn round12_m_v1(directory: &Path, public: [u8; 32]) -> MTableV1 {
    let mut writer = begin_m_table_v1(
        public,
        MProducerSealV1::TestOnly {
            directory: directory.to_path_buf(),
        },
    )
    .unwrap();
    let values: [u64; 256] =
        core::array::from_fn(|lane| if lane & 1 == 0 { 15_883 } else { 15_885 });
    for _ in 0..128 {
        writer.push_next_slot_v1(scalar_chunk_v1(&values)).unwrap();
    }
    writer.seal_v1().unwrap()
}
fn kat_scalar_v1(bytes: [u8; 32]) -> Scalar {
    Scalar::from_be_bytes_exact(bytes).unwrap()
}
fn continue_oracle_v1(
    evaluated: EvaluatedGlobalRoundV1,
    challenge: u64,
    directory: &Path,
) -> GlobalCubicOracleV1 {
    match evaluated
        .fold_with_raw_challenge_v1(
            Scalar::from_u64(challenge),
            FoldSinkSealV1::TestOnly {
                directory: directory.to_path_buf(),
            },
        )
        .unwrap()
    {
        OracleTransitionV1::Continue(oracle) => *oracle,
        OracleTransitionV1::Complete(_) => panic!("unexpected early completion"),
    }
}
#[test]
fn independent_round12_through_plane_round_kat_pins_m_fold_and_masks() {
    let directory = crate::testing::TestDirectory::new("m-table-round12-kat");
    let public = [0x31; 32];
    let pair = round12_pair_v1(directory.path(), public);
    let multiplicity = round12_m_v1(directory.path(), public);
    assert_eq!(pair.descriptor.completed_rounds, 12);
    assert_eq!(pair.descriptor.remaining_log_values, 17);
    assert_eq!(multiplicity.descriptor.value_count, 32_768);
    let m_snapshot = multiplicity.snapshot_digest;
    let axes = OracleAxesV1 {
        z: Scalar::from_u64(40_000),
        rho: [Scalar::from_u64(3); 29],
        alpha: Scalar::from_u64(3),
        lambda: Scalar::from_u64(5),
        mu: Scalar::from_u64(7),
    };
    let mut point = [Scalar::zero(); 29];
    point[..12].fill(Scalar::from_u64(2));
    let mut masks = [[Scalar::zero(); 3]; MASK_ROUNDS_V1];
    for (round, values) in [(12, [2, 3, 5]), (13, [7, 11, 13]), (14, [17, 19, 23])] {
        masks[round - 3] = values.map(Scalar::from_u64);
    }
    let oracle = begin_global_cubic_oracle_v1(GlobalCubicPrefixReadyV1 {
        pair,
        multiplicity,
        axes,
        point,
        base_claim: SecretScalarV1::new(kat_scalar_v1(hex_literal::hex!(
            "26ed099b45728e68cfa3c3e6b5898c7ffb912e773633c644d0bcc04b5306ff06"
        ))),
        mask_carry: SecretScalarV1::new(Scalar::from_u64(11)),
        masks: MaskCoefficientsV1(masks),
        message_override: None,
    })
    .unwrap();
    // These literals were computed modulo the pinned P-256 base-field prime by
    // a separate integer model; no production evaluator/interpolator is used.
    let evaluated = oracle.evaluate_next_v1().unwrap();
    assert_eq!(
        evaluated.message_v1(),
        &hex_literal::hex!(
            "56c8f4529b7c8dcf44c633b6772e91fb7f8c89b5e6c3a34f698e72c59a09eda603000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000"
        )
    );
    let oracle = continue_oracle_v1(evaluated, 2, directory.path());
    let live_m = &oracle.live.as_ref().unwrap().multiplicity;
    assert_eq!(live_m.descriptor.completed_plane_rounds, 0);
    assert_eq!(live_m.snapshot_digest, m_snapshot);
    assert_eq!(
        oracle.base_claim,
        kat_scalar_v1(hex_literal::hex!(
            "d912f663ba8d7198305c3c194a767380046ed189c9cc39bb32d10ac4ad2fa509"
        ))
    );
    assert_eq!(
        oracle.mask_carry,
        kat_scalar_v1(hex_literal::hex!(
            "7fffffff80000000800000000000000000000000800000000000000000000026"
        ))
    );
    let evaluated = oracle.evaluate_next_v1().unwrap();
    assert_eq!(
        evaluated.message_v1(),
        &hex_literal::hex!(
            "b58926ad44ed5629bb39cc8989d16e048073764a193c5cf097718dfa63f612990b000000000000000000000000000000000000000000000000000000000000000700000000000000000000000000000000000000000000000000000000000000"
        )
    );
    let oracle = continue_oracle_v1(evaluated, 3, directory.path());
    let live_m = &oracle.live.as_ref().unwrap().multiplicity;
    assert_eq!(oracle.next_round, 14);
    assert_eq!(live_m.descriptor.completed_plane_rounds, 0);
    assert_eq!(live_m.snapshot_digest, m_snapshot);
    assert_eq!(
        oracle.base_claim,
        kat_scalar_v1(hex_literal::hex!(
            "4dda13368ae51cd19f4787cd6b1318fff7225cee6c678c89c9c07df6a5ce3ea4"
        ))
    );
    assert_eq!(
        oracle.mask_carry,
        kat_scalar_v1(hex_literal::hex!(
            "bfffffff40000000c00000000000000000000000c0000000000000000000014a"
        ))
    );
    let evaluated = oracle.evaluate_next_v1().unwrap();
    assert_eq!(
        evaluated.message_v1(),
        &hex_literal::hex!(
            "e2d479a2174af5727ed053a5be2e34e0f4284bc00a9a0e51983070139c5648732ca0d92fb681f0376c827f77f8689d7d4e8b31a6e5e72650d1dc1cf07ee509561100000000000000000000000000000000000000000000000000000000000000"
        )
    );
    let mut oracle = continue_oracle_v1(evaluated, 4, directory.path());
    let live_m = &mut oracle.live.as_mut().unwrap().multiplicity;
    assert_eq!(live_m.descriptor.completed_plane_rounds, 1);
    assert_eq!(live_m.descriptor.value_count, 16_384);
    assert_ne!(live_m.snapshot_digest, m_snapshot);
    let first_slot = live_m.read_slot_v1(0).unwrap();
    assert_eq!(
        decode_scalar_be_v1(&first_slot.as_slice_v1()[..32]).unwrap(),
        kat_scalar_v1(hex_literal::hex!(
            "0000000000000000000000000000000000000000000000000000000000003e13"
        ))
    );
    assert_eq!(
        oracle.base_claim,
        kat_scalar_v1(hex_literal::hex!(
            "38e4b08ac4dd78ea278ac7ae3490ccfb9e7d1826e64d9bc4a19305bf6417ad47"
        ))
    );
    assert_eq!(
        oracle.mask_carry,
        kat_scalar_v1(hex_literal::hex!(
            "dfffffff20000000e00000000000000000000000e00000000000000000000653"
        ))
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
        [0_u64, 1, 2, 3].map(|point| evaluate_cubic_v1(&coefficients, Scalar::from_u64(point)));
    assert_eq!(interpolate_cubic_v1(&evaluations).unwrap().0, coefficients);
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
fn fail_with_scalar_bytes_v1() -> Result<(), MOracleErrorV1> {
    let _bytes = ZeroizingScalarBytesV1([0x5a; 32]);
    Err(MOracleErrorV1::Spool)
}
#[test]
fn scalar_bytes_owner_drops_on_success_error_and_unwind() {
    assert!(core::mem::needs_drop::<ZeroizingScalarBytesV1>());
    drop(ZeroizingScalarBytesV1([0x5a; 32]));
    assert!(fail_with_scalar_bytes_v1().is_err());
    assert!(
        std::panic::catch_unwind(|| {
            let _bytes = ZeroizingScalarBytesV1([0x5a; 32]);
            panic!("exercise zeroizing unwind");
        })
        .is_err()
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
    assert!(!source.contains("OraclePrefixSealV1"));
    assert!(!source.contains("OracleTranscriptSealV1"));
    assert!(!source.contains("prefix_three_rounds: Infallible"));
    assert!(!source.contains("shared_transcript: Infallible"));
    assert!(source.contains("struct GlobalCubicPrefixReadyV1"));
    assert!(source.contains("base_claim: SecretScalarV1"));
    assert!(source.contains("mask_carry: SecretScalarV1"));
    assert!(source.contains("fn fold_with_raw_challenge_v1"));
    assert!(source.contains("#[cfg(test)]\n    message_override"));
    assert!(source.contains("impl Drop for GlobalCubicOracleV1"));
    assert!(source.contains("impl Drop for GlobalCubicCompleteV1"));
    assert!(source.contains("struct ZeroizingScalarArrayV1"));
    assert!(source.contains("struct ZeroizingScalarBytesV1"));
    assert!(source.contains("initial_sum: Option<SecretScalarV1>"));
    assert!(source.contains("compiler_fence(core::sync::atomic::Ordering::SeqCst)"));
    assert!(source.contains("let mut evaluations = ZeroizingScalarArrayV1"));
    assert!(source.contains("evaluate_round_polynomial_v1"));
    assert!(source.contains("fold-A,U,and-M-only-for-plane-rounds"));
    assert!(source.lines().count() <= 1_400);
    assert!(include_str!("m_table_oracle_v1_tests.rs").lines().count() <= 500);
}
