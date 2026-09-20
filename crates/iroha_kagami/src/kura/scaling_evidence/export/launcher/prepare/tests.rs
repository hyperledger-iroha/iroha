//! Canonical preparation controls over real one/four-lane signed transcripts.

use super::*;
use norito::codec::Encode as _;

#[path = "../../../fixture.rs"]
mod fixture;

/// Test-only raw transport assembly for the retained pair owner's signed fixtures.
/// It performs no admission and returns no authority or publication capability.
pub(in crate::kura::scaling_evidence::export) fn encode_facts(
    plan: TrustedRunPlan,
    limits: VerificationLimits,
    heights: Vec<crate::kura::scaling_evidence::export::SuppliedHeightEvidence>,
) -> Vec<u8> {
    let RequestV1 { plan, limits, .. } = RequestV1::from_parts(plan, limits, vec![]).unwrap();
    norito::encode_canonical(&PrepareFactsV1 {
        version: 1,
        plan,
        limits,
        heights: heights
            .into_iter()
            .map(|height| SuppliedEvidenceHeightV1 {
                height: height.height,
                finality: height.finality,
                contexts: height.contexts,
                queries: height.queries,
            })
            .collect(),
    })
    .unwrap()
}

fn facts(f: &fixture::Fixture) -> PrepareFactsV1 {
    let RequestV1 { plan, limits, .. } =
        RequestV1::from_parts(f.plan(), fixture::limits(), vec![]).unwrap();
    PrepareFactsV1 {
        version: 1,
        plan,
        limits,
        heights: f
            .heights
            .iter()
            .enumerate()
            .map(|(index, height)| SuppliedEvidenceHeightV1 {
                height: index as u64 + 1,
                finality: norito::encode_canonical(&height.proof).unwrap(),
                contexts: height.evidence.clone(),
                queries: height.queries(),
            })
            .collect(),
    }
}

fn caps(facts_length: usize) -> PrepareOutputCaps {
    PrepareOutputCaps {
        request_bytes: 1024 * 1024,
        bundle_bytes: 1024 * 1024,
        total_bytes: facts_length as u64 + 2 * 1024 * 1024,
    }
}

fn prepare_frame(bytes: &[u8]) -> Result<PreparedTransports> {
    prepare(
        bytes,
        iroha_crypto::sha256(bytes),
        bytes.len() as u64,
        caps(bytes.len()),
    )
}

fn prepare_facts(facts: &PrepareFactsV1) -> Result<PreparedTransports> {
    prepare_frame(&norito::encode_canonical(facts).unwrap())
}

fn change_query(
    facts: &mut PrepareFactsV1,
    index: usize,
    change: impl FnOnce(&mut CommittedTransaction),
) {
    let raw = facts
        .heights
        .iter_mut()
        .flat_map(|height| &mut height.queries)
        .nth(index)
        .unwrap();
    let mut query: CommittedTransaction = canonical(raw).unwrap();
    change(&mut query);
    *raw = norito::encode_canonical(&query).unwrap();
}

#[test]
fn one_and_four_lane_pair_preserves_every_independent_fact_binding_and_original_byte() {
    for lanes in [1, 4] {
        let f = fixture::Fixture::new(lanes);
        let original = facts(&f);
        assert_eq!(
            encode_facts(
                f.plan(),
                fixture::limits(),
                f.heights
                    .iter()
                    .enumerate()
                    .map(|(index, height)| {
                        crate::kura::scaling_evidence::export::SuppliedHeightEvidence {
                            height: index as u64 + 1,
                            finality: norito::encode_canonical(&height.proof).unwrap(),
                            contexts: height.evidence.clone(),
                            queries: height.queries(),
                        }
                    })
                    .collect()
            ),
            norito::encode_canonical(&original).unwrap()
        );
        let (request, bundle) = prepare_facts(&original).unwrap().into_buffers();
        let request: RequestV1 = canonical(&request).unwrap();
        let bundle: SuppliedEvidenceBundleV1 = canonical(&bundle).unwrap();
        assert_eq!(request.version, 1);
        assert_eq!(request.plan.encode(), original.plan.encode());
        assert_eq!(request.limits.encode(), original.limits.encode());
        assert_eq!(bundle.version, 1);
        assert_eq!(bundle.heights.len(), original.heights.len());
        assert_eq!(request.bindings.len(), original.heights.len());
        for ((got, binding), raw) in bundle
            .heights
            .iter()
            .zip(&request.bindings)
            .zip(&original.heights)
        {
            assert_eq!(got.height, raw.height);
            assert_eq!(got.finality, raw.finality);
            assert_eq!(got.contexts, raw.contexts);
            assert_eq!(binding.contexts_hash, Hash::new(&raw.contexts));
            assert_eq!(got.queries, raw.queries);
            assert_eq!(binding.height, raw.height);
            assert_eq!(binding.finality_hash, Hash::new(&raw.finality));
            assert_eq!(
                binding.query_hashes,
                raw.queries.iter().map(Hash::new).collect::<Vec<_>>()
            );
        }
        let request = norito::encode_canonical(&request).unwrap();
        let (plan, limits, _) = super::super::decode(
            &request,
            iroha_crypto::sha256(&request),
            request.len() as u64,
        )
        .unwrap()
        .into_parts();
        let mut verifier = f.start(plan, limits);
        f.push(&mut verifier).unwrap();
        let complete = verifier.finish().unwrap();
        assert_eq!(complete.rows().len(), original.plan.scheduled.len());
        assert!(
            complete
                .rows()
                .iter()
                .any(|row| row.phase == WorkloadPhase::Warmup)
        );
        assert!(
            complete
                .rows()
                .iter()
                .any(|row| row.phase == WorkloadPhase::Measurement)
        );
    }
}

