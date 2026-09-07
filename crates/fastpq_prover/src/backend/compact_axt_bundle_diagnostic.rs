//! Explicit two-delta AXT bundle diagnostic with real whole-batch SMT paths.
//!
//! Private paths, witnesses, traces, proof DTOs and prover relations leave scope
//! before raw typed verification. Only deterministic public proof bytes persist.
//! This ignored diagnostic does not qualify production security or admission.

use super::{
    compact_axt_batch::AxtTransferBatch,
    compact_axt_context::tests::Fixture,
    compact_bundle::{
        AxtBundleWire, BundleLimits, BundleWire, encode_axt_wire, encode_wire,
        verify_axt_transfer_bundle, verify_transfer_bundle,
    },
    compact_protocol::{self, FixedAir, shared_openings},
    compact_public_api::AxtVerificationContext,
    compact_public_batch::{BatchContextLimits, PublicTransferBatch},
};
use crate::{
    Error, ProofSemantics, VerifyLimits,
    gadgets::{
        compact_smt_air::{COLUMN_COUNT, PATH_LEVELS, PHYSICAL_ROW_COUNT, SmtWitness},
        compact_trace_columns::smt_row_cells,
        public_transfer_statement::{
            PreparedPublicTransfers, PublicTransferLimits, prepare_public_transfers,
            public_claims_from_transcripts,
        },
        transfer::attach_transfer_smt_witnesses,
    },
    proof::PublicIO,
};
use iroha_data_model::{
    fastpq::{TransferDeltaTranscript, TransferSmtWitness, TransferTranscript},
    nexus::compute_remote_spend_claim_commitment_v1,
};
use sha2::{Digest as _, Sha256};

fn context(fixture: &Fixture) -> AxtVerificationContext<'_> {
    AxtVerificationContext {
        binding: &fixture.binding,
        metadata: fixture.metadata(),
        mirrors: fixture.outer,
        remote_spend_claims: fixture.remote.as_deref(),
    }
}

