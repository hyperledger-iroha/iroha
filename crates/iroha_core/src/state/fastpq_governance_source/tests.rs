//! Retained corpus count, replacement, zero-amount and variable-identity boundaries.

use super::*;
use crate::state::GovernanceLockRecord;
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::{
    account::controller::{MultisigMember, MultisigPolicy},
    asset::AssetDefinitionId,
};
use iroha_model_base::domain::DomainId;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use std::collections::BTreeMap;

pub(super) fn custody() -> GovernanceLockCustody {
    GovernanceLockCustody {
        escrowed: true,
        asset_definition_id: AssetDefinitionId::derive_from_components(
            DomainId::try_new("source-mandatory", "universal").unwrap(),
            "asset".parse().unwrap(),
        ),
        bond_escrow_account: BOB_ID.clone(),
        slash_receiver_account: BOB_ID.clone(),
    }
}

fn locks() -> GovernanceLocksForReferendum {
    GovernanceLocksForReferendum {
        locks: BTreeMap::from([(
            ALICE_ID.clone(),
            GovernanceLockRecord {
                owner: ALICE_ID.clone(),
                amount: Quantity::zero(),
                slashed: Quantity::from(7_u32),
                expiry_height: 1,
                direction: 0,
                duration_blocks: 1,
                custody: custody(),
            },
        )]),
    }
}

fn large_controller(members: u8) -> AccountId {
    AccountId::new_multisig(
        MultisigPolicy::new(
            1,
            (1..=members)
                .map(|marker| {
                    let key = PublicKey::from_bytes(Algorithm::MlDsa, &vec![marker; 1952]).unwrap();
                    MultisigMember::new(key, 1).unwrap()
                })
                .collect(),
        )
        .unwrap(),
    )
}

#[test]
fn zero_slashed_and_overdue_records_keep_the_global_count_across_referenda() {
    let mut profile = FastpqSourcePolicyV1::bootstrap();
    profile.mandatory.max_retained_obligations = 2;
    let corpus = BTreeMap::from([("a".to_owned(), locks()), ("b".to_owned(), locks())]);
    validate_retained(profile, corpus.iter(), None, None).unwrap();
    let before = norito::encode_canonical(&corpus).unwrap();
    assert!(
        validate_retained(
            profile,
            corpus.iter(),
            Some(("c", &ALICE_ID, &custody())),
            None
        )
        .unwrap_err()
        .contains("global retained")
    );
    validate_retained(
        profile,
        corpus.iter(),
        Some(("a", &ALICE_ID, &custody())),
        None,
    )
    .unwrap();
    assert_eq!(norito::encode_canonical(&corpus).unwrap(), before);
}

#[test]
fn future_release_counts_actual_controller_frames_and_all_five_max_quantities() {
    let profile = FastpqSourcePolicyV1::bootstrap();
    let measured = future_release(&ALICE_ID, &custody(), profile.mandatory.per_obligation).unwrap();
    assert_eq!(measured.max_executed_entries, 1);
    assert_eq!(measured.max_transcripts, 1);
    assert_eq!(measured.max_deltas, 1);
    assert_eq!(measured.max_input_transcript_bytes, 891);
    assert_eq!(measured.max_statement_bytes, 1880);
    assert_eq!(
        measured.max_statement_bytes,
        measured.max_total_statement_bytes
    );
    let mut tiny = profile.mandatory.per_obligation;
    tiny.max_input_transcript_bytes = measured.max_input_transcript_bytes - 1;
    assert!(future_release(&ALICE_ID, &custody(), tiny).is_err());
    for dimension in 0..6 {
        let mut ceiling = measured;
        match dimension {
            0 => ceiling.max_executed_entries -= 1,
            1 => ceiling.max_transcripts -= 1,
            2 => ceiling.max_deltas -= 1,
            3 => ceiling.max_input_transcript_bytes -= 1,
            4 => ceiling.max_statement_bytes -= 1,
            5 => ceiling.max_total_statement_bytes -= 1,
            _ => unreachable!(),
        }
        assert!(
            future_release(&ALICE_ID, &custody(), ceiling).is_err(),
            "dimension {dimension}"
        );
    }
    assert_eq!(
        future_release(&ALICE_ID, &custody(), measured).unwrap(),
        measured
    );
}

#[test]
fn rekey_preflight_measures_new_identity_and_preserves_retained_state_on_refusal() {
    let profile = FastpqSourcePolicyV1::bootstrap();
    let corpus = BTreeMap::from([("lock".to_owned(), locks())]);
    let original = norito::encode_canonical(&corpus).unwrap();
    let oversized = large_controller(33);
    assert!(
        validate_retained(profile, corpus.iter(), None, Some((&ALICE_ID, &oversized))).is_err()
    );
    assert_eq!(norito::encode_canonical(&corpus).unwrap(), original);
    validate_retained(profile, corpus.iter(), None, Some((&ALICE_ID, &BOB_ID))).unwrap();
}

