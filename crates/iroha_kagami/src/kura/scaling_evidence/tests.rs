//! Signed Native source, output, authority, budget and complete-cohort controls.
use super::*;
use fixture::{Fixture, Height, limits, mutate_height};
use iroha_data_model::block::{execution_output::ExecutionOutputV1, lane_consensus::LanePhaseV1};
use norito::codec::Encode as _;

fn push_with(
    verifier: &mut ScalingProofVerifier,
    height: &Height,
    queries: &[Vec<u8>],
) -> Result<()> {
    verifier.push_height(
        &norito::encode_canonical(&height.proof).unwrap(),
        &height.block.encode_wire().unwrap(),
        &height.evidence,
        &queries.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
}

#[test]
fn fixture_finality_commits_exact_network_inputs_and_typed_outputs() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes);
        for height in &fixture.heights {
            let commitment = &height
                .proof
                .finality_artifact
                .commit_qc
                .execution_commitment;
            assert_eq!(
                commitment.transaction_input_commitment,
                height.block.network_input_merkle_commitment()
            );
            assert_eq!(
                commitment.transaction_output_commitment,
                height.block.output_merkle_commitment()
            );
            for bytes in height.queries() {
                let query: CommittedTransaction = canonical(&bytes).unwrap();
                assert!(
                    query.verify_inclusion_in_authenticated_execution(&height.block, commitment)
                );
            }
        }
    }
}

#[test]
fn one_and_four_lane_signed_runs_authenticate_every_warmup_and_measurement_effect() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes);
        assert_eq!(
            fixture.heights.len(),
            1 + 8 / lanes,
            "one input per distinct route and carrier"
        );
        let mut verifier = fixture.start(fixture.plan(), limits());
        fixture.push(&mut verifier).unwrap();
        let complete = verifier.finish().unwrap();
        assert_eq!(complete.rows().len(), 8);
        for phase in [WorkloadPhase::Warmup, WorkloadPhase::Measurement] {
            assert_eq!(
                complete
                    .rows()
                    .iter()
                    .filter(|row| row.phase == phase)
                    .count(),
                4
            );
        }
        for (row, (logical, tx, route, phase)) in complete.rows().iter().zip(&fixture.requests) {
            assert_eq!(&row.logical_id, logical);
            assert_eq!(row.entrypoint_hash, tx.hash_as_entrypoint());
            assert_eq!(&row.authority, tx.authority());
            assert_eq!(row.phase, *phase);
            assert_eq!(
                (row.lane_id, row.dataspace_id),
                (route.lane_id, route.dataspace_id)
            );
            let carrier = &fixture.heights[row.carrier_height as usize - 1].block;
            assert_eq!(row.carrier_hash, carrier.hash());
            assert_eq!(row.admission_carrier_hash, fixture.heights[0].block.hash());
            let group = &carrier
                .execution_context()
                .unwrap()
                .native_lane_decisions
                .as_ref()
                .unwrap()
                .groups[row.leaf_index as usize];
            assert_eq!(
                row.input_descriptor_hash,
                group.payload.descriptor.canonical_hash().unwrap()
            );
            assert_eq!(
                row.instance_id,
                group.payload.descriptor.slots[0].instance_id
            );
            assert!(norito::encode_canonical(row).unwrap().len() as u64 <= ROW_RESERVATION);
        }
        for binding in &fixture.plan().active_lanes {
            assert_eq!(
                complete
                    .rows()
                    .iter()
                    .filter(|row| row.lane_id == binding.lane_id)
                    .count(),
                8 / lanes
            );
        }
        let decoded: Vec<AuthenticatedRequest> = canonical(complete.canonical_rows()).unwrap();
        assert_eq!(decoded.len(), 8);
        assert!(
            complete.input_bytes() > fixture.heights[1].block.encode_wire().unwrap().len() as u64
        );
    }
}

#[test]
fn independent_network_context_interval_and_required_budget_fail_before_evidence() {
    let fixture = Fixture::new(1);
    for change in 0..9 {
        let mut plan = fixture.plan();
        let mut cap = limits();
        match change {
            0 => cap.admitted_proof_bytes = 0,
            1 => cap.admitted_proof_bytes = MAX_PROOF_BYTES + 1,
            2 => cap.input_bytes = u64::MAX,
            3 => cap.output_bytes = 1,
            4 => plan.first_height = 0,
            5 => plan.last_height = u64::MAX,
            6 => cap.heights = 1,
            7 => cap.requests = 7,
            _ => plan.scheduled[0].logical_id = "A".repeat(64),
        }
        assert!(
            ScalingProofVerifier::new(plan, cap).is_err(),
            "invalid admission {change}"
        );
    }
    for change in 0..2 {
        let mut plan = fixture.plan();
        if change == 0 {
            plan.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new(b"wrong network"),
            ));
        } else {
            plan.first_context =
                HeightContextId(HashOf::from_untyped_unchecked(Hash::new(b"wrong context")));
        }
        match ScalingProofVerifier::new(plan, limits()) {
            Err(_) => assert_eq!(change, 0),
            Ok(mut verifier) => {
                assert!(fixture.heights[0].push(&mut verifier).is_err());
                assert!(verifier.finish().is_err());
            }
        }
    }
}

