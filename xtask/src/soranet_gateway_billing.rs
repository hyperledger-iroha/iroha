//! Billing harness for the SoraGlobal Gateway CDN (SN15-M0-9).
//! Rates usage against the meter catalog, enforces guardrails, and emits ledger
//! projections plus CSV/Parquet exports for reconciliation.

use std::{
    collections::BTreeMap,
    fs::{self, File},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use arrow_array::{ArrayRef, Float64Array, RecordBatch, StringArray, UInt16Array, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use eyre::{Result, WrapErr};
use iroha_data_model::{
    account::AccountId,
    asset::AssetDefinitionId,
    isi::{InstructionBox, TransferAssetBatch, TransferAssetBatchEntry},
};
use iroha_primitives::numeric::Numeric;
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use parquet::{arrow::ArrowWriter, basic::Compression, file::properties::WriterProperties};

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct MeterDefinition {
    pub id: String,
    pub description: String,
    pub unit: String,
    pub price_micros: u64,
    pub region_multipliers_bps: BTreeMap<String, u16>,
    pub discount_tiers: Vec<DiscountTier>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct DiscountTier {
    pub threshold: u64,
    pub discount_bps: u16,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct UsageEntry {
    pub meter_id: String,
    pub quantity: u64,
    pub region: String,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct UsageSnapshot {
    pub window: String,
    pub tenant: Option<String>,
    pub entries: Vec<UsageEntry>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct GuardrailsConfig {
    pub catalog_version: String,
    pub currency: String,
    pub soft_cap_micros: u64,
    pub hard_cap_micros: u64,
    pub alert_threshold_bps: u16,
    pub min_invoice_micros: u64,
    pub notes: Vec<String>,
}

impl GuardrailsConfig {
    pub fn default_for_m0() -> Self {
        Self {
            catalog_version: "m0".to_string(),
            currency: "XOR".to_string(),
            soft_cap_micros: 140_000_000,
            hard_cap_micros: 220_000_000,
            alert_threshold_bps: 9_000,
            min_invoice_micros: 1_000_000,
            notes: vec![],
        }
    }
}

#[derive(Debug, Clone, JsonSerialize)]
pub struct InvoiceLine {
    pub meter_id: String,
    pub description: String,
    pub unit: String,
    pub region: String,
    pub quantity: u64,
    pub base_price_micros: u64,
    pub region_multiplier_bps: u16,
    pub discount_bps: u16,
    pub line_total_micros: u64,
    pub effective_unit_micros: u64,
}

#[derive(Debug, JsonSerialize)]
pub struct GuardrailOutcome {
    pub soft_cap_micros: u64,
    pub hard_cap_micros: u64,
    pub min_invoice_micros: u64,
    pub alert_threshold_bps: u16,
    pub alert_threshold_micros: u64,
    pub total_micros: u64,
    pub alert_triggered: bool,
    pub hard_cap_exceeded: bool,
    pub below_min_invoice: bool,
}

#[derive(Debug, JsonSerialize)]
pub struct InvoiceSummary {
    pub window: String,
    pub tenant: Option<String>,
    pub normalized_entries: Vec<UsageEntry>,
    pub merge_notes: Vec<MergeNote>,
    pub lines: Vec<InvoiceLine>,
    pub totals: InvoiceTotals,
    pub guardrails: GuardrailOutcome,
}

#[derive(Debug, JsonSerialize)]
pub struct InvoiceTotals {
    pub line_count: usize,
    pub total_micros: u64,
    pub total_xor: f64,
}

#[derive(Debug, JsonSerialize)]
pub struct LedgerProjection {
    pub asset_definition: AssetDefinitionId,
    pub payer: AccountId,
    pub treasury: AccountId,
    pub total_micros: u64,
    pub total_xor: f64,
    pub transfer_batch: InstructionBox,
}

#[derive(Debug)]
pub struct BillingOptions {
    pub usage_path: PathBuf,
    pub catalog_path: PathBuf,
    pub guardrails_path: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub payer: String,
    pub treasury: String,
    pub asset_definition: String,
    pub allow_hard_cap: bool,
}

#[derive(Debug)]
pub struct BillingOutcome {
    pub invoice_path: PathBuf,
    pub csv_path: PathBuf,
    pub parquet_path: PathBuf,
    pub ledger_path: PathBuf,
    pub reconciliation_report_path: PathBuf,
    pub total_micros: u64,
}

#[derive(Debug, Clone, JsonSerialize)]
pub struct MergeNote {
    pub meter_id: String,
    pub region: String,
    pub merged_count: usize,
    pub total_quantity: u64,
}

pub fn run_billing(options: BillingOptions) -> Result<BillingOutcome> {
    let catalog = load_catalog(&options.catalog_path)?;
    let usage = load_usage(&options.usage_path)?;
    let guardrails = load_guardrails(options.guardrails_path.as_deref())?;

    let meter_map: BTreeMap<_, _> = catalog
        .iter()
        .map(|meter| (meter.id.clone(), meter))
        .collect();

    let (normalized_entries, merge_notes) = normalize_usage_entries(&usage.entries)?;

    let mut lines = Vec::new();
    for entry in &normalized_entries {
        let Some(meter) = meter_map.get(&entry.meter_id) else {
            return Err(eyre::eyre!(
                "usage entry references unknown meter id `{}`",
                entry.meter_id
            ));
        };
        let discount_bps = resolve_discount(entry, meter)?;
        let region_multiplier_bps = meter
            .region_multipliers_bps
            .get(&entry.region)
            .copied()
            .or_else(|| meter.region_multipliers_bps.get("GLOBAL").copied())
            .ok_or_else(|| {
                eyre::eyre!(
                    "usage entry region `{}` missing from meter `{}` catalog (expected one of {:?} or GLOBAL fallback)",
                    entry.region,
                    meter.id,
                    meter.region_multipliers_bps.keys().collect::<Vec<_>>()
                )
            })?;
        let line_total_micros = calculate_line_total(
            meter.price_micros,
            entry.quantity,
            region_multiplier_bps,
            discount_bps,
        )?;
        let effective_unit_micros = line_total_micros.div_ceil(entry.quantity);
        lines.push(InvoiceLine {
            meter_id: meter.id.clone(),
            description: meter.description.clone(),
            unit: meter.unit.clone(),
            region: entry.region.clone(),
            quantity: entry.quantity,
            base_price_micros: meter.price_micros,
            region_multiplier_bps,
            discount_bps,
            line_total_micros,
            effective_unit_micros: effective_unit_micros as u64,
        });
    }

    lines.sort_by(|a, b| a.meter_id.cmp(&b.meter_id).then(a.region.cmp(&b.region)));

    let total_micros: u64 = lines.iter().map(|line| line.line_total_micros).sum();
    let total_xor = total_micros as f64 / 1_000_000.0;
    let guardrail_outcome = evaluate_guardrails(&guardrails, total_micros);

    if guardrail_outcome.hard_cap_exceeded && !options.allow_hard_cap {
        return Err(eyre::eyre!(
            "hard cap exceeded ({} micro-XOR) and override not allowed",
            guardrails.hard_cap_micros
        ));
    }
    if guardrail_outcome.below_min_invoice {
        return Err(eyre::eyre!(
            "total {} micro-XOR is below the minimum invoice threshold {}",
            total_micros,
            guardrails.min_invoice_micros
        ));
    }

    let invoice = InvoiceSummary {
        window: usage.window.clone(),
        tenant: usage.tenant.clone(),
        normalized_entries,
        merge_notes,
        lines: lines.clone(),
        totals: InvoiceTotals {
            line_count: lines.len(),
            total_micros,
            total_xor,
        },
        guardrails: guardrail_outcome,
    };

    fs::create_dir_all(&options.output_dir).wrap_err_with(|| {
        format!(
            "failed to create output directory {}",
            options.output_dir.display()
        )
    })?;

    let invoice_path = options.output_dir.join("billing_invoice.json");
    let file = File::create(&invoice_path)
        .wrap_err_with(|| format!("failed to create invoice {}", invoice_path.display()))?;
    json::to_writer_pretty(file, &invoice).wrap_err("failed to write invoice JSON")?;

    let csv_path = options.output_dir.join("billing_invoice.csv");
    let csv = render_csv(&lines);
    fs::write(&csv_path, csv)
        .wrap_err_with(|| format!("failed to write CSV export {}", csv_path.display()))?;

    let parquet_path = options.output_dir.join("billing_invoice.parquet");
    write_parquet(&lines, &parquet_path)?;

    let catalog_copy = options.output_dir.join("meter_catalog.json");
    fs::copy(&options.catalog_path, &catalog_copy).wrap_err_with(|| {
        format!(
            "failed to copy catalog {} into artefacts",
            options.catalog_path.display()
        )
    })?;

    let usage_copy = options.output_dir.join("billing_usage_snapshot.json");
    fs::copy(&options.usage_path, &usage_copy).wrap_err_with(|| {
        format!(
            "failed to copy usage snapshot {} into artefacts",
            options.usage_path.display()
        )
    })?;

    let guardrail_copy = options.output_dir.join("billing_guardrails.json");
    let guard_file = File::create(&guardrail_copy).wrap_err_with(|| {
        format!(
            "failed to create guardrail copy {}",
            guardrail_copy.display()
        )
    })?;
    json::to_writer_pretty(guard_file, &guardrails).wrap_err("failed to write guardrail copy")?;

    let ledger_path = options.output_dir.join("billing_ledger_projection.json");
    let ledger_projection = build_ledger_projection(
        &options.payer,
        &options.treasury,
        &options.asset_definition,
        total_micros,
    )?;
    let ledger_file = File::create(&ledger_path).wrap_err_with(|| {
        format!(
            "failed to create ledger projection {}",
            ledger_path.display()
        )
    })?;
    json::to_writer_pretty(ledger_file, &ledger_projection)
        .wrap_err("failed to write ledger projection")?;

    let reconciliation_report_path = options.output_dir.join("billing_reconciliation_report.md");
    let report = render_reconciliation_report(&invoice, &ledger_projection, &guardrails);
    fs::write(&reconciliation_report_path, report).wrap_err_with(|| {
        format!(
            "failed to write reconciliation report {}",
            reconciliation_report_path.display()
        )
    })?;

    Ok(BillingOutcome {
        invoice_path,
        csv_path,
        parquet_path,
        ledger_path,
        reconciliation_report_path,
        total_micros,
    })
}

fn load_catalog(path: &Path) -> Result<Vec<MeterDefinition>> {
    let file = File::open(path)
        .wrap_err_with(|| format!("failed to open meter catalog {}", path.display()))?;
    let meters: Vec<MeterDefinition> = json::from_reader(file)
        .wrap_err_with(|| format!("failed to parse meter catalog {}", path.display()))?;
    if meters.is_empty() {
        return Err(eyre::eyre!("meter catalog {} is empty", path.display()));
    }
    Ok(meters)
}

fn load_usage(path: &Path) -> Result<UsageSnapshot> {
    let file = File::open(path)
        .wrap_err_with(|| format!("failed to open usage snapshot {}", path.display()))?;
    let usage: UsageSnapshot = json::from_reader(file)
        .wrap_err_with(|| format!("failed to parse usage snapshot {}", path.display()))?;
    if usage.entries.is_empty() {
        return Err(eyre::eyre!(
            "usage snapshot {} contains no entries",
            path.display()
        ));
    }
    Ok(usage)
}

fn load_guardrails(path: Option<&Path>) -> Result<GuardrailsConfig> {
    if let Some(path) = path {
        let file = File::open(path)
            .wrap_err_with(|| format!("failed to open guardrail config {}", path.display()))?;
        let guardrails: GuardrailsConfig = json::from_reader(file)
            .wrap_err_with(|| format!("failed to parse guardrail config {}", path.display()))?;
        return Ok(guardrails);
    }

    Ok(GuardrailsConfig::default_for_m0())
}

fn calculate_line_total(
    base_price_micros: u64,
    quantity: u64,
    region_multiplier_bps: u16,
    discount_bps: u16,
) -> Result<u64> {
    if quantity == 0 {
        return Err(eyre::eyre!("quantity for a line item cannot be zero"));
    }
    let base_total = (base_price_micros as u128)
        .checked_mul(quantity as u128)
        .ok_or_else(|| eyre::eyre!("line total overflow for meter spend calculation"))?;
    let region_applied = apply_bps(base_total, region_multiplier_bps);
    let discounted = apply_bps(region_applied, 10_000 - discount_bps);
    u64::try_from(discounted)
        .map_err(|_| eyre::eyre!("line total exceeds u64 range for meter spend calculation"))
}

fn apply_bps(value: u128, bps: u16) -> u128 {
    (value * u128::from(bps) + 5_000) / 10_000
}

fn evaluate_guardrails(config: &GuardrailsConfig, total_micros: u64) -> GuardrailOutcome {
    let alert_threshold =
        apply_bps(config.soft_cap_micros as u128, config.alert_threshold_bps) as u64;
    GuardrailOutcome {
        soft_cap_micros: config.soft_cap_micros,
        hard_cap_micros: config.hard_cap_micros,
        min_invoice_micros: config.min_invoice_micros,
        alert_threshold_bps: config.alert_threshold_bps,
        alert_threshold_micros: alert_threshold,
        total_micros,
        alert_triggered: total_micros >= alert_threshold,
        hard_cap_exceeded: total_micros > config.hard_cap_micros,
        below_min_invoice: total_micros < config.min_invoice_micros,
    }
}

fn render_csv(lines: &[InvoiceLine]) -> String {
    let mut out = String::from(
        "meter_id,region,unit,quantity,effective_unit_micros,line_total_micros,discount_bps,region_multiplier_bps\n",
    );
    for line in lines {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            line.meter_id,
            line.region,
            line.unit,
            line.quantity,
            line.effective_unit_micros,
            line.line_total_micros,
            line.discount_bps,
            line.region_multiplier_bps
        ));
    }
    out
}

fn write_parquet(lines: &[InvoiceLine], path: &Path) -> Result<()> {
    let meter_ids: ArrayRef = Arc::new(StringArray::from_iter(
        lines.iter().map(|line| Some(line.meter_id.as_str())),
    ));
    let regions: ArrayRef = Arc::new(StringArray::from_iter(
        lines.iter().map(|line| Some(line.region.as_str())),
    ));
    let units: ArrayRef = Arc::new(StringArray::from_iter(
        lines.iter().map(|line| Some(line.unit.as_str())),
    ));
    let quantities: ArrayRef = Arc::new(UInt64Array::from_iter(
        lines.iter().map(|line| Some(line.quantity)),
    ));
    let line_totals: ArrayRef = Arc::new(UInt64Array::from_iter(
        lines.iter().map(|line| Some(line.line_total_micros)),
    ));
    let effective_unit: ArrayRef = Arc::new(UInt64Array::from_iter(
        lines.iter().map(|line| Some(line.effective_unit_micros)),
    ));
    let discount_bps: ArrayRef = Arc::new(UInt16Array::from_iter(
        lines.iter().map(|line| Some(line.discount_bps)),
    ));
    let region_bps: ArrayRef = Arc::new(UInt16Array::from_iter(
        lines.iter().map(|line| Some(line.region_multiplier_bps)),
    ));
    let line_totals_xor: ArrayRef = Arc::new(Float64Array::from_iter(
        lines
            .iter()
            .map(|line| Some(line.line_total_micros as f64 / 1_000_000.0)),
    ));

    let schema = Arc::new(Schema::new(vec![
        Field::new("meter_id", DataType::Utf8, false),
        Field::new("region", DataType::Utf8, false),
        Field::new("unit", DataType::Utf8, false),
        Field::new("quantity", DataType::UInt64, false),
        Field::new("line_total_micros", DataType::UInt64, false),
        Field::new("line_total_xor", DataType::Float64, false),
        Field::new("effective_unit_micros", DataType::UInt64, false),
        Field::new("discount_bps", DataType::UInt16, false),
        Field::new("region_multiplier_bps", DataType::UInt16, false),
    ]));

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            meter_ids,
            regions,
            units,
            quantities,
            line_totals,
            line_totals_xor,
            effective_unit,
            discount_bps,
            region_bps,
        ],
    )
    .wrap_err("failed to build Parquet record batch")?;

    let props = WriterProperties::builder()
        .set_compression(Compression::UNCOMPRESSED)
        .build();
    let file = File::create(path)
        .wrap_err_with(|| format!("failed to create Parquet file {}", path.display()))?;
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))
        .wrap_err("failed to create Parquet writer")?;
    writer
        .write(&batch)
        .wrap_err("failed to write Parquet record batch")?;
    writer.close().wrap_err("failed to close Parquet writer")?;
    Ok(())
}

