//! Explicit complete SHAKE-candidate proof with real public transfer facts.
//!
//! This public deterministic fixture measures the shared engine after all private
//! paths and trace columns are dropped. It does not qualify production security.

use super::*;
use crate::{
    OperationKind, ProofSemantics, PublicInputs, StateTransition,
    backend::compact_public_transfer::PublicTransferAir,
    gadgets::{
        compact_smt_air::{COLUMN_COUNT, PATH_LEVELS, PHYSICAL_ROW_COUNT, SmtWitness},
        compact_trace_columns::smt_row_cells,
        public_transfer_statement::{
            PublicTransferLimits, prepare_public_transfers, public_claims_from_transcripts,
        },
        transfer::attach_transfer_smt_witnesses,
    },
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

struct VerifyOnly<'a>(&'a PublicTransferAir);
impl FixedAir for VerifyOnly<'_> {
    fn schema(&self) -> FixedAirSchema {
        self.0.schema()
    }
    fn statement_bytes(&self) -> &[u8] {
        self.0.statement_bytes()
    }
    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        self.0.evaluate(point, current, next)
    }
    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
        panic!("succinct verification must never prepare a private witness or trace")
    }
}

#[test]
#[ignore = "explicit complete 65536x342 SHAKE candidate proof and 375-query raw verification"]
fn complete_candidate_transfer_verifies_after_private_witnesses_are_dropped() {
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let limits = VerifyLimits {
        max_proof_bytes: 4_326_227,
        max_queries: 375,
        ..VerifyLimits::default()
    };
    let conversion_limits = VerifyLimits {
        max_proof_bytes: 16 * 1024 * 1024,
        ..limits
    };
    let started = std::time::Instant::now();
    let (rows, claims, inputs, expected, bytes, proving) = {
        let asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let mut rows: Vec<_> = [(&*ALICE_ID, 100_u64, 83_u64), (&*BOB_ID, 200, 217)]
            .into_iter()
            .map(|(account, before, after)| {
                StateTransition::new(
                    format!("asset/{asset}/{account}").into_bytes(),
                    before.to_le_bytes().to_vec(),
                    after.to_le_bytes().to_vec(),
                    OperationKind::Transfer,
                )
            })
            .collect();
        rows.sort_by(|a, b| a.key.cmp(&b.key));
        let mut private = vec![TransferTranscript {
            batch_hash: Hash::new(b"SHAKE candidate public transfer fixture"),
            authority_digest: Hash::new(b"public caller context, not an authorization grant"),
            poseidon_preimage_digest: None,
            deltas: vec![TransferDeltaTranscript {
                from_account: (*ALICE_ID).clone(),
                to_account: (*BOB_ID).clone(),
                asset_definition: asset,
                amount: Quantity::from(17_u64),
                from_balance_before: Quantity::from(100_u64),
                from_balance_after: Quantity::from(83_u64),
                to_balance_before: Quantity::from(200_u64),
                to_balance_after: Quantity::from(217_u64),
                from_smt_witness: TransferSmtWitness::default(),
                to_smt_witness: TransferSmtWitness::default(),
            }],
        }];
        private[0].poseidon_preimage_digest =
            Some(crate::gadgets::transfer::compute_poseidon_digest(
                &private[0].deltas[0],
                &private[0].batch_hash,
            ));
        let claims =
            public_claims_from_transcripts(&private, PublicTransferLimits::default()).unwrap();
        let (old_root, new_root) = attach_transfer_smt_witnesses(&mut private).unwrap();
        assert_eq!(
            claims,
            public_claims_from_transcripts(&private, PublicTransferLimits::default()).unwrap()
        );
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
            ProofSemantics::TransferStateTransition,
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
        let statements = prepared.compact_statements(&[]).unwrap();
        let columns = {
            let delta = &private[0].deltas[0];
            let paths = [&delta.from_smt_witness, &delta.to_smt_witness];
            for (path, update) in paths.iter().zip(statements[0].updates) {
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
            let witness = SmtWitness::from_inputs(&statements[0], &siblings)
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
        drop(private);
        let air = PublicTransferAir::new(&prepared, &expected).unwrap();
        eprintln!(
            "candidate construction complete at {:?}; starting full proof",
            started.elapsed()
        );
        let prove_started = std::time::Instant::now();
        let proof = prove_shake_shared(&air, &columns, conversion_limits).unwrap();
        let proving = prove_started.elapsed();
        drop(columns);
        let bytes = norito::encode_canonical(&proof).unwrap();
        assert!(bytes.len() <= limits.max_proof_bytes);
        eprintln!(
            "candidate proof complete: {proving:?}, {} bytes",
            bytes.len()
        );
        // All paths, private witness rows, columns, retained trees, expanded
        // openings, prover-only AIR caches and proof DTOs leave this scope.
        drop(air);
        drop(prepared);
        (rows, claims, inputs, expected, bytes, proving)
    };
    let prepared = prepare_public_transfers(
        &rows,
        &claims,
        inputs,
        ProofSemantics::TransferStateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let air = PublicTransferAir::new(&prepared, &expected).unwrap();
    let verifier = VerifyOnly(&air);
    let verifying_started = std::time::Instant::now();
    let budget =
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 128 * 1024 * 1024, 16);
    let (verified, usage) = norito::core::with_decode_limits_measured(budget, || {
        codec::decode_and_verify_shake(&verifier, &bytes, limits, 128 * 1024 * 1024)
    });
    let work = verified.unwrap();
    let verifying = verifying_started.elapsed();
    assert_eq!(work.proof_bytes, bytes.len());
    assert_eq!(work.transcripts, 1);
    assert_eq!(work.air_evaluations, 375);
    assert_eq!(work.terminal_degree_checks, 1);
    assert!(work.row_leaves <= 750);
    assert_eq!(work.oracle_leaves, 750);
    assert!(work.row_leaves + work.oracle_leaves + work.fri_leaves <= 5759);
    assert!(work.parent_hashes <= 38_782);
    let charges = usage.total_allocated_bytes();
    assert!(charges > 0 && charges < 128 * 1024 * 1024);
    let facade = crate::backend::compact_public_api::verify_shake_transfer(
        &prepared, &expected, &bytes, limits, charges,
    )
    .unwrap();
    assert_eq!(facade.public_io(), expected);
    assert_eq!(facade.work(), work);

    assert_eq!(
        codec::decode_and_verify_shake(&verifier, &bytes, limits, charges).unwrap(),
        work
    );
    assert!(matches!(
        codec::decode_and_verify_shake(&verifier, &bytes, limits, charges - 1),
        Err(Error::Encode(norito::Error::TotalAllocationExceeded { .. }))
    ));
    assert!(matches!(
        codec::decode_and_verify_shake(&verifier, &bytes, VerifyLimits::default(), usize::MAX),
        Err(Error::VerifierLimitExceeded {
            limit: "max_proof_bytes",
            ..
        })
    ));
    assert!(matches!(
        codec::decode_and_verify_shake(
            &verifier,
            &bytes,
            VerifyLimits {
                max_queries: 374,
                ..limits
            },
            usize::MAX
        ),
        Err(Error::VerifierLimitExceeded {
            limit: "max_queries",
            ..
        })
    ));
    assert!(codec::decode_and_verify(&verifier, &bytes, limits).is_err());
    assert!(norito::decode_canonical::<SharedProof>(&bytes).is_err());
    assert_eq!(
        norito::encode_canonical(&norito::decode_canonical::<ShakeSharedProof>(&bytes).unwrap())
            .unwrap(),
        bytes
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(codec::decode_and_verify_shake(&verifier, &trailing, limits, usize::MAX).is_err());
    drop(trailing);
    let candidate: ShakeSharedProof = norito::decode_canonical(&bytes).unwrap();
    for kind in 0..7 {
        let mut bad = candidate.clone();
        match kind {
            0 => bad.rows[0].values[341] = crate::backend::add_mod(bad.rows[0].values[341], 1),
            1 => {
                let mut words = bad.row_root.words();
                words[5] = crate::backend::add_mod(words[5], 1);
                bad.row_root = WireDigest::new(words).unwrap();
            }
            2 => bad.queries[0].mixed = bad.queries[0].mixed.add(GoldilocksFp4V1::ONE),
            3 => bad.queries[0].quotient = bad.queries[0].quotient.add(GoldilocksFp4V1::ONE),
            4 => {
                bad.rounds[0].groups[0].values[1] =
                    bad.rounds[0].groups[0].values[1].add(GoldilocksFp4V1::ONE)
            }
            5 => bad.terminal_values[0] = bad.terminal_values[0].add(GoldilocksFp4V1::ONE),
            6 => bad.rows[0].index ^= 8,
            _ => unreachable!(),
        }
        assert!(
            verify_shake_shared(&verifier, bad, limits).is_err(),
            "tamper {kind}"
        );
    }
    // Exact public caller context affects the transcript and every oracle.
    let changed_inputs = PublicInputs {
        slot: inputs.slot + 1,
        ..inputs
    };
    let changed_prepared = prepare_public_transfers(
        &rows,
        &claims,
        changed_inputs,
        ProofSemantics::TransferStateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let changed_expected = PublicIO {
        slot: expected.slot + 1,
        ..expected
    };
    let changed_air = PublicTransferAir::new(&changed_prepared, &changed_expected).unwrap();
    assert!(
        codec::decode_and_verify_shake(&VerifyOnly(&changed_air), &bytes, limits, usize::MAX)
            .is_err()
    );
    let sha = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(
        sha,
        "ccb9eb92d8f517d85213399c276a2f5ac2da4f6eb740b29d4df79b54bd487e0e"
    );
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("compact-shake-full-transfer-{sha}.bin"));
    std::fs::write(&path, &bytes).unwrap();
    eprintln!(
        "candidate_proving={proving:?}; raw_verifying={verifying:?}; work={work:?}; decode_allocation_charges={charges}; decode_elements={}; total={:?}; public_fixture_artifact={}; production_security_qualified=false",
        usage.total_elements(),
        started.elapsed(),
        path.display()
    );
}
