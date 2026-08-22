use super::state_lineage::*;
use super::*;
use halo2_proofs::halo2curves::{
    ff::PrimeField,
    group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
    pasta::{EpAffine, EqAffine, Fp, Fq},
};

#[test]
fn hard_cut_ledger_literals_match_the_reviewed_profile() {
    assert_eq!(OFFLINE_CASH_HALO2_K_V2, 17);
    assert_eq!(OFFLINE_CASH_PARITY_RAW_PROOF_BYTES_V2, 3_232);
    assert_eq!(OFFLINE_CASH_PARITY_AUGMENTED_PROOF_BYTES_V2, 3_264);
    assert_eq!(OFFLINE_CASH_CHILD_PROOF_ABSOLUTE_MAX_BYTES_V2, 3_264);
    assert_eq!(
        OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_QUALIFICATION_TARGET_BYTES_V2,
        6_272
    );
    assert_eq!(
        OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_ABSOLUTE_MAX_BYTES_V2,
        6_528
    );
    assert_eq!(OFFLINE_CASH_P256_PAIRED_AUGMENTED_PROOF_BYTES_V2, 6_528);
    assert_eq!(
        OFFLINE_CASH_P256_PAIRED_FINAL_STATE_TARGET_MISS_BYTES_V2,
        256
    );
    assert!(!OFFLINE_CASH_P256_PAIR_ESTABLISHES_FINAL_STATE_QUALIFICATION_V2);
    assert_eq!(OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2, 576);
    assert_eq!(OFFLINE_CASH_PAIRED_PARENT_LINEAGE_BYTES_V2, 1_152);
    assert_eq!(OFFLINE_CASH_STATE_ABI_WORDS_V2, 237);
    assert_eq!(OFFLINE_CASH_STATE_INSTANCE_CELLS_V2, 34);
    assert_eq!(OFFLINE_CASH_STATE_FINAL_CELL_ZERO_PADDING_WORDS_V2, 1);
    assert_eq!(OFFLINE_CASH_PAYMENT_FIXED_ENVELOPE_BYTES_V2, 448);
    assert_eq!(OFFLINE_CASH_PAYMENT_MAX_BYTES_V2, 8_128);
    assert_eq!(OFFLINE_CASH_EXACT_COMPONENT_RAW_SESSION_BYTES_V2, 9_152);
    assert_eq!(
        OFFLINE_CASH_EXACT_COMPONENT_THREE_FRAME_TEXT_SESSION_BYTES_V2,
        12_219
    );
    assert_eq!(
        OFFLINE_CASH_UNRESOLVED_AGGREGATE_RAW_SESSION_POLICY_CEILING_BYTES_V2,
        9_403
    );
    assert_eq!(
        OFFLINE_CASH_UNRESOLVED_AGGREGATE_THREE_FRAME_TEXT_CEILING_BYTES_V2,
        12_554
    );
    assert_eq!(OFFLINE_CASH_PARAMS_BYTES_V2, 8_388_676);
    assert_eq!(OFFLINE_CASH_P256_PROCESSED_VERIFYING_KEY_BYTES_V2, 394);
    assert_eq!(
        OFFLINE_CASH_P256_PROCESSED_PROVING_KEY_BYTES_V2,
        113_246_726
    );
    assert_eq!(
        OFFLINE_CASH_MINIMUM_STRAIGHTFORWARD_RESIDENCE_BYTES_V2,
        155_189_834
    );
    assert_eq!(
        OFFLINE_CASH_STRAIGHTFORWARD_RESIDENCE_WORKSPACE_BYTES_V2,
        33_554_432
    );
    assert_eq!(OFFLINE_CASH_PROCESS_RSS_QUALIFICATION_BYTES_V2, 268_435_456);
    assert_eq!(OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V2, 536_870_912);
    assert_eq!(OFFLINE_CASH_P256_ARTIFACT_SUBSET_BYTES_V2, 243_271_592);
    assert_eq!(
        OFFLINE_CASH_EXISTING_PROVING_KEY_ARCHIVE_MAX_BYTES_V2,
        67_108_864
    );
}

