//! Canonical soracles reference fixtures (price feed + Twitter follow binding).
//!
//! These tests ensure the JSON fixtures under `fixtures/oracle/` stay aligned
//! with the data model helpers and the deterministic hashing rules used by
//! aggregators and connectors.

use std::{collections::BTreeMap, fs, num::NonZeroU64, path::Path, str::FromStr};

use iroha_crypto::{Hash, Signature, SignatureOf};
use iroha_data_model::{
    account::AccountId,
    oracle::{
        AbsoluteOutlier, AggregationRule, ConnectorRequest, ConnectorRequestMethod, FeedConfig,
        FeedConfigVersion, FeedEvent, FeedId, KeyedHash, Observation, ObservationBody,
        ObservationOutcome, ObservationValue, OutlierPolicy, Report, RiskClass,
        aggregate_observations,
    },
};
use norito::{
    json::{self, FastJsonWrite, JsonDeserialize, JsonSerialize},
    literal,
};

const PRICE_FEED_CONFIG_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/oracle/feed_config_price_xor_usd.json"
));
const PRICE_CONNECTOR_REQUEST_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/oracle/connector_request_price_xor_usd.json"
));
const PRICE_OBSERVATION_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/oracle/observation_price_xor_usd.json"
));
const PRICE_REPORT_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/oracle/report_price_xor_usd.json"
));
const PRICE_FEED_EVENT_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/oracle/feed_event_price_xor_usd.json"
));

const FOLLOW_FEED_CONFIG_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/oracle/feed_config_twitter_follow.json"
));
const FOLLOW_CONNECTOR_REQUEST_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/oracle/connector_request_twitter_follow.json"
));
const FOLLOW_OBSERVATION_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/oracle/observation_twitter_follow.json"
));
const FOLLOW_REPORT_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/oracle/report_twitter_follow.json"
));
const FOLLOW_FEED_EVENT_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/oracle/feed_event_twitter_follow.json"
));

#[test]
fn price_reference_fixtures_are_canonical() {
    let providers = sample_providers();
    let feed_config = price_feed_config(&providers);
    let connector_request = price_connector_request(feed_config.feed_config_version);
    let request_hash: Hash = connector_request.hash().into();
    let observation_a = price_observation(
        &providers[0],
        ObservationValue::new(1_002_500, 5),
        request_hash,
    );
    let observation_b = price_observation(
        &providers[1],
        ObservationValue::new(1_001_500, 5),
        request_hash,
    );

    assert_fixture_eq(
        "feed_config_price_xor_usd.json",
        PRICE_FEED_CONFIG_FIXTURE,
        &feed_config,
    );
    assert_fixture_eq(
        "connector_request_price_xor_usd.json",
        PRICE_CONNECTOR_REQUEST_FIXTURE,
        &connector_request,
    );
    assert_fixture_eq(
        "observation_price_xor_usd.json",
        PRICE_OBSERVATION_FIXTURE,
        &observation_a,
    );

    let aggregation = aggregate_observations(
        &feed_config,
        observation_a.body.slot,
        request_hash,
        providers[1].clone(),
        &[observation_a.clone(), observation_b.clone()],
    )
    .expect("price feed aggregates");
    let report = Report {
        body: aggregation.report.clone(),
        signature: zero_signature(),
    };
    let feed_event = FeedEvent {
        feed_id: feed_config.feed_id.clone(),
        feed_config_version: feed_config.feed_config_version,
        slot: observation_a.body.slot,
        outcome: aggregation.outcome.clone(),
    };

    assert_fixture_eq("report_price_xor_usd.json", PRICE_REPORT_FIXTURE, &report);
    assert_fixture_eq(
        "feed_event_price_xor_usd.json",
        PRICE_FEED_EVENT_FIXTURE,
        &feed_event,
    );
}

