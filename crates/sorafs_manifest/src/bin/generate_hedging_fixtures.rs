//! Generates deterministic SoraFS hedging and billing fixtures.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use hex::encode;
use norito::{
    core::NoritoSerialize,
    json::{Map, Value, to_string_pretty},
};
use sorafs_manifest::{
    BillingLineDirectionV1, BillingLineItemKindV1, BillingLineItemV1, BillingStatementV1,
    HEDGING_PRICE_FEED_VERSION_V1, HedgingFeedStatusV1, HedgingPriceFeedV1,
    HedgingReferencePriceDecisionV1, MICRO_XOR_PER_XOR, XorAmount, billing_line_item_id_v1,
    billing_statement_id_v1, build_billing_line_item_v1, build_billing_statement_v1,
    derive_reference_price_decision_v1, reference_price_decision_id_v1,
};

fn main() -> Result<(), Box<dyn Error>> {
    let fixture_dir = PathBuf::from("fixtures/sorafs_manifest/hedging");
    let negative_dir = fixture_dir.join("negative");
    fs::create_dir_all(&fixture_dir)?;
    fs::create_dir_all(&negative_dir)?;

    let primary_feed = feed(
        "xor-usd-primary",
        "governance-primary",
        2_000_000,
        1_700_604_700,
        6_000,
        HedgingFeedStatusV1::Ok,
    );
    let secondary_feed = feed(
        "xor-usd-secondary",
        "market-secondary",
        2_050_000,
        1_700_604_690,
        4_000,
        HedgingFeedStatusV1::Degraded,
    );
    primary_feed.validate()?;
    secondary_feed.validate()?;

    let decision = derive_reference_price_decision_v1(
        1_700_604_800,
        vec![secondary_feed.clone(), primary_feed.clone()],
        300,
        500,
    )?;

    let storage_line = build_billing_line_item_v1(
        BillingLineItemKindV1::Storage,
        BillingLineDirectionV1::Debit,
        "deal:storage:hot:week-2026-01",
        XorAmount::from_micro(10 * MICRO_XOR_PER_XOR),
        decision.xor_usd_micros,
        64 * 7 * 24,
        Some("hot-tier weekly storage".to_owned()),
    )?;
    let egress_line = build_billing_line_item_v1(
        BillingLineItemKindV1::Egress,
        BillingLineDirectionV1::Debit,
        "egress:provider-a:week-2026-01",
        XorAmount::from_micro(MICRO_XOR_PER_XOR / 2),
        decision.xor_usd_micros,
        8_589_934_592,
        Some("8 GiB egress".to_owned()),
    )?;
    let incentive_line = build_billing_line_item_v1(
        BillingLineItemKindV1::IncentiveCredit,
        BillingLineDirectionV1::Credit,
        "incentive:provider-a:uptime-2026-01",
        XorAmount::from_micro(MICRO_XOR_PER_XOR),
        decision.xor_usd_micros,
        1,
        None,
    )?;

    let statement = build_billing_statement_v1(
        b"provider-a".to_vec(),
        1_700_000_000,
        1_700_604_800,
        1_700_691_200,
        decision.clone(),
        vec![
            storage_line.clone(),
            egress_line.clone(),
            incentive_line.clone(),
        ],
        Some(digest("sorafs.billing.previous.statement")),
    )?;

    write_norito_pair(
        &fixture_dir.join("price_feed_primary_v1"),
        &primary_feed,
        price_feed_json(&primary_feed),
    )?;
    write_norito_pair(
        &fixture_dir.join("price_feed_secondary_v1"),
        &secondary_feed,
        price_feed_json(&secondary_feed),
    )?;
    write_norito_pair(
        &fixture_dir.join("reference_price_decision_v1"),
        &decision,
        reference_price_decision_json(&decision),
    )?;
    write_norito_pair(
        &fixture_dir.join("billing_line_storage_v1"),
        &storage_line,
        billing_line_json(&storage_line),
    )?;
    write_norito_pair(
        &fixture_dir.join("billing_line_egress_v1"),
        &egress_line,
        billing_line_json(&egress_line),
    )?;
    write_norito_pair(
        &fixture_dir.join("billing_line_incentive_credit_v1"),
        &incentive_line,
        billing_line_json(&incentive_line),
    )?;
    write_norito_pair(
        &fixture_dir.join("billing_statement_v1"),
        &statement,
        billing_statement_json(&statement),
    )?;

    let mut stale_decision = decision.clone();
    stale_decision.feeds[0].observed_at_unix = 1_700_000_000;
    stale_decision.decision_id = reference_price_decision_id_v1(&stale_decision)?;
    assert!(stale_decision.validate().is_err());
    write_norito_pair(
        &negative_dir.join("stale_reference_price_decision_v1"),
        &stale_decision,
        reference_price_decision_json(&stale_decision),
    )?;

    let mut tampered_line = storage_line.clone();
    tampered_line.usd_micros += 1;
    tampered_line.line_id = billing_line_item_id_v1(&tampered_line)?;
    assert!(tampered_line.validate().is_ok());
    let mut tampered_statement = statement.clone();
    tampered_statement.lines[0] = tampered_line;
    tampered_statement.total_debit_usd_micros += 1;
    tampered_statement.net_due_usd_micros += 1;
    tampered_statement.statement_id = billing_statement_id_v1(&tampered_statement)?;
    assert!(tampered_statement.validate().is_err());
    write_norito_pair(
        &negative_dir.join("line_usd_mismatch_statement_v1"),
        &tampered_statement,
        billing_statement_json(&tampered_statement),
    )?;

    let mut tampered_totals = statement;
    tampered_totals.total_debit_usd_micros += 1;
    assert!(tampered_totals.validate().is_err());
    write_norito_pair(
        &negative_dir.join("tampered_totals_statement_v1"),
        &tampered_totals,
        billing_statement_json(&tampered_totals),
    )?;

    Ok(())
}

