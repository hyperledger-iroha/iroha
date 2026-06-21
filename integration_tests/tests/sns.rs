#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! SNS registrar integration coverage.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, TryRecvError},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eyre::{Result, WrapErr, bail, eyre};
use integration_tests::sandbox::{self, start_network_async_or_skip};
use iroha::{client::Client as IrohaClient, sns::SnsNamespacePath};
use iroha_data_model::{
    account::{AccountAddress, AccountId},
    metadata::Metadata,
    sns::{
        DOMAIN_NAME_SUFFIX_ID, FreezeNameRequestV1, GovernanceHookV1, NameControllerV1,
        NameRecordV1, NameSelectorV1, NameStatus, PaymentProofV1, RegisterNameRequestV1,
        RenewNameRequestV1, TransferNameRequestV1,
    },
};
use iroha_primitives::{json::Json, soradns::derive_gateway_hosts};
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use reqwest::{Client as HttpClient, Url};
use tokio::time::{sleep, timeout};

const METRIC_READY_RETRIES: usize = 60;
const METRIC_RETRY_DELAY_MS: u64 = 250;
const STATUS_READY_ATTEMPTS: usize = 60;
const STATUS_RETRY_DELAY: Duration = Duration::from_millis(250);
const SNS_CLIENT_CALL_TIMEOUT: Duration = Duration::from_secs(180);
const TEST_SNS_LEASE_PAYMENT_NANOS: u64 = 500_000_000;

/// End-to-end registrar flow: register → fetch record → fetch policy.
#[tokio::test]
async fn sns_registrar_round_trip() -> Result<()> {
    let Some(network) = start_sns_network(stringify!(sns_registrar_round_trip)).await? else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let client = network.client();
    let label = unique_label("roundtrip");
    let request = build_register_request(&label)?;
    let response = register_request(&client, request.clone()).await?;
    assert_eq!(
        response.selector.normalized_label(),
        request.selector.normalized_label()
    );
    assert_same_owner_controller(
        &response.owner,
        &request.owner,
        "register response owner should match request owner controller",
    );
    assert!(
        matches!(response.status, NameStatus::Active),
        "new registrations must start in the Active state"
    );

    let literal = request.selector.normalized_label().to_owned();
    let fetched = get_sns_name(&client, &literal).await?;
    assert_eq!(fetched.name_hash, response.name_hash);
    assert_same_owner_controller(
        &fetched.owner,
        &request.owner,
        "fetched owner should preserve request owner controller",
    );

    let policy = get_sns_policy(&client, request.selector.suffix_id).await?;
    assert_eq!(policy.suffix_key(), "domain");

    Ok(())
}

/// Freeze/unfreeze flow publishes lifecycle transitions for guardians/council.
#[tokio::test]
async fn sns_freeze_unfreeze_lifecycle() -> Result<()> {
    let Some(network) = start_sns_network(stringify!(sns_freeze_unfreeze_lifecycle)).await? else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let client = network.client();
    let label = unique_label("freeze");
    let literal = domain_literal(&label);
    register_name(&client, &label).await?;

    let freeze = FreezeNameRequestV1 {
        reason: "compliance review".to_string(),
        until_ms: now_millis() + 60_000,
        guardian_ticket: Json::from("guardian-ticket"),
    };
    let freeze_until_ms = freeze.until_ms;
    freeze_name(&client, &literal, freeze).await?;
    let frozen = wait_for_status(&client, &literal, |status| {
        matches!(status, NameStatus::Frozen(_))
    })
    .await?;
    match frozen.status {
        NameStatus::Frozen(state) => {
            assert!(
                state.reason.contains("compliance"),
                "freeze reason should be recorded"
            );
            assert!(
                state.until_ms >= freeze_until_ms,
                "freeze expiration must propagate"
            );
        }
        other => bail!("expected Frozen status, got {other:?}"),
    }

    let governance = stub_governance_hook();
    unfreeze_name(&client, &literal, governance).await?;
    let active = wait_for_status(&client, &literal, |status| {
        matches!(status, NameStatus::Active)
    })
    .await?;
    assert!(
        matches!(active.status, NameStatus::Active),
        "record should return to Active after unfreeze"
    );
    Ok(())
}