#[test]
fn facts_declare_a_distinct_v1_frame_with_ambient_layout_independence() {
    let f = fixture::Fixture::new(4);
    let original = facts(&f);
    let decoded = crate::kura::scaling_evidence::tests::assert_declared_scaling_frame::<
        PrepareFactsV1,
        RequestV1,
    >(
        &original,
        "iroha_kagami::scaling_evidence::PrepareFactsV1",
        [
            62, 113, 180, 115, 70, 255, 35, 205, 12, 200, 80, 23, 203, 79, 193, 149,
        ],
    );
    assert_eq!(decoded.version, 1);
    assert!(prepare_facts(&decoded).is_ok());
    let mut bytes = norito::encode_canonical(&original).unwrap();
    bytes[6..22].copy_from_slice(&norito::schema::identity::frame_hash::<RequestV1>());
    assert!(prepare_frame(&bytes).is_err());
}

#[test]
fn version_bare_compressed_truncated_and_trailing_facts_never_fall_back() {
    let f = fixture::Fixture::new(1);
    let original = facts(&f);
    let bytes = norito::encode_canonical(&original).unwrap();
    assert!(prepare_frame(&original.encode()).is_err());
    assert!(prepare_frame(&bytes[..bytes.len() - 1]).is_err());
    let mut trailer = bytes.clone();
    trailer.push(0);
    assert!(prepare_frame(&trailer).is_err());
    let mut compressed = bytes.clone();
    // Compression discriminator follows magic, version and 16-byte schema.
    compressed[22] = 1;
    assert!(prepare_frame(&compressed).is_err());
    for version in [0, 2, u16::MAX] {
        let mut changed = facts(&f);
        changed.version = version;
        assert!(
            prepare_facts(&changed)
                .err()
                .unwrap()
                .to_string()
                .contains("unsupported prepare facts version")
        );
    }
}

#[test]
fn independent_cap_admission_and_raw_digest_precede_malformed_decode() {
    let invalid = b"not a Norito frame";
    let error = prepare(invalid, [0; 32], invalid.len() as u64, caps(invalid.len()))
        .err()
        .unwrap();
    assert!(error.to_string().contains("prepare facts digest mismatch"));
    let mut invalid_caps = caps(invalid.len());
    invalid_caps.request_bytes = 0;
    assert!(
        prepare(invalid, [0; 32], invalid.len() as u64, invalid_caps)
            .err()
            .unwrap()
            .to_string()
            .contains("invalid prepare byte allocations")
    );
    assert!(prepare_frame(&[]).is_err());
    let f = fixture::Fixture::new(1);
    let bytes = norito::encode_canonical(&facts(&f)).unwrap();
    assert!(prepare(&bytes, [0; 32], bytes.len() as u64, caps(bytes.len())).is_err());
}

