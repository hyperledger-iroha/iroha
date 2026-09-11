//! Immutable generic model frames, argument identities and marker isolation.

use std::{collections::BTreeMap, num::NonZeroU64, time::Duration};

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_primitives::{json::Json, numeric::NumericSpec};
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json::Value};

use crate::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    block::BlockHeader,
    events::data::prelude::MetadataChanged,
    id::NetworkId,
    isi::{InstructionBox, Log, error::Mismatch},
    nft::NftId,
    query::{AnyQueryBox, SingularQueryBox, executor::prelude::FindExecutorDataModel},
    rwa::RwaId,
    smart_contract::payloads::{ExecutorContext, Validate},
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionDomain},
    trigger::TriggerId,
};
use iroha_model_base::domain::DomainId;

/// Check the declared serializer identity and reproduce its immutable frame record.
pub fn encoded_record<T: NoritoSchema + NoritoSerialize>(value: &T) -> Value {
    let bare = norito::codec::encode_adaptive(value);
    let frame = norito::encode_canonical(value).expect("encode complete identity capture frame");
    let expected_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(
        norito::core::Header::read(frame.as_slice()).unwrap().schema,
        expected_hash
    );
    assert_eq!(T::frame_name(), T::nominal_name());
    norito::json!({
        "actual_type_name": (T::nominal_name()),
        "serialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "bare_hex": (hex::encode(bare)),
        "frame_hex": (hex::encode(frame)),
    })
}

fn record<T>(value: &T) -> Value
where
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let frame = norito::encode_canonical(value).expect("encode generic identity frame");
    let decoded: T = norito::decode_from_bytes(&frame).expect("decode generic identity frame");
    assert_eq!(
        norito::encode_canonical(&decoded).expect("re-encode generic identity frame"),
        frame,
    );
    let mut wrong_schema = frame.clone();
    wrong_schema[6] ^= 1;
    assert!(matches!(
        norito::decode_from_bytes::<T>(&wrong_schema),
        Err(norito::core::Error::SchemaMismatch)
    ));
    for end in [0, norito::core::Header::SIZE - 1, frame.len() - 1] {
        assert!(norito::decode_from_bytes::<T>(&frame[..end]).is_err());
    }
    norito::json!({
        "encoding": (encoded_record(value)),
        "deserialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "decode_reencode_exact": true,
    })
}

fn family<T>(label: &str, make: impl Fn() -> T) -> Value
where
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    norito::json!({
        "case": label,
        "root": (record(&make())),
        "vec": (record(&vec![make(), make()])),
        "option": (record(&Some(make()))),
        "map": (record(&BTreeMap::from([(7_u32, make()), (31, make())]))),
    })
}

fn key_pair() -> KeyPair {
    KeyPair::try_from_seed(vec![0x39; 32], Algorithm::Ed25519)
        .expect("deterministic identity fixture key")
}

fn account() -> AccountId {
    AccountId::new(key_pair().public_key().clone())
}

fn domain() -> DomainId {
    DomainId::try_new("market", "universal").expect("valid identity fixture domain")
}

fn asset_definition() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(domain(), "rose".parse().expect("asset name"))
}

fn metadata<Id>(target: Id) -> MetadataChanged<Id> {
    MetadataChanged {
        target,
        key: "label".parse().expect("metadata name"),
        value: Json::new(norito::json!({"text": "雪", "version": 7, "active": true})),
    }
}

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"model generic identity fixture genesis",
    )))
}

fn context() -> ExecutorContext {
    ExecutorContext {
        authority: account(),
        curr_block: BlockHeader {
            height: NonZeroU64::new(1).expect("nonzero fixture block height"),
            prev_block_hash: None,
            merkle_root: None,
            result_merkle_root: None,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            npos_effects_hash: None,
            execution_context_hash: None,
            sccp_commitment_root: None,
            creation_time_ms: 1_700_000_000_007,
            view_change_index: 0,
            confidential_features: None,
        },
    }
}

fn instruction() -> InstructionBox {
    Log {
        level: crate::Level::INFO,
        msg: "identity fixture 雪".to_owned(),
    }
    .into()
}

fn signed_transaction() -> SignedTransaction {
    let mut builder = TransactionBuilder::new(
        network(),
        account(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction()]);
    builder.set_creation_time(Duration::from_millis(1_700_000_000_007));
    builder
        .try_sign(key_pair().private_key())
        .expect("sign deterministic identity fixture")
}

