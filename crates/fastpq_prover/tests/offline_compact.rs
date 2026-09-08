//! Public-consumer negative coverage for fixed offline quantity artifacts.

use fastpq_prover::{
    AXT_DEFAULT_PARAMETER, Error, ProofSemantics, PublicInputs, VerifyLimits,
    gadgets::public_transfer_statement::{
        PublicTransferLimits, TransferSmtBuildLimits, prepare_quantity_public_transfers,
        quantity_rows_for_public_preparation,
    },
    offline_compact::{
        BundleVerificationLimits, ExpectedAxtContext, ExpectedStatement, ProvingError,
        ProvingLimits, VerificationError, VerificationLimits, prove_quantity_axt_artifact,
        prove_quantity_ordinary_artifact, quantity_profile_id, verify_quantity_axt_artifact,
        verify_quantity_ordinary_artifact,
    },
    verify_axt_proof_envelope,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    DomainId,
    asset::AssetDefinitionId,
    fastpq::{
        FASTPQ_AXT_COMPACT_ARTIFACT_V1_SCHEMA_NAME,
        FASTPQ_ORDINARY_COMPACT_ARTIFACT_V1_SCHEMA_NAME, FastpqAxtCompactArtifactV1,
        FastpqAxtPreProofMirrorsV1, FastpqAxtPublicMetadataV1, FastpqCompactArtifactDecodeError,
        FastpqCompactArtifactDecodeLimits, FastpqOperationKind, FastpqOrdinaryCompactArtifactV1,
        FastpqPublicInputs, FastpqPublicTransferDeltaV1, FastpqPublicTransferStatementV1,
        FastpqPublicTransferTranscriptV1, FastpqStateTransition,
    },
    nexus::{AxtFastpqBinding, AxtProofEnvelope, DataSpaceId},
};
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use norito::{NoritoSerialize, core::DecodeLimits};

fn policy() -> VerificationLimits {
    VerificationLimits {
        transport: FastpqCompactArtifactDecodeLimits {
            max_wire_bytes: 1_000_000,
            max_bundle_frame_bytes: 500_000,
            norito: DecodeLimits::new(1_000_000, 1_000_000, 2_000_000, 8_000_000, 32),
        },
        public_statement: PublicTransferLimits::default(),
        bundle: BundleVerificationLimits {
            max_segments: 2,
            max_wire_bytes: 500_000,
            max_total_segment_bytes: 400_000,
            max_total_statement_bytes: 500_000,
            max_total_queries: 750,
            max_total_decode_allocation_charges: 8_000_000,
            segment: VerifyLimits {
                max_queries: 375,
                ..VerifyLimits::default()
            },
        },
        max_segment_decode_allocation_charges: 4_000_000,
        total_decode: DecodeLimits::new(1_000_000, 1_000_000, 4_000_000, 16_000_000, 32),
    }
}

// These independent public roots are fixture expectations, not ledger authority.
// The ordinary quantity preparation is real; no private witness or proof is built.
fn fixture() -> (FastpqPublicTransferStatementV1, ExpectedStatement) {
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let mut sender = Quantity::from(u128::MAX);
    let mut receiver = Quantity::zero();
    let mut deltas = Vec::new();
    for _ in 0..2 {
        let next_sender = sender.try_sub(&Quantity::one()).unwrap();
        let next_receiver = receiver.try_add(&Quantity::one()).unwrap();
        deltas.push(FastpqPublicTransferDeltaV1 {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: asset.clone(),
            amount: Quantity::one(),
            from_balance_before: sender,
            from_balance_after: next_sender.clone(),
            to_balance_before: receiver,
            to_balance_after: next_receiver.clone(),
        });
        sender = next_sender;
        receiver = next_receiver;
    }
    let claims = vec![FastpqPublicTransferTranscriptV1 {
        batch_hash: Hash::new(b"offline quantity source"),
        deltas,
        authority_digest: Hash::new(b"offline authority fixture"),
        poseidon_preimage_digest: None,
    }];
    let mut dsid = [0; 16];
    dsid[..8].copy_from_slice(&7_u64.to_le_bytes());
    let inputs = PublicInputs {
        dsid,
        slot: 19,
        old_root: Hash::prehashed([1; 32]).into(),
        new_root: Hash::prehashed([2; 32]).into(),
        perm_root: [3; 32],
        tx_set_hash: [4; 32],
    };
    let limits = PublicTransferLimits::default();
    let rows =
        quantity_rows_for_public_preparation(&claims, inputs, limits, limits.max_rows).unwrap();
    let prepared = prepare_quantity_public_transfers(
        &rows,
        &claims,
        inputs,
        ProofSemantics::StateTransition,
        limits,
    )
    .unwrap();
    let mut expected = ExpectedStatement {
        inputs: FastpqPublicInputs {
            dsid: inputs.dsid,
            slot: inputs.slot,
            old_root: inputs.old_root,
            new_root: inputs.new_root,
            perm_root: inputs.perm_root,
            tx_set_hash: inputs.tx_set_hash,
        },
        ordering_hash: prepared.ordering_hash().into(),
        public_statement_digest: [0; 32],
    };
    drop(prepared);
    let statement = FastpqPublicTransferStatementV1 {
        public_inputs: expected.inputs,
        ordering_hash: expected.ordering_hash,
        transitions: rows
            .into_iter()
            .map(|row| FastpqStateTransition {
                key: row.key,
                pre_value: row.pre_value,
                post_value: row.post_value,
                operation: FastpqOperationKind::Transfer,
            })
            .collect(),
        transcripts: claims,
    };
    expected.public_statement_digest =
        Hash::new(norito::encode_canonical(&statement).unwrap()).into();
    (statement, expected)
}

