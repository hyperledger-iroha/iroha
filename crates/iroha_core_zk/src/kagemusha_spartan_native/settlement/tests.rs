//! The local ledger fixture signs synthetic result rows. This exercises both inclusion proofs
//! and actual consensus certificates, independently of the charged World execution test.
use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::AccountId,
    sumeragi_finality::test_fixtures::NativeFinalityFixture,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use std::{collections::BTreeMap, time::Duration};

#[derive(Clone, Default)]
struct Archive(BTreeMap<[u8; 32], Vec<u8>>);
impl state::ObjectStore for Archive {
    fn write_object(&mut self, bytes: &[u8], max: usize) -> Result<[u8; 32], state::Error> {
        assert!(bytes.len() <= max);
        let key =
            crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1(bytes);
        self.0.insert(key, bytes.to_vec());
        Ok(key)
    }
    fn read_object(&mut self, key: &[u8; 32], max: usize) -> Result<Vec<u8>, state::Error> {
        let bytes = self
            .0
            .get(key)
            .ok_or(state::Error::WitnessLost("test archive object"))?;
        assert!(bytes.len() <= max);
        Ok(bytes.clone())
    }
}

#[test]
fn certified_spartan_payout_survives_a_rejected_relay_and_restart() {
    certified_outcome_and_restart(false);
}

#[test]
fn certified_spartan_activation_survives_a_rejected_retry_and_restart() {
    certified_outcome_and_restart(true);
}

fn certified_outcome_and_restart(activation: bool) {
    let mut fixture = NativeFinalityFixture::start("future-settlement-custody");
    let base = SumeragiCommitVerifierV1::new(&fixture.verifier()).unwrap();
    let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let operation = [0x77; 32];
    let scheme = [3; 32];
    let asset = [4; 32];
    let manifest = [5; 32];
    let claim = b"test retained claim";
    let wallet = [0x43; 32];
    let account = AccountId::new(keypair.public_key().clone());
    let scope = ClaimScope {
        network: fixture.network_id(),
        scheme: &scheme,
        asset: &asset,
        manifest_digest: &manifest,
        claim,
        activation: activation.then_some((&wallet, &account)),
    };
    let instruction: iroha_data_model::isi::InstructionBox = if activation {
        Activate {
            scheme,
            asset,
            manifest_digest: manifest,
            wallet,
            activation: claim.to_vec(),
        }
        .into()
    } else {
        Unload {
            scheme,
            asset,
            manifest_digest: manifest,
            claim: claim.to_vec(),
        }
        .into()
    };
    let mut store = Archive::default();
    let mut root = IndexRoot([0; 32]);
    let mut entries = Vec::new();
    for (i, accepted) in [true, false].into_iter().enumerate() {
        let mut builder = TransactionBuilder::new(
            fixture.network_id(),
            AccountId::new(keypair.public_key().clone()),
            FeePaymentIntent::authority(vec![], None),
        );
        builder.set_creation_time(Duration::from_millis(
            fixture.next_header().creation_time_ms - 1,
        ));
        let signed = builder
            .with_instructions([instruction.clone()])
            .sign(keypair.private_key());
        let signed_wire = signed.encode_wire_v1().unwrap();
        let tx = scope.envelope(&signed_wire).unwrap();
        let wrong = ClaimScope {
            manifest_digest: &[8; 32],
            ..scope
        };
        assert!(wrong.envelope(&signed_wire).is_err());
        if activation {
            let stranger = AccountId::new(
                KeyPair::random_with_algorithm(Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            );
            assert!(
                ClaimScope {
                    activation: Some((&wallet, &stranger)),
                    ..scope
                }
                .envelope(&signed_wire)
                .is_err()
            );
            assert!(
                ClaimScope {
                    activation: Some((&[0x44; 32], &account)),
                    ..scope
                }
                .envelope(&signed_wire)
                .is_err()
            );
            assert!(
                ClaimScope {
                    activation: None,
                    ..scope
                }
                .envelope(&signed_wire)
                .is_err()
            );
        }
        let execution = if accepted {
            Ok(vec![])
        } else {
            state::settlement_fixture::rejected()
        };
        let block = state::settlement_fixture::certify(
            &mut fixture,
            std::slice::from_ref(&tx),
            vec![execution],
        );
        let original = state::settlement_fixture::original(&block, 0);
        let proof = original.to_canonical_bytes().unwrap();
        let verified = decode_model(&proof)
            .unwrap()
            .verify(&base, base.chain_id(), &tx)
            .unwrap();
        assert_eq!(verified.accepted(), accepted);
        if let Some((prior, _)) = entries.first() {
            assert!(original.verify(&base, base.chain_id(), prior).is_err());
        }
        root = scope
            .record(
                &mut store,
                root,
                &operation,
                &signed_wire,
                &proof,
                verified,
                i != 0,
            )
            .unwrap();
        entries.push((tx, verified));
    }
    let mut reopened = store.clone();
    let saved = archived_claim(&mut reopened, root, &operation).unwrap();
    if activation {
        assert!(saved.is_none());
        assert!(
            root.get(&mut reopened, &key(b"kgunpay1", &operation))
                .unwrap()
                .is_none()
        );
    } else {
        let (stored_asset, stored_claim) = saved.unwrap();
        assert_eq!(
            (stored_asset, stored_claim.as_slice()),
            (asset, claim.as_slice())
        );
    }
    for (tx, verdict) in entries {
        let row = retained_row(
            &mut reopened,
            root,
            &scope.attempt_key(tx.hash_as_entrypoint().as_ref()),
            &operation,
            &scope,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (row.height, row.block, row.accepted),
            (verdict.height(), verdict.commit_hash(), verdict.accepted())
        );
    }
    let paid = retained_row(
        &mut reopened,
        root,
        &scope.accepted_key(&operation),
        &operation,
        &scope,
    )
    .unwrap()
    .unwrap();
    assert!(paid.accepted);
    assert_eq!(paid.height, 2);
    reopened.0.remove(&paid.finality);
    assert!(
        retained_row(
            &mut reopened,
            root,
            &scope.accepted_key(&operation),
            &operation,
            &scope
        )
        .is_err()
    );
}
