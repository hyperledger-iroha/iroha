//! Immutable generic query frames and nominal marker composition.

use std::collections::BTreeMap;

use norito::{
    NoritoDeserialize, NoritoSchema, NoritoSerialize,
    json::{self, Value},
};

use super::{
    CommittedTransaction, ErasedIterQuery, QueryWithFilter,
    dsl::{CompoundPredicate, SelectorTuple},
    tx_predicate::CommittedTxPredicate,
};
use crate::{account::Account, asset::AssetDefinitionId};
use iroha_model_base::domain::DomainId;

pub fn record<T>(value: &T) -> Value
where
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let frame = norito::encode_canonical(value).expect("encode query identity fixture");
    let decoded: T = norito::decode_from_bytes(&frame).expect("decode query identity fixture");
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
    let expected_hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(norito::schema::identity::frame_hash::<T>(), expected_hash);
    assert_eq!(norito::schema::identity::frame_hash::<T>(), expected_hash);
    let header = norito::core::Header::read(frame.as_slice()).unwrap();
    assert_eq!(header.schema, expected_hash);
    assert_eq!(T::frame_name(), T::nominal_name());

    let mut wrong_schema = frame.clone();
    // The schema follows the four-byte magic and the two version bytes.
    wrong_schema[6] ^= 1;
    assert!(matches!(
        norito::decode_from_bytes::<T>(&wrong_schema),
        Err(norito::core::Error::SchemaMismatch)
    ));
    for end in [0, norito::core::Header::SIZE - 1, frame.len() - 1] {
        assert!(norito::decode_from_bytes::<T>(&frame[..end]).is_err());
    }
    let raw = norito::codec::encode_adaptive(value);
    norito::json!({
        "nominal": (T::nominal_name()),
        "serialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "deserialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "bare_hex": (hex::encode(raw)),
        "frame_hex": (hex::encode(frame)),
    })
}

pub fn family<T>(label: &str, make: impl Fn() -> T) -> Value
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

fn account_predicate() -> CompoundPredicate<Account> {
    CompoundPredicate::<Account>::build(|row| {
        row.equals("metadata.label", "雪").exists("metadata.active")
    })
}

fn transaction_tree() -> CommittedTxPredicate {
    use CommittedTxPredicate as P;
    P::And(vec![
        P::TsIn(vec![7, 31, 999]),
        P::Or(vec![
            P::ResultEq(true),
            P::Not(Box::new(P::ResultEq(false))),
        ]),
        P::MetadataIn {
            key: "label".parse().unwrap(),
            values: vec![
                iroha_primitives::json::Json::new(norito::json!({"kind": "雪"})),
                iroha_primitives::json::Json::new(norito::json!([true, 7])),
            ],
        },
    ])
}

fn transaction_predicate() -> CompoundPredicate<CommittedTransaction> {
    CompoundPredicate::from_committed_tx_predicate(transaction_tree())
}

fn account_query_payload() -> Vec<u8> {
    norito::codec::Encode::encode(&super::account::FindAccountsWithAsset::new(
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("market", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
    ))
}

fn committed_transaction(merged: bool) -> CommittedTransaction {
    use crate::{
        account::AccountId,
        prelude::{DataTriggerSequence, ExecutionStep, TimeTriggerEntrypoint},
        transaction::{TransactionEntrypoint, TransactionResult},
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, MerkleTree};

    let authority = AccountId::new(
        KeyPair::try_from_seed(vec![0x19; 32], Algorithm::Ed25519)
            .expect("query identity account seed")
            .public_key()
            .clone(),
    );
    let entrypoint = TransactionEntrypoint::Time(TimeTriggerEntrypoint {
        id: "identity_fixture".parse().unwrap(),
        instructions: ExecutionStep(
            vec![
                crate::isi::Log {
                    level: crate::Level::INFO,
                    msg: "query identity fixture".into(),
                }
                .into(),
            ]
            .into(),
        ),
        authority,
    });
    let result = TransactionResult::new(Ok(DataTriggerSequence::default()));
    let entrypoint_hash = entrypoint.hash();
    let result_hash = result.hash();
    let entrypoints: MerkleTree<TransactionEntrypoint> = [entrypoint_hash].into_iter().collect();
    let results: MerkleTree<TransactionResult> = [result_hash].into_iter().collect();
    CommittedTransaction {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"query identity carrier")),
        entrypoint_hash,
        entrypoint_proof: entrypoints.get_proof(0).unwrap(),
        entrypoint,
        result_hash,
        result_proof: results.get_proof(0).unwrap(),
        result,
        merge_inclusion: merged.then(|| super::CertifiedMergeTransactionInclusion {
            version: 1,
            merge_entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"query identity merge")),
            merge_epoch_id: 7,
            execution_batch_hash: Hash::new(b"query identity execution"),
            entrypoint_count: 1,
            entrypoint_merkle_root: entrypoints.root().unwrap(),
            result_merkle_root: results.root().unwrap(),
        }),
    }
}