#[test]
fn exact_input_request_bundle_and_aggregate_caps_accept_and_one_short_rejects() {
    for lanes in [1, 4] {
        let f = fixture::Fixture::new(lanes);
        let bytes = norito::encode_canonical(&facts(&f)).unwrap();
        let (request, bundle) = prepare_frame(&bytes).unwrap().into_buffers();
        let exact = PrepareOutputCaps {
            request_bytes: request.len() as u64,
            bundle_bytes: bundle.len() as u64,
            total_bytes: (bytes.len() + request.len() + bundle.len()) as u64,
        };
        let got = prepare(
            &bytes,
            iroha_crypto::sha256(&bytes),
            bytes.len() as u64,
            exact,
        )
        .unwrap()
        .into_buffers();
        assert_eq!(got, (request, bundle));
        for change in 0..4 {
            let mut bound = exact;
            let mut facts_max = bytes.len() as u64;
            match change {
                0 => facts_max -= 1,
                1 => bound.request_bytes -= 1,
                2 => bound.bundle_bytes -= 1,
                _ => bound.total_bytes -= 1,
            }
            assert!(
                prepare(&bytes, iroha_crypto::sha256(&bytes), facts_max, bound).is_err(),
                "cap {change}"
            );
        }
    }
}

#[test]
fn zero_overflow_and_global_caps_cannot_saturate_into_acceptance() {
    for invalid in [0, MAX_PROOF_BYTES + 1, u64::MAX] {
        for field in 0..4 {
            let mut cap = PrepareOutputCaps {
                request_bytes: 1,
                bundle_bytes: 1,
                total_bytes: 3,
            };
            let mut facts_max = 1;
            match field {
                0 => facts_max = invalid,
                1 => cap.request_bytes = invalid,
                2 => cap.bundle_bytes = invalid,
                _ => cap.total_bytes = invalid,
            }
            assert!(cap.admit(facts_max, 1).is_err());
        }
    }
    assert!(
        PrepareOutputCaps {
            request_bytes: MAX_PROOF_BYTES,
            bundle_bytes: MAX_PROOF_BYTES,
            total_bytes: MAX_PROOF_BYTES
        }
        .admit(MAX_PROOF_BYTES, 1)
        .is_err()
    );
}

#[test]
fn height_interval_rejects_missing_extra_duplicate_reordered_zero_and_overflow_rows() {
    let f = fixture::Fixture::new(4);
    for change in 0..8 {
        let mut value = facts(&f);
        match change {
            0 => {
                value.heights.pop();
            }
            1 => {
                value.heights.push(SuppliedEvidenceHeightV1 {
                    height: 3,
                    finality: vec![1],
                    contexts: vec![1],
                    queries: vec![],
                });
            }
            2 => value.heights[1].height = 1,
            3 => value.heights.swap(0, 1),
            4 => value.plan.first_height = 0,
            5 => value.plan.last_height = u64::MAX,
            6 => value.plan.first_height = 3,
            _ => value.limits.heights = 1,
        }
        assert!(prepare_facts(&value).is_err(), "height mutation {change}");
    }
}

#[test]
fn bounded_inner_preflight_runs_before_decoding_finality_or_query_frames() {
    let f = fixture::Fixture::new(1);
    let mut heights = facts(&f).heights;
    heights[1].queries = vec![vec![1]; fixture::limits().requests + 1];
    let error = derive_bindings(&heights, &f.plan(), fixture::limits())
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("prepare query count exceeds work allocation")
    );
    let mut heights = facts(&f).heights;
    heights[0].finality = vec![0; MAX_FINALITY_BYTES + 1];
    assert!(derive_bindings(&heights, &f.plan(), fixture::limits()).is_err());
    let mut heights = facts(&f).heights;
    heights[0].contexts = vec![0; MAX_FINALITY_BYTES + 1];
    assert!(derive_bindings(&heights, &f.plan(), fixture::limits()).is_err());
    let mut heights = facts(&f).heights;
    heights[1].queries[0] = vec![0; MAX_TRANSACTION_BYTES + 1];
    assert!(derive_bindings(&heights, &f.plan(), fixture::limits()).is_err());
    let mut small = fixture::limits();
    small.input_bytes = 1;
    assert!(derive_bindings(&facts(&f).heights, &f.plan(), small).is_err());
}