#[test]
fn proof_session_resource_and_subset_equations_are_explicit() {
    assert_eq!(
        OFFLINE_CASH_P256_PAIRED_AUGMENTED_PROOF_BYTES_V2,
        OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_ABSOLUTE_MAX_BYTES_V2
    );
    assert_eq!(
        OFFLINE_CASH_P256_PAIRED_AUGMENTED_PROOF_BYTES_V2
            - OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_QUALIFICATION_TARGET_BYTES_V2,
        256
    );
    assert!(!OFFLINE_CASH_P256_PAIR_ESTABLISHES_FINAL_STATE_QUALIFICATION_V2);
    assert_eq!(
        OFFLINE_CASH_STATE_FINAL_CELL_ZERO_PADDING_WORDS_V2,
        OFFLINE_CASH_STATE_INSTANCE_CELLS_V2 * OFFLINE_CASH_STATE_WORDS_PER_INSTANCE_V2
            - OFFLINE_CASH_STATE_ABI_WORDS_V2
    );
    assert_eq!(
        OFFLINE_CASH_PAYMENT_MAX_BYTES_V2,
        OFFLINE_CASH_PAYMENT_FIXED_ENVELOPE_BYTES_V2
            + OFFLINE_CASH_FINAL_STATE_PAIRED_PROOF_ABSOLUTE_MAX_BYTES_V2
            + OFFLINE_CASH_PAIRED_PARENT_LINEAGE_BYTES_V2
    );
    assert_eq!(
        OFFLINE_CASH_EXACT_COMPONENT_RAW_SESSION_BYTES_V2,
        OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V2
            + OFFLINE_CASH_PAYMENT_MAX_BYTES_V2
            + OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V2
    );
    assert_eq!(
        OFFLINE_CASH_EXACT_COMPONENT_THREE_FRAME_TEXT_SESSION_BYTES_V2,
        3 * TEXT_PREFIX_BYTES
            + unpadded_base64url_len(OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V2)
            + unpadded_base64url_len(OFFLINE_CASH_PAYMENT_MAX_BYTES_V2)
            + unpadded_base64url_len(OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V2)
    );
    assert!(
        OFFLINE_CASH_EXACT_COMPONENT_RAW_SESSION_BYTES_V2
            < OFFLINE_CASH_UNRESOLVED_AGGREGATE_RAW_SESSION_POLICY_CEILING_BYTES_V2
    );
    assert_eq!(
        OFFLINE_CASH_UNRESOLVED_AGGREGATE_THREE_FRAME_TEXT_CEILING_BYTES_V2,
        3 * TEXT_PREFIX_BYTES
            + maximum_three_frame_unpadded_base64url_len(
                OFFLINE_CASH_UNRESOLVED_AGGREGATE_RAW_SESSION_POLICY_CEILING_BYTES_V2,
            )
    );
    assert_eq!(
        OFFLINE_CASH_UNRESOLVED_AGGREGATE_THREE_FRAME_TEXT_CEILING_BYTES_V2,
        3 * TEXT_PREFIX_BYTES
            + unpadded_base64url_len(1)
            + unpadded_base64url_len(1)
            + unpadded_base64url_len(
                OFFLINE_CASH_UNRESOLVED_AGGREGATE_RAW_SESSION_POLICY_CEILING_BYTES_V2 - 2,
            )
    );
    assert_eq!(
        OFFLINE_CASH_MINIMUM_STRAIGHTFORWARD_RESIDENCE_BYTES_V2,
        OFFLINE_CASH_PARAMS_BYTES_V2
            + OFFLINE_CASH_P256_PROCESSED_PROVING_KEY_BYTES_V2
            + OFFLINE_CASH_STRAIGHTFORWARD_RESIDENCE_WORKSPACE_BYTES_V2
    );
    assert!(
        OFFLINE_CASH_MINIMUM_STRAIGHTFORWARD_RESIDENCE_BYTES_V2
            <= OFFLINE_CASH_PROCESS_RSS_QUALIFICATION_BYTES_V2
    );
    assert_eq!(
        OFFLINE_CASH_P256_ARTIFACT_SUBSET_BYTES_V2,
        2 * (OFFLINE_CASH_PARAMS_BYTES_V2
            + OFFLINE_CASH_P256_PROCESSED_PROVING_KEY_BYTES_V2
            + OFFLINE_CASH_P256_PROCESSED_VERIFYING_KEY_BYTES_V2)
    );
    assert!(OFFLINE_CASH_P256_ARTIFACT_SUBSET_BYTES_V2 <= OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V2);
    assert!(!OFFLINE_CASH_COMPLETE_ARTIFACT_INVENTORY_AVAILABLE_V2);
    assert!(
        OFFLINE_CASH_P256_PROCESSED_PROVING_KEY_BYTES_V2
            > OFFLINE_CASH_EXISTING_PROVING_KEY_ARCHIVE_MAX_BYTES_V2
    );
}