fn ordinary(statement: FastpqPublicTransferStatementV1, bundle_frame: Vec<u8>) -> Vec<u8> {
    norito::encode_canonical(&FastpqOrdinaryCompactArtifactV1 {
        profile_id: quantity_profile_id(),
        statement,
        bundle_frame,
    })
    .unwrap()
}

fn binding() -> AxtFastpqBinding {
    AxtFastpqBinding {
        parameter: AXT_DEFAULT_PARAMETER.into(),
        source_dsid: 7,
        source_dataspace: "source".into(),
        source_receipt_id: "receipt-1".into(),
        source_tx_commitment: "11".repeat(32),
        claim_type: "authorization".into(),
        claim_digest: "22".repeat(32),
        witness_commitment: "33".repeat(32),
        policy_commitment: "44".repeat(32),
        verified_effect_type: "restricted_effect".into(),
        corridor: "test-corridor".into(),
        verifier_id: "fastpq".into(),
        verifier_version: "v1".into(),
        target_dsids: vec![9],
        effect_binding: None,
        remote_spend_intent_commitments: Vec::new(),
    }
}

fn metadata() -> FastpqAxtPublicMetadataV1 {
    FastpqAxtPublicMetadataV1 {
        parameter: AXT_DEFAULT_PARAMETER.into(),
        entry_hash: [0x11; 32],
        committed_amount: None,
        expiry_slot: 23_u64.to_le_bytes(),
        manifest_root: [0x42; 32],
        da_commitment: [0; 33],
    }
}

fn mirrors() -> FastpqAxtPreProofMirrorsV1 {
    FastpqAxtPreProofMirrorsV1 {
        dsid: DataSpaceId::new(7),
        manifest_root: [0x42; 32],
        da_commitment: None,
        committed_amount: None,
        expiry_slot: Some(23),
    }
}

// A negative carrier fixture uses the exact public nominal wire identity. It
// never manufactures a successful child, or selects an AIR through facade inputs.
#[derive(NoritoSerialize)]
#[norito(schema_name = "fastpq_prover::compact_candidate::ShakeOrdinaryTransferBundleV1")]
struct CandidateCarrier {
    version: u16,
    intermediate_roots: Vec<[u8; 32]>,
    segments: Vec<Vec<u8>>,
}

fn carrier(segments: Vec<Vec<u8>>) -> Vec<u8> {
    norito::encode_canonical(&CandidateCarrier {
        version: 1,
        intermediate_roots: vec![Hash::prehashed([9; 32]).into()],
        segments,
    })
    .unwrap()
}

#[test]
fn public_quantity_verifier_requires_all_seven_independent_expected_inputs() {
    let (statement, expected) = fixture();
    let bytes = ordinary(statement, vec![0]);
    for index in 0..7 {
        let mut wrong = expected;
        match index {
            0 => wrong.inputs.dsid[0] ^= 1,
            1 => wrong.inputs.slot ^= 1,
            2 => wrong.inputs.old_root[0] ^= 1,
            3 => wrong.inputs.new_root[0] ^= 1,
            4 => wrong.inputs.perm_root[0] ^= 1,
            5 => wrong.inputs.tx_set_hash[0] ^= 1,
            _ => wrong.ordering_hash[0] ^= 1,
        }
        assert!(matches!(
            verify_quantity_ordinary_artifact(&bytes, wrong, policy()),
            Err(VerificationError::Verify(Error::PublicIoMismatch {
                field: "compact_model_public_io"
            }))
        ));
    }
}

