//! Whole-entry rejection shape boundaries and pre-work owner checks.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::{isi::Log, transaction::TransactionBuilder};
use iroha_test_samples::{ALICE_ID, BOB_ID, gen_account_in};
use nonzero_ext::nonzero;

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    )
}

fn generous() -> FastpqSourceLimitsV1 {
    FastpqSourceLimitsV1 {
        max_executed_entries: 1,
        max_transcripts: 2,
        max_deltas: 2,
        max_input_transcript_bytes: 1_000_000,
        max_statement_bytes: 1_000_000,
        max_total_statement_bytes: 1_000_000,
    }
}

fn exact(usage: crate::fastpq::FastpqSourceTranscriptUsage) -> FastpqSourceLimitsV1 {
    FastpqSourceLimitsV1 {
        max_executed_entries: 1,
        max_transcripts: usage.transcripts.try_into().unwrap(),
        max_deltas: usage.deltas.try_into().unwrap(),
        max_input_transcript_bytes: usage.input_transcript_bytes.try_into().unwrap(),
        max_statement_bytes: usage.max_statement_bytes.try_into().unwrap(),
        max_total_statement_bytes: usage.total_statement_bytes.try_into().unwrap(),
    }
}

#[test]
fn empty_rejection_tail_has_no_transfer_or_statement_usage() {
    let usage = measure_tail(iroha_crypto::Hash::new(b"empty tail"), &[], generous()).unwrap();
    assert_eq!(
        (
            usage.transcripts,
            usage.deltas,
            usage.input_transcript_bytes
        ),
        (0, 0, 0)
    );
    assert_eq!(
        (usage.max_statement_bytes, usage.total_statement_bytes),
        (0, 0)
    );
}

#[test]
fn fee_penalty_and_combined_tail_use_one_exact_statement_frame() {
    let asset = asset();
    let hash = iroha_crypto::Hash::new(b"same retained entry");
    let tails = [
        TransferIdentities {
            from: &ALICE_ID,
            to: &BOB_ID,
            asset: &asset,
        },
        TransferIdentities {
            from: &BOB_ID,
            to: &ALICE_ID,
            asset: &asset,
        },
    ];
    let penalty = measure_tail(hash, &tails[..1], generous()).unwrap();
    let fee = measure_tail(hash, &tails[1..], generous()).unwrap();
    let combined = measure_tail(hash, &tails, generous()).unwrap();
    assert_eq!(
        (penalty.transcripts, fee.transcripts, combined.transcripts),
        (1, 1, 2)
    );
    assert_eq!(combined.deltas, 2);
    assert_eq!(
        combined.input_transcript_bytes,
        penalty.input_transcript_bytes + fee.input_transcript_bytes
    );
    assert_eq!(combined.max_statement_bytes, combined.total_statement_bytes);
    assert!(
        combined.total_statement_bytes < penalty.total_statement_bytes + fee.total_statement_bytes
    );
    let exact = exact(combined);
    measure_tail(hash, &tails, exact).unwrap();
    for dimension in 0..5 {
        let mut short = exact;
        match dimension {
            0 => short.max_transcripts -= 1,
            1 => short.max_deltas -= 1,
            2 => short.max_input_transcript_bytes -= 1,
            3 => short.max_statement_bytes -= 1,
            _ => short.max_total_statement_bytes -= 1,
        }
        assert!(
            matches!(measure_tail(hash, &tails, short), Err(TailError::Intrinsic)),
            "dimension {dimension}"
        );
    }
}

/// Public-material shape fixtures carry no private signing or execution authority.
fn mldsa_tail_account(members: usize, marker: u8) -> AccountId {
    use iroha_crypto::{Algorithm, PublicKey};
    use iroha_data_model::account::controller::{MultisigMember, MultisigPolicy};
    let keys: Vec<_> = (0..members)
        .map(|index| {
            let mut material = vec![marker; 1_952];
            material[..8].copy_from_slice(&(index as u64).to_le_bytes());
            PublicKey::from_bytes(Algorithm::MlDsa, &material).unwrap()
        })
        .collect();
    if members == 1 {
        AccountId::new(keys[0].clone())
    } else {
        AccountId::new_multisig(
            MultisigPolicy::new(
                1,
                keys.into_iter()
                    .map(|key| MultisigMember::new(key, 1).unwrap())
                    .collect(),
            )
            .unwrap(),
        )
    }
}

