//! Sora Name Service (SNS) registrar helpers.
//!
//! Provides convenience wrappers around the Torii `/v1/sns/*` endpoints so
//! operators can register and inspect names during the N0 closed beta without
//! crafting raw JSON payloads.

use crate::{Run, RunContext};
use crate::cli_output::print_with_optional_text;
use clap::{Args, Subcommand};
use eyre::{Result, WrapErr, eyre};
use iroha::data_model::{
    account::{AccountAddress, AccountId},
    metadata::Metadata,
    name::Name,
    sns::{
        AuctionKind, FreezeNameRequestV1, GovernanceHookV1, NameControllerV1, NameSelectorV1,
        PaymentProofV1, PriceTierV1, RegisterNameRequestV1, RenewNameRequestV1, ReservedNameV1,
        SuffixFeeSplitV1, SuffixPolicyV1, SuffixStatus, TokenValue, TransferNameRequestV1,
        UpdateControllersRequestV1,
    },
};
use iroha::sns::CaseExportQuery;
use iroha_primitives::json::Json;
use norito::json::{self, Map, Value};
use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::LazyLock,
};

const DEFAULT_ARBITRATION_SCHEMA: &str =
    include_str!("../../../../docs/examples/sns/arbitration_case_schema.json");
static CASE_SCHEMA_VALUE: LazyLock<Value> = LazyLock::new(|| {
    json::from_str(DEFAULT_ARBITRATION_SCHEMA)
        .expect("embedded arbitration schema must be valid JSON")
});
const DEFAULT_SUFFIX_CATALOG: &str =
    include_str!("../../../../docs/examples/sns/suffix_catalog_v1.json");
static SUFFIX_CATALOG: LazyLock<SuffixCatalog> = LazyLock::new(|| {
    json::from_str(DEFAULT_SUFFIX_CATALOG).expect("embedded suffix catalog must be valid JSON")
});

#[derive(Subcommand, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    /// Register a SNS name via `/v1/sns/registrations`.
    Register(RegisterArgs),
    /// Renew a SNS name via `/v1/sns/registrations/{selector}/renew`.
    Renew(RenewArgs),
    /// Transfer ownership of a SNS name.
    Transfer(TransferArgs),
    /// Replace controllers on a SNS name.
    UpdateControllers(UpdateControllersArgs),
    /// Freeze a SNS name.
    Freeze(FreezeArgs),
    /// Unfreeze a SNS name.
    Unfreeze(UnfreezeArgs),
    /// Fetch a SNS name record.
    Registration(GetRegistrationArgs),
    /// Fetch the policy for a suffix.
    Policy(GetPolicyArgs),
    /// Governance helpers (arbitration, transparency exports, etc.).
    #[command(subcommand)]
    Governance(GovernanceCommand),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Register(args) => args.run(context),
            Command::Renew(args) => args.run(context),
            Command::Transfer(args) => args.run(context),
            Command::UpdateControllers(args) => args.run(context),
            Command::Freeze(args) => args.run(context),
            Command::Unfreeze(args) => args.run(context),
            Command::Registration(args) => args.run(context),
            Command::Policy(args) => args.run(context),
            Command::Governance(args) => args.run(context),
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum GovernanceCommand {
    /// Manage arbitration cases referenced by SN-6a.
    #[command(subcommand)]
    Case(CaseCommand),
}

impl Run for GovernanceCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            GovernanceCommand::Case(cmd) => cmd.run(context),
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum CaseCommand {
    /// Validate and submit a dispute case payload.
    Create(CaseCreateArgs),
    /// Export cases for transparency reporting.
    Export(CaseExportArgs),
}

impl Run for CaseCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            CaseCommand::Create(args) => args.run(context),
            CaseCommand::Export(args) => args.run(context),
        }
    }
}

#[derive(Args, Debug, Clone, Default)]
pub struct PaymentOptions {
    /// Optional path to a JSON file containing `PaymentProofV1`. When omitted the inline flags are used.
    #[arg(long = "payment-json", value_name = "PATH")]
    pub json_path: Option<PathBuf>,
    /// Payment asset identifier (e.g., `xor#sora`).
    #[arg(long = "payment-asset-id", value_name = "ASSET-ID")]
    pub asset_id: Option<String>,
    /// Gross payment amount (base + surcharges) in native units.
    #[arg(long = "payment-gross", value_name = "U64")]
    pub gross: Option<u64>,
    /// Net payment amount forwarded to the registry. Defaults to `payment-gross`.
    #[arg(long = "payment-net", value_name = "U64")]
    pub net: Option<u64>,
    /// Settlement transaction reference (string or JSON literal).
    #[arg(long = "payment-settlement", value_name = "JSON-OR-STRING")]
    pub settlement: Option<String>,
    /// Account that authorised the payment. Defaults to the CLI config account.
    #[arg(long = "payment-payer", value_name = "ACCOUNT-ID")]
    pub payer: Option<String>,
    /// Steward/treasury signature attesting to the payment (string or JSON literal).
    #[arg(long = "payment-signature", value_name = "JSON-OR-STRING")]
    pub signature: Option<String>,
}

