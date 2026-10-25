//! Public signing preparation agrees across the native holder and the C boundary.

use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{NetworkId, account::AccountId, privacy::*};
use p256::ecdsa::{
    Signature, SigningKey,
    signature::{
        Signer as _, Verifier as _,
        hazmat::{PrehashSigner as _, PrehashVerifier as _},
    },
};

fn statement() -> IrohaZkX509StarkP256StatementV1 {
    let bundle = privacy_exact12_fixture_bundle_v1().expect("canonical model fixtures");
    let row = bundle
        .rows
        .into_iter()
        .find(|row| row.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
        .expect("X509 row");
    let PrivacyStatementV1::IrohaZkX509StarkP256V1(statement) =
        norito::decode_canonical(&row.statement_norito).expect("statement")
    else {
        panic!("X509 fixture")
    };
    statement
}
fn archive(statement: &IrohaZkX509StarkP256StatementV1) -> Vec<u8> {
    norito::encode_canonical(&PrivacyStatementV1::IrohaZkX509StarkP256V1(
        statement.clone(),
    ))
    .expect("canonical statement")
}
fn through_c(input: &[u8], expected: c_int) -> Option<Prepared> {
    let mut message = Output::default();
    let mut digest = Output::default();
    message.length = 99;
    digest.length = 99;
    let status = unsafe {
        connect_norito_zk_x509_prepare_ownership_signing_v1(
            input.as_ptr(),
            input.len() as c_ulong,
            &mut message.pointer,
            &mut message.length,
            &mut digest.pointer,
            &mut digest.length,
        )
    };
    assert_eq!(status, expected);
    if status != 0 {
        assert!(message.pointer.is_null());
        assert!(digest.pointer.is_null());
        assert_eq!((message.length, digest.length), (0, 0));
        return None;
    }
    assert!(!message.pointer.is_null());
    assert!(!digest.pointer.is_null());
    assert_eq!(digest.length, 32);
    assert!(message.length > 0 && message.length as usize <= MAX_STATEMENT_BYTES);
    Some(Prepared {
        message: unsafe { std::slice::from_raw_parts(message.pointer, message.length as usize) }
            .to_vec(),
        digest: unsafe { std::slice::from_raw_parts(digest.pointer, 32) }
            .try_into()
            .expect("digest"),
    })
}
fn low_s(signature: Signature) -> Signature {
    signature.normalize_s().unwrap_or(signature)
}

#[test]
fn c_preparation_matches_holder_and_real_p256_message_and_prehash_signers() {
    let original = statement();
    let prepared = through_c(&archive(&original), 0).expect("prepared");
    assert_eq!(
        prepared.message,
        holder::ownership_signing_message_v1(&original).unwrap()
    );
    assert_eq!(
        prepared.digest,
        holder::ownership_challenge_v1(&original).unwrap()
    );
    // A test signing key only; the production entrypoint never accepts one.
    let key = SigningKey::from_bytes((&[0x42; 32]).into()).unwrap();
    let message_signature = low_s(key.sign(&prepared.message));
    let prehash_signature = low_s(key.sign_prehash(&prepared.digest).unwrap());
    assert_eq!(message_signature.to_bytes(), prehash_signature.to_bytes());
    key.verifying_key()
        .verify(&prepared.message, &prehash_signature)
        .unwrap();
    key.verifying_key()
        .verify_prehash(&prepared.digest, &message_signature)
        .unwrap();
    let double_hash_signature = low_s(key.sign(&prepared.digest));
    assert!(
        key.verifying_key()
            .verify(&prepared.message, &double_hash_signature)
            .is_err()
    );
    assert!(
        key.verifying_key()
            .verify_prehash(&prepared.digest, &double_hash_signature)
            .is_err()
    );

    let mut variants = Vec::new();
    let mut changed = original.clone();
    changed.wallet_challenge = PrivacyChallengeV1::new([0x91; 32]);
    variants.push(changed);
    let mut changed = original.clone();
    changed.context.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([0x92; 32]);
    variants.push(changed);
    let mut changed = original.clone();
    let wallet = KeyPair::try_from_seed(vec![0x93; 32], Algorithm::Ed25519).unwrap();
    changed.wallet_account = AccountId::new(wallet.public_key().clone());
    variants.push(changed);
    let mut changed = original.clone();
    changed.context.network_id =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x94; 32])));
    variants.push(changed);
    let mut changed = original.clone();
    changed.certificate_policy_record_epoch += 1;
    variants.push(changed);
    let mut changed = original;
    changed.presentation_not_after_unix_seconds -= 1;
    variants.push(changed);
    for variant in variants {
        let rebound = through_c(&archive(&variant), 0).unwrap();
        assert_ne!(rebound.message, prepared.message);
        assert_ne!(rebound.digest, prepared.digest);
        assert_eq!(
            rebound.message,
            holder::ownership_signing_message_v1(&variant).unwrap()
        );
        assert_eq!(
            rebound.digest,
            holder::ownership_challenge_v1(&variant).unwrap()
        );
        assert!(
            key.verifying_key()
                .verify(&rebound.message, &message_signature)
                .is_err()
        );
        assert!(
            key.verifying_key()
                .verify_prehash(&rebound.digest, &message_signature)
                .is_err()
        );
    }
}