#[test]
fn follow_reference_fixtures_are_canonical() {
    let providers = sample_providers();
    let feed_config = follow_feed_config(&providers);

    let follow_keyed = KeyedHash::new(
        "pepper-social-v1",
        b"pepper-social-v1",
        b"twitter_user_id:1234567890",
    );
    let follow_value = ObservationValue::from_keyed_hash(&follow_keyed);

    let connector_request =
        follow_connector_request(feed_config.feed_config_version, &follow_keyed);
    let request_hash: Hash = connector_request.hash().into();
    let observation_a = follow_observation(
        &providers[0],
        follow_value,
        request_hash,
        Some(1_700_000_120_000),
    );
    let observation_b = follow_observation(
        &providers[1],
        follow_value,
        request_hash,
        Some(1_700_000_120_001),
    );

    assert_fixture_eq(
        "feed_config_twitter_follow.json",
        FOLLOW_FEED_CONFIG_FIXTURE,
        &feed_config,
    );
    assert_fixture_eq(
        "connector_request_twitter_follow.json",
        FOLLOW_CONNECTOR_REQUEST_FIXTURE,
        &connector_request,
    );
    assert_fixture_eq(
        "observation_twitter_follow.json",
        FOLLOW_OBSERVATION_FIXTURE,
        &observation_a,
    );

    let aggregation = aggregate_observations(
        &feed_config,
        observation_a.body.slot,
        request_hash,
        providers[0].clone(),
        &[observation_a.clone(), observation_b.clone()],
    )
    .expect("follow feed aggregates");
    let report = Report {
        body: aggregation.report.clone(),
        signature: zero_signature(),
    };
    let feed_event = FeedEvent {
        feed_id: feed_config.feed_id.clone(),
        feed_config_version: feed_config.feed_config_version,
        slot: observation_a.body.slot,
        outcome: aggregation.outcome.clone(),
    };

    assert_fixture_eq("report_twitter_follow.json", FOLLOW_REPORT_FIXTURE, &report);
    assert_fixture_eq(
        "feed_event_twitter_follow.json",
        FOLLOW_FEED_EVENT_FIXTURE,
        &feed_event,
    );
}

#[test]
#[ignore = "regenerates oracle reference fixtures"]
#[allow(clippy::too_many_lines)]
fn regenerate_follow_reference_fixtures() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("oracle");
    let providers = sample_providers();
    let price_feed_config = price_feed_config(&providers);
    let price_connector_request = price_connector_request(price_feed_config.feed_config_version);
    let price_request_hash: Hash = price_connector_request.hash().into();
    let price_observation_a = price_observation(
        &providers[0],
        ObservationValue::new(1_002_500, 5),
        price_request_hash,
    );
    let price_observation_b = price_observation(
        &providers[1],
        ObservationValue::new(1_001_500, 5),
        price_request_hash,
    );
    let price_aggregation = aggregate_observations(
        &price_feed_config,
        price_observation_a.body.slot,
        price_request_hash,
        providers[1].clone(),
        &[price_observation_a.clone(), price_observation_b.clone()],
    )
    .expect("price feed aggregates");
    let price_report = Report {
        body: price_aggregation.report.clone(),
        signature: zero_signature(),
    };
    let price_feed_event = FeedEvent {
        feed_id: price_feed_config.feed_id.clone(),
        feed_config_version: price_feed_config.feed_config_version,
        slot: price_observation_a.body.slot,
        outcome: price_aggregation.outcome.clone(),
    };

    write_fixture(
        &base.join("feed_config_price_xor_usd.json"),
        &price_feed_config,
    );
    write_fixture(
        &base.join("connector_request_price_xor_usd.json"),
        &price_connector_request,
    );
    write_fixture(
        &base.join("observation_price_xor_usd.json"),
        &price_observation_a,
    );
    write_fixture(&base.join("report_price_xor_usd.json"), &price_report);
    write_fixture(
        &base.join("feed_event_price_xor_usd.json"),
        &price_feed_event,
    );

    let feed_config = follow_feed_config(&providers);
    let follow_keyed = KeyedHash::new(
        "pepper-social-v1",
        b"pepper-social-v1",
        b"twitter_user_id:1234567890",
    );
    let follow_value = ObservationValue::from_keyed_hash(&follow_keyed);
    let connector_request =
        follow_connector_request(feed_config.feed_config_version, &follow_keyed);
    let request_hash: Hash = connector_request.hash().into();
    let observation_a = follow_observation(
        &providers[0],
        follow_value,
        request_hash,
        Some(1_700_000_120_000),
    );
    let observation_b = follow_observation(
        &providers[1],
        follow_value,
        request_hash,
        Some(1_700_000_120_001),
    );
    let aggregation = aggregate_observations(
        &feed_config,
        observation_a.body.slot,
        request_hash,
        providers[0].clone(),
        &[observation_a.clone(), observation_b.clone()],
    )
    .expect("follow feed aggregates");
    let report = Report {
        body: aggregation.report.clone(),
        signature: zero_signature(),
    };
    let feed_event = FeedEvent {
        feed_id: feed_config.feed_id.clone(),
        feed_config_version: feed_config.feed_config_version,
        slot: observation_a.body.slot,
        outcome: aggregation.outcome.clone(),
    };

    write_fixture(&base.join("feed_config_twitter_follow.json"), &feed_config);
    write_fixture(
        &base.join("connector_request_twitter_follow.json"),
        &connector_request,
    );
    write_fixture(
        &base.join("observation_twitter_follow.json"),
        &observation_a,
    );
    write_fixture(&base.join("report_twitter_follow.json"), &report);
    write_fixture(&base.join("feed_event_twitter_follow.json"), &feed_event);
}