fn normalize_usage_entries(entries: &[UsageEntry]) -> Result<(Vec<UsageEntry>, Vec<MergeNote>)> {
    let mut aggregated: BTreeMap<(String, String), (u64, usize)> = BTreeMap::new();
    for entry in entries {
        if entry.quantity == 0 {
            return Err(eyre::eyre!(
                "usage entry for meter `{}` in region `{}` has zero quantity",
                entry.meter_id,
                entry.region
            ));
        }
        let meter_id = entry.meter_id.trim();
        if meter_id.is_empty() {
            return Err(eyre::eyre!("meter id cannot be empty"));
        }
        let region = entry.region.trim();
        if region.is_empty() {
            return Err(eyre::eyre!(
                "region cannot be empty for meter `{}`",
                meter_id
            ));
        }
        let key = (meter_id.to_string(), region.to_uppercase());
        let (total, count) = aggregated.entry(key).or_insert((0, 0));
        *total = total
            .checked_add(entry.quantity)
            .ok_or_else(|| eyre::eyre!("usage quantity overflow for meter `{meter_id}`"))?;
        *count += 1;
    }

    let mut normalized = Vec::with_capacity(aggregated.len());
    let mut merge_notes = Vec::new();
    for ((meter_id, region), (quantity, merged_count)) in aggregated {
        normalized.push(UsageEntry {
            meter_id: meter_id.clone(),
            region: region.clone(),
            quantity,
        });
        if merged_count > 1 {
            merge_notes.push(MergeNote {
                meter_id,
                region,
                merged_count,
                total_quantity: quantity,
            });
        }
    }

    normalized.sort_by(|a, b| a.meter_id.cmp(&b.meter_id).then(a.region.cmp(&b.region)));
    merge_notes.sort_by(|a, b| a.meter_id.cmp(&b.meter_id).then(a.region.cmp(&b.region)));

    Ok((normalized, merge_notes))
}

