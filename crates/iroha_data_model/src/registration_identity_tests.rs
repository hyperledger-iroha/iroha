//! Captured registration/block identities and populated canonical encoding contracts.

use std::time::Duration;

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
use iroha_primitives::json::Json;
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, codec::Encode as _};

use crate::{
    account::{Account, AccountId, NewAccount, OpaqueAccountId, rekey},
    block::SignedBlock,
    domain::{Domain, DomainId, NewDomain},
    isi::Log,
    metadata::Metadata,
    nexus::{DataSpaceId, UniversalAccountId},
    transaction::{DataTriggerSequence, FeePaymentIntent, TransactionBuilder},
};

fn assert_identity<T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>>(
    nominal: &str,
    captured: &str,
) {
    let expected: [u8; 16] = hex::decode(captured)
        .expect("captured schema hash")
        .try_into()
        .expect("16-byte captured hash");
    assert_eq!(T::nominal_name(), nominal);
    assert_eq!(T::frame_name(), nominal);
    assert_eq!(norito::schema::identity::frame_hash::<T>(), expected);
    assert_eq!(norito::schema::identity::frame_hash::<T>(), expected);
    assert_eq!(norito::schema::identity::frame_hash::<T>(), expected);
}

#[test]
fn registration_and_block_identities_match_observed_hashes() {
    // Controlled compiler capture: be82d3661d9e2a79fd1a60d6922f1387d3821a0ad5a65d80d294aca1251caedd.
    assert_identity::<NewAccount>(
        "iroha_data_model::account::model::NewAccount",
        "b6e3317c1ea5553ee3a3e0d3b0589f45",
    );
    assert_identity::<NewDomain>(
        "iroha_data_model::domain::model::NewDomain",
        "8c722cf7c6d93bac82c539f64320e58e",
    );
    assert_identity::<SignedBlock>(
        "iroha_data_model::block::model::SignedBlock",
        "b19667a93e1767b82366f5d275d9fa41",
    );
}

fn assert_record<T>(value: &T)
where
    T: NoritoSchema
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + norito::json::JsonSerialize
        + norito::json::JsonDeserialize,
{
    let bare = value.encode();
    let frame = norito::encode_canonical(value).expect("encode canonical identity frame");
    let decoded: T = norito::decode_canonical(&frame).expect("decode canonical identity frame");
    // IdEqOrdHash intentionally ignores entity metadata, so compare every encoded field.
    assert_eq!(decoded.encode(), bare);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
    let header = norito::core::Header::read(frame.as_slice()).expect("read identity header");
    assert_eq!(header.schema, norito::schema::identity::frame_hash::<T>());
    let text = norito::json::to_json(value).expect("encode registration/block JSON");
    let decoded: T = norito::json::from_json(&text).expect("decode registration/block JSON");
    assert_eq!(decoded.encode(), bare);
    assert_eq!(norito::json::to_json(&decoded).unwrap(), text);
    let mut wrong_schema = frame.clone();
    wrong_schema[6] ^= 1;
    assert!(norito::decode_canonical::<T>(&wrong_schema).is_err());
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    let mut trailing = frame;
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
}

fn assert_family<T>(value: T, signer: &KeyPair)
where
    T: Clone
        + NoritoSchema
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + norito::json::JsonSerialize
        + norito::json::JsonDeserialize,
{
    let hash = HashOf::new(&value);
    let signature = SignatureOf::try_from_hash(signer.private_key(), hash)
        .expect("sign public deterministic fixture");
    signature
        .verify(signer.public_key(), &value)
        .expect("verify fixture");
    assert_record(&value);
    assert_record(&None::<T>);
    assert_record(&Some(value.clone()));
    assert_record(&vec![value.clone(), value]);
    assert_record(&hash);
    assert_record(&signature);
}

fn signer() -> KeyPair {
    KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
        .expect("derive public deterministic fixture signer")
}

fn metadata() -> Metadata {
    let mut value = Metadata::default();
    value.insert(
        "capture".parse().expect("metadata key"),
        Json::new(norito::json!({"message": "雪", "count": 7, "enabled": true})),
    );
    value
}

fn domain() -> NewDomain {
    Domain::new(DomainId::try_new("wonderland", "sora").expect("qualified domain"))
        .with_logo(
            "sorafs://bafybeigdyrztk/logo.png"
                .parse()
                .expect("valid logo"),
        )
        .with_metadata(metadata())
}

#[test]
fn registration_builders_preserve_populated_codec_and_json_values() {
    let signer = signer();
    let id = AccountId::new(signer.public_key().clone());
    let label = rekey::AccountAlias::new(
        "alice".parse().expect("account label"),
        Some(rekey::AccountAliasDomain::new(
            "wonderland".parse().expect("alias domain"),
        )),
        DataSpaceId::UNIVERSAL,
    );
    let account = Account::new(id)
        .with_metadata(metadata())
        .with_label(Some(label))
        .with_uaid(Some(UniversalAccountId::from_hash(Hash::prehashed(
            [0xAB; 32],
        ))))
        .with_opaque_ids(vec![OpaqueAccountId::from_hash(Hash::prehashed(
            [0xCD; 32],
        ))]);
    assert_family(account, &signer);
    assert_family(domain(), &signer);
}

#[test]
fn signed_block_identity_preserves_canonical_envelope() {
    let signer = signer();
    let authority = AccountId::new(signer.public_key().clone());
    let mut transaction =
        TransactionBuilder::new_genesis(authority, FeePaymentIntent::authority(Vec::new(), None))
            .with_instructions([Log {
                level: crate::Level::INFO,
                msg: "registration identity 雪".to_owned(),
            }]);
    transaction.set_creation_time(Duration::from_millis(1_700_000_000_007));
    let transaction = transaction
        .try_sign(signer.private_key())
        .expect("sign fixture transaction");
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let mut block = SignedBlock::try_genesis(vec![transaction], signer.private_key(), None, None)
        .expect("signed genesis proposal");
    assert!(!block.has_results());
    assert!(block.is_resultless_proposal());
    let proposal_hash = block.hash();
    let proposal_wire = block.encode_wire().expect("canonical resultless proposal");
    block
        .set_transaction_results(
            Vec::new(),
            &[entrypoint_hash],
            vec![Ok(DataTriggerSequence::default())],
        )
        .expect("attach the transaction's successful execution result");
    assert!(block.has_results());
    assert!(!block.is_resultless_proposal());
    assert_eq!(block.hash(), proposal_hash);
    assert_eq!(
        block.canonical_resultless_proposal().encode_wire().unwrap(),
        proposal_wire,
    );
    block
        .validate_entrypoint_merkle_cache()
        .expect("entrypoint commitment");
    block
        .validate_result_merkle_cache()
        .expect("result commitment");
    let signatures: Vec<_> = block.signatures().collect();
    assert_eq!(signatures.len(), 1);
    signatures[0]
        .signature()
        .verify_hash(signer.public_key(), block.hash())
        .expect("verify actual block signature");
    let wire = block.canonical_wire().expect("canonical block envelope");
    let decoded = crate::block::decode_framed_signed_block(wire.as_framed())
        .expect("decode canonical block envelope");
    assert_eq!(decoded.encode_wire().unwrap(), wire.as_framed());
    assert_eq!(decoded.hash(), block.hash());
    assert_family(block, &signer);
}
