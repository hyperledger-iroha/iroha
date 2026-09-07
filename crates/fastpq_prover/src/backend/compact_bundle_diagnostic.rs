//! Explicit complete two-delta ordinary bundle regression with real SMT paths.
//!
//! The fixture is public and deterministic. Private witnesses exist only while
//! constructing/proving the two segments and are dropped before verification.
//! This expensive ignored diagnostic does not qualify production security.

use super::{
    compact_bundle::{BundleLimits, BundleWire, encode_wire, verify_transfer_bundle},
    compact_protocol::{self, shared_openings},
    compact_public_batch::{BatchContextLimits, PublicTransferBatch},
};
use crate::{
    Error, OperationKind, ProofSemantics, PublicInputs, StateTransition, VerifyLimits,
    gadgets::{
        compact_smt_air::{COLUMN_COUNT, PATH_LEVELS, PHYSICAL_ROW_COUNT, SmtWitness},
        compact_trace_columns::smt_row_cells,
        public_transfer_statement::{
            PublicTransferLimits, prepare_public_transfers, public_claims_from_transcripts,
        },
        transfer::attach_transfer_smt_witnesses,
    },
    proof::PublicIO,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    DomainId,
    asset::id::AssetDefinitionId,
    fastpq::{TransferDeltaTranscript, TransferSmtWitness, TransferTranscript},
};
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use sha2::{Digest as _, Sha256};