#[test]
fn no_missing_reordered_duplicate_or_unsigned_finality_prefix_can_finish() {
    let fixture = Fixture::new(1);
    let mut baseline = fixture.start(fixture.plan(), limits());
    fixture.push(&mut baseline).unwrap();
    assert_eq!(baseline.finish().unwrap().rows().len(), 8);
    let mut missing = ScalingProofVerifier::new(fixture.plan(), limits()).unwrap();
    assert!(fixture.push(&mut missing).is_err());
    assert!(missing.finish().is_err());
    assert!(fixture.start(fixture.plan(), limits()).finish().is_err());
    let mut duplicate = fixture.start(fixture.plan(), limits());
    assert!(fixture.heights[0].push(&mut duplicate).is_err());
    assert!(duplicate.finish().is_err());
    let mut unsigned = fixture.heights[1].clone();
    unsigned
        .proof
        .finality_artifact
        .commit_qc
        .aggregate_signature
        .clear();
    let mut verifier = fixture.start(fixture.plan(), limits());
    assert!(unsigned.push(&mut verifier).is_err());
    assert!(
        fixture.push(&mut verifier).is_err(),
        "invalid signature permanently poisons owner"
    );
    assert!(verifier.finish().is_err());
}

#[test]
fn canonical_decoders_reject_headerless_trailing_and_oversized_inputs() {
    let fixture = Fixture::new(1);
    let height = &fixture.heights[0];
    let finality = norito::encode_canonical(&height.proof).unwrap();
    let block = height.block.encode_wire().unwrap();
    for invalid in [
        height.proof.encode(),
        [finality.clone(), vec![0]].concat(),
        vec![0; MAX_FINALITY_BYTES + 1],
    ] {
        let mut verifier = ScalingProofVerifier::new(fixture.plan(), limits()).unwrap();
        assert!(
            verifier
                .push_height(&invalid, &block, &height.evidence, &[])
                .is_err()
        );
        assert!(verifier.finish().is_err());
    }
    let mut extra = block.clone();
    extra.push(0);
    let mut verifier = ScalingProofVerifier::new(fixture.plan(), limits()).unwrap();
    assert!(
        verifier
            .push_height(&finality, &extra, &height.evidence, &[])
            .is_err()
    );
    for contexts in [
        Vec::new(),
        height.evidence[norito::core::Header::SIZE..].to_vec(),
        [height.evidence.clone(), vec![0]].concat(),
        vec![0; MAX_FINALITY_BYTES + 1],
    ] {
        let mut verifier = ScalingProofVerifier::new(fixture.plan(), limits()).unwrap();
        assert!(
            verifier
                .push_height(&finality, &block, &contexts, &[])
                .is_err()
        );
        assert!(verifier.finish().is_err());
    }
}

#[test]
fn same_header_does_not_authenticate_substituted_result_bearing_wire() {
    let mut fixture = Fixture::new(1);
    let mut verifier = fixture.start(fixture.plan(), limits());
    let old = fixture.heights[1].block.header();
    fixture.heights[1]
        .block
        .add_signature(iroha_data_model::block::BlockSignature::new(
            1,
            iroha_crypto::SignatureOf::try_from_hash(fixture.keys[1].private_key(), old.hash())
                .unwrap(),
        ))
        .unwrap();
    assert_eq!(fixture.heights[1].block.header(), old);
    assert!(fixture.push(&mut verifier).is_err());
    assert!(verifier.finish().is_err());
}

#[test]
fn exact_first_admission_context_witness_and_decision_joins_are_required() {
    for change in 0..9 {
        let mut fixture = Fixture::new(1);
        let mut verifier = fixture.start(fixture.plan(), limits());
        if change == 0 {
            fixture.heights[1].evidence.clear();
        } else if change == 1 {
            fixture.heights[1].evidence = fixture.heights[0].evidence.clone();
        } else {
            mutate_height(&mut fixture.heights[1], &fixture.keys, |raw| {
                let group = &mut raw
                    .payload
                    .execution_context
                    .as_mut()
                    .unwrap()
                    .native_lane_decisions
                    .as_mut()
                    .unwrap()
                    .groups[0];
                match change {
                    2 => {
                        group.payload.descriptor.admission_carrier_hash =
                            HashOf::from_untyped_unchecked(Hash::new(b"other source carrier"))
                    }
                    3 => {
                        group.payload.descriptor.admitted_input_hash =
                            Hash::new(b"other complete input")
                    }
                    4 => group.payload.descriptor.admission_priority.admission_index += 1,
                    5 => {
                        group.payload.descriptor.slots[0].instance_id = Hash::new(b"other opening")
                    }
                    6 => group.decisions[0].commit_qc.shares[0].signature[0] ^= 1,
                    7 => {
                        group.decisions[0].manifest.value.origin_producer =
                            (group.decisions[0].manifest.value.origin_producer + 1) % 4
                    }
                    _ => group.decisions[0].manifest.value.origin_view += 1,
                }
            });
        }
        assert!(
            fixture.push(&mut verifier).is_err(),
            "exact source/context/Decision control {change}"
        );
        assert!(verifier.finish().is_err());
    }
}