fn resolve_discount(entry: &UsageEntry, meter: &MeterDefinition) -> Result<u16> {
    let discount_bps = meter
        .discount_tiers
        .iter()
        .filter(|tier| entry.quantity >= tier.threshold)
        .map(|tier| tier.discount_bps)
        .max()
        .unwrap_or(0);
    Ok(discount_bps)
}

fn build_ledger_projection(
    payer: &str,
    treasury: &str,
    asset_definition: &str,
    total_micros: u64,
) -> Result<LedgerProjection> {
    let payer: AccountId = AccountId::parse_encoded(payer)
        .map(|parsed| parsed.into_account_id())
        .wrap_err_with(|| format!("failed to parse payer account id `{payer}`"))?;
    let treasury: AccountId = AccountId::parse_encoded(treasury)
        .map(|parsed| parsed.into_account_id())
        .wrap_err_with(|| format!("failed to parse treasury account id `{treasury}`"))?;
    let asset_definition: AssetDefinitionId = AssetDefinitionId::from_str(asset_definition)
        .wrap_err_with(|| format!("failed to parse asset definition `{asset_definition}`"))?;
    let amount = micros_to_numeric(total_micros)?;
    let batch = TransferAssetBatch::new(vec![TransferAssetBatchEntry::new(
        payer.clone(),
        treasury.clone(),
        asset_definition.clone(),
        amount,
    )]);
    Ok(LedgerProjection {
        asset_definition,
        payer,
        treasury,
        total_micros,
        total_xor: total_micros as f64 / 1_000_000.0,
        transfer_batch: InstructionBox::from(batch),
    })
}