fn current_families() -> Vec<Value> {
    let mut rows = vec![
        family("predicate-u32-pass", || CompoundPredicate::<u32>::PASS),
        family("predicate-u64-pass", || CompoundPredicate::<u64>::PASS),
        family("predicate-account-json", account_predicate),
        family("predicate-transaction-pass", || {
            CompoundPredicate::<CommittedTransaction>::PASS
        }),
        family("predicate-transaction-tree", transaction_predicate),
        family("selector-u32-full", SelectorTuple::<u32>::default),
        family("selector-u64-full", SelectorTuple::<u64>::default),
        family("selector-account-full", SelectorTuple::<Account>::default),
        family(
            "selector-transaction-full",
            SelectorTuple::<CommittedTransaction>::default,
        ),
        family("filtered-account-pass", || {
            QueryWithFilter::<Account>::new((), CompoundPredicate::PASS, SelectorTuple::default())
        }),
        family("filtered-account-json", || {
            QueryWithFilter::new((), account_predicate(), SelectorTuple::default())
        }),
        family("filtered-transaction-tree", || {
            QueryWithFilter::new((), transaction_predicate(), SelectorTuple::default())
        }),
        family("erased-account-json", || {
            ErasedIterQuery::new(
                account_predicate(),
                SelectorTuple::default(),
                account_query_payload(),
            )
        }),
        family("erased-transaction-tree", || {
            ErasedIterQuery::new(
                transaction_predicate(),
                SelectorTuple::default(),
                norito::codec::Encode::encode(&super::transaction::FindTransactions),
            )
        }),
        family("transaction-tree", transaction_tree),
        family("committed-ordinary", || committed_transaction(false)),
        family("committed-merged", || committed_transaction(true)),
        family("merge-inclusion", || {
            committed_transaction(true).merge_inclusion.unwrap()
        }),
    ];
    rows.extend(super::tx_predicate::generic_membership_identity_records());
    #[cfg(feature = "ids_projection")]
    rows.extend([
        family("selector-mode-full", || super::dsl::SelectorMode::Full),
        family("selector-mode-ids", || super::dsl::SelectorMode::IdsOnly),
        family("selector-account-ids", SelectorTuple::<Account>::ids_only),
        family("filtered-account-ids", || {
            QueryWithFilter::new((), account_predicate(), SelectorTuple::ids_only())
        }),
        family("erased-account-ids", || {
            ErasedIterQuery::new(
                account_predicate(),
                SelectorTuple::ids_only(),
                account_query_payload(),
            )
        }),
    ]);
    rows
}

#[test]
fn complete_query_frames_match_pre_declaration_fixtures() {
    use sha2::{Digest as _, Sha256};

    #[cfg(not(feature = "ids_projection"))]
    let (source, digest, family_count) = (
        include_str!("../../tests/fixtures/query_generic_full_identity_frames.json"),
        "bf9729bee6ebcf73579659cd0d088a5f3d630385d69ec7e75188432f8b1b32ab",
        24,
    );
    #[cfg(feature = "ids_projection")]
    let (source, digest, family_count) = (
        include_str!("../../tests/fixtures/query_generic_ids_identity_frames.json"),
        "78ecf99df5eff1eecba94498266d0b925ed01c88a076b40c77700535db9a7115",
        29,
    );
    assert_eq!(hex::encode(Sha256::digest(source.as_bytes())), digest);
    let expected: Vec<Value> = json::from_str(source).expect("immutable query fixture");
    let actual = current_families();
    assert_eq!(expected.len(), family_count);
    assert_eq!(actual.len(), family_count);
    let mut cases = std::collections::BTreeSet::new();
    for (expected, actual) in expected.iter().zip(&actual) {
        let case = expected.get("case").and_then(Value::as_str).unwrap();
        assert!(cases.insert(case), "each fixture case is distinct");
        assert_eq!(expected.get("case"), actual.get("case"));
        for shape in ["root", "vec", "option", "map"] {
            let expected = expected.get(shape).expect("captured shape");
            let actual = actual.get(shape).expect("current shape");
            assert_eq!(actual, expected, "{case}: {shape}");
        }
    }
}