#[test]
fn globally_authenticated_native_execution_cannot_replace_independent_route_authority() {
    let fixture = Fixture::new(4);
    for change in 0..8 {
        let mut plan = fixture.plan();
        match change {
            0 => plan.nexus_amx_context_hash = Hash::new(b"other context"),
            1 => plan.active_lanes[0].dataspace_id = DataSpaceId::new(99),
            2 => plan.active_lanes[0].incarnation = Hash::new(b"other incarnation"),
            3 => plan.active_lanes[0].activation_height = 2,
            4 => plan.lane_authorities.rosters[0].validators.swap(0, 1),
            5 => plan.execution_policy_hash = Hash::new(b"other policy"),
            6 => {
                plan.lane_authorities.rosters[0].validator_set_hash =
                    HashOf::from_untyped_unchecked(Hash::new(b"other committee"))
            }
            _ => plan.lane_authorities.rosters[0].validator_set_hash_version = 2,
        }
        match ScalingProofVerifier::new(plan, limits()) {
            Err(_) => {}
            Ok(mut verifier) => {
                fixture.heights[0].push(&mut verifier).unwrap();
                assert!(
                    fixture.push(&mut verifier).is_err(),
                    "authority control {change}"
                );
                assert!(verifier.finish().is_err());
            }
        }
    }
}

#[test]
fn globally_signed_native_batch_requires_complete_source_and_output_alignment() {
    for change in 0..10 {
        let mut fixture = Fixture::new(4);
        let mut verifier = fixture.start(fixture.plan(), limits());
        let original_queries = fixture.heights[1].queries();
        mutate_height(&mut fixture.heights[1], &fixture.keys, |raw| {
            let batch = raw
                .payload
                .execution_context
                .as_mut()
                .unwrap()
                .native_lane_decisions
                .as_mut()
                .unwrap();
            let group = &mut batch.groups[0];
            match change {
                0 => {
                    group.payload.descriptor.slots.pop();
                }
                1 => {
                    group.decisions.pop();
                }
                2 => {
                    raw.result.as_mut().unwrap().outputs.pop();
                }
                3 => {
                    let row = raw.result.as_mut().unwrap().outputs[0].clone();
                    raw.result.as_mut().unwrap().outputs.push(row);
                }
                4 => {
                    if let ExecutionOutputV1::Network(row) =
                        &mut raw.result.as_mut().unwrap().outputs[0]
                    {
                        row.input_index = 1;
                    }
                }
                5 => {
                    group.payload.input.certificate.attestations.pop();
                }
                6 => {
                    group.payload.input.certificate.binding.entrypoint_hash =
                        HashOf::from_untyped_unchecked(Hash::new(b"other entrypoint"))
                }
                7 => group.payload.descriptor.slots[0].route.lane_id = LaneId::new(99),
                8 => group.decisions[0].commit_qc.statement.phase = LanePhaseV1::Prepare,
                _ => {
                    batch.groups.swap(0, 1);
                }
            }
            raw.result.as_mut().unwrap().output_merkle = raw
                .result
                .as_ref()
                .unwrap()
                .outputs
                .iter()
                .map(HashOf::new)
                .collect();
        });
        assert!(
            push_with(&mut verifier, &fixture.heights[1], &original_queries).is_err(),
            "source/output alignment control {change}"
        );
        assert!(verifier.finish().is_err());
    }
}

#[test]
fn compact_query_proofs_must_match_full_network_output_and_same_input_index() {
    for change in 0..7 {
        let fixture = Fixture::new(4);
        let height = &fixture.heights[1];
        let mut verifier = fixture.start(fixture.plan(), limits());
        let mut queries = height.queries();
        if change == 0 {
            queries.swap(0, 1);
        } else if change == 1 {
            queries.pop();
        } else if change == 2 {
            queries.push(queries[0].clone());
        } else {
            let mut q: CommittedTransaction = canonical(&queries[0]).unwrap();
            let other: CommittedTransaction = canonical(&queries[1]).unwrap();
            match change {
                3 => {
                    if let ExecutionOutputV1::Network(row) = &mut q.output {
                        row.input_index = 1;
                    }
                }
                4 => q.output_proof = other.output_proof,
                5 => q.entrypoint = other.entrypoint,
                _ => q.output_hash = other.output_hash,
            }
            queries[0] = norito::encode_canonical(&q).unwrap();
        }
        assert!(
            push_with(&mut verifier, height, &queries).is_err(),
            "typed query control {change}"
        );
        assert!(verifier.finish().is_err());
    }
}