#[test]
fn public_quantity_verifier_rejects_header_drift_before_malformed_child_decode() {
    for authority_header in [true, false] {
        let (mut statement, expected) = fixture();
        if authority_header {
            statement.transcripts[0].authority_digest = Hash::new(b"substituted authority header");
        } else {
            statement.transcripts[0].batch_hash = Hash::new(b"substituted call header");
        }
        assert_eq!(statement.public_inputs, expected.inputs);
        assert_eq!(statement.ordering_hash, expected.ordering_hash);
        let ordinary_bytes = ordinary(statement.clone(), vec![0]);
        let binding = binding();
        let metadata = metadata();
        let mirrors = mirrors();
        let axt_bytes = norito::encode_canonical(&FastpqAxtCompactArtifactV1 {
            profile_id: quantity_profile_id(),
            statement,
            binding: binding.clone(),
            metadata: metadata.clone(),
            mirrors,
            remote_spend_claims: None,
            bundle_frame: vec![0],
        })
        .unwrap();
        for result in [
            verify_quantity_ordinary_artifact(&ordinary_bytes, expected, policy()),
            verify_quantity_axt_artifact(
                &axt_bytes,
                expected,
                ExpectedAxtContext {
                    binding: &binding,
                    metadata: &metadata,
                    mirrors,
                    remote_spend_claims: None,
                },
                policy(),
            ),
        ] {
            assert!(matches!(
                result,
                Err(VerificationError::Verify(Error::PublicIoMismatch {
                    field: "compact_artifact_public_statement_digest",
                }))
            ));
        }
    }
}

#[test]
fn public_axt_verifier_compares_independent_metadata_mirrors_and_remote_presence() {
    let (statement, expected) = fixture();
    let binding = binding();
    let metadata = metadata();
    let mirrors = mirrors();
    let context = ExpectedAxtContext {
        binding: &binding,
        metadata: &metadata,
        mirrors,
        remote_spend_claims: None,
    };
    let artifact = FastpqAxtCompactArtifactV1 {
        profile_id: quantity_profile_id(),
        statement,
        binding: binding.clone(),
        metadata: metadata.clone(),
        mirrors,
        remote_spend_claims: None,
        bundle_frame: vec![0],
    };
    for case in 0..5 {
        let mut changed = artifact.clone();
        match case {
            0 => changed.binding.source_receipt_id.push('x'),
            1 => changed.metadata.entry_hash[0] ^= 1,
            2 => changed.mirrors.expiry_slot = Some(24),
            3 => changed.remote_spend_claims = Some(Vec::new()),
            _ => changed.metadata.committed_amount = Some([0; 16]),
        }
        let bytes = norito::encode_canonical(&changed).unwrap();
        assert!(matches!(
            verify_quantity_axt_artifact(&bytes, expected, context, policy()),
            Err(VerificationError::Verify(Error::PublicIoMismatch { .. }))
        ));
    }
}

#[test]
fn public_verifier_enforces_cumulative_queries_frames_and_statement_caps_before_children() {
    let (statement, expected) = fixture();
    let bytes = ordinary(statement, carrier(vec![vec![0, 0], vec![0, 0]]));
    for (label, limits) in [
        (
            "max_bundle_segments",
            VerificationLimits {
                bundle: BundleVerificationLimits {
                    max_segments: 1,
                    ..policy().bundle
                },
                ..policy()
            },
        ),
        (
            "max_bundle_queries",
            VerificationLimits {
                bundle: BundleVerificationLimits {
                    max_total_queries: 749,
                    ..policy().bundle
                },
                ..policy()
            },
        ),
    ] {
        assert!(matches!(
            verify_quantity_ordinary_artifact(&bytes, expected, limits),
            Err(VerificationError::Verify(Error::VerifierLimitExceeded { limit, .. })) if limit == label
        ));
    }
    // The shared carrier decoder enforces its derived cumulative element cap
    // before the later explicit sum check or any child decoding.
    let mut limits = policy();
    limits.bundle.max_total_segment_bytes = 3;
    assert!(matches!(
        verify_quantity_ordinary_artifact(&bytes, expected, limits),
        Err(VerificationError::Verify(Error::Encode(
            norito::Error::TotalElementsExceeded { .. }
        )))
    ));
    let mut limits = policy();
    limits.bundle.max_total_statement_bytes = 0;
    assert!(matches!(
        verify_quantity_ordinary_artifact(&bytes, expected, limits),
        Err(VerificationError::Verify(
            Error::VerifierLimitExceeded { .. }
        ))
    ));
}

