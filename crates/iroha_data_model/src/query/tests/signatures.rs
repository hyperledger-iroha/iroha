use std::num::NonZeroU64;

use super::*;

#[test]
fn query_signature_decode_from_slice_roundtrip() {
    let key_pair =
        iroha_crypto::KeyPair::try_from_seed(vec![0x51; 32], iroha_crypto::Algorithm::Ed25519)
            .expect("generate checked query signature fixture keypair");
    let public_key: PublicKey = key_pair.public_key().clone();
    let authority = AccountId::new(public_key);

    let cursor = ForwardCursor {
        query: "cursor-1".to_owned(),
        cursor: NonZeroU64::new(1).expect("nonzero"),
        gas_budget: Some(5),
    };
    let payload = QueryRequestWithAuthority {
        authority: authority.clone(),
        request: QueryRequest::Continue(cursor),
    };

    let signature = iroha_crypto::SignatureOf::try_new(key_pair.private_key(), &payload)
        .expect("checked query signature fixture");
    signature
        .verify(key_pair.public_key(), &payload)
        .expect("checked query signature fixture verifies");
    let query_signature = QuerySignature(signature.clone());

    let encoded = norito::to_bytes(&query_signature).expect("encode query signature");
    let decoded: QuerySignature =
        norito::core::decode_from_bytes(&encoded).expect("decode query signature");
    assert_eq!(decoded, query_signature);

    let inner_encoded = norito::to_bytes(&signature).expect("encode inner signature");
    let inner_decoded: iroha_crypto::SignatureOf<QueryRequestWithAuthority> =
        norito::core::decode_from_bytes(&inner_encoded).expect("decode inner signature");
    assert_eq!(inner_decoded, signature);
}

#[test]
fn query_signature_try_deserialize_rejects_empty_signature_material() {
    let query_signature = QuerySignature(iroha_crypto::SignatureOf::from_signature(
        iroha_crypto::Signature::from_bytes(&[]),
    ));
    let encoded = norito::to_bytes(&query_signature).expect("encode invalid query signature");
    let archived =
        norito::from_bytes::<QuerySignature>(&encoded).expect("archive invalid query signature");

    let err = <QuerySignature as norito::core::NoritoDeserialize<'_>>::try_deserialize(archived)
        .expect_err("empty query signature must fail closed");
    let message = err.to_string();
    assert!(
        message.contains("empty"),
        "unexpected query signature decode error: {message}"
    );
}

#[test]
fn query_signature_try_deserialize_rejects_all_zero_signature_material() {
    let query_signature = QuerySignature(iroha_crypto::SignatureOf::from_signature(
        iroha_crypto::Signature::from_bytes(&[0_u8; 64]),
    ));
    let encoded = norito::to_bytes(&query_signature).expect("encode invalid query signature");
    let archived =
        norito::from_bytes::<QuerySignature>(&encoded).expect("archive invalid query signature");

    let err = <QuerySignature as norito::core::NoritoDeserialize<'_>>::try_deserialize(archived)
        .expect_err("all-zero query signature must fail closed");
    let message = err.to_string();
    assert!(
        message.contains("all zero"),
        "unexpected query signature decode error: {message}"
    );
}

#[test]
fn query_request_try_sign_matches_compatibility_sign() {
    let key_pair =
        iroha_crypto::KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519)
            .expect("generate checked query signing fixture keypair");
    let make_payload = || {
        let authority = AccountId::new(key_pair.public_key().clone());
        let cursor = ForwardCursor {
            query: "cursor-try-sign".to_owned(),
            cursor: NonZeroU64::new(1).expect("nonzero"),
            gas_budget: Some(5),
        };
        QueryRequestWithAuthority {
            authority,
            request: QueryRequest::Continue(cursor),
        }
    };

    let fallible = make_payload()
        .try_sign(&key_pair)
        .expect("query signing should succeed");
    let compatibility = make_payload().sign(&key_pair);

    let fallible_bytes = norito::to_bytes(&fallible).expect("encode fallible signed query");
    let compatibility_bytes =
        norito::to_bytes(&compatibility).expect("encode compatibility signed query");
    assert_eq!(fallible_bytes, compatibility_bytes);

    let QuerySignature(signature) = &fallible.signature;
    signature
        .verify(key_pair.public_key(), &fallible.payload)
        .expect("query signature should verify");
}

#[cfg(feature = "json")]
#[test]
fn query_signature_json_rejects_empty_signature_material() {
    let encoded = json_wrappers::base64_encode(&[]);
    let err = norito::json::from_value::<QuerySignature>(norito::json::Value::from(encoded))
        .expect_err("empty query signature JSON must fail closed");
    let message = err.to_string();

    assert!(
        message.contains("QuerySignature"),
        "unexpected query signature JSON error: {message}"
    );
    assert!(
        message.contains("empty"),
        "unexpected empty query signature JSON error: {message}"
    );
}

#[cfg(feature = "json")]
#[test]
fn query_signature_json_rejects_all_zero_signature_material() {
    let encoded = json_wrappers::base64_encode(&[0_u8; 64]);
    let err = norito::json::from_value::<QuerySignature>(norito::json::Value::from(encoded))
        .expect_err("all-zero query signature JSON must fail closed");
    let message = err.to_string();

    assert!(
        message.contains("QuerySignature"),
        "unexpected query signature JSON error: {message}"
    );
    assert!(
        message.contains("all zero"),
        "unexpected all-zero query signature JSON error: {message}"
    );
}