#[test]
fn every_work_and_byte_limit_is_reused_from_actual_launcher_admission() {
    let f = fixture::Fixture::new(1);
    for invalid in [0, MAX_REQUESTS as u64 + 1, u64::MAX] {
        for leaves in [false, true] {
            let mut value = facts(&f);
            if leaves {
                value.limits.leaves_per_carrier = invalid;
            } else {
                value.limits.requests = invalid;
            }
            assert!(prepare_facts(&value).is_err());
        }
    }
    for change in 0..5 {
        let mut value = facts(&f);
        match change {
            0 => value.limits.admitted_proof_bytes = 0,
            1 => value.limits.input_bytes = u64::MAX,
            2 => value.limits.output_bytes = 1,
            3 => value.limits.requests = 7,
            _ => value.limits.leaves_per_carrier = 0,
        }
        assert!(prepare_facts(&value).is_err());
    }
}

#[test]
fn canonical_inner_schema_version_and_original_digest_bindings_are_not_summary_counts() {
    let f = fixture::Fixture::new(1);
    for change in 0..6 {
        let mut value = facts(&f);
        match change {
            0 => value.heights[0].finality = f.heights[0].proof.encode(),
            1 => value.heights[1].queries[0].push(0),
            2 => {
                value.heights[1].queries[0] = norito::encode_canonical(&f.heights[1].proof).unwrap()
            }
            3 => value.heights[0].finality = value.heights[1].queries[0].clone(),
            4 => {
                let mut proof = f.heights[0].proof.clone();
                proof.version = 0;
                value.heights[0].finality = norito::encode_canonical(&proof).unwrap();
            }
            _ => value.heights[1].finality = norito::encode_canonical(&f.heights[0].proof).unwrap(),
        }
        assert!(
            prepare_facts(&value).is_err(),
            "inner frame mutation {change}"
        );
    }
}

#[test]
fn independent_network_context_route_and_original_signed_bytes_are_required() {
    for lanes in [1, 4] {
        let f = fixture::Fixture::new(lanes);
        for change in 0..4 {
            let mut value = facts(&f);
            match change {
                0 => {
                    value.plan.network_id = NetworkId::from_genesis_hash(
                        HashOf::from_untyped_unchecked(Hash::new(b"different launch network")),
                    )
                }
                1 => {
                    value.plan.first_context = HeightContextId(HashOf::from_untyped_unchecked(
                        Hash::new(b"different launch anchor"),
                    ))
                }
                2 => value.plan.scheduled[0].route.lane_id = LaneId::new(900),
                _ => {
                    let bytes = &mut value.plan.scheduled[0].signed_transaction;
                    let last = bytes.len() - 1;
                    bytes[last] ^= 1;
                }
            }
            assert!(prepare_facts(&value).is_err());
        }
    }
}

#[test]
fn missing_duplicate_extra_and_reordered_queries_never_create_a_complete_pair() {
    for lanes in [1, 4] {
        let f = fixture::Fixture::new(lanes);
        for change in 0..4 {
            let mut value = facts(&f);
            match change {
                0 => {
                    value.heights[1].queries.pop();
                }
                1 => value.heights[2].queries[0] = value.heights[1].queries[0].clone(),
                2 => {
                    let extra = value.heights[1].queries[0].clone();
                    value.heights[1].queries.push(extra);
                }
                _ => {
                    let first = value.heights[1].queries[0].clone();
                    value.heights[1].queries[0] = value.heights[2].queries[0].clone();
                    value.heights[2].queries[0] = first;
                }
            }
            assert!(prepare_facts(&value).is_err());
        }
    }
}

#[test]
fn coherent_counts_still_reject_missing_or_repeated_scheduled_identity() {
    for lanes in [1, 4] {
        let f = fixture::Fixture::new(lanes);
        let mut missing = facts(&f);
        missing.heights[1].queries.pop();
        assert!(
            prepare_facts(&missing)
                .err()
                .unwrap()
                .to_string()
                .contains("prepare queries omit scheduled requests")
        );
        let mut repeated = facts(&f);
        let original: CommittedTransaction = canonical(&repeated.heights[1].queries[0]).unwrap();
        change_query(&mut repeated, 1, |query| {
            query.entrypoint = original.entrypoint;
            query.entrypoint_hash = original.entrypoint_hash;
        });
        assert!(
            prepare_facts(&repeated)
                .err()
                .unwrap()
                .to_string()
                .contains("prepare request appears more than once")
        );
    }
}