#[derive(Args, Debug)]
pub struct RegisterArgs {
    /// Label (without suffix) to register. Automatically lower-cased & NFC-normalised.
    #[arg(long, value_name = "LABEL")]
    pub label: String,
    /// Numeric suffix identifier (see `SuffixPolicyV1::suffix_id`).
    #[arg(long = "suffix-id", value_name = "U16")]
    pub suffix_id: u16,
    /// Owner account identifier; defaults to the CLI config account.
    #[arg(long, value_name = "ACCOUNT-ID")]
    pub owner: Option<String>,
    /// Controller account identifiers (repeatable). Defaults to `[owner]`.
    #[arg(long = "controller", value_name = "ACCOUNT-ID")]
    pub controllers: Vec<String>,
    /// Registration term in years.
    #[arg(long = "term-years", value_name = "U8", default_value_t = 1)]
    pub term_years: u8,
    /// Optional pricing class hint advertised by the steward.
    #[arg(long = "pricing-class", value_name = "U8")]
    pub pricing_class_hint: Option<u8>,
    /// Payment options for the registration.
    #[command(flatten)]
    pub payment: PaymentOptions,
    /// Optional path to a JSON object that will populate `Metadata`.
    #[arg(long = "metadata-json", value_name = "PATH")]
    pub metadata_json: Option<PathBuf>,
    /// Optional path to a JSON document describing `GovernanceHookV1`.
    #[arg(long = "governance-json", value_name = "PATH")]
    pub governance_json: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct RenewArgs {
    /// Selector literal (e.g. `makoto.sora`).
    #[arg(long, value_name = "LABEL.SUFFIX")]
    pub selector: String,
    /// Additional term to purchase (years).
    #[arg(long = "term-years", value_name = "U8", default_value_t = 1)]
    pub term_years: u8,
    /// Payment options covering the renewal fee.
    #[command(flatten)]
    pub payment: PaymentOptions,
}

#[derive(Args, Debug)]
pub struct TransferArgs {
    /// Selector literal (e.g. `makoto.sora`).
    #[arg(long, value_name = "LABEL.SUFFIX")]
    pub selector: String,
    /// New owner account identifier.
    #[arg(long = "new-owner", value_name = "ACCOUNT-ID")]
    pub new_owner: String,
    /// Path to `GovernanceHookV1` JSON proving transfer approval.
    #[arg(long = "governance-json", value_name = "PATH")]
    pub governance_json: PathBuf,
}

#[derive(Args, Debug)]
pub struct UpdateControllersArgs {
    /// Selector literal (e.g. `makoto.sora`).
    #[arg(long, value_name = "LABEL.SUFFIX")]
    pub selector: String,
    /// Replacement controller account identifiers (repeatable). Defaults to `[config account]`.
    #[arg(long = "controller", value_name = "ACCOUNT-ID")]
    pub controllers: Vec<String>,
}

#[derive(Args, Debug)]
pub struct FreezeArgs {
    /// Selector literal (e.g. `makoto.sora`).
    #[arg(long, value_name = "LABEL.SUFFIX")]
    pub selector: String,
    /// Reason recorded in the freeze log.
    #[arg(long, value_name = "TEXT")]
    pub reason: String,
    /// Timestamp (ms since epoch) when the freeze should auto-expire.
    #[arg(long = "until-ms", value_name = "U64")]
    pub until_ms: u64,
    /// Guardian ticket signature (string or JSON literal).
    #[arg(long = "guardian-ticket", value_name = "JSON-OR-STRING")]
    pub guardian_ticket: String,
}

#[derive(Args, Debug)]
pub struct UnfreezeArgs {
    /// Selector literal (e.g. `makoto.sora`).
    #[arg(long, value_name = "LABEL.SUFFIX")]
    pub selector: String,
    /// Path to `GovernanceHookV1` JSON authorising the unfreeze.
    #[arg(long = "governance-json", value_name = "PATH")]
    pub governance_json: PathBuf,
}

#[derive(Args, Debug)]
pub struct CaseCreateArgs {
    /// Path to the arbitration case payload (JSON).
    #[arg(long = "case-json", value_name = "PATH")]
    pub case_json: PathBuf,
    /// Optional path to a JSON schema. Defaults to the embedded SN-6a schema.
    #[arg(long = "schema", value_name = "PATH")]
    pub schema: Option<PathBuf>,
    /// Validate the payload only; do not submit to Torii.
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

#[derive(Args, Debug, Default)]
pub struct CaseExportArgs {
    /// Filter to cases updated after the provided ISO-8601 timestamp.
    #[arg(long = "since", value_name = "ISO-8601")]
    pub since: Option<String>,
    /// Optional status filter (open, triage, decision, remediation, closed, suspended).
    #[arg(long = "status", value_name = "STATUS")]
    pub status: Option<String>,
    /// Maximum number of cases to return.
    #[arg(long = "limit", value_name = "U32")]
    pub limit: Option<u32>,
}

impl RegisterArgs {
    fn build_request(
        &self,
        default_owner: &AccountId,
        resolve: &dyn Fn(&str) -> Result<AccountId>,
    ) -> Result<RegisterNameRequestV1> {
        let owner = match &self.owner {
            Some(raw) => resolve(raw).wrap_err("invalid owner account id")?,
            None => default_owner.clone(),
        };

        let selector =
            NameSelectorV1::new(self.suffix_id, &self.label).wrap_err("invalid selector label")?;

        let controllers = if self.controllers.is_empty() {
            vec![account_controller(&owner)?]
        } else {
            self.controllers
                .iter()
                .map(|literal| {
                    let account = resolve(literal)
                        .wrap_err_with(|| format!("invalid controller account `{literal}`"))?;
                    account_controller(&account)
                })
                .collect::<Result<Vec<_>>>()?
        };

        let metadata = match &self.metadata_json {
            Some(path) => load_metadata(path)?,
            None => Metadata::default(),
        };

        let governance = load_optional_governance(self.governance_json.as_ref())?;

        let payment = self.payment.build(&owner, resolve)?;

        Ok(RegisterNameRequestV1 {
            selector,
            owner,
            controllers,
            term_years: self.term_years,
            pricing_class_hint: self.pricing_class_hint,
            payment,
            governance,
            metadata,
        })
    }
}

impl PaymentOptions {
    fn build(
        &self,
        default_payer: &AccountId,
        resolve: &dyn Fn(&str) -> Result<AccountId>,
    ) -> Result<PaymentProofV1> {
        if let Some(path) = &self.json_path {
            return load_json_file::<PaymentProofV1>(path);
        }
        let asset_id = self.asset_id.clone().ok_or_else(|| {
            eyre!("`--payment-asset-id` is required when --payment-json is not provided")
        })?;
        let gross = self.gross.ok_or_else(|| {
            eyre!("`--payment-gross` is required when --payment-json is not provided")
        })?;
        let net = self.net.unwrap_or(gross);
        let settlement_literal = self.settlement.as_ref().ok_or_else(|| {
            eyre!("`--payment-settlement` is required when --payment-json is not provided")
        })?;
        let settlement_tx = parse_json_literal(settlement_literal);
        let payer = match &self.payer {
            Some(literal) => {
                resolve(literal).wrap_err("invalid payment payer account id")?
            }
            None => default_payer.clone(),
        };
        let signature_literal = self.signature.as_ref().ok_or_else(|| {
            eyre!("`--payment-signature` is required when --payment-json is not provided")
        })?;
        let signature = parse_json_literal(signature_literal);
        Ok(PaymentProofV1 {
            asset_id,
            gross_amount: gross,
            net_amount: net,
            settlement_tx,
            payer,
            signature,
        })
    }
}

impl Run for RegisterArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        let request = self.build_request(&context.config().account, &|literal| {
            crate::resolve_account_id(context, literal)
        })?;
        let response = client.sns().register(&request)?;
        context.print_data(&response)
    }
}

