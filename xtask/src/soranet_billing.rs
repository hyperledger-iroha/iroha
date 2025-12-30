//! SN15-M0 billing preview pack for the SoraGlobal Gateway CDN (SN15-M0-9).
//! Generates meter catalog (JSON/CSV/Parquet), rating plan, ledger posting
//! hooks + projection, guardrails, and invoice/dispute/reconciliation templates
//! so billing drills share a deterministic evidence bundle.

use std::{
    collections::BTreeMap,
    fmt::Write,
    fs::{self, File},
    path::{Path, PathBuf},
};

use arrow_array::{ArrayRef, Int64Array, RecordBatch, StringArray, UInt16Array, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use eyre::{Result, WrapErr, eyre};
use norito::{
    derive::JsonSerialize,
    json::{self, JsonSerialize as JsonValueSerialize},
};
use parquet::{arrow::ArrowWriter, basic::Compression, file::properties::WriterProperties};
use serde::Serialize;

const BILLING_CURRENCY: &str = "XOR";
const BILLING_PERIOD_DAYS: u32 = 30;
const BILLING_GRACE_DAYS: u32 = 7;
const COMMIT_DISCOUNT_BPS: u16 = 800;
const PREPAY_CREDIT_BPS: u16 = 250;
const DISPUTE_HOLD_BPS: u16 = 2_000;
const TREASURY_CUT_BPS: u16 = 500;

#[derive(Debug)]
pub struct BillingM0Options {
    pub output_dir: PathBuf,
    pub billing_period: String,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct BillingM0Outcome {
    pub summary_path: PathBuf,
    pub meter_catalog_path: PathBuf,
    pub meter_catalog_csv_path: PathBuf,
    pub meter_catalog_parquet_path: PathBuf,
    pub rating_plan_path: PathBuf,
    pub ledger_hooks_path: PathBuf,
    pub ledger_projection_path: PathBuf,
    pub guardrails_path: PathBuf,
    pub invoice_template_path: PathBuf,
    pub dispute_template_path: PathBuf,
    pub reconciliation_template_path: PathBuf,
}

#[derive(Debug, JsonSerialize)]
struct BillingM0Summary {
    billing_period: String,
    meter_catalog_path: String,
    meter_catalog_csv_path: String,
    meter_catalog_parquet_path: String,
    rating_plan_path: String,
    ledger_hooks_path: String,
    ledger_projection_path: String,
    guardrails_path: String,
    invoice_template_path: String,
    dispute_template_path: String,
    reconciliation_template_path: String,
}

#[derive(Debug, Clone, Copy)]
struct RegionRate {
    id: &'static str,
    name: &'static str,
    multiplier_bps: u16,
    storage_class: &'static str,
}

const REGION_DEFAULTS: &[RegionRate] = &[
    RegionRate {
        id: "NA",
        name: "North America",
        multiplier_bps: 10_000,
        storage_class: "standard",
    },
    RegionRate {
        id: "EU",
        name: "Europe",
        multiplier_bps: 9_500,
        storage_class: "standard",
    },
    RegionRate {
        id: "APAC",
        name: "Asia Pacific",
        multiplier_bps: 11_000,
        storage_class: "standard",
    },
    RegionRate {
        id: "LATAM",
        name: "Latin America",
        multiplier_bps: 10_250,
        storage_class: "regional-cache",
    },
];

const REGION_DOH: &[RegionRate] = &[
    RegionRate {
        id: "NA",
        name: "North America",
        multiplier_bps: 10_000,
        storage_class: "standard",
    },
    RegionRate {
        id: "EU",
        name: "Europe",
        multiplier_bps: 9_800,
        storage_class: "standard",
    },
    RegionRate {
        id: "APAC",
        name: "Asia Pacific",
        multiplier_bps: 10_500,
        storage_class: "standard",
    },
    RegionRate {
        id: "LATAM",
        name: "Latin America",
        multiplier_bps: 10_000,
        storage_class: "regional-cache",
    },
];

#[derive(Debug, Clone, Copy)]
struct PriceTier {
    label: &'static str,
    upto: Option<u64>,
    rate_micros: u64,
}

#[derive(Debug, Clone, Copy)]
struct DiscountTier {
    threshold: u64,
    discount_bps: u16,
}

#[derive(Debug, Clone, Copy)]
struct Guardrail {
    warn_ratio: f64,
    critical_ratio: f64,
    hard_cap: u64,
}

#[derive(Debug, Clone, Copy)]
struct AlertRule {
    id: &'static str,
    signal: &'static str,
    trigger: &'static str,
    route: &'static str,
    note: &'static str,
}

const ALERT_RULES: &[AlertRule] = &[
    AlertRule {
        id: "usage-spike",
        signal: "http.request",
        trigger: "1.20x rolling_1h vs 7d baseline",
        route: "pager:soranet-edge",
        note: "Catch runaway billing before caps trip",
    },
    AlertRule {
        id: "egress-growth",
        signal: "http.egress_gib",
        trigger: "1.15x d/d across two regions",
        route: "email:billing@sora.net",
        note: "Keeps cross-region burn visible before invoicing",
    },
    AlertRule {
        id: "verification-lag",
        signal: "car.verification_ms",
        trigger: "p95 verification_ms > 750 for 15m",
        route: "pager:soranet-edge",
        note: "Trustless verification latency must stay under SLA",
    },
    AlertRule {
        id: "dispute-backlog",
        signal: "open_disputes",
        trigger: "more than 5 open for >48h",
        route: "email:treasury@sora.net",
        note: "Escrow holds should not accumulate silently",
    },
];

#[derive(Debug, Clone)]
struct BaselineMeter {
    id: &'static str,
    category: &'static str,
    unit: &'static str,
    description: &'static str,
    price_micros: u64,
    included_units: u64,
    burst_cap: u64,
    quantum: Option<u64>,
    tiers: &'static [PriceTier],
    discount_tiers: &'static [DiscountTier],
    region_multipliers: &'static [RegionRate],
    dimensions: &'static [&'static str],
    notes: &'static [&'static str],
    guardrail: Guardrail,
}

#[derive(Debug, JsonSerialize)]
struct MeterCatalog {
    version: u8,
    billing_period: String,
    period_days: u32,
    grace_days: u32,
    currency: &'static str,
    meters: Vec<MeterRecord>,
    regions: Vec<RegionRecord>,
    notes: Vec<String>,
}