/// Registration increments telemetry metrics and yields deterministic gateway bindings.
#[tokio::test]
async fn sns_registration_emits_metrics_and_gateway_bindings() -> Result<()> {
    let Some(network) = start_sns_network(stringify!(
        sns_registration_emits_metrics_and_gateway_bindings
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let client = network.client();
    let metrics_endpoint = client.torii_url.join("metrics")?;
    let http = HttpClient::new();
    let metric_labels = [("result", "ok"), ("suffix", "domain")];
    let baseline = read_metric_sample(
        &http,
        &metrics_endpoint,
        "sns_registrar_status_total",
        &metric_labels,
    )
    .await?
    .unwrap_or(0.0);

    let label = unique_label("telemetry");
    let literal = domain_literal(&label);
    register_name(&client, &label).await?;

    let mut observed_after = None;
    for _ in 0..METRIC_READY_RETRIES {
        let current = read_metric_sample(
            &http,
            &metrics_endpoint,
            "sns_registrar_status_total",
            &metric_labels,
        )
        .await?;
        if let Some(value) = current.filter(|value| *value >= baseline + 1.0) {
            observed_after = Some(value);
            break;
        }
        sleep(Duration::from_millis(METRIC_RETRY_DELAY_MS)).await;
    }
    let observed_after = observed_after.unwrap_or(baseline);
    assert!(
        observed_after >= baseline + 1.0,
        "sns_registrar_status_total did not advance (baseline {baseline}, observed {observed_after})"
    );

    let bindings = derive_gateway_hosts(&format!("{literal}.domain"))
        .map_err(|err| eyre!("gateway host derivation failed: {err}"))?;
    assert!(
        bindings.canonical_host().ends_with(".gw.sora.id"),
        "canonical host {} must live under gw.sora.id",
        bindings.canonical_host()
    );
    assert!(
        bindings.pretty_host().ends_with(".gw.sora.name"),
        "pretty host {} must live under gw.sora.name",
        bindings.pretty_host()
    );
    assert!(
        bindings.matches_host(bindings.pretty_host()),
        "derived host should match its pretty form"
    );
    assert!(
        bindings.matches_host(bindings.canonical_host()),
        "derived host should match its canonical form"
    );

    Ok(())
}

/// Renewal extends expiry windows and transfers update ownership.
#[tokio::test]
async fn sns_renew_and_transfer_flow() -> Result<()> {
    let Some(network) = start_sns_network(stringify!(sns_renew_and_transfer_flow)).await? else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let client = network.client();
    let label = unique_label("renew-transfer");
    let literal = domain_literal(&label);
    let record = register_name(&client, &label).await?;
    let original_expiry = record.expires_at_ms;
    let policy = get_sns_policy(&client, record.selector.suffix_id).await?;
    let base_price = policy
        .pricing
        .iter()
        .find(|tier| tier.tier_id == record.pricing_class)
        .ok_or_else(|| {
            eyre!(
                "pricing class {} not found in suffix policy {}",
                record.pricing_class,
                record.selector.suffix_id
            )
        })?
        .base_price
        .amount;
    let renew_term_years: u8 = 2;
    let renew_amount = u64::try_from(base_price)
        .map_err(|_| eyre!("base price {base_price} exceeds u64 range"))?
        .saturating_mul(u64::from(renew_term_years));

    let renew_request = RenewNameRequestV1 {
        term_years: renew_term_years,
        payment: stub_payment_proof_with_amount(&record.owner, renew_amount),
    };
    let renewed = renew_name(&client, &literal, renew_request).await?;
    assert!(
        renewed.expires_at_ms > original_expiry,
        "renewal should extend expiry: before {original_expiry}, after {}",
        renewed.expires_at_ms
    );
    assert_eq!(
        renewed.owner, record.owner,
        "renewal must not change ownership"
    );

    let transfer_request = TransferNameRequestV1 {
        new_owner: BOB_ID.clone(),
        governance: stub_governance_hook(),
    };
    let transferred = transfer_name(&client, &literal, transfer_request).await?;
    assert_same_owner_controller(
        &transferred.owner,
        &BOB_ID,
        "transfer should reassign ownership to Bob's controller",
    );

    let fetched = wait_for_record(&client, &literal, |record| {
        record.owner.controller() == BOB_ID.controller()
    })
    .await?;
    assert_same_owner_controller(
        &fetched.owner,
        &BOB_ID,
        "persisted record should reflect Bob's controller after transfer",
    );
    assert!(
        fetched.expires_at_ms >= renewed.expires_at_ms,
        "transfer must not shorten the renewed expiry window"
    );

    Ok(())
}

fn build_register_request(label: &str) -> Result<RegisterNameRequestV1> {
    let selector = NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, domain_literal(label))
        .map_err(|err| eyre!("invalid selector: {err}"))?;
    let owner = ALICE_ID.clone();
    let controller_address = AccountAddress::from_account_id(&owner)
        .map_err(|err| eyre!("failed to encode account address: {err}"))?;

    Ok(RegisterNameRequestV1 {
        selector,
        owner: owner.clone(),
        controllers: vec![NameControllerV1::account(&controller_address)],
        term_years: 1,
        pricing_class_hint: None,
        payment: stub_payment_proof(&owner),
        governance: None,
        metadata: Metadata::default(),
    })
}

fn stub_payment_proof(payer: &AccountId) -> PaymentProofV1 {
    stub_payment_proof_with_amount(payer, TEST_SNS_LEASE_PAYMENT_NANOS)
}

fn stub_payment_proof_with_amount(payer: &AccountId, amount: u64) -> PaymentProofV1 {
    PaymentProofV1 {
        asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
        gross_amount: amount,
        net_amount: amount,
        settlement_tx: Json::from("mock-settlement"),
        payer: payer.clone(),
        signature: Json::from("mock-signature"),
    }
}

async fn register_name(client: &IrohaClient, label: &str) -> Result<NameRecordV1> {
    register_request(client, build_register_request(label)?).await
}

async fn register_request(
    client: &IrohaClient,
    request: RegisterNameRequestV1,
) -> Result<NameRecordV1> {
    let client = client.clone();
    run_sns_client_call("register SNS name", move || {
        let response = client.sns().register(&request)?;
        Ok(response.name_record)
    })
    .await
}

async fn freeze_name(
    client: &IrohaClient,
    literal: &str,
    request: FreezeNameRequestV1,
) -> Result<NameRecordV1> {
    let client = client.clone();
    let literal = literal.to_owned();
    run_sns_client_call("freeze SNS name", move || {
        client
            .sns()
            .freeze(SnsNamespacePath::Domain, &literal, &request)
    })
    .await
}

async fn unfreeze_name(
    client: &IrohaClient,
    literal: &str,
    governance: GovernanceHookV1,
) -> Result<NameRecordV1> {
    let client = client.clone();
    let literal = literal.to_owned();
    run_sns_client_call("unfreeze SNS name", move || {
        client
            .sns()
            .unfreeze(SnsNamespacePath::Domain, &literal, &governance)
    })
    .await
}

async fn renew_name(
    client: &IrohaClient,
    literal: &str,
    request: RenewNameRequestV1,
) -> Result<NameRecordV1> {
    let client = client.clone();
    let literal = literal.to_owned();
    run_sns_client_call("renew SNS name", move || {
        client
            .sns()
            .renew(SnsNamespacePath::Domain, &literal, &request)
    })
    .await
}

async fn transfer_name(
    client: &IrohaClient,
    literal: &str,
    request: TransferNameRequestV1,
) -> Result<NameRecordV1> {
    let client = client.clone();
    let literal = literal.to_owned();
    run_sns_client_call("transfer SNS name", move || {
        client
            .sns()
            .transfer(SnsNamespacePath::Domain, &literal, &request)
    })
    .await
}

async fn get_sns_name(client: &IrohaClient, literal: &str) -> Result<NameRecordV1> {
    let client = client.clone();
    let literal = literal.to_owned();
    run_sns_client_call("get SNS name", move || {
        client.sns().get_name(SnsNamespacePath::Domain, &literal)
    })
    .await
}

async fn get_sns_policy(
    client: &IrohaClient,
    suffix_id: u16,
) -> Result<iroha_data_model::sns::SuffixPolicyV1> {
    let client = client.clone();
    run_sns_client_call("get SNS policy", move || client.sns().get_policy(suffix_id)).await
}

async fn run_sns_client_call<T, F>(operation: &'static str, call: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name(format!("sns-client-{operation}"))
        .spawn(move || {
            let _ = sender.send(call());
        })
        .map_err(|err| eyre!("failed to spawn SNS client operation `{operation}`: {err}"))?;

    timeout(SNS_CLIENT_CALL_TIMEOUT, async move {
        loop {
            match receiver.try_recv() {
                Ok(result) => return result,
                Err(TryRecvError::Empty) => sleep(Duration::from_millis(50)).await,
                Err(TryRecvError::Disconnected) => {
                    return Err(eyre!(
                        "SNS client operation `{operation}` exited without a result"
                    ));
                }
            }
        }
    })
    .await
    .map_err(|_| {
        eyre!(
            "timed out waiting for SNS client operation `{operation}` after {:?}",
            SNS_CLIENT_CALL_TIMEOUT
        )
    })?
}

fn domain_literal(label: &str) -> String {
    format!("{label}.universal")
}

async fn start_sns_network(test_name: &str) -> Result<Option<sandbox::SerializedNetwork>> {
    start_network_async_or_skip(NetworkBuilder::new(), test_name).await
}

async fn wait_for_status<F>(
    client: &IrohaClient,
    literal: &str,
    predicate: F,
) -> Result<NameRecordV1>
where
    F: Fn(&NameStatus) -> bool,
{
    wait_for_record(client, literal, |record| predicate(&record.status)).await
}

async fn wait_for_record<F>(
    client: &IrohaClient,
    literal: &str,
    predicate: F,
) -> Result<NameRecordV1>
where
    F: Fn(&NameRecordV1) -> bool,
{
    for _ in 0..STATUS_READY_ATTEMPTS {
        let record = get_sns_name(client, literal).await?;
        if predicate(&record) {
            return Ok(record);
        }
        sleep(STATUS_RETRY_DELAY).await;
    }
    bail!("registration `{literal}` did not reach expected state");
}

fn stub_governance_hook() -> GovernanceHookV1 {
    GovernanceHookV1 {
        proposal_id: "governance-001".to_string(),
        council_vote_hash: Json::from("council-hash"),
        dao_vote_hash: Json::from("dao-hash"),
        steward_ack: Json::from("steward-ack"),
        guardian_clearance: Some(Json::from("guardian-clearance")),
    }
}

fn unique_label(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let next = COUNTER.fetch_add(1, Ordering::Relaxed);
    let normalized_prefix: String = prefix
        .chars()
        .map(|ch| ch.to_ascii_lowercase())
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect();
    let normalized_prefix = if normalized_prefix.is_empty() {
        "sns"
    } else {
        normalized_prefix.as_str()
    };
    format!("{normalized_prefix}{next}")
}

#[test]
fn unique_label_matches_default_pricing_constraints() {
    let label = unique_label("renew-transfer");
    assert!(
        label.len() >= 3,
        "generated labels must satisfy min length: `{label}`"
    );
    assert!(
        label
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit()),
        "generated labels must stay within [a-z0-9]: `{label}`"
    );
}