impl RenewArgs {
    fn selector_literal(&self) -> &str {
        self.selector.trim()
    }

    fn build_request(
        &self,
        default_payer: &AccountId,
        resolve: &dyn Fn(&str) -> Result<AccountId>,
    ) -> Result<RenewNameRequestV1> {
        Ok(RenewNameRequestV1 {
            term_years: self.term_years,
            payment: self.payment.build(default_payer, resolve)?,
        })
    }
}

impl Run for RenewArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        let request = self.build_request(&context.config().account, &|literal| {
            crate::resolve_account_id(context, literal)
        })?;
        let record = client.sns().renew(self.selector_literal(), &request)?;
        context.print_data(&record)
    }
}

impl TransferArgs {
    fn selector_literal(&self) -> &str {
        self.selector.trim()
    }

    fn build_request(
        &self,
        resolve: &dyn Fn(&str) -> Result<AccountId>,
    ) -> Result<TransferNameRequestV1> {
        let new_owner =
            resolve(&self.new_owner).wrap_err("invalid new owner account id")?;
        let governance =
            load_json_file::<GovernanceHookV1>(&self.governance_json).wrap_err_with(|| {
                format!(
                    "failed to load governance hook from `{}`",
                    self.governance_json.display()
                )
            })?;
        Ok(TransferNameRequestV1 {
            new_owner,
            governance,
        })
    }
}

impl Run for TransferArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let request = self.build_request(&|literal| crate::resolve_account_id(context, literal))?;
        let record = context
            .client_from_config()
            .sns()
            .transfer(self.selector_literal(), &request)?;
        context.print_data(&record)
    }
}

impl UpdateControllersArgs {
    fn selector_literal(&self) -> &str {
        self.selector.trim()
    }

    fn build_request(
        &self,
        default_controller: &AccountId,
        resolve: &dyn Fn(&str) -> Result<AccountId>,
    ) -> Result<UpdateControllersRequestV1> {
        let controllers = if self.controllers.is_empty() {
            vec![account_controller(default_controller)?]
        } else {
            self.controllers
                .iter()
                .map(|literal| {
                    let account = resolve(literal)
                        .wrap_err_with(|| format!("invalid controller account `{literal}`"))?;
                    account_controller(&account)
                })
                .collect::<Result<Vec<_>>>()?
        };
        Ok(UpdateControllersRequestV1 { controllers })
    }
}

impl Run for UpdateControllersArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let request = self.build_request(&context.config().account, &|literal| {
            crate::resolve_account_id(context, literal)
        })?;
        let record = context
            .client_from_config()
            .sns()
            .update_controllers(self.selector_literal(), &request)?;
        context.print_data(&record)
    }
}

impl FreezeArgs {
    fn selector_literal(&self) -> &str {
        self.selector.trim()
    }

    fn build_request(&self) -> FreezeNameRequestV1 {
        FreezeNameRequestV1 {
            reason: self.reason.clone(),
            until_ms: self.until_ms,
            guardian_ticket: parse_json_literal(&self.guardian_ticket),
        }
    }
}

impl Run for FreezeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let request = self.build_request();
        let record = context
            .client_from_config()
            .sns()
            .freeze(self.selector_literal(), &request)?;
        context.print_data(&record)
    }
}

impl UnfreezeArgs {
    fn selector_literal(&self) -> &str {
        self.selector.trim()
    }

    fn governance(&self) -> Result<GovernanceHookV1> {
        load_json_file::<GovernanceHookV1>(&self.governance_json).wrap_err_with(|| {
            format!(
                "failed to load governance hook from `{}`",
                self.governance_json.display()
            )
        })
    }
}

impl Run for UnfreezeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let hook = self.governance()?;
        let record = context
            .client_from_config()
            .sns()
            .unfreeze(self.selector_literal(), &hook)?;
        context.print_data(&record)
    }
}

#[derive(Args, Debug)]
pub struct GetRegistrationArgs {
    /// Selector literal (`label.suffix`). IH58 (preferred)/snx1 (second-best) inputs are accepted.
    #[arg(long, value_name = "SELECTOR")]
    pub selector: String,
}

impl Run for GetRegistrationArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        let record = client.sns().get_registration(&self.selector)?;
        context.print_data(&record)
    }
}

#[derive(Args, Debug)]
pub struct GetPolicyArgs {
    /// Numeric suffix identifier (`SuffixPolicyV1::suffix_id`).
    #[arg(long = "suffix-id", value_name = "U16")]
    pub suffix_id: u16,
}