#[derive(Debug, JsonSerialize)]
struct MeterRecord {
    id: String,
    category: String,
    unit: String,
    description: String,
    price_micros: u64,
    included_units: u64,
    burst_cap: u64,
    quantum: Option<u64>,
    tiers: Vec<MeterTierRecord>,
    discount_tiers: Vec<DiscountTierRecord>,
    region_multipliers_bps: BTreeMap<String, u16>,
    dimensions: Vec<String>,
    notes: Vec<String>,
}

#[derive(Debug, JsonSerialize)]
struct MeterTierRecord {
    label: String,
    upto: Option<u64>,
    rate_micros: u64,
}

#[derive(Debug, JsonSerialize)]
struct DiscountTierRecord {
    threshold: u64,
    discount_bps: u16,
}

#[derive(Debug, JsonSerialize)]
struct RegionRecord {
    id: String,
    name: String,
    billing_multiplier_bps: u16,
    storage_class: String,
}

#[derive(Debug, Serialize)]
struct RatingPlan {
    version: u8,
    billing_period: String,
    currency: &'static str,
    period_days: u32,
    grace_days: u32,
    contract: RatingContract,
    meters: Vec<RateCard>,
    guardrails: RatingGuardrails,
}

#[derive(Debug, Serialize)]
struct RatingContract {
    commit_12mo_discount_bps: u16,
    prepay_credit_bps: u16,
    dispute_hold_bps: u16,
    treasury_cut_bps: u16,
    surge_surcharge_bps: u16,
}

#[derive(Debug, Serialize)]
struct RatingGuardrails {
    warn_utilization_bps: u16,
    critical_utilization_bps: u16,
    alert_routes: Vec<&'static str>,
    ledger_currency: &'static str,
}

#[derive(Debug, Serialize)]
struct RateCard {
    id: String,
    category: String,
    unit: String,
    price_micros: u64,
    included_units: u64,
    burst_cap: u64,
    quantum: Option<u64>,
    tiers: Vec<RateTier>,
    discounts: Vec<RateDiscount>,
    region_multiplier_bps: BTreeMap<String, u16>,
}

#[derive(Debug, Serialize)]
struct RateTier {
    label: String,
    upto: Option<u64>,
    rate_micros: u64,
}

#[derive(Debug, Serialize)]
struct RateDiscount {
    threshold: u64,
    discount_bps: u16,
}

#[derive(Debug, Serialize)]
struct LedgerHooks {
    version: u8,
    billing_period: String,
    currency: &'static str,
    accounts: LedgerAccounts,
    posting_rules: PostingRules,
    guardrails: LedgerGuardrails,
}

#[derive(Debug, Serialize)]
struct LedgerAccounts {
    receivable: String,
    revenue: String,
    escrow: String,
    refunds: String,
    treasury: String,
}

#[derive(Debug, Serialize)]
struct PostingRules {
    invoice: PostingRule,
    payment: PostingRule,
    dispute_hold: PostingRule,
    dispute_release: PostingRule,
    refund: PostingRule,
    treasury: PostingRule,
}

#[derive(Debug, Serialize)]
struct PostingRule {
    debit: String,
    credit: String,
    memo: &'static str,
}

#[derive(Debug, Serialize)]
struct LedgerGuardrails {
    dispute_hold_percent_bps: u16,
    max_credit_memos_per_period: u8,
    prepay_required_bps: u16,
    treasury_cut_bps: u16,
}

#[derive(Debug, JsonSerialize)]
struct LedgerProjection {
    billing_period: String,
    currency: &'static str,
    usage: Vec<UsageCharge>,
    totals: LedgerTotals,
    transfers: Vec<LedgerTransfer>,
    notes: Vec<String>,
}

#[derive(Debug, JsonSerialize)]
struct UsageCharge {
    meter_id: String,
    region: String,
    units: u64,
    effective_rate_micros: u64,
    discount_bps: u16,
    amount_micros: u64,
}

#[derive(Debug, JsonSerialize)]
struct LedgerTotals {
    gross_micros: u64,
    discounts_micros: u64,
    dispute_hold_micros: u64,
    treasury_cut_micros: u64,
    net_micros: u64,
}

#[derive(Debug, JsonSerialize)]
struct LedgerTransfer {
    from: String,
    to: String,
    amount_micros: u64,
    memo: String,
}

#[derive(Debug, Clone)]
struct ExportRow {
    meter_id: String,
    category: String,
    unit: String,
    tier_label: String,
    region: String,
    base_rate_micros: u64,
    effective_rate_micros: u64,
    included_units: u64,
    burst_cap: u64,
    quantum: Option<i64>,
    max_discount_bps: u16,
}

#[derive(Debug, Clone)]
struct UsageSample {
    meter_id: &'static str,
    region: &'static str,
    quantity: u64,
}