#[test]
fn public_verifier_preserves_raw_and_enclosing_decode_limits() {
    let (statement, expected) = fixture();
    let bytes = ordinary(statement, vec![0, 0]);
    let zero = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32);
    let mut limits = policy();
    limits.transport.max_wire_bytes = bytes.len() - 1;
    let (result, usage) = norito::core::with_decode_limits_measured(zero, || {
        verify_quantity_ordinary_artifact(&bytes, expected, limits)
    });
    assert!(matches!(
        result,
        Err(VerificationError::Transport(
            FastpqCompactArtifactDecodeError::WireBytes { .. }
        ))
    ));
    assert_eq!(usage.total_allocated_bytes(), 0);
    assert!(matches!(
        norito::core::with_decode_limits_scope(zero, || verify_quantity_ordinary_artifact(
            &bytes,
            expected,
            policy()
        )),
        Err(VerificationError::Transport(
            FastpqCompactArtifactDecodeError::Norito(norito::Error::TotalAllocationExceeded { .. })
        ))
    ));
    let mut limits = policy();
    limits.transport.max_bundle_frame_bytes = 1;
    assert!(matches!(
        verify_quantity_ordinary_artifact(&bytes, expected, limits),
        Err(VerificationError::Transport(
            FastpqCompactArtifactDecodeError::BundleBytes { actual: 2, max: 1 }
        ))
    ));
}

#[test]
fn public_quantity_profile_and_route_are_fixed_and_codec_context_is_restored() {
    let expected_profile = quantity_profile_id();
    for flags in [0, 1, 2, 3, norito::core::default_encode_flags()] {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(quantity_profile_id(), expected_profile);
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
    }
    let (statement, expected) = fixture();
    let mut artifact = FastpqOrdinaryCompactArtifactV1 {
        profile_id: expected_profile,
        statement,
        bundle_frame: vec![0],
    };
    artifact.profile_id.0[0] ^= 1;
    assert!(matches!(
        verify_quantity_ordinary_artifact(
            &norito::encode_canonical(&artifact).unwrap(),
            expected,
            policy()
        ),
        Err(VerificationError::Transport(
            FastpqCompactArtifactDecodeError::ProfileMismatch
        ))
    ));
    artifact.profile_id = expected_profile;
    let bytes = norito::encode_canonical(&artifact).unwrap();
    let binding = binding();
    let metadata = metadata();
    assert!(matches!(
        verify_quantity_axt_artifact(
            &bytes,
            expected,
            ExpectedAxtContext {
                binding: &binding,
                metadata: &metadata,
                mirrors: mirrors(),
                remote_spend_claims: None
            },
            policy()
        ),
        Err(VerificationError::Transport(
            FastpqCompactArtifactDecodeError::Norito(_)
        ))
    ));
}

#[test]
fn public_verifier_never_accepts_missing_or_empty_child_occurrences() {
    let (statement, expected) = fixture();
    for children in [Vec::new(), vec![vec![0]], vec![vec![0], Vec::new()]] {
        let bytes = ordinary(statement.clone(), carrier(children));
        assert!(matches!(
            verify_quantity_ordinary_artifact(&bytes, expected, policy()),
            Err(VerificationError::Verify(Error::TransferInvariant { .. }))
        ));
    }
}

#[test]
fn public_offline_facade_does_not_qualify_either_schema_at_existing_axt_ingress() {
    for schema in [
        FASTPQ_ORDINARY_COMPACT_ARTIFACT_V1_SCHEMA_NAME,
        FASTPQ_AXT_COMPACT_ARTIFACT_V1_SCHEMA_NAME,
    ] {
        let mut header = norito::encode_canonical(&0_u8).unwrap();
        header[6..22].copy_from_slice(&norito::core::schema_hash_for_name(schema));
        header.truncate(norito::core::Header::SIZE);
        header[23..31].copy_from_slice(&u64::MAX.to_le_bytes());
        let envelope = AxtProofEnvelope {
            dsid: DataSpaceId::new(7),
            manifest_root: [0x42; 32],
            da_commitment: None,
            proof: header,
            fastpq_binding: Some(binding()),
            committed_amount: None,
            amount_commitment: None,
        };
        assert!(matches!(
            verify_axt_proof_envelope(&envelope),
            Err(Error::UnqualifiedCompactArtifact { schema: rejected }) if rejected == schema
        ));
    }
}