fn feed(
    feed_id: &str,
    source: &str,
    xor_usd_micros: u64,
    observed_at_unix: u64,
    weight_bps: u16,
    status: HedgingFeedStatusV1,
) -> HedgingPriceFeedV1 {
    HedgingPriceFeedV1 {
        version: HEDGING_PRICE_FEED_VERSION_V1,
        feed_id: feed_id.to_owned(),
        source: source.to_owned(),
        observed_at_unix,
        xor_usd_micros,
        weight_bps,
        evidence_digest: digest(feed_id),
        status,
    }
}

fn digest(label: &str) -> [u8; 32] {
    *blake3::hash(label.as_bytes()).as_bytes()
}

fn write_norito_pair<T>(
    base_path: &Path,
    value: &T,
    mut json_value: Value,
) -> Result<(), Box<dyn Error>>
where
    T: NoritoSerialize,
{
    let bytes = norito::to_bytes(value)?;
    fs::write(base_path.with_extension("to"), &bytes)?;
    if let Value::Object(map) = &mut json_value {
        map.insert("norito_bytes_hex".into(), Value::from(encode(&bytes)));
    }
    let json = to_string_pretty(&json_value)?;
    fs::write(base_path.with_extension("json"), json)?;
    Ok(())
}

fn price_feed_json(feed: &HedgingPriceFeedV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(feed.version));
    map.insert("feed_id".into(), Value::from(feed.feed_id.clone()));
    map.insert("source".into(), Value::from(feed.source.clone()));
    map.insert(
        "observed_at_unix".into(),
        Value::from(feed.observed_at_unix),
    );
    map.insert("xor_usd_micros".into(), Value::from(feed.xor_usd_micros));
    map.insert("weight_bps".into(), Value::from(feed.weight_bps));
    map.insert(
        "evidence_digest_hex".into(),
        Value::from(encode(feed.evidence_digest)),
    );
    map.insert("status".into(), Value::from(feed_status(feed.status)));
    Value::Object(map)
}

fn reference_price_decision_json(decision: &HedgingReferencePriceDecisionV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(decision.version));
    map.insert(
        "decision_id_hex".into(),
        Value::from(encode(decision.decision_id)),
    );
    map.insert(
        "effective_at_unix".into(),
        Value::from(decision.effective_at_unix),
    );
    map.insert(
        "xor_usd_micros".into(),
        Value::from(decision.xor_usd_micros),
    );
    map.insert(
        "max_feed_age_secs".into(),
        Value::from(decision.max_feed_age_secs),
    );
    map.insert(
        "max_divergence_bps".into(),
        Value::from(decision.max_divergence_bps),
    );
    map.insert(
        "feeds".into(),
        Value::Array(decision.feeds.iter().map(price_feed_json).collect()),
    );
    map.insert("degraded".into(), Value::from(decision.degraded));
    map.insert(
        "degradation_reasons".into(),
        Value::Array(
            decision
                .degradation_reasons
                .iter()
                .cloned()
                .map(Value::from)
                .collect(),
        ),
    );
    Value::Object(map)
}

