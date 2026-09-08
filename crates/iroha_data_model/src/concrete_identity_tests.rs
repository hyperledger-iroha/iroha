//! Immutable concrete model frames, feature-independent event tags and owned projections.

use std::{collections::BTreeMap, num::NonZeroU64, time::Duration};

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_primitives::json::Json;
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json::Value};
use sha2::{Digest, Sha256};

use crate::{
    account::AccountId,
    block::BlockHeader,
    events::{
        EventBox,
        data::{
            DataEvent,
            game::GameSessionEventV1,
            prelude::{AccountEvent, MetadataChanged, PeerEvent},
        },
        execute_trigger::{ExecuteTriggerEvent, ExecuteTriggerEventFilter},
        time::{ExecutionTime, Schedule, TimeEventFilter},
    },
    isi::{InstructionBox, Log},
    metadata::Metadata,
    peer::PeerId,
    smart_contract::payloads::{ExecutorContext, SmartContractContext, TriggerContext},
    trigger::{
        TriggerId,
        action::{Action, Repeats, TimeTriggerRetryPolicy},
    },
};

fn serialized_record<T: NoritoSchema + NoritoSerialize>(value: &T) -> Value {
    let bare = norito::codec::encode_adaptive(value);
    let frame = norito::encode_canonical(value).expect("encode concrete identity frame");
    let header = norito::core::Header::read(&mut frame.as_slice())
        .expect("read emitted concrete identity header");
    let expected_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(<T as NoritoSerialize>::schema_hash(), expected_hash);
    assert_eq!(header.schema, expected_hash);
    norito::json!({
        "actual_type_name": (T::nominal_name()),
        "serialize_hash": (hex::encode(<T as NoritoSerialize>::schema_hash())),
        "bare_hex": (hex::encode(bare)),
        "frame_hex": (hex::encode(&frame)),
        "frame_sha256": (hex::encode(Sha256::digest(&frame))),
        "header_flags": (header.flags),
    })
}

fn record<T>(value: &T) -> Value
where
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let frame = norito::encode_canonical(value).expect("encode concrete identity frame");
    let decoded: T = norito::decode_from_bytes(&frame).expect("decode concrete identity frame");
    assert_eq!(
        norito::encode_canonical(&decoded).expect("re-encode concrete identity frame"),
        frame,
    );
    let bare = norito::codec::encode_adaptive(value);
    let decoded_bare: T = norito::codec::decode_adaptive(&bare)
        .expect("decode current-layout concrete identity payload");
    assert_eq!(norito::codec::encode_adaptive(&decoded_bare), bare);
    assert_eq!(T::frame_name(), T::nominal_name());
    assert_eq!(
        <T as NoritoDeserialize>::schema_hash(),
        norito::schema::identity::frame_hash::<T>(),
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
        "encoding": (serialized_record(value)),
        "deserialize_hash": (hex::encode(<T as NoritoDeserialize>::schema_hash())),
        "decode_reencode_exact": true,
        "bare_decode_reencode_exact": true,
    })
}

fn family<T>(label: &str, value: T) -> Value
where
    T: Clone + NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    norito::json!({
        "case": label,
        "root": (record(&value)),
        "vec": (record(&vec![value.clone(), value.clone()])),
        "option": (record(&Some(value.clone()))),
        "map": (record(&BTreeMap::from([(7_u32, value.clone()), (31, value)]))),
    })
}

/// Check an encoding-only adapter through its declared projection and owned decoder.
pub(crate) fn projected_record<P, T>(projection: &P, material: &T) -> Value
where
    P: NoritoSchema + NoritoSerialize,
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    assert_eq!(P::frame_name(), T::frame_name());
    let projected = norito::encode_canonical(projection).expect("encode projected frame");
    let owned = norito::encode_canonical(material).expect("encode owned projection material");
    assert_eq!(
        <P as NoritoSerialize>::schema_hash(),
        <T as NoritoSerialize>::schema_hash()
    );
    assert_eq!(
        projected, owned,
        "the active root projection must remain exact"
    );
    let decoded: T = norito::decode_from_bytes(&projected).expect("decode through projected owner");
    assert_eq!(
        norito::encode_canonical(&decoded).expect("re-encode projected owner"),
        owned
    );
    norito::json!({
        "adapter": (serialized_record(projection)),
        "owner": (record(material)),
        "adapter_decode_exercised": false,
        "owned_decode_reencode_exact": true,
    })
}

/// Read the unchanged default/HTTP capture; feature flags describe its provenance.
pub(crate) fn fixture_values(source: &str) -> Value {
    let fixture: Value = norito::json::from_str(source).expect("parse immutable concrete capture");
    assert_eq!(fixture["governance"], Value::Bool(true));
    assert_eq!(fixture["http"], Value::Bool(true));
    assert_eq!(fixture["ids_projection"], Value::Bool(false));
    fixture["values"].clone()
}

fn key_pair() -> KeyPair {
    KeyPair::try_from_seed(vec![0x57; 32], Algorithm::Ed25519)
        .expect("deterministic concrete fixture key")
}