/// Write the billing preview pack to disk and return the summary paths.
pub fn write_billing_m0_pack(options: BillingM0Options) -> Result<BillingM0Outcome> {
    let BillingM0Options {
        output_dir,
        billing_period,
    } = options;
    fs::create_dir_all(&output_dir).wrap_err_with(|| {
        format!(
            "failed to create billing M0 output directory `{}`",
            output_dir.display()
        )
    })?;

    let meter_catalog_path = output_dir.join("billing_meter_catalog.json");
    let meter_catalog_csv_path = output_dir.join("billing_meter_catalog.csv");
    let meter_catalog_parquet_path = output_dir.join("billing_meter_catalog.parquet");
    let rating_plan_path = output_dir.join("billing_rating_plan.toml");
    let ledger_hooks_path = output_dir.join("billing_ledger_hooks.toml");
    let ledger_projection_path = output_dir.join("billing_ledger_projection.json");
    let guardrails_path = output_dir.join("billing_guardrails.yaml");
    let invoice_template_path = output_dir.join("billing_invoice_template.md");
    let dispute_template_path = output_dir.join("billing_dispute_template.md");
    let reconciliation_template_path = output_dir.join("billing_reconciliation_template.md");
    let summary_path = output_dir.join("billing_m0_summary.json");
    let billing_period = billing_period.trim().to_string();

    let meters = baseline_meters();
    let regions = baseline_regions();

    let catalog = meter_catalog(&billing_period, &meters, regions);
    write_json_pretty(&catalog, &meter_catalog_path)
        .wrap_err("failed to write billing meter catalog JSON")?;

    let export_rows = meter_export_rows(&meters, regions)?;
    write_meter_csv(&export_rows, &meter_catalog_csv_path)
        .wrap_err("failed to write meter catalog CSV export")?;
    write_meter_parquet(&export_rows, &meter_catalog_parquet_path)
        .wrap_err("failed to write meter catalog Parquet export")?;

    let rating_plan = rating_plan(&billing_period, &meters, regions);
    let rating_toml =
        toml::to_string_pretty(&rating_plan).wrap_err("failed to render billing rating plan")?;
    fs::write(&rating_plan_path, rating_toml)
        .wrap_err_with(|| format!("failed to write rating plan {}", rating_plan_path.display()))?;

    let ledger_hooks = ledger_hooks(&billing_period);
    let ledger_hooks_toml =
        toml::to_string_pretty(&ledger_hooks).wrap_err("failed to render ledger hooks")?;
    fs::write(&ledger_hooks_path, ledger_hooks_toml).wrap_err_with(|| {
        format!(
            "failed to write ledger hooks {}",
            ledger_hooks_path.display()
        )
    })?;

    let projection = ledger_projection(&billing_period, &meters, regions)?;
    write_json_pretty(&projection, &ledger_projection_path).wrap_err_with(|| {
        format!(
            "failed to write ledger projection {}",
            ledger_projection_path.display()
        )
    })?;

    let guardrails = render_guardrails_yaml(&billing_period, &meters)?;
    fs::write(&guardrails_path, guardrails).wrap_err_with(|| {
        format!(
            "failed to write billing guardrails {}",
            guardrails_path.display()
        )
    })?;

    fs::write(
        &invoice_template_path,
        render_invoice_template(&billing_period),
    )
    .wrap_err_with(|| {
        format!(
            "failed to write invoice template {}",
            invoice_template_path.display()
        )
    })?;
    fs::write(
        &dispute_template_path,
        render_dispute_template(&billing_period),
    )
    .wrap_err_with(|| {
        format!(
            "failed to write dispute template {}",
            dispute_template_path.display()
        )
    })?;
    fs::write(
        &reconciliation_template_path,
        render_reconciliation_template(&billing_period),
    )
    .wrap_err_with(|| {
        format!(
            "failed to write reconciliation template {}",
            reconciliation_template_path.display()
        )
    })?;

    let summary = BillingM0Summary {
        billing_period,
        meter_catalog_path: relative_display(&output_dir, &meter_catalog_path),
        meter_catalog_csv_path: relative_display(&output_dir, &meter_catalog_csv_path),
        meter_catalog_parquet_path: relative_display(&output_dir, &meter_catalog_parquet_path),
        rating_plan_path: relative_display(&output_dir, &rating_plan_path),
        ledger_hooks_path: relative_display(&output_dir, &ledger_hooks_path),
        ledger_projection_path: relative_display(&output_dir, &ledger_projection_path),
        guardrails_path: relative_display(&output_dir, &guardrails_path),
        invoice_template_path: relative_display(&output_dir, &invoice_template_path),
        dispute_template_path: relative_display(&output_dir, &dispute_template_path),
        reconciliation_template_path: relative_display(&output_dir, &reconciliation_template_path),
    };
    let file = File::create(&summary_path).wrap_err_with(|| {
        format!(
            "failed to create billing M0 summary {}",
            summary_path.display()
        )
    })?;
    json::to_writer_pretty(file, &summary).wrap_err_with(|| {
        format!(
            "failed to write billing M0 summary {}",
            summary_path.display()
        )
    })?;

    Ok(BillingM0Outcome {
        summary_path,
        meter_catalog_path,
        meter_catalog_csv_path,
        meter_catalog_parquet_path,
        rating_plan_path,
        ledger_hooks_path,
        ledger_projection_path,
        guardrails_path,
        invoice_template_path,
        dispute_template_path,
        reconciliation_template_path,
    })
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn baseline_regions() -> &'static [RegionRate] {
    REGION_DEFAULTS
}