fn digest(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn state_abi_fields() -> OfflineCashStateAbiFieldsV2 {
    OfflineCashStateAbiFieldsV2 {
        operation: OfflineCashStateOperationV2::SendSplit,
        release_digest: digest(1),
        protocol_digest: digest(2),
        semantic_digest: digest(3),
        context_digest: digest(4),
        request_digest: digest(5),
        parent_0: digest(6),
        parent_1: digest(7),
        result: digest(8),
        link: digest(9),
        transition_digest: digest(10),
        amount: 0x1122_3344_5566_7788_99aa_bbcc_ddee_ff00,
        scale: 9,
    }
}

#[test]
fn parent_lineage_codec_is_typed_canonical_and_bootstrap_explicit() {
    let zero = [0_u8; 576];
    let eq_bootstrap = OfflineCashEqParentLineageV2::decode(&zero).expect("Eq bootstrap");
    let ep_bootstrap = OfflineCashEpParentLineageV2::decode(&zero).expect("Ep bootstrap");
    assert!(eq_bootstrap.is_bootstrap());
    assert!(ep_bootstrap.is_bootstrap());
    assert_eq!(eq_bootstrap, OfflineCashEqParentLineageV2::bootstrap());
    assert_eq!(ep_bootstrap, OfflineCashEpParentLineageV2::bootstrap());
    assert_eq!(eq_bootstrap.encode(), zero);
    assert_eq!(ep_bootstrap.encode(), zero);

    let eq_challenges = std::array::from_fn(|index| Fp::from((index + 1) as u64));
    let eq = OfflineCashEqParentLineageV2::live(eq_challenges, EqAffine::generator())
        .expect("live Eq lineage");
    let eq_bytes = eq.encode();
    assert!(!eq.is_bootstrap());
    assert_eq!(eq_bytes.len(), 576);
    for (index, challenge) in eq_challenges.iter().enumerate() {
        assert_eq!(
            &eq_bytes[index * 32..(index + 1) * 32],
            challenge.to_repr().as_ref()
        );
    }
    assert_eq!(
        &eq_bytes[17 * 32..],
        EqAffine::generator().to_bytes().as_ref()
    );
    assert_eq!(
        OfflineCashEqParentLineageV2::decode(&eq_bytes).expect("round-trip Eq"),
        eq
    );

    let ep_challenges = std::array::from_fn(|index| Fq::from((index + 31) as u64));
    let ep = OfflineCashEpParentLineageV2::live(ep_challenges, EpAffine::generator())
        .expect("live Ep lineage");
    let ep_bytes = ep.encode();
    assert!(!ep.is_bootstrap());
    assert_eq!(
        OfflineCashEpParentLineageV2::decode(&ep_bytes).expect("round-trip Ep"),
        ep
    );

    let eq_zero_challenge_live = OfflineCashEqParentLineageV2::live(
        std::array::from_fn(|_| Fp::from(0)),
        EqAffine::generator(),
    )
    .expect("zero-challenge Eq live lineage");
    let ep_zero_challenge_live = OfflineCashEpParentLineageV2::live(
        std::array::from_fn(|_| Fq::from(0)),
        EpAffine::generator(),
    )
    .expect("zero-challenge Ep live lineage");
    assert!(!eq_zero_challenge_live.is_bootstrap());
    assert!(!ep_zero_challenge_live.is_bootstrap());
    assert_ne!(eq_zero_challenge_live.encode(), zero);
    assert_ne!(ep_zero_challenge_live.encode(), zero);
    assert_eq!(
        OfflineCashEqParentLineageV2::decode(&eq_zero_challenge_live.encode())
            .expect("zero-challenge Eq round-trip"),
        eq_zero_challenge_live
    );
    assert_eq!(
        OfflineCashEpParentLineageV2::decode(&ep_zero_challenge_live.encode())
            .expect("zero-challenge Ep round-trip"),
        ep_zero_challenge_live
    );

    assert_eq!(
        OfflineCashEqParentLineageV2::decode(&eq_bytes[..575]),
        Err(OfflineCashParentLineageCodecErrorV2::InvalidLength { actual: 575 })
    );
    assert_eq!(
        OfflineCashEqParentLineageV2::decode(&[0_u8; 577]),
        Err(OfflineCashParentLineageCodecErrorV2::InvalidLength { actual: 577 })
    );
    let mut noncanonical_scalar = eq_bytes;
    noncanonical_scalar[..32].fill(0xff);
    assert_eq!(
        OfflineCashEqParentLineageV2::decode(&noncanonical_scalar),
        Err(OfflineCashParentLineageCodecErrorV2::NonCanonicalRoundChallenge { index: 0 })
    );
    let mut invalid_point = eq_bytes;
    invalid_point[17 * 32..].fill(0xff);
    assert_eq!(
        OfflineCashEqParentLineageV2::decode(&invalid_point),
        Err(OfflineCashParentLineageCodecErrorV2::InvalidFoldedGenerator)
    );
    let mut identity_point = eq_bytes;
    identity_point[17 * 32..].copy_from_slice(EqAffine::identity().to_bytes().as_ref());
    assert_eq!(
        OfflineCashEqParentLineageV2::decode(&identity_point),
        Err(OfflineCashParentLineageCodecErrorV2::IdentityFoldedGenerator)
    );
    assert_eq!(
        OfflineCashEqParentLineageV2::live(eq_challenges, EqAffine::identity()),
        Err(OfflineCashParentLineageCodecErrorV2::IdentityFoldedGenerator)
    );

    let mut ep_noncanonical_scalar = ep_bytes;
    ep_noncanonical_scalar[..32].fill(0xff);
    assert_eq!(
        OfflineCashEpParentLineageV2::decode(&ep_noncanonical_scalar),
        Err(OfflineCashParentLineageCodecErrorV2::NonCanonicalRoundChallenge { index: 0 })
    );
    let mut ep_invalid_point = ep_bytes;
    ep_invalid_point[17 * 32..].fill(0xff);
    assert_eq!(
        OfflineCashEpParentLineageV2::decode(&ep_invalid_point),
        Err(OfflineCashParentLineageCodecErrorV2::InvalidFoldedGenerator)
    );
    let mut ep_identity_point = ep_bytes;
    ep_identity_point[17 * 32..].copy_from_slice(EpAffine::identity().to_bytes().as_ref());
    assert_eq!(
        OfflineCashEpParentLineageV2::decode(&ep_identity_point),
        Err(OfflineCashParentLineageCodecErrorV2::IdentityFoldedGenerator)
    );
    assert_eq!(
        OfflineCashEpParentLineageV2::live(ep_challenges, EpAffine::identity()),
        Err(OfflineCashParentLineageCodecErrorV2::IdentityFoldedGenerator)
    );
}

#[test]
fn state_abi_has_exact_order_parent_lineage_tail_and_zero_padding() {
    let fields = state_abi_fields();
    let challenges = std::array::from_fn(|index| Fp::from((index + 101) as u64));
    let lineage = OfflineCashEqParentLineageV2::live(challenges, EqAffine::generator())
        .expect("live Eq lineage");
    let instances = OfflineCashStatePublicInstancesV2::eq(fields, &lineage).expect("Eq ABI");
    let words = instances.words();

    assert_eq!(instances.parity(), OfflineCashHalo2ParityV2::Eq);
    assert_eq!(
        &words[..8],
        &[
            2,
            2,
            17,
            1,
            OfflineCashStateOperationV2::SendSplit as u32,
            2,
            8,
            144
        ]
    );
    for (offset, byte) in [
        (OFFLINE_CASH_STATE_RELEASE_WORD_START_V2, 1),
        (OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2, 2),
        (OFFLINE_CASH_STATE_SEMANTIC_WORD_START_V2, 3),
        (OFFLINE_CASH_STATE_CONTEXT_WORD_START_V2, 4),
        (OFFLINE_CASH_STATE_REQUEST_WORD_START_V2, 5),
        (OFFLINE_CASH_STATE_PARENT_0_WORD_START_V2, 6),
        (OFFLINE_CASH_STATE_PARENT_1_WORD_START_V2, 7),
        (OFFLINE_CASH_STATE_RESULT_WORD_START_V2, 8),
        (OFFLINE_CASH_STATE_LINK_WORD_START_V2, 9),
        (OFFLINE_CASH_STATE_TRANSITION_WORD_START_V2, 10),
    ] {
        assert_eq!(
            &words[offset..offset + 8],
            &[u32::from_le_bytes([byte; 4]); 8]
        );
    }
    assert_eq!(
        &words
            [OFFLINE_CASH_STATE_AMOUNT_WORD_START_V2..OFFLINE_CASH_STATE_AMOUNT_WORD_START_V2 + 4],
        &[0xddee_ff00, 0x99aa_bbcc, 0x5566_7788, 0x1122_3344]
    );
    assert_eq!(words[OFFLINE_CASH_STATE_SCALE_WORD_V2], 9);
    let expected_lineage_words: Vec<u32> = lineage
        .encode()
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("four-byte lineage limb")))
        .collect();
    assert_eq!(
        &words[OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2..],
        expected_lineage_words.as_slice()
    );
    assert_eq!(instances.eq_parent_lineage().expect("Eq accessor"), lineage);
    assert_eq!(
        instances.ep_parent_lineage(),
        Err(OfflineCashStateAbiErrorV2::ParityMismatch)
    );

    let ep_lineage = OfflineCashEpParentLineageV2::live(
        std::array::from_fn(|index| Fq::from((index + 201) as u64)),
        EpAffine::generator(),
    )
    .expect("live Ep lineage");
    let ep_instances =
        OfflineCashStatePublicInstancesV2::ep(state_abi_fields(), &ep_lineage).expect("Ep ABI");
    assert_eq!(
        ep_instances.ep_parent_lineage().expect("Ep accessor"),
        ep_lineage
    );
    assert_eq!(
        ep_instances.eq_parent_lineage(),
        Err(OfflineCashStateAbiErrorV2::ParityMismatch)
    );

    let cells = instances.packed_cell_bytes();
    assert_eq!(cells.len(), 34);
    assert_eq!(&cells[33][24..], &[0_u8; 4]);
    assert_eq!(
        OfflineCashStatePublicInstancesV2::unpack_cell_bytes(&cells).expect("canonical cells"),
        *words
    );
    assert_eq!(
        OfflineCashStatePublicInstancesV2::unpack_cell_bytes(&[]),
        Err(OfflineCashStateAbiErrorV2::NonCanonicalPacking)
    );
    assert_eq!(
        OfflineCashStatePublicInstancesV2::unpack_cell_bytes(&cells[..33]),
        Err(OfflineCashStateAbiErrorV2::NonCanonicalPacking)
    );
    let mut thirty_five_cells = cells.to_vec();
    thirty_five_cells.push([0_u8; 28]);
    assert_eq!(
        OfflineCashStatePublicInstancesV2::unpack_cell_bytes(&thirty_five_cells),
        Err(OfflineCashStateAbiErrorV2::NonCanonicalPacking)
    );
    for padding_offset in 24..28 {
        let mut noncanonical_cells = cells;
        noncanonical_cells[33][padding_offset] = 1;
        assert_eq!(
            OfflineCashStatePublicInstancesV2::unpack_cell_bytes(&noncanonical_cells),
            Err(OfflineCashStateAbiErrorV2::NonCanonicalPacking),
            "padding offset {padding_offset}"
        );
    }
}