#[tokio::test]
async fn bounded_sns_client_call_returns_completed_result() -> Result<()> {
    let value =
        run_sns_client_call("test immediate result", || Ok::<_, eyre::Report>(42_u8)).await?;
    assert_eq!(value, 42);
    Ok(())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn assert_same_owner_controller(actual: &AccountId, expected: &AccountId, context: &str) {
    assert_eq!(
        actual.controller(),
        expected.controller(),
        "{context}; expected owner controller `{expected}`, got `{actual}`"
    );
}

async fn read_metric_sample(
    http: &HttpClient,
    endpoint: &Url,
    metric_name: &str,
    labels: &[(&str, &str)],
) -> Result<Option<f64>> {
    let response = http
        .get(endpoint.clone())
        .send()
        .await?
        .error_for_status()
        .wrap_err("metrics endpoint returned error")?;
    let body = response.text().await?;
    Ok(parse_metric_value(&body, metric_name, labels))
}

fn parse_metric_value(body: &str, metric_name: &str, labels: &[(&str, &str)]) -> Option<f64> {
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || !line.starts_with(metric_name) {
            continue;
        }
        let remainder = &line[metric_name.len()..];
        let (label_map, value_str) = if let Some(rest) = remainder.strip_prefix('{') {
            let end = rest.find('}')?;
            let label_segment = &rest[..end];
            let map = parse_label_map(label_segment);
            (map, rest[end + 1..].trim())
        } else {
            (HashMap::new(), remainder.trim())
        };
        if labels.iter().all(|(key, value)| {
            label_map
                .get(*key)
                .map(String::as_str)
                .is_some_and(|current| current == *value)
        }) {
            let value_token = value_str.split_whitespace().next().unwrap_or_default();
            if let Ok(parsed) = value_token.parse::<f64>() {
                return Some(parsed);
            }
        }
    }
    None
}

fn parse_label_map(segment: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for entry in segment.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.splitn(2, '=');
        let key = parts.next().unwrap_or_default().trim();
        let raw_value = parts.next().unwrap_or_default().trim();
        let cleaned = raw_value.trim_matches('"').replace("\\\"", "\"");
        if !key.is_empty() {
            map.insert(key.to_string(), cleaned);
        }
    }
    map
}