#[test]
fn ordinary_inclusion_never_substitutes_for_scheduled_lane_execution() {
    let mut fixture = Fixture::new(1);
    let mut builder =
        iroha_data_model::block::builder::BlockBuilder::new(fixture.heights[1].block.header());
    builder.push_transaction(fixture.requests[0].1.clone());
    let mut block = builder.build(BTreeSet::new());
    let outputs = fixture::successful_outputs(&block);
    fixture::attach_outputs(&mut block, outputs, &fixture.keys);
    fixture.heights[1].block = block;
    fixture.heights[1].contexts = Default::default();
    fixture.heights[1].resign(&fixture.keys);
    assert!(fixture.heights[1].block.execution_context().is_none());
    assert!(
        fixture.heights[1]
            .proof
            .finality_artifact
            .commit_qc
            .execution_commitment
            .merge_carrier
            .is_none()
    );
    let mut unrelated = fixture.plan();
    unrelated.scheduled.remove(0);
    let mut baseline = fixture.start(unrelated, limits());
    fixture.heights[1].push(&mut baseline).unwrap();
    assert!(baseline.finish().is_err());
    let mut verifier = fixture.start(fixture.plan(), limits());
    let rejected = fixture.heights[1].push(&mut verifier);
    assert_eq!(
        rejected.unwrap_err().to_string(),
        "scheduled transaction used ordinary fallback"
    );
    assert!(verifier.finish().is_err());
}

#[test]
fn required_signed_instruction_and_duplicate_logical_identity_are_checked_at_admission() {
    let fixture = Fixture::new(1);
    let mut duplicate = fixture.plan();
    duplicate.scheduled[1].logical_id = duplicate.scheduled[0].logical_id.clone();
    assert!(ScalingProofVerifier::new(duplicate, limits()).is_err());
    let mut changed = fixture.plan();
    changed.scheduled[0].logical_id = "f".repeat(64);
    assert!(
        ScalingProofVerifier::new(changed, limits()).is_err(),
        "same signed bytes cannot be relabelled as a different effect"
    );
    let mut malformed = fixture.plan();
    malformed.scheduled[0].signed_transaction.push(0);
    assert!(ScalingProofVerifier::new(malformed, limits()).is_err());
    let mut outside = fixture.plan();
    outside.scheduled[0].route.lane_id = LaneId::new(88);
    assert!(ScalingProofVerifier::new(outside, limits()).is_err());
}

#[test]
fn missing_schedule_rows_and_duplicate_execution_cannot_be_hidden_at_finish() {
    let fixture = Fixture::new(1);
    let mut missing = fixture.plan();
    missing.scheduled.pop();
    let mut verifier = fixture.start(missing, limits());
    assert!(fixture.push(&mut verifier).is_err());
    assert!(verifier.finish().is_err());
    let mut extended = fixture.plan();
    extended.last_height += 1;
    let mut verifier = fixture.start(extended, limits());
    fixture.push(&mut verifier).unwrap();
    assert!(
        verifier.finish().is_err(),
        "all rows do not erase incomplete finality interval"
    );
    let mut duplicate = Fixture::new(4);
    let mut verifier = duplicate.start(duplicate.plan(), limits());
    mutate_height(&mut duplicate.heights[1], &duplicate.keys, |raw| {
        let groups = &mut raw
            .payload
            .execution_context
            .as_mut()
            .unwrap()
            .native_lane_decisions
            .as_mut()
            .unwrap()
            .groups;
        groups[1] = groups[0].clone();
    });
    assert!(
        duplicate.push(&mut verifier).is_err(),
        "duplicate route/input cannot execute twice"
    );
}

#[test]
fn exact_total_input_allocation_accepts_and_one_byte_short_poisons() {
    let fixture = Fixture::new(1);
    let mut measured = fixture.start(fixture.plan(), limits());
    fixture.push(&mut measured).unwrap();
    let used = measured.finish().unwrap().input_bytes();
    let mut exact = limits();
    exact.input_bytes = used;
    exact.admitted_proof_bytes = used + exact.output_bytes;
    let mut verifier = fixture.start(fixture.plan(), exact);
    fixture.push(&mut verifier).unwrap();
    assert_eq!(verifier.finish().unwrap().input_bytes(), used);
    exact.input_bytes -= 1;
    let mut verifier = fixture.start(fixture.plan(), exact);
    assert!(fixture.push(&mut verifier).is_err());
    assert!(verifier.finish().is_err());
}