fn baseline_meters() -> Vec<BaselineMeter> {
    vec![
        BaselineMeter {
            id: "http.request",
            category: "edge",
            unit: "request",
            description: "Ingress HTTP/3 requests processed by the SoraGlobal Gateway data plane.",
            price_micros: 5,
            included_units: 500_000,
            burst_cap: 2_500_000,
            quantum: None,
            tiers: &[],
            discount_tiers: &[
                DiscountTier {
                    threshold: 1_000_000,
                    discount_bps: 500,
                },
                DiscountTier {
                    threshold: 5_000_000,
                    discount_bps: 900,
                },
            ],
            region_multipliers: REGION_DEFAULTS,
            dimensions: &["tenant", "region", "path_class"],
            notes: &[
                "Counts cache hits and misses after WAF policy.",
                "Traffic served through SoraNet and direct edges shares the allowance.",
            ],
            guardrail: Guardrail {
                warn_ratio: 0.85,
                critical_ratio: 0.95,
                hard_cap: 2_500_000,
            },
        },
        BaselineMeter {
            id: "http.egress_gib",
            category: "egress",
            unit: "gibibyte",
            description: "Egress bandwidth served from the gateway cache/origin, measured in GiB.",
            price_micros: 250_000,
            included_units: 0,
            burst_cap: 520,
            quantum: None,
            tiers: &[
                PriceTier {
                    label: "first_100_gib",
                    upto: Some(100),
                    rate_micros: 250_000,
                },
                PriceTier {
                    label: "next_200_gib",
                    upto: Some(300),
                    rate_micros: 225_000,
                },
                PriceTier {
                    label: "over_300_gib",
                    upto: None,
                    rate_micros: 210_000,
                },
            ],
            discount_tiers: &[
                DiscountTier {
                    threshold: 100,
                    discount_bps: 300,
                },
                DiscountTier {
                    threshold: 300,
                    discount_bps: 1_000,
                },
            ],
            region_multipliers: REGION_DEFAULTS,
            dimensions: &["tenant", "region", "cache_tier"],
            notes: &[
                "Rate applies after dedupe/compression and excludes trustless verification burn.",
                "Manifest-driven pulls count against the same meter as cache hits.",
            ],
            guardrail: Guardrail {
                warn_ratio: 0.82,
                critical_ratio: 0.96,
                hard_cap: 520,
            },
        },
        BaselineMeter {
            id: "dns.doh_query",
            category: "dns",
            unit: "request",
            description: "DNS-over-HTTPS/QUIC queries relayed by the gateway resolver layer.",
            price_micros: 3,
            included_units: 200_000,
            burst_cap: 1_000_000,
            quantum: None,
            tiers: &[],
            discount_tiers: &[DiscountTier {
                threshold: 2_000_000,
                discount_bps: 1_000,
            }],
            region_multipliers: REGION_DOH,
            dimensions: &["tenant", "pop", "resolver_path"],
            notes: &[
                "Serve-stale responses are billed once per stale window.",
                "DoH/DoT/DoQ/ODoH share the same allowance.",
            ],
            guardrail: Guardrail {
                warn_ratio: 0.82,
                critical_ratio: 0.95,
                hard_cap: 1_000_000,
            },
        },
        BaselineMeter {
            id: "waf.decision",
            category: "security",
            unit: "decision",
            description: "WAF enforcement decisions (block/challenge/allow) evaluated on ingress traffic.",
            price_micros: 20,
            included_units: 10_000,
            burst_cap: 240_000,
            quantum: None,
            tiers: &[],
            discount_tiers: &[],
            region_multipliers: &[RegionRate {
                id: "GLOBAL",
                name: "Global",
                multiplier_bps: 10_000,
                storage_class: "standard",
            }],
            dimensions: &["tenant", "rule_id"],
            notes: &[
                "Monitoring-only rules are zero rated.",
                "Duplicate rule_ids across tenants are considered independent meters.",
            ],
            guardrail: Guardrail {
                warn_ratio: 0.8,
                critical_ratio: 0.95,
                hard_cap: 240_000,
            },
        },
        BaselineMeter {
            id: "car.verification_ms",
            category: "trustless",
            unit: "ms",
            description: "Aggregated milliseconds spent verifying trustless CAR payloads (Merkle/KZG/SDR).",
            price_micros: 10,
            included_units: 80_000,
            burst_cap: 1_200_000,
            quantum: Some(10),
            tiers: &[],
            discount_tiers: &[DiscountTier {
                threshold: 500_000,
                discount_bps: 1_500,
            }],
            region_multipliers: &[RegionRate {
                id: "GLOBAL",
                name: "Global",
                multiplier_bps: 10_000,
                storage_class: "standard",
            }],
            dimensions: &["tenant", "proof_kind"],
            notes: &[
                "Meter ticks in 10 ms quantums to align with CPU buckets.",
                "Cache-binding checks and proof verification share the same meter.",
            ],
            guardrail: Guardrail {
                warn_ratio: 0.85,
                critical_ratio: 0.97,
                hard_cap: 1_200_000,
            },
        },
        BaselineMeter {
            id: "storage.gib_month",
            category: "storage",
            unit: "gib-month",
            description: "Pinned storage billed in GiB-month with SoraNS anchor retention.",
            price_micros: 180_000,
            included_units: 20,
            burst_cap: 600,
            quantum: None,
            tiers: &[],
            discount_tiers: &[
                DiscountTier {
                    threshold: 50,
                    discount_bps: 500,
                },
                DiscountTier {
                    threshold: 200,
                    discount_bps: 1_000,
                },
            ],
            region_multipliers: REGION_DEFAULTS,
            dimensions: &["tenant", "storage_class"],
            notes: &[
                "Retention class tags must match SoraNS anchors.",
                "Billing rolls forward based on mid-cycle usage snapshots.",
            ],
            guardrail: Guardrail {
                warn_ratio: 0.8,
                critical_ratio: 0.95,
                hard_cap: 600,
            },
        },
    ]
}

fn meter_catalog(
    billing_period: &str,
    meters: &[BaselineMeter],
    regions: &[RegionRate],
) -> MeterCatalog {
    let meter_records = meters
        .iter()
        .map(|meter| MeterRecord {
            id: meter.id.to_string(),
            category: meter.category.to_string(),
            unit: meter.unit.to_string(),
            description: meter.description.to_string(),
            price_micros: meter.price_micros,
            included_units: meter.included_units,
            burst_cap: meter.burst_cap,
            quantum: meter.quantum,
            tiers: tiers_for_record(meter),
            discount_tiers: meter
                .discount_tiers
                .iter()
                .map(|tier| DiscountTierRecord {
                    threshold: tier.threshold,
                    discount_bps: tier.discount_bps,
                })
                .collect(),
            region_multipliers_bps: region_map_for_meter(meter, regions),
            dimensions: meter.dimensions.iter().map(|d| d.to_string()).collect(),
            notes: meter.notes.iter().map(|note| note.to_string()).collect(),
        })
        .collect();
    let region_records = regions
        .iter()
        .map(|region| RegionRecord {
            id: region.id.to_string(),
            name: region.name.to_string(),
            billing_multiplier_bps: region.multiplier_bps,
            storage_class: region.storage_class.to_string(),
        })
        .collect();
    MeterCatalog {
        version: 2,
        billing_period: billing_period.to_string(),
        period_days: BILLING_PERIOD_DAYS,
        grace_days: BILLING_GRACE_DAYS,
        currency: BILLING_CURRENCY,
        meters: meter_records,
        regions: region_records,
        notes: vec![
            "Basis points (bps) use 10_000 = 1.0 multiplier to avoid float rounding.".to_string(),
            "Meters assume anonymised SoraNet transport unless an explicit direct mode is declared in the manifest."
                .to_string(),
            "CAR verification time is measured per request after cache binding, not per chunk."
                .to_string(),
            "CSV/Parquet exports flatten tiers x regions for deterministic reconciliation."
                .to_string(),
        ],
    }
}