#[test]
#[ignore = "explicit two complete 65536x342 AXT proofs with ordered raw bundle verification"]
fn complete_two_delta_axt_bundle_verifies_after_private_witnesses_are_dropped() {
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let limits = BundleLimits {
        max_segments: 2,
        max_wire_bytes: 8 * 1024 * 1024,
        max_total_segment_bytes: 8 * 1024 * 1024,
        max_total_statement_bytes: 512 * 1024,
        max_total_queries: 272,
        max_total_decode_allocation_charges: 128 * 1024 * 1024,
        segment: VerifyLimits {
            max_proof_bytes: 4 * 1024 * 1024,
            ..VerifyLimits::default()
        },
    };
    let construction_started = std::time::Instant::now();
    let (
        fixture,
        rows,
        claims,
        inputs,
        expected,
        encoded,
        proving_seconds,
        conversion_seconds,
        typed_bytes,
        segment_bytes,
        construction_seconds,
    ) = {
        let mut fixture = Fixture::multiple(2, true);
        let mut transcripts = {
            let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
            assert_eq!(prepared.pairs().len(), 2);
            assert_eq!(prepared.claims().len(), 2);
            let remote = fixture.remote.as_ref().unwrap();
            assert_eq!(remote.len(), 2);
            assert_ne!(remote[0].handle_replay_key, remote[1].handle_replay_key);
            assert_eq!(remote[0].effective_amount, remote[1].effective_amount);
            assert_eq!(remote[0].from, remote[1].from);
            assert_eq!(remote[0].to, remote[1].to);
            prepared
                .claims()
                .iter()
                .map(|claim| TransferTranscript {
                    batch_hash: claim.batch_hash,
                    authority_digest: claim.authority_digest,
                    poseidon_preimage_digest: claim.poseidon_preimage_digest,
                    deltas: claim
                        .deltas
                        .iter()
                        .map(|delta| TransferDeltaTranscript {
                            from_account: delta.from_account.clone(),
                            to_account: delta.to_account.clone(),
                            asset_definition: delta.asset_definition.clone(),
                            amount: delta.amount.clone(),
                            from_balance_before: delta.from_balance_before.clone(),
                            from_balance_after: delta.from_balance_after.clone(),
                            to_balance_before: delta.to_balance_before.clone(),
                            to_balance_after: delta.to_balance_after.clone(),
                            from_smt_witness: TransferSmtWitness::default(),
                            to_smt_witness: TransferSmtWitness::default(),
                        })
                        .collect(),
                })
                .collect::<Vec<_>>()
        };
        // Exactly one attachment over the entire chronological batch establishes
        // the shared touched-tree context; isolated delta attachment is invalid.
        let (old_root, new_root) = attach_transfer_smt_witnesses(&mut transcripts).unwrap();
        let middle = transcripts[0].deltas[0].to_smt_witness.root_after;
        assert_eq!(
            middle,
            transcripts[1].deltas[0].from_smt_witness.root_before
        );
        assert_ne!(old_root, middle);
        assert_ne!(middle, new_root);
        fixture.set_touched_roots(old_root, new_root);
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        assert_eq!(
            public_claims_from_transcripts(&transcripts, PublicTransferLimits::default())
                .unwrap()
                .as_slice(),
            prepared.claims()
        );
        let expected = fixture.expected(&prepared);
        let inputs = *prepared.public_inputs();
        let rows = prepared.transitions().to_vec();
        let claims = prepared.claims().to_vec();
        let batch = AxtTransferBatch::new(
            &prepared,
            &expected,
            &[middle],
            context(&fixture),
            BatchContextLimits {
                max_segments: 2,
                max_total_statement_bytes: limits.max_total_statement_bytes,
            },
        )
        .unwrap();
        assert_eq!(batch.segment_count(), 2);
        assert_eq!(batch.public_io(), expected);
        let mut frames = Vec::new();
        let mut proving = [0.0; 2];
        let mut conversion = [0.0; 2];
        let mut typed_bytes = [0; 2];
        let mut segment_bytes = [0; 2];
        let mut construction = [0.0; 2];
        for (ordinal, delta) in transcripts
            .iter()
            .flat_map(|entry| &entry.deltas)
            .enumerate()
        {
            let construction_started = std::time::Instant::now();
            let columns = {
                let statement = &batch.statements()[ordinal];
                let paths = [&delta.from_smt_witness, &delta.to_smt_witness];
                for (path, update) in paths.iter().zip(statement.updates) {
                    assert_eq!(path.path_bits.as_slice(), update.path.to_le_bytes());
                    assert_eq!(path.siblings.len(), PATH_LEVELS);
                }
                let siblings = core::array::from_fn(|update| {
                    core::array::from_fn(|level| {
                        let bytes = paths[update].siblings[level];
                        core::array::from_fn(|limb| {
                            u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
                        })
                    })
                });
                let witness = SmtWitness::from_inputs(statement, &siblings)
                    .unwrap()
                    .into_physical();
                let mut columns = (0..COLUMN_COUNT)
                    .map(|_| Vec::with_capacity(PHYSICAL_ROW_COUNT))
                    .collect::<Vec<_>>();
                for row in witness.rows() {
                    for (column, value) in columns.iter_mut().zip(smt_row_cells(row)) {
                        column.push(value);
                    }
                }
                columns
            };
            construction[ordinal] = construction_started.elapsed().as_secs_f64();
            let relation = batch.segment(ordinal).unwrap();
            assert_eq!(
                relation.schema().identity,
                "fastpq:prototype:axt-transfer-bundle-segment:v1:342cols:923slots:65536rows"
            );
            let started = std::time::Instant::now();
            let proof = compact_protocol::prove(&relation, &columns).unwrap();
            proving[ordinal] = started.elapsed().as_secs_f64();
            drop(columns);
            typed_bytes[ordinal] = norito::core::encoded_frame_len(&proof).unwrap();
            let started = std::time::Instant::now();
            let shared = shared_openings::from_compact(&relation, &proof, limits.segment).unwrap();
            let frame = norito::encode_canonical(&shared).unwrap();
            segment_bytes[ordinal] = frame.len();
            assert!(frame.len() < typed_bytes[ordinal]);
            assert!(frame.len() > VerifyLimits::default().max_proof_bytes);
            frames.push(frame);
            conversion[ordinal] = started.elapsed().as_secs_f64();
        }
        let encoded = encode_axt_wire(
            &AxtBundleWire {
                version: 1,
                intermediate_roots: vec![middle],
                segments: frames,
            },
            2,
            limits,
        )
        .unwrap();
        drop(batch);
        drop(prepared);
        // All private transcripts, paths, columns and proof DTOs leave scope.
        (
            fixture,
            rows,
            claims,
            inputs,
            expected,
            encoded,
            proving,
            conversion,
            typed_bytes,
            segment_bytes,
            construction,
        )
    };
    let construction_and_proving = construction_started.elapsed();
    let prepared = prepare_public_transfers(
        &rows,
        &claims,
        inputs,
        ProofSemantics::AxtTransferClaim,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let verify = |prepared: &PreparedPublicTransfers<'_>,
                  expected: &PublicIO,
                  bytes: &[u8],
                  limits: BundleLimits| {
        verify_axt_transfer_bundle(prepared, expected, context(&fixture), bytes, limits)
    };
    let started = std::time::Instant::now();
    let (verified, usage) = {
        let flags = norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        let result = norito::core::with_decode_limits_measured(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 16),
            || verify(&prepared, &expected, &encoded, limits),
        );
        assert_eq!(norito::core::get_decode_flags(), flags);
        result
    };
    let verified = verified.unwrap();
    let verifying = started.elapsed();
    assert_eq!(verified.public_io(), expected);
    assert_eq!(verified.segments(), 2);
    assert_eq!(verified.wire_bytes(), encoded.len());
    assert_eq!(verified.work().transcripts, 2);
    assert_eq!(verified.work().air_evaluations, 272);
    assert_eq!(verified.work().terminal_degree_checks, 2);
    assert_eq!(
        verified.work().proof_bytes,
        segment_bytes.iter().sum::<usize>()
    );
    assert_eq!(verified.work().oracle_leaves, 544);
    assert!((272..=544).contains(&verified.work().row_leaves));
    assert!(verified.work().proof_bytes < encoded.len());
    assert!(usage.total_allocated_bytes() > 0);
    // The cumulative charge ceiling is inclusive across the outer frame and
    // both child decodes; neither child may reset the parent scope.
    assert_eq!(
        verify(
            &prepared,
            &expected,
            &encoded,
            BundleLimits {
                max_total_decode_allocation_charges: usage.total_allocated_bytes(),
                max_total_queries: verified.work().air_evaluations,
                max_total_statement_bytes: verified.statement_bytes(),
                ..limits
            },
        )
        .unwrap(),
        verified
    );
    assert!(matches!(
        verify(
            &prepared,
            &expected,
            &encoded,
            BundleLimits {
                max_total_decode_allocation_charges: usage.total_allocated_bytes() - 1,
                ..limits
            },
        ),
        Err(Error::Encode(norito::Error::TotalAllocationExceeded { .. }))
    ));
    assert!(matches!(
        verify(&prepared, &expected, &encoded, BundleLimits::default()),
        Err(Error::VerifierLimitExceeded {
            limit: "max_bundle_wire_bytes",
            ..
        })
    ));
    for (restricted, name) in [
        (
            BundleLimits {
                max_segments: 1,
                ..limits
            },
            "max_bundle_segments",
        ),
        (
            BundleLimits {
                max_total_queries: 271,
                ..limits
            },
            "max_bundle_queries",
        ),
        (
            BundleLimits {
                max_total_statement_bytes: verified.statement_bytes() - 1,
                ..limits
            },
            "max_compact_bundle_statement_bytes",
        ),
    ] {
        assert!(
            matches!(verify(&prepared, &expected, &encoded, restricted), Err(Error::VerifierLimitExceeded { limit, .. }) if limit == name)
        );
    }
    // An explicit larger outer envelope does not raise the default child cap.
    assert!(
        verify(
            &prepared,
            &expected,
            &encoded,
            BundleLimits {
                segment: VerifyLimits::default(),
                ..limits
            }
        )
        .is_err()
    );
    let wire: AxtBundleWire = norito::decode_canonical(&encoded).unwrap();
    for mutation in 0..6 {
        let mut changed = wire.clone();
        match mutation {
            0 => changed.segments.swap(0, 1),
            1 => changed.segments[1] = changed.segments[0].clone(),
            2 => {
                changed.segments.pop();
            }
            3 => changed.segments.push(changed.segments[1].clone()),
            4 => changed.intermediate_roots[0][0] ^= 1,
            5 => {
                let last = changed.segments[1].len() - 1;
                changed.segments[1][last] ^= 1;
            }
            _ => unreachable!(),
        }
        if matches!(mutation, 0 | 1 | 4) {
            encode_axt_wire(&changed, 2, limits).unwrap();
        }
        if mutation == 4 {
            AxtTransferBatch::new(
                &prepared,
                &expected,
                &changed.intermediate_roots,
                context(&fixture),
                BatchContextLimits {
                    max_segments: 2,
                    max_total_statement_bytes: limits.max_total_statement_bytes,
                },
            )
            .unwrap();
        }
        let raw = norito::encode_canonical(&changed).unwrap();
        assert!(
            verify(&prepared, &expected, &raw, limits).is_err(),
            "mutation {mutation}"
        );
    }
    let mut wrong_expected = expected;
    wrong_expected.slot ^= 1;
    assert!(matches!(
        verify(&prepared, &wrong_expected, &encoded, limits),
        Err(Error::PublicIoMismatch { .. })
    ));
    let mut wrong_mirror = context(&fixture);
    wrong_mirror.mirrors.manifest_root[0] ^= 1;
    assert!(matches!(
        verify_axt_transfer_bundle(&prepared, &expected, wrong_mirror, &encoded, limits),
        Err(Error::InvalidAxtBinding { .. })
    ));
    let missing = AxtVerificationContext {
        remote_spend_claims: None,
        ..context(&fixture)
    };
    assert!(matches!(
        verify_axt_transfer_bundle(&prepared, &expected, missing, &encoded, limits),
        Err(Error::MissingMetadata { .. })
    ));

    // Both alternative contexts are internally well formed. Their rejection
    // must come from binding the proof to the original complete pre-proof facts.
    let mut manifest = fixture.outer.manifest_root;
    manifest[0] ^= 1;
    let mut changed_metadata = context(&fixture);
    changed_metadata.metadata.manifest_root = &manifest;
    changed_metadata.mirrors.manifest_root = manifest;
    let mut remote = fixture.remote.as_ref().unwrap().clone();
    remote[0].handle_replay_key.handle_era = 17;
    remote.sort_by_key(compute_remote_spend_claim_commitment_v1);
    let mut binding = fixture.binding.clone();
    binding.remote_spend_intent_commitments = remote
        .iter()
        .map(compute_remote_spend_claim_commitment_v1)
        .collect();
    let changed_remote = AxtVerificationContext {
        binding: &binding,
        remote_spend_claims: Some(&remote),
        ..context(&fixture)
    };
    for changed_context in [changed_metadata, changed_remote] {
        let changed = AxtTransferBatch::new(
            &prepared,
            &expected,
            &wire.intermediate_roots,
            changed_context,
            BatchContextLimits {
                max_segments: 2,
                max_total_statement_bytes: limits.max_total_statement_bytes,
            },
        )
        .unwrap();
        assert_eq!(changed.public_io(), expected);
        assert!(
            verify_axt_transfer_bundle(&prepared, &expected, changed_context, &encoded, limits)
                .is_err()
        );
    }

    // Removing one duplicate remote transfer fact and updating its commitment
    // list still cannot omit a real public occurrence from the source batch.
    let mut omitted_remote = fixture.remote.as_ref().unwrap().clone();
    omitted_remote.pop();
    let mut omitted_binding = fixture.binding.clone();
    omitted_binding.remote_spend_intent_commitments = omitted_remote
        .iter()
        .map(compute_remote_spend_claim_commitment_v1)
        .collect();
    let omitted = AxtVerificationContext {
        binding: &omitted_binding,
        remote_spend_claims: Some(&omitted_remote),
        ..context(&fixture)
    };
    assert!(
        matches!(verify_axt_transfer_bundle(&prepared, &expected, omitted, &encoded, limits), Err(Error::InvalidAxtBinding { details }) if details.contains("one-for-one"))
    );

    // Neither the ordinary entry point nor retagging only the outer carrier can
    // erase the child's distinct AXT identity and complete statement binding.
    assert!(verify_transfer_bundle(&prepared, &expected, &encoded, limits).is_err());
    let ordinary = fixture.prepare(ProofSemantics::TransferStateTransition);
    let ordinary_expected = fixture.expected(&ordinary);
    assert!(
        verify_axt_transfer_bundle(
            &ordinary,
            &ordinary_expected,
            context(&fixture),
            &encoded,
            limits
        )
        .is_err()
    );
    assert!(verify_transfer_bundle(&ordinary, &ordinary_expected, &encoded, limits).is_err());
    PublicTransferBatch::new(
        &ordinary,
        &ordinary_expected,
        &wire.intermediate_roots,
        BatchContextLimits {
            max_segments: 2,
            max_total_statement_bytes: limits.max_total_statement_bytes,
        },
    )
    .unwrap();
    let ordinary_wire = encode_wire(
        &BundleWire {
            version: wire.version,
            intermediate_roots: wire.intermediate_roots.clone(),
            segments: wire.segments.clone(),
        },
        2,
        limits,
    )
    .unwrap();
    assert!(verify_transfer_bundle(&ordinary, &ordinary_expected, &ordinary_wire, limits).is_err());

    let mut changed_inputs = inputs;
    changed_inputs.perm_root[0] ^= 1;
    let changed = prepare_public_transfers(
        &rows,
        &claims,
        changed_inputs,
        ProofSemantics::AxtTransferClaim,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let changed_expected = PublicIO {
        perm_root: changed_inputs.perm_root,
        ..expected
    };
    assert!(
        AxtTransferBatch::new(
            &changed,
            &changed_expected,
            &wire.intermediate_roots,
            context(&fixture),
            BatchContextLimits {
                max_segments: 2,
                max_total_statement_bytes: limits.max_total_statement_bytes
            }
        )
        .is_ok()
    );
    assert!(verify(&changed, &changed_expected, &encoded, limits).is_err());
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(verify(&prepared, &expected, &trailing, limits).is_err());
    let digest = Sha256::digest(&encoded);
    let name = format!("compact-axt-two-delta-bundle-{}.bin", hex::encode(digest));
    let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation");
    std::fs::create_dir_all(&artifact_dir).unwrap();
    let artifact = artifact_dir.join(name);
    std::fs::write(&artifact, &encoded).unwrap();
    eprintln!(
        "compact_axt_two_delta_construction_and_proving={construction_and_proving:?}; construction_seconds={construction_seconds:?}; typed_bytes={typed_bytes:?}; segment_bytes={segment_bytes:?}; proving_seconds={proving_seconds:?}; conversion_seconds={conversion_seconds:?}; verifying={verifying:?}; wire_bytes={}; statement_bytes={}; decode_allocation_charges={}; work={:?}; default_admitted=false; production_security_qualified=false; artifact={}",
        encoded.len(),
        verified.statement_bytes(),
        usage.total_allocated_bytes(),
        verified.work(),
        artifact.display()
    );
}
