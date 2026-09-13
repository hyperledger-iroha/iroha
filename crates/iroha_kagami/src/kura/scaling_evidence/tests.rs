//! Signed authentication, alignment, budget and complete-cohort adverse controls.

use super::*;
use fixture::{Fixture, limits};
use norito::codec::Encode as _;

#[test]
fn one_and_four_lane_signed_runs_authenticate_every_warmup_and_measurement_effect() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes);
        let mut verifier = fixture.start(fixture.plan(), limits());
        fixture.push(&mut verifier).unwrap();
        let complete = verifier.finish().unwrap();
        assert_eq!(complete.rows().len(), 8);
        assert_eq!(
            complete
                .rows()
                .iter()
                .filter(|r| r.phase == WorkloadPhase::Warmup)
                .count(),
            4
        );
        assert_eq!(
            complete
                .rows()
                .iter()
                .filter(|r| r.phase == WorkloadPhase::Measurement)
                .count(),
            4
        );
        for (row, (logical, tx, route, phase)) in complete.rows().iter().zip(&fixture.requests) {
            assert_eq!(&row.logical_id, logical);
            assert_eq!(row.entrypoint_hash, tx.hash_as_entrypoint());
            assert_eq!(&row.authority, tx.authority());
            assert_eq!(row.phase, *phase);
            assert_eq!(
                (row.lane_id, row.dataspace_id),
                (route.lane_id, route.dataspace_id)
            );
            assert_eq!(row.carrier_height, 2);
            assert_eq!(row.carrier_hash, fixture.carrier.hash());
            assert_eq!(row.merge_entry_hash, fixture.entry.canonical_hash());
            assert!(norito::encode_canonical(row).unwrap().len() as u64 <= ROW_RESERVATION);
        }
        for binding in &fixture.entry.active_lanes {
            assert_eq!(
                complete
                    .rows()
                    .iter()
                    .filter(|r| r.lane_id == binding.lane_id)
                    .count(),
                8 / lanes
            );
        }
        let decoded: Vec<AuthenticatedRequest> = canonical(complete.canonical_rows()).unwrap();
        assert_eq!(decoded.len(), 8);
        assert!(complete.input_bytes() > fixture.carrier.encode_wire().unwrap().len() as u64);
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
                Hash::new(b"wrong independent network"),
            ));
        } else {
            plan.first_context = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"wrong independent context",
            )));
        }
        match ScalingProofVerifier::new(plan, limits()) {
            Err(_) => assert_eq!(
                change, 0,
                "wrong network may fail at signed request validation"
            ),
            Ok(mut verifier) => {
                assert!(
                    verifier
                        .push_height(
                            &norito::encode_canonical(&fixture.first).unwrap(),
                            &fixture.genesis.encode_wire().unwrap(),
                            None,
                            &[]
                        )
                        .is_err()
                );
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
    let prefix = fixture.start(fixture.plan(), limits());
    assert!(prefix.finish().is_err());
    let mut duplicate = fixture.start(fixture.plan(), limits());
    assert!(
        duplicate
            .push_height(
                &norito::encode_canonical(&fixture.first).unwrap(),
                &fixture.genesis.encode_wire().unwrap(),
                None,
                &[]
            )
            .is_err()
    );
    assert!(duplicate.finish().is_err());
    let mut unsigned = fixture.second.clone();
    unsigned
        .finality_artifact
        .commit_qc
        .aggregate_signature
        .clear();
    let mut verifier = fixture.start(fixture.plan(), limits());
    assert!(
        verifier
            .push_height(
                &norito::encode_canonical(&unsigned).unwrap(),
                &fixture.carrier.encode_wire().unwrap(),
                Some(&fixture.entry.canonical_bytes()),
                &fixture
                    .queries()
                    .iter()
                    .map(Vec::as_slice)
                    .collect::<Vec<_>>()
            )
            .is_err()
    );
    assert!(
        fixture.push(&mut verifier).is_err(),
        "a caught invalid signature permanently poisons the owner"
    );
    assert!(verifier.finish().is_err());
}

#[test]
fn canonical_decoders_reject_headerless_trailing_and_oversized_inputs() {
    let fixture = Fixture::new(1);
    let mut baseline = fixture.start(fixture.plan(), limits());
    fixture.push(&mut baseline).unwrap();
    assert_eq!(baseline.finish().unwrap().rows().len(), 8);
    let finality = norito::encode_canonical(&fixture.first).unwrap();
    let block = fixture.genesis.encode_wire().unwrap();
    for invalid in [
        fixture.first.encode(),
        [finality.clone(), vec![0]].concat(),
        vec![0; MAX_FINALITY_BYTES + 1],
    ] {
        let mut verifier = ScalingProofVerifier::new(fixture.plan(), limits()).unwrap();
        assert!(verifier.push_height(&invalid, &block, None, &[]).is_err());
        assert!(verifier.finish().is_err());
    }
    let mut carrier = block.clone();
    carrier.push(0);
    let mut verifier = ScalingProofVerifier::new(fixture.plan(), limits()).unwrap();
    assert!(
        verifier
            .push_height(&finality, &carrier, None, &[])
            .is_err()
    );
    let mut verifier = fixture.start(fixture.plan(), limits());
    assert!(
        verifier
            .push_height(
                &norito::encode_canonical(&fixture.second).unwrap(),
                &fixture.carrier.encode_wire().unwrap(),
                Some(&fixture.entry.encode()),
                &fixture
                    .queries()
                    .iter()
                    .map(Vec::as_slice)
                    .collect::<Vec<_>>()
            )
            .is_err()
    );
}

#[test]
fn same_header_does_not_authenticate_substituted_result_bearing_wire() {
    let mut fixture = Fixture::new(1);
    let mut verifier = fixture.start(fixture.plan(), limits());
    // Block signatures are part of the canonical executed wire but not header hash.
    let old_header = fixture.carrier.header();
    let changed = iroha_data_model::block::BlockSignature::new(
        1,
        iroha_crypto::SignatureOf::try_from_hash(fixture.keys[1].private_key(), old_header.hash())
            .unwrap(),
    );
    fixture.carrier.add_signature(changed).unwrap();
    assert_eq!(fixture.carrier.header(), old_header);
    assert!(fixture.push(&mut verifier).is_err());
    assert!(verifier.finish().is_err());
}

#[test]
fn complete_full_entry_reference_and_commitqc_merge_identity_are_both_required() {
    for change in 0..9 {
        let mut fixture = Fixture::new(1);
        let mut baseline = fixture.start(fixture.plan(), limits());
        fixture.push(&mut baseline).unwrap();
        assert_eq!(baseline.finish().unwrap().rows().len(), 8);
        let mut verifier = fixture.start(fixture.plan(), limits());
        if change == 0 {
            // Authenticate the changed commitment with actual three-of-four
            // signatures so the exact merge binding, not a stale signature, rejects it.
            fixture.resign_carrier_with_merge(None);
            assert!(fixture.push(&mut verifier).is_err());
        } else if change == 1 {
            assert!(
                verifier
                    .push_height(
                        &norito::encode_canonical(&fixture.second).unwrap(),
                        &fixture.carrier.encode_wire().unwrap(),
                        None,
                        &fixture
                            .queries()
                            .iter()
                            .map(Vec::as_slice)
                            .collect::<Vec<_>>()
                    )
                    .is_err()
            );
        } else {
            let mut context = fixture.carrier.execution_context().unwrap().clone();
            let reference = context.merge_entry.as_mut().unwrap();
            match change {
                2 => reference.encoded_len += 1,
                3 => reference.epoch_id += 1,
                4 => {
                    reference.entry_hash =
                        HashOf::from_untyped_unchecked(Hash::new(b"wrong entry hash"))
                }
                5 => reference.execution_batch_hash = None,
                6 => reference.base_state_height = None,
                7 => reference.merge_qc.aggregate_signature[0] ^= 1,
                _ => reference.entrypoint_count = Some(9),
            }
            fixture.carrier.set_execution_context(Some(context));
            fixture.resign_carrier();
            assert!(
                fixture.push(&mut verifier).is_err(),
                "fully globally re-signed mismatched reference {change}"
            );
        }
        assert!(verifier.finish().is_err());
    }
}

#[test]
fn rehashed_and_resigned_entry_cannot_change_independent_route_or_authority() {
    for change in 0..8 {
        let mut fixture = Fixture::new(4);
        let mut verifier = fixture.start(fixture.plan(), limits());
        match change {
            0 => fixture.entry.lane_catalog_hash = Hash::new(b"other catalog"),
            1 => fixture.entry.active_lanes[0].dataspace_id = DataSpaceId::new(99),
            2 => fixture.entry.active_lanes[0].incarnation = Hash::new(b"other incarnation"),
            3 => fixture.entry.active_lanes[0].activation_height = 2,
            4 => fixture.entry.lane_authority_catalog.lane_roster_indices[0] = 1,
            5 => fixture.entry.merge_qc.carrier_height = 3,
            6 => {
                fixture.entry.merge_qc.carrier_parent_hash =
                    HashOf::from_untyped_unchecked(Hash::new(b"other parent"))
            }
            _ => fixture.entry.version = MergeLedgerEntry::VERSION - 1,
        }
        fixture.rebuild_carrier();
        assert!(
            fixture.push(&mut verifier).is_err(),
            "outer signatures cannot replace trusted route facts {change}"
        );
        assert!(verifier.finish().is_err());
    }
}

#[test]
fn rehashed_signed_batch_still_requires_every_aligned_transcript_vector() {
    for change in 0..10 {
        let mut fixture = Fixture::new(1);
        let mut verifier = fixture.start(fixture.plan(), limits());
        let batch = fixture.entry.execution_batch.as_mut().unwrap();
        let lane = &mut batch.lanes[0];
        match change {
            0 => {
                lane.entrypoint_hashes.pop();
            }
            1 => {
                lane.result_hashes.pop();
            }
            2 => {
                lane.routing_plans.pop();
            }
            3 => {
                lane.reservation_keys.pop();
            }
            4 => {
                lane.native_amx_receipts.pop();
            }
            5 => {
                lane.authenticated_signed_replay_aliases.pop();
            }
            6 => lane.authenticated_signed_replay_aliases[0] = Some(Hash::new(b"sealed alias")),
            7 => {
                lane.routing_plans[0] = norito::encode_canonical(&RoutingPlan::single(
                    RoutingDecision::new(LaneId::new(99), DataSpaceId::new(99)),
                ))
                .unwrap()
            }
            8 => {
                let mut key: LaneQueueReservationKeyV1 =
                    canonical(&lane.reservation_keys[0]).unwrap();
                key.entrypoint_hash =
                    HashOf::from_untyped_unchecked(Hash::new(b"another reservation"));
                lane.reservation_keys[0] = norito::encode_canonical(&key).unwrap();
            }
            _ => {
                lane.commit_qc.payload_availability_qc =
                    lane.prepare_qc.payload_availability_qc.clone()
            }
        }
        fixture::rehash_batch(batch);
        fixture.rebuild_carrier();
        assert!(
            fixture.push(&mut verifier).is_err(),
            "aligned-vector control {change}"
        );
        assert!(verifier.finish().is_err());
    }
}

#[test]
fn compact_query_proofs_must_match_the_full_merge_leaf_and_same_index() {
    for change in 0..7 {
        let fixture = Fixture::new(4);
        let mut verifier = fixture.start(fixture.plan(), limits());
        let mut queries = fixture.queries();
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
                3 => q.merge_inclusion = None,
                4 => q.result_proof = other.result_proof,
                5 => q.entrypoint = other.entrypoint,
                _ => q.merge_inclusion.as_mut().unwrap().entrypoint_count += 1,
            }
            queries[0] = norito::encode_canonical(&q).unwrap();
        }
        assert!(
            verifier
                .push_height(
                    &norito::encode_canonical(&fixture.second).unwrap(),
                    &fixture.carrier.encode_wire().unwrap(),
                    Some(&fixture.entry.canonical_bytes()),
                    &queries.iter().map(Vec::as_slice).collect::<Vec<_>>()
                )
                .is_err(),
            "query control {change}"
        );
        assert!(verifier.finish().is_err());
    }
}

