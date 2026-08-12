use super::*;

fn scalar_chunk_v1(values: &[u64]) -> ConfidentialSpoolChunkV1 {
    let mut chunk =
        ConfidentialSpoolChunkV1::new_zeroed_v1(SLOT_PLAINTEXT_BYTES_V1).expect("tiny exact chunk");
    for (lane, value) in values.iter().copied().enumerate() {
        let offset = lane * SCALAR_BYTES_V1 as usize;
        chunk.as_mut_slice_v1()[offset..offset + SCALAR_BYTES_V1 as usize]
            .copy_from_slice(&Scalar::from_u64(value).to_be_bytes());
    }
    chunk
}

fn message_v1(values: [u64; 3]) -> [u8; 96] {
    let mut message = [0_u8; 96];
    for (encoded, value) in message.chunks_exact_mut(32).zip(values) {
        encoded.copy_from_slice(&Scalar::from_u64(value).to_le_bytes());
    }
    message
}

fn manual_mapping_v1(completed_rounds: u8) -> [u8; 32] {
    let remaining_log = GLOBAL_SUMCHECK_ROUNDS_V1 - completed_rounds;
    let values = 1_u64 << remaining_log;
    let slots = values.div_ceil(SCALARS_PER_SLOT_V1);
    let plaintext = values * SCALAR_BYTES_V1;
    let file = slots * SLOT_CIPHERTEXT_BYTES_V1;
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.phase23.global-lookup.external-sumcheck.mapping\0");
    hash.update(&[1, 15]);
    hash.update(&global_lookup_topology_digest_v1());
    hash.update(&[29, 3, completed_rounds, remaining_log]);
    for value in [values, slots, 256, 8_192, 8_208, plaintext, file] {
        hash.update(&value.to_be_bytes());
    }
    let language = b"statement=15;variables=(c0..c13,y0..y14);rounds0..2=streamed;materialize-after-round2;columns=A,U;index-little-endian-over-remaining-variables;slot=floor(index/256);lane=index%256;canonical-T256-scalar-big-endian-32;fold=low+r*(high-low);A-complete-before-U;fresh-sealed-output;final-unused-lanes-zero";
    hash.update(&(language.len() as u16).to_be_bytes());
    hash.update(language);
    for slot in 0..slots {
        hash.update(&slot.to_be_bytes());
        hash.update(&(slot * 256).to_be_bytes());
        let valid = (values - slot * 256).min(256) as u16;
        hash.update(&valid.to_be_bytes());
    }
    hash.finalize()
}

fn manual_manifest_v1() -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.phase23.global-lookup.external-sumcheck.manifest\0");
    hash.update(&[1, 15]);
    hash.update(&global_lookup_topology_digest_v1());
    for value in [
        67_108_864_u64,
        262_144,
        2_151_677_952,
        1_075_838_976,
        5_379_194_880,
        8_606_711_808,
        25_820_562_240,
        34_427_274_048,
        2_097_176,
        1_048_604,
        134_217_726,
        16_384,
    ] {
        hash.update(&value.to_be_bytes());
    }
    for language in [
        MAPPING_LANGUAGE_V1,
        b"initial=two-columns*(write+seal-read);each-round=evaluator-read-AU+fold-read-AU+next-write-and-seal-AU;file-peak=current-A+current-U+one-next-column;OS-page-cache,allocator,stack,AAD,cipher-state,handles-excluded",
    ] {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    hash.update(&[1, 0, 0, 0, 0, 0, 0, 0, 0]);
    hash.finalize()
}

fn manual_context_v1(
    public_context: [u8; 32],
    descriptor: &ExternalColumnDescriptorV1,
    role: ExternalColumnRoleV1,
) -> [u8; 32] {
    let generation = descriptor.completed_rounds - 3;
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.phase23.global-lookup.external-sumcheck.context\0");
    hash.update(&[1, 15]);
    hash.update(&manual_manifest_v1());
    hash.update(&public_context);
    hash.update(&manual_mapping_v1(descriptor.completed_rounds));
    hash.update(&[role as u8, generation, descriptor.completed_rounds]);
    hash.finalize()
}