#[test]
fn globally_resigned_execution_rejection_is_not_an_admission_exemption() {
    let mut fixture = Fixture::new(4);
    let mut verifier = fixture.start(fixture.plan(), limits());
    mutate_height(&mut fixture.heights[1], &fixture.keys, |raw| {
        let result = raw.result.as_mut().unwrap();
        if let ExecutionOutputV1::Network(row) = &mut result.outputs[0] {
            row.result.0 = Err(
                iroha_data_model::transaction::error::TransactionRejectionReason::IvmExecution(
                    iroha_data_model::transaction::error::IvmExecutionFail {
                        reason: "fixture rejection".into(),
                    },
                ),
            );
        }
        result.output_merkle = result.outputs.iter().map(HashOf::new).collect();
    });
    assert!(
        fixture.push(&mut verifier).is_err(),
        "authenticated rejected output fails complete useful cohort"
    );
    assert!(verifier.finish().is_err());
}

#[test]
fn exact_three_of_four_global_signatures_pops_and_parent_are_mandatory() {
    for change in 0..5 {
        let fixture = Fixture::new(1);
        let mut verifier = fixture.start(fixture.plan(), limits());
        let mut height = fixture.heights[1].clone();
        let proof = &mut height.proof;
        match change {
            0 => {
                proof.finality_artifact.commit_qc.signers.pop();
            }
            1 => proof.finality_artifact.commit_qc.signers = vec![0, 1, 1],
            2 => proof.finality_artifact.validator_set_pops[0][0] ^= 1,
            3 => proof.finality_artifact.height_context.parent_commit_qc = None,
            _ => proof.finality_artifact.commit_qc.aggregate_signature[0] ^= 1,
        }
        assert!(
            height.push(&mut verifier).is_err(),
            "finality authority control {change}"
        );
        assert!(verifier.finish().is_err());
    }
}

#[test]
fn unsigned_summary_and_observed_accounts_have_no_proof_input_path() {
    let fixture = Fixture::new(1);
    let mut verifier = fixture.start(fixture.plan(), limits());
    assert!(
        verifier
            .push_height(
                b"{\"verified\":true}",
                &fixture.heights[1].block.encode_wire().unwrap(),
                &fixture.heights[1].evidence,
                &[]
            )
            .is_err()
    );
    assert!(verifier.finish().is_err());
    let mut no_work = Fixture::new(1);
    let mut verifier = no_work.start(no_work.plan(), limits());
    let mut block =
        iroha_data_model::block::builder::BlockBuilder::new(no_work.heights[1].block.header())
            .build(BTreeSet::new());
    fixture::attach_outputs(&mut block, Vec::new(), &no_work.keys);
    no_work.heights[1].block = block;
    no_work.heights[1].contexts = Default::default();
    no_work.heights[1].resign(&no_work.keys);
    no_work.heights[1].push(&mut verifier).unwrap();
    assert!(
        verifier.finish().is_err(),
        "authenticated no-work carrier cannot finish cohort"
    );
    assert!(std::mem::size_of::<AuthenticatedRequest>() < ROW_RESERVATION as usize);
}

// This helper is shared by the four adjacent owner test modules. Schema selection
// is checked independently of decoding, using explicit names and digest goldens.
pub(super) fn assert_declared_scaling_frame<T, Other>(
    value: &T,
    expected_name: &str,
    expected_hash: [u8; 16],
) -> T
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
    Other: norito::NoritoSchema,
{
    use norito::core::{DecodeFlagsGuard, Header, header_flags};
    use norito::schema::identity::frame_hash;

    assert_eq!(T::nominal_name(), expected_name);
    assert_eq!(T::frame_name(), expected_name);
    assert_eq!(frame_hash::<T>(), expected_hash);
    let canonical = norito::encode_canonical(value).unwrap();
    let header = Header::read(canonical.as_slice()).unwrap();
    assert_eq!(header.magic, norito::core::MAGIC);
    assert_eq!(header.major, norito::core::VERSION_MAJOR);
    assert_eq!(header.minor, norito::core::VERSION_MINOR);
    assert_eq!(header.schema, expected_hash);
    assert_eq!(header.compression, norito::core::Compression::None);
    // Flags belong to this value's canonical encoding, including any dynamic
    // bits it actually uses. The default-layout writer is an independent entry.
    let default_frame = {
        let _layout = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        norito::to_bytes(value).unwrap()
    };
    assert_eq!(canonical, default_frame);
    assert_eq!(
        header.flags,
        Header::read(default_frame.as_slice()).unwrap().flags
    );
    assert_eq!(norito::canonical_frame_len(value).unwrap(), canonical.len());
    let decoded: T = norito::decode_canonical(&canonical).unwrap();
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), canonical);

    for flags in [
        0,
        header_flags::PACKED_SEQ | header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN,
    ] {
        let _ambient = DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
        assert_eq!(norito::encode_canonical(value).unwrap(), canonical);
        assert_eq!(norito::canonical_frame_len(value).unwrap(), canonical.len());
        let roundtrip: T = norito::decode_canonical(&canonical).unwrap();
        assert_eq!(norito::encode_canonical(&roundtrip).unwrap(), canonical);
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));
    }

    // Substitute another real scaling owner's declared identity while retaining
    // exact payload, checksum, version, flags, length and alignment bytes.
    let wrong_hash = frame_hash::<Other>();
    assert_ne!(wrong_hash, expected_hash);
    let mut wrong = canonical.clone();
    wrong[6..22].copy_from_slice(&wrong_hash);
    assert_eq!(&wrong[..6], &canonical[..6]);
    assert_eq!(&wrong[22..], &canonical[22..]);
    assert_eq!(Header::read(wrong.as_slice()).unwrap().schema, wrong_hash);
    norito::core::from_bytes_view(&wrong).expect("wrong-schema frame is structurally valid");
    assert!(matches!(
        norito::decode_canonical::<T>(&wrong),
        Err(norito::Error::SchemaMismatch)
    ));
    decoded
}

