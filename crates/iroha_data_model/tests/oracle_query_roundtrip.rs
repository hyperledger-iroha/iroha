//! Oracle query wire-format roundtrip tests.

use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    nexus::UniversalAccountId,
    oracle::{FeedId, KeyedHash, OracleChangeId, OracleDisputeId, OracleProviderKey},
    prelude::AccountId,
    query::oracle::prelude::*,
};

macro_rules! assert_roundtrip {
    ($value:expr, $ty:ty) => {{
        let value: $ty = $value;
        let bytes = norito::to_bytes(&value).expect("encode query");
        let decoded: $ty = norito::decode_from_bytes(&bytes).expect("decode query");
        assert_eq!(decoded, value);
    }};
}

#[test]
fn oracle_queries_roundtrip_through_norito() {
    let feed_id: FeedId = "price_xor_usd".parse().expect("feed id");
    let provider_id = AccountId::new(KeyPair::random().public_key().clone());
    let provider_key = OracleProviderKey::new(feed_id.clone(), provider_id);
    let dispute_id = OracleDisputeId(42);
    let change_id = OracleChangeId(Hash::new(b"oracle-change-query"));
    let uaid = UniversalAccountId::from_hash(Hash::new(b"oracle-query-uaid"));
    let binding_hash = KeyedHash::new("pepper", b"pepper-bytes", b"twitter-user");

    assert_roundtrip!(FindOracleFeeds, FindOracleFeeds);
    assert_roundtrip!(FindOracleFeedById::new(feed_id.clone()), FindOracleFeedById);
    assert_roundtrip!(
        FindOracleHistoryByFeedId::new(feed_id.clone()),
        FindOracleHistoryByFeedId
    );
    assert_roundtrip!(
        FindOracleProviderStatsByFeedId::new(feed_id.clone()),
        FindOracleProviderStatsByFeedId
    );
    assert_roundtrip!(
        FindOracleProviderStatsByKey::new(provider_key),
        FindOracleProviderStatsByKey
    );
    assert_roundtrip!(FindOracleDisputes, FindOracleDisputes);
    assert_roundtrip!(
        FindOracleDisputeById::new(dispute_id),
        FindOracleDisputeById
    );
    assert_roundtrip!(
        FindOracleDisputesByFeedId::new(feed_id),
        FindOracleDisputesByFeedId
    );
    assert_roundtrip!(FindOracleChanges, FindOracleChanges);
    assert_roundtrip!(FindOracleChangeById::new(change_id), FindOracleChangeById);
    assert_roundtrip!(
        FindTwitterBindingsByUaid::new(uaid),
        FindTwitterBindingsByUaid
    );
    assert_roundtrip!(
        FindTwitterBindingByHash::new(binding_hash),
        FindTwitterBindingByHash
    );
}

#[test]
fn oracle_query_decode_rejects_truncated_norito_payloads() {
    let feed_id: FeedId = "price_xor_usd".parse().expect("feed id");
    let query = FindOracleFeedById::new(feed_id);
    let bytes = norito::to_bytes(&query).expect("encode query");
    let truncated_lengths = [0_usize, 1, bytes.len() / 2, bytes.len().saturating_sub(1)];

    for len in truncated_lengths {
        assert!(
            norito::decode_from_bytes::<FindOracleFeedById>(&bytes[..len]).is_err(),
            "truncated query payload of length {len} must reject"
        );
    }
}