#[test]
fn release_geometry_accounting_and_false_claims_are_exact() {
    let release = descriptor_v1(3).expect("release descriptor");
    assert_eq!(release.value_count, 1 << 26);
    assert_eq!(release.slot_count, 262_144);
    assert_eq!(release.file_bytes, 2_151_677_952);
    assert_eq!(RELEASE_FIRST_NEXT_COLUMN_FILE_BYTES_V1, 1_075_838_976);
    assert_eq!(RELEASE_PEAK_FILE_BYTES_V1, 5_379_194_880);
    assert_eq!(RELEASE_INITIAL_WRITE_AND_SEAL_IO_BYTES_V1, 8_606_711_808);
    assert_eq!(RELEASE_ROUND_IO_BYTES_V1, 25_820_562_240);
    assert_eq!(RELEASE_TOTAL_IO_BYTES_V1, 34_427_274_048);
    assert_eq!(RELEASE_AUTHENTICATED_ROUND_READS_V1, 2_097_176);
    assert_eq!(RELEASE_NEXT_WRITE_AND_SEAL_RECORDS_V1, 1_048_604);
    assert_eq!(RELEASE_SCALAR_FOLDS_V1, 134_217_726);
    assert_eq!(FOLD_NAMED_CHUNK_HEAP_BYTES_V1, 16_384);
    assert!(STORAGE_MECHANICS_COMPLETE_V1);
    for gate in [
        AUTHENTICATED_M_TABLE_WIRED_V1,
        EQUATION_CORRECTNESS_VERIFIED_V1,
        TRANSCRIPT_WIRED_V1,
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
fn independent_literal_frames_pin_mapping_manifest_and_role_contexts() {
    let release = descriptor_v1(3).expect("release descriptor");
    let tiny = descriptor_v1(27).expect("tiny descriptor");
    assert_eq!(release.mapping_digest, manual_mapping_v1(3));
    assert_eq!(tiny.mapping_digest, manual_mapping_v1(27));
    assert_eq!(manifest_digest_v1().unwrap(), manual_manifest_v1());
    let public = [0x5a; 32];
    let a = column_context_v1(public, &tiny, ExternalColumnRoleV1::CandidateA, 24).unwrap();
    let u = column_context_v1(public, &tiny, ExternalColumnRoleV1::InverseU, 24).unwrap();
    assert_eq!(
        a,
        manual_context_v1(public, &tiny, ExternalColumnRoleV1::CandidateA)
    );
    assert_eq!(
        u,
        manual_context_v1(public, &tiny, ExternalColumnRoleV1::InverseU)
    );
    assert_ne!(a, u);
    assert_ne!(release.mapping_digest, tiny.mapping_digest);

    assert_eq!(
        release.mapping_digest,
        hex_literal::hex!("b46b6e09176e0da18217932db04339c858a4290916575788007a053416e381d7")
    );
    assert_eq!(
        tiny.mapping_digest,
        hex_literal::hex!("285a2d2b4c7a31f1743a6f78b3e0cc2bfa95a84a207d06d7abc932533defee2b")
    );
    assert_eq!(
        manifest_digest_v1().unwrap(),
        hex_literal::hex!("8396538b1b6ddceb293269d64b0a989c994dd602eb8ef4de311cec4de02cabb2")
    );
    assert_eq!(
        a,
        hex_literal::hex!("a1977b7a5d0caa8b08e89545b15d3720556e02e8dfd353fdc4967ce21e1d3f4b")
    );
    assert_eq!(
        u,
        hex_literal::hex!("0216eccd7d7a6361a49828860caa027dae9b4015af2526ce4fea02ceea596507")
    );
}

#[test]
fn tiny_authenticated_pair_roundtrip_and_two_folds_are_exact() {
    let directory = tempfile::tempdir().expect("tempdir");
    let public = [0x33; 32];
    let mut initial = begin_initial_pair_v1(
        public,
        InitialProducerSealV1::TestOnly {
            directory: directory.path().to_path_buf(),
            completed_rounds: 27,
        },
    )
    .expect("begin tiny pair");
    initial
        .push_candidate_slot_v1(0, scalar_chunk_v1(&[1, 2, 3, 4]))
        .expect("candidate");
    initial
        .push_inverse_slot_v1(0, scalar_chunk_v1(&[5, 6, 7, 8]))
        .expect("inverse");
    let pair = initial.seal_v1().expect("seal pair");

    let evaluated = evaluate_round_v1(
        pair,
        RoundEvaluatorSealV1::TestOnly {
            message: message_v1([9, 10, 11]),
        },
    )
    .expect("authenticate/evaluate");
    let challenged = evaluated
        .derive_challenge_v1(RoundTranscriptSealV1::TestOnly {
            challenge: Scalar::from_u64(2),
        })
        .expect("challenge after message");
    let mut pair = challenged
        .fold_v1(FoldSinkSealV1::TestOnly {
            directory: directory.path().to_path_buf(),
        })
        .expect("first fold");
    assert_eq!(pair.descriptor.completed_rounds, 28);
    let a = pair
        .candidate
        .as_mut()
        .expect("candidate")
        .read_slot_v1(0)
        .expect("authenticated candidate");
    assert_eq!(
        decode_scalar_be_v1(&a.as_slice_v1()[..32]).unwrap(),
        Scalar::from_u64(3)
    );
    assert_eq!(
        decode_scalar_be_v1(&a.as_slice_v1()[32..64]).unwrap(),
        Scalar::from_u64(5)
    );
    drop(a);

    let evaluated = evaluate_round_v1(
        pair,
        RoundEvaluatorSealV1::TestOnly {
            message: message_v1([12, 13, 14]),
        },
    )
    .expect("second evaluate");
    let challenged = evaluated
        .derive_challenge_v1(RoundTranscriptSealV1::TestOnly {
            challenge: Scalar::from_u64(3),
        })
        .expect("second challenge");
    let mut final_pair = challenged
        .fold_v1(FoldSinkSealV1::TestOnly {
            directory: directory.path().to_path_buf(),
        })
        .expect("second fold");
    assert_eq!(final_pair.descriptor.completed_rounds, 29);
    let a = final_pair
        .candidate
        .as_mut()
        .expect("candidate")
        .read_slot_v1(0)
        .expect("final candidate");
    assert_eq!(
        decode_scalar_be_v1(&a.as_slice_v1()[..32]).unwrap(),
        Scalar::from_u64(9)
    );
    assert!(a.as_slice_v1()[32..].iter().all(|byte| *byte == 0));
}

#[test]
fn malformed_padding_order_message_and_challenge_fail_closed() {
    assert_eq!(descriptor_v1(2), Err(ExternalStorageErrorV1::Shape));
    assert_eq!(descriptor_v1(30), Err(ExternalStorageErrorV1::Shape));
    let descriptor = descriptor_v1(29).unwrap();
    let mut nonzero_padding = scalar_chunk_v1(&[1]);
    nonzero_padding.as_mut_slice_v1()[64] = 1;
    assert_eq!(
        validate_chunk_v1(&descriptor, 0, nonzero_padding.as_slice_v1()),
        Err(ExternalStorageErrorV1::Encoding)
    );
    let mut bad_message = message_v1([1, 2, 3]);
    bad_message[..32].fill(0xff);
    assert_eq!(
        validate_message_v1(&bad_message),
        Err(ExternalStorageErrorV1::Encoding)
    );

    let directory = tempfile::tempdir().unwrap();
    let mut initial = begin_initial_pair_v1(
        [7; 32],
        InitialProducerSealV1::TestOnly {
            directory: directory.path().to_path_buf(),
            completed_rounds: 29,
        },
    )
    .unwrap();
    assert_eq!(
        initial.push_inverse_slot_v1(0, scalar_chunk_v1(&[1])),
        Err(ExternalStorageErrorV1::Order)
    );
    assert_eq!(
        initial.push_inverse_slot_v1(0, scalar_chunk_v1(&[1])),
        Err(ExternalStorageErrorV1::Order)
    );
}

#[test]
fn source_guards_freeze_privacy_typestate_and_budgets() {
    let source = include_str!("external_sumcheck_storage_v1.rs");
    assert!(!source.contains("pub struct"));
    assert!(!source.contains("pub fn"));
    assert!(!source.contains("Vec<Scalar>"));
    assert!(!source.contains("Clone for External"));
    assert!(!source.contains("Serialize"));
    assert!(!source.contains("Deserialize"));
    assert!(!source.contains("pub fn path"));
    assert!(!source.contains("pub fn snapshot"));
    assert!(!source.contains("key_v1"));
    assert!(source.contains("producer: Infallible"));
    assert!(source.contains("evaluator: Infallible"));
    assert!(source.contains("transcript: Infallible"));
    assert!(source.contains("sink: Infallible"));
    assert!(source.contains("low + self.challenge * (high - low)"));
    assert!(source.lines().count() <= 850);
    assert!(
        include_str!("external_sumcheck_storage_v1_tests.rs")
            .lines()
            .count()
            <= 400
    );
}