impl Run for GetPolicyArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        let policy = client.sns().get_policy(self.suffix_id)?;
        let catalog_version = suffix_catalog().version;
        let catalog_entry = suffix_catalog().get(self.suffix_id);
        let mut verify_error = None;
        let mut catalog_error = None;
        if let Some(entry) = catalog_entry {
            let resolve = |literal: &str| crate::resolve_account_id(context, literal);
            if let Err(err) = entry
                .verify_policy(&policy, &resolve)
                .wrap_err_with(|| format!("catalog mismatch for suffix id {}", self.suffix_id))
            {
                catalog_error = Some(err.to_string());
                verify_error = Some(err);
            }
        }

        let output =
            build_policy_output_value(&policy, catalog_entry, catalog_version, catalog_error.as_deref())?;
        let text = render_policy_summary_text(
            &policy,
            catalog_entry,
            catalog_version,
            catalog_error.as_deref(),
        );
        print_with_optional_text(context, Some(text), &output)?;

        if let Some(err) = verify_error {
            return Err(err);
        }
        Ok(())
    }
}

fn build_policy_output_value(
    policy: &SuffixPolicyV1,
    catalog_entry: Option<&SuffixCatalogEntry>,
    catalog_version: u32,
    catalog_error: Option<&str>,
) -> Result<Value> {
    let mut map = Map::new();
    let policy_value = json::to_value(policy).wrap_err("failed to serialize suffix policy")?;
    map.insert("policy".into(), policy_value);
    map.insert(
        "catalog_version".into(),
        Value::from(u64::from(catalog_version)),
    );
    match catalog_entry {
        Some(entry) => {
            let entry_value =
                json::to_value(entry).wrap_err("failed to serialize suffix catalog entry")?;
            map.insert("catalog_entry".into(), entry_value);
            map.insert("catalog_present".into(), Value::from(true));
            map.insert(
                "catalog_match".into(),
                Value::from(catalog_error.is_none()),
            );
        }
        None => {
            map.insert("catalog_entry".into(), Value::Null);
            map.insert("catalog_present".into(), Value::from(false));
            map.insert("catalog_match".into(), Value::Null);
        }
    }
    if let Some(error) = catalog_error {
        map.insert("catalog_error".into(), Value::from(error.to_string()));
    }
    Ok(Value::Object(map))
}

fn render_policy_summary_text(
    policy: &SuffixPolicyV1,
    catalog_entry: Option<&SuffixCatalogEntry>,
    catalog_version: u32,
    catalog_error: Option<&str>,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "SNS suffix policy");
    let _ = writeln!(out, "suffix_id: {}", policy.suffix_id);
    let _ = writeln!(out, "suffix: {}", policy.suffix);
    let _ = writeln!(out, "status: {:?}", policy.status);
    let _ = writeln!(out, "steward: {}", policy.steward);
    let _ = writeln!(out, "payment_asset_id: {}", policy.payment_asset_id);
    let _ = writeln!(
        out,
        "term_years: {}-{}",
        policy.min_term_years, policy.max_term_years
    );
    let _ = writeln!(out, "grace_period_days: {}", policy.grace_period_days);
    let _ = writeln!(
        out,
        "redemption_period_days: {}",
        policy.redemption_period_days
    );
    let _ = writeln!(out, "policy_version: {}", policy.policy_version);
    match catalog_entry {
        Some(entry) => {
            let _ = writeln!(
                out,
                "catalog: {} (status: {}, version {})",
                entry.suffix, entry.status, catalog_version
            );
            if let Some(error) = catalog_error {
                let _ = writeln!(out, "catalog_match: false ({error})");
            } else {
                let _ = writeln!(out, "catalog_match: true");
            }
        }
        None => {
            let _ = writeln!(
                out,
                "catalog: missing (version {})",
                catalog_version
            );
        }
    }
    out
}

impl CaseCreateArgs {
    fn load_schema(&self) -> Result<Value> {
        self.schema.as_ref().map_or_else(
            || Ok(CASE_SCHEMA_VALUE.clone()),
            |path| load_json_value(path),
        )
    }

    fn load_case(&self) -> Result<Value> {
        load_json_value(&self.case_json)
    }
}

impl Run for CaseCreateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let schema = self.load_schema()?;
        let case_payload = self.load_case()?;
        validate_case_against_schema(&schema, &case_payload, "case")?;
        if self.dry_run {
            let text = "Arbitration case validated (dry-run).".to_string();
            print_with_optional_text(context, Some(text), &case_payload)
        } else {
            let result = context
                .client_from_config()
                .sns()
                .create_case(&case_payload)?;
            print_with_optional_text(context, None, &result)
        }
    }
}

impl Run for CaseExportArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let query = CaseExportQuery {
            since: self.since.as_deref(),
            status: self.status.as_deref(),
            limit: self.limit,
        };
        let response = context.client_from_config().sns().export_cases(query)?;
        context.print_data(&response)
    }
}

fn account_controller(account: &AccountId) -> Result<NameControllerV1> {
    let address =
        AccountAddress::from_account_id(account).wrap_err("failed to encode controller address")?;
    Ok(NameControllerV1::account(&address))
}

fn load_optional_governance(path: Option<&PathBuf>) -> Result<Option<GovernanceHookV1>> {
    path.map_or_else(
        || Ok(None),
        |p| load_json_file::<GovernanceHookV1>(p).map(Some),
    )
}

fn load_metadata(path: &Path) -> Result<Metadata> {
    let contents = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read metadata JSON from `{}`", path.display()))?;
    let value: norito::json::Value =
        norito::json::from_str(&contents).wrap_err("invalid metadata JSON")?;
    let obj = value
        .as_object()
        .ok_or_else(|| eyre!("metadata JSON must be an object"))?;
    let mut metadata = Metadata::default();
    for (key, value) in obj {
        let name = Name::from_str(key).wrap_err_with(|| format!("invalid metadata key `{key}`"))?;
        metadata.insert(name, value.clone());
    }
    Ok(metadata)
}

