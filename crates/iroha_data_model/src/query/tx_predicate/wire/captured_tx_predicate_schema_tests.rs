//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::Case::bidirectional::<super::Node>(
        "iroha_data_model::query::tx_predicate::wire::Node",
    )
    .check();
}

pub(in crate::query::tx_predicate) fn generic_membership_identity_records()
-> Vec<norito::json::Value> {
    use super::{HashOf, Json, MembershipDecodeBudgetGuard, MembershipValues};
    use crate::query::generic_identity_tests::family;
    // Private membership payloads are only decoded inside a predicate's bounded scope.
    let _budget = MembershipDecodeBudgetGuard::enter();
    vec![
        family("membership-u64", || {
            MembershipValues::from(vec![7_u64, 31, 999])
        }),
        family("membership-bool", || {
            MembershipValues::from(vec![false, true])
        }),
        family("membership-account", || {
            MembershipValues::from(vec![crate::account::AccountId::new(
                iroha_crypto::KeyPair::try_from_seed(
                    vec![0x19; 32],
                    iroha_crypto::Algorithm::Ed25519,
                )
                .expect("query identity account seed")
                .public_key()
                .clone(),
            )])
        }),
        family("membership-entrypoint", || {
            MembershipValues::from(vec![HashOf::<
                crate::transaction::signed::TransactionEntrypoint,
            >::from_untyped_unchecked(
                iroha_crypto::Hash::new(b"query entrypoint identity fixture"),
            )])
        }),
        family("membership-block", || {
            MembershipValues::from(vec![
                HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
                    iroha_crypto::Hash::new(b"query block identity fixture"),
                ),
            ])
        }),
        family("membership-json", || {
            MembershipValues::from(vec![
                Json::new(norito::json!({"label": "雪"})),
                Json::new(norito::json!([true, 7])),
            ])
        }),
    ]
}

#[test]
fn membership_frame_decode_requires_its_existing_predicate_budget() {
    use super::MembershipValues;
    let value = MembershipValues::from(vec![7_u64, 31, 999]);
    let bytes = norito::encode_canonical(&value).unwrap();
    assert!(matches!(
        norito::decode_from_bytes::<MembershipValues<u64>>(&bytes),
        Err(norito::core::Error::Message(reason))
            if reason == "CommittedTxPredicate membership decoded outside its predicate budget"
    ));
    {
        let _budget = super::MembershipDecodeBudgetGuard::enter();
        let decoded = norito::decode_from_bytes::<MembershipValues<u64>>(&bytes).unwrap();
        assert_eq!(decoded.0, [7, 31, 999]);
    }
    assert!(norito::decode_from_bytes::<MembershipValues<u64>>(&bytes).is_err());
}

#[test]
fn membership_identity_uses_marker_identity_without_a_payload_codec() {
    use norito::NoritoSchema;
    type MarkerMembership = super::MembershipValues<crate::query::generic_identity_tests::MarkerA>;
    assert_eq!(
        MarkerMembership::nominal_name(),
        "iroha_data_model::query::tx_predicate::wire::MembershipValues<query_fixture::MarkerA>"
    );
}