fn rating_plan(
    billing_period: &str,
    meters: &[BaselineMeter],
    regions: &[RegionRate],
) -> RatingPlan {
    let meters = meters
        .iter()
        .map(|meter| RateCard {
            id: meter.id.to_string(),
            category: meter.category.to_string(),
            unit: meter.unit.to_string(),
            price_micros: meter.price_micros,
            included_units: meter.included_units,
            burst_cap: meter.burst_cap,
            quantum: meter.quantum,
            tiers: tiers_for_record(meter)
                .into_iter()
                .map(|tier| RateTier {
                    label: tier.label,
                    upto: tier.upto,
                    rate_micros: tier.rate_micros,
                })
                .collect(),
            discounts: meter
                .discount_tiers
                .iter()
                .map(|tier| RateDiscount {
                    threshold: tier.threshold,
                    discount_bps: tier.discount_bps,
                })
                .collect(),
            region_multiplier_bps: region_map_for_meter(meter, regions),
        })
        .collect();

    RatingPlan {
        version: 2,
        billing_period: billing_period.to_string(),
        currency: BILLING_CURRENCY,
        period_days: BILLING_PERIOD_DAYS,
        grace_days: BILLING_GRACE_DAYS,
        contract: RatingContract {
            commit_12mo_discount_bps: COMMIT_DISCOUNT_BPS,
            prepay_credit_bps: PREPAY_CREDIT_BPS,
            dispute_hold_bps: DISPUTE_HOLD_BPS,
            treasury_cut_bps: TREASURY_CUT_BPS,
            surge_surcharge_bps: 500,
        },
        meters,
        guardrails: RatingGuardrails {
            warn_utilization_bps: 8_500,
            critical_utilization_bps: 9_500,
            alert_routes: vec!["pager:soranet-edge", "email:billing@sora.net"],
            ledger_currency: "xor.sora",
        },
    }
}

fn ledger_hooks(billing_period: &str) -> LedgerHooks {
    LedgerHooks {
        version: 2,
        billing_period: billing_period.to_string(),
        currency: BILLING_CURRENCY,
        accounts: LedgerAccounts {
            receivable: "sora.cdn.receivable".to_string(),
            revenue: "sora.cdn.revenue".to_string(),
            escrow: "sora.cdn.dispute.v1".to_string(),
            refunds: "sora.cdn.refund_pool".to_string(),
            treasury: "sora.treasury".to_string(),
        },
        posting_rules: PostingRules {
            invoice: PostingRule {
                debit: "customer".to_string(),
                credit: "sora.cdn.receivable".to_string(),
                memo: "usage invoice",
            },
            payment: PostingRule {
                debit: "sora.cdn.receivable".to_string(),
                credit: "sora.cdn.revenue".to_string(),
                memo: "payment applied",
            },
            dispute_hold: PostingRule {
                debit: "sora.cdn.receivable".to_string(),
                credit: "sora.cdn.dispute.v1".to_string(),
                memo: "dispute hold until resolved",
            },
            dispute_release: PostingRule {
                debit: "sora.cdn.dispute.v1".to_string(),
                credit: "sora.cdn.revenue".to_string(),
                memo: "dispute resolved",
            },
            refund: PostingRule {
                debit: "sora.cdn.revenue".to_string(),
                credit: "sora.cdn.refund_pool".to_string(),
                memo: "approved refund",
            },
            treasury: PostingRule {
                debit: "sora.cdn.receivable".to_string(),
                credit: "sora.treasury".to_string(),
                memo: "treasury skim",
            },
        },
        guardrails: LedgerGuardrails {
            dispute_hold_percent_bps: DISPUTE_HOLD_BPS,
            max_credit_memos_per_period: 2,
            prepay_required_bps: PREPAY_CREDIT_BPS,
            treasury_cut_bps: TREASURY_CUT_BPS,
        },
    }
}

fn ledger_projection(
    billing_period: &str,
    meters: &[BaselineMeter],
    regions: &[RegionRate],
) -> Result<LedgerProjection> {
    let usage = sample_usage();
    let charges = usage
        .iter()
        .map(|entry| compute_usage_charge(entry, meters, regions))
        .collect::<Result<Vec<_>>>()?;

    let gross = charges.iter().try_fold(0_u128, |acc, charge| {
        acc.checked_add(charge.amount_micros as u128)
            .ok_or_else(|| eyre!("overflow computing gross charges"))
    })?;
    let commit_discount = scale_by_bps(gross, COMMIT_DISCOUNT_BPS)?;
    let prepay_credit = scale_by_bps(gross, PREPAY_CREDIT_BPS)?;
    let discounts = commit_discount
        .checked_add(prepay_credit)
        .ok_or_else(|| eyre!("overflow computing discounts"))?;
    let net = gross
        .checked_sub(discounts)
        .ok_or_else(|| eyre!("discount exceeds gross charges"))?;
    let dispute_hold = scale_by_bps(net, DISPUTE_HOLD_BPS)?;
    let treasury_cut = scale_by_bps(net, TREASURY_CUT_BPS)?;
    let revenue = net
        .checked_sub(dispute_hold)
        .and_then(|value| value.checked_sub(treasury_cut))
        .ok_or_else(|| eyre!("ledger projection underflow"))?;

    let totals = LedgerTotals {
        gross_micros: gross as u64,
        discounts_micros: discounts as u64,
        dispute_hold_micros: dispute_hold as u64,
        treasury_cut_micros: treasury_cut as u64,
        net_micros: net as u64,
    };
    let transfers = vec![
        LedgerTransfer {
            from: "customer".to_string(),
            to: "sora.cdn.receivable".to_string(),
            amount_micros: totals.gross_micros,
            memo: "invoice capture".to_string(),
        },
        LedgerTransfer {
            from: "sora.cdn.receivable".to_string(),
            to: "sora.cdn.revenue".to_string(),
            amount_micros: revenue as u64,
            memo: "net after discounts/holds".to_string(),
        },
        LedgerTransfer {
            from: "sora.cdn.receivable".to_string(),
            to: "sora.cdn.dispute.v1".to_string(),
            amount_micros: totals.dispute_hold_micros,
            memo: "escrow hold until dispute closure".to_string(),
        },
        LedgerTransfer {
            from: "sora.cdn.receivable".to_string(),
            to: "sora.treasury".to_string(),
            amount_micros: totals.treasury_cut_micros,
            memo: "treasury skim (ops reserve)".to_string(),
        },
    ];

    Ok(LedgerProjection {
        billing_period: billing_period.to_string(),
        currency: BILLING_CURRENCY,
        usage: charges,
        totals,
        transfers,
        notes: vec![
            "Discounts include commit (12mo) and prepay credit; meter-level discounts are already baked into usage lines."
                .to_string(),
            "Dispute holds use the net total after discounts, matching billing_ledger_hooks.toml."
                .to_string(),
            "Treasury skim is separated so governance packets can reconcile receivable vs reserve movement."
                .to_string(),
        ],
    })
}