#[test]
fn public_generic_identity_frames_match_capture() {
    let rows = vec![
        family("metadata-account", || metadata(account())),
        family("metadata-domain", || metadata(domain())),
        family("metadata-asset-definition", || metadata(asset_definition())),
        family("metadata-asset", || {
            metadata(AssetId::new(asset_definition(), account()))
        }),
        family("metadata-nft", || {
            metadata(NftId::new(domain(), "art".parse().expect("nft name")))
        }),
        family("metadata-rwa", || {
            metadata(RwaId::generated(
                domain(),
                Hash::new(b"identity fixture RWA"),
            ))
        }),
        family("metadata-trigger", || {
            metadata("identity_trigger".parse::<TriggerId>().expect("trigger id"))
        }),
        family("validate-instruction", || Validate {
            context: context(),
            target: instruction(),
        }),
        family("validate-query", || Validate {
            context: context(),
            target: AnyQueryBox::Singular(SingularQueryBox::FindExecutorDataModel(
                FindExecutorDataModel,
            )),
        }),
        family("validate-transaction", || Validate {
            context: context(),
            target: signed_transaction(),
        }),
        family("mismatch-numeric-spec", || Mismatch {
            expected: NumericSpec::integer(),
            actual: NumericSpec::try_fractional(4).expect("bounded numeric scale"),
        }),
        family("mismatch-transaction-domain", || Mismatch {
            expected: TransactionDomain::Network(network()),
            actual: TransactionDomain::Genesis,
        }),
    ];
    assert_eq!(rows.len(), 12);
    let argument_families = vec![
        family("rwa-id", || {
            RwaId::generated(domain(), Hash::new(b"identity fixture RWA"))
        }),
        family("signed-transaction", signed_transaction),
        family("any-query", || {
            AnyQueryBox::Singular(SingularQueryBox::FindExecutorDataModel(
                FindExecutorDataModel,
            ))
        }),
        family("network-domain", || TransactionDomain::Network(network())),
        family("genesis-domain", || TransactionDomain::Genesis),
    ];
    assert_eq!(argument_families.len(), 5);
    let expected: Value = norito::json::from_str(include_str!(
        "../tests/fixtures/model_generic_identity_frames.json"
    ))
    .expect("read immutable generic identity frames");
    assert_eq!(
        norito::json!({"families": rows, "argument_families": argument_families}),
        expected
    );
}

#[derive(Debug, norito::NoritoSchema)]
#[norito_schema(name = "identity_fixture::MarkerA")]
struct MarkerA;

#[test]
fn generic_identity_arguments_preserve_root_projections() {
    let trigger = "identity_trigger".parse::<TriggerId>().unwrap();
    assert_eq!(
        TriggerId::nominal_name(),
        "iroha_data_model::trigger::model::model::TriggerId",
    );
    record(&trigger);

    assert_eq!(
        InstructionBox::nominal_name(),
        "iroha_data_model::isi::InstructionBox",
    );
    assert_eq!(
        InstructionBox::frame_name(),
        <(String, Vec<u8>)>::frame_name(),
    );
    assert_ne!(InstructionBox::frame_name(), InstructionBox::nominal_name());
    let expected_hash = norito::schema::identity::frame_hash::<InstructionBox>();
    assert_eq!(
        norito::schema::identity::frame_hash::<InstructionBox>(),
        expected_hash
    );
    assert_eq!(
        norito::schema::identity::frame_hash::<InstructionBox>(),
        expected_hash
    );
    let frame = norito::encode_canonical(&instruction()).unwrap();
    assert_eq!(
        norito::core::Header::read(frame.as_slice()).unwrap().schema,
        expected_hash
    );
    let decoded: InstructionBox = norito::decode_from_bytes(&frame).unwrap();
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
    let mut wrong_schema = frame.clone();
    wrong_schema[6] ^= 1;
    assert!(matches!(
        norito::decode_from_bytes::<InstructionBox>(&wrong_schema),
        Err(norito::core::Error::SchemaMismatch)
    ));
    for end in [0, norito::core::Header::SIZE - 1, frame.len() - 1] {
        assert!(norito::decode_from_bytes::<InstructionBox>(&frame[..end]).is_err());
    }
}

#[derive(Debug, norito::NoritoSchema)]
#[norito_schema(name = "identity_fixture::MarkerB")]
struct MarkerB;

fn assert_marker_identity<T: core::fmt::Debug + NoritoSchema>() {
    for (constructor, name) in [
        (
            "iroha_data_model::events::data::events::model::MetadataChanged",
            MetadataChanged::<T>::nominal_name(),
        ),
        (
            "iroha_data_model::smart_contract::payloads::Validate",
            Validate::<T>::nominal_name(),
        ),
        (
            "iroha_data_model::isi::error::model::Mismatch",
            Mismatch::<T>::nominal_name(),
        ),
    ] {
        assert_eq!(name, format!("{constructor}<{}>", T::nominal_name()));
    }
}

#[test]
fn generic_identity_markers_do_not_require_payload_codecs() {
    assert_marker_identity::<MarkerA>();
    assert_marker_identity::<MarkerB>();
    assert_ne!(
        Vec::<Validate<MarkerA>>::nominal_name(),
        Vec::<Validate<MarkerB>>::nominal_name(),
    );
}

fn reject_other_argument<T, U>(left: &T, right: &U)
where
    T: NoritoSchema + NoritoSerialize,
    U: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    assert_eq!(
        norito::codec::encode_adaptive(left),
        norito::codec::encode_adaptive(right),
    );
    assert_ne!(
        norito::schema::identity::frame_hash::<T>(),
        norito::schema::identity::frame_hash::<U>(),
    );
    assert!(matches!(
        norito::decode_from_bytes::<U>(&norito::encode_canonical(left).unwrap()),
        Err(norito::core::Error::SchemaMismatch)
    ));
}

#[test]
fn generic_frames_preserve_nominal_arguments_despite_equal_payloads() {
    // These string forms share root-frame projections and payload bytes. A
    // containing generic record must still retain its argument's nominal name.
    reject_other_argument(
        &metadata(Box::<str>::from("same target")),
        &metadata(String::from("same target")),
    );
    reject_other_argument(
        &Validate {
            context: context(),
            target: Box::<str>::from("same target"),
        },
        &Validate {
            context: context(),
            target: String::from("same target"),
        },
    );
    reject_other_argument(
        &Mismatch {
            expected: Box::<str>::from("expected"),
            actual: Box::<str>::from("observed"),
        },
        &Mismatch {
            expected: String::from("expected"),
            actual: String::from("observed"),
        },
    );
}