fn load_json_value(path: &Path) -> Result<Value> {
    let contents = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read JSON from `{}`", path.display()))?;
    json::from_str(&contents).wrap_err_with(|| {
        format!(
            "failed to parse JSON in `{}`; ensure it is valid Norito JSON",
            path.display()
        )
    })
}

fn suffix_catalog() -> &'static SuffixCatalog {
    &SUFFIX_CATALOG
}

#[allow(dead_code)]
#[derive(Debug, Clone, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct SuffixCatalog {
    version: u32,
    generated_at: String,
    #[norito(default)]
    suffixes: Vec<SuffixCatalogEntry>,
}

impl SuffixCatalog {
    fn get(&self, suffix_id: u16) -> Option<&SuffixCatalogEntry> {
        self.suffixes
            .iter()
            .find(|entry| entry.suffix_id == suffix_id)
    }
}

#[derive(Debug, Clone, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct SuffixCatalogEntry {
    suffix: String,
    suffix_id: u16,
    status: String,
    steward_account: String,
    fund_splitter_account: String,
    payment_asset_id: String,
    referral_cap_bps: u16,
    min_term_years: u8,
    max_term_years: u8,
    grace_period_days: u16,
    redemption_period_days: u16,
    policy_version: u16,
    #[norito(default)]
    reserved_labels: Vec<CatalogReservedLabel>,
    #[norito(default)]
    pricing: Vec<CatalogPriceTier>,
    fee_split: CatalogFeeSplit,
}

#[derive(Debug, Clone, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct CatalogReservedLabel {
    label: String,
    #[norito(default)]
    assigned_to: Option<String>,
    #[norito(default)]
    release_at_ms: Option<u64>,
    #[norito(default)]
    note: String,
}

#[derive(Debug, Clone, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct CatalogTokenValue {
    asset_id: String,
    amount: u128,
}

#[derive(Debug, Clone, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct CatalogPriceTier {
    tier_id: u8,
    label_regex: String,
    base_price: CatalogTokenValue,
    auction_kind: String,
    #[norito(default)]
    dutch_floor: Option<CatalogTokenValue>,
    min_duration_years: u8,
    max_duration_years: u8,
}

#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct CatalogFeeSplit {
    treasury_bps: u16,
    steward_bps: u16,
    referral_max_bps: u16,
    escrow_bps: u16,
}

impl CatalogFeeSplit {
    fn to_model(&self) -> SuffixFeeSplitV1 {
        SuffixFeeSplitV1 {
            treasury_bps: self.treasury_bps,
            steward_bps: self.steward_bps,
            referral_max_bps: self.referral_max_bps,
            escrow_bps: self.escrow_bps,
        }
    }
}

impl CatalogTokenValue {
    fn to_model(&self) -> TokenValue {
        TokenValue::new(self.asset_id.clone(), self.amount)
    }
}

impl CatalogPriceTier {
    fn to_model(&self) -> Result<PriceTierV1> {
        Ok(PriceTierV1 {
            tier_id: self.tier_id,
            label_regex: self.label_regex.clone(),
            base_price: self.base_price.to_model(),
            auction_kind: parse_auction_kind(&self.auction_kind)
                .wrap_err_with(|| format!("invalid auction_kind `{}`", self.auction_kind))?,
            dutch_floor: self.dutch_floor.as_ref().map(CatalogTokenValue::to_model),
            min_duration_years: self.min_duration_years,
            max_duration_years: self.max_duration_years,
        })
    }
}

impl CatalogReservedLabel {
    fn to_model(&self, resolve: &dyn Fn(&str) -> Result<AccountId>) -> Result<ReservedNameV1> {
        let assigned = match self.assigned_to.as_deref() {
            Some(raw) if !raw.trim().is_empty() => {
                let literal = raw.trim();
                Some(resolve(literal).wrap_err_with(|| {
                    format!(
                        "invalid account `{literal}` in reserved label `{}`",
                        self.label
                    )
                })?)
            }
            _ => None,
        };
        Ok(ReservedNameV1 {
            normalized_label: self.label.to_lowercase(),
            assigned_to: assigned,
            release_at_ms: self.release_at_ms,
            note: self.note.clone(),
        })
    }
}

fn parse_status(value: &str) -> Result<SuffixStatus> {
    match value.trim().to_lowercase().as_str() {
        "active" => Ok(SuffixStatus::Active),
        "paused" => Ok(SuffixStatus::Paused),
        "revoked" => Ok(SuffixStatus::Revoked),
        other => Err(eyre!("unknown suffix status `{other}`")),
    }
}

fn parse_auction_kind(value: &str) -> Result<AuctionKind> {
    match value.trim().to_lowercase().as_str() {
        "vickrey_commit_reveal" | "vickrey" => Ok(AuctionKind::VickreyCommitReveal),
        "dutch_reopen" | "dutch" => Ok(AuctionKind::DutchReopen),
        other => Err(eyre!("unknown auction_kind `{other}`")),
    }
}