/// Identity-only markers deliberately have no payload codec or schema-export implementation.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "query_fixture::MarkerA")]
pub(super) struct MarkerA;

#[derive(norito::NoritoSchema)]
#[norito_schema(name = "query_fixture::MarkerB")]
struct MarkerB;

fn assert_marker_composition<T: NoritoSchema + Send + Sync>() {
    for (constructor, actual) in [
        (
            "iroha_data_model::query::dsl::CompoundPredicate",
            CompoundPredicate::<T>::nominal_name(),
        ),
        (
            "iroha_data_model::query::dsl::SelectorTuple",
            SelectorTuple::<T>::nominal_name(),
        ),
        (
            "iroha_data_model::query::model::QueryWithFilter",
            QueryWithFilter::<T>::nominal_name(),
        ),
        (
            "iroha_data_model::query::ErasedIterQuery",
            ErasedIterQuery::<T>::nominal_name(),
        ),
    ] {
        assert_eq!(actual, format!("{constructor}<{}>", T::nominal_name()));
    }
}

#[test]
fn query_identity_requires_nominal_markers_without_payload_codecs() {
    assert_marker_composition::<MarkerA>();
    assert_marker_composition::<MarkerB>();
    assert_ne!(
        QueryWithFilter::<MarkerA>::nominal_name(),
        QueryWithFilter::<MarkerB>::nominal_name()
    );
    assert_eq!(
        Vec::<QueryWithFilter<MarkerA>>::nominal_name(),
        "alloc::vec::Vec<iroha_data_model::query::model::QueryWithFilter<query_fixture::MarkerA>>"
    );
}

#[test]
fn captured_query_hash_markers_retain_their_codec_identities() {
    fn check<T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>>(nominal: &str) {
        assert_eq!(T::nominal_name(), nominal);
        assert_eq!(T::frame_name(), nominal);
        let expected = norito::core::schema_hash_for_name(nominal);
        assert_eq!(norito::schema::identity::frame_hash::<T>(), expected);
        assert_eq!(norito::schema::identity::frame_hash::<T>(), expected);
    }
    // Both marker names occur inside the original populated MembershipValues<HashOf<T>> frames.
    check::<crate::block::BlockHeader>("iroha_data_model::block::header::model::BlockHeader");
    check::<crate::transaction::TransactionEntrypoint>(
        "iroha_data_model::transaction::signed::model::TransactionEntrypoint",
    );
}

fn reject_other_marker<T, U>(left: &T, right: &U)
where
    T: NoritoSchema + NoritoSerialize,
    U: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    assert_eq!(
        norito::codec::encode_adaptive(left),
        norito::codec::encode_adaptive(right),
        "the payload cannot distinguish these item markers"
    );
    assert_ne!(
        norito::schema::identity::frame_hash::<T>(),
        norito::schema::identity::frame_hash::<U>()
    );
    let frame = norito::encode_canonical(left).unwrap();
    assert!(matches!(
        norito::decode_from_bytes::<U>(&frame),
        Err(norito::core::Error::SchemaMismatch)
    ));
}

#[test]
fn identical_query_payloads_cannot_cross_item_marker_frames() {
    fn filtered<T: Send + Sync>() -> QueryWithFilter<T> {
        QueryWithFilter::new((), CompoundPredicate::PASS, SelectorTuple::default())
    }
    fn erased<T: Send + Sync>() -> ErasedIterQuery<T> {
        ErasedIterQuery::new(
            CompoundPredicate::PASS,
            SelectorTuple::default(),
            vec![7, 31],
        )
    }
    macro_rules! check {
        ($left:expr, $right:expr) => {
            reject_other_marker(&$left, &$right);
            reject_other_marker(&vec![$left, $left], &vec![$right, $right]);
            reject_other_marker(&Some($left), &Some($right));
            reject_other_marker(
                &BTreeMap::from([(7_u32, $left)]),
                &BTreeMap::from([(7_u32, $right)]),
            );
        };
    }
    check!(
        CompoundPredicate::<u32>::PASS,
        CompoundPredicate::<u64>::PASS
    );
    check!(
        SelectorTuple::<u32>::default(),
        SelectorTuple::<u64>::default()
    );
    check!(filtered::<u32>(), filtered::<u64>());
    check!(erased::<u32>(), erased::<u64>());
}