fn account() -> AccountId {
    AccountId::new(key_pair().public_key().clone())
}

fn trigger_id() -> TriggerId {
    "concrete_identity_trigger"
        .parse()
        .expect("valid trigger identifier")
}

fn instruction() -> InstructionBox {
    Log {
        level: crate::Level::INFO,
        msg: "concrete identity 雪".to_owned(),
    }
    .into()
}

fn metadata() -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        "capture".parse().expect("metadata name"),
        Json::new(norito::json!({"message": "雪", "count": 7, "enabled": true})),
    );
    metadata
}

fn scheduled_action(retry: bool) -> Action {
    let action = Action::new(
        vec![instruction()],
        Repeats::Exactly(2),
        account(),
        TimeEventFilter::new(ExecutionTime::Schedule(
            Schedule::starting_at(Duration::from_secs(1)).with_period(Duration::from_secs(5)),
        )),
    )
    .expect("valid scheduled action")
    .with_metadata(metadata());
    if retry {
        action
            .with_retry_policy(TimeTriggerRetryPolicy {
                max_retries: std::num::NonZeroU32::new(3).expect("nonzero retry count"),
                retry_after_ms: NonZeroU64::new(1_000).expect("nonzero retry delay"),
            })
            .expect("scheduled action accepts retry policy")
    } else {
        action
    }
}

fn explicit_action() -> Action {
    Action::new(
        vec![instruction()],
        Repeats::Indefinitely,
        account(),
        ExecuteTriggerEventFilter::new().for_trigger(trigger_id()),
    )
    .expect("constructor binds execute-trigger authority")
    .with_metadata(metadata())
}

fn header() -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(7).expect("nonzero header height"),
        Some(HashOf::from_untyped_unchecked(Hash::new(
            b"concrete fixture predecessor",
        ))),
        None,
        None,
        1_700_000_000_007,
        3,
    )
}

fn trigger_context() -> TriggerContext {
    TriggerContext {
        id: trigger_id(),
        authority: account(),
        curr_block: header(),
        event: EventBox::ExecuteTrigger(ExecuteTriggerEvent {
            trigger_id: trigger_id(),
            authority: account(),
            args: Json::new(norito::json!({"quantity": 9, "memo": "雪"})),
        }),
    }
}

#[cfg(feature = "http")]
fn stream_block() -> crate::block::SignedBlock {
    use crate::{
        block::{SignedBlock, builder::BlockBuilder},
        transaction::{DataTriggerSequence, FeePaymentIntent, TransactionBuilder},
    };
    let keys = key_pair();
    let mut transaction =
        TransactionBuilder::new_genesis(account(), FeePaymentIntent::authority(Vec::new(), None))
            .with_instructions([instruction()]);
    transaction.set_creation_time(Duration::from_millis(1_700_000_000_007));
    let transaction = transaction
        .try_sign(keys.private_key())
        .expect("sign fixture transaction");
    let proposal =
        SignedBlock::try_genesis(vec![transaction.clone()], keys.private_key(), None, None)
            .expect("build signed genesis proposal");
    let mut builder = BlockBuilder::new(proposal.header());
    builder.set_da_proof_policies(proposal.da_proof_policies().cloned());
    builder.push_transaction(transaction);
    builder.push_result(Ok(DataTriggerSequence::default()));
    let block = builder
        .try_build_with_signature(0, keys.private_key())
        .expect("sign result-bearing stream block");
    block
        .validate_entrypoint_merkle_cache()
        .expect("canonical entrypoint commitment");
    block
        .validate_result_merkle_cache()
        .expect("canonical result commitment");
    let signatures = block.signatures().collect::<Vec<_>>();
    assert_eq!(signatures.len(), 1);
    signatures[0]
        .signature()
        .verify_hash(keys.public_key(), block.hash())
        .expect("verify final stream-block signature");
    block
}