#[test]
fn canonical_input_admission_and_failure_outputs_are_bounded() {
    let original = archive(&statement());
    through_c(&[], EMPTY);
    through_c(&vec![0; MAX_STATEMENT_BYTES + 1], TOO_LARGE);
    through_c(&vec![0; MAX_STATEMENT_BYTES], NON_CANONICAL);
    through_c(&original[..original.len() - 1], NON_CANONICAL);
    let mut trailing = original.clone();
    trailing.push(0);
    through_c(&trailing, NON_CANONICAL);
    let mut changed = original.clone();
    *changed.last_mut().unwrap() ^= 1;
    through_c(&changed, NON_CANONICAL);
    let bundle = privacy_exact12_fixture_bundle_v1().unwrap();
    for row in bundle.rows {
        if row.protocol_id != PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 {
            through_c(&row.statement_norito, WRONG_PROTOCOL);
        }
    }
    // A correctly encoded different root schema cannot enter the statement path.
    through_c(&norito::encode_canonical(&42_u64).unwrap(), NON_CANONICAL);
}

#[test]
fn null_and_oversize_c_admission_clear_outputs_before_reading_input() {
    let mut message = Output::default();
    let mut digest = Output::default();
    let status = unsafe {
        connect_norito_zk_x509_prepare_ownership_signing_v1(
            ptr::null(),
            1,
            &mut message.pointer,
            &mut message.length,
            &mut digest.pointer,
            &mut digest.length,
        )
    };
    assert_eq!(status, NULL);
    assert!(message.pointer.is_null() && digest.pointer.is_null());
    assert_eq!((message.length, digest.length), (0, 0));
    let byte = 0_u8;
    let status = unsafe {
        connect_norito_zk_x509_prepare_ownership_signing_v1(
            &byte,
            (MAX_STATEMENT_BYTES + 1) as c_ulong,
            &mut message.pointer,
            &mut message.length,
            &mut digest.pointer,
            &mut digest.length,
        )
    };
    assert_eq!(status, TOO_LARGE);
    assert!(message.pointer.is_null() && digest.pointer.is_null());
    assert_eq!((message.length, digest.length), (0, 0));
    let status = unsafe {
        connect_norito_zk_x509_prepare_ownership_signing_v1(
            &byte,
            1,
            ptr::null_mut(),
            &mut message.length,
            &mut digest.pointer,
            &mut digest.length,
        )
    };
    assert_eq!(status, NULL);
    assert!(digest.pointer.is_null());
    assert_eq!((message.length, digest.length), (0, 0));
}

#[test]
fn failure_resets_prepopulated_c_output_slots() {
    // Sentinels are never dereferenced; failed preparation must overwrite them.
    let mut message = ptr::dangling_mut::<u8>();
    let mut digest = ptr::dangling_mut::<u8>();
    let mut message_len = 17;
    let mut digest_len = 32;
    let input = [0_u8];
    let status = unsafe {
        connect_norito_zk_x509_prepare_ownership_signing_v1(
            input.as_ptr(),
            1,
            &mut message,
            &mut message_len,
            &mut digest,
            &mut digest_len,
        )
    };
    assert_eq!(status, NON_CANONICAL);
    assert!(message.is_null() && digest.is_null());
    assert_eq!((message_len, digest_len), (0, 0));
}