fn meter_export_rows(meters: &[BaselineMeter], regions: &[RegionRate]) -> Result<Vec<ExportRow>> {
    let mut rows = Vec::new();
    for meter in meters {
        let max_discount = meter
            .discount_tiers
            .iter()
            .map(|tier| tier.discount_bps)
            .max()
            .unwrap_or(0);
        let tiers = if meter.tiers.is_empty() {
            vec![PriceTier {
                label: "base",
                upto: None,
                rate_micros: meter.price_micros,
            }]
        } else {
            meter.tiers.to_vec()
        };
        let region_set = if meter.region_multipliers.is_empty() {
            regions
        } else {
            meter.region_multipliers
        };
        for tier in &tiers {
            for region in region_set {
                let effective = scale_by_bps(tier.rate_micros as u128, region.multiplier_bps)?;
                rows.push(ExportRow {
                    meter_id: meter.id.to_string(),
                    category: meter.category.to_string(),
                    unit: meter.unit.to_string(),
                    tier_label: tier.label.to_string(),
                    region: region.id.to_string(),
                    base_rate_micros: tier.rate_micros,
                    effective_rate_micros: effective as u64,
                    included_units: meter.included_units,
                    burst_cap: meter.burst_cap,
                    quantum: meter.quantum.map(|value| value as i64),
                    max_discount_bps: max_discount,
                });
            }
        }
    }
    Ok(rows)
}

fn render_guardrails_yaml(billing_period: &str, meters: &[BaselineMeter]) -> Result<String> {
    let mut out = String::new();
    writeln!(
        &mut out,
        "# Guardrails and alert thresholds for SN15-M0 billing"
    )?;
    writeln!(&mut out, "period: \"{billing_period}\"")?;
    writeln!(&mut out, "version: 2")?;
    writeln!(&mut out, "caps:")?;
    for meter in meters {
        writeln!(&mut out, "  {}:", meter.id)?;
        writeln!(&mut out, "    unit: {}", meter.unit)?;
        writeln!(
            &mut out,
            "    warn_ratio: {:.2}",
            meter.guardrail.warn_ratio
        )?;
        writeln!(
            &mut out,
            "    critical_ratio: {:.2}",
            meter.guardrail.critical_ratio
        )?;
        writeln!(&mut out, "    hard_cap: {}", meter.guardrail.hard_cap)?;
    }
    writeln!(&mut out, "alerts:")?;
    for alert in ALERT_RULES {
        writeln!(&mut out, "  - id: {}", alert.id)?;
        writeln!(&mut out, "    signal: {}", alert.signal)?;
        writeln!(&mut out, "    trigger: \"{}\"", alert.trigger)?;
        writeln!(&mut out, "    route: {}", alert.route)?;
        writeln!(&mut out, "    note: \"{}\"", alert.note)?;
    }
    writeln!(&mut out, "notes:")?;
    writeln!(
        &mut out,
        "  - Guardrails throttle before hard stops; hard caps mirror burst_cap per meter."
    )?;
    writeln!(
        &mut out,
        "  - Alerts are intended for promtool linting and replay in Alertmanager canaries."
    )?;
    writeln!(
        &mut out,
        "  - Region multipliers and discounts are captured in billing_meter_catalog.(json|csv|parquet) for reconciliation."
    )?;
    Ok(out)
}

fn render_invoice_template(billing_period: &str) -> String {
    format!(
        "# SoraGlobal Gateway Invoice\n\
\n\
Period: {period}\n\
Tenant: <tenant>\n\
Prepared: <generated UTC timestamp>\n\
\n\
| Meter | Units | Rate (XOR) | Amount (XOR) |\n\
| ---- | ----- | --------- | ------------ |\n\
| http.request | <count> | 0.000005 | <calc> |\n\
| http.egress_gib | <gib> | 0.25 | <calc> |\n\
| dns.doh_query | <count> | 0.000003 | <calc> |\n\
| waf.decision | <count> | 0.00002 | <calc> |\n\
| car.verification_ms | <ms> | 0.00000001 | <calc> |\n\
| storage.gib_month | <gib-month> | 0.18 | <calc> |\n\
\n\
Subtotal: <xor>\n\
Discounts/credits: <xor>\n\
Taxes: <xor>\n\
Total due (XOR): <xor>\n\
\n\
Dispute window: 7 days from invoice delivery. Attach `billing_dispute_template.md` with meter identifiers, timestamps, and evidence paths. Credit memos apply to the next period after dispute closure.\n",
        period = billing_period
    )
}

fn render_dispute_template(billing_period: &str) -> String {
    format!(
        "# SoraGlobal Gateway Dispute Form\n\
\n\
Period: {period}\n\
Tenant: <tenant>\n\
Invoice ID: <billing invoice id>\n\
Submission timestamp: <UTC ISO8601>\n\
\n\
Meters under dispute:\n\
- meter_id: <http.request|http.egress_gib|dns.doh_query|waf.decision|car.verification_ms|storage.gib_month>\n\
  window: <start..end UTC>\n\
  expected_value: <number>\n\
  observed_value: <number>\n\
  evidence: <path to logs/artefacts>\n\
\n\
Requested outcome: <credit memo | refund | reversal>\n\
Impact summary: <short text>\n\
\n\
Notes:\n\
- Include Alertmanager extracts if caps/thresholds were crossed.\n\
- Disputes reserve 20% of the net receivable in escrow until resolved.\n",
        period = billing_period
    )
}