#[test]
fn concrete_identity_frames_match_capture() {
    let mut rows = vec![
        family("action-scheduled", scheduled_action(false)),
        family("action-scheduled-retry", scheduled_action(true)),
        family("action-execute-trigger", explicit_action()),
        family(
            "data-peer-added",
            DataEvent::Peer(PeerEvent::Added(PeerId::new(
                key_pair().public_key().clone(),
            ))),
        ),
        family(
            "data-account-metadata",
            DataEvent::Account(AccountEvent::MetadataInserted(MetadataChanged {
                target: account(),
                key: "capture".parse().expect("metadata key"),
                value: Json::new(norito::json!({"message": "雪", "count": 7})),
            })),
        ),
        family(
            "data-game-session",
            DataEvent::GameSession(GameSessionEventV1 {
                session_id: Hash::new(b"concrete fixture game"),
                revision: 7,
                phase: 2,
                dispute_root: Hash::new(b"concrete fixture dispute"),
                payout_claims: Vec::new(),
                item_stakes: Vec::new(),
                resources: Vec::new(),
                terminal_at_height: None,
            }),
        ),
        family(
            "smart-contract-context",
            SmartContractContext {
                authority: account(),
                curr_block: header(),
            },
        ),
        family(
            "executor-context",
            ExecutorContext {
                authority: account(),
                curr_block: header(),
            },
        ),
    ];
    rows.push(family("trigger-context", trigger_context()));
    #[cfg(feature = "governance")]
    rows.push(family(
        "data-governance-submitted",
        DataEvent::Governance(
            crate::events::data::governance::GovernanceEvent::ProposalSubmitted(
                crate::events::data::governance::GovernanceProposalSubmitted {
                    id: [0x37; 32],
                    proposer: account(),
                    contract_address: None,
                },
            ),
        ),
    ));
    #[cfg(feature = "http")]
    {
        use crate::block::stream::{BlockMessage, BlockSubscriptionRequest};
        rows.push(family(
            "block-subscription-first",
            BlockSubscriptionRequest::new(NonZeroU64::new(1).expect("first block")),
        ));
        rows.push(family(
            "block-subscription-later",
            BlockSubscriptionRequest::new(NonZeroU64::new(9).expect("later block")),
        ));
        rows.push(family("block-message", BlockMessage(stream_block())));
    }
    assert_eq!(
        rows.len(),
        9 + usize::from(cfg!(feature = "governance")) + 3 * usize::from(cfg!(feature = "http"))
    );
    let expected = fixture_values(include_str!(
        "../tests/fixtures/model_concrete_identity_frames.json"
    ));
    let captured = expected["families"].as_array().expect("captured families");
    let captured_names = captured
        .iter()
        .map(|row| row["case"].as_str().expect("captured family label"))
        .collect::<Vec<_>>();
    assert_eq!(
        captured_names,
        [
            "action-scheduled",
            "action-scheduled-retry",
            "action-execute-trigger",
            "data-peer-added",
            "data-account-metadata",
            "data-game-session",
            "smart-contract-context",
            "executor-context",
            "trigger-context",
            "data-governance-submitted",
            "block-subscription-first",
            "block-subscription-later",
            "block-message",
        ]
    );
    let available = captured
        .iter()
        .filter(
            |row| match row["case"].as_str().expect("captured family label") {
                "data-governance-submitted" => cfg!(feature = "governance"),
                "block-subscription-first" | "block-subscription-later" | "block-message" => {
                    cfg!(feature = "http")
                }
                _ => true,
            },
        )
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(rows, available);

    // Disabled governance rejects the reserved variant; it cannot become Social.
    #[cfg(not(feature = "governance"))]
    {
        let governance = captured
            .iter()
            .find(|row| row["case"].as_str() == Some("data-governance-submitted"))
            .expect("immutable governance case");
        let frame = hex::decode(
            governance["root"]["encoding"]["frame_hex"]
                .as_str()
                .expect("captured governance frame"),
        )
        .expect("valid captured frame hex");
        assert!(norito::decode_from_bytes::<DataEvent>(&frame).is_err());
    }
}

#[cfg(feature = "http")]
#[test]
fn block_message_send_identity_projection_matches_capture() {
    use crate::block::stream::{BlockMessage, BlockMessageSend};
    let block = stream_block();
    let owner = BlockMessage(block.clone());
    let projection = BlockMessageSend(std::sync::Arc::new(block));
    assert_eq!(
        projected_record(&projection, &owner),
        fixture_values(include_str!(
            "../tests/fixtures/block_message_send_identity_frame.json"
        )),
    );
}

#[test]
fn data_event_schema_reserves_disabled_capability_discriminants() {
    use iroha_schema::{IntoSchema as _, Metadata};

    let schema = DataEvent::schema();
    let Metadata::Enum(metadata) = schema.get::<DataEvent>().expect("data event schema") else {
        panic!("DataEvent schema must be an enum");
    };
    let expected = [
        ("Peer", 0),
        ("Domain", 1),
        ("Account", 2),
        ("Asset", 3),
        ("AssetDefinition", 4),
        ("Trigger", 5),
        ("Role", 6),
        ("Configuration", 7),
        ("Executor", 8),
        ("Proof", 9),
        ("VerifyingKey", 10),
        ("RuntimeUpgrade", 11),
        ("SmartContract", 12),
        ("Soradns", 13),
        ("Sorafs", 14),
        ("Musubi", 15),
        ("SpaceDirectory", 16),
        ("Escrow", 17),
        ("Oracle", 18),
        ("Governance", 19),
        ("Social", 20),
        ("Bridge", 21),
        ("GameSession", 22),
    ]
    .into_iter()
    .filter(|(name, _)| *name != "Governance" || cfg!(feature = "governance"))
    .collect::<Vec<_>>();
    assert_eq!(
        metadata
            .variants
            .iter()
            .map(|variant| {
                assert!(variant.ty.is_some(), "every data event carries its payload");
                (variant.tag.as_str(), variant.discriminant)
            })
            .collect::<Vec<_>>(),
        expected
    );
}
