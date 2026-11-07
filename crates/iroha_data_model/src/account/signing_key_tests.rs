//! Off-chain signing envelope, record and vector tests.
use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture key")
}

fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])))
}

fn context<'a>(
    purpose: OffchainSignaturePurposeV1,
    network_id: &'a NetworkId,
    account: &'a AccountId,
    generation: u32,
    domain_tag: &'a str,
) -> OffchainSigningContextV1<'a> {
    OffchainSigningContextV1 {
        purpose,
        network_id,
        account,
        generation,
        domain_tag,
    }
}

#[test]
fn message_has_the_documented_layout() {
    let account = AccountId::new(key(1).public_key().clone());
    let network_id = network(7);
    let message = offchain_signing_message_v1(
        OffchainSignaturePurposeV1::ConnectRaw,
        &network_id,
        &account,
        3,
        "tag",
        b"hello",
    )
    .unwrap();
    let address = AccountAddress::from_account_id(&account)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let mut expected = OFFCHAIN_SIGNING_DOMAIN_V1.to_vec();
    expected.push(1);
    expected.extend_from_slice(network_id.as_bytes());
    expected.extend_from_slice(&u32::try_from(address.len()).unwrap().to_le_bytes());
    expected.extend_from_slice(&address);
    expected.extend_from_slice(&3_u32.to_le_bytes());
    expected.extend_from_slice(&3_u32.to_le_bytes());
    expected.extend_from_slice(b"tag");
    expected.extend_from_slice(&5_u64.to_le_bytes());
    expected.extend_from_slice(b"hello");
    assert_eq!(message, expected);
}

#[test]
fn message_is_never_a_32_byte_hash() {
    let account = AccountId::new(key(1).public_key().clone());
    let network_id = network(7);
    let message = offchain_signing_message_v1(
        OffchainSignaturePurposeV1::ConnectRaw,
        &network_id,
        &account,
        0,
        "t",
        &[],
    )
    .unwrap();
    // The shortest envelope is already longer than a transaction or query hash.
    assert_ne!(message.len(), Hash::LENGTH);
    assert!(message.starts_with(OFFCHAIN_SIGNING_DOMAIN_V1));
    assert!(!message.is_empty() && message.len() != Hash::LENGTH);
}

#[test]
fn every_field_changes_the_message() {
    let account = AccountId::new(key(1).public_key().clone());
    let other_account = AccountId::new(key(2).public_key().clone());
    let network_id = network(7);
    let other_network = network(8);
    let base = context(
        OffchainSignaturePurposeV1::ConnectRaw,
        &network_id,
        &account,
        1,
        "tag",
    );
    let reference = base.message(b"bytes").unwrap();
    let variants = [
        context(
            OffchainSignaturePurposeV1::ToriiCanonicalRequest,
            &network_id,
            &account,
            1,
            "tag",
        ),
        context(
            OffchainSignaturePurposeV1::ConnectRaw,
            &other_network,
            &account,
            1,
            "tag",
        ),
        context(
            OffchainSignaturePurposeV1::ConnectRaw,
            &network_id,
            &other_account,
            1,
            "tag",
        ),
        context(
            OffchainSignaturePurposeV1::ConnectRaw,
            &network_id,
            &account,
            2,
            "tag",
        ),
        context(
            OffchainSignaturePurposeV1::ConnectRaw,
            &network_id,
            &account,
            1,
            "tah",
        ),
    ];
    for variant in variants {
        assert_ne!(variant.message(b"bytes").unwrap(), reference);
    }
    assert_ne!(base.message(b"bytez").unwrap(), reference);
    // Framing keeps tag and bytes apart: moving a byte between them changes the message.
    let shifted = context(
        OffchainSignaturePurposeV1::ConnectRaw,
        &network_id,
        &account,
        1,
        "tagb",
    );
    assert_ne!(shifted.message(b"ytes").unwrap(), reference);
}

#[test]
fn empty_domain_tag_is_refused() {
    let account = AccountId::new(key(1).public_key().clone());
    let network_id = network(7);
    assert_eq!(
        offchain_signing_message_v1(
            OffchainSignaturePurposeV1::ConnectRaw,
            &network_id,
            &account,
            1,
            "",
            b"x",
        ),
        Err(OffchainSigningError::EmptyDomainTag)
    );
}

#[test]
fn signature_verifies_only_in_its_own_context_and_key() {
    let transaction_key = key(1);
    let signing_key = key(2);
    let account = AccountId::new(transaction_key.public_key().clone());
    let network_id = network(7);
    let signer = context(
        OffchainSignaturePurposeV1::ConnectRaw,
        &network_id,
        &account,
        1,
        "tag",
    );
    let signature = signer.sign(signing_key.private_key(), b"payload").unwrap();
    signer
        .verify(signing_key.public_key(), &signature, b"payload")
        .unwrap();
    assert_eq!(
        signer.verify(transaction_key.public_key(), &signature, b"payload"),
        Err(OffchainSigningError::Verification),
        "the account's transaction key does not verify its signing key's signature"
    );
    assert_eq!(
        signer.verify(signing_key.public_key(), &signature, b"other"),
        Err(OffchainSigningError::Verification)
    );
    let torii = context(
        OffchainSignaturePurposeV1::ToriiCanonicalRequest,
        &network_id,
        &account,
        1,
        "tag",
    );
    assert_eq!(
        torii.verify(signing_key.public_key(), &signature, b"payload"),
        Err(OffchainSigningError::Verification),
        "a Connect raw signature is not a Torii canonical-request signature"
    );
    let rotated = context(
        OffchainSignaturePurposeV1::ConnectRaw,
        &network_id,
        &account,
        2,
        "tag",
    );
    assert_eq!(
        rotated.verify(signing_key.public_key(), &signature, b"payload"),
        Err(OffchainSigningError::Verification),
        "a signature of an older key generation does not verify"
    );
}