fn billing_line_json(line: &BillingLineItemV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(line.version));
    map.insert("line_id_hex".into(), Value::from(encode(line.line_id)));
    map.insert("kind".into(), Value::from(line_kind(line.kind)));
    map.insert(
        "direction".into(),
        Value::from(line_direction(line.direction)),
    );
    map.insert("source_id".into(), Value::from(line.source_id.clone()));
    map.insert(
        "xor_amount_micro".into(),
        Value::from(line.xor_amount.as_micro().to_string()),
    );
    map.insert(
        "usd_micros".into(),
        Value::from(line.usd_micros.to_string()),
    );
    map.insert(
        "quantity_units".into(),
        Value::from(line.quantity_units.to_string()),
    );
    match &line.note {
        Some(note) => map.insert("note".into(), Value::from(note.clone())),
        None => map.insert("note".into(), Value::Null),
    };
    Value::Object(map)
}

fn billing_statement_json(statement: &BillingStatementV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(statement.version));
    map.insert(
        "statement_id_hex".into(),
        Value::from(encode(statement.statement_id)),
    );
    map.insert(
        "account_id".into(),
        Value::from(String::from_utf8_lossy(&statement.account_id).to_string()),
    );
    map.insert(
        "account_id_hex".into(),
        Value::from(encode(&statement.account_id)),
    );
    map.insert(
        "period_start_unix".into(),
        Value::from(statement.period_start_unix),
    );
    map.insert(
        "period_end_unix".into(),
        Value::from(statement.period_end_unix),
    );
    map.insert("due_at_unix".into(), Value::from(statement.due_at_unix));
    map.insert(
        "reference_price".into(),
        reference_price_decision_json(&statement.reference_price),
    );
    map.insert(
        "lines".into(),
        Value::Array(statement.lines.iter().map(billing_line_json).collect()),
    );
    map.insert(
        "total_debit_xor_micro".into(),
        Value::from(statement.total_debit_xor.as_micro().to_string()),
    );
    map.insert(
        "total_credit_xor_micro".into(),
        Value::from(statement.total_credit_xor.as_micro().to_string()),
    );
    map.insert(
        "net_due_xor_micro".into(),
        Value::from(statement.net_due_xor.as_micro().to_string()),
    );
    map.insert(
        "total_debit_usd_micros".into(),
        Value::from(statement.total_debit_usd_micros.to_string()),
    );
    map.insert(
        "total_credit_usd_micros".into(),
        Value::from(statement.total_credit_usd_micros.to_string()),
    );
    map.insert(
        "net_due_usd_micros".into(),
        Value::from(statement.net_due_usd_micros.to_string()),
    );
    match statement.previous_statement_id {
        Some(previous) => map.insert(
            "previous_statement_id_hex".into(),
            Value::from(encode(previous)),
        ),
        None => map.insert("previous_statement_id_hex".into(), Value::Null),
    };
    Value::Object(map)
}

fn feed_status(status: HedgingFeedStatusV1) -> &'static str {
    match status {
        HedgingFeedStatusV1::Ok => "ok",
        HedgingFeedStatusV1::Degraded => "degraded",
        HedgingFeedStatusV1::Rejected => "rejected",
    }
}

fn line_direction(direction: BillingLineDirectionV1) -> &'static str {
    match direction {
        BillingLineDirectionV1::Debit => "debit",
        BillingLineDirectionV1::Credit => "credit",
    }
}

fn line_kind(kind: BillingLineItemKindV1) -> &'static str {
    match kind {
        BillingLineItemKindV1::Storage => "storage",
        BillingLineItemKindV1::Egress => "egress",
        BillingLineItemKindV1::ReserveRent => "reserve_rent",
        BillingLineItemKindV1::SettlementFee => "settlement_fee",
        BillingLineItemKindV1::Penalty => "penalty",
        BillingLineItemKindV1::IncentiveCredit => "incentive_credit",
        BillingLineItemKindV1::Adjustment => "adjustment",
    }
}