impl SuffixCatalogEntry {
    #[allow(clippy::too_many_lines)]
    fn verify_policy(
        &self,
        policy: &SuffixPolicyV1,
        resolve: &dyn Fn(&str) -> Result<AccountId>,
    ) -> Result<()> {
        let mut mismatches = Vec::new();
        let catalog_suffix = self.suffix.trim_start_matches('.').to_lowercase();
        let policy_suffix = policy.suffix.to_lowercase();
        if catalog_suffix != policy_suffix {
            mismatches.push(format!(
                "suffix differs (catalog `{catalog_suffix}`, policy `{policy_suffix}`)"
            ));
        }
        let expected_steward = resolve(self.steward_account.trim()).wrap_err_with(|| {
            format!("invalid steward account `{}`", self.steward_account)
        })?;
        if policy.steward != expected_steward {
            mismatches.push(format!(
                "steward differs (catalog `{expected_steward}`, policy `{}`)",
                policy.steward
            ));
        }
        let expected_status = parse_status(&self.status)
            .wrap_err_with(|| format!("invalid status `{}`", self.status))?;
        if policy.status != expected_status {
            mismatches.push(format!(
                "status differs (catalog {:?}, policy {:?})",
                expected_status, policy.status
            ));
        }
        if self.payment_asset_id != policy.payment_asset_id {
            mismatches.push(format!(
                "payment asset differs (catalog `{}`, policy `{}`)",
                self.payment_asset_id, policy.payment_asset_id
            ));
        }
        if self.min_term_years != policy.min_term_years {
            mismatches.push(format!(
                "min_term_years differs (catalog {}, policy {})",
                self.min_term_years, policy.min_term_years
            ));
        }
        if self.max_term_years != policy.max_term_years {
            mismatches.push(format!(
                "max_term_years differs (catalog {}, policy {})",
                self.max_term_years, policy.max_term_years
            ));
        }
        if self.grace_period_days != policy.grace_period_days {
            mismatches.push(format!(
                "grace_period_days differs (catalog {}, policy {})",
                self.grace_period_days, policy.grace_period_days
            ));
        }
        if self.redemption_period_days != policy.redemption_period_days {
            mismatches.push(format!(
                "redemption_period_days differs (catalog {}, policy {})",
                self.redemption_period_days, policy.redemption_period_days
            ));
        }
        if self.referral_cap_bps != policy.referral_cap_bps {
            mismatches.push(format!(
                "referral_cap_bps differs (catalog {}, policy {})",
                self.referral_cap_bps, policy.referral_cap_bps
            ));
        }
        let expected_fund_splitter = resolve(self.fund_splitter_account.trim()).wrap_err_with(
            || {
                format!(
                    "invalid fund_splitter_account `{}`",
                    self.fund_splitter_account
                )
            },
        )?;
        if policy.fund_splitter_account != expected_fund_splitter {
            mismatches.push(format!(
                "fund_splitter_account differs (catalog `{expected_fund_splitter}`, policy `{}`)",
                policy.fund_splitter_account
            ));
        }
        if self.policy_version != policy.policy_version {
            mismatches.push(format!(
                "policy_version differs (catalog {}, policy {})",
                self.policy_version, policy.policy_version
            ));
        }
        let expected_fee_split = self.fee_split.to_model();
        if policy.fee_split != expected_fee_split {
            mismatches.push(format!(
                "fee_split differs (catalog {:?}, policy {:?})",
                expected_fee_split, policy.fee_split
            ));
        }
        let mut catalog_reserved = self
            .reserved_labels
            .iter()
            .map(|label| label.to_model(resolve))
            .collect::<Result<Vec<_>>>()?;
        let mut policy_reserved = policy.reserved_labels.clone();
        catalog_reserved.sort_by(|a, b| a.normalized_label.cmp(&b.normalized_label));
        policy_reserved.sort_by(|a, b| a.normalized_label.cmp(&b.normalized_label));
        if catalog_reserved != policy_reserved {
            mismatches.push(format!(
                "reserved_labels differ (catalog {catalog_reserved:?}, policy {policy_reserved:?})"
            ));
        }
        let mut catalog_pricing = self
            .pricing
            .iter()
            .map(CatalogPriceTier::to_model)
            .collect::<Result<Vec<_>>>()?;
        let mut policy_pricing = policy.pricing.clone();
        catalog_pricing.sort_by_key(|tier| tier.tier_id);
        policy_pricing.sort_by_key(|tier| tier.tier_id);
        if catalog_pricing != policy_pricing {
            mismatches.push(format!(
                "pricing tiers differ (catalog {catalog_pricing:?}, policy {policy_pricing:?})"
            ));
        }
        if mismatches.is_empty() {
            Ok(())
        } else {
            Err(eyre!(
                "suffix policy does not match catalog entry `{}`:\n  - {}",
                self.suffix,
                mismatches.join("\n  - ")
            ))
        }
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    use iroha::data_model::sns::fixtures;

    fn resolve_catalog_account(literal: &str) -> Result<AccountId> {
        AccountId::from_str(literal)
            .wrap_err_with(|| format!("invalid catalog account `{literal}`"))
    }

    #[test]
    fn catalog_entry_matches_default_policy() {
        let entry = suffix_catalog()
            .get(0x0001)
            .expect("catalog contains .sora");
        let policy = fixtures::default_policy();
        entry
            .verify_policy(&policy, &resolve_catalog_account)
            .expect("catalog should align");
    }

    #[test]
    fn catalog_detects_price_mismatch() {
        let entry = suffix_catalog()
            .get(0x0001)
            .expect("catalog contains .sora");
        let mut policy = fixtures::default_policy();
        if let Some(tier) = policy.pricing.first_mut() {
            tier.base_price.amount = tier.base_price.amount.saturating_add(1);
        }
        let err = entry
            .verify_policy(&policy, &resolve_catalog_account)
            .expect_err("mismatch must trigger error");
        assert!(
            err.to_string().contains("pricing tiers differ"),
            "unexpected error: {err}"
        );
    }
}

fn validate_case_against_schema(schema: &Value, candidate: &Value, path: &str) -> Result<()> {
    validate_value(schema, candidate, path)
}

fn validate_value(schema: &Value, candidate: &Value, path: &str) -> Result<()> {
    if let Some(enum_values) = schema.get("enum").and_then(|v| v.as_array()) {
        let value = candidate
            .as_str()
            .ok_or_else(|| eyre!("{path} must be a string to match enum constraints"))?;
        let allowed = enum_values
            .iter()
            .filter_map(|entry| entry.as_str())
            .collect::<Vec<_>>();
        if !allowed.contains(&value) {
            return Err(eyre!(
                "`{path}` must match one of {:?}; found `{value}`",
                allowed
            ));
        }
    }

    if let Some(type_name) = schema.get("type").and_then(|t| t.as_str()) {
        match type_name {
            "object" => validate_object(schema, candidate, path)?,
            "array" => validate_array(schema, candidate, path)?,
            "string" => {
                candidate
                    .as_str()
                    .ok_or_else(|| eyre!("{path} must be a string"))?;
            }
            "integer" => {
                if !(candidate.as_i64().is_some() || candidate.as_u64().is_some()) {
                    return Err(eyre!("{path} must be an integer"));
                }
            }
            "number" => {
                if !(candidate.as_f64().is_some()
                    || candidate.as_i64().is_some()
                    || candidate.as_u64().is_some())
                {
                    return Err(eyre!("{path} must be numeric"));
                }
            }
            "boolean" => {
                if candidate.as_bool().is_none() {
                    return Err(eyre!("{path} must be a boolean"));
                }
            }
            _ => {}
        }
    } else if schema.get("properties").is_some() {
        // Treat schemas with implicit object type as objects.
        validate_object(schema, candidate, path)?;
    }

    Ok(())
}

fn validate_object(schema: &Value, candidate: &Value, path: &str) -> Result<()> {
    let obj = candidate
        .as_object()
        .ok_or_else(|| eyre!("{path} must be an object"))?;
    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        for entry in required {
            let field = entry
                .as_str()
                .ok_or_else(|| eyre!("invalid required entry in schema for `{path}`"))?;
            if !obj.contains_key(field) {
                return Err(eyre!("`{path}` is missing required field `{field}`"));
            }
        }
    }

    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for (key, child_schema) in props {
            if let Some(child_value) = obj.get(key) {
                let child_path = format!("{path}.{key}");
                validate_value(child_schema, child_value, &child_path)?;
            }
        }
    }

    Ok(())
}