#[test]
fn malformed_empty_group_and_owner_substitution_refuse_restore() {
    let profile = FastpqSourcePolicyV1::bootstrap();
    let mut corpus = BTreeMap::from([("lock".to_owned(), GovernanceLocksForReferendum::default())]);
    assert!(
        validate_retained(profile, corpus.iter(), None, None)
            .unwrap_err()
            .contains("empty referendum")
    );
    corpus.insert("lock".to_owned(), locks());
    corpus
        .get_mut("lock")
        .unwrap()
        .locks
        .get_mut(&*ALICE_ID)
        .unwrap()
        .owner = BOB_ID.clone();
    assert!(
        validate_retained(profile, corpus.iter(), None, None)
            .unwrap_err()
            .contains("owner different from its record")
    );
}

#[test]
fn rekey_cannot_merge_two_retained_obligations() {
    let profile = FastpqSourcePolicyV1::bootstrap();
    let mut group = locks();
    let mut second = group.locks[&*ALICE_ID].clone();
    second.owner = BOB_ID.clone();
    group.locks.insert(BOB_ID.clone(), second);
    let corpus = BTreeMap::from([("lock".to_owned(), group)]);
    assert!(
        validate_retained(profile, corpus.iter(), None, Some((&ALICE_ID, &BOB_ID)))
            .unwrap_err()
            .contains("merge distinct retained obligations")
    );
}

#[test]
fn smaller_replacement_and_failed_retry_never_release_an_obligation() {
    let mut profile = FastpqSourcePolicyV1::bootstrap();
    profile.mandatory.max_retained_obligations = 1;
    let mut corpus = BTreeMap::from([("retry".to_owned(), locks())]);
    for _ in 0..3 {
        validate_retained(profile, corpus.iter(), None, None).unwrap();
        assert!(
            validate_retained(
                profile,
                corpus.iter(),
                Some(("next", &ALICE_ID, &custody())),
                None
            )
            .unwrap_err()
            .contains("global retained")
        );
    }
    let retained = corpus
        .get_mut("retry")
        .unwrap()
        .locks
        .get_mut(&*ALICE_ID)
        .unwrap();
    retained.amount = Quantity::zero();
    retained.expiry_height = u64::MAX;
    validate_retained(
        profile,
        corpus.iter(),
        Some(("retry", &ALICE_ID, &custody())),
        None,
    )
    .unwrap();
    assert!(
        validate_retained(
            profile,
            corpus.iter(),
            Some(("next", &ALICE_ID, &custody())),
            None
        )
        .is_err()
    );
    corpus.remove("retry");
    validate_retained(
        profile,
        corpus.iter(),
        Some(("next", &ALICE_ID, &custody())),
        None,
    )
    .unwrap();
}

#[test]
fn widest_future_frame_bounds_canonical_quantity_length_and_scale_boundaries() {
    let custody = custody();
    let reserved = future_release(
        &ALICE_ID,
        &custody,
        FastpqSourcePolicyV1::bootstrap().mandatory.per_obligation,
    )
    .unwrap();
    for width in [1, 2, MAX_MANTISSA_BYTES - 1, MAX_MANTISSA_BYTES] {
        for scale in [0, 1, MAX_DECIMAL_SCALE - 1, MAX_DECIMAL_SCALE] {
            let mut bytes = vec![0xff; width];
            *bytes.last_mut().unwrap() = 0x7f;
            let quantity = Quantity::try_from_numeric(
                Numeric::try_new(BigInt::from_twos_bytes(&bytes).unwrap(), scale).unwrap(),
            )
            .unwrap();
            let hash = Hash::new(b"canonical quantity boundary");
            let transcript = TransferTranscript {
                batch_hash: hash,
                authority_digest: hash,
                poseidon_preimage_digest: Some(hash),
                deltas: vec![TransferDeltaTranscript {
                    from_account: custody.bond_escrow_account.clone(),
                    to_account: ALICE_ID.clone(),
                    asset_definition: custody.asset_definition_id.clone(),
                    amount: quantity.clone(),
                    from_balance_before: quantity.clone(),
                    from_balance_after: quantity.clone(),
                    to_balance_before: quantity.clone(),
                    to_balance_after: quantity,
                    from_smt_witness: TransferSmtWitness::default(),
                    to_smt_witness: TransferSmtWitness::default(),
                }],
            };
            // Independent canonical encoder, not the incremental frame meter.
            let actual = norito::encode_canonical(&transcript).unwrap();
            assert!(
                actual.len() as u64 <= reserved.max_input_transcript_bytes,
                "width {width}, scale {scale}"
            );
        }
    }
}