#[test]
fn ordinary_inclusion_never_substitutes_for_scheduled_lane_execution() {
    let mut fixture = Fixture::new(1);
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(2).unwrap(),
        Some(fixture.genesis.hash()),
        None,
        None,
        100,
        0,
    );
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
    builder.push_transaction(fixture.requests[0].1.clone());
    builder.push_result(Ok(iroha_data_model::trigger::DataTriggerSequence::default()));
    fixture.carrier = builder.build_with_signature(0, fixture.keys[0].private_key());
    fixture.resign_carrier_with_merge(None);
    assert!(fixture.carrier.execution_context().is_none());
    assert!(
        fixture
            .second
            .finality_artifact
            .commit_qc
            .execution_commitment
            .merge_carrier
            .is_none()
    );
    // The exact ordinary carrier is structurally authentic when this transaction
    // is outside the requested cohort. Other missing rows still prevent finish.
    let mut unrelated = fixture.plan();
    unrelated.scheduled.remove(0);
    let mut baseline = fixture.start(unrelated, limits());
    baseline
        .push_height(
            &norito::encode_canonical(&fixture.second).unwrap(),
            &fixture.carrier.encode_wire().unwrap(),
            None,
            &[],
        )
        .unwrap();
    assert!(baseline.finish().is_err());
    let mut verifier = fixture.start(fixture.plan(), limits());
    let rejected = verifier.push_height(
        &norito::encode_canonical(&fixture.second).unwrap(),
        &fixture.carrier.encode_wire().unwrap(),
        None,
        &[],
    );
    assert!(rejected.is_err());
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
    assert!(
        fixture.push(&mut verifier).is_err(),
        "full entry contains an extra unscheduled transaction"
    );
    assert!(verifier.finish().is_err());
    let mut extended = fixture.plan();
    extended.last_height = 3;
    let mut verifier = fixture.start(extended, limits());
    fixture.push(&mut verifier).unwrap();
    assert!(
        verifier.finish().is_err(),
        "all rows do not erase an incomplete finality interval"
    );
    let mut reordered = Fixture::new(4);
    let mut verifier = reordered.start(reordered.plan(), limits());
    let batch = reordered.entry.execution_batch.as_mut().unwrap();
    batch.lanes.swap(0, 1);
    fixture::rehash_batch(batch);
    reordered.rebuild_carrier();
    assert!(
        reordered.push(&mut verifier).is_err(),
        "globally signed reordered lane transcript is not canonical"
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
    let batch = fixture.entry.execution_batch.as_mut().unwrap();
    let lane = &mut batch.lanes[0];
    lane.results[0].0 = Err(
        iroha_data_model::transaction::error::TransactionRejectionReason::IvmExecution(
            iroha_data_model::transaction::error::IvmExecutionFail {
                reason: "fixture rejection".to_owned(),
            },
        ),
    );
    lane.result_hashes[0] = lane.results[0].hash().into();
    fixture::rehash_batch(batch);
    fixture.rebuild_carrier();
    assert!(
        fixture.push(&mut verifier).is_err(),
        "even a globally authenticated rejection fails the complete useful cohort"
    );
    assert!(verifier.finish().is_err());
}