#[test]
fn bootstrap_sentinel_requires_uninhabited_authenticated_mode() {
    let fields = OfflineCashStateAbiFieldsV2 {
        operation: OfflineCashStateOperationV2::ReceiveFold,
        ..state_abi_fields()
    };
    assert_eq!(
        OfflineCashStatePublicInstancesV2::ep(fields, &OfflineCashEpParentLineageV2::bootstrap()),
        Err(OfflineCashStateAbiErrorV2::UnauthenticatedBootstrap)
    );
    assert_eq!(
        OfflineCashStatePublicInstancesV2::eq(
            state_abi_fields(),
            &OfflineCashEqParentLineageV2::bootstrap(),
        ),
        Err(OfflineCashStateAbiErrorV2::UnauthenticatedBootstrap)
    );

    let mut invalid = state_abi_fields();
    invalid.protocol_digest = [0; 32];
    let live = OfflineCashEqParentLineageV2::live(
        std::array::from_fn(|index| Fp::from((index + 1) as u64)),
        EqAffine::generator(),
    )
    .expect("live Eq lineage");
    assert_eq!(
        OfflineCashStatePublicInstancesV2::eq(invalid, &live),
        Err(OfflineCashStateAbiErrorV2::InvalidLayout)
    );
}

#[test]
fn transcript_and_terminal_contracts_are_acyclic_direct_and_fail_closed() {
    assert_eq!(
        OFFLINE_CASH_STATE_INSTANCE_QUERY_V2,
        crate::zk::pasta_ipa_recursion::PastaIpaInstanceQueryV1::Direct
    );
    assert!(!OFFLINE_CASH_STATE_QUERY_INSTANCE_V2);
    assert!(!OFFLINE_CASH_STATE_CURRENT_ACCUMULATOR_IN_PUBLIC_INSTANCES_V2);
    assert_eq!(OFFLINE_CASH_STATE_CURRENT_ACCUMULATOR_PUBLIC_WORDS_V2, 0);
    assert!(!OFFLINE_CASH_STATE_CURRENT_ACCUMULATOR_IN_DIGESTS_V2);
    assert!(!OFFLINE_CASH_STATE_CURRENT_PROOF_BYTES_IN_DIGESTS_V2);
    assert!(OFFLINE_CASH_STATE_PARENT_LINEAGE_PRECEDES_CURRENT_TRANSCRIPT_V2);
    assert!(!OFFLINE_CASH_STATE_POST_PROOF_FOLD_IN_PAYMENT_V2);
    assert_eq!(
        OFFLINE_CASH_STATE_LINEAGE_CHILD_ORDER_V2,
        [
            OfflineCashStateLineageChildRoleV2::StateParent0,
            OfflineCashStateLineageChildRoleV2::StateParent1,
            OfflineCashStateLineageChildRoleV2::GuardBundle,
        ]
    );
    assert_eq!(
        OFFLINE_CASH_GUARD_BUNDLE_LINEAGE_CHILD_ORDER_V2,
        [
            OfflineCashGuardBundleLineageChildRoleV2::GuardUse,
            OfflineCashGuardBundleLineageChildRoleV2::PlatformBind,
            OfflineCashGuardBundleLineageChildRoleV2::AndroidKeyCert,
            OfflineCashGuardBundleLineageChildRoleV2::P256Signature,
        ]
    );
    assert!(
        OFFLINE_CASH_STATE_TRANSCRIPT_ORDER_V2
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    assert_eq!(
        OFFLINE_CASH_STATE_TRANSCRIPT_ORDER_V2,
        [
            OfflineCashStateTranscriptStageV2::ParentLineageFinalized,
            OfflineCashStateTranscriptStageV2::PublicInstancesAbsorbed,
            OfflineCashStateTranscriptStageV2::CurrentProofRead,
            OfflineCashStateTranscriptStageV2::CurrentAccumulatorDerived,
            OfflineCashStateTranscriptStageV2::SuccessorLineageProduced,
            OfflineCashStateTranscriptStageV2::FuturePublicInstances,
        ]
    );
    assert!(
        OFFLINE_CASH_STATE_TERMINAL_ORDER_V2
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    assert_eq!(
        OFFLINE_CASH_STATE_TERMINAL_ORDER_V2.last(),
        Some(&OfflineCashStateTerminalStageV2::IssueReceipt)
    );
    assert_eq!(
        fail_closed_offline_cash_state_terminal_v2().expect_err("terminal must fail closed"),
        OfflineCashStateTerminalErrorV2::VerificationUnavailable
    );
}

#[test]
fn p256_eq_ep_records_are_metadata_only_and_never_eligible() {
    assert_eq!(OFFLINE_CASH_P256_SIGNATURE_METADATA_V2.len(), 2);
    let [eq, ep] = OFFLINE_CASH_P256_SIGNATURE_METADATA_V2;
    assert_eq!(eq.parity, OfflineCashHalo2ParityV2::Eq);
    assert_eq!(ep.parity, OfflineCashHalo2ParityV2::Ep);
    assert_eq!(
        eq.circuit_role,
        OfflineCashHalo2CircuitRoleV2::P256Signature
    );
    assert_eq!(
        ep.circuit_role,
        OfflineCashHalo2CircuitRoleV2::P256Signature
    );
    assert_eq!(
        (eq.proving_key_role, eq.verifying_key_role),
        (
            OfflineCashP256ArtifactRoleV2::P256SignaturePkEq,
            OfflineCashP256ArtifactRoleV2::P256SignatureVkEq,
        )
    );
    assert_eq!(
        (ep.proving_key_role, ep.verifying_key_role),
        (
            OfflineCashP256ArtifactRoleV2::P256SignaturePkEp,
            OfflineCashP256ArtifactRoleV2::P256SignatureVkEp,
        )
    );
    for metadata in OFFLINE_CASH_P256_SIGNATURE_METADATA_V2 {
        assert_eq!(metadata.k, 17);
        assert_eq!(metadata.raw_proof_bytes, 3_232);
        assert_eq!(metadata.augmented_proof_bytes, 3_264);
        assert_eq!(metadata.processed_proving_key_bytes, 113_246_726);
        assert_eq!(metadata.processed_verifying_key_bytes, 394);
        assert!(!metadata.activation_eligible);
    }
}

#[test]
fn every_unclosed_v2_gate_is_pinned_in_the_blocker_inventory() {
    assert!(!OFFLINE_CASH_P256_V3_INTERVAL_EVIDENCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_P256_V3_SLOPE_EVIDENCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_SESSION_FRAMING_PROFILE_FROZEN_V2);
    assert!(!OFFLINE_CASH_FINAL_STATE_PAIR_TARGET_DECISION_AVAILABLE_V2);
    assert!(OFFLINE_CASH_STATE_PARENT_LINEAGE_CONTRACT_IS_ACYCLIC_V2);
    assert!(!OFFLINE_CASH_STATE_DIRECT_INSTANCE_VERIFIER_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_STATE_RECURSIVE_FOLD_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_STATE_TERMINAL_RECEIPT_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_V2_WIRE_RELEASE_TYPES_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_COMPACT_SHA_EVIDENCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_DER_KEYMINT_GOVERNED_ROOT_CLOSURE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_RECURSION_BOOTSTRAP_PROTOCOL_IDENTITY_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_GUARD_BUNDLE_EVIDENCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_STATE_EVIDENCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_COMPLETE_ARTIFACT_INVENTORY_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_COMPLETE_ARTIFACT_SET_SIZE_EVIDENCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_ARTIFACT_EVIDENCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_MEASURED_PROCESS_RSS_EVIDENCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_DEVICE_EVIDENCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_VERIFICATION_BACKEND_AVAILABLE_V2);
    assert_eq!(
        OFFLINE_CASH_ACTIVATION_BLOCKERS_V2,
        [
            OfflineCashActivationPreflightErrorV2::P256V3IntervalEvidenceUnavailable,
            OfflineCashActivationPreflightErrorV2::P256V3SlopeEvidenceUnavailable,
            OfflineCashActivationPreflightErrorV2::SessionFramingProfileUnresolved,
            OfflineCashActivationPreflightErrorV2::FinalStatePairTargetDecisionUnavailable {
                qualification_target: 6_272,
                absolute_maximum: 6_528,
            },
            OfflineCashActivationPreflightErrorV2::StateDirectInstanceVerifierUnavailable,
            OfflineCashActivationPreflightErrorV2::StateRecursiveFoldUnavailable,
            OfflineCashActivationPreflightErrorV2::StateTerminalReceiptUnavailable,
            OfflineCashActivationPreflightErrorV2::V2WireReleaseTypesUnavailable,
            OfflineCashActivationPreflightErrorV2::HelperScaffoldProofSizeExceeded {
                actual: 4_736,
                maximum: 3_264,
            },
            OfflineCashActivationPreflightErrorV2::CompactShaEvidenceUnavailable,
            OfflineCashActivationPreflightErrorV2::DerKeyMintGovernedRootClosureUnavailable,
            OfflineCashActivationPreflightErrorV2::RecursionBootstrapProtocolIdentityUnavailable,
            OfflineCashActivationPreflightErrorV2::GuardBundleEvidenceUnavailable,
            OfflineCashActivationPreflightErrorV2::StateEvidenceUnavailable,
            OfflineCashActivationPreflightErrorV2::CompleteArtifactInventoryUnavailable,
            OfflineCashActivationPreflightErrorV2::ProvingKeyArchiveCapExceeded {
                actual: 113_246_726,
                maximum: 67_108_864,
            },
            OfflineCashActivationPreflightErrorV2::CompleteArtifactSetSizeEvidenceUnavailable,
            OfflineCashActivationPreflightErrorV2::ArtifactEvidenceUnavailable,
            OfflineCashActivationPreflightErrorV2::MeasuredProcessRssEvidenceUnavailable,
            OfflineCashActivationPreflightErrorV2::DeviceEvidenceUnavailable,
            OfflineCashActivationPreflightErrorV2::VerificationUnavailable,
        ]
    );
}

#[test]
fn helper_scaffold_and_activation_preflight_fail_closed() {
    assert_eq!(OFFLINE_CASH_HELPER_SCAFFOLD_AUGMENTED_PROOF_BYTES_V2, 4_736);
    assert!(
        OFFLINE_CASH_HELPER_SCAFFOLD_AUGMENTED_PROOF_BYTES_V2
            > OFFLINE_CASH_CHILD_PROOF_ABSOLUTE_MAX_BYTES_V2
    );
    assert_eq!(
        preflight_offline_cash_activation_v2(),
        Err(OfflineCashActivationPreflightErrorV2::P256V3IntervalEvidenceUnavailable)
    );

    let source = include_str!("offline_cash_v2.rs");
    assert!(!source.contains("OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V2"));
    assert!(!source.contains("12_543"));
    assert!(source.contains("not an active wire maximum"));
    assert!(source.contains("does not establish final-STATE pair qualification"));
    assert!(source.contains("aggregate predecessor lineage"));
    assert!(source.contains("Err(OFFLINE_CASH_ACTIVATION_BLOCKERS_V2[0])"));
    assert!(!source.contains("Ok(())"));
    assert!(!source.contains("impl OfflineCashPairedProofVerifier"));

    let lineage_source = include_str!("offline_cash_v2_state_lineage.rs");
    assert!(lineage_source.contains("PastaIpaInstanceQueryV1::Direct"));
    assert!(lineage_source.contains("PastaIpaInstanceQueryV1::Direct => false"));
    assert!(lineage_source.contains("current proof accumulator is never"));
    assert!(lineage_source.contains("enum OfflineCashAuthenticatedBootstrapModeV2 {}"));
    assert!(lineage_source.contains("match authorization {}"));
    assert!(
        lineage_source.contains("Err(OfflineCashStateTerminalErrorV2::VerificationUnavailable)")
    );
    assert!(!lineage_source.contains("VerifierIPA"));
    assert!(!lineage_source.contains("verify_proof"));
    assert!(!lineage_source.contains("pub(super) current_accumulator:"));

    let backend = include_str!("offline_cash_v1/halo2_backend.rs");
    assert!(backend.contains("VerificationUnavailable"));
}