fn assert_fixture_eq<T>(path: &str, fixture: &str, expected: &T)
where
    T: Clone + PartialEq + FastJsonWrite + JsonDeserialize + JsonSerialize,
{
    let decoded: T = json::from_str(fixture).unwrap_or_else(|error| {
        panic!(
            "failed to decode fixture {path}: {error:?}\nexpected JSON:\n{}",
            json::to_json_pretty(expected).expect("serialise expected")
        )
    });

    assert!(
        decoded == *expected,
        "fixture {path} does not match expected shape\nexpected JSON:\n{}",
        json::to_json_pretty(expected).expect("serialise expected")
    );
}

fn write_fixture<T: JsonSerialize>(path: &Path, value: &T) {
    let json = json::to_json_pretty(value).expect("serialise fixture");
    fs::write(path, json.as_bytes()).unwrap_or_else(|err| {
        panic!("failed to write {}: {err}", path.display());
    });
}

fn sample_providers() -> Vec<AccountId> {
    [
        "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE",
        "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
    ]
    .into_iter()
    .map(parse_fixture_account)
    .collect()
}

fn parse_fixture_account(literal: &str) -> AccountId {
    AccountId::parse_encoded(literal)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("fixture account id")
}

fn price_feed_config(providers: &[AccountId]) -> FeedConfig {
    FeedConfig {
        feed_id: FeedId::from_str("price_xor_usd").expect("feed id"),
        feed_config_version: FeedConfigVersion(1),
        providers: providers.to_vec(),
        connector_id: "http-bin".to_string(),
        connector_version: 2,
        cadence_slots: NonZeroU64::new(5).expect("cadence"),
        aggregation: AggregationRule::MedianMad(300),
        outlier_policy: OutlierPolicy::Mad(350),
        min_signers: 2,
        committee_size: 2,
        risk_class: RiskClass::Medium,
        max_observers: 4,
        max_value_len: 128,
        max_error_rate_bps: 250,
        dispute_window_slots: NonZeroU64::new(10).expect("dispute window"),
        replay_window_slots: NonZeroU64::new(4).expect("replay window"),
    }
}

fn price_connector_request(feed_config_version: FeedConfigVersion) -> ConnectorRequest {
    let mut headers = BTreeMap::new();
    headers.insert(
        "user-agent".to_string(),
        iroha_data_model::oracle::RedactedHeaderValue::Plain("soracles/1.0".to_string()),
    );
    headers.insert(
        "x-api-key".to_string(),
        iroha_data_model::oracle::RedactedHeaderValue::Hashed(Hash::new(b"price-key")),
    );

    let mut query = BTreeMap::new();
    query.insert("pair".to_string(), "XOR/USD".to_string());

    ConnectorRequest {
        feed_id: FeedId::from_str("price_xor_usd").expect("feed id"),
        feed_config_version,
        slot: 10,
        connector_id: "http-bin".to_string(),
        connector_version: 2,
        method: ConnectorRequestMethod::Get,
        endpoint: "/price".to_string(),
        query,
        headers,
        body_hash: Hash::new(b""),
    }
}