#[test]
fn authenticated_request_and_vector_declare_distinct_v1_canonical_frames() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes);
        let mut verifier = fixture.start(fixture.plan(), limits());
        fixture.push(&mut verifier).unwrap();
        let complete = verifier.finish().unwrap();
        let first = &complete.rows()[0];
        let decoded = assert_declared_scaling_frame::<AuthenticatedRequest, export::ExportRowV1>(
            first,
            "iroha_kagami::scaling_evidence::AuthenticatedRequestV1",
            [
                31, 184, 227, 1, 217, 145, 213, 68, 167, 99, 231, 153, 179, 32, 17, 73,
            ],
        );
        assert_eq!(decoded.logical_id, first.logical_id);
        assert_eq!(decoded.entrypoint_hash, first.entrypoint_hash);
        assert_eq!(decoded.carrier_hash, first.carrier_hash);
        let rows = complete.rows().to_vec();
        let decoded =
            assert_declared_scaling_frame::<Vec<AuthenticatedRequest>, AuthenticatedRequest>(
                &rows,
                "alloc::vec::Vec<iroha_kagami::scaling_evidence::AuthenticatedRequestV1>",
                [
                    67, 26, 224, 87, 202, 26, 164, 187, 24, 40, 128, 47, 237, 246, 73, 124,
                ],
            );
        assert_eq!(decoded.len(), 8);
        assert_eq!(
            norito::encode_canonical(&decoded).unwrap(),
            complete.canonical_rows()
        );
    }
}

fn native_owner(fixture: &Fixture) -> NativeExecutionEvidenceVerifier {
    NativeExecutionEvidenceVerifier::new(
        fixture.plan().network_id,
        fixture.plan().first_context,
        NativeExecutionEvidenceLimits {
            max_carriers: 1025,
            max_carrier_bytes: MAX_CARRIER_BYTES as u64,
            max_proof_bytes: MAX_FINALITY_BYTES as u64,
            max_retained_bytes: 128 * 1024 * 1024,
        },
    )
    .unwrap()
}
fn native_push(
    owner: &mut NativeExecutionEvidenceVerifier,
    height: &Height,
) -> std::result::Result<VerifiedNativeExecutionCarrier, String> {
    owner.push_height(&height.proof, height.block.clone(), &height.evidence)
}

#[test]
fn canonical_signed_wire_measurement_matches_actual_full_encoding() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes);
        for height in &fixture.heights {
            assert_eq!(
                norito::canonical_frame_len(&height.block)
                    .unwrap()
                    .checked_add(1)
                    .unwrap(),
                height.block.encode_wire().unwrap().len()
            );
            assert_eq!(
                height
                    .proof
                    .finality_artifact
                    .commit_qc
                    .execution_commitment
                    .executed_block_wire_len,
                height.block.encode_wire().unwrap().len() as u64
            );
        }
        let mut changed = fixture.heights[1].block.clone();
        changed
            .add_signature(iroha_data_model::block::BlockSignature::new(
                1,
                iroha_crypto::SignatureOf::try_from_hash(
                    fixture.keys[1].private_key(),
                    changed.hash(),
                )
                .unwrap(),
            ))
            .unwrap();
        assert_eq!(
            norito::canonical_frame_len(&changed).unwrap() + 1,
            changed.encode_wire().unwrap().len()
        );
    }
}