#[test]
fn exact_three_of_four_global_signatures_pops_and_parent_are_mandatory() {
    for change in 0..5 {
        let fixture = Fixture::new(1);
        let mut baseline = fixture.start(fixture.plan(), limits());
        fixture.push(&mut baseline).unwrap();
        assert_eq!(baseline.finish().unwrap().rows().len(), 8);
        let mut verifier = fixture.start(fixture.plan(), limits());
        let mut proof = fixture.second.clone();
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
            verifier
                .push_height(
                    &norito::encode_canonical(&proof).unwrap(),
                    &fixture.carrier.encode_wire().unwrap(),
                    Some(&fixture.entry.canonical_bytes()),
                    &fixture
                        .queries()
                        .iter()
                        .map(Vec::as_slice)
                        .collect::<Vec<_>>()
                )
                .is_err(),
            "exact finality authority control {change}"
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
                b"{\"verified\":true,\"lanes\":[0]}",
                &fixture.carrier.encode_wire().unwrap(),
                Some(&fixture.entry.canonical_bytes()),
                &fixture
                    .queries()
                    .iter()
                    .map(Vec::as_slice)
                    .collect::<Vec<_>>()
            )
            .is_err()
    );
    assert!(verifier.finish().is_err());
    let mut no_work = Fixture::new(1);
    let mut verifier = no_work.start(no_work.plan(), limits());
    no_work.entry.execution_batch = None;
    no_work.rebuild_carrier();
    verifier
        .push_height(
            &norito::encode_canonical(&no_work.second).unwrap(),
            &no_work.carrier.encode_wire().unwrap(),
            Some(&no_work.entry.canonical_bytes()),
            &[],
        )
        .unwrap();
    assert!(
        verifier.finish().is_err(),
        "a certified snapshot without actual scheduled work is insufficient"
    );
    // Observed Account query bytes remain solely with the separate postcondition
    // owner; neither the API nor AuthenticatedRun declares state membership.
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