fn validate_array(schema: &Value, candidate: &Value, path: &str) -> Result<()> {
    let arr = candidate
        .as_array()
        .ok_or_else(|| eyre!("{path} must be an array"))?;
    if let Some(min_items_value) = schema.get("minItems").and_then(Value::as_u64) {
        let min_items = usize::try_from(min_items_value)
            .map_err(|_| eyre!("{path} minimum exceeds supported usize range"))?;
        if arr.len() < min_items {
            return Err(eyre!(
                "{path} must contain at least {min_items} entries (found {})",
                arr.len()
            ));
        }
    }

    if let Some(items) = schema.get("items") {
        for (index, entry) in arr.iter().enumerate() {
            let child_path = format!("{path}[{index}]");
            validate_value(items, entry, &child_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod schema_tests {
    use super::*;

    fn sample_case() -> Value {
        norito::json!({
            "case_id": "SNS-2026-00001",
            "selector": {
                "suffix_id": 1,
                "label": "alpha",
                "global_form": "alpha.sora"
            },
            "dispute_type": "ownership",
            "priority": "high",
            "reported_at": "2026-04-26T00:00:00Z",
            "status": "open",
            "reporter": {
                "role": "registrar",
                "contact": "ops@sora.net"
            },
            "respondents": [
                {"role": "registrant", "account_id": "alice@wonderland"}
            ],
            "allegations": [
                {"code": "REG-1", "summary": "Ownership conflict"}
            ],
            "evidence": [
                {
                    "id": "doc-1",
                    "kind": "document",
                    "hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                    "sealed": false
                }
            ],
            "sla": {
                "acknowledge_by": "2026-04-26T02:00:00Z",
                "resolution_by": "2026-04-29T02:00:00Z"
            },
            "actions": [
                {"timestamp": "2026-04-26T00:00:00Z", "actor": "registrar", "action": "submitted"}
            ]
        })
    }

    #[test]
    fn validates_sample_case() {
        let schema = CASE_SCHEMA_VALUE.clone();
        let payload = sample_case();
        validate_case_against_schema(&schema, &payload, "case").expect("sample payload valid");
    }

    #[test]
    fn detects_missing_required_field() {
        let schema = CASE_SCHEMA_VALUE.clone();
        let mut payload = sample_case();
        payload
            .as_object_mut()
            .expect("payload is object")
            .remove("actions");
        let err = validate_case_against_schema(&schema, &payload, "case").unwrap_err();
        assert!(err.to_string().contains("actions"));
    }
}

fn load_json_file<T>(path: &Path) -> Result<T>
where
    T: norito::json::JsonDeserialize,
{
    let contents = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read JSON from `{}`", path.display()))?;
    norito::json::from_str(&contents).wrap_err("invalid JSON payload")
}

fn parse_json_literal(raw: &str) -> Json {
    norito::json::from_str(raw).unwrap_or_else(|_| Json::from(raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::sns::fixtures;
    use iroha::data_model::sns::ControllerType;
    use iroha_test_samples::ALICE_ID;
    use tempfile::NamedTempFile;

    fn default_owner() -> AccountId {
        (*ALICE_ID).clone()
    }

    fn resolve_account_literal(literal: &str) -> Result<AccountId> {
        AccountId::from_str(literal)
            .wrap_err_with(|| format!("invalid test account literal `{literal}`"))
    }

    fn base_register_args() -> RegisterArgs {
        RegisterArgs {
            label: "Makoto".into(),
            suffix_id: 0x0001,
            owner: None,
            controllers: Vec::new(),
            term_years: 1,
            pricing_class_hint: None,
            payment: sample_payment_options(),
            metadata_json: None,
            governance_json: None,
        }
    }

    fn sample_payment_options() -> PaymentOptions {
        PaymentOptions {
            asset_id: Some("xor#sora".into()),
            gross: Some(120),
            settlement: Some("\"tx-1\"".into()),
            signature: Some("\"sig\"".into()),
            ..PaymentOptions::default()
        }
    }

    fn sample_catalog_entry() -> SuffixCatalogEntry {
        SuffixCatalogEntry {
            suffix: "sora".to_string(),
            suffix_id: 1,
            status: "active".to_string(),
            steward_account: "alice@wonderland".to_string(),
            fund_splitter_account: "splitter@wonderland".to_string(),
            payment_asset_id: "xor#sora".to_string(),
            referral_cap_bps: 0,
            min_term_years: 1,
            max_term_years: 2,
            grace_period_days: 30,
            redemption_period_days: 30,
            policy_version: 1,
            reserved_labels: Vec::new(),
            pricing: Vec::new(),
            fee_split: CatalogFeeSplit {
                treasury_bps: 50,
                steward_bps: 50,
                referral_max_bps: 0,
                escrow_bps: 0,
            },
        }
    }

    #[test]
    fn build_request_defaults_owner_and_controller() {
        let args = base_register_args();
        let request = args
            .build_request(&default_owner(), &resolve_account_literal)
            .expect("request");
        assert_eq!(request.owner, *ALICE_ID);
        assert_eq!(request.controllers.len(), 1);
        assert_eq!(
            request.controllers[0].controller_type,
            ControllerType::Account
        );
        assert_eq!(request.selector.normalized_label(), "makoto");
        assert_eq!(request.payment.net_amount, 120);
    }

    #[test]
    fn parse_json_literal_falls_back_to_string() {
        let value = parse_json_literal("opaque-sig");
        assert_eq!(value, Json::from("opaque-sig"));
        let complex = parse_json_literal("{\"hash\":\"0x01\"}");
        let parsed: norito::json::Value =
            norito::json::from_str(complex.get()).expect("parse json literal");
        let hash = parsed
            .as_object()
            .and_then(|obj| obj.get("hash"))
            .and_then(|entry| entry.as_str());
        assert_eq!(hash, Some("0x01"));
    }

    #[test]
    fn load_metadata_rejects_non_object() {
        let file = NamedTempFile::new().expect("temp file");
        fs::write(file.path(), "[1,2,3]").expect("write");
        let err = load_metadata(file.path()).expect_err("must fail");
        assert!(
            err.to_string().contains("metadata JSON must be an object"),
            "{err:?}"
        );
    }

    #[test]
    fn renew_args_builds_request() {
        let args = RenewArgs {
            selector: "makoto.sora".into(),
            term_years: 2,
            payment: sample_payment_options(),
        };
        let payload = args
            .build_request(&default_owner(), &resolve_account_literal)
            .expect("renew payload");
        assert_eq!(payload.term_years, 2);
        assert_eq!(payload.payment.gross_amount, 120);
    }

    #[test]
    fn build_policy_output_value_includes_catalog() {
        let policy = fixtures::default_policy();
        let entry = sample_catalog_entry();
        let value = build_policy_output_value(&policy, Some(&entry), 7, None)
            .expect("policy output");
        let obj = value.as_object().expect("policy output object");
        assert_eq!(obj.get("catalog_version").and_then(Value::as_u64), Some(7));
        assert_eq!(
            obj.get("catalog_match").and_then(Value::as_bool),
            Some(true)
        );
        assert!(obj.get("catalog_entry").is_some());
    }

    #[test]
    fn render_policy_summary_text_mentions_catalog() {
        let policy = fixtures::default_policy();
        let entry = sample_catalog_entry();
        let text = render_policy_summary_text(&policy, Some(&entry), 7, None);
        assert!(text.contains("catalog: sora"));
        assert!(text.contains("catalog_match: true"));
    }

    #[test]
    fn build_policy_output_value_records_error() {
        let policy = fixtures::default_policy();
        let entry = sample_catalog_entry();
        let value = build_policy_output_value(
            &policy,
            Some(&entry),
            7,
            Some("mismatch details"),
        )
        .expect("policy output");
        let obj = value.as_object().expect("policy output object");
        assert_eq!(
            obj.get("catalog_match").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            obj.get("catalog_error").and_then(Value::as_str),
            Some("mismatch details")
        );
    }

    #[test]
    fn transfer_args_loads_governance_hook() {
        let gov = sample_governance_file();
        let new_owner = default_owner().to_string();
        let args = TransferArgs {
            selector: "makoto.sora".into(),
            new_owner: new_owner.clone(),
            governance_json: gov.path().to_path_buf(),
        };
        let payload = args
            .build_request(&resolve_account_literal)
            .expect("transfer payload");
        assert_eq!(payload.new_owner, new_owner.parse().unwrap());
        assert_eq!(payload.governance.proposal_id, "proposal-1");
    }

    #[test]
    fn freeze_args_parse_guardian_ticket() {
        let args = FreezeArgs {
            selector: "makoto.sora".into(),
            reason: "guardian freeze".into(),
            until_ms: 42,
            guardian_ticket: "{\"sig\":\"abc\"}".into(),
        };
        let payload = args.build_request();
        assert_eq!(payload.reason, "guardian freeze");
        assert_eq!(payload.until_ms, 42);
        let ticket: norito::json::Value =
            norito::json::from_str(payload.guardian_ticket.get()).expect("guardian ticket json");
        let sig = ticket
            .as_object()
            .and_then(|obj| obj.get("sig"))
            .and_then(|entry| entry.as_str());
        assert_eq!(sig, Some("abc"));
    }

    #[test]
    fn unfreeze_args_loads_governance() {
        let gov = sample_governance_file();
        let args = UnfreezeArgs {
            selector: "makoto.sora".into(),
            governance_json: gov.path().to_path_buf(),
        };
        let hook = args.governance().expect("governance");
        assert_eq!(hook.proposal_id, "proposal-1");
    }

    fn sample_governance_file() -> NamedTempFile {
        let file = NamedTempFile::new().expect("temp file");
        let body = r#"{
  "proposal_id": "proposal-1",
  "council_vote_hash": "0xaaa",
  "dao_vote_hash": "0xbbb",
  "steward_ack": "signature",
  "guardian_clearance": null
}"#;
        fs::write(file.path(), body).expect("write governance");
        file
    }
}