#[test]
fn repeated_admission_retains_the_first_exact_carrier_and_control_position() {
    let fixture = Fixture::new(1);
    let first = &fixture.heights[0];
    let repeat = fixture::successor(
        &fixture.keys,
        first,
        first.block.execution_context().cloned(),
        first.contexts.clone(),
    );
    let mut context = fixture.heights[1]
        .block
        .execution_context()
        .unwrap()
        .clone();
    context
        .native_lane_decisions
        .as_mut()
        .unwrap()
        .base_state_height = 2;
    let execution = fixture::successor(&fixture.keys, &repeat, Some(context), Default::default());
    let mut owner = native_owner(&fixture);
    native_push(&mut owner, first).unwrap();
    native_push(&mut owner, &repeat).unwrap();
    let authenticated = native_push(&mut owner, &execution).unwrap();
    assert_eq!(
        authenticated
            .block()
            .execution_context()
            .unwrap()
            .native_lane_decisions
            .as_ref()
            .unwrap()
            .groups[0]
            .payload
            .descriptor
            .admission_priority
            .carrier_height,
        1
    );
    // Fully re-open and re-sign the later-source claim so the earliest-source
    // rule, rather than stale signatures or malformed manifest shape, rejects it.
    let mut reopened = repeat;
    reopened.contexts.contexts[0].opening_global_height = 2;
    reopened.contexts.contexts[0].opening_global_context_id =
        reopened.proof.finality_artifact.context_id();
    reopened.contexts.contexts[0]
        .admission_priority
        .carrier_height = 2;
    reopened.resign(&fixture.keys);
    let frozen = &reopened.contexts.contexts[0];
    let mut context = execution.block.execution_context().unwrap().clone();
    let group = &mut context.native_lane_decisions.as_mut().unwrap().groups[0];
    group.payload.descriptor.admission_priority.carrier_height = 2;
    group.payload.descriptor.admission_carrier_hash = reopened.block.hash();
    group.payload.descriptor.slots[0].instance_id =
        iroha_core::state::native_lane_instance_for_testing(
            frozen.clone(),
            &reopened.proof.finality_artifact,
        )
        .unwrap();
    fixture::resign_group(
        group,
        frozen,
        &reopened.proof.finality_artifact,
        &fixture.keys,
    );
    group.validate_structure().unwrap();
    let substituted =
        fixture::successor(&fixture.keys, &reopened, Some(context), Default::default());
    let mut owner = native_owner(&fixture);
    native_push(&mut owner, first).unwrap();
    native_push(&mut owner, &reopened).unwrap();
    let error = native_push(&mut owner, &substituted).unwrap_err();
    assert!(error.contains("exact first finalized admission"), "{error}");
}

#[test]
fn retired_context_cannot_reappear_under_its_old_opening_proof() {
    let fixture = Fixture::new(1);
    let first = &fixture.heights[0];
    let retired = fixture::successor(&fixture.keys, first, None, Default::default());
    let resurrected = fixture::successor(&fixture.keys, &retired, None, first.contexts.clone());
    let mut owner = native_owner(&fixture);
    native_push(&mut owner, first).unwrap();
    native_push(&mut owner, &retired).unwrap();
    let error = native_push(&mut owner, &resurrected).unwrap_err();
    assert!(error.contains("resurrects"), "{error}");
    assert!(
        native_push(&mut owner, &fixture.heights[1])
            .unwrap_err()
            .contains("poisoned")
    );
}

#[test]
fn context_root_and_finality_failures_permanently_poison_the_offline_owner() {
    let fixture = Fixture::new(1);
    for change in 0..3 {
        let mut owner = native_owner(&fixture);
        let mut first = fixture.heights[0].clone();
        match change {
            0 => {
                first.evidence = iroha_core::state::native_context_evidence_for_testing(
                    fixture.plan().network_id,
                    1,
                    Default::default(),
                )
                .unwrap()
                .0;
            }
            1 => first.proof.finality_artifact.commit_qc.aggregate_signature[0] ^= 1,
            _ => {
                first.contexts.contexts[0].opening_global_context_id = HeightContextId(
                    HashOf::from_untyped_unchecked(Hash::new(b"different opening context")),
                );
                first.resign(&fixture.keys);
            }
        }
        assert!(
            native_push(&mut owner, &first).is_err(),
            "authority failure {change}"
        );
        assert!(
            native_push(&mut owner, &fixture.heights[0])
                .unwrap_err()
                .contains("poisoned"),
            "no reuse after signature failure or later context join failure"
        );
    }
}

#[test]
fn globally_resigned_foreign_proposal_cannot_substitute_for_exact_executed_wire() {
    let fixture = Fixture::new(1);
    let mut height = fixture.heights[0].clone();
    height.proof.finality_artifact.subject.payload_hash = Hash::new(b"other resultless proposal");
    fixture::resign_claimed_subject(&mut height, &fixture.keys);
    let mut owner = native_owner(&fixture);
    assert!(
        native_push(&mut owner, &height)
            .unwrap_err()
            .contains("exact current Network carrier")
    );
}