fn micros_to_numeric(micros: u64) -> Result<Numeric> {
    let units = micros / 1_000_000;
    let fractional = micros % 1_000_000;
    let repr = format!("{units}.{:06}", fractional);
    Numeric::from_str(&repr)
        .wrap_err_with(|| format!("failed to convert {micros} micro-XOR into Numeric"))
}

fn render_reconciliation_report(
    invoice: &InvoiceSummary,
    ledger: &LedgerProjection,
    guardrails: &GuardrailsConfig,
) -> String {
    let total_xor = format!("{:.6}", invoice.totals.total_xor);
    let guard_soft = format!("{:.6}", guardrails.soft_cap_micros as f64 / 1_000_000.0);
    let guard_hard = format!("{:.6}", guardrails.hard_cap_micros as f64 / 1_000_000.0);
    let alert_threshold = format!("{:.2}", guardrails.alert_threshold_bps as f64 / 100.0);
    format!(
        "# SoraGlobal Gateway Billing Reconciliation\n\
Window: {}\n\
Tenant: {}\n\
Catalog: {}\n\
Total due: {total_xor} XOR ({} micro-XOR)\n\
Guardrails: soft cap {guard_soft} XOR, hard cap {guard_hard} XOR, alert threshold {alert_threshold}%\n\
Payer -> Treasury: {} -> {} ({})\n\
\n\
Line items: {}\n\
Normalized usage entries: {}\n\
Merged duplicate tuples: {}\n\
Guardrail alert triggered: {}\n\
Hard cap exceeded: {}\n\
Below minimum invoice: {}\n\
\n\
Next steps:\n\
- Validate usage snapshot against the catalog and region rules.\n\
- Attach CSV/Parquet exports alongside the invoice JSON for audit replay.\n\
- Ensure the ledger projection total matches `total_micros` before dispatching.\n\
- If disputing, note deltas and attach dashboard/log evidence.\n",
        invoice.window,
        invoice
            .tenant
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string()),
        guardrails.catalog_version,
        invoice.totals.total_micros,
        ledger.payer,
        ledger.treasury,
        ledger.asset_definition,
        invoice.totals.line_count,
        invoice.normalized_entries.len(),
        invoice.merge_notes.len(),
        invoice.guardrails.alert_triggered,
        invoice.guardrails.hard_cap_exceeded,
        invoice.guardrails.below_min_invoice
    )
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs::File};

    use norito::json::{self, Value};
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn billing_normalizes_duplicate_usage_entries() {
        let temp = tempdir().expect("tempdir");
        let catalog_path = temp.path().join("catalog.json");
        let usage_path = temp.path().join("usage.json");
        let guardrails_path = temp.path().join("guardrails.json");
        let output_dir = temp.path().join("out");

        let mut regions = BTreeMap::new();
        regions.insert("NA".to_string(), 10_000);
        let catalog = vec![MeterDefinition {
            id: "http.request".to_string(),
            description: "requests".to_string(),
            unit: "request".to_string(),
            price_micros: 10,
            region_multipliers_bps: regions,
            discount_tiers: vec![],
        }];
        json::to_writer_pretty(File::create(&catalog_path).expect("catalog file"), &catalog)
            .expect("write catalog");

        let usage = UsageSnapshot {
            window: "2026-10".to_string(),
            tenant: Some("demo".to_string()),
            entries: vec![
                UsageEntry {
                    meter_id: "http.request".to_string(),
                    quantity: 10,
                    region: "na".to_string(),
                },
                UsageEntry {
                    meter_id: "http.request".to_string(),
                    quantity: 5,
                    region: "NA".to_string(),
                },
            ],
        };
        json::to_writer_pretty(File::create(&usage_path).expect("usage file"), &usage)
            .expect("write usage");

        let guardrails = GuardrailsConfig {
            catalog_version: "m0".to_string(),
            currency: "XOR".to_string(),
            soft_cap_micros: 1_000_000,
            hard_cap_micros: 2_000_000,
            alert_threshold_bps: 9_000,
            min_invoice_micros: 1,
            notes: vec![],
        };
        json::to_writer_pretty(
            File::create(&guardrails_path).expect("guardrails file"),
            &guardrails,
        )
        .expect("write guardrails");

        let outcome = run_billing(BillingOptions {
            usage_path,
            catalog_path,
            guardrails_path: Some(guardrails_path),
            output_dir: output_dir.clone(),
            payer:
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    .to_string(),
            treasury:
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    .to_string(),
            asset_definition: "xor#wonderland".to_string(),
            allow_hard_cap: false,
        })
        .expect("billing run succeeds");

        let invoice: Value =
            json::from_reader(File::open(outcome.invoice_path).expect("invoice file"))
                .expect("parse invoice");

        let normalized = invoice["normalized_entries"]
            .as_array()
            .expect("normalized entries array");
        assert_eq!(normalized.len(), 1);
        assert_eq!(
            normalized[0]["quantity"],
            Value::from(15u64),
            "duplicate usage entries should be merged"
        );
        let merge_notes = invoice["merge_notes"]
            .as_array()
            .expect("merge notes array");
        assert_eq!(merge_notes.len(), 1);
        assert_eq!(merge_notes[0]["merged_count"], Value::from(2u64));
        assert_eq!(
            invoice["totals"]["total_micros"],
            Value::from(150u64),
            "rating should reflect merged quantity"
        );
    }

    #[test]
    fn billing_rejects_unknown_region_in_usage() {
        let temp = tempdir().expect("tempdir");
        let catalog_path = temp.path().join("catalog.json");
        let usage_path = temp.path().join("usage.json");
        let guardrails_path = temp.path().join("guardrails.json");
        let output_dir = temp.path().join("out");

        let mut regions = BTreeMap::new();
        regions.insert("NA".to_string(), 10_000);
        let catalog = vec![MeterDefinition {
            id: "http.request".to_string(),
            description: "requests".to_string(),
            unit: "request".to_string(),
            price_micros: 10,
            region_multipliers_bps: regions,
            discount_tiers: vec![],
        }];
        json::to_writer_pretty(File::create(&catalog_path).expect("catalog file"), &catalog)
            .expect("write catalog");

        let usage = UsageSnapshot {
            window: "2026-10".to_string(),
            tenant: None,
            entries: vec![UsageEntry {
                meter_id: "http.request".to_string(),
                quantity: 10,
                region: "EU".to_string(),
            }],
        };
        json::to_writer_pretty(File::create(&usage_path).expect("usage file"), &usage)
            .expect("write usage");

        let guardrails = GuardrailsConfig {
            catalog_version: "m0".to_string(),
            currency: "XOR".to_string(),
            soft_cap_micros: 1_000_000,
            hard_cap_micros: 2_000_000,
            alert_threshold_bps: 9_000,
            min_invoice_micros: 1,
            notes: vec![],
        };
        json::to_writer_pretty(
            File::create(&guardrails_path).expect("guardrails file"),
            &guardrails,
        )
        .expect("write guardrails");

        let result = run_billing(BillingOptions {
            usage_path,
            catalog_path,
            guardrails_path: Some(guardrails_path),
            output_dir,
            payer:
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    .to_string(),
            treasury:
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    .to_string(),
            asset_definition: "xor#wonderland".to_string(),
            allow_hard_cap: false,
        });

        assert!(
            result
                .expect_err("region mismatch should fail")
                .to_string()
                .contains("missing from meter"),
            "unknown region should be rejected"
        );
    }
}
