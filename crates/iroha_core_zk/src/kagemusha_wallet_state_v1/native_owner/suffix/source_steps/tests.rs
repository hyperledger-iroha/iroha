//! Real map witnesses and signed model originals; no proof-verifier stand-in.
use super::*;
use crate::kagemusha_wallet_state_v1::{
    session_custody,
    tests::{MemoryArchive, fixture},
};

fn credit(number: u8) -> KagemushaWalletCreditDigestLeafV1 {
    let mut credit_id = [0; 32];
    credit_id[0] = number;
    let mut payment_digest = [0; 32];
    payment_digest[0] = number + 42;
    KagemushaWalletCreditDigestLeafV1 {
        credit_id,
        payment_digest,
        burned: false,
    }
}

fn decode_record(
    witness: KagemushaWalletSuffixCreditWitnessV1,
) -> KagemushaWalletCreditDigestRecordV1 {
    match witness {
        KagemushaWalletSuffixCreditWitnessV1::Present { membership } => {
            let (leaf, opening) =
                KagemushaWalletIndexedOpeningV1::from_transcript(&membership).unwrap();
            KagemushaWalletCreditDigestRecordV1::Present {
                leaf: leaf.unwrap(),
                opening,
            }
        }
        KagemushaWalletSuffixCreditWitnessV1::Inserted { low, empty } => {
            let (leaf, low_opening) =
                KagemushaWalletIndexedOpeningV1::from_transcript(&low).unwrap();
            let (none, slot_opening) =
                KagemushaWalletIndexedOpeningV1::from_transcript(&empty).unwrap();
            assert!(none.is_none());
            KagemushaWalletCreditDigestRecordV1::Inserted {
                witness: KagemushaWalletIndexedInsertV1 {
                    low: leaf.unwrap(),
                    low_opening,
                    slot_opening,
                },
            }
        }
    }
}

#[test]
fn independent_credit_witnesses_verify_and_preserve_first_identity() {
    let mut store = MemoryArchive::new();
    let mut selected = CreditTree::default();
    let mut first = credit(9);
    first.burned = true;
    selected.record(&mut store, &first).unwrap();
    let selected_original = archive::encode(&selected).unwrap();
    let mut draft = selected.clone();
    let mut expected = draft.clone();
    for number in [7, 25, 2, 31, 11, 3, 9] {
        let credit = credit(number);
        let before = draft.root();
        let witness = record_credit(&mut store, &mut draft, &credit).unwrap();
        if number == 9 {
            assert!(matches!(
                witness,
                KagemushaWalletSuffixCreditWitnessV1::Present { .. }
            ));
        } else {
            assert!(matches!(
                witness,
                KagemushaWalletSuffixCreditWitnessV1::Inserted { .. }
            ));
        }
        let record = decode_record(witness);
        assert_eq!(record.verify(&before, &credit).unwrap(), draft.root());
        expected.record(&mut store, &credit).unwrap();
        assert_eq!(expected.root(), draft.root());
        assert_eq!(archive::encode(&selected).unwrap(), selected_original);
    }
    assert_eq!(
        draft.opening(&mut store, &first.credit_id).unwrap().0,
        first
    );
    let unchanged = archive::encode(&draft).unwrap();
    assert!(record_credit(&mut store, &mut draft, &first).is_err());
    assert_eq!(archive::encode(&draft).unwrap(), unchanged);
}

#[test]
fn retained_original_rejects_duplicate_empty_oversized_or_missing() {
    use KagemushaWalletRetainedInputRoleV1 as R;
    let input = KagemushaWalletRetainedInputV1 {
        role: R::Payment,
        bytes: vec![1, 2, 3],
    };
    assert_eq!(
        retained(std::slice::from_ref(&input), R::Payment).unwrap(),
        [1, 2, 3]
    );
    assert!(retained(&[input.clone(), input.clone()], R::Payment).is_err());
    assert!(retained(&[input], R::Credential).is_err());
    for bytes in [vec![], vec![0; KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 + 1]] {
        assert!(
            retained(
                &[KagemushaWalletRetainedInputV1 {
                    role: R::Payment,
                    bytes
                }],
                R::Payment
            )
            .is_err()
        );
    }
}