#[test]
fn query_carrier_self_hash_and_input_output_order_are_checked_through_prepare() {
    let f = fixture::Fixture::new(4);
    for change in 0..5 {
        let mut value = facts(&f);
        change_query(&mut value, 0, |query| match change {
            0 => query.block_hash = f.heights[0].block.hash(),
            1 => {
                query.entrypoint_hash =
                    HashOf::from_untyped_unchecked(Hash::new(b"changed request"))
            }
            2 => query.output_hash = HashOf::from_untyped_unchecked(Hash::new(b"changed output")),
            3 => {
                query.entrypoint_proof =
                    canonical::<CommittedTransaction>(&f.heights[1].queries()[1])
                        .unwrap()
                        .entrypoint_proof
            }
            _ => {
                let iroha_data_model::block::execution_output::ExecutionOutputV1::Network(output) =
                    &mut query.output
                else {
                    unreachable!()
                };
                output.input_index += 1;
                query.output_hash = HashOf::new(&query.output);
            }
        });
        assert!(prepare_facts(&value).is_err(), "query mutation {change}");
    }
}

#[test]
fn rejected_request_is_not_removed_or_promoted_into_successful_transport() {
    let f = fixture::Fixture::new(1);
    let mut value = facts(&f);
    change_query(&mut value, 0, |query| {
        let iroha_data_model::block::execution_output::ExecutionOutputV1::Network(output) =
            &mut query.output
        else {
            unreachable!()
        };
        output.result.0 = Err(
            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::NotPermitted("rejected fixture".into()),
            ),
        );
        query.output_hash = HashOf::new(&query.output);
    });
    assert!(
        prepare_facts(&value)
            .err()
            .unwrap()
            .to_string()
            .contains("prepare signed request failed or changed")
    );
}

#[test]
fn omitted_warmup_measurement_and_unscheduled_requests_fail_even_with_valid_query_counts() {
    for lanes in [1, 4] {
        let f = fixture::Fixture::new(lanes);
        for phase in [WorkloadPhase::Warmup, WorkloadPhase::Measurement] {
            let mut value = facts(&f);
            let position = value
                .plan
                .scheduled
                .iter()
                .position(|r| r.phase == phase)
                .unwrap();
            value.plan.scheduled.remove(position);
            assert!(prepare_facts(&value).is_err());
        }
        let mut value = facts(&f);
        value.plan.scheduled.last_mut().unwrap().phase = WorkloadPhase::Warmup;
        assert!(prepare_facts(&value).is_err());
        let mut value = facts(&f);
        value.plan.scheduled[1].logical_id = value.plan.scheduled[0].logical_id.clone();
        assert!(prepare_facts(&value).is_err());
    }
}

#[test]
fn unaligned_facts_preserve_the_exact_two_canonical_outputs() {
    let f = fixture::Fixture::new(1);
    let bytes = norito::encode_canonical(&facts(&f)).unwrap();
    let expected = prepare_frame(&bytes).unwrap().into_buffers();
    let mut unaligned = 0;
    for offset in 0..16 {
        let mut storage = vec![0; offset + bytes.len()];
        storage[offset..].copy_from_slice(&bytes);
        let raw = &storage[offset..];
        if raw.as_ptr() as usize % std::mem::align_of::<u64>() != 0 {
            unaligned += 1;
        }
        assert_eq!(prepare_frame(raw).unwrap().into_buffers(), expected);
    }
    assert!(unaligned > 0);
}

#[test]
fn preparation_does_not_authenticate_a_claimed_inclusion_proof() {
    let f = fixture::Fixture::new(4);
    let mut value = facts(&f);
    // Claim a different input proof with the same leaf index and self-consistent
    // query bytes. Only the complete carrier can authenticate its sibling path.
    change_query(&mut value, 0, |query| {
        let other: CommittedTransaction = canonical(&f.heights[2].queries()[0]).unwrap();
        query.entrypoint_proof = other.entrypoint_proof;
    });
    let (request, bundle) = prepare_facts(&value).unwrap().into_buffers();
    let (plan, limits, _) = super::super::decode(
        &request,
        iroha_crypto::sha256(&request),
        request.len() as u64,
    )
    .unwrap()
    .into_parts();
    let bundle: SuppliedEvidenceBundleV1 = canonical(&bundle).unwrap();
    let mut verifier = f.start(plan, limits);
    let row = &bundle.heights[1];
    let result = verifier.push_height(
        &row.finality,
        &f.heights[1].block.encode_wire().unwrap(),
        &row.contexts,
        &row.queries.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "queried leaf is not this exact typed Network output"
    );
    assert!(verifier.finish().is_err());
}