#[test]
#[ignore = "explicit two complete 65536x342 proofs with ordered raw bundle verification"]
fn complete_two_delta_bundle_verifies_after_private_witnesses_are_dropped() {
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
    let (rows, claims, inputs, expected, encoded, proving_seconds, conversion_seconds) = {
        let asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let mut rows = Vec::new();
        let mut deltas = Vec::new();
        for ordinal in 0..2_u64 {
            let before = [100 - ordinal * 25, 200 + ordinal * 25];
            let after = [before[0] - 25, before[1] + 25];
            for (account, index) in [(&*ALICE_ID, 0), (&*BOB_ID, 1)] {
                rows.push(StateTransition::new(
                    iroha_data_model::fastpq::transfer_balance_key(&asset, account).unwrap(),
                    before[index].to_le_bytes().to_vec(),
                    after[index].to_le_bytes().to_vec(),
                    OperationKind::Transfer,
                ));
            }
            deltas.push(TransferDeltaTranscript {
                from_account: (*ALICE_ID).clone(),
                to_account: (*BOB_ID).clone(),
                asset_definition: asset.clone(),
                amount: Quantity::from(25_u64),
                from_balance_before: Quantity::from(before[0]),
                from_balance_after: Quantity::from(after[0]),
                to_balance_before: Quantity::from(before[1]),
                to_balance_after: Quantity::from(after[1]),
                from_smt_witness: TransferSmtWitness::default(),
                to_smt_witness: TransferSmtWitness::default(),
            });
        }
        rows.sort_by(|left, right| left.key.cmp(&right.key));
        let mut transcripts = vec![TransferTranscript {
            batch_hash: Hash::new(b"compact ordinary two-delta bundle fixture"),
            authority_digest: Hash::new(b"public authority context, not authorization"),
            poseidon_preimage_digest: None,
            deltas,
        }];
        let claims =
            public_claims_from_transcripts(&transcripts, PublicTransferLimits::default()).unwrap();
        let (old_root, new_root) = attach_transfer_smt_witnesses(&mut transcripts).unwrap();
        assert_eq!(
            public_claims_from_transcripts(&transcripts, PublicTransferLimits::default()).unwrap(),
            claims
        );
        let middle = transcripts[0].deltas[0].to_smt_witness.root_after;
        assert_eq!(
            middle,
            transcripts[0].deltas[1].from_smt_witness.root_before
        );
        assert_ne!(old_root, middle);
        assert_ne!(middle, new_root);
        let inputs = PublicInputs {
            dsid: [7; 16],
            slot: 17,
            old_root,
            new_root,
            perm_root: Hash::new(b"public permission context").into(),
            tx_set_hash: Hash::new(b"public transaction context").into(),
        };
        let prepared = prepare_public_transfers(
            &rows,
            &claims,
            inputs,
            ProofSemantics::StateTransition,
            PublicTransferLimits::default(),
        )
        .unwrap();
        let expected = PublicIO {
            dsid: inputs.dsid,
            slot: inputs.slot,
            old_root,
            new_root,
            perm_root: inputs.perm_root,
            tx_set_hash: inputs.tx_set_hash,
            ordering_hash: prepared.ordering_hash().into(),
        };
        let batch = PublicTransferBatch::new(
            &prepared,
            &expected,
            &[middle],
            BatchContextLimits {
                max_segments: limits.max_segments,
                max_total_statement_bytes: limits.max_total_statement_bytes,
            },
        )
        .unwrap();
        assert_eq!(batch.segment_count(), 2);
        let mut frames = Vec::new();
        let mut proving = [0.0; 2];
        let mut conversion = [0.0; 2];
        for (ordinal, delta) in transcripts[0].deltas.iter().enumerate() {
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
            let relation = batch.segment(ordinal).unwrap();
            let started = std::time::Instant::now();
            let proof = compact_protocol::prove(&relation, &columns).unwrap();
            proving[ordinal] = started.elapsed().as_secs_f64();
            drop(columns);
            let started = std::time::Instant::now();
            let shared = shared_openings::from_compact(&relation, &proof, limits.segment).unwrap();
            frames.push(norito::encode_canonical(&shared).unwrap());
            conversion[ordinal] = started.elapsed().as_secs_f64();
        }
        let encoded = encode_wire(
            &BundleWire {
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
        (rows, claims, inputs, expected, encoded, proving, conversion)
    };
    let construction_and_proving = construction_started.elapsed();
    let prepared = prepare_public_transfers(
        &rows,
        &claims,
        inputs,
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let started = std::time::Instant::now();
    let (verified, usage) = norito::core::with_decode_limits_measured(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 16),
        || verify_transfer_bundle(&prepared, &expected, &encoded, limits),
    );
    let verified = verified.unwrap();
    let verifying = started.elapsed();
    assert_eq!(verified.public_io(), expected);
    assert_eq!(verified.segments(), 2);
    assert_eq!(verified.wire_bytes(), encoded.len());
    assert_eq!(verified.work().transcripts, 2);
    assert_eq!(verified.work().air_evaluations, 272);
    assert_eq!(verified.work().terminal_degree_checks, 2);
    assert!(verified.work().proof_bytes < encoded.len());
    // The cumulative charge ceiling is inclusive across the outer frame and
    // both child decodes; neither child may reset the parent scope.
    assert_eq!(
        verify_transfer_bundle(
            &prepared,
            &expected,
            &encoded,
            BundleLimits {
                max_total_decode_allocation_charges: usage.total_allocated_bytes(),
                ..limits
            },
        )
        .unwrap(),
        verified
    );
    assert!(matches!(
        verify_transfer_bundle(
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
    assert!(
        verify_transfer_bundle(&prepared, &expected, &encoded, BundleLimits::default()).is_err()
    );
    for restricted in [
        BundleLimits {
            max_total_queries: 271,
            ..limits
        },
        BundleLimits {
            max_total_statement_bytes: verified.statement_bytes() - 1,
            ..limits
        },
    ] {
        assert!(verify_transfer_bundle(&prepared, &expected, &encoded, restricted).is_err());
    }
    let wire: BundleWire = norito::decode_canonical(&encoded).unwrap();
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
        let raw = norito::encode_canonical(&changed).unwrap();
        assert!(
            verify_transfer_bundle(&prepared, &expected, &raw, limits).is_err(),
            "mutation {mutation}"
        );
    }
    let mut changed_inputs = inputs;
    changed_inputs.perm_root[0] ^= 1;
    let changed = prepare_public_transfers(
        &rows,
        &claims,
        changed_inputs,
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let changed_expected = PublicIO {
        perm_root: changed_inputs.perm_root,
        ..expected
    };
    assert!(
        PublicTransferBatch::new(
            &changed,
            &changed_expected,
            &wire.intermediate_roots,
            BatchContextLimits {
                max_segments: 2,
                max_total_statement_bytes: limits.max_total_statement_bytes
            }
        )
        .is_ok()
    );
    assert!(verify_transfer_bundle(&changed, &changed_expected, &encoded, limits).is_err());
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(verify_transfer_bundle(&prepared, &expected, &trailing, limits).is_err());
    let digest = Sha256::digest(&encoded);
    let name = format!(
        "compact-ordinary-two-delta-bundle-{}.bin",
        hex::encode(digest)
    );
    let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation");
    std::fs::create_dir_all(&artifact_dir).unwrap();
    let artifact = artifact_dir.join(name);
    std::fs::write(&artifact, &encoded).unwrap();
    eprintln!(
        "compact_two_delta_construction_and_proving={construction_and_proving:?}; proving_seconds={proving_seconds:?}; conversion_seconds={conversion_seconds:?}; verifying={verifying:?}; wire_bytes={}; statement_bytes={}; decode_allocation_charges={}; work={:?}; default_admitted=false; production_security_qualified=false; artifact={}",
        encoded.len(),
        verified.statement_bytes(),
        usage.total_allocated_bytes(),
        verified.work(),
        artifact.display()
    );
}