#[test]
fn public_producer_rejects_limits_expectation_drift_and_false_roots_without_a_trace() {
    // One public producer test owns these serial negative requests. Concurrent
    // producer calls intentionally return Busy, so parallel test cases would
    // obscure the particular admission failure being asserted here.
    let (statement, expected) = fixture();
    let proving = ProvingLimits {
        private_smt: TransferSmtBuildLimits::for_update_limit(4).unwrap(),
        max_total_trace_cells: usize::MAX,
        max_segment_charge_bytes: usize::MAX,
    };
    let mut verification = policy();
    verification.transport.max_wire_bytes = 20 * 1024 * 1024;
    verification.transport.max_bundle_frame_bytes = 16 * 1024 * 1024;
    verification.transport.norito = DecodeLimits::new(
        20 * 1024 * 1024,
        20 * 1024 * 1024,
        25 * 1024 * 1024,
        96 * 1024 * 1024,
        32,
    );
    verification.total_decode = DecodeLimits::new(
        20 * 1024 * 1024,
        20 * 1024 * 1024,
        30 * 1024 * 1024,
        192 * 1024 * 1024,
        32,
    );
    verification.bundle.max_wire_bytes = 16 * 1024 * 1024;
    verification.bundle.max_total_segment_bytes = 16 * 1024 * 1024;
    verification.bundle.segment.max_proof_bytes = 5 * 1024 * 1024;
    for name in [
        "max_public_transfer_rows",
        "max_public_transfer_transcripts",
        "max_public_transfer_deltas",
        "max_bundle_segments",
        "max_queries",
        "max_bundle_queries",
        "max_proof_bytes",
        "max_bundle_segment_bytes",
        "max_compact_prover_trace_cells",
        "max_compact_prover_segment_charge_bytes",
        "max_compact_producer_statement_bytes",
    ] {
        let mut limits = verification;
        let mut work = proving;
        match name {
            "max_public_transfer_rows" => limits.public_statement.max_rows = 0,
            "max_public_transfer_transcripts" => limits.public_statement.max_transcripts = 0,
            "max_public_transfer_deltas" => limits.public_statement.max_deltas = 0,
            "max_bundle_segments" => limits.bundle.max_segments = 1,
            "max_queries" => limits.bundle.segment.max_queries = 374,
            "max_bundle_queries" => limits.bundle.max_total_queries = 749,
            "max_proof_bytes" => limits.bundle.segment.max_proof_bytes = 0,
            "max_bundle_segment_bytes" => limits.bundle.max_total_segment_bytes = 0,
            "max_compact_prover_trace_cells" => work.max_total_trace_cells = 0,
            "max_compact_prover_segment_charge_bytes" => work.max_segment_charge_bytes = 0,
            "max_compact_producer_statement_bytes" => limits.public_statement.max_public_bytes = 0,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                prove_quantity_ordinary_artifact(&statement, expected, work, limits),
                Err(ProvingError::Prove(Error::VerifierLimitExceeded { limit, .. })) if limit == name
            ),
            "expected public producer limit {name}"
        );
    }
    for name in [
        "max_compact_producer_segment_decode_allocation_charges",
        "max_compact_producer_bundle_decode_allocation_charges",
    ] {
        let mut limited = verification;
        if name == "max_compact_producer_segment_decode_allocation_charges" {
            limited.max_segment_decode_allocation_charges = 0;
        } else {
            limited.bundle.max_total_decode_allocation_charges = 0;
        }
        assert!(
            matches!(
                prove_quantity_ordinary_artifact(&statement, expected, proving, limited),
                Err(ProvingError::Prove(Error::VerifierLimitExceeded { limit, max: 0, .. }))
                    if limit == name
            ),
            "expected public producer decoder limit {name}"
        );
    }
    for (total, names) in [
        (
            false,
            [
                "max_compact_producer_transport_decode_sequence_elements",
                "max_compact_producer_transport_decode_field_bytes",
                "max_compact_producer_transport_decode_total_elements",
                "max_compact_producer_transport_decode_allocation_charges",
                "max_compact_producer_transport_decode_nesting_depth",
            ],
        ),
        (
            true,
            [
                "max_compact_producer_total_decode_sequence_elements",
                "max_compact_producer_total_decode_field_bytes",
                "max_compact_producer_total_decode_total_elements",
                "max_compact_producer_total_decode_allocation_charges",
                "max_compact_producer_total_decode_nesting_depth",
            ],
        ),
    ] {
        for (dimension, name) in names.into_iter().enumerate() {
            let mut limited = verification;
            let original = if total {
                limited.total_decode
            } else {
                limited.transport.norito
            };
            let mut values = [
                original.max_sequence_elements(),
                original.max_field_bytes(),
                original.max_total_elements(),
                original.max_total_allocated_bytes(),
                original.max_nesting_depth(),
            ];
            values[dimension] = 0;
            let decode = DecodeLimits::new(values[0], values[1], values[2], values[3], values[4]);
            if total {
                limited.total_decode = decode;
            } else {
                limited.transport.norito = decode;
            }
            assert!(
                matches!(
                    prove_quantity_ordinary_artifact(&statement, expected, proving, limited),
                    Err(ProvingError::Prove(Error::VerifierLimitExceeded { limit, max: 0, .. }))
                        if limit == name
                ),
                "expected public producer decoder limit {name}"
            );
        }
    }
    for index in 0..7 {
        let mut wrong = expected;
        match index {
            0 => wrong.inputs.dsid[0] ^= 1,
            1 => wrong.inputs.slot ^= 1,
            2 => wrong.inputs.old_root[0] ^= 1,
            3 => wrong.inputs.new_root[0] ^= 1,
            4 => wrong.inputs.perm_root[0] ^= 1,
            5 => wrong.inputs.tx_set_hash[0] ^= 1,
            _ => wrong.ordering_hash[0] ^= 1,
        }
        assert!(matches!(
            prove_quantity_ordinary_artifact(&statement, wrong, proving, verification),
            Err(ProvingError::Prove(Error::PublicIoMismatch {
                field: "compact_model_public_io"
            }))
        ));
    }
    let mut changed = statement.clone();
    changed.transcripts[0].authority_digest = Hash::new(b"changed producer authority");
    assert!(matches!(
        prove_quantity_ordinary_artifact(&changed, expected, proving, verification),
        Err(ProvingError::Prove(Error::PublicIoMismatch {
            field: "compact_public_statement_digest"
        }))
    ));
    // The fixture's independent endpoints intentionally do not describe its
    // touched tree. A producer must reject them, not silently replace them.
    assert!(matches!(
        prove_quantity_ordinary_artifact(&statement, expected, proving, verification),
        Err(ProvingError::Prove(Error::TransferInvariant { details }))
            if details.contains("derived transfer SMT roots differ from public inputs")
    ));
    let binding = binding();
    let metadata = metadata();
    let rejected = prove_quantity_axt_artifact(
        &statement,
        expected,
        ExpectedAxtContext {
            binding: &binding,
            metadata: &metadata,
            mirrors: mirrors(),
            remote_spend_claims: None,
        },
        proving,
        verification,
    );
    assert!(
        matches!(&rejected,
            Err(ProvingError::Prove(Error::InvalidProofSemantics { profile, details }))
                if *profile == "axt_opaque_effect"
                    && details == "generic AXT consumers require a witnessed transfer claim; opaque effect carriers require independent authenticated-effect validation"
        ),
        "opaque AXT producer returned {rejected:?}"
    );
    // The earlier fixture selects the opaque route. Select a canonical transfer
    // binding here, retaining an intentionally different source commitment, so
    // the next rejection specifically exercises original transcript linkage.
    let mut transfer_binding = binding;
    transfer_binding.claim_type = "tx_predicate".into();
    fastpq_prover::validate_axt_transfer_claim_binding(&transfer_binding).unwrap();
    assert_ne!(
        statement.transcripts[0].batch_hash.as_ref(),
        metadata.entry_hash.as_slice()
    );
    let rejected = prove_quantity_axt_artifact(
        &statement,
        expected,
        ExpectedAxtContext {
            binding: &transfer_binding,
            metadata: &metadata,
            mirrors: mirrors(),
            remote_spend_claims: None,
        },
        proving,
        verification,
    );
    assert!(
        matches!(&rejected,
            Err(ProvingError::Prove(Error::InvalidAxtBinding { details }))
                if details == "transfer transcript batch_hash does not match source_tx_commitment"
        ),
        "mismatched AXT source commitment returned {rejected:?}"
    );
}