fn price_observation(
    provider: &AccountId,
    value: ObservationValue,
    request_hash: Hash,
) -> Observation {
    Observation {
        body: ObservationBody {
            feed_id: FeedId::from_str("price_xor_usd").expect("feed id"),
            feed_config_version: FeedConfigVersion(1),
            slot: 10,
            provider_id: provider.clone(),
            connector_id: "http-bin".to_string(),
            connector_version: 2,
            request_hash,
            outcome: ObservationOutcome::Value(value),
            timestamp_ms: Some(1_700_000_000_123),
        },
        signature: zero_signature(),
    }
}

fn follow_feed_config(providers: &[AccountId]) -> FeedConfig {
    FeedConfig {
        feed_id: FeedId::from_str("twitter_follow_binding").expect("feed id"),
        feed_config_version: FeedConfigVersion(1),
        providers: providers.to_vec(),
        connector_id: "twitter-follow".to_string(),
        connector_version: 1,
        cadence_slots: NonZeroU64::new(3).expect("cadence"),
        aggregation: AggregationRule::MedianMad(300),
        outlier_policy: OutlierPolicy::Absolute(AbsoluteOutlier { max_delta: 0 }),
        min_signers: 2,
        committee_size: 2,
        risk_class: RiskClass::Low,
        max_observers: 4,
        max_value_len: 96,
        max_error_rate_bps: 250,
        dispute_window_slots: NonZeroU64::new(6).expect("dispute window"),
        replay_window_slots: NonZeroU64::new(3).expect("replay window"),
    }
}

fn follow_connector_request(
    feed_config_version: FeedConfigVersion,
    keyed_hash: &KeyedHash,
) -> ConnectorRequest {
    let mut headers = BTreeMap::new();
    headers.insert(
        "user-agent".to_string(),
        iroha_data_model::oracle::RedactedHeaderValue::Plain("soracles/1.0".to_string()),
    );
    headers.insert(
        "x-api-key".to_string(),
        iroha_data_model::oracle::RedactedHeaderValue::Hashed(Hash::new(b"twitter-api-key")),
    );
    headers.insert(
        "x-pepper-id".to_string(),
        iroha_data_model::oracle::RedactedHeaderValue::Plain(keyed_hash.pepper_id.clone()),
    );
    headers.insert(
        "x-challenge-sig".to_string(),
        iroha_data_model::oracle::RedactedHeaderValue::Hashed(Hash::new(b"follow-challenge:proof")),
    );

    let mut query = BTreeMap::new();
    query.insert(
        "user_keyed_hash".to_string(),
        hash_literal(&keyed_hash.digest),
    );

    ConnectorRequest {
        feed_id: FeedId::from_str("twitter_follow_binding").expect("feed id"),
        feed_config_version,
        slot: 24,
        connector_id: "twitter-follow".to_string(),
        connector_version: 1,
        method: ConnectorRequestMethod::Get,
        endpoint: "/v1/twitter/follow".to_string(),
        query,
        headers,
        body_hash: Hash::new(b""),
    }
}

fn follow_observation(
    provider: &AccountId,
    value: ObservationValue,
    request_hash: Hash,
    timestamp_ms: Option<u64>,
) -> Observation {
    Observation {
        body: ObservationBody {
            feed_id: FeedId::from_str("twitter_follow_binding").expect("feed id"),
            feed_config_version: FeedConfigVersion(1),
            slot: 24,
            provider_id: provider.clone(),
            connector_id: "twitter-follow".to_string(),
            connector_version: 1,
            request_hash,
            outcome: ObservationOutcome::Value(value),
            timestamp_ms,
        },
        signature: zero_signature(),
    }
}

fn zero_signature<T>() -> SignatureOf<T> {
    let signature =
        Signature::from_hex("00".repeat(128)).expect("generate deterministic zero signature");
    SignatureOf::from_signature(signature)
}

fn hash_literal(hash: &Hash) -> String {
    literal::format("hash", &hex::encode_upper(hash.as_ref()))
}