fn render_reconciliation_template(billing_period: &str) -> String {
    format!(
        "# Reconciliation Checklist ({period})\n\
\n\
1. Pull {period} meter snapshots from the billing spool.\n\
2. Recompute charges using billing_meter_catalog.json + billing_rating_plan.toml (bps multipliers and tiers are deterministic); confirm CSV/Parquet exports match the JSON view.\n\
3. Validate ledger postings using billing_ledger_hooks.toml and billing_ledger_projection.json (invoice -> payment -> dispute hold/release -> refund/treasury skim).\n\
4. Compare Alertmanager exports against billing_guardrails.yaml to confirm caps and warnings fired correctly.\n\
5. Attach invoice and dispute templates plus any credit memos; publish evidence to artifacts/soranet/gateway_m0/billing/<period>/.\n",
        period = billing_period
    )
}

fn write_json_pretty<T: JsonValueSerialize>(value: &T, path: &Path) -> Result<()> {
    let file =
        File::create(path).wrap_err_with(|| format!("failed to create {}", path.display()))?;
    json::to_writer_pretty(file, value)
        .wrap_err_with(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn write_meter_csv(rows: &[ExportRow], path: &Path) -> Result<()> {
    let mut out = String::from(
        "meter_id,category,unit,tier,region,base_rate_micros,effective_rate_micros,included_units,burst_cap,quantum,max_discount_bps\n",
    );
    for row in rows {
        let quantum = row
            .quantum
            .map(|value| value.to_string())
            .unwrap_or_default();
        writeln!(
            &mut out,
            "{},{},{},{},{},{},{},{},{},{},{}",
            row.meter_id,
            row.category,
            row.unit,
            row.tier_label,
            row.region,
            row.base_rate_micros,
            row.effective_rate_micros,
            row.included_units,
            row.burst_cap,
            quantum,
            row.max_discount_bps
        )?;
    }
    fs::write(path, out).wrap_err_with(|| format!("failed to write {}", path.display()))
}

fn write_meter_parquet(rows: &[ExportRow], path: &Path) -> Result<()> {
    let schema = meter_schema();
    let batch = RecordBatch::try_new(
        std::sync::Arc::new(schema.clone()),
        vec![
            make_string_array(rows.iter().map(|row| Some(row.meter_id.as_str()))),
            make_string_array(rows.iter().map(|row| Some(row.category.as_str()))),
            make_string_array(rows.iter().map(|row| Some(row.unit.as_str()))),
            make_string_array(rows.iter().map(|row| Some(row.tier_label.as_str()))),
            make_string_array(rows.iter().map(|row| Some(row.region.as_str()))),
            make_u64_array(rows.iter().map(|row| row.base_rate_micros)),
            make_u64_array(rows.iter().map(|row| row.effective_rate_micros)),
            make_u64_array(rows.iter().map(|row| row.included_units)),
            make_u64_array(rows.iter().map(|row| row.burst_cap)),
            make_i64_array(rows.iter().map(|row| row.quantum)),
            make_u16_array(rows.iter().map(|row| row.max_discount_bps)),
        ],
    )
    .wrap_err("failed to build meter catalog record batch")?;

    let props = WriterProperties::builder()
        .set_compression(Compression::UNCOMPRESSED)
        .build();
    let file = fs::File::create(path)
        .wrap_err_with(|| format!("failed to create Parquet {}", path.display()))?;
    let mut writer = ArrowWriter::try_new(file, schema.into(), Some(props))
        .wrap_err("failed to create Parquet writer")?;
    writer
        .write(&batch)
        .wrap_err("failed to write meter catalog Parquet batch")?;
    writer
        .close()
        .wrap_err("failed to finalize Parquet writer")?;
    Ok(())
}

fn meter_schema() -> Schema {
    Schema::new(vec![
        Field::new("meter_id", DataType::Utf8, false),
        Field::new("category", DataType::Utf8, false),
        Field::new("unit", DataType::Utf8, false),
        Field::new("tier", DataType::Utf8, false),
        Field::new("region", DataType::Utf8, false),
        Field::new("base_rate_micros", DataType::UInt64, false),
        Field::new("effective_rate_micros", DataType::UInt64, false),
        Field::new("included_units", DataType::UInt64, false),
        Field::new("burst_cap", DataType::UInt64, false),
        Field::new("quantum", DataType::Int64, true),
        Field::new("max_discount_bps", DataType::UInt16, false),
    ])
}

fn region_map_for_meter(meter: &BaselineMeter, defaults: &[RegionRate]) -> BTreeMap<String, u16> {
    let regions = if meter.region_multipliers.is_empty() {
        defaults
    } else {
        meter.region_multipliers
    };
    regions
        .iter()
        .map(|region| (region.id.to_string(), region.multiplier_bps))
        .collect()
}

fn tiers_for_record(meter: &BaselineMeter) -> Vec<MeterTierRecord> {
    if meter.tiers.is_empty() {
        return vec![MeterTierRecord {
            label: "base".to_string(),
            upto: None,
            rate_micros: meter.price_micros,
        }];
    }
    meter
        .tiers
        .iter()
        .map(|tier| MeterTierRecord {
            label: tier.label.to_string(),
            upto: tier.upto,
            rate_micros: tier.rate_micros,
        })
        .collect()
}

fn apply_region_and_discount(rate_micros: u64, region_bps: u16, discount_bps: u16) -> Result<u64> {
    let region_adjusted = scale_by_bps(rate_micros as u128, region_bps)?;
    let discounted = scale_by_bps(region_adjusted, 10_000_u16.saturating_sub(discount_bps))?;
    Ok(discounted as u64)
}

fn scale_by_bps(value: u128, bps: u16) -> Result<u128> {
    let scaled = value
        .checked_mul(bps as u128)
        .ok_or_else(|| eyre!("overflow applying bps {bps} to value {value}"))?;
    Ok(scaled.div_ceil(10_000))
}

fn quantize_units(units: u64, quantum: Option<u64>) -> u64 {
    if let Some(quantum) = quantum {
        units.div_ceil(quantum) * quantum
    } else {
        units
    }
}

fn best_discount(meter: &BaselineMeter, billable_units: u64) -> u16 {
    meter
        .discount_tiers
        .iter()
        .filter(|tier| billable_units >= tier.threshold)
        .map(|tier| tier.discount_bps)
        .max()
        .unwrap_or(0)
}

fn compute_usage_charge(
    sample: &UsageSample,
    meters: &[BaselineMeter],
    regions: &[RegionRate],
) -> Result<UsageCharge> {
    let meter = meters
        .iter()
        .find(|meter| meter.id == sample.meter_id)
        .ok_or_else(|| eyre!("usage references unknown meter {}", sample.meter_id))?;
    let region = meter
        .region_multipliers
        .iter()
        .find(|region| region.id == sample.region)
        .or_else(|| regions.iter().find(|region| region.id == sample.region))
        .ok_or_else(|| {
            eyre!(
                "usage references unknown region {} for meter {}",
                sample.region,
                sample.meter_id
            )
        })?;

    let mut billable_units = quantize_units(sample.quantity, meter.quantum);
    if billable_units <= meter.included_units {
        billable_units = 0;
    } else {
        billable_units -= meter.included_units;
    }
    let discount_bps = best_discount(meter, billable_units);

    let mut remaining = billable_units;
    let mut billed_so_far = 0_u64;
    let mut total: u128 = 0;
    let tiers = if meter.tiers.is_empty() {
        vec![PriceTier {
            label: "base",
            upto: None,
            rate_micros: meter.price_micros,
        }]
    } else {
        meter.tiers.to_vec()
    };
    for tier in tiers {
        if remaining == 0 {
            break;
        }
        let tier_limit = tier.upto.unwrap_or(u64::MAX);
        let available = tier_limit.saturating_sub(billed_so_far);
        if available == 0 {
            billed_so_far = billed_so_far.saturating_add(available);
            continue;
        }
        let units_at_tier = remaining.min(available);
        let effective_rate =
            apply_region_and_discount(tier.rate_micros, region.multiplier_bps, discount_bps)?;
        let tier_total = (units_at_tier as u128)
            .checked_mul(effective_rate as u128)
            .ok_or_else(|| eyre!("overflow computing tier total"))?;
        total = total
            .checked_add(tier_total)
            .ok_or_else(|| eyre!("overflow adding tier total"))?;
        billed_so_far = billed_so_far
            .checked_add(units_at_tier)
            .ok_or_else(|| eyre!("overflow tracking billed units"))?;
        remaining -= units_at_tier;
    }

    let effective_rate = if billable_units == 0 {
        0
    } else {
        total.div_ceil(billable_units as u128) as u64
    };

    Ok(UsageCharge {
        meter_id: meter.id.to_string(),
        region: region.id.to_string(),
        units: billable_units,
        effective_rate_micros: effective_rate,
        discount_bps,
        amount_micros: total as u64,
    })
}

fn sample_usage() -> Vec<UsageSample> {
    vec![
        UsageSample {
            meter_id: "http.request",
            region: "NA",
            quantity: 1_500_000,
        },
        UsageSample {
            meter_id: "http.egress_gib",
            region: "EU",
            quantity: 420,
        },
        UsageSample {
            meter_id: "dns.doh_query",
            region: "APAC",
            quantity: 2_800_000,
        },
        UsageSample {
            meter_id: "waf.decision",
            region: "GLOBAL",
            quantity: 12_000,
        },
        UsageSample {
            meter_id: "car.verification_ms",
            region: "GLOBAL",
            quantity: 540_000,
        },
        UsageSample {
            meter_id: "storage.gib_month",
            region: "NA",
            quantity: 85,
        },
    ]
}

fn make_string_array<'a, I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = Option<&'a str>>,
{
    std::sync::Arc::new(StringArray::from_iter(values))
}

fn make_u64_array<I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = u64>,
{
    std::sync::Arc::new(UInt64Array::from_iter_values(values))
}

fn make_i64_array<I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = Option<i64>>,
{
    std::sync::Arc::new(Int64Array::from_iter(values))
}

fn make_u16_array<I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = u16>,
{
    std::sync::Arc::new(UInt16Array::from_iter_values(values))
}

#[cfg(test)]
mod tests {
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn meters_expose_expected_regions_and_tiers() {
        let meters = baseline_meters();
        let regions = baseline_regions();
        let catalog = meter_catalog("2026-11", &meters, regions);
        assert_eq!(catalog.meters.len(), meters.len());
        let storage = catalog
            .meters
            .iter()
            .find(|meter| meter.id == "storage.gib_month")
            .expect("storage meter present");
        assert!(!storage.discount_tiers.is_empty());
        assert!(catalog.regions.iter().any(|region| region.id == "NA"));
    }

    #[test]
    fn export_rows_cover_all_regions() {
        let meters = baseline_meters();
        let regions = baseline_regions();
        let rows = meter_export_rows(&meters, regions).expect("rows");
        // 6 meters across 4 primary regions with two global-only meters yields 26 flattened rows.
        assert!(rows.len() >= 26);
        assert!(
            rows.iter()
                .any(|row| row.meter_id == "http.egress_gib" && row.tier_label == "first_100_gib")
        );
        assert!(rows.iter().any(|row| row.region == "GLOBAL"));
    }

    #[test]
    fn usage_charge_respects_quantum_and_included() {
        let meters = baseline_meters();
        let regions = baseline_regions();
        let usage = UsageSample {
            meter_id: "car.verification_ms",
            region: "GLOBAL",
            quantity: 101,
        };
        let charge = compute_usage_charge(&usage, &meters, regions).expect("charge");
        // Quantum is 10 ms and included is 80_000, so this should zero out.
        assert_eq!(charge.units, 0);
        assert_eq!(charge.amount_micros, 0);
    }

    #[test]
    fn parquet_round_trips_export_rows() {
        let meters = baseline_meters();
        let regions = baseline_regions();
        let rows = meter_export_rows(&meters, regions).expect("rows");
        let temp = tempdir().expect("tempdir");
        let parquet_path = temp.path().join("billing.parquet");
        write_meter_parquet(&rows, &parquet_path).expect("parquet");

        let file = fs::File::open(&parquet_path).expect("parquet exists");
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)
            .expect("reader builder")
            .build()
            .expect("reader");
        let mut total_rows = 0;
        for batch in reader {
            total_rows += batch.expect("batch").num_rows();
        }
        assert_eq!(total_rows, rows.len());
    }

    #[test]
    fn scale_by_bps_handles_rounding() {
        let scaled = scale_by_bps(1_000, 9_500).expect("scale");
        assert_eq!(scaled, 950);
        let rounded_up = scale_by_bps(1, 10_000).expect("scale");
        assert_eq!(rounded_up, 1);
    }
}