#[test]
fn a_signed_transaction_hash_is_not_an_offchain_signature() {
    let signing_key = key(2);
    let account = AccountId::new(key(1).public_key().clone());
    let network_id = network(7);
    let hash = Hash::new(b"transaction payload");
    // A key that signs the bare hash (the oracle this design removes) does not produce a
    // signature the envelope accepts, and the envelope's signature is not over the hash.
    let bare = Signature::new(signing_key.private_key(), hash.as_ref());
    let signer = context(
        OffchainSignaturePurposeV1::ConnectRaw,
        &network_id,
        &account,
        1,
        "tag",
    );
    assert_eq!(
        signer.verify(signing_key.public_key(), &bare, hash.as_ref()),
        Err(OffchainSigningError::Verification)
    );
    let enveloped = signer
        .sign(signing_key.private_key(), hash.as_ref())
        .unwrap();
    assert!(
        enveloped
            .verify(signing_key.public_key(), hash.as_ref())
            .is_err()
    );
}

#[test]
fn connect_raw_purpose_follows_the_tag() {
    assert_eq!(
        OffchainSignaturePurposeV1::for_connect_raw_tag(TORII_CANONICAL_REQUEST_DOMAIN_TAG_V1),
        OffchainSignaturePurposeV1::ToriiCanonicalRequest
    );
    assert_eq!(
        OffchainSignaturePurposeV1::for_connect_raw_tag("iroha-connect/v1/login"),
        OffchainSignaturePurposeV1::ConnectRaw
    );
    assert_eq!(OffchainSignaturePurposeV1::ConnectRaw.code(), 1);
    assert_eq!(OffchainSignaturePurposeV1::ToriiCanonicalRequest.code(), 2);
}

#[test]
fn record_reports_only_an_active_key() {
    let public_key = key(3).public_key().clone();
    let active = AccountSigningKeyRecordV1 {
        generation: 4,
        public_key: Some(public_key.clone()),
    };
    assert_eq!(
        active.active(),
        Some(AccountSigningKeyV1 {
            public_key,
            generation: 4
        })
    );
    let revoked = AccountSigningKeyRecordV1 {
        generation: 5,
        public_key: None,
    };
    assert_eq!(revoked.active(), None);
}

#[derive(crate::DeriveJsonDeserialize)]
struct EnvelopeVectors {
    cases: Vec<EnvelopeVector>,
}

#[derive(crate::DeriveJsonDeserialize)]
struct EnvelopeVector {
    case_id: String,
    purpose: u8,
    network_id_hex: String,
    account_canonical_hex: String,
    account_public_key_multihash: String,
    generation: u32,
    domain_tag: String,
    bytes_hex: String,
    message_hex: String,
}

/// The vectors the JavaScript SDK checks too, computed independently from the layout.
#[test]
fn message_matches_the_shared_cross_sdk_vectors() {
    let vectors: EnvelopeVectors = norito::json::from_str(include_str!(
        "../../../../fixtures/account/offchain_signing_vectors.json"
    ))
    .expect("off-chain signing vectors");
    assert!(!vectors.cases.is_empty());
    for case in vectors.cases {
        let public_key: PublicKey = case
            .account_public_key_multihash
            .parse()
            .expect("vector public key");
        let account = AccountId::new(public_key);
        assert_eq!(
            AccountAddress::from_account_id(&account)
                .unwrap()
                .canonical_hex()
                .unwrap(),
            case.account_canonical_hex,
            "{}",
            case.case_id
        );
        let network: [u8; 32] = hex::decode(&case.network_id_hex)
            .unwrap()
            .try_into()
            .unwrap();
        let network_id =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(network)));
        let purpose = match case.purpose {
            1 => OffchainSignaturePurposeV1::ConnectRaw,
            2 => OffchainSignaturePurposeV1::ToriiCanonicalRequest,
            other => panic!("unknown purpose {other}"),
        };
        let message = offchain_signing_message_v1(
            purpose,
            &network_id,
            &account,
            case.generation,
            &case.domain_tag,
            &hex::decode(&case.bytes_hex).unwrap(),
        )
        .unwrap();
        assert_eq!(hex::encode(message), case.message_hex, "{}", case.case_id);
    }
}

#[test]
fn records_roundtrip_through_norito() {
    let record = AccountSigningKeyRecordV1 {
        generation: 9,
        public_key: Some(key(4).public_key().clone()),
    };
    let bytes = norito::encode_canonical(&record).unwrap();
    assert_eq!(
        norito::decode_canonical::<AccountSigningKeyRecordV1>(&bytes).unwrap(),
        record
    );
    let reservation = AccountSigningKeyReservationV1 {
        account: AccountId::new(key(5).public_key().clone()),
        generation: 2,
        active: false,
    };
    let bytes = norito::encode_canonical(&reservation).unwrap();
    assert_eq!(
        norito::decode_canonical::<AccountSigningKeyReservationV1>(&bytes).unwrap(),
        reservation
    );
}