fn open_request() -> KagemushaWalletRequestV1 {
    let vectors: norito::json::Value = norito::json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/kagemusha/wallet_v1_vectors.json"
    )))
    .unwrap();
    let row = vectors["envelopes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["variant"].as_str() == Some("Request"))
        .unwrap();
    let envelope: KagemushaWalletEnvelopeV1 =
        archive::decode(&hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap()).unwrap();
    let KagemushaWalletMessageV1::Request { request } = envelope.message else {
        panic!("Request fixture");
    };
    request
}

#[test]
fn issued_request_requires_exact_local_original_and_preserves_read_refusal() {
    let request = open_request();
    let mut store = MemoryArchive::new();
    let index =
        session_custody::retain_request(&mut store, IndexRoot::default(), &request).unwrap();
    let scheme = request.body.scheme_id;
    let wallet = request.receiver_credential.body.wallet_id;
    let digest = request.request_digest();
    let (decoded, bytes) = issued_request(&mut store, &index, &scheme, &wallet, &digest).unwrap();
    assert_eq!(decoded, request);
    assert_eq!(bytes, archive::encode(&request).unwrap());
    assert!(issued_request(&mut store, &IndexRoot::default(), &scheme, &wallet, &digest).is_err());
    assert!(issued_request(&mut store, &index, &scheme, &[99; 32], &digest).is_err());
    store.hide_object_once(object_digest(&bytes));
    assert!(issued_request(&mut store, &index, &scheme, &wallet, &digest).is_err());
    assert_eq!(
        issued_request(&mut store, &index, &scheme, &wallet, &digest)
            .unwrap()
            .1,
        bytes
    );
}

#[test]
fn prior_charge_preserves_signed_original_and_refuses_mutated_terms() {
    let scheme = fixture("KagemushaWalletSchemeV1");
    let claim: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
    let capsule: KagemushaWalletRecoveryCapsuleV1 = fixture("KagemushaWalletRecoveryCapsuleV1");
    let mut before = capsule.body().successor_state;
    before.core.wallet_id = claim.credential.body.wallet_id;
    before.core.asset_digest = claim.credential.body.asset_digest;
    let originals = ChargeOriginalsV1 {
        quote: claim.charge.quote().unwrap().to_canonical_bytes().unwrap(),
        certificates: archive::encode(&claim.certificates).unwrap(),
    };
    let statement = claim.package.statement;
    let retained = checked_charge(&scheme, &before, &statement, Some(&originals))
        .unwrap()
        .unwrap();
    assert_eq!(retained.quote, originals.quote);
    assert_eq!(retained.certificate_set, originals.certificates);
    assert!(checked_charge(&scheme, &before, &statement, None).is_err());
    for mutate in 0..4 {
        let mut changed = statement;
        let KagemushaWalletEffectV1::Unload {
            amount,
            redeem_ordinal,
            online_charge,
            charge_quote,
            ..
        } = &mut changed.effect
        else {
            panic!("Unload");
        };
        match mutate {
            0 => *amount += 1,
            1 => *redeem_ordinal += 1,
            2 => *online_charge += 1,
            _ => charge_quote[0] ^= 1,
        }
        assert!(checked_charge(&scheme, &before, &changed, Some(&originals)).is_err());
    }
    let mut mutated = originals.clone();
    mutated.quote.push(0);
    assert!(checked_charge(&scheme, &before, &statement, Some(&mutated)).is_err());
    mutated = originals;
    mutated.certificates =
        archive::encode(&KagemushaWalletCertificateSetV1::new(vec![]).unwrap()).unwrap();
    assert!(checked_charge(&scheme, &before, &statement, Some(&mutated)).is_err());
}