#[test]
fn native_interval_admission_bounds_carrier_proof_count_and_retained_bytes() {
    let fixture = Fixture::new(1);
    let first = &fixture.heights[0];
    let body = first.block.encode_wire().unwrap().len() as u64;
    let proof = norito::encode_canonical(&first.proof).unwrap().len() as u64;
    let contexts = first.evidence.len() as u64;
    for change in 0..4 {
        let mut cap = NativeExecutionEvidenceLimits {
            max_carriers: 1025,
            max_carrier_bytes: body,
            max_proof_bytes: proof.max(contexts),
            max_retained_bytes: 128 * 1024 * 1024,
        };
        match change {
            0 => cap.max_carrier_bytes = body - 1,
            1 => cap.max_proof_bytes = proof.max(contexts) - 1,
            2 => cap.max_retained_bytes = body + proof + contexts - 1,
            _ => cap.max_carriers = 1,
        }
        let mut owner = NativeExecutionEvidenceVerifier::new(
            fixture.plan().network_id,
            fixture.plan().first_context,
            cap,
        )
        .unwrap();
        if change == 3 {
            native_push(&mut owner, first).unwrap();
            assert!(native_push(&mut owner, &fixture.heights[1]).is_err());
        } else {
            assert!(
                native_push(&mut owner, first).is_err(),
                "bounded before retention {change}"
            );
        }
        assert!(
            native_push(&mut owner, first)
                .unwrap_err()
                .contains("poisoned")
        );
    }
}

#[test]
fn authentic_pipeline_and_time_outputs_cannot_replace_network_query_owners() {
    use iroha_data_model::{
        block::execution_output::{
            PipelineEventPositionV1, PipelineExecutionOutputV1, PipelineInvocationV1,
            TimeExecutionOutputV1, TimeInvocationV1, TriggerUseV1,
        },
        events::time::{TimeEvent, TimeInterval},
        transaction::signed::{ExecutionStep, TransactionResult},
        trigger::{DataTriggerStep, TriggerId},
    };
    let mut fixture = Fixture::new(4);
    let last = fixture.heights.len() - 1;
    let height = &mut fixture.heights[last];
    let mut outputs = height.block.execution_outputs().to_vec();
    let network_count = outputs.len();
    for time in [false, true] {
        let id: TriggerId = if time {
            "offline_timer"
        } else {
            "offline_pipeline"
        }
        .parse()
        .unwrap();
        let trigger = TriggerUseV1 {
            trigger_id: id.clone(),
            registered_at_height: 1,
            action_hash: Hash::new(if time {
                b"timer action".as_slice()
            } else {
                b"pipeline action".as_slice()
            }),
        };
        let result = TransactionResult::new(Ok(vec![DataTriggerStep {
            id,
            instructions: ExecutionStep(Vec::new().into()),
        }]));
        outputs.push(if time {
            ExecutionOutputV1::Time(TimeExecutionOutputV1 {
                invocation: TimeInvocationV1 {
                    schedule_index: 0,
                    event: TimeEvent {
                        interval: TimeInterval {
                            since_ms: 0,
                            length_ms: 1,
                        },
                    },
                    trigger,
                },
                result,
                failure_root: None,
                completions: Vec::new(),
            })
        } else {
            ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 {
                invocation: PipelineInvocationV1 {
                    event: PipelineEventPositionV1::BlockApproved,
                    candidate_index: 0,
                    trigger,
                },
                result,
                failure_root: None,
                completions: Vec::new(),
            })
        });
    }
    fixture::attach_outputs(&mut height.block, outputs, &fixture.keys);
    height.resign(&fixture.keys);
    let mut baseline = fixture.start(fixture.plan(), limits());
    fixture.push(&mut baseline).unwrap();
    assert_eq!(baseline.finish().unwrap().rows().len(), 8);
    for index in network_count..network_count + 2 {
        let height = &fixture.heights[last];
        let mut queries = height.queries();
        let mut query: CommittedTransaction = canonical(&queries[0]).unwrap();
        query.output = height.block.execution_outputs()[index].clone();
        query.output_hash = HashOf::new(&query.output);
        query.output_proof = height.block.output_proof(index as u32).unwrap();
        assert!(query.output_proof.verify(
            &query.output_hash,
            &height.block.output_merkle_commitment().unwrap()
        ));
        queries[0] = norito::encode_canonical(&query).unwrap();
        let mut verifier = fixture.start(fixture.plan(), limits());
        for earlier in &fixture.heights[1..last] {
            earlier.push(&mut verifier).unwrap();
        }
        assert!(
            push_with(&mut verifier, height, &queries).is_err(),
            "authentic internal output is not a Network receipt"
        );
        assert!(verifier.finish().is_err());
    }
}