#[test]
fn bootstrap_covers_combined_ballot_penalty_and_gas_tail_at_widest_quantities() {
    let ceiling = iroha_data_model::parameter::FastpqSourcePolicyV1::bootstrap().intrinsic;
    let asset = asset();
    let hash = iroha_crypto::Hash::new(b"bootstrap combined rejection tail");
    for (from, to, expected_input) in [
        (ALICE_ID.clone(), BOB_ID.clone(), 1_782),
        (mldsa_tail_account(1, 17), mldsa_tail_account(1, 23), 17_150),
    ] {
        // These two occurrences stand for the retained penalty and PipelineGas
        // shapes, which the preflight deliberately measures with maximum quantities.
        let tails = [
            TransferIdentities {
                from: &from,
                to: &to,
                asset: &asset,
            },
            TransferIdentities {
                from: &to,
                to: &from,
                asset: &asset,
            },
        ];
        let usage = measure_tail(hash, &tails, ceiling).unwrap();
        assert_eq!((usage.transcripts, usage.deltas), (2, 2));
        assert_eq!(usage.input_transcript_bytes, expected_input);
        assert_eq!(usage.max_statement_bytes, usage.total_statement_bytes);
        assert!(exact(usage).fits_within(ceiling));
        let one = measure_tail(hash, &tails[..1], ceiling).unwrap();
        assert_eq!(usage.input_transcript_bytes, 2 * one.input_transcript_bytes);
        assert!(usage.total_statement_bytes < 2 * one.total_statement_bytes);
    }
    // Bootstrap covers one full 16-member pair, not two such pairs. Preserve
    // finite exact admission instead of treating the member count as authority.
    let from = mldsa_tail_account(16, 17);
    let to = mldsa_tail_account(16, 23);
    let tails = [
        TransferIdentities {
            from: &from,
            to: &to,
            asset: &asset,
        },
        TransferIdentities {
            from: &to,
            to: &from,
            asset: &asset,
        },
    ];
    measure_tail(hash, &tails[..1], ceiling).unwrap();
    let wider = measure_tail(hash, &tails, generous()).unwrap();
    assert_eq!(wider.input_transcript_bytes, 252_514);
    assert!(matches!(
        measure_tail(hash, &tails, ceiling),
        Err(TailError::Intrinsic)
    ));
}

#[test]
fn identity_lengths_are_refused_before_source_shape_allocation() {
    let asset = asset();
    let tails = [TransferIdentities {
        from: &ALICE_ID,
        to: &BOB_ID,
        asset: &asset,
    }];
    let ceiling = FastpqSourceLimitsV1 {
        max_input_transcript_bytes: 1,
        ..generous()
    };
    assert!(matches!(
        measure_tail(iroha_crypto::Hash::new(b"identities"), &tails, ceiling),
        Err(TailError::Intrinsic)
    ));
}

#[test]
fn widest_original_quantity_covers_fee_and_governance_encoding_boundaries() {
    let widest = widest_quantity().unwrap();
    let max_len = norito::canonical_frame_len(&widest).unwrap();
    let mut bytes = [0xff; MAX_MANTISSA_BYTES];
    *bytes.last_mut().unwrap() = 0x7f;
    for scale in [0, 1, MAX_DECIMAL_SCALE] {
        for mantissa in [
            BigInt::from_twos_bytes(&bytes).unwrap(),
            BigInt::from(u128::MAX),
        ] {
            let quantity =
                Quantity::try_from_numeric(Numeric::try_new(mantissa, scale).unwrap()).unwrap();
            assert!(norito::canonical_frame_len(&quantity).unwrap() <= max_len);
        }
    }
    for quantity in [
        Quantity::zero(),
        Quantity::one(),
        Quantity::from(u64::MAX),
        Quantity::from(u128::MAX),
    ] {
        assert!(norito::canonical_frame_len(&quantity).unwrap() <= max_len);
    }
}

#[test]
fn codec_and_impossible_shape_failures_are_never_capacity_rejections() {
    for error in [
        EntryFrameLengthError::EntryIdentity,
        EntryFrameLengthError::EmptyOccurrence,
        EntryFrameLengthError::Prefix(PrefixLengthError::UnsupportedLayout),
        EntryFrameLengthError::Prefix(PrefixLengthError::Overflow),
        EntryFrameLengthError::Prefix(PrefixLengthError::QuantityShape),
    ] {
        assert!(matches!(classify(error), TailError::Fault(_)));
    }
}

#[test]
fn missing_source_owner_fails_before_fee_or_effect_work() {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (authority, key) = gen_account_in("wonderland");
    let signed = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        iroha_logger::Level::INFO,
        "tail fixture".to_owned(),
    )])
    .sign(key.private_key());
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 1, 0));
    let hash = iroha_crypto::Hash::from(signed.hash_as_entrypoint());
    let mut tx = block.transaction();
    tx.tx_call_hash = Some(hash);
    assert!(matches!(
        preflight(&mut tx, &authority, &signed, None, None),
        Err(ValidationFail::InternalError(_))
    ));
    assert_eq!(tx.last_tx_gas_used, 0);
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
    assert!(tx.fastpq_rejection_tail_context(hash).is_err());
}

#[test]
fn admitted_empty_source_preflight_leaves_owner_and_work_unchanged() {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (authority, key) = gen_account_in("wonderland");
    let signed = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        iroha_logger::Level::INFO,
        "tail fixture".to_owned(),
    )])
    .sign(key.private_key());
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 1, 0));
    let hash = iroha_crypto::Hash::from(signed.hash_as_entrypoint());
    let mut tx = block.transaction_for_fastpq_testing(hash);
    let before = tx.fastpq_rejection_tail_context(hash).unwrap();
    preflight(&mut tx, &authority, &signed, None, None).unwrap();
    assert_eq!(tx.fastpq_rejection_tail_context(hash).unwrap(), before);
    assert_eq!(tx.last_tx_gas_used, 0);
    assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
}
