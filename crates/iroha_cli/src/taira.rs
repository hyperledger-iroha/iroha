//! Taira public testnet diagnostics and write canaries.
use crate::{CliOutputFormat, Run, RunContext, quote_and_sign_transaction};
use eyre::{Context, Result, eyre};
use iroha::{
    client::{
        AccountFaucetClaimV1, AccountFaucetPolicyV1, AccountFaucetPreparedTransactionV1,
        AccountOnboardingCurrentStateV1, AccountOnboardingPlanReceiptV1,
        AccountOnboardingPlanRequestV1, AccountOnboardingPrepareResponseV1,
        AccountOnboardingPreparedTransactionV1, AccountOnboardingProofRequiredPrepareResponseV1,
        Client as IrohaClient, PreparedTransactionOutcomeV1, TairaPublicResetMutationBindingV1,
        TransactionWaitOptions,
    },
    config::Config,
    data_model::{
        NetworkId,
        account::{AccountId, address::ChainDiscriminantGuard},
        alias_setup::AccountAliasName,
        asset::AssetDefinitionId,
        isi::{InstructionBox, Log},
        level::Level as LogLevel,
        metadata::Metadata,
        name::Name,
        prelude::{FindTransactions, QueryBuilderExt, SignedTransaction, TransactionEntrypoint},
        query::{
            CommittedTxFilters,
            dsl::CompoundPredicate,
            parameters::{FetchSize, Pagination},
        },
        transaction::{Executable, FeePaymentIntent},
    },
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_primitives::json::Json as IrohaJson;
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::{FeeQuoteResponse, PipelineTransactionStatusResponse};
use iroha_version::codec::DecodeVersioned as _;
use norito::json::{self, JsonDeserialize, JsonSerialize, Map, Value};
use reqwest::blocking::Client as HttpClient;
use scrypt::{Params as ScryptParams, scrypt as derive_scrypt};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read as _, Write as _},
    num::NonZeroU64,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::Url;
use zeroize::Zeroizing;
const DEFAULT_PUBLIC_ROOT: &str = "https://taira.sora.org";
const DEFAULT_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const DEFAULT_CHAIN_DISCRIMINANT: u16 = 369;
/// Canonical first-release Taira fee/faucet asset definition.
pub(crate) const DEFAULT_GAS_ASSET_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
const CANARY_ALIAS_PREFIX: &str = "tairarolloutcanary";
const DEFAULT_WRITE_TTL_MS: u64 = 120_000;
const DEFAULT_WRITE_STATUS_TIMEOUT_MS: u64 = 120_000;
const PREPARED_ENVELOPE_SCHEMA_V1: &str = "iroha.taira.prepared-mutation-envelope.v1";
const PREPARED_BINDING_SCHEMA_V1: &str = "iroha.taira.public-reset.mutation-binding.v1";
const PREPARED_OPERATION_SCHEMA_V1: &str = "iroha.taira.prepared-transaction.v1";
const PREPARED_ONBOARDING_PROOF_REQUIRED_SCHEMA_V1: &str =
    "iroha.taira.prepared-onboarding-proof-required.v1";
const PREPARED_ENVELOPE_MAX_BYTES: u64 = 4 * 1024 * 1024;
const PREPARED_TRANSACTION_MAX_BYTES: usize = 1024 * 1024;
const PREPARED_TRANSACTION_CLOCK_SKEW_MS: u64 = 30_000;
const WRITE_CANARY_MUTATION_KIND: &str = "write_canary";
const WRITE_CANARY_OPERATION: &str = "final_canary";
const FAUCET_POW_ALGORITHM: &str = "scrypt-leading-zero-bits-v1";
const FAUCET_POW_DOMAIN_SEPARATOR: &[u8] = b"iroha:accounts:faucet:pow:v1";
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const REQUIRED_MCP_TOOLS: &[&str] = &[
    "iroha.health",
    "iroha.musubi.queries.exact_package",
    "iroha.musubi.queries.exact_release",
    "iroha.musubi.instructions.release_yank_set",
    "iroha.transactions.submit",
    "iroha.transactions.submit_and_wait",
];
#[derive(Clone, Copy)]
enum RouteCheckMethod {
    Get,
    PostEmptyObject,
}
const ROUTE_CHECKS: &[(&str, RouteCheckMethod, &str, &[u16])] = &[
    ("status", RouteCheckMethod::Get, "/status", &[200]),
    ("time_now", RouteCheckMethod::Get, "/v1/time/now", &[200]),
    (
        "sumeragi_status",
        RouteCheckMethod::Get,
        "/v1/sumeragi/status",
        &[401],
    ),
    (
        "pipeline_transaction_status",
        RouteCheckMethod::Get,
        "/v1/pipeline/transactions/status",
        &[400],
    ),
    (
        "sccp_capabilities",
        RouteCheckMethod::Get,
        "/v1/sccp/capabilities",
        &[200],
    ),
    (
        "zk_proofs_count",
        RouteCheckMethod::Get,
        "/v1/zk/proofs/count",
        &[200],
    ),
    (
        "public_lane_validators",
        RouteCheckMethod::Get,
        "/v1/nexus/public-lanes/0/validators",
        &[200],
    ),
    // A missing selector should reach the mounted contract-state route and be
    // rejected as bad input. Treating that as mounted keeps the doctor aligned
    // with the rollout harness instead of requiring a real contract key.
    (
        "contracts_state",
        RouteCheckMethod::Get,
        "/v1/contracts/state",
        &[400],
    ),
    // The V1 directory is POST-only and authenticates before typed body decode.
    // An unsigned malformed object must therefore fail at the mounted canonical
    // account boundary, independently of registry data.
    (
        "musubi_ordered_prefix",
        RouteCheckMethod::PostEmptyObject,
        "/v1/musubi/queries/ordered-prefix",
        &[401],
    ),
    (
        "soracloud_status",
        RouteCheckMethod::Get,
        "/v1/soracloud/status",
        &[401],
    ),
];
/// Taira public testnet helpers.
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Check Taira read-side health and MCP route posture.
    Doctor(Doctor),
    /// Preflight or execute the strictly authorized compiled public reset.
    PublicReset(crate::taira_public_reset::PublicReset),
    /// Prepare, submit, or recover exactly one authorized public-reset child.
    WriteCanary(WriteCanary),
    /// Generate the canonical deploy-mode Inrou canary workspace from AArch64 guest assets.
    InrouWorkspace(InrouWorkspace),
    /// Build the canonical offline artifact stage that operators preseed into all validators.
    InrouStage(InrouStage),
    /// Register an exact preseeded stage, mutate explicitly, and verify the four-replica Inrou canary.
    InrouCanary(InrouCanary),
    /// Revalidate an exact retained stage and verify its live four-replica service without mutation.
    InrouCheck(InrouCheck),
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Doctor(cmd) => cmd.run(context),
            Self::PublicReset(_) => eyre::bail!(
                "`taira public-reset` must be dispatched before client configuration is loaded"
            ),
            Self::WriteCanary(cmd) => cmd.run(context),
            Self::InrouWorkspace(cmd) => cmd.run(context),
            Self::InrouStage(cmd) => cmd.run(context),
            Self::InrouCanary(cmd) => cmd.run(context),
            Self::InrouCheck(cmd) => cmd.run(context),
        }
    }
}
/// Read-only Taira public endpoint diagnostics.
#[derive(clap::Args, Debug)]
pub struct Doctor {
    /// Public Torii root URL.
    #[arg(long, default_value = DEFAULT_PUBLIC_ROOT)]
    pub public_root: String,
    /// Emit a stable JSON report.
    #[arg(long)]
    pub json: bool,
}
impl Run for Doctor {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let mut output = RunContextReportOutput { context };
        self.run_with_output(&mut output)
    }
}
impl Doctor {
    fn run_with_output<O: ReportOutput>(&self, output: &mut O) -> Result<()> {
        let report = run_doctor(&self.public_root)?;
        render_report_to(output, self.json, &report)?;
        if report_status(&report) == Some("fail") {
            eyre::bail!("Taira doctor found hard failures");
        }
        Ok(())
    }

    /// Run the public read-only diagnostic without loading client configuration
    /// or constructing any signing identity.
    pub(super) fn run_without_client_config<W: std::io::Write>(
        &self,
        output_format: CliOutputFormat,
        write: W,
    ) -> Result<()> {
        let mut output = WriterReportOutput {
            write,
            output_format,
        };
        self.run_with_output(&mut output)
    }
}
type PreparedMutationBindingV1 = TairaPublicResetMutationBindingV1;

#[derive(Clone, Debug, PartialEq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct FinalCanaryPreparedTransactionV1 {
    schema: String,
    binding: PreparedMutationBindingV1,
    operation: String,
    transaction_hash_hex: String,
    signed_transaction_wire_hex: String,
    signed_transaction_wire_sha256: String,
    semantic_hash_hex: String,
    fee_payment: FeePaymentIntent,
    fee_quote: FeeQuoteResponse,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PreparedOnboardingProofRequiredV1 {
    schema: String,
    receipt: AccountOnboardingPlanReceiptV1,
    result: AccountOnboardingProofRequiredPrepareResponseV1,
}

#[derive(Clone, Debug, PartialEq, JsonSerialize, JsonDeserialize)]
#[norito(
    tag = "kind",
    content = "envelope",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum PreparedTransactionOperationV1 {
    OnboardingPrepared(AccountOnboardingPreparedTransactionV1),
    OnboardingProofRequired(PreparedOnboardingProofRequiredV1),
    FaucetPrepared(AccountFaucetPreparedTransactionV1),
    FinalCanary(FinalCanaryPreparedTransactionV1),
}

impl PreparedTransactionOperationV1 {
    fn binding(&self) -> &PreparedMutationBindingV1 {
        match self {
            Self::OnboardingPrepared(operation) => &operation.binding,
            Self::OnboardingProofRequired(operation) => &operation.result.binding,
            Self::FaucetPrepared(operation) => &operation.binding,
            Self::FinalCanary(operation) => &operation.binding,
        }
    }

    const fn label(&self) -> &'static str {
        match self {
            Self::OnboardingPrepared(_) | Self::OnboardingProofRequired(_) => "onboarding",
            Self::FaucetPrepared(_) => "faucet",
            Self::FinalCanary(_) => WRITE_CANARY_OPERATION,
        }
    }

    fn transaction_hash_hex(&self) -> Option<&str> {
        match self {
            Self::OnboardingPrepared(operation) => Some(&operation.transaction_hash_hex),
            Self::OnboardingProofRequired(_) => None,
            Self::FaucetPrepared(operation) => Some(&operation.transaction_hash_hex),
            Self::FinalCanary(operation) => Some(&operation.transaction_hash_hex),
        }
    }

    fn signed_transaction_wire_hex(&self) -> Option<&str> {
        match self {
            Self::OnboardingPrepared(operation) => Some(&operation.signed_transaction_wire_hex),
            Self::OnboardingProofRequired(_) => None,
            Self::FaucetPrepared(operation) => Some(&operation.signed_transaction_wire_hex),
            Self::FinalCanary(operation) => Some(&operation.signed_transaction_wire_hex),
        }
    }

    fn signed_transaction_wire_sha256(&self) -> Option<&str> {
        match self {
            Self::OnboardingPrepared(operation) => Some(&operation.signed_transaction_wire_sha256),
            Self::OnboardingProofRequired(_) => None,
            Self::FaucetPrepared(operation) => Some(&operation.signed_transaction_wire_sha256),
            Self::FinalCanary(operation) => Some(&operation.signed_transaction_wire_sha256),
        }
    }

    fn semantic_hash_hex(&self) -> &str {
        match self {
            Self::OnboardingPrepared(operation) => &operation.semantic_hash_hex,
            Self::OnboardingProofRequired(operation) => &operation.result.semantic_hash_hex,
            Self::FaucetPrepared(operation) => &operation.semantic_hash_hex,
            Self::FinalCanary(operation) => &operation.semantic_hash_hex,
        }
    }

    fn fee_payment(&self) -> Option<&FeePaymentIntent> {
        match self {
            Self::OnboardingPrepared(operation) => Some(&operation.fee_payment),
            Self::OnboardingProofRequired(_) => None,
            Self::FaucetPrepared(operation) => Some(&operation.fee_payment),
            Self::FinalCanary(operation) => Some(&operation.fee_payment),
        }
    }

    fn fee_quote(&self) -> Option<&FeeQuoteResponse> {
        match self {
            Self::FinalCanary(operation) => Some(&operation.fee_quote),
            Self::OnboardingPrepared(_)
            | Self::OnboardingProofRequired(_)
            | Self::FaucetPrepared(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PreparedMutationEnvelopeV1 {
    schema: String,
    binding: PreparedMutationBindingV1,
    public_root: String,
    chain_id: String,
    network_id: String,
    authority: String,
    operation: PreparedTransactionOperationV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreparedEnvelopeAction {
    Prepare(u32),
    Submit(u32),
    Recover(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreparedLifetimeCheck {
    Structural,
    LiveForward,
}

/// One strictly ordered mutation in the Taira write-canary bootstrap.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteCanaryOperation {
    /// Prepare, submit, or recover the sponsored account/alias onboarding transaction.
    Onboarding,
    /// Prepare, submit, or recover the faucet funding transaction.
    Faucet,
    /// Prepare, submit, or recover the final authority-signed log canary transaction.
    FinalCanary,
}

impl WriteCanaryOperation {
    const fn label(self) -> &'static str {
        match self {
            Self::Onboarding => "onboarding",
            Self::Faucet => "faucet",
            Self::FinalCanary => WRITE_CANARY_OPERATION,
        }
    }

    const fn mutation_kind(self) -> &'static str {
        match self {
            Self::Onboarding => "onboarding",
            Self::Faucet => "faucet",
            Self::FinalCanary => WRITE_CANARY_MUTATION_KIND,
        }
    }
}

/// Signed Taira write canary using an exact durable prepared envelope.
#[derive(clap::Args, Debug)]
#[command(group(
    clap::ArgGroup::new("prepared_action")
        .required(true)
        .multiple(false)
        .args([
            "prepare_envelope",
            "submit_prepared_envelope_fd",
            "recover_prepared_envelope_fd",
        ])
))]
pub struct WriteCanary {
    /// Public Torii root URL.
    #[arg(long, default_value = DEFAULT_PUBLIC_ROOT)]
    pub public_root: String,
    /// Independently trusted faucet authority; required for the faucet child.
    #[arg(long, required_if_eq("operation", "faucet"))]
    pub faucet_authority: Option<String>,
    /// Independently trusted exact faucet asset definition; required for the faucet child.
    #[arg(long, required_if_eq("operation", "faucet"))]
    pub faucet_asset_id: Option<String>,
    /// Independently trusted exact faucet transfer amount; required for the faucet child.
    #[arg(long, required_if_eq("operation", "faucet"))]
    pub faucet_amount: Option<String>,
    /// Owner-only onboarding token; required only while preparing or submitting the envelope.
    #[arg(long, value_name = "PATH")]
    pub onboarding_token_file: Option<PathBuf>,
    /// Exact ordered child operation; each invocation handles one transaction only.
    #[arg(long, value_enum)]
    pub operation: WriteCanaryOperation,
    /// Exact admitted public-reset authorization digest.
    #[arg(long, value_parser = validate_sha256_argument)]
    pub authorization_sha256: String,
    /// Exact admitted public-reset authorization nonce.
    #[arg(long, value_parser = validate_authorization_nonce_argument)]
    pub authorization_nonce: String,
    /// Exact write-canary phase (`pre_edge` or `post_edge`).
    #[arg(long, value_parser = validate_write_canary_phase_argument)]
    pub mutation_phase: String,
    /// Exact lowercase SHA-256 idempotency key bound into the canary transaction.
    #[arg(long, value_parser = validate_write_canary_idempotency_key)]
    pub idempotency_key: String,
    /// Exact signed execution expiry; preparation and submission are barred at this instant.
    #[arg(long)]
    pub execution_expires_at_unix_ms: u64,
    /// Quote and sign one exact envelope without performing any ledger mutation.
    #[arg(long, requires = "prepared_output_fd")]
    pub prepare_envelope: bool,
    /// Inherited writable numeric descriptor receiving canonical envelope JSON.
    #[arg(long, value_name = "FD", requires = "prepare_envelope")]
    pub prepared_output_fd: Option<u32>,
    /// Submit only exact bytes read from this inherited numeric envelope descriptor.
    #[arg(long, value_name = "FD")]
    pub submit_prepared_envelope_fd: Option<u32>,
    /// Read-only classify exact bytes read from this inherited numeric envelope descriptor.
    #[arg(long, value_name = "FD")]
    pub recover_prepared_envelope_fd: Option<u32>,
    /// Inherited descriptor for the exact Applied predecessor envelope during preparation.
    #[arg(long, value_name = "FD")]
    pub prerequisite_envelope_fd: Option<u32>,
    /// Emit a stable JSON receipt.
    #[arg(long)]
    pub json: bool,
}
impl Run for WriteCanary {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let receipt = run_write_canary_exact(context, &self)?;
        render_report(context, self.json, &receipt)?;
        ensure_write_canary_succeeded(&receipt)
    }
}

impl WriteCanary {
    fn faucet_policy(&self) -> Result<AccountFaucetPolicyV1> {
        if self.operation != WriteCanaryOperation::Faucet {
            eyre::bail!("faucet policy is valid only for the exact faucet child");
        }
        let authority_raw = self
            .faucet_authority
            .as_deref()
            .ok_or_else(|| eyre!("the faucet child requires --faucet-authority"))?;
        let authority = AccountId::parse_encoded(authority_raw)
            .wrap_err("--faucet-authority is not a canonical AccountId")?;
        if authority.to_string() != authority_raw {
            eyre::bail!("--faucet-authority must use its exact canonical representation");
        }
        let asset_raw = self
            .faucet_asset_id
            .as_deref()
            .ok_or_else(|| eyre!("the faucet child requires --faucet-asset-id"))?;
        let asset_definition_id = AssetDefinitionId::from_str(asset_raw)
            .wrap_err("--faucet-asset-id is not a canonical asset definition")?;
        if asset_definition_id.to_string() != asset_raw {
            eyre::bail!("--faucet-asset-id must use its exact canonical representation");
        }
        let amount_raw = self
            .faucet_amount
            .as_deref()
            .ok_or_else(|| eyre!("the faucet child requires --faucet-amount"))?;
        let amount = Quantity::from_str(amount_raw)
            .wrap_err("--faucet-amount is not an exact positive quantity")?;
        if amount.to_string() != amount_raw {
            eyre::bail!("--faucet-amount must use its exact canonical representation");
        }
        AccountFaucetPolicyV1::try_new(authority, asset_definition_id, amount)
            .wrap_err("invalid independently trusted faucet policy")
    }

    fn prepared_action(&self) -> Result<PreparedEnvelopeAction> {
        match (
            self.prepare_envelope,
            self.submit_prepared_envelope_fd,
            self.recover_prepared_envelope_fd,
        ) {
            (true, None, None) => Ok(PreparedEnvelopeAction::Prepare(
                self.prepared_output_fd
                    .ok_or_else(|| eyre!("--prepare-envelope requires --prepared-output-fd"))?,
            )),
            (false, Some(fd), None) => {
                if self.prepared_output_fd.is_some() {
                    eyre::bail!("--prepared-output-fd is valid only with --prepare-envelope");
                }
                Ok(PreparedEnvelopeAction::Submit(fd))
            }
            (false, None, Some(fd)) => {
                if self.prepared_output_fd.is_some() {
                    eyre::bail!("--prepared-output-fd is valid only with --prepare-envelope");
                }
                Ok(PreparedEnvelopeAction::Recover(fd))
            }
            _ => eyre::bail!(
                "select exactly one of --prepare-envelope, --submit-prepared-envelope-fd, or --recover-prepared-envelope-fd"
            ),
        }
    }

    fn binding(&self) -> Result<PreparedMutationBindingV1> {
        validate_sha256_argument(&self.authorization_sha256).map_err(|error| eyre!(error))?;
        validate_authorization_nonce_argument(&self.authorization_nonce)
            .map_err(|error| eyre!(error))?;
        validate_write_canary_phase_argument(&self.mutation_phase).map_err(|error| eyre!(error))?;
        validate_write_canary_idempotency_key(&self.idempotency_key)
            .map_err(|error| eyre!(error))?;
        let expected_idempotency_key = write_canary_child_idempotency_key(
            &self.authorization_nonce,
            &self.mutation_phase,
            self.operation.mutation_kind(),
        );
        if self.idempotency_key != expected_idempotency_key {
            eyre::bail!(
                "idempotency key does not match the exact authorization nonce, phase, and child kind"
            );
        }
        if self.execution_expires_at_unix_ms == 0 {
            eyre::bail!("execution expiry must be a positive Unix millisecond instant");
        }
        Ok(PreparedMutationBindingV1 {
            schema: PREPARED_BINDING_SCHEMA_V1.to_owned(),
            authorization_sha256: self.authorization_sha256.clone(),
            authorization_nonce: self.authorization_nonce.clone(),
            kind: self.operation.mutation_kind().to_owned(),
            phase: self.mutation_phase.clone(),
            idempotency_key: self.idempotency_key.clone(),
            execution_expires_at_unix_ms: self.execution_expires_at_unix_ms,
        })
    }

    fn validate_prerequisite_action(&self, action: PreparedEnvelopeAction) -> Result<()> {
        match (action, self.operation, self.prerequisite_envelope_fd) {
            (PreparedEnvelopeAction::Prepare(_), WriteCanaryOperation::Onboarding, None)
            | (
                PreparedEnvelopeAction::Prepare(_),
                WriteCanaryOperation::Faucet | WriteCanaryOperation::FinalCanary,
                Some(_),
            )
            | (PreparedEnvelopeAction::Submit(_), _, None)
            | (PreparedEnvelopeAction::Recover(_), _, None) => Ok(()),
            (PreparedEnvelopeAction::Prepare(_), WriteCanaryOperation::Onboarding, Some(_)) => {
                eyre::bail!("onboarding preparation has no predecessor envelope")
            }
            (
                PreparedEnvelopeAction::Prepare(_),
                WriteCanaryOperation::Faucet | WriteCanaryOperation::FinalCanary,
                None,
            ) => eyre::bail!(
                "faucet and final-canary preparation require --prerequisite-envelope-fd"
            ),
            (
                PreparedEnvelopeAction::Submit(_) | PreparedEnvelopeAction::Recover(_),
                _,
                Some(_),
            ) => {
                eyre::bail!(
                    "--prerequisite-envelope-fd is valid only while preparing the next operation"
                )
            }
        }
    }

    fn require_onboarding_token(&self) -> Result<&Path> {
        self.onboarding_token_file.as_deref().ok_or_else(|| {
            eyre!(
                "the onboarding operation requires --onboarding-token-file in prepare/submit mode"
            )
        })
    }
}

fn validate_sha256_argument(value: &str) -> Result<String, String> {
    validate_lower_hex_argument(value, "SHA-256", 64)
}

fn validate_authorization_nonce_argument(value: &str) -> Result<String, String> {
    if value.len() != 32
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
    {
        return Err(
            "must be exactly 32 lowercase URL-safe ASCII characters (`a-z`, `0-9`, `-`, `_`)"
                .to_owned(),
        );
    }
    Ok(value.to_owned())
}

fn validate_write_canary_phase_argument(value: &str) -> Result<String, String> {
    let first = value.as_bytes().first().copied();
    let last = value.as_bytes().last().copied();
    if value.is_empty()
        || value.len() > 64
        || first.is_some_and(|byte| matches!(byte, b'-' | b'_'))
        || last.is_some_and(|byte| matches!(byte, b'-' | b'_'))
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
    {
        return Err("must be a 1..=64 byte canonical lowercase phase slug".to_owned());
    }
    Ok(value.to_owned())
}

fn validate_lower_hex_argument(value: &str, label: &str, length: usize) -> Result<String, String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{label} must be exactly {length} lowercase hexadecimal characters"
        ));
    }
    Ok(value.to_owned())
}

fn write_canary_child_idempotency_key(
    authorization_nonce: &str,
    phase: &str,
    child_kind: &str,
) -> String {
    let mut digest = Sha256::new();
    for frame in [
        b"iroha:taira:public-reset:child-idempotency:v1\0".as_slice(),
        authorization_nonce.as_bytes(),
        phase.as_bytes(),
        child_kind.as_bytes(),
    ] {
        let length = u64::try_from(frame.len()).expect("idempotency frame length fits u64");
        digest.update(length.to_be_bytes());
        digest.update(frame);
    }
    hex::encode(digest.finalize())
}
/// Canonical deploy-mode Taira Inrou canary workspace generator.
#[derive(clap::Args, Debug)]
pub struct InrouWorkspace {
    /// Direct regular AArch64 kernel image prepared for PortableVM.
    #[arg(long, value_name = "PATH")]
    pub kernel: PathBuf,
    /// Direct regular AArch64 ext4 root filesystem image prepared for PortableVM.
    #[arg(long, value_name = "PATH")]
    pub rootfs: PathBuf,
    /// Direct regular AArch64 initrd image prepared for PortableVM.
    #[arg(long, value_name = "PATH")]
    pub initrd: PathBuf,
    /// Fresh owner-only directory to create with the exact canonical workspace layout.
    #[arg(long, value_name = "PATH")]
    pub output_dir: PathBuf,
    /// Emit a stable JSON report.
    #[arg(long)]
    pub json: bool,
}
impl Run for InrouWorkspace {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let receipt = crate::soracloud::create_taira_inrou_canary_workspace(
            &self.kernel,
            &self.rootfs,
            &self.initrd,
            &self.output_dir,
        )?;
        let mut extra = Map::new();
        extra.insert(
            "output_dir".into(),
            Value::String(self.output_dir.display().to_string()),
        );
        extra.insert("workspace".into(), json::to_value(&receipt)?);
        let report = report_value(
            "taira_inrou_workspace",
            "ok",
            "offline",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            extra,
        )?;
        render_report(context, self.json, &report)
    }
}
/// Explicit mutation mode for the Taira Inrou canary.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum InrouCanaryMode {
    /// Register a new canary service.
    Deploy,
    /// Replace an already-deployed canary revision.
    Upgrade,
}
/// Canonical offline Taira Inrou artifact staging.
#[derive(clap::Args, Debug)]
pub struct InrouStage {
    /// Build the exact deploy or upgrade revision; no mutation mode is inferred.
    #[arg(long, value_enum)]
    pub mode: InrouCanaryMode,
    /// Path to the canonical strict-null, unpublished four-replica Inrou source manifest.
    #[arg(long, value_name = "PATH")]
    pub container: PathBuf,
    /// Path to the matching public HttpService manifest.
    #[arg(long, value_name = "PATH")]
    pub service: PathBuf,
    /// Canonical service bundle bytes to preseed.
    #[arg(long, value_name = "PATH")]
    pub bundle_file: PathBuf,
    /// Fresh owner-only directory that will contain exact manifests and payloads.
    #[arg(long, value_name = "PATH")]
    pub stage_dir: PathBuf,
    /// Emit a stable JSON receipt.
    #[arg(long)]
    pub json: bool,
}
impl Run for InrouStage {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        ensure_canonical_taira_client_identity(context.config())?;
        let _chain_discriminant = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
        let receipt = crate::soracloud::stage_taira_inrou_canary_deployment(
            self.mode,
            &self.container,
            &self.service,
            &self.bundle_file,
            &self.stage_dir,
            &context.config().key_pair,
        )?;
        let mut extra = Map::new();
        extra.insert(
            "stage_dir".into(),
            Value::String(self.stage_dir.display().to_string()),
        );
        extra.insert("receipt".into(), json::to_value(&receipt)?);
        let report = report_value(
            "taira_inrou_stage",
            "ok",
            "offline",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            extra,
        )?;
        render_report(context, self.json, &report)
    }
}
const PREPARED_INROU_OPERATION_SCHEMA_V1: &str = "iroha.taira.prepared-soracloud-transaction.v1";

/// One strictly ordered transaction in the Taira Inrou deployment canary.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum InrouCanaryOperation {
    /// Register the canonical service-bundle manifest.
    BundlePin,
    /// Register the canonical AArch64 guest-image manifest.
    GuestPin,
    /// Deploy or upgrade the service after both manifests are Applied.
    ServiceMutation,
}

impl InrouCanaryOperation {
    const fn label(self) -> &'static str {
        match self {
            Self::BundlePin => "bundle_pin",
            Self::GuestPin => "guest_pin",
            Self::ServiceMutation => "service_mutation",
        }
    }

    const fn mutation_kind(self) -> &'static str {
        match self {
            Self::BundlePin => "inrou_bundle_pin",
            Self::GuestPin => "inrou_guest_pin",
            Self::ServiceMutation => "inrou_canary",
        }
    }

    const fn tagged_kind(self) -> &'static str {
        match self {
            Self::BundlePin => "inrou_bundle_pin",
            Self::GuestPin => "inrou_guest_pin",
            Self::ServiceMutation => "inrou_canary",
        }
    }

    const fn prepared_operation(self) -> crate::soracloud::TairaInrouCanaryPreparedOperationV1 {
        match self {
            Self::BundlePin => crate::soracloud::TairaInrouCanaryPreparedOperationV1::BundlePin,
            Self::GuestPin => crate::soracloud::TairaInrouCanaryPreparedOperationV1::GuestPin,
            Self::ServiceMutation => {
                crate::soracloud::TairaInrouCanaryPreparedOperationV1::ServiceMutation
            }
        }
    }

    const fn predecessor(self) -> (&'static str, &'static str) {
        match self {
            Self::BundlePin => ("write_canary", "final_canary"),
            Self::GuestPin => ("inrou_bundle_pin", "bundle_pin"),
            Self::ServiceMutation => ("inrou_guest_pin", "guest_pin"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PreparedInrouTransactionV1 {
    schema: String,
    binding: crate::soracloud::TairaMutationBindingV1,
    operation: String,
    transaction_hash_hex: String,
    signed_transaction_wire_hex: String,
    signed_transaction_wire_sha256: String,
    fee_payment: FeePaymentIntent,
    fee_quote: FeeQuoteResponse,
}

#[derive(Clone, Debug, PartialEq, JsonSerialize, JsonDeserialize)]
#[norito(
    tag = "kind",
    content = "envelope",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum PreparedInrouOperationV1 {
    InrouBundlePin(PreparedInrouTransactionV1),
    InrouGuestPin(PreparedInrouTransactionV1),
    InrouCanary(PreparedInrouTransactionV1),
}

impl PreparedInrouOperationV1 {
    fn transaction(&self) -> &PreparedInrouTransactionV1 {
        match self {
            Self::InrouBundlePin(value) | Self::InrouGuestPin(value) | Self::InrouCanary(value) => {
                value
            }
        }
    }

    const fn tagged_kind(&self) -> &'static str {
        match self {
            Self::InrouBundlePin(_) => "inrou_bundle_pin",
            Self::InrouGuestPin(_) => "inrou_guest_pin",
            Self::InrouCanary(_) => "inrou_canary",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PreparedInrouStageIdentityV1 {
    service_name: String,
    service_version: String,
    route_host: String,
    route_path_prefix: String,
    healthcheck_path: String,
    stage_mode: String,
    bundle_hash: String,
    bundle_content_cid: String,
    bundle_manifest_digest_hex: String,
    guest_content_cid: String,
    guest_manifest_digest_hex: String,
    container_manifest_hash: String,
    service_manifest_hash: String,
}

impl From<&crate::soracloud::TairaInrouStageIdentity> for PreparedInrouStageIdentityV1 {
    fn from(value: &crate::soracloud::TairaInrouStageIdentity) -> Self {
        Self {
            service_name: value.service_name.clone(),
            service_version: value.service_version.clone(),
            route_host: value.route_host.clone(),
            route_path_prefix: value.route_path_prefix.clone(),
            healthcheck_path: value.healthcheck_path.clone(),
            stage_mode: value.stage_mode.clone(),
            bundle_hash: value.bundle_hash.clone(),
            bundle_content_cid: value.bundle_content_cid.clone(),
            bundle_manifest_digest_hex: value.bundle_manifest_digest_hex.clone(),
            guest_content_cid: value.guest_content_cid.clone(),
            guest_manifest_digest_hex: value.guest_manifest_digest_hex.clone(),
            container_manifest_hash: value.container_manifest_hash.clone(),
            service_manifest_hash: value.service_manifest_hash.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PreparedInrouEnvelopeV1 {
    schema: String,
    binding: crate::soracloud::TairaMutationBindingV1,
    public_root: String,
    chain_id: String,
    network_id: String,
    authority: String,
    stage: PreparedInrouStageIdentityV1,
    operation: PreparedInrouOperationV1,
}

struct ValidatedPreparedInrouV1 {
    envelope: PreparedInrouEnvelopeV1,
    prepared: crate::soracloud::PreparedSoracloudTransactionV1,
    envelope_bytes: Vec<u8>,
    stage: crate::soracloud::TairaInrouStageIdentity,
}

/// Canonical Taira Inrou exact-transaction preparation and recovery.
#[derive(clap::Args, Debug)]
#[command(group(
    clap::ArgGroup::new("prepared_action")
        .required(true)
        .multiple(false)
        .args([
            "prepare_envelope",
            "submit_prepared_envelope_fd",
            "recover_prepared_envelope_fd",
        ])
))]
pub struct InrouCanary {
    /// Public Torii root URL used for mutation, status, and route probes.
    #[arg(long, default_value = DEFAULT_PUBLIC_ROOT)]
    pub public_root: String,
    /// Owner-only stage created by `iroha taira inrou-stage` and preseeded into all validators.
    #[arg(long, value_name = "PATH")]
    pub stage_dir: PathBuf,
    /// Submit an explicit deploy or upgrade mutation; conflicts are never retried as another mode.
    #[arg(long, value_enum)]
    pub mode: InrouCanaryMode,
    /// Exact ordered child operation; each invocation handles one transaction only.
    #[arg(long, value_enum)]
    pub operation: InrouCanaryOperation,
    /// Exact admitted public-reset authorization digest.
    #[arg(long, value_parser = validate_sha256_argument)]
    pub authorization_sha256: String,
    /// Exact admitted public-reset authorization nonce.
    #[arg(long, value_parser = validate_authorization_nonce_argument)]
    pub authorization_nonce: String,
    /// Exact mutation phase; Inrou V1 is admitted only in `pre_edge`.
    #[arg(long, value_parser = validate_write_canary_phase_argument)]
    pub mutation_phase: String,
    /// Exact child-kind-derived idempotency key bound into the signed transaction.
    #[arg(long, value_parser = validate_write_canary_idempotency_key)]
    pub idempotency_key: String,
    /// Exact signed execution expiry; preparation and submission are barred at this instant.
    #[arg(long)]
    pub execution_expires_at_unix_ms: u64,
    /// Quote and sign one exact transaction envelope without submitting it.
    #[arg(long, requires = "prepared_output_fd")]
    pub prepare_envelope: bool,
    /// Inherited writable descriptor receiving canonical envelope JSON.
    #[arg(long, value_name = "FD", requires = "prepare_envelope")]
    pub prepared_output_fd: Option<u32>,
    /// Submit only exact bytes read from this inherited descriptor.
    #[arg(long, value_name = "FD")]
    pub submit_prepared_envelope_fd: Option<u32>,
    /// Read-only classify exact bytes read from this inherited descriptor.
    #[arg(long, value_name = "FD")]
    pub recover_prepared_envelope_fd: Option<u32>,
    /// Inherited exact Applied predecessor envelope, required only while preparing.
    #[arg(long, value_name = "FD")]
    pub prerequisite_envelope_fd: Option<u32>,
    /// Maximum convergence time for adverts, placements, runtime health, and all four routes.
    #[arg(long, value_name = "SECS", default_value_t = 180)]
    pub timeout_secs: u64,
    /// Emit a stable redacted JSON receipt.
    #[arg(long)]
    pub json: bool,
}
impl Run for InrouCanary {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let receipt = run_inrou_canary_exact(context, &self)?;
        render_report(context, self.json, &receipt)?;
        if report_status(&receipt) != Some("ok") {
            eyre::bail!("Taira Inrou prepared operation found hard failures");
        }
        Ok(())
    }
}

impl InrouCanary {
    fn prepared_action(&self) -> Result<PreparedEnvelopeAction> {
        match (
            self.prepare_envelope,
            self.submit_prepared_envelope_fd,
            self.recover_prepared_envelope_fd,
        ) {
            (true, None, None) => Ok(PreparedEnvelopeAction::Prepare(
                self.prepared_output_fd
                    .ok_or_else(|| eyre!("--prepare-envelope requires --prepared-output-fd"))?,
            )),
            (false, Some(fd), None) if self.prepared_output_fd.is_none() => {
                Ok(PreparedEnvelopeAction::Submit(fd))
            }
            (false, None, Some(fd)) if self.prepared_output_fd.is_none() => {
                Ok(PreparedEnvelopeAction::Recover(fd))
            }
            _ => eyre::bail!(
                "select exactly one prepared action and use --prepared-output-fd only with --prepare-envelope"
            ),
        }
    }

    fn binding(&self) -> Result<crate::soracloud::TairaMutationBindingV1> {
        validate_sha256_argument(&self.authorization_sha256).map_err(|error| eyre!(error))?;
        validate_authorization_nonce_argument(&self.authorization_nonce)
            .map_err(|error| eyre!(error))?;
        validate_write_canary_phase_argument(&self.mutation_phase).map_err(|error| eyre!(error))?;
        if self.mutation_phase != "pre_edge" {
            eyre::bail!("Taira Inrou prepared operations are admitted only in pre_edge");
        }
        validate_write_canary_idempotency_key(&self.idempotency_key)
            .map_err(|error| eyre!(error))?;
        let expected = write_canary_child_idempotency_key(
            &self.authorization_nonce,
            &self.mutation_phase,
            self.operation.mutation_kind(),
        );
        if self.idempotency_key != expected {
            eyre::bail!(
                "idempotency key does not match the exact authorization nonce, phase, and Inrou child kind"
            );
        }
        if self.execution_expires_at_unix_ms == 0 {
            eyre::bail!("execution expiry must be a positive Unix millisecond instant");
        }
        Ok(crate::soracloud::TairaMutationBindingV1 {
            authorization_sha256: self.authorization_sha256.clone(),
            authorization_nonce: self.authorization_nonce.clone(),
            kind: self.operation.mutation_kind().to_owned(),
            phase: self.mutation_phase.clone(),
            idempotency_key: self.idempotency_key.clone(),
            execution_expires_at_unix_ms: self.execution_expires_at_unix_ms,
        })
    }

    fn validate_prerequisite_action(&self, action: PreparedEnvelopeAction) -> Result<()> {
        match (action, self.prerequisite_envelope_fd) {
            (PreparedEnvelopeAction::Prepare(_), Some(_))
            | (PreparedEnvelopeAction::Submit(_) | PreparedEnvelopeAction::Recover(_), None) => {
                Ok(())
            }
            (PreparedEnvelopeAction::Prepare(_), None) => {
                eyre::bail!("every Inrou child preparation requires --prerequisite-envelope-fd")
            }
            (PreparedEnvelopeAction::Submit(_) | PreparedEnvelopeAction::Recover(_), Some(_)) => {
                eyre::bail!(
                    "--prerequisite-envelope-fd is valid only while preparing the next Inrou child"
                )
            }
        }
    }
}

fn run_inrou_canary_exact<C: RunContext>(context: &mut C, args: &InrouCanary) -> Result<Value> {
    validate_inrou_canary_timeout(args.timeout_secs)?;
    ensure_canonical_taira_client_identity(context.config())?;
    let _chain_discriminant = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
    let public_root = normalize_root_url(&args.public_root)?;
    let binding = args.binding()?;
    let action = args.prepared_action()?;
    args.validate_prerequisite_action(action)?;
    let expected_fee_payment = context.transaction_fee_payment()?;
    match action {
        PreparedEnvelopeAction::Prepare(output_fd) => {
            require_inrou_binding_current(&binding)?;
            preflight_taira_network_identity(&public_root, context.config())?;
            prove_inrou_predecessor_applied(
                context.config(),
                args,
                &public_root,
                &binding,
                &expected_fee_payment,
            )?;
            let stage = crate::soracloud::load_taira_inrou_stage_identity(
                context.config(),
                &args.stage_dir,
                args.mode,
            )?;
            let prepared = crate::soracloud::prepare_taira_inrou_canary_operation(
                context.config(),
                expected_fee_payment.clone(),
                binding.clone(),
                &args.stage_dir,
                &public_root,
                None,
                args.timeout_secs,
                args.mode,
                args.operation.prepared_operation(),
            )?;
            let envelope = make_prepared_inrou_envelope(
                context.config(),
                &public_root,
                args.operation,
                &stage,
                &expected_fee_payment,
                prepared,
            )?;
            let envelope_bytes = canonical_prepared_inrou_envelope_bytes(&envelope)?;
            write_prepared_envelope(output_fd, &envelope_bytes)?;
            prepared_inrou_report(
                context.config(),
                &public_root,
                args,
                &envelope,
                &envelope_bytes,
                &stage,
                "Prepared",
                None,
                None,
                None,
            )
        }
        PreparedEnvelopeAction::Submit(fd) => {
            let validated = load_and_validate_prepared_inrou(
                context.config(),
                args,
                &public_root,
                &binding,
                fd,
                &expected_fee_payment,
                PreparedLifetimeCheck::LiveForward,
            )?;
            let outcome = submit_prepared_inrou_until(
                context.config(),
                &public_root,
                args.timeout_secs,
                &validated.prepared,
                &binding,
            );
            report_prepared_inrou_outcome(context.config(), &public_root, args, &validated, outcome)
        }
        PreparedEnvelopeAction::Recover(fd) => {
            let validated = load_and_validate_prepared_inrou(
                context.config(),
                args,
                &public_root,
                &binding,
                fd,
                &expected_fee_payment,
                PreparedLifetimeCheck::Structural,
            )?;
            let outcome = recover_prepared_inrou(
                context.config(),
                &public_root,
                args.timeout_secs,
                &validated.prepared,
            );
            report_prepared_inrou_outcome(context.config(), &public_root, args, &validated, outcome)
        }
    }
}

fn require_inrou_binding_current(binding: &crate::soracloud::TairaMutationBindingV1) -> Result<()> {
    if current_unix_ms()? >= binding.execution_expires_at_unix_ms {
        eyre::bail!("prepared Inrou mutation execution expiry bars a new forward effect");
    }
    Ok(())
}

fn make_prepared_inrou_envelope(
    config: &Config,
    public_root: &str,
    operation: InrouCanaryOperation,
    stage: &crate::soracloud::TairaInrouStageIdentity,
    expected_fee_payment: &FeePaymentIntent,
    prepared: crate::soracloud::PreparedSoracloudTransactionV1,
) -> Result<PreparedInrouEnvelopeV1> {
    let transaction = prepared.decode_and_validate()?;
    if prepared.operation != operation.label()
        || prepared.binding.kind != operation.mutation_kind()
        || transaction.authority() != &config.account
        || transaction.network_id() != Some(&config.network_id)
    {
        eyre::bail!("prepared Inrou transaction differs from its exact CLI identity");
    }
    validate_expected_prepared_fee_payment(expected_fee_payment, transaction.fee_payment_intent())?;
    crate::soracloud::verify_taira_inrou_prepared_transaction_identity_v1(
        &transaction,
        operation.prepared_operation(),
        stage,
        &prepared.binding.idempotency_key,
    )
    .wrap_err("prepared Inrou transaction executable authentication failed")?;
    validate_inrou_transaction_lifetime(
        &transaction,
        &prepared.binding,
        PreparedLifetimeCheck::LiveForward,
    )?;
    let transaction_envelope = PreparedInrouTransactionV1 {
        schema: PREPARED_INROU_OPERATION_SCHEMA_V1.to_owned(),
        binding: prepared.binding.clone(),
        operation: prepared.operation.clone(),
        transaction_hash_hex: prepared.tx_hash_hex,
        signed_transaction_wire_sha256: hex::encode(Sha256::digest(&prepared.wire)),
        signed_transaction_wire_hex: hex::encode(&prepared.wire),
        fee_payment: prepared.fee_payment,
        fee_quote: prepared.fee_quote,
    };
    let tagged = match operation {
        InrouCanaryOperation::BundlePin => {
            PreparedInrouOperationV1::InrouBundlePin(transaction_envelope)
        }
        InrouCanaryOperation::GuestPin => {
            PreparedInrouOperationV1::InrouGuestPin(transaction_envelope)
        }
        InrouCanaryOperation::ServiceMutation => {
            PreparedInrouOperationV1::InrouCanary(transaction_envelope)
        }
    };
    Ok(PreparedInrouEnvelopeV1 {
        schema: PREPARED_ENVELOPE_SCHEMA_V1.to_owned(),
        binding: prepared.binding,
        public_root: public_root.to_owned(),
        chain_id: config.chain.to_string(),
        network_id: config.network_id.to_string(),
        authority: config.account.to_string(),
        stage: stage.into(),
        operation: tagged,
    })
}

fn canonical_prepared_inrou_envelope_bytes(envelope: &PreparedInrouEnvelopeV1) -> Result<Vec<u8>> {
    let mut bytes = json::to_json(envelope)
        .wrap_err("encode canonical prepared Inrou envelope")?
        .into_bytes();
    bytes.push(b'\n');
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PREPARED_ENVELOPE_MAX_BYTES {
        eyre::bail!("prepared Inrou envelope exceeds its V1 byte bound");
    }
    Ok(bytes)
}

fn read_prepared_inrou_envelope(fd: u32) -> Result<(PreparedInrouEnvelopeV1, Vec<u8>)> {
    let path = inherited_fd_path(fd)?;
    let file = File::open(path)
        .wrap_err_with(|| format!("failed to duplicate prepared Inrou input FD {fd}"))?;
    let mut bytes = Vec::new();
    file.take(PREPARED_ENVELOPE_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .wrap_err("failed to read prepared Inrou envelope")?;
    if bytes.is_empty()
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PREPARED_ENVELOPE_MAX_BYTES
    {
        eyre::bail!("prepared Inrou envelope is empty or exceeds its V1 byte bound");
    }
    let envelope: PreparedInrouEnvelopeV1 =
        json::from_slice(&bytes).wrap_err("prepared Inrou envelope is not canonical V1 JSON")?;
    if canonical_prepared_inrou_envelope_bytes(&envelope)? != bytes {
        eyre::bail!("prepared Inrou envelope bytes are not exact canonical V1 JSON");
    }
    Ok((envelope, bytes))
}

fn load_and_validate_prepared_inrou(
    config: &Config,
    args: &InrouCanary,
    public_root: &str,
    expected_binding: &crate::soracloud::TairaMutationBindingV1,
    fd: u32,
    expected_fee_payment: &FeePaymentIntent,
    lifetime_check: PreparedLifetimeCheck,
) -> Result<ValidatedPreparedInrouV1> {
    let (envelope, envelope_bytes) = read_prepared_inrou_envelope(fd)?;
    let stage =
        crate::soracloud::load_taira_inrou_stage_identity(config, &args.stage_dir, args.mode)?;
    let operation = envelope.operation.transaction();
    if envelope.schema != PREPARED_ENVELOPE_SCHEMA_V1
        || &envelope.binding != expected_binding
        || envelope.public_root != public_root
        || envelope.chain_id != config.chain.to_string()
        || envelope.network_id != config.network_id.to_string()
        || envelope.authority != config.account.to_string()
        || envelope.stage != PreparedInrouStageIdentityV1::from(&stage)
        || envelope.operation.tagged_kind() != args.operation.tagged_kind()
        || operation.schema != PREPARED_INROU_OPERATION_SCHEMA_V1
        || operation.binding != envelope.binding
        || operation.operation != args.operation.label()
    {
        eyre::bail!("prepared Inrou envelope does not bind the exact CLI authorization and stage");
    }
    let wire = hex::decode(&operation.signed_transaction_wire_hex)
        .wrap_err("prepared Inrou transaction wire is not hexadecimal")?;
    if wire.is_empty()
        || wire.len() > PREPARED_TRANSACTION_MAX_BYTES
        || hex::encode(&wire) != operation.signed_transaction_wire_hex
        || hex::encode(Sha256::digest(&wire)) != operation.signed_transaction_wire_sha256
    {
        eyre::bail!("prepared Inrou transaction wire is not canonical or bounded");
    }
    let prepared = crate::soracloud::PreparedSoracloudTransactionV1 {
        operation: operation.operation.clone(),
        wire,
        tx_hash_hex: operation.transaction_hash_hex.clone(),
        fee_payment: operation.fee_payment.clone(),
        fee_quote: operation.fee_quote.clone(),
        binding: operation.binding.clone(),
    };
    let transaction = prepared.decode_and_validate()?;
    if transaction.authority() != &config.account
        || transaction.network_id() != Some(&config.network_id)
    {
        eyre::bail!("prepared Inrou transaction has a substituted authority or network");
    }
    validate_expected_prepared_fee_payment(expected_fee_payment, transaction.fee_payment_intent())?;
    crate::soracloud::verify_taira_inrou_prepared_transaction_identity_v1(
        &transaction,
        args.operation.prepared_operation(),
        &stage,
        &prepared.binding.idempotency_key,
    )
    .wrap_err("prepared Inrou transaction executable authentication failed")?;
    validate_inrou_transaction_lifetime(&transaction, &prepared.binding, lifetime_check)?;
    Ok(ValidatedPreparedInrouV1 {
        envelope,
        prepared,
        envelope_bytes,
        stage,
    })
}

fn validate_inrou_transaction_lifetime(
    transaction: &SignedTransaction,
    binding: &crate::soracloud::TairaMutationBindingV1,
    lifetime_check: PreparedLifetimeCheck,
) -> Result<()> {
    let creation_ms = u64::try_from(transaction.creation_time().as_millis())
        .wrap_err("prepared Inrou transaction creation time exceeds u64")?;
    let ttl_ms = u64::try_from(
        transaction
            .time_to_live()
            .ok_or_else(|| eyre!("prepared Inrou transaction omits its required TTL"))?
            .as_millis(),
    )
    .wrap_err("prepared Inrou transaction TTL exceeds u64")?;
    validate_prepared_transaction_time_window(
        creation_ms,
        ttl_ms,
        binding.execution_expires_at_unix_ms,
        "prepared Inrou transaction",
    )?;
    if lifetime_check == PreparedLifetimeCheck::LiveForward {
        validate_live_prepared_transaction_freshness(
            creation_ms,
            ttl_ms,
            current_unix_ms()?,
            "prepared Inrou transaction",
        )?;
    }
    Ok(())
}

fn submit_prepared_inrou_until(
    config: &Config,
    public_root: &str,
    timeout_secs: u64,
    prepared: &crate::soracloud::PreparedSoracloudTransactionV1,
    binding: &crate::soracloud::TairaMutationBindingV1,
) -> crate::soracloud::PreparedSoracloudRecoveryV1 {
    let first = recover_prepared_inrou(config, public_root, timeout_secs, prepared);
    if !matches!(first, crate::soracloud::PreparedSoracloudRecoveryV1::Absent) {
        return first;
    }
    if require_inrou_binding_current(binding).is_err() {
        return crate::soracloud::PreparedSoracloudRecoveryV1::Rejected {
            terminal_kind: "ExecutionExpiredBeforeSubmit".to_owned(),
        };
    }
    let expected_hash = prepared.tx_hash_hex.clone();
    match crate::soracloud::submit_prepared_soracloud_transaction(
        config,
        public_root,
        timeout_secs,
        prepared,
    ) {
        Ok(hash) if hex::encode(hash.as_ref()) == expected_hash => {}
        Ok(_) => {
            return crate::soracloud::PreparedSoracloudRecoveryV1::Rejected {
                terminal_kind: "SubmittedHashMismatch".to_owned(),
            };
        }
        Err(_) => return recover_prepared_inrou(config, public_root, timeout_secs, prepared),
    }
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(timeout_secs))
        .unwrap_or_else(Instant::now);
    loop {
        let outcome = recover_prepared_inrou(config, public_root, timeout_secs, prepared);
        if !matches!(
            outcome,
            crate::soracloud::PreparedSoracloudRecoveryV1::Absent
                | crate::soracloud::PreparedSoracloudRecoveryV1::Pending { .. }
        ) || Instant::now() >= deadline
        {
            return outcome;
        }
        std::thread::sleep(
            Duration::from_millis(200).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

fn recover_prepared_inrou(
    config: &Config,
    public_root: &str,
    timeout_secs: u64,
    prepared: &crate::soracloud::PreparedSoracloudTransactionV1,
) -> crate::soracloud::PreparedSoracloudRecoveryV1 {
    crate::soracloud::recover_prepared_soracloud_transaction(
        config,
        public_root,
        timeout_secs,
        prepared,
    )
    .unwrap_or_else(|_| crate::soracloud::PreparedSoracloudRecoveryV1::Pending {
        terminal_kind: "ObservationUnavailable".to_owned(),
    })
}

fn report_prepared_inrou_outcome(
    config: &Config,
    public_root: &str,
    args: &InrouCanary,
    validated: &ValidatedPreparedInrouV1,
    outcome: crate::soracloud::PreparedSoracloudRecoveryV1,
) -> Result<Value> {
    match outcome {
        crate::soracloud::PreparedSoracloudRecoveryV1::Absent => prepared_inrou_report(
            config,
            public_root,
            args,
            &validated.envelope,
            &validated.envelope_bytes,
            &validated.stage,
            "Pending",
            None,
            Some("Absent".to_owned()),
            None,
        ),
        crate::soracloud::PreparedSoracloudRecoveryV1::Applied {
            block_height,
            evidence_sha256,
        } => prepared_inrou_report(
            config,
            public_root,
            args,
            &validated.envelope,
            &validated.envelope_bytes,
            &validated.stage,
            "Applied",
            Some(block_height),
            Some(evidence_sha256),
            Some(validated),
        ),
        crate::soracloud::PreparedSoracloudRecoveryV1::Pending { terminal_kind } => {
            prepared_inrou_report(
                config,
                public_root,
                args,
                &validated.envelope,
                &validated.envelope_bytes,
                &validated.stage,
                "Pending",
                None,
                Some(terminal_kind),
                None,
            )
        }
        crate::soracloud::PreparedSoracloudRecoveryV1::Rejected { terminal_kind } => {
            prepared_inrou_report(
                config,
                public_root,
                args,
                &validated.envelope,
                &validated.envelope_bytes,
                &validated.stage,
                "Rejected",
                None,
                Some(terminal_kind),
                None,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn prepared_inrou_report(
    config: &Config,
    public_root: &str,
    args: &InrouCanary,
    envelope: &PreparedInrouEnvelopeV1,
    envelope_bytes: &[u8],
    stage: &crate::soracloud::TairaInrouStageIdentity,
    outcome: &str,
    applied_block_height: Option<u64>,
    evidence: Option<String>,
    applied: Option<&ValidatedPreparedInrouV1>,
) -> Result<Value> {
    let mut report = if args.operation == InrouCanaryOperation::ServiceMutation
        && outcome == "Applied"
        && applied.is_some()
    {
        let mut status_config = config.clone();
        status_config.torii_api_url = Url::parse(&format!("{public_root}/"))
            .wrap_err("failed to bind prepared Inrou status client")?;
        status_config.torii_request_timeout = Duration::from_secs(args.timeout_secs.max(1));
        let status_client = IrohaClient::new(status_config);
        verify_inrou_check(public_root, &status_client, stage, args.timeout_secs)?
    } else {
        report_value(
            "taira_inrou_canary",
            "ok",
            public_root,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Map::new(),
        )?
    };
    let object = report
        .as_object_mut()
        .ok_or_else(|| eyre!("prepared Inrou report root is not an object"))?;
    object.insert(
        "command".to_owned(),
        Value::String("taira_inrou_canary".to_owned()),
    );
    object.insert(
        "authorization_sha256".to_owned(),
        Value::String(envelope.binding.authorization_sha256.clone()),
    );
    object.insert(
        "authorization_nonce".to_owned(),
        Value::String(envelope.binding.authorization_nonce.clone()),
    );
    object.insert(
        "mutation_kind".to_owned(),
        Value::String(envelope.binding.kind.clone()),
    );
    object.insert(
        "mutation_phase".to_owned(),
        Value::String(envelope.binding.phase.clone()),
    );
    object.insert(
        "idempotency_key".to_owned(),
        Value::String(envelope.binding.idempotency_key.clone()),
    );
    object.insert(
        "operation".to_owned(),
        Value::String(envelope.operation.transaction().operation.clone()),
    );
    object.insert(
        "transaction_hash_hex".to_owned(),
        Value::String(
            envelope
                .operation
                .transaction()
                .transaction_hash_hex
                .clone(),
        ),
    );
    object.insert(
        "prepared_envelope_sha256".to_owned(),
        Value::String(hex::encode(Sha256::digest(envelope_bytes))),
    );
    object.insert(
        "prepared_envelope_size".to_owned(),
        Value::from(u64::try_from(envelope_bytes.len()).expect("bounded envelope")),
    );
    object.insert(
        "recovery_outcome".to_owned(),
        Value::String(outcome.to_owned()),
    );
    object.insert(
        "applied_block_height".to_owned(),
        applied_block_height.map(Value::from).unwrap_or(Value::Null),
    );
    object.insert(
        "evidence".to_owned(),
        evidence.map(Value::String).unwrap_or(Value::Null),
    );
    object.insert(
        "execution_expires_at_unix_ms".to_owned(),
        Value::from(envelope.binding.execution_expires_at_unix_ms),
    );
    object.insert(
        "fee_payment".to_owned(),
        json::to_value(&envelope.operation.transaction().fee_payment)?,
    );
    object.insert(
        "fee_quote".to_owned(),
        json::to_value(&envelope.operation.transaction().fee_quote)?,
    );
    object.insert(
        "mutation_mode".to_owned(),
        Value::String(stage.stage_mode.clone()),
    );
    Ok(report)
}

fn prove_inrou_predecessor_applied(
    config: &Config,
    args: &InrouCanary,
    public_root: &str,
    binding: &crate::soracloud::TairaMutationBindingV1,
    expected_fee_payment: &FeePaymentIntent,
) -> Result<()> {
    let fd = args
        .prerequisite_envelope_fd
        .ok_or_else(|| eyre!("Inrou preparation is missing its predecessor envelope"))?;
    let path = inherited_fd_path(fd)?;
    let file = File::open(path)
        .wrap_err_with(|| format!("failed to duplicate predecessor envelope FD {fd}"))?;
    let mut bytes = Vec::new();
    file.take(PREPARED_ENVELOPE_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.is_empty()
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PREPARED_ENVELOPE_MAX_BYTES
    {
        eyre::bail!("Inrou predecessor envelope is empty or oversized");
    }
    let (expected_kind, expected_operation) = args.operation.predecessor();
    let value = decode_exact_inrou_predecessor_v1(&bytes, expected_kind)?;
    let root = value
        .as_object()
        .ok_or_else(|| eyre!("Inrou predecessor envelope root is not an object"))?;
    let predecessor_binding = root
        .get("binding")
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("Inrou predecessor envelope omits its binding"))?;
    let binding_string = |name: &str| {
        predecessor_binding
            .get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("Inrou predecessor binding omits `{name}`"))
    };
    let expected_idempotency_key = write_canary_child_idempotency_key(
        &binding.authorization_nonce,
        &binding.phase,
        expected_kind,
    );
    if binding_string("authorization_sha256")? != binding.authorization_sha256
        || binding_string("authorization_nonce")? != binding.authorization_nonce
        || binding_string("kind")? != expected_kind
        || binding_string("phase")? != binding.phase
        || binding_string("idempotency_key")? != expected_idempotency_key
        || predecessor_binding
            .get("execution_expires_at_unix_ms")
            .and_then(Value::as_u64)
            != Some(binding.execution_expires_at_unix_ms)
        || root.get("public_root").and_then(Value::as_str) != Some(public_root)
        || root.get("chain_id").and_then(Value::as_str) != Some(DEFAULT_CHAIN_ID)
        || root.get("network_id").and_then(Value::as_str)
            != Some(config.network_id.to_string().as_str())
        || root.get("authority").and_then(Value::as_str)
            != Some(config.account.to_string().as_str())
    {
        eyre::bail!("Inrou predecessor differs from the exact authorization and network");
    }
    let tagged = root
        .get("operation")
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("Inrou predecessor omits its tagged operation"))?;
    let payload = tagged
        .get("envelope")
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("Inrou predecessor omits its transaction envelope"))?;
    let expected_tag = match expected_kind {
        "write_canary" => "final_canary",
        "inrou_bundle_pin" => "inrou_bundle_pin",
        "inrou_guest_pin" => "inrou_guest_pin",
        _ => return Err(eyre!("unsupported Inrou predecessor kind")),
    };
    if tagged.get("kind").and_then(Value::as_str) != Some(expected_tag)
        || payload.get("operation").and_then(Value::as_str) != Some(expected_operation)
        || payload.get("binding") != root.get("binding")
    {
        eyre::bail!("Inrou predecessor operation tag or binding is invalid");
    }
    let wire_hex = payload
        .get("signed_transaction_wire_hex")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("Inrou predecessor omits its transaction wire"))?;
    let wire = hex::decode(wire_hex).wrap_err("Inrou predecessor wire is not hexadecimal")?;
    let transaction = SignedTransaction::decode_all_versioned(&wire)
        .wrap_err("Inrou predecessor is not a versioned SignedTransaction")?;
    transaction
        .verify_signature()
        .wrap_err("Inrou predecessor signature is invalid")?;
    let transaction_hash = hex::encode(transaction.hash().as_ref());
    let transaction_wire_sha256 = hex::encode(Sha256::digest(&wire));
    if wire.is_empty()
        || wire.len() > PREPARED_TRANSACTION_MAX_BYTES
        || hex::encode(&wire) != wire_hex
        || transaction.encode_wire_v1()? != wire
        || payload.get("transaction_hash_hex").and_then(Value::as_str)
            != Some(transaction_hash.as_str())
        || payload
            .get("signed_transaction_wire_sha256")
            .and_then(Value::as_str)
            != Some(transaction_wire_sha256.as_str())
        || transaction.authority() != &config.account
        || transaction.network_id() != Some(&config.network_id)
    {
        eyre::bail!("Inrou predecessor transaction identity is invalid");
    }
    let payload_fee_payment: FeePaymentIntent = json::from_value(
        payload
            .get("fee_payment")
            .cloned()
            .ok_or_else(|| eyre!("Inrou predecessor omits its fee payment"))?,
    )
    .wrap_err("Inrou predecessor fee payment is not exact typed V1 JSON")?;
    if payload.get("fee_payment") != Some(&json::to_value(&payload_fee_payment)?)
        || transaction.fee_payment_intent() != &payload_fee_payment
    {
        eyre::bail!("Inrou predecessor has a substituted fee-payment closure");
    }
    validate_expected_prepared_fee_payment(expected_fee_payment, &payload_fee_payment)?;
    let expected_binding_json = json::to_json(
        root.get("binding")
            .expect("predecessor binding was validated above"),
    )?;
    let binding_name = Name::from_str(PREPARED_BINDING_METADATA)?;
    if transaction
        .metadata()
        .get(&binding_name)
        .map(IrohaJson::get)
        .map(String::as_str)
        != Some(expected_binding_json.as_str())
    {
        eyre::bail!("Inrou predecessor transaction metadata has a substituted binding");
    }
    match expected_kind {
        "write_canary" => {
            let verified = verify_final_canary_prepared_operation_v1(
                &Value::Object(payload.clone()),
                &config.network_id,
                &config.account,
            )
            .wrap_err("Inrou predecessor final-canary executable authentication failed")?;
            if verified.hash() != transaction.hash() || verified.encode_wire_v1()? != wire {
                eyre::bail!("Inrou predecessor final-canary verifier selected different bytes");
            }
        }
        "inrou_bundle_pin" | "inrou_guest_pin" => {
            let typed: PreparedInrouTransactionV1 =
                json::from_value(Value::Object(payload.clone()))
                    .wrap_err("Inrou predecessor operation is not exact typed V1 JSON")?;
            if json::to_value(&typed)? != Value::Object(payload.clone()) {
                eyre::bail!("Inrou predecessor operation is outside its typed V1 JSON closure");
            }
            let stage = crate::soracloud::load_taira_inrou_stage_identity(
                config,
                &args.stage_dir,
                args.mode,
            )?;
            if root.get("stage")
                != Some(&json::to_value(PreparedInrouStageIdentityV1::from(&stage))?)
            {
                eyre::bail!("Inrou predecessor retained-stage identity was substituted");
            }
            let prepared = crate::soracloud::PreparedSoracloudTransactionV1 {
                operation: typed.operation.clone(),
                wire: wire.clone(),
                tx_hash_hex: typed.transaction_hash_hex,
                fee_payment: typed.fee_payment,
                fee_quote: typed.fee_quote,
                binding: typed.binding,
            };
            let verified = prepared
                .decode_and_validate()
                .wrap_err("Inrou predecessor transaction closure is invalid")?;
            let prepared_operation = match expected_kind {
                "inrou_bundle_pin" => {
                    crate::soracloud::TairaInrouCanaryPreparedOperationV1::BundlePin
                }
                "inrou_guest_pin" => {
                    crate::soracloud::TairaInrouCanaryPreparedOperationV1::GuestPin
                }
                _ => unreachable!("matched exact Inrou predecessor kinds"),
            };
            crate::soracloud::verify_taira_inrou_prepared_transaction_identity_v1(
                &verified,
                prepared_operation,
                &stage,
                &expected_idempotency_key,
            )
            .wrap_err("Inrou predecessor executable authentication failed")?;
            validate_inrou_transaction_lifetime(
                &verified,
                &prepared.binding,
                PreparedLifetimeCheck::Structural,
            )?;
            if verified.hash() != transaction.hash() || verified.encode_wire_v1()? != wire {
                eyre::bail!("Inrou predecessor typed verifier selected different bytes");
            }
        }
        _ => return Err(eyre!("unsupported Inrou predecessor kind")),
    }
    let mut status_config = config.clone();
    status_config.torii_api_url = Url::parse(&format!("{public_root}/"))?;
    status_config.torii_request_timeout = Duration::from_secs(args.timeout_secs.max(1));
    let client = IrohaClient::new(status_config);
    let status = client
        .get_transaction_status_response_global(transaction.hash())?
        .ok_or_else(|| eyre!("Inrou predecessor transaction is absent"))?;
    if status.hash != transaction_hash
        || status.scope != "global"
        || status.status.kind != "Applied"
        || !status.status.block_height.is_some_and(|height| height > 0)
    {
        eyre::bail!("Inrou predecessor has not reached exact global Applied state");
    }
    verify_exact_committed_transaction(&client, &transaction, &wire)?;
    Ok(())
}

fn decode_exact_inrou_predecessor_v1(bytes: &[u8], expected_kind: &str) -> Result<Value> {
    let (value, mut canonical) = match expected_kind {
        "write_canary" => {
            let envelope: PreparedMutationEnvelopeV1 = json::from_slice(bytes)
                .wrap_err("Inrou predecessor is not an exact prepared-mutation V1 envelope")?;
            if !matches!(
                &envelope.operation,
                PreparedTransactionOperationV1::FinalCanary(_)
            ) {
                eyre::bail!("Inrou predecessor is not the final-canary V1 variant");
            }
            (
                json::to_value(&envelope)?,
                json::to_json(&envelope)?.into_bytes(),
            )
        }
        "inrou_bundle_pin" | "inrou_guest_pin" => {
            let envelope: PreparedInrouEnvelopeV1 = json::from_slice(bytes)
                .wrap_err("Inrou predecessor is not an exact prepared-Inrou V1 envelope")?;
            let expected_variant = matches!(
                (&envelope.operation, expected_kind),
                (
                    PreparedInrouOperationV1::InrouBundlePin(_),
                    "inrou_bundle_pin"
                ) | (
                    PreparedInrouOperationV1::InrouGuestPin(_),
                    "inrou_guest_pin"
                )
            );
            if !expected_variant {
                eyre::bail!("Inrou predecessor has the wrong prepared-Inrou V1 variant");
            }
            (
                json::to_value(&envelope)?,
                json::to_json(&envelope)?.into_bytes(),
            )
        }
        _ => return Err(eyre!("unsupported Inrou predecessor kind")),
    };
    canonical.push(b'\n');
    if canonical != bytes {
        eyre::bail!("Inrou predecessor envelope is not canonical newline JSON");
    }
    Ok(value)
}

fn verify_exact_committed_transaction(
    client: &IrohaClient,
    expected: &SignedTransaction,
    expected_wire: &[u8],
) -> Result<()> {
    let entrypoint_hash = expected.hash_as_entrypoint();
    let one = NonZeroU64::new(1).expect("nonzero exact transaction bound");
    let committed = client
        .query(FindTransactions::new())
        .filter(CompoundPredicate::from_filters(CommittedTxFilters {
            entry_eq: Some(entrypoint_hash),
            ..CommittedTxFilters::default()
        }))
        .with_pagination(Pagination::new(Some(one), 0))
        .with_fetch_size(FetchSize::new(Some(one)))
        .execute_all()?;
    let [committed] = committed.as_slice() else {
        eyre::bail!("Applied predecessor lacks one exact committed transaction proof");
    };
    let TransactionEntrypoint::External(transaction) = committed.entrypoint() else {
        eyre::bail!("Applied predecessor resolves to a non-external entrypoint");
    };
    if committed.result().is_err()
        || transaction.hash() != expected.hash()
        || transaction.hash_as_entrypoint() != entrypoint_hash
        || transaction.encode_wire_v1()? != expected_wire
    {
        eyre::bail!("committed predecessor differs from its exact prepared transaction");
    }
    Ok(())
}
/// Read-only verification of one retained canonical Taira Inrou stage.
#[derive(clap::Args, Debug)]
pub struct InrouCheck {
    /// Public Torii root URL used only for signed status and public route reads.
    #[arg(long, default_value = DEFAULT_PUBLIC_ROOT)]
    pub public_root: String,
    /// Owner-only stage created by `iroha taira inrou-stage` and retained after deployment.
    #[arg(long, value_name = "PATH")]
    pub stage_dir: PathBuf,
    /// Exact revision mode encoded by the retained stage.
    #[arg(long, value_enum)]
    pub mode: InrouCanaryMode,
    /// Maximum convergence time for signed status and all four route identities.
    #[arg(long, value_name = "SECS", default_value_t = 180)]
    pub timeout_secs: u64,
    /// Emit a stable JSON evidence receipt.
    #[arg(long)]
    pub json: bool,
}
impl Run for InrouCheck {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_inrou_canary_timeout(self.timeout_secs)?;
        ensure_canonical_taira_client_identity(context.config())?;
        let _chain_discriminant = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
        let stage = crate::soracloud::load_taira_inrou_stage_identity(
            context.config(),
            &self.stage_dir,
            self.mode,
        )?;
        let public_root = normalize_root_url(&self.public_root)?;
        preflight_taira_network_identity(&public_root, context.config())?;
        let mut status_config = context.config().clone();
        status_config.torii_api_url = Url::parse(&format!("{public_root}/"))
            .wrap_err("failed to bind the signed status client to the selected Taira root")?;
        let status_client = IrohaClient::new(status_config);
        let receipt = verify_inrou_check(&public_root, &status_client, &stage, self.timeout_secs)?;
        render_report(context, self.json, &receipt)?;
        if report_status(&receipt) != Some("ok") {
            eyre::bail!("Taira Inrou read-only check found hard failures");
        }
        Ok(())
    }
}
fn ensure_canonical_taira_client_identity(config: &Config) -> Result<()> {
    if config.chain.to_string() != DEFAULT_CHAIN_ID {
        eyre::bail!(
            "Taira Inrou canary requires canonical chain `{DEFAULT_CHAIN_ID}`; configured `{}`",
            config.chain
        );
    }
    if config.account_chain_discriminant != DEFAULT_CHAIN_DISCRIMINANT {
        eyre::bail!(
            "Taira Inrou canary requires chain discriminant {DEFAULT_CHAIN_DISCRIMINANT}; configured {}",
            config.account_chain_discriminant
        );
    }
    Ok(())
}
fn preflight_taira_network_identity(public_root: &str, config: &Config) -> Result<()> {
    let http = http_client()?;
    let puzzle_url = join_url(public_root, "/v1/accounts/faucet/puzzle")?;
    let puzzle = http_json(&http, reqwest::Method::GET, puzzle_url.as_str(), None)?;
    if puzzle.status != 200 {
        eyre::bail!(
            "Taira network-identity preflight returned HTTP {} from {}",
            puzzle.status,
            puzzle_url
        );
    }
    let body = puzzle
        .body
        .as_ref()
        .ok_or_else(|| eyre!("Taira network-identity preflight returned a non-JSON puzzle"))?;
    validate_taira_puzzle_identity(body, &config.network_id).map(|_| ())
}
#[derive(Debug)]
struct HttpJson {
    status: u16,
    body: Option<Value>,
}
#[derive(Debug)]
struct CanarySigner {
    key_pair: KeyPair,
    account_id: AccountId,
}
fn run_doctor(public_root: &str) -> Result<Value> {
    let public_root = normalize_root_url(public_root)?;
    let http = http_client()?;
    let mut checks = Vec::new();
    let mut warnings = Vec::new();
    let mut failures = Vec::new();
    let empty_object = norito::json!({});
    for (name, method, path, expected_statuses) in ROUTE_CHECKS {
        let url = join_url(&public_root, path)?;
        let (method, body) = match method {
            RouteCheckMethod::Get => (reqwest::Method::GET, None),
            RouteCheckMethod::PostEmptyObject => (reqwest::Method::POST, Some(&empty_object)),
        };
        let result = http_json(&http, method, url.as_str(), body)?;
        let status_ok = expected_statuses.contains(&result.status);
        let semantic_error = if status_ok {
            match *name {
                "status" => validate_public_status(result.body.as_ref()).err(),
                "time_now" => validate_time_snapshot(result.body.as_ref()).err(),
                _ => None,
            }
        } else {
            None
        };
        let ok = status_ok && semantic_error.is_none();
        push_check(
            &mut checks,
            name,
            result.status,
            ok,
            semantic_error
                .clone()
                .or_else(|| route_check_detail(expected_statuses)),
        );
        if !status_ok {
            failures.push(format!(
                "{name} returned HTTP {}; expected {}",
                result.status,
                expected_statuses
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(" or ")
            ));
        } else if let Some(error) = semantic_error {
            failures.push(error);
        }
        if *name == "status" && ok {
            collect_status_warnings(result.body.as_ref(), &mut warnings);
        }
    }
    let mcp_url = join_url(&public_root, "/v1/mcp")?;
    let mcp_get = http_json(&http, reqwest::Method::GET, mcp_url.as_str(), None)?;
    let mcp_get_error = (mcp_get.status == 200)
        .then(|| validate_mcp_protocol_response(mcp_get.body.as_ref(), false).err())
        .flatten();
    let mcp_get_ok = mcp_get.status == 200 && mcp_get_error.is_none();
    push_check(
        &mut checks,
        "mcp_get",
        mcp_get.status,
        mcp_get_ok,
        mcp_get_error.clone(),
    );
    if !mcp_get_ok {
        failures.push(
            mcp_get_error.unwrap_or_else(|| format!("mcp_get returned HTTP {}", mcp_get.status)),
        );
    }
    let initialize = norito::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "iroha-taira-doctor", "version": "1" }
        }
    });
    let mcp_init = http_json(
        &http,
        reqwest::Method::POST,
        mcp_url.as_str(),
        Some(&initialize),
    )?;
    let mcp_init_error = (mcp_init.status == 200)
        .then(|| validate_mcp_protocol_response(mcp_init.body.as_ref(), true).err())
        .flatten();
    let mcp_init_ok = mcp_init.status == 200 && mcp_init_error.is_none();
    push_check(
        &mut checks,
        "mcp_initialize",
        mcp_init.status,
        mcp_init_ok,
        mcp_init_error.clone(),
    );
    if !mcp_init_ok {
        failures.push(
            mcp_init_error
                .unwrap_or_else(|| format!("mcp_initialize returned HTTP {}", mcp_init.status)),
        );
    }
    let tools_payload = norito::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });
    let tools = http_json(
        &http,
        reqwest::Method::POST,
        mcp_url.as_str(),
        Some(&tools_payload),
    )?;
    let parsed_tool_names = (tools.status == 200)
        .then(|| mcp_tool_names(tools.body.as_ref()))
        .transpose();
    let tools_error = parsed_tool_names.as_ref().err().cloned();
    let tools_ok = tools.status == 200 && tools_error.is_none();
    push_check(
        &mut checks,
        "mcp_tools_list",
        tools.status,
        tools_ok,
        tools_error.clone(),
    );
    if !tools_ok {
        failures.push(
            tools_error.unwrap_or_else(|| format!("mcp_tools_list returned HTTP {}", tools.status)),
        );
    } else {
        let tool_names = parsed_tool_names
            .expect("successful MCP tool parsing has one result")
            .expect("HTTP 200 MCP tools/list was parsed above");
        let missing: Vec<String> = REQUIRED_MCP_TOOLS
            .iter()
            .copied()
            .filter(|name| !tool_names.iter().any(|present| present == name))
            .map(str::to_owned)
            .collect();
        if missing.is_empty() {
            push_check(
                &mut checks,
                "mcp_required_tools",
                200,
                true,
                Some("all required curated tools are present".to_owned()),
            );
        } else {
            if !missing.is_empty() {
                failures.push(format!("MCP tools/list missing: {}", missing.join(", ")));
            }
            push_check(
                &mut checks,
                "mcp_required_tools",
                200,
                false,
                Some(format!("missing=[{}]", missing.join(", "))),
            );
        }
    }
    let status = if failures.is_empty() { "ok" } else { "fail" };
    report_value(
        "taira_doctor",
        status,
        &public_root,
        checks,
        warnings,
        failures,
        Map::new(),
    )
}
#[derive(Clone, Debug)]
struct InrouProbeIdentity {
    service_name: String,
    service_version: String,
    route_host: String,
    route_path_prefix: String,
    healthcheck_path: String,
    stage_mode: String,
    container_manifest_hash: String,
    service_manifest_hash: String,
}
impl From<&crate::soracloud::TairaInrouStageIdentity> for InrouProbeIdentity {
    fn from(stage: &crate::soracloud::TairaInrouStageIdentity) -> Self {
        Self {
            service_name: stage.service_name.clone(),
            service_version: stage.service_version.clone(),
            route_host: stage.route_host.clone(),
            route_path_prefix: stage.route_path_prefix.clone(),
            healthcheck_path: stage.healthcheck_path.clone(),
            stage_mode: stage.stage_mode.clone(),
            container_manifest_hash: stage.container_manifest_hash.clone(),
            service_manifest_hash: stage.service_manifest_hash.clone(),
        }
    }
}
fn validate_exact_inrou_canary_status(
    status: &Value,
    deployment: &InrouProbeIdentity,
) -> Result<(u64, u64), String> {
    if !crate::soracloud::is_taira_inrou_canary_service_version(&deployment.service_version) {
        return Err("retained Taira Inrou stage has a noncanonical revision identity".to_owned());
    }
    validate_soracloud_status(Some(status))?;
    let root = status
        .as_object()
        .ok_or_else(|| "Soracloud status response is not an object".to_owned())?;
    if root.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return Err("Soracloud status is not canonical schema version 1".to_owned());
    }
    if root
        .get("runtime_manager")
        .and_then(Value::as_object)
        .and_then(|runtime| runtime.get("available"))
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("runtime manager is unavailable".to_owned());
    }
    let topology = root
        .get("hosted_http_topology")
        .and_then(Value::as_object)
        .ok_or_else(|| "Soracloud status is missing hosted HTTP topology".to_owned())?;
    let active_adverts = topology
        .get("active_capability_adverts")
        .and_then(Value::as_u64)
        .ok_or_else(|| "Soracloud status is missing exact active_capability_adverts".to_owned())?;
    let hosted_replicas = topology
        .get("hosted_replica_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| "Soracloud status is missing exact hosted_replica_count".to_owned())?;
    let placed_hosts = topology
        .get("placed_host_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| "Soracloud status is missing exact placed_host_count".to_owned())?;
    let unavailable_replicas = topology
        .get("unavailable_replica_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| "Soracloud status is missing exact unavailable_replica_count".to_owned())?;
    if active_adverts != 4 || placed_hosts != 4 || hosted_replicas != 4 || unavailable_replicas != 0
    {
        return Err(format!(
            "requires exactly four available Inrou hosts and placements (adverts={active_adverts}, placed_hosts={placed_hosts}, replicas={hosted_replicas}, unavailable_replicas={unavailable_replicas})"
        ));
    }
    let services = root
        .get("control_plane")
        .and_then(Value::as_object)
        .and_then(|control_plane| control_plane.get("services"))
        .and_then(Value::as_array)
        .ok_or_else(|| "Soracloud status is missing control-plane services".to_owned())?;
    let mut matching_services = services.iter().filter(|service| {
        service.get("service_name").and_then(Value::as_str)
            == Some(deployment.service_name.as_str())
    });
    let service = matching_services.next().ok_or_else(|| {
        format!(
            "canary service `{}` is not present in authoritative status",
            deployment.service_name
        )
    })?;
    if matching_services.next().is_some() {
        return Err(format!(
            "canary service `{}` appears more than once in authoritative status",
            deployment.service_name
        ));
    }
    if service.get("current_version").and_then(Value::as_str)
        != Some(deployment.service_version.as_str())
    {
        return Err("authoritative canary version does not match the retained stage".to_owned());
    }
    let (expected_action, revision_count_is_valid): (&str, fn(u64) -> bool) =
        match deployment.stage_mode.as_str() {
            "deploy" => ("Deploy", |count| count == 1),
            "upgrade" => ("Upgrade", |count| count >= 2),
            other => return Err(format!("unsupported Inrou mutation mode `{other}`")),
        };
    let revision_count = service
        .get("revision_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| "authoritative canary status is missing revision_count".to_owned())?;
    if !revision_count_is_valid(revision_count) {
        return Err(format!(
            "authoritative canary revision count {revision_count} is invalid after {}",
            deployment.stage_mode,
        ));
    }
    if !service.get("active_rollout").is_some_and(Value::is_null) {
        return Err("Taira Inrou canary must report an explicit null active_rollout".to_owned());
    }
    match deployment.stage_mode.as_str() {
        "deploy" => {
            if !service.get("last_rollout").is_some_and(Value::is_null) {
                return Err(
                    "initial Taira Inrou deploy must report an explicit null last_rollout"
                        .to_owned(),
                );
            }
        }
        "upgrade" => {
            let rollout = service
                .get("last_rollout")
                .and_then(Value::as_object)
                .ok_or_else(|| "Taira Inrou upgrade is missing its promoted rollout".to_owned())?;
            let baseline_version = rollout
                .get("baseline_version")
                .and_then(Value::as_str)
                .filter(|version| {
                    crate::soracloud::is_taira_inrou_canary_service_version(version)
                        && *version != deployment.service_version.as_str()
                });
            let promoted = baseline_version.is_some()
                && rollout.get("candidate_version").and_then(Value::as_str)
                    == Some(deployment.service_version.as_str())
                && rollout.get("canary_percent").and_then(Value::as_u64) == Some(100)
                && rollout.get("traffic_percent").and_then(Value::as_u64) == Some(100)
                && rollout
                    .get("stage")
                    .and_then(|stage| tagged_enum_name(stage, "stage"))
                    == Some("Promoted");
            if !promoted {
                return Err(
                    "Taira Inrou upgrade rollout is not an exact promoted immutable revision transition"
                        .to_owned(),
                );
            }
        }
        _ => unreachable!("mutation mode was validated above"),
    }
    let revision = service
        .get("latest_revision")
        .and_then(Value::as_object)
        .ok_or_else(|| "canary service is missing its latest revision".to_owned())?;
    for (field, expected) in [
        (
            "container_manifest_hash",
            deployment.container_manifest_hash.as_str(),
        ),
        (
            "service_manifest_hash",
            deployment.service_manifest_hash.as_str(),
        ),
    ] {
        let actual = revision
            .get(field)
            .and_then(|value| json::from_value::<Hash>(value.clone()).ok())
            .map(|hash| hash.to_string());
        if actual.as_deref() != Some(expected) {
            return Err(format!(
                "authoritative {field} does not match the fully revalidated retained stage"
            ));
        }
    }
    let canonical = revision.get("replicas").and_then(Value::as_u64) == Some(4)
        && revision.get("service_version").and_then(Value::as_str)
            == Some(deployment.service_version.as_str())
        && revision
            .get("action")
            .and_then(|action| tagged_enum_name(action, "action"))
            == Some(expected_action)
        && revision
            .get("runtime")
            .and_then(|runtime| tagged_enum_name(runtime, "runtime"))
            == Some("Inrou")
        && revision
            .get("execution_plane")
            .and_then(|plane| tagged_enum_name(plane, "execution_plane"))
            == Some("HttpService")
        && revision.get("route_host").and_then(Value::as_str)
            == Some(deployment.route_host.as_str())
        && revision.get("route_path_prefix").and_then(Value::as_str)
            == Some(deployment.route_path_prefix.as_str());
    if !canonical {
        return Err(
            "authoritative canary revision differs from the canonical four-replica Inrou route"
                .to_owned(),
        );
    }
    Ok((active_adverts, hosted_replicas))
}
fn exact_inrou_health_identity(
    body: &Value,
    deployment: &InrouProbeIdentity,
) -> Option<(u64, String, String)> {
    let object = body.as_object()?;
    if object.len() != 5 {
        return None;
    }
    let service = object.get("service").and_then(Value::as_str)?;
    let service_version = object.get("service_version").and_then(Value::as_str)?;
    let runtime = object.get("runtime").and_then(Value::as_str)?;
    let replica_slot = object.get("replica_slot").and_then(Value::as_u64)?;
    let identity = object.get("identity").and_then(Value::as_str)?;
    let expected_identity = format!("{}:replica:{replica_slot}", deployment.service_name);
    if service != deployment.service_name.as_str()
        || service_version != deployment.service_version.as_str()
        || runtime != "Inrou"
        || !(1..=4).contains(&replica_slot)
        || identity != expected_identity
    {
        return None;
    }
    let encoded = json::to_vec(body).ok()?;
    Some((
        replica_slot,
        identity.to_owned(),
        hex::encode(Sha256::digest(encoded)),
    ))
}
fn inrou_canary_health_path(route_prefix: &str, healthcheck_path: &str) -> String {
    format!(
        "{}/{}",
        route_prefix.trim_end_matches('/'),
        healthcheck_path.trim_start_matches('/')
    )
}
struct InrouProbeObservation {
    health_path: String,
    active_host_adverts: u64,
    hosted_replica_count: u64,
    checks: Vec<Value>,
    failures: Vec<String>,
    replica_identities: Value,
}
fn probe_inrou_service(
    public_root: &str,
    status_client: &IrohaClient,
    deployment: &InrouProbeIdentity,
    timeout_secs: u64,
) -> Result<InrouProbeObservation> {
    validate_inrou_canary_timeout(timeout_secs)?;
    let http = HttpClient::builder()
        .timeout(Duration::from_secs(timeout_secs.min(5)))
        .user_agent("iroha-taira-inrou-probe/1")
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .wrap_err("failed to build Taira Inrou verification HTTP client")?;
    let health_path =
        inrou_canary_health_path(&deployment.route_path_prefix, &deployment.healthcheck_path);
    let health_base = join_url(public_root, &health_path)?;
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut nonce = 0_u64;
    let mut status_ready = false;
    let mut active_adverts = 0_u64;
    let mut hosted_replicas = 0_u64;
    let mut last_status_code = 0_u16;
    let mut last_status_error = "status not observed".to_owned();
    let mut last_route_code = 0_u16;
    let mut last_route_error = "route not observed".to_owned();
    let mut identities = BTreeMap::<u64, (String, String)>::new();
    while Instant::now() < deadline && (!status_ready || identities.len() < 4) {
        status_ready = false;
        active_adverts = 0;
        hosted_replicas = 0;
        match account_signed_soracloud_status(status_client) {
            Ok(response) => {
                last_status_code = response.status;
                match response
                    .body
                    .as_ref()
                    .ok_or_else(|| "Soracloud status returned non-JSON".to_owned())
                    .and_then(|status| validate_exact_inrou_canary_status(status, deployment))
                {
                    Ok((adverts, replicas)) if response.status == 200 => {
                        status_ready = true;
                        active_adverts = adverts;
                        hosted_replicas = replicas;
                        last_status_error.clear();
                    }
                    Ok(_) => last_status_error = format!("HTTP {}", response.status),
                    Err(error) => last_status_error = error,
                }
            }
            Err(error) => last_status_error = format!("{error:#}"),
        }
        let mut health_url = health_base.clone();
        health_url
            .query_pairs_mut()
            .append_pair("taira_inrou_probe", &nonce.to_string());
        nonce = nonce.saturating_add(1);
        let route_response = http
            .get(health_url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::HOST, deployment.route_host.as_str())
            .send();
        match route_response {
            Ok(response) => {
                last_route_code = response.status().as_u16();
                let response = match decode_http_json_response(response) {
                    Ok(response) => response,
                    Err(error) => {
                        last_route_error = format!("{error:#}");
                        if !status_ready || identities.len() < 4 {
                            std::thread::sleep(Duration::from_millis(200));
                        }
                        continue;
                    }
                };
                if response.status != 200 {
                    last_route_error = format!("HTTP {}", response.status);
                    if !status_ready || identities.len() < 4 {
                        std::thread::sleep(Duration::from_millis(200));
                    }
                    continue;
                }
                let identity = response
                    .body
                    .as_ref()
                    .and_then(|body| exact_inrou_health_identity(body, deployment));
                if let Some((slot, identity, digest)) = identity {
                    identities.insert(slot, (identity, digest));
                    last_route_error.clear();
                } else {
                    last_route_error =
                        "health response violated the canary identity contract".to_owned();
                }
            }
            Err(error) => last_route_error = format!("{error:#}"),
        }
        if !status_ready || identities.len() < 4 {
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    let mut checks = Vec::new();
    push_check(
        &mut checks,
        "inrou_authoritative_status",
        last_status_code,
        status_ready,
        Some(if status_ready {
            format!("active_adverts={active_adverts}, hosted_replicas={hosted_replicas}")
        } else {
            last_status_error.clone()
        }),
    );
    push_check(
        &mut checks,
        "inrou_public_routes",
        last_route_code,
        identities.len() == 4,
        Some(if identities.len() == 4 {
            "observed deterministic identities for replica slots 1, 2, 3, and 4".to_owned()
        } else {
            format!(
                "observed {}/4 replica identities; {last_route_error}",
                identities.len()
            )
        }),
    );
    let mut failures = Vec::new();
    if !status_ready {
        failures.push(format!(
            "authoritative Inrou status did not converge: {last_status_error}"
        ));
    }
    if identities.len() != 4 {
        failures.push(format!(
            "public Inrou route did not reach all replicas: {last_route_error}"
        ));
    }
    let replica_identities = Value::Array(
        identities
            .into_iter()
            .map(|(slot, (identity, response_sha256))| {
                norito::json!({
                    "replica_slot": slot,
                    "identity": identity,
                    "response_sha256": response_sha256
                })
            })
            .collect(),
    );
    Ok(InrouProbeObservation {
        health_path,
        active_host_adverts: active_adverts,
        hosted_replica_count: hosted_replicas,
        checks,
        failures,
        replica_identities,
    })
}
fn verify_inrou_check(
    public_root: &str,
    status_client: &IrohaClient,
    stage: &crate::soracloud::TairaInrouStageIdentity,
    timeout_secs: u64,
) -> Result<Value> {
    let expected = InrouProbeIdentity::from(stage);
    let observation = probe_inrou_service(public_root, status_client, &expected, timeout_secs)?;
    let observed_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock predates the Unix epoch; refusing stale-looking Inrou evidence")?
        .as_millis();
    let observed_at_unix_ms = u64::try_from(observed_at_unix_ms)
        .wrap_err("Inrou evidence timestamp exceeds the V1 u64 range")?;
    let mut extra = Map::new();
    extra.insert(
        "service_name".to_owned(),
        Value::from(stage.service_name.clone()),
    );
    extra.insert(
        "service_version".to_owned(),
        Value::from(stage.service_version.clone()),
    );
    extra.insert(
        "route_host".to_owned(),
        Value::from(stage.route_host.clone()),
    );
    extra.insert(
        "route_path".to_owned(),
        Value::from(observation.health_path),
    );
    extra.insert(
        "active_host_adverts".to_owned(),
        Value::from(observation.active_host_adverts),
    );
    extra.insert(
        "hosted_replica_count".to_owned(),
        Value::from(observation.hosted_replica_count),
    );
    extra.insert(
        "bundle_hash".to_owned(),
        Value::from(stage.bundle_hash.clone()),
    );
    extra.insert(
        "bundle_content_cid".to_owned(),
        Value::from(stage.bundle_content_cid.clone()),
    );
    extra.insert(
        "bundle_manifest_digest_hex".to_owned(),
        Value::from(stage.bundle_manifest_digest_hex.clone()),
    );
    extra.insert(
        "guest_content_cid".to_owned(),
        Value::from(stage.guest_content_cid.clone()),
    );
    extra.insert(
        "guest_manifest_digest_hex".to_owned(),
        Value::from(stage.guest_manifest_digest_hex.clone()),
    );
    extra.insert(
        "container_manifest_hash".to_owned(),
        Value::from(stage.container_manifest_hash.clone()),
    );
    extra.insert(
        "service_manifest_hash".to_owned(),
        Value::from(stage.service_manifest_hash.clone()),
    );
    extra.insert(
        "observed_at_unix_ms".to_owned(),
        Value::from(observed_at_unix_ms),
    );
    extra.insert(
        "replica_identities".to_owned(),
        observation.replica_identities,
    );
    report_value(
        "taira_inrou_check",
        if observation.failures.is_empty() {
            "ok"
        } else {
            "fail"
        },
        public_root,
        observation.checks,
        Vec::new(),
        observation.failures,
        extra,
    )
}
fn validate_inrou_canary_timeout(timeout_secs: u64) -> Result<()> {
    if timeout_secs == 0 {
        eyre::bail!("--timeout-secs must be greater than zero");
    }
    Ok(())
}

const PREPARED_BINDING_METADATA: &str = "taira_public_reset_binding";
const PREPARED_OPERATION_METADATA: &str = "taira_prepared_operation";
const PREPARED_SEMANTIC_METADATA: &str = "taira_prepared_semantic_hash";

struct ValidatedPreparedOperation {
    envelope: PreparedMutationEnvelopeV1,
    transaction: Option<SignedTransaction>,
    wire: Option<Vec<u8>>,
    envelope_bytes: Vec<u8>,
}

impl ValidatedPreparedOperation {
    fn transaction(&self) -> Result<&SignedTransaction> {
        self.transaction
            .as_ref()
            .ok_or_else(|| eyre!("the prepared operation has no transaction"))
    }

    fn wire(&self) -> Result<&[u8]> {
        self.wire
            .as_deref()
            .ok_or_else(|| eyre!("the prepared operation has no transaction wire"))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PreparedRecoveryClassification {
    Absent,
    Applied {
        block_height: Option<u64>,
        evidence: String,
    },
    Pending {
        terminal_kind: String,
    },
    Rejected {
        terminal_kind: String,
    },
}

fn run_write_canary_exact<C: RunContext>(context: &mut C, args: &WriteCanary) -> Result<Value> {
    ensure_canonical_taira_client_identity(context.config())?;
    let _guard = ChainDiscriminantGuard::enter(DEFAULT_CHAIN_DISCRIMINANT);
    let public_root = normalize_root_url(&args.public_root)?;
    let binding = args.binding()?;
    let action = args.prepared_action()?;
    args.validate_prerequisite_action(action)?;
    let expected_fee_payment = context.transaction_fee_payment()?;
    match action {
        PreparedEnvelopeAction::Prepare(output) => {
            require_forward_binding_current(&binding)?;
            prove_predecessor_applied(
                context.config(),
                args,
                &public_root,
                &binding,
                &expected_fee_payment,
            )?;
            let envelope = prepare_one_write_canary_operation(
                context.config(),
                args,
                &public_root,
                &binding,
                expected_fee_payment.clone(),
            )?;
            let envelope_bytes = canonical_prepared_envelope_bytes(&envelope)?;
            write_prepared_envelope(output, &envelope_bytes)?;
            let (outcome, evidence) = initial_prepared_report_state(&envelope.operation);
            prepared_operation_report(
                &public_root,
                args,
                &envelope,
                &envelope_bytes,
                outcome,
                None,
                evidence,
            )
        }
        PreparedEnvelopeAction::Submit(fd) => {
            let validated = load_and_validate_prepared_operation(
                context.config(),
                args,
                &public_root,
                fd,
                &expected_fee_payment,
                PreparedLifetimeCheck::LiveForward,
            )?;
            let classification = submit_exact_prepared_operation(
                context.config(),
                args,
                &public_root,
                &validated,
                &expected_fee_payment,
            )?;
            report_prepared_classification(&public_root, args, &validated, classification)
        }
        PreparedEnvelopeAction::Recover(fd) => {
            let validated = load_and_validate_prepared_operation(
                context.config(),
                args,
                &public_root,
                fd,
                &expected_fee_payment,
                PreparedLifetimeCheck::Structural,
            )?;
            let client = IrohaClient::new(write_canary_config(
                context.config(),
                &public_root,
                &CanarySigner {
                    account_id: context.config().account.clone(),
                    key_pair: context.config().key_pair.clone(),
                },
            )?);
            let classification = classify_exact_prepared_operation(&client, &validated)?;
            report_prepared_classification(&public_root, args, &validated, classification)
        }
    }
}

fn initial_prepared_report_state(
    operation: &PreparedTransactionOperationV1,
) -> (&'static str, Option<String>) {
    let proof_required_evidence = match operation {
        PreparedTransactionOperationV1::OnboardingProofRequired(proof_required) => {
            Some(proof_required.result.semantic_hash_hex.as_str())
        }
        PreparedTransactionOperationV1::OnboardingPrepared(_)
        | PreparedTransactionOperationV1::FaucetPrepared(_)
        | PreparedTransactionOperationV1::FinalCanary(_) => None,
    };
    initial_prepared_report_state_from_evidence(proof_required_evidence)
}

fn initial_prepared_report_state_from_evidence(
    proof_required_evidence: Option<&str>,
) -> (&'static str, Option<String>) {
    match proof_required_evidence {
        Some(evidence) => ("ProofRequired", Some(evidence.to_owned())),
        None => ("Prepared", None),
    }
}

fn prepare_one_write_canary_operation(
    config: &Config,
    args: &WriteCanary,
    public_root: &str,
    binding: &PreparedMutationBindingV1,
    fee_payment: FeePaymentIntent,
) -> Result<PreparedMutationEnvelopeV1> {
    match args.operation {
        WriteCanaryOperation::Onboarding => {
            let _ = args.require_onboarding_token()?;
            prepare_onboarding_operation(config, args, public_root, binding, &fee_payment)
        }
        WriteCanaryOperation::Faucet => {
            prepare_faucet_operation(config, args, public_root, binding, &fee_payment)
        }
        WriteCanaryOperation::FinalCanary => {
            prepare_final_canary_operation(config, args, public_root, binding, fee_payment)
        }
    }
}

fn prove_predecessor_applied(
    config: &Config,
    args: &WriteCanary,
    public_root: &str,
    current_binding: &PreparedMutationBindingV1,
    expected_fee_payment: &FeePaymentIntent,
) -> Result<()> {
    let predecessor = match args.operation {
        WriteCanaryOperation::Onboarding => return Ok(()),
        WriteCanaryOperation::Faucet => WriteCanaryOperation::Onboarding,
        WriteCanaryOperation::FinalCanary => WriteCanaryOperation::Faucet,
    };
    let fd = args
        .prerequisite_envelope_fd
        .ok_or_else(|| eyre!("the next operation requires its exact predecessor envelope"))?;
    let (envelope, envelope_bytes) = read_prepared_envelope(fd)?;
    let mut predecessor_binding = current_binding.clone();
    predecessor_binding.kind = predecessor.mutation_kind().to_owned();
    predecessor_binding.idempotency_key = write_canary_child_idempotency_key(
        &predecessor_binding.authorization_nonce,
        &predecessor_binding.phase,
        predecessor.mutation_kind(),
    );
    let wire = envelope
        .operation
        .signed_transaction_wire_hex()
        .map(|wire| hex::decode(wire).wrap_err("predecessor transaction wire is not hexadecimal"))
        .transpose()?;
    let mut validated = validate_prepared_operation(
        config,
        args,
        predecessor,
        &predecessor_binding,
        public_root,
        envelope,
        wire,
        expected_fee_payment,
        PreparedLifetimeCheck::Structural,
    )?;
    validated.envelope_bytes = envelope_bytes;
    let signer = CanarySigner {
        account_id: config.account.clone(),
        key_pair: config.key_pair.clone(),
    };
    let client = IrohaClient::new(write_canary_config(config, public_root, &signer)?);
    match classify_exact_prepared_operation(&client, &validated)? {
        PreparedRecoveryClassification::Applied { .. } => Ok(()),
        PreparedRecoveryClassification::Absent => {
            eyre::bail!("predecessor transaction is absent; refusing to prepare the next operation")
        }
        PreparedRecoveryClassification::Pending { terminal_kind } => eyre::bail!(
            "predecessor transaction is not Applied (`{terminal_kind}`); refusing to prepare the next operation"
        ),
        PreparedRecoveryClassification::Rejected { terminal_kind } => eyre::bail!(
            "predecessor transaction is terminally rejected (`{terminal_kind}`); refusing to prepare the next operation"
        ),
    }
}

fn prepare_final_canary_operation(
    config: &Config,
    args: &WriteCanary,
    public_root: &str,
    binding: &PreparedMutationBindingV1,
    fee_payment: FeePaymentIntent,
) -> Result<PreparedMutationEnvelopeV1> {
    let signer = resolve_canary_signer(config)?;
    let canary_config = write_canary_config(config, public_root, &signer)?;
    let client = IrohaClient::new(canary_config.clone());
    let message = prepared_canary_message(binding)?;
    let semantic_sha256 = prepared_semantic_sha256(binding, WRITE_CANARY_OPERATION, &message)?;
    let mut metadata = Metadata::default();
    insert_string_metadata(&mut metadata, "taira_canary", "write-canary")?;
    insert_string_metadata(
        &mut metadata,
        "taira_write_canary_idempotency_v1",
        &binding.idempotency_key,
    )?;
    let binding_value = json::to_value(binding).wrap_err("serialize prepared mutation binding")?;
    metadata.insert(
        Name::from_str(PREPARED_BINDING_METADATA)?,
        IrohaJson::from_norito_value_ref(&binding_value)
            .wrap_err("encode prepared mutation binding metadata")?,
    );
    insert_string_metadata(
        &mut metadata,
        PREPARED_OPERATION_METADATA,
        WRITE_CANARY_OPERATION,
    )?;
    insert_string_metadata(&mut metadata, PREPARED_SEMANTIC_METADATA, &semantic_sha256)?;
    let instruction = Log::new(LogLevel::INFO, message);
    let executable = Executable::Instructions(vec![InstructionBox::from(instruction)].into());
    let (transaction, fee_quote) =
        quote_and_sign_transaction(&client, executable, fee_payment.clone(), metadata)
            .wrap_err("failed to quote and sign exact Taira canary transaction")?;
    let wire = transaction
        .encode_wire_v1()
        .map_err(|error| eyre!("failed to encode exact Taira canary transaction: {error}"))?;
    if wire.len() > PREPARED_TRANSACTION_MAX_BYTES {
        eyre::bail!("prepared Taira canary transaction exceeds its V1 byte bound");
    }
    let prepared = IrohaClient::prepare_transaction_payload(&transaction);
    if prepared.as_bytes() != wire {
        eyre::bail!("prepared submission changed exact Taira canary transaction bytes");
    }
    let operation = FinalCanaryPreparedTransactionV1 {
        schema: PREPARED_OPERATION_SCHEMA_V1.to_owned(),
        binding: binding.clone(),
        operation: WRITE_CANARY_OPERATION.to_owned(),
        transaction_hash_hex: hex::encode(transaction.hash().as_ref()),
        signed_transaction_wire_hex: hex::encode(&wire),
        signed_transaction_wire_sha256: hex::encode(Sha256::digest(&wire)),
        semantic_hash_hex: semantic_sha256,
        fee_payment: transaction.fee_payment_intent().clone(),
        fee_quote,
    };
    let envelope = PreparedMutationEnvelopeV1 {
        schema: PREPARED_ENVELOPE_SCHEMA_V1.to_owned(),
        binding: binding.clone(),
        public_root: public_root.to_owned(),
        chain_id: canary_config.chain.to_string(),
        network_id: canary_config.network_id.to_string(),
        authority: canary_config.account.to_string(),
        operation: PreparedTransactionOperationV1::FinalCanary(operation),
    };
    validate_prepared_operation(
        config,
        args,
        args.operation,
        binding,
        public_root,
        envelope.clone(),
        Some(wire),
        &fee_payment,
        PreparedLifetimeCheck::LiveForward,
    )?;
    Ok(envelope)
}

fn prepared_canary_message(binding: &PreparedMutationBindingV1) -> Result<String> {
    validate_prepared_binding(binding)?;
    Ok(format!(
        "taira-public-reset-write-canary-v1:{}:{}:{}:{}",
        binding.authorization_sha256,
        binding.authorization_nonce,
        binding.phase,
        binding.idempotency_key
    ))
}

fn prepared_semantic_sha256(
    binding: &PreparedMutationBindingV1,
    operation: &str,
    semantic_identity: &str,
) -> Result<String> {
    validate_prepared_binding(binding)?;
    let binding_bytes = json::to_vec(binding).wrap_err("encode prepared mutation binding")?;
    let mut digest = Sha256::new();
    for frame in [
        b"iroha:taira:prepared-operation:semantic:v1\0".as_slice(),
        binding_bytes.as_slice(),
        operation.as_bytes(),
        semantic_identity.as_bytes(),
    ] {
        let length = u64::try_from(frame.len()).expect("frame length fits u64");
        digest.update(length.to_be_bytes());
        digest.update(frame);
    }
    Ok(hex::encode(digest.finalize()))
}

fn require_forward_binding_current(binding: &PreparedMutationBindingV1) -> Result<()> {
    let now = current_unix_ms()?;
    if now >= binding.execution_expires_at_unix_ms {
        eyre::bail!("prepared mutation execution expiry bars a new forward effect");
    }
    Ok(())
}

fn current_unix_ms() -> Result<u64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock is before the Unix epoch")?;
    u64::try_from(elapsed.as_millis()).wrap_err("current Unix milliseconds exceed u64")
}

fn validate_prepared_binding(binding: &PreparedMutationBindingV1) -> Result<()> {
    if binding.schema != PREPARED_BINDING_SCHEMA_V1 {
        eyre::bail!("prepared mutation binding has an unsupported schema");
    }
    validate_sha256_argument(&binding.authorization_sha256).map_err(|error| eyre!(error))?;
    validate_authorization_nonce_argument(&binding.authorization_nonce)
        .map_err(|error| eyre!(error))?;
    validate_write_canary_phase_argument(&binding.phase).map_err(|error| eyre!(error))?;
    validate_write_canary_idempotency_key(&binding.idempotency_key)
        .map_err(|error| eyre!(error))?;
    if binding.execution_expires_at_unix_ms == 0 {
        eyre::bail!("prepared mutation binding has a zero execution expiry");
    }
    if !matches!(
        binding.kind.as_str(),
        "onboarding" | "faucet" | WRITE_CANARY_MUTATION_KIND
    ) {
        eyre::bail!("prepared mutation binding has an unsupported operation kind");
    }
    Ok(())
}

fn canonical_prepared_envelope_bytes(envelope: &PreparedMutationEnvelopeV1) -> Result<Vec<u8>> {
    let mut bytes = json::to_json(envelope)
        .wrap_err("encode canonical prepared mutation envelope")?
        .into_bytes();
    bytes.push(b'\n');
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PREPARED_ENVELOPE_MAX_BYTES {
        eyre::bail!("prepared mutation envelope exceeds its V1 byte bound");
    }
    Ok(bytes)
}

fn inherited_fd_path(fd: u32) -> Result<PathBuf> {
    if !(3..=65_535).contains(&fd) {
        eyre::bail!("prepared-envelope descriptor must be an inherited FD in 3..=65535");
    }
    if Path::new("/proc/self/fd").is_dir() {
        return Ok(PathBuf::from(format!("/proc/self/fd/{fd}")));
    }
    if Path::new("/dev/fd").is_dir() {
        return Ok(PathBuf::from(format!("/dev/fd/{fd}")));
    }
    eyre::bail!("this platform does not expose inherited descriptor paths")
}

fn write_prepared_envelope(fd: u32, bytes: &[u8]) -> Result<()> {
    let path = inherited_fd_path(fd)?;
    let mut file = File::options()
        .write(true)
        .open(path)
        .wrap_err_with(|| format!("failed to duplicate prepared-envelope output FD {fd}"))?;
    file.write_all(bytes)
        .wrap_err("failed to write canonical prepared mutation envelope")?;
    file.flush()
        .wrap_err("failed to flush canonical prepared mutation envelope")
}

fn read_prepared_envelope(fd: u32) -> Result<(PreparedMutationEnvelopeV1, Vec<u8>)> {
    let path = inherited_fd_path(fd)?;
    let file = File::open(path)
        .wrap_err_with(|| format!("failed to duplicate prepared-envelope input FD {fd}"))?;
    let mut bytes = Vec::new();
    file.take(PREPARED_ENVELOPE_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .wrap_err("failed to read prepared mutation envelope")?;
    if bytes.is_empty()
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PREPARED_ENVELOPE_MAX_BYTES
    {
        eyre::bail!("prepared mutation envelope is empty or exceeds its V1 byte bound");
    }
    let envelope: PreparedMutationEnvelopeV1 =
        json::from_slice(&bytes).wrap_err("prepared mutation envelope is not canonical V1 JSON")?;
    let canonical = canonical_prepared_envelope_bytes(&envelope)?;
    if canonical != bytes {
        eyre::bail!("prepared mutation envelope bytes are not exact canonical V1 JSON");
    }
    Ok((envelope, bytes))
}

fn load_and_validate_prepared_operation(
    config: &Config,
    args: &WriteCanary,
    public_root: &str,
    fd: u32,
    expected_fee_payment: &FeePaymentIntent,
    lifetime_check: PreparedLifetimeCheck,
) -> Result<ValidatedPreparedOperation> {
    let (envelope, envelope_bytes) = read_prepared_envelope(fd)?;
    let wire = envelope
        .operation
        .signed_transaction_wire_hex()
        .map(|wire| hex::decode(wire).wrap_err("prepared transaction wire is not hexadecimal"))
        .transpose()?;
    let binding = args.binding()?;
    let mut validated = validate_prepared_operation(
        config,
        args,
        args.operation,
        &binding,
        public_root,
        envelope,
        wire,
        expected_fee_payment,
        lifetime_check,
    )?;
    validated.envelope_bytes = envelope_bytes;
    Ok(validated)
}

fn validate_prepared_operation(
    config: &Config,
    args: &WriteCanary,
    expected_operation: WriteCanaryOperation,
    expected_binding: &PreparedMutationBindingV1,
    public_root: &str,
    envelope: PreparedMutationEnvelopeV1,
    supplied_wire: Option<Vec<u8>>,
    expected_fee_payment: &FeePaymentIntent,
    lifetime_check: PreparedLifetimeCheck,
) -> Result<ValidatedPreparedOperation> {
    expected_fee_payment
        .validate()
        .map_err(|error| eyre!("invalid requested prepared-operation fee payment: {error}"))?;
    validate_prepared_binding(&envelope.binding)?;
    if envelope.schema != PREPARED_ENVELOPE_SCHEMA_V1
        || &envelope.binding != expected_binding
        || envelope.public_root != public_root
        || envelope.chain_id != DEFAULT_CHAIN_ID
        || envelope.network_id != config.network_id.to_string()
        || envelope.authority != config.account.to_string()
    {
        eyre::bail!("prepared mutation envelope does not bind the exact CLI authorization");
    }
    let operation = &envelope.operation;
    if operation.binding() != &envelope.binding
        || operation.label() != expected_operation.label()
        || !matches!(
            (expected_operation, operation),
            (
                WriteCanaryOperation::Onboarding,
                PreparedTransactionOperationV1::OnboardingPrepared(_)
                    | PreparedTransactionOperationV1::OnboardingProofRequired(_)
            ) | (
                WriteCanaryOperation::Faucet,
                PreparedTransactionOperationV1::FaucetPrepared(_)
            ) | (
                WriteCanaryOperation::FinalCanary,
                PreparedTransactionOperationV1::FinalCanary(_)
            )
        )
    {
        eyre::bail!("prepared operation variant does not match the exact CLI operation");
    }
    let signer = CanarySigner {
        account_id: config.account.clone(),
        key_pair: config.key_pair.clone(),
    };
    let client = IrohaClient::new(write_canary_config(config, public_root, &signer)?);
    let transaction = match operation {
        PreparedTransactionOperationV1::OnboardingPrepared(prepared) => {
            let expected_alias = canary_alias(signer.key_pair.public_key());
            let expected_request = AccountOnboardingPlanRequestV1::try_new(
                expected_alias.clone(),
                &signer.account_id,
                std::iter::empty(),
            )?;
            if prepared.account_id != config.account.to_string() || prepared.alias != expected_alias
            {
                eyre::bail!(
                    "prepared onboarding target differs from the configured canary identity"
                );
            }
            Some(client.verify_account_onboarding_prepared_transaction(
                &expected_request,
                prepared,
                &prepared.receipt,
                &prepared.binding,
                expected_fee_payment,
            )?)
        }
        PreparedTransactionOperationV1::OnboardingProofRequired(proof_required) => {
            let expected_alias = canary_alias(signer.key_pair.public_key());
            let expected_request = AccountOnboardingPlanRequestV1::try_new(
                expected_alias.clone(),
                &signer.account_id,
                std::iter::empty(),
            )?;
            if proof_required.schema != PREPARED_ONBOARDING_PROOF_REQUIRED_SCHEMA_V1
                || proof_required.result.account_id != config.account.to_string()
                || proof_required.result.alias != expected_alias
                || proof_required.receipt.body.request.account_id != config.account.to_string()
                || proof_required.receipt.body.request.alias != expected_alias
            {
                eyre::bail!(
                    "proof-required onboarding target differs from the configured canary identity"
                );
            }
            client.verify_account_onboarding_proof_required_result(
                &expected_request,
                &proof_required.result,
                &proof_required.receipt,
                &proof_required.result.binding,
            )?;
            None
        }
        PreparedTransactionOperationV1::FaucetPrepared(prepared) => {
            let faucet_policy = args.faucet_policy()?;
            if prepared.account_id != config.account.to_string()
                || prepared.asset_definition_id != faucet_policy.asset_definition_id().to_string()
            {
                eyre::bail!("prepared faucet target differs from the configured canary identity");
            }
            Some(client.verify_account_faucet_prepared_transaction(
                prepared,
                &prepared.claim,
                &prepared.binding,
                expected_fee_payment,
                &faucet_policy,
            )?)
        }
        PreparedTransactionOperationV1::FinalCanary(prepared) => {
            validate_final_canary_envelope(prepared)?;
            let wire = supplied_wire
                .as_deref()
                .ok_or_else(|| eyre!("prepared final canary omits its transaction wire"))?;
            let transaction = SignedTransaction::decode_all_versioned(wire)
                .wrap_err("prepared final canary is not a versioned SignedTransaction")?;
            if transaction.authority() != &config.account {
                eyre::bail!("prepared final canary has a substituted authority");
            }
            prepared
                .fee_quote
                .validate_for_signed_payload(transaction.payload())
                .map_err(|error| {
                    eyre!("prepared final-canary fee quote is semantically invalid: {error}")
                })?;
            Some(transaction)
        }
    };
    if let Some(transaction) = transaction.as_ref() {
        validate_expected_prepared_fee_payment(
            expected_fee_payment,
            transaction.fee_payment_intent(),
        )?;
        let wire = supplied_wire
            .as_deref()
            .ok_or_else(|| eyre!("prepared transaction omits its exact wire"))?;
        validate_prepared_transaction_closure(transaction, operation, wire, &config.network_id)?;
        validate_prepared_transaction_lifetime(transaction, &envelope.binding, lifetime_check)?;
    } else if supplied_wire.is_some() {
        eyre::bail!("proof-required onboarding result must not contain transaction bytes");
    }
    Ok(ValidatedPreparedOperation {
        envelope,
        transaction,
        wire: supplied_wire,
        envelope_bytes: Vec::new(),
    })
}

fn validate_expected_prepared_fee_payment(
    expected: &FeePaymentIntent,
    actual: &FeePaymentIntent,
) -> Result<()> {
    expected
        .validate()
        .map_err(|error| eyre!("invalid requested prepared-operation fee payment: {error}"))?;
    actual
        .validate()
        .map_err(|error| eyre!("invalid signed prepared-operation fee payment: {error}"))?;
    if !expected.has_same_payer_and_gas_bound(actual) {
        eyre::bail!(
            "prepared transaction fee payer, sponsor revision, or gas bound differs from the independent CLI selection"
        );
    }
    Ok(())
}

fn validate_final_canary_envelope(operation: &FinalCanaryPreparedTransactionV1) -> Result<()> {
    if operation.schema != PREPARED_OPERATION_SCHEMA_V1
        || operation.operation != WRITE_CANARY_OPERATION
        || operation.fee_quote.intent != operation.fee_payment
    {
        eyre::bail!("prepared final-canary identity or fee closure is invalid");
    }
    Ok(())
}

fn validate_prepared_transaction_closure(
    transaction: &SignedTransaction,
    operation: &PreparedTransactionOperationV1,
    wire: &[u8],
    expected_network_id: &NetworkId,
) -> Result<()> {
    let transaction_hash_hex = operation
        .transaction_hash_hex()
        .ok_or_else(|| eyre!("prepared operation omits its transaction hash"))?;
    let wire_hex = operation
        .signed_transaction_wire_hex()
        .ok_or_else(|| eyre!("prepared operation omits its transaction wire"))?;
    let wire_sha256 = operation
        .signed_transaction_wire_sha256()
        .ok_or_else(|| eyre!("prepared operation omits its wire digest"))?;
    let fee_payment = operation
        .fee_payment()
        .ok_or_else(|| eyre!("prepared operation omits its fee intent"))?;
    if wire.len() > PREPARED_TRANSACTION_MAX_BYTES
        || wire_hex.len() > PREPARED_TRANSACTION_MAX_BYTES.saturating_mul(2)
        || hex::encode(wire) != wire_hex
        || hex::encode(Sha256::digest(wire)) != wire_sha256
    {
        eyre::bail!("prepared operation exact wire or digest is invalid");
    }
    let canonical = transaction
        .encode_wire_v1()
        .map_err(|error| eyre!("failed to re-encode prepared transaction: {error}"))?;
    if canonical != wire
        || hex::encode(transaction.hash().as_ref()) != transaction_hash_hex
        || transaction.network_id() != Some(expected_network_id)
        || transaction.fee_payment_intent() != fee_payment
    {
        eyre::bail!("prepared operation transaction bytes do not bind its declared identity");
    }
    transaction
        .verify_signature()
        .wrap_err("prepared operation transaction signature is invalid")?;
    let prepared = IrohaClient::prepare_transaction_payload(transaction);
    if prepared.as_bytes() != wire || prepared.hash() != transaction.hash() {
        eyre::bail!("prepared client payload differs from the exact signed transaction");
    }
    let binding_json = json::to_json(operation.binding())
        .wrap_err("serialize expected prepared mutation binding")?;
    let metadata = transaction.metadata();
    let binding_name = Name::from_str(PREPARED_BINDING_METADATA)?;
    if metadata
        .get(&binding_name)
        .map(IrohaJson::get)
        .map(String::as_str)
        != Some(binding_json.as_str())
    {
        eyre::bail!("prepared transaction metadata does not bind `{PREPARED_BINDING_METADATA}`");
    }
    for (key, expected) in [
        (PREPARED_OPERATION_METADATA, operation.label()),
        (PREPARED_SEMANTIC_METADATA, operation.semantic_hash_hex()),
    ] {
        let actual = metadata
            .get(&Name::from_str(key)?)
            .and_then(|value| value.try_into_any_norito::<String>().ok());
        if actual.as_deref() != Some(expected) {
            eyre::bail!("prepared transaction metadata does not bind `{key}`");
        }
    }
    match operation {
        PreparedTransactionOperationV1::FinalCanary(operation) => {
            let expected_message = prepared_canary_message(&operation.binding)?;
            let expected_semantic = prepared_semantic_sha256(
                &operation.binding,
                WRITE_CANARY_OPERATION,
                &expected_message,
            )?;
            if operation.semantic_hash_hex != expected_semantic {
                eyre::bail!("prepared final-canary semantic digest is invalid");
            }
            let idempotency_name = Name::from_str("taira_write_canary_idempotency_v1")?;
            let canary_name = Name::from_str("taira_canary")?;
            if metadata
                .get(&idempotency_name)
                .and_then(|value| value.try_into_any_norito::<String>().ok())
                .as_deref()
                != Some(operation.binding.idempotency_key.as_str())
                || metadata
                    .get(&canary_name)
                    .and_then(|value| value.try_into_any_norito::<String>().ok())
                    .as_deref()
                    != Some("write-canary")
            {
                eyre::bail!("prepared final canary omits its exact idempotency metadata");
            }
            let executable_matches = match transaction.instructions() {
                Executable::Instructions(instructions) if instructions.len() == 1 => instructions
                    .first()
                    .and_then(|instruction| instruction.as_any().downcast_ref::<Log>())
                    .is_some_and(|log| log.level == LogLevel::INFO && log.msg == expected_message),
                _ => false,
            };
            if !executable_matches || metadata.iter().len() != 5 {
                eyre::bail!("prepared final-canary instruction or metadata closure is not exact");
            }
        }
        PreparedTransactionOperationV1::OnboardingPrepared(_)
        | PreparedTransactionOperationV1::FaucetPrepared(_) => {
            if metadata.iter().len() != 3 {
                eyre::bail!("server-prepared transaction metadata closure is not exact");
            }
        }
        PreparedTransactionOperationV1::OnboardingProofRequired(_) => {
            eyre::bail!("proof-required onboarding result cannot contain a transaction")
        }
    }
    Ok(())
}

/// Authenticate one exact first-release final-canary operation without network I/O.
///
/// # Errors
/// Returns an error for noncanonical JSON, a substituted binding, wire, signature, network,
/// authority, fee closure, metadata, semantic message, instruction sequence, or lifetime.
pub(crate) fn verify_final_canary_prepared_operation_v1(
    operation_value: &Value,
    expected_network_id: &NetworkId,
    expected_authority: &AccountId,
) -> Result<SignedTransaction> {
    let operation: FinalCanaryPreparedTransactionV1 = json::from_value(operation_value.clone())
        .wrap_err("prepared final canary is not exact typed V1 JSON")?;
    if json::to_value(&operation)? != *operation_value {
        eyre::bail!("prepared final canary is outside its exact typed V1 JSON closure");
    }
    validate_final_canary_envelope(&operation)?;
    if operation.signed_transaction_wire_hex.len()
        > PREPARED_TRANSACTION_MAX_BYTES.saturating_mul(2)
    {
        eyre::bail!("prepared final-canary transaction wire is oversized");
    }
    let wire = hex::decode(&operation.signed_transaction_wire_hex)
        .wrap_err("prepared final-canary transaction wire is not hexadecimal")?;
    let transaction = SignedTransaction::decode_all_versioned(&wire)
        .wrap_err("prepared final canary is not a versioned SignedTransaction")?;
    if transaction.authority() != expected_authority {
        eyre::bail!("prepared final canary has a substituted authority");
    }
    operation
        .fee_quote
        .validate_for_signed_payload(transaction.payload())
        .map_err(|error| {
            eyre!("prepared final-canary fee quote is semantically invalid: {error}")
        })?;
    let binding = operation.binding.clone();
    let tagged = PreparedTransactionOperationV1::FinalCanary(operation);
    validate_prepared_transaction_closure(&transaction, &tagged, &wire, expected_network_id)?;
    validate_prepared_transaction_lifetime(
        &transaction,
        &binding,
        PreparedLifetimeCheck::Structural,
    )?;
    Ok(transaction)
}

fn validate_prepared_transaction_lifetime(
    transaction: &SignedTransaction,
    binding: &PreparedMutationBindingV1,
    lifetime_check: PreparedLifetimeCheck,
) -> Result<()> {
    let creation_ms = u64::try_from(transaction.creation_time().as_millis())
        .wrap_err("prepared transaction creation time exceeds u64")?;
    let ttl_ms = u64::try_from(
        transaction
            .time_to_live()
            .ok_or_else(|| eyre!("prepared transaction omits its required TTL"))?
            .as_millis(),
    )
    .wrap_err("prepared transaction TTL exceeds u64")?;
    validate_prepared_transaction_time_window(
        creation_ms,
        ttl_ms,
        binding.execution_expires_at_unix_ms,
        "prepared transaction",
    )?;
    if lifetime_check == PreparedLifetimeCheck::LiveForward {
        validate_live_prepared_transaction_freshness(
            creation_ms,
            ttl_ms,
            current_unix_ms()?,
            "prepared transaction",
        )?;
    }
    Ok(())
}

fn validate_prepared_transaction_time_window(
    creation_ms: u64,
    ttl_ms: u64,
    execution_expiry_ms: u64,
    label: &str,
) -> Result<u64> {
    let expiry_ms = creation_ms
        .checked_add(ttl_ms)
        .ok_or_else(|| eyre!("{label} lifetime overflows u64"))?;
    if ttl_ms == 0 || expiry_ms > execution_expiry_ms {
        eyre::bail!("{label} lifetime is outside the signed execution window");
    }
    Ok(expiry_ms)
}

fn validate_live_prepared_transaction_freshness(
    creation_ms: u64,
    ttl_ms: u64,
    now_ms: u64,
    label: &str,
) -> Result<()> {
    let expiry_ms = creation_ms
        .checked_add(ttl_ms)
        .ok_or_else(|| eyre!("{label} lifetime overflows u64"))?;
    if creation_ms > now_ms.saturating_add(PREPARED_TRANSACTION_CLOCK_SKEW_MS) {
        eyre::bail!("{label} was created beyond the permitted future clock skew");
    }
    if now_ms >= expiry_ms {
        eyre::bail!("{label} is already expired");
    }
    Ok(())
}

fn submit_exact_prepared_operation(
    config: &Config,
    args: &WriteCanary,
    public_root: &str,
    validated: &ValidatedPreparedOperation,
    expected_fee_payment: &FeePaymentIntent,
) -> Result<PreparedRecoveryClassification> {
    match args.operation {
        WriteCanaryOperation::Onboarding | WriteCanaryOperation::Faucet => {
            submit_server_prepared_operation(
                config,
                args,
                public_root,
                validated,
                expected_fee_payment,
            )
        }
        WriteCanaryOperation::FinalCanary => {
            let signer = CanarySigner {
                account_id: config.account.clone(),
                key_pair: config.key_pair.clone(),
            };
            let client = IrohaClient::new(write_canary_config(config, public_root, &signer)?);
            let classification = classify_exact_prepared_operation(&client, validated)?;
            if !submit_required_after_classification(&validated.envelope.binding, &classification)?
            {
                return Ok(classification);
            }
            let transaction = validated.transaction()?;
            let prepared = IrohaClient::prepare_transaction_payload(transaction);
            if prepared.as_bytes() != validated.wire()? {
                eyre::bail!("raw submit bytes differ from the retained prepared envelope");
            }
            let submitted = match client.submit_prepared_transaction_payload(&prepared) {
                Ok(submitted) => submitted,
                Err(error) => {
                    return match classify_exact_prepared_operation(&client, validated)? {
                        PreparedRecoveryClassification::Absent => Err(hint_submit_error(error)),
                        reconciled => Ok(reconciled),
                    };
                }
            };
            if submitted != transaction.hash() {
                eyre::bail!("raw submit returned a different transaction hash");
            }
            let _ = client.wait_for_transaction_applied(
                submitted,
                TransactionWaitOptions {
                    timeout: Duration::from_millis(DEFAULT_WRITE_STATUS_TIMEOUT_MS),
                    poll_interval: Duration::from_millis(500),
                },
            );
            match classify_exact_prepared_operation(&client, validated)? {
                PreparedRecoveryClassification::Absent => {
                    Ok(PreparedRecoveryClassification::Pending {
                        terminal_kind: "AcceptedNotVisible".to_owned(),
                    })
                }
                reconciled => Ok(reconciled),
            }
        }
    }
}

fn classify_exact_prepared_operation(
    client: &IrohaClient,
    validated: &ValidatedPreparedOperation,
) -> Result<PreparedRecoveryClassification> {
    if let PreparedTransactionOperationV1::OnboardingProofRequired(proof_required) =
        &validated.envelope.operation
    {
        let expected_account = AccountId::parse_encoded(&proof_required.result.account_id)
            .wrap_err("prepared proof-required onboarding account is not canonical")?;
        let alias = proof_required
            .result
            .alias
            .parse::<AccountAliasName>()
            .wrap_err("prepared proof-required onboarding alias is not canonical")?;
        let current_state =
            client.prove_account_onboarding_current_state(&expected_account, &alias)?;
        return Ok(classify_proof_required_current_state(
            &proof_required.result.semantic_hash_hex,
            current_state,
        ));
    }
    let expected_hash = validated.transaction()?.hash();
    let Some(status) = client
        .get_transaction_status_response_global(expected_hash)
        .wrap_err("read-only exact prepared-transaction status lookup failed")?
    else {
        return Ok(PreparedRecoveryClassification::Absent);
    };
    if status.hash != hex::encode(expected_hash.as_ref()) || status.scope != "global" {
        eyre::bail!("prepared-transaction status response has a mismatched identity or scope");
    }
    match status.status.kind.as_str() {
        "Applied" if prepared_recovery_status_is_final_applied(&status) => {
            let block_height = status
                .status
                .block_height
                .filter(|height| *height > 0)
                .ok_or_else(|| eyre!("Applied prepared transaction omits its block height"))?;
            let evidence =
                verify_exact_committed_prepared_operation(client, validated)?.to_string();
            Ok(PreparedRecoveryClassification::Applied {
                block_height: Some(block_height),
                evidence,
            })
        }
        "Rejected" | "Expired" if prepared_recovery_status_is_final_failure(&status) => {
            Ok(PreparedRecoveryClassification::Rejected {
                terminal_kind: status.status.kind,
            })
        }
        "Queued" | "Approved" | "Committed" | "Applied" | "Rejected" | "Expired" => {
            Ok(PreparedRecoveryClassification::Pending {
                terminal_kind: status.status.kind,
            })
        }
        other => Err(eyre!(
            "prepared-transaction status response has unsupported kind `{other}`"
        )),
    }
}

fn prepared_recovery_status_is_final_applied(status: &PipelineTransactionStatusResponse) -> bool {
    status.status.kind == "Applied" && status.scope == "global" && status.resolved_from == "state"
}

fn prepared_recovery_status_is_final_failure(status: &PipelineTransactionStatusResponse) -> bool {
    matches!(status.status.kind.as_str(), "Rejected" | "Expired")
        && status.scope == "global"
        && status.resolved_from == "state"
}

fn classify_proof_required_current_state(
    semantic_hash_hex: &str,
    current_state: AccountOnboardingCurrentStateV1,
) -> PreparedRecoveryClassification {
    match current_state {
        AccountOnboardingCurrentStateV1::Applied {
            block_height: _,
            block_hash: _,
        } => {
            PreparedRecoveryClassification::Applied {
                // The durable report is not itself a state proof: every reopen and successor
                // re-runs the atomic observation. Do not mislabel its anchor as a transaction
                // application height or replace the authenticated semantic evidence with it.
                block_height: None,
                evidence: semantic_hash_hex.to_owned(),
            }
        }
        AccountOnboardingCurrentStateV1::AliasConflict { .. } => {
            PreparedRecoveryClassification::Pending {
                terminal_kind: "OnboardingAliasConflict".to_owned(),
            }
        }
        AccountOnboardingCurrentStateV1::AliasAbsent { .. } => {
            PreparedRecoveryClassification::Pending {
                terminal_kind: "OnboardingStateAbsent".to_owned(),
            }
        }
    }
}

fn submit_required_after_classification(
    binding: &PreparedMutationBindingV1,
    classification: &PreparedRecoveryClassification,
) -> Result<bool> {
    if matches!(classification, PreparedRecoveryClassification::Absent) {
        require_forward_binding_current(binding)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn verify_exact_committed_prepared_operation(
    client: &IrohaClient,
    validated: &ValidatedPreparedOperation,
) -> Result<Hash> {
    let expected_binding = json::to_json(&validated.envelope.binding)
        .wrap_err("serialize expected committed mutation binding")?;
    let binding_name = Name::from_str(PREPARED_BINDING_METADATA)?;
    let operation_name = Name::from_str(PREPARED_OPERATION_METADATA)?;
    let expected_transaction = validated.transaction()?;
    let entrypoint_hash = expected_transaction.hash_as_entrypoint();
    let one = NonZeroU64::new(1).expect("nonzero committed lookup bound");
    let committed = client
        .query(FindTransactions::new())
        .filter(CompoundPredicate::from_filters(CommittedTxFilters {
            entry_eq: Some(entrypoint_hash),
            ..CommittedTxFilters::default()
        }))
        .with_pagination(Pagination::new(Some(one), 0))
        .with_fetch_size(FetchSize::new(Some(one)))
        .execute_all()
        .wrap_err("read-only exact prepared-transaction proof query failed")?;
    let [committed] = committed.as_slice() else {
        eyre::bail!("Applied status lacks one exact bounded committed-transaction proof");
    };
    if committed.result().is_err() {
        eyre::bail!("Applied status resolves to a failed committed transaction");
    }
    let TransactionEntrypoint::External(transaction) = committed.entrypoint() else {
        eyre::bail!("Applied status resolves to a non-external transaction entrypoint");
    };
    let binding_matches = transaction
        .metadata()
        .get(&binding_name)
        .and_then(|value| value.try_into_any_norito::<String>().ok())
        .as_deref()
        == Some(expected_binding.as_str());
    let operation_matches = transaction
        .metadata()
        .get(&operation_name)
        .and_then(|value| value.try_into_any_norito::<String>().ok())
        .as_deref()
        == Some(validated.envelope.operation.label());
    let wire = transaction
        .encode_wire_v1()
        .map_err(|error| eyre!("failed to encode committed prepared transaction: {error}"))?;
    if !binding_matches
        || !operation_matches
        || wire != validated.wire()?
        || transaction.hash() != expected_transaction.hash()
        || transaction.hash_as_entrypoint() != entrypoint_hash
    {
        eyre::bail!("committed proof differs from the exact prepared transaction");
    }
    Ok(entrypoint_hash.into())
}

fn report_prepared_classification(
    public_root: &str,
    args: &WriteCanary,
    validated: &ValidatedPreparedOperation,
    classification: PreparedRecoveryClassification,
) -> Result<Value> {
    match classification {
        PreparedRecoveryClassification::Absent => prepared_operation_report(
            public_root,
            args,
            &validated.envelope,
            &validated.envelope_bytes,
            "Pending",
            None,
            Some("Absent".to_owned()),
        ),
        PreparedRecoveryClassification::Applied {
            block_height,
            evidence,
        } => prepared_operation_report(
            public_root,
            args,
            &validated.envelope,
            &validated.envelope_bytes,
            "Applied",
            block_height,
            Some(evidence),
        ),
        PreparedRecoveryClassification::Pending { terminal_kind } => prepared_operation_report(
            public_root,
            args,
            &validated.envelope,
            &validated.envelope_bytes,
            "Pending",
            None,
            Some(terminal_kind),
        ),
        PreparedRecoveryClassification::Rejected { terminal_kind } => prepared_operation_report(
            public_root,
            args,
            &validated.envelope,
            &validated.envelope_bytes,
            "Rejected",
            None,
            Some(terminal_kind),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn prepared_operation_report(
    public_root: &str,
    args: &WriteCanary,
    envelope: &PreparedMutationEnvelopeV1,
    envelope_bytes: &[u8],
    outcome: &str,
    applied_block_height: Option<u64>,
    evidence: Option<String>,
) -> Result<Value> {
    let operation = &envelope.operation;
    let mut extra = Map::new();
    extra.insert(
        "authorization_sha256".to_owned(),
        Value::String(envelope.binding.authorization_sha256.clone()),
    );
    extra.insert(
        "authorization_nonce".to_owned(),
        Value::String(envelope.binding.authorization_nonce.clone()),
    );
    extra.insert(
        "mutation_kind".to_owned(),
        Value::String(envelope.binding.kind.clone()),
    );
    extra.insert(
        "mutation_phase".to_owned(),
        Value::String(envelope.binding.phase.clone()),
    );
    extra.insert(
        "idempotency_key".to_owned(),
        Value::String(envelope.binding.idempotency_key.clone()),
    );
    extra.insert(
        "operation".to_owned(),
        Value::String(operation.label().to_owned()),
    );
    extra.insert(
        "transaction_hash_hex".to_owned(),
        operation
            .transaction_hash_hex()
            .map(|hash| Value::String(hash.to_owned()))
            .unwrap_or(Value::Null),
    );
    extra.insert(
        "prepared_envelope_sha256".to_owned(),
        Value::String(hex::encode(Sha256::digest(envelope_bytes))),
    );
    extra.insert(
        "prepared_envelope_size".to_owned(),
        Value::from(u64::try_from(envelope_bytes.len()).expect("bounded envelope size")),
    );
    extra.insert(
        "recovery_outcome".to_owned(),
        Value::String(outcome.to_owned()),
    );
    extra.insert(
        "applied_block_height".to_owned(),
        applied_block_height.map(Value::from).unwrap_or(Value::Null),
    );
    extra.insert(
        "evidence".to_owned(),
        evidence.map(Value::String).unwrap_or(Value::Null),
    );
    extra.insert(
        "execution_expires_at_unix_ms".to_owned(),
        Value::from(envelope.binding.execution_expires_at_unix_ms),
    );
    if args.operation == WriteCanaryOperation::FinalCanary {
        let fee_payment = operation
            .fee_payment()
            .ok_or_else(|| eyre!("final canary report omits its exact fee payment"))?;
        let fee_quote = operation
            .fee_quote()
            .ok_or_else(|| eyre!("final canary report omits its exact fee quote"))?;
        extra.insert(
            "fee_payment".to_owned(),
            json::to_value(fee_payment).wrap_err("serialize exact fee payment")?,
        );
        extra.insert(
            "fee_quote".to_owned(),
            json::to_value(fee_quote).wrap_err("serialize exact fee quote")?,
        );
    }
    report_value(
        "taira_write_canary",
        "ok",
        public_root,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        extra,
    )
}

fn prepare_onboarding_operation(
    config: &Config,
    args: &WriteCanary,
    public_root: &str,
    binding: &PreparedMutationBindingV1,
    fee_payment: &FeePaymentIntent,
) -> Result<PreparedMutationEnvelopeV1> {
    let token = read_onboarding_token_file(args.require_onboarding_token()?)?;
    let signer = resolve_canary_signer(config)?;
    let canary_config = write_canary_config(config, public_root, &signer)?;
    let client = IrohaClient::new(canary_config.clone());
    let alias = canary_alias(signer.key_pair.public_key());
    let request =
        AccountOnboardingPlanRequestV1::try_new(alias, &signer.account_id, std::iter::empty())?;
    let receipt = client
        .plan_account_onboarding(&request, token.as_str())
        .wrap_err("failed to obtain an authenticated onboarding plan")?;
    let operation = match client
        .prepare_account_onboarding_transaction(
            &request,
            &receipt,
            binding,
            fee_payment,
            token.as_str(),
        )
        .wrap_err("failed to prepare exact sponsored onboarding transaction")?
    {
        AccountOnboardingPrepareResponseV1::Prepared(prepared) => {
            PreparedTransactionOperationV1::OnboardingPrepared(prepared)
        }
        AccountOnboardingPrepareResponseV1::ProofRequired(result) => {
            PreparedTransactionOperationV1::OnboardingProofRequired(
                PreparedOnboardingProofRequiredV1 {
                    schema: PREPARED_ONBOARDING_PROOF_REQUIRED_SCHEMA_V1.to_owned(),
                    receipt,
                    result,
                },
            )
        }
    };
    let wire = operation
        .signed_transaction_wire_hex()
        .map(|wire| hex::decode(wire).wrap_err("prepared onboarding wire is not hexadecimal"))
        .transpose()?;
    let envelope = PreparedMutationEnvelopeV1 {
        schema: PREPARED_ENVELOPE_SCHEMA_V1.to_owned(),
        binding: binding.clone(),
        public_root: public_root.to_owned(),
        chain_id: canary_config.chain.to_string(),
        network_id: canary_config.network_id.to_string(),
        authority: canary_config.account.to_string(),
        operation,
    };
    validate_prepared_operation(
        config,
        args,
        WriteCanaryOperation::Onboarding,
        binding,
        public_root,
        envelope.clone(),
        wire,
        fee_payment,
        PreparedLifetimeCheck::LiveForward,
    )?;
    Ok(envelope)
}

fn prepare_faucet_operation(
    config: &Config,
    args: &WriteCanary,
    public_root: &str,
    binding: &PreparedMutationBindingV1,
    fee_payment: &FeePaymentIntent,
) -> Result<PreparedMutationEnvelopeV1> {
    let signer = resolve_canary_signer(config)?;
    let canary_config = write_canary_config(config, public_root, &signer)?;
    let client = IrohaClient::new(canary_config.clone());
    let faucet_policy = args.faucet_policy()?;
    let claim =
        solve_account_faucet_claim(public_root, &signer.account_id, &canary_config.network_id)?;
    let prepared = client
        .prepare_account_faucet_transaction(&claim, binding, fee_payment, &faucet_policy)
        .wrap_err("failed to prepare exact faucet transaction")?;
    let wire = hex::decode(&prepared.signed_transaction_wire_hex)
        .wrap_err("prepared faucet wire is not hexadecimal")?;
    let envelope = PreparedMutationEnvelopeV1 {
        schema: PREPARED_ENVELOPE_SCHEMA_V1.to_owned(),
        binding: binding.clone(),
        public_root: public_root.to_owned(),
        chain_id: canary_config.chain.to_string(),
        network_id: canary_config.network_id.to_string(),
        authority: canary_config.account.to_string(),
        operation: PreparedTransactionOperationV1::FaucetPrepared(prepared),
    };
    validate_prepared_operation(
        config,
        args,
        WriteCanaryOperation::Faucet,
        binding,
        public_root,
        envelope.clone(),
        Some(wire),
        fee_payment,
        PreparedLifetimeCheck::LiveForward,
    )?;
    Ok(envelope)
}

fn submit_server_prepared_operation(
    config: &Config,
    args: &WriteCanary,
    public_root: &str,
    validated: &ValidatedPreparedOperation,
    expected_fee_payment: &FeePaymentIntent,
) -> Result<PreparedRecoveryClassification> {
    let signer = resolve_canary_signer(config)?;
    let client = IrohaClient::new(write_canary_config(config, public_root, &signer)?);
    let classification = classify_exact_prepared_operation(&client, validated)?;
    if !submit_required_after_classification(&validated.envelope.binding, &classification)? {
        return Ok(classification);
    }
    let submitted = match &validated.envelope.operation {
        PreparedTransactionOperationV1::OnboardingPrepared(prepared) => {
            let token = read_onboarding_token_file(args.require_onboarding_token()?)?;
            let request = AccountOnboardingPlanRequestV1::try_new(
                canary_alias(signer.key_pair.public_key()),
                &signer.account_id,
                std::iter::empty(),
            )?;
            client.submit_prepared_account_onboarding_transaction(
                &request,
                prepared,
                expected_fee_payment,
                token.as_str(),
            )
        }
        PreparedTransactionOperationV1::FaucetPrepared(prepared) => {
            let faucet_policy = args.faucet_policy()?;
            client.submit_prepared_account_faucet_transaction(
                prepared,
                expected_fee_payment,
                &faucet_policy,
            )
        }
        PreparedTransactionOperationV1::OnboardingProofRequired(_) => {
            eyre::bail!("a proof-required onboarding result must never be submitted")
        }
        PreparedTransactionOperationV1::FinalCanary(_) => {
            eyre::bail!("final-canary transaction reached the server-prepared submit path")
        }
    };
    let submitted = match submitted {
        Ok(submitted) => submitted,
        Err(error) => {
            return match classify_exact_prepared_operation(&client, validated)? {
                PreparedRecoveryClassification::Absent => Err(error).wrap_err(
                    "exact server-prepared transaction submission failed before observability",
                ),
                reconciled => Ok(reconciled),
            };
        }
    };
    match submitted.outcome {
        PreparedTransactionOutcomeV1::Rejected => Ok(PreparedRecoveryClassification::Rejected {
            terminal_kind: "Rejected".to_owned(),
        }),
        PreparedTransactionOutcomeV1::Applied | PreparedTransactionOutcomeV1::Pending => {
            match classify_exact_prepared_operation(&client, validated)? {
                PreparedRecoveryClassification::Absent => {
                    Ok(PreparedRecoveryClassification::Pending {
                        terminal_kind: "AcceptedNotVisible".to_owned(),
                    })
                }
                reconciled => Ok(reconciled),
            }
        }
    }
}
fn write_canary_config(
    config: &Config,
    public_root: &str,
    signer: &CanarySigner,
) -> Result<Config> {
    let mut canary_config = config.clone();
    canary_config.chain = DEFAULT_CHAIN_ID.into();
    canary_config.torii_api_url = Url::parse(&format!("{public_root}/"))
        .wrap_err_with(|| format!("invalid public root `{public_root}`"))?;
    canary_config.account = signer.account_id.clone();
    canary_config.account_chain_discriminant = DEFAULT_CHAIN_DISCRIMINANT;
    canary_config.key_pair = signer.key_pair.clone();
    canary_config.transaction_ttl = Duration::from_millis(DEFAULT_WRITE_TTL_MS);
    canary_config.transaction_status_timeout =
        Duration::from_millis(DEFAULT_WRITE_STATUS_TIMEOUT_MS);
    canary_config.transaction_add_nonce = false;
    Ok(canary_config)
}

trait ReportOutput {
    fn output_format(&self) -> CliOutputFormat;
    fn print_data(&mut self, data: &Value) -> Result<()>;
    fn println_data(&mut self, data: String) -> Result<()>;
}
struct RunContextReportOutput<'a, C: RunContext> {
    context: &'a mut C,
}
impl<C: RunContext> ReportOutput for RunContextReportOutput<'_, C> {
    fn output_format(&self) -> CliOutputFormat {
        self.context.output_format()
    }
    fn print_data(&mut self, data: &Value) -> Result<()> {
        self.context.print_data(data)
    }
    fn println_data(&mut self, data: String) -> Result<()> {
        self.context.println_data(data)
    }
}
struct WriterReportOutput<W> {
    write: W,
    output_format: CliOutputFormat,
}
impl<W: std::io::Write> ReportOutput for WriterReportOutput<W> {
    fn output_format(&self) -> CliOutputFormat {
        self.output_format
    }
    fn print_data(&mut self, data: &Value) -> Result<()> {
        let rendered = json::to_json_pretty(data).wrap_err("failed to render Taira report JSON")?;
        writeln!(self.write, "{rendered}").wrap_err("failed to write Taira report JSON")
    }
    fn println_data(&mut self, data: String) -> Result<()> {
        writeln!(self.write, "{data}").wrap_err("failed to write Taira report text")
    }
}
fn render_report<C: RunContext>(context: &mut C, json: bool, report: &Value) -> Result<()> {
    let mut output = RunContextReportOutput { context };
    render_report_to(&mut output, json, report)
}
fn render_report_to<O: ReportOutput>(output: &mut O, json: bool, report: &Value) -> Result<()> {
    if json || output.output_format() == CliOutputFormat::Json {
        output.print_data(report)
    } else {
        let object = report
            .as_object()
            .ok_or_else(|| eyre!("report must be an object"))?;
        let command = object
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("taira");
        let status = object
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let public_root = object
            .get("public_root")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_PUBLIC_ROOT);
        output.println_data(format!("{command}: {status} ({public_root})"))?;
        if let Some(checks) = object.get("checks").and_then(Value::as_array) {
            for check in checks {
                let name = check.get("name").and_then(Value::as_str).unwrap_or("check");
                let ok = check.get("ok").and_then(Value::as_bool).unwrap_or(false);
                let status_code = check
                    .get("http_status")
                    .and_then(Value::as_u64)
                    .map_or_else(|| "-".to_owned(), |value| value.to_string());
                let detail = check
                    .get("detail")
                    .and_then(Value::as_str)
                    .map(|detail| format!(" ({detail})"))
                    .unwrap_or_default();
                let marker = if ok { "ok" } else { "fail" };
                output.println_data(format!("  {marker} {name} HTTP {status_code}{detail}"))?;
            }
        }
        if let Some(warnings) = object.get("warnings").and_then(Value::as_array) {
            print_receipt_fields(output, object)?;
            for warning in warnings {
                if let Some(warning) = warning.as_str() {
                    output.println_data(format!("  warn {warning}"))?;
                }
            }
        }
        if let Some(failures) = object.get("failures").and_then(Value::as_array) {
            for failure in failures {
                if let Some(failure) = failure.as_str() {
                    output.println_data(format!("  fail {failure}"))?;
                }
            }
        }
        Ok(())
    }
}
fn report_status(report: &Value) -> Option<&str> {
    report
        .as_object()
        .and_then(|object| object.get("status"))
        .and_then(Value::as_str)
}
fn ensure_write_canary_succeeded(report: &Value) -> Result<()> {
    if report_status(report) == Some("ok") {
        return Ok(());
    }
    eyre::bail!("Taira write canary found hard failures")
}
fn print_receipt_fields<O: ReportOutput>(output: &mut O, object: &Map) -> Result<()> {
    const RECEIPT_FIELDS: &[&str] = &[
        "chain",
        "chain_discriminant",
        "account_id",
        "alias",
        "faucet_asset_id",
        "fee_payment",
        "fee_quote",
        "faucet_tx_hash",
        "ping_tx_hash",
        "applied_block_height",
        "terminal_kind",
        "tx_query_verified",
        "config_path",
    ];
    for field in RECEIPT_FIELDS {
        let Some(value) = object
            .get(*field)
            .filter(|value| !matches!(value, Value::Null))
        else {
            continue;
        };
        output.println_data(format!("  {field}: {}", display_json_scalar(value)))?;
    }
    Ok(())
}
fn display_json_scalar(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        _ => compact_json(value),
    }
}
fn report_value(
    command: &str,
    status: &str,
    public_root: &str,
    checks: Vec<Value>,
    warnings: Vec<String>,
    failures: Vec<String>,
    extra: Map,
) -> Result<Value> {
    let mut root = Map::new();
    root.insert("command".into(), Value::String(command.to_owned()));
    root.insert("status".into(), Value::String(status.to_owned()));
    root.insert("public_root".into(), Value::String(public_root.to_owned()));
    root.insert("checks".into(), Value::Array(checks));
    root.insert(
        "warnings".into(),
        Value::Array(warnings.into_iter().map(Value::String).collect()),
    );
    root.insert(
        "failures".into(),
        Value::Array(failures.into_iter().map(Value::String).collect()),
    );
    for (key, value) in extra {
        root.insert(key, value);
    }
    Ok(Value::Object(root))
}
fn push_check(
    checks: &mut Vec<Value>,
    name: &str,
    http_status: u16,
    ok: bool,
    detail: Option<String>,
) {
    let mut object = Map::new();
    object.insert("name".into(), Value::String(name.to_owned()));
    object.insert("http_status".into(), Value::from(u64::from(http_status)));
    object.insert("ok".into(), Value::from(ok));
    if let Some(detail) = detail {
        object.insert("detail".into(), Value::String(detail));
    }
    checks.push(Value::Object(object));
}
fn route_check_detail(expected_statuses: &[u16]) -> Option<String> {
    (expected_statuses.len() == 1 && expected_statuses[0] != 200).then(|| {
        format!(
            "mounted route is expected to return HTTP {} for this preflight shape",
            expected_statuses[0]
        )
    })
}
fn normalize_root_url(raw: &str) -> Result<String> {
    if raw.is_empty() {
        eyre::bail!("public root URL must not be empty");
    }
    if raw != raw.trim() {
        eyre::bail!("public root URL must not contain surrounding whitespace");
    }
    let parsed = Url::parse(raw).wrap_err_with(|| format!("invalid URL `{raw}`"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => eyre::bail!("unsupported URL scheme `{other}`"),
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        eyre::bail!("public root URL must not contain credentials");
    }
    if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
        eyre::bail!("public root URL must be an origin without a path, query, or fragment");
    }
    let canonical = parsed.origin().ascii_serialization();
    if raw != canonical {
        eyre::bail!("public root URL must use the canonical origin spelling `{canonical}`");
    }
    Ok(canonical)
}
fn join_url(root: &str, path: &str) -> Result<Url> {
    let root = format!("{}/", root.trim_end_matches('/'));
    let suffix = path.trim_start_matches('/');
    Url::parse(&root)
        .and_then(|url| url.join(suffix))
        .wrap_err_with(|| format!("failed to build URL from `{root}` and `{path}`"))
}
fn validate_onboarding_token(token: &str) -> Result<&str> {
    let bytes = token.as_bytes();
    if !(32..=256).contains(&bytes.len()) || !bytes.iter().all(|byte| (0x21..=0x7e).contains(byte))
    {
        eyre::bail!(
            "account onboarding token must contain 32..256 printable ASCII bytes without spaces or normalization"
        );
    }
    Ok(token)
}
#[cfg(unix)]
fn validate_onboarding_token_file_metadata(metadata: &fs::Metadata) -> Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    if metadata.uid() != rustix::process::geteuid().as_raw() {
        eyre::bail!("account onboarding token file must be owned by the current user");
    }
    let mode = metadata.permissions().mode();
    if mode & 0o077 != 0 {
        eyre::bail!("account onboarding token file must not grant group or other access");
    }
    if mode & 0o400 == 0 {
        eyre::bail!("account onboarding token file must be owner-readable");
    }
    Ok(())
}
#[cfg(not(unix))]
fn validate_onboarding_token_file_metadata(_metadata: &fs::Metadata) -> Result<()> {
    Ok(())
}
fn read_onboarding_token_file(path: &Path) -> Result<Zeroizing<String>> {
    let before =
        fs::symlink_metadata(path).wrap_err("failed to inspect account onboarding token file")?;
    if before.file_type().is_symlink() || !before.file_type().is_file() {
        eyre::bail!("account onboarding token file must be a regular non-symlink file");
    }
    validate_onboarding_token_file_metadata(&before)?;
    #[cfg(unix)]
    let mut file = {
        let descriptor = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )
        .wrap_err("failed to securely open account onboarding token file")?;
        fs::File::from(descriptor)
    };
    #[cfg(not(unix))]
    let mut file = fs::OpenOptions::new()
        .read(true)
        .open(path)
        .wrap_err("failed to open account onboarding token file")?;
    let opened = file
        .metadata()
        .wrap_err("failed to inspect opened account onboarding token file")?;
    if !opened.file_type().is_file() {
        eyre::bail!("account onboarding token file must remain a regular file");
    }
    validate_onboarding_token_file_metadata(&opened)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if opened.dev() != before.dev() || opened.ino() != before.ino() {
            eyre::bail!("account onboarding token file changed during secure open");
        }
    }
    let mut raw = Zeroizing::new(Vec::with_capacity(257));
    std::io::Read::by_ref(&mut file)
        .take(257)
        .read_to_end(&mut raw)
        .wrap_err("failed to read account onboarding token file")?;
    if raw.len() > 256 {
        eyre::bail!("account onboarding token file exceeds the maximum credential length");
    }
    let token = std::str::from_utf8(&raw)
        .map_err(|_| eyre!("account onboarding token file must contain printable ASCII"))?;
    validate_onboarding_token(token)?;
    Ok(Zeroizing::new(token.to_owned()))
}
fn http_client() -> Result<HttpClient> {
    HttpClient::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("iroha-taira-devex/1")
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .wrap_err("failed to build HTTP client")
}
fn http_json(
    http: &HttpClient,
    method: reqwest::Method,
    url: &str,
    body: Option<&Value>,
) -> Result<HttpJson> {
    let mut request = http
        .request(method, url)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(body) = body {
        let bytes = json::to_vec(body).map_err(|err| eyre!("encode JSON request body: {err}"))?;
        request = request
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(bytes);
    }
    let response = request
        .send()
        .wrap_err_with(|| format!("request failed for {url}"))?;
    decode_http_json_response(response)
}
fn account_signed_soracloud_status(client: &IrohaClient) -> Result<HttpJson> {
    let response = client
        .get_soracloud_status_response()
        .wrap_err("canonical account-signed Soracloud status request failed")?;
    let status = response.status().as_u16();
    let text = String::from_utf8(response.body().to_vec())
        .wrap_err("canonical account-signed Soracloud status response is not UTF-8")?;
    let body = if text.trim().is_empty() {
        None
    } else {
        json::from_str::<Value>(&text).ok()
    };
    Ok(HttpJson { status, body })
}
fn decode_http_json_response(response: reqwest::blocking::Response) -> Result<HttpJson> {
    let status = response.status().as_u16();
    let text = response
        .text()
        .wrap_err("failed to read Taira HTTP response body")?;
    let parsed = if text.trim().is_empty() {
        None
    } else {
        json::from_str::<Value>(&text).ok()
    };
    Ok(HttpJson {
        status,
        body: parsed,
    })
}
fn collect_status_warnings(status: Option<&Value>, warnings: &mut Vec<String>) {
    let Some(status) = status else {
        warnings.push("/status returned a non-JSON body".to_owned());
        return;
    };
    if value_path_bool(status, &["sumeragi", "tx_queue_saturated"]).unwrap_or(false) {
        warnings.push("public transaction queue reports saturation".to_owned());
    }
    if let Some(rejected) = value_path_u64(status, &["txs_rejected_recent_5m"])
        && rejected > 0
    {
        warnings.push(format!(
            "recent rejected transactions in /status: {rejected}"
        ));
    }
    if let Some(queue) = value_path_u64(status, &["queue_size"])
        && queue > 0
    {
        warnings.push(format!("public queue has {queue} pending transaction(s)"));
    }
}
fn value_path<'a>(mut value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    for key in path {
        value = value.as_object()?.get(*key)?;
    }
    Some(value)
}
fn value_path_u64(value: &Value, path: &[&str]) -> Option<u64> {
    value_path(value, path).and_then(Value::as_u64)
}
fn value_path_bool(value: &Value, path: &[&str]) -> Option<bool> {
    value_path(value, path).and_then(Value::as_bool)
}
fn validate_public_status(status: Option<&Value>) -> Result<(), String> {
    let status = status
        .and_then(Value::as_object)
        .ok_or_else(|| "/status returned a non-object JSON body".to_owned())?;
    if status.contains_key("tx_queue_saturated") {
        return Err(
            "/status contains retired root `tx_queue_saturated`; expected `sumeragi.tx_queue_saturated`"
                .to_owned(),
        );
    }
    status
        .get("sumeragi")
        .and_then(Value::as_object)
        .and_then(|sumeragi| sumeragi.get("tx_queue_saturated"))
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            "/status field `sumeragi.tx_queue_saturated` must be a boolean".to_owned()
        })?;
    Ok(())
}
fn validate_time_snapshot(snapshot: Option<&Value>) -> Result<(), String> {
    let snapshot = snapshot
        .and_then(Value::as_object)
        .ok_or_else(|| "/v1/time/now returned a non-object JSON body".to_owned())?;
    let positive_u64 = |field: &str| {
        snapshot
            .get(field)
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .ok_or_else(|| format!("/v1/time/now field `{field}` must be a positive integer"))
    };
    positive_u64("now")?;
    positive_u64("sample_count")?;
    positive_u64("peer_count")?;
    snapshot
        .get("confidence_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "/v1/time/now field `confidence_ms` must be a nonnegative integer".to_owned()
        })?;
    snapshot
        .get("offset_ms")
        .and_then(Value::as_i64)
        .ok_or_else(|| "/v1/time/now field `offset_ms` must be an integer".to_owned())?;
    if snapshot.get("enforcement_mode").and_then(Value::as_str) != Some("reject") {
        return Err("/v1/time/now is not using fail-closed time enforcement".to_owned());
    }
    if snapshot.get("fallback").and_then(Value::as_bool) != Some(false) {
        return Err("/v1/time/now is using the local-clock fallback".to_owned());
    }
    for field in ["healthy", "min_samples_ok", "offset_ok", "confidence_ok"] {
        if snapshot
            .get("health")
            .and_then(Value::as_object)
            .and_then(|health| health.get(field))
            .and_then(Value::as_bool)
            != Some(true)
        {
            return Err(format!("/v1/time/now health field `{field}` is not true"));
        }
    }
    Ok(())
}
fn tagged_enum_name<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    let object = value.as_object()?;
    if object.len() != 2 || !object.get("value").is_some_and(Value::is_null) {
        return None;
    }
    object.get(field)?.as_str()
}
fn exact_soracloud_status_object<'a>(
    value: &'a Value,
    context: &str,
    fields: &[&str],
) -> Result<&'a Map, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    if let Some(field) = fields.iter().find(|field| !object.contains_key(**field)) {
        return Err(format!("{context} is missing required field `{field}`"));
    }
    if let Some(field) = object
        .keys()
        .find(|field| !fields.contains(&field.as_str()))
    {
        return Err(format!("{context} contains unknown field `{field}`"));
    }
    Ok(object)
}
fn soracloud_status_field<'a>(
    object: &'a Map,
    field: &str,
    context: &str,
) -> Result<&'a Value, String> {
    object
        .get(field)
        .ok_or_else(|| format!("{context} is missing required field `{field}`"))
}
fn soracloud_status_u64(object: &Map, field: &str, context: &str) -> Result<u64, String> {
    soracloud_status_field(object, field, context)?
        .as_u64()
        .ok_or_else(|| format!("{context}.{field} must be a nonnegative integer"))
}
fn soracloud_status_bool(object: &Map, field: &str, context: &str) -> Result<bool, String> {
    soracloud_status_field(object, field, context)?
        .as_bool()
        .ok_or_else(|| format!("{context}.{field} must be a boolean"))
}
fn soracloud_status_string<'a>(
    object: &'a Map,
    field: &str,
    context: &str,
) -> Result<&'a str, String> {
    soracloud_status_field(object, field, context)?
        .as_str()
        .ok_or_else(|| format!("{context}.{field} must be a string"))
}
fn validate_soracloud_nullable_string(
    object: &Map,
    field: &str,
    context: &str,
) -> Result<(), String> {
    let value = soracloud_status_field(object, field, context)?;
    if value.is_null() || value.is_string() {
        Ok(())
    } else {
        Err(format!("{context}.{field} must be a string or null"))
    }
}
fn validate_soracloud_string_array(object: &Map, field: &str, context: &str) -> Result<(), String> {
    let values = soracloud_status_field(object, field, context)?
        .as_array()
        .ok_or_else(|| format!("{context}.{field} must be an array"))?;
    if values.iter().all(Value::is_string) {
        Ok(())
    } else {
        Err(format!("{context}.{field} must contain only strings"))
    }
}
fn validate_soracloud_tagged_unit<'a>(
    value: &'a Value,
    tag: &str,
    variants: &[&str],
    context: &str,
) -> Result<&'a str, String> {
    let object = exact_soracloud_status_object(value, context, &[tag, "value"])?;
    if !soracloud_status_field(object, "value", context)?.is_null() {
        return Err(format!("{context}.value must be null"));
    }
    let variant = soracloud_status_string(object, tag, context)?;
    if variants.contains(&variant) {
        Ok(variant)
    } else {
        Err(format!("{context}.{tag} has unknown variant `{variant}`"))
    }
}
fn validate_soracloud_hash(value: &Value, context: &str) -> Result<(), String> {
    let raw = value
        .as_str()
        .ok_or_else(|| format!("{context} must be a canonical hash string"))?;
    let hash = json::from_value::<Hash>(value.clone())
        .map_err(|_| format!("{context} must be a canonical hash string"))?;
    if hash.to_string() != raw {
        return Err(format!("{context} must use exact canonical hash text"));
    }
    Ok(())
}
fn validate_soracloud_nullable_hash(
    object: &Map,
    field: &str,
    context: &str,
) -> Result<(), String> {
    let value = soracloud_status_field(object, field, context)?;
    if value.is_null() {
        Ok(())
    } else {
        validate_soracloud_hash(value, &format!("{context}.{field}"))
    }
}
fn validate_soracloud_u128(value: &Value, context: &str) -> Result<(), String> {
    let literal =
        json::to_string(value).map_err(|_| format!("{context} must be an unsigned integer"))?;
    let parsed = literal
        .parse::<u128>()
        .map_err(|_| format!("{context} must be an unsigned integer"))?;
    if parsed.to_string() != literal {
        return Err(format!(
            "{context} must use exact canonical unsigned integer text"
        ));
    }
    Ok(())
}
fn validate_canonical_soracloud_route_host(host: &str, context: &str) -> Result<(), String> {
    if host.is_empty()
        || host.chars().any(char::is_whitespace)
        || host.chars().any(char::is_control)
    {
        return Err(format!(
            "{context} must be an exact nonempty host without whitespace"
        ));
    }
    if let Ok(address) = host.parse::<std::net::IpAddr>() {
        if address.to_string() == host {
            return Ok(());
        }
        return Err(format!("{context} must use canonical IP-literal spelling"));
    }
    if host
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return Err(format!("{context} must use canonical IPv4 spelling"));
    }
    if !host.is_ascii()
        || host.len() > 253
        || host.bytes().any(|byte| byte.is_ascii_uppercase())
        || host.starts_with('.')
        || host.ends_with('.')
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
    {
        return Err(format!(
            "{context} must be a lowercase canonical DNS host or canonical IP literal"
        ));
    }
    Ok(())
}
fn validate_canonical_soracloud_route_prefix(prefix: &str, context: &str) -> Result<(), String> {
    let Some(suffix) = prefix.strip_prefix('/') else {
        return Err(format!(
            "{context} must be an exact canonical absolute route prefix"
        ));
    };
    if prefix.is_empty()
        || prefix.contains('\\')
        || prefix.contains('?')
        || prefix.contains('#')
        || prefix.chars().any(char::is_whitespace)
        || prefix.chars().any(char::is_control)
        || (prefix != "/"
            && (prefix.ends_with('/')
                || suffix
                    .split('/')
                    .any(|component| component.is_empty() || matches!(component, "." | ".."))))
    {
        return Err(format!(
            "{context} must be an exact canonical absolute route prefix"
        ));
    }
    Ok(())
}
fn validate_soracloud_service_health(value: &Value) -> Result<&str, String> {
    const FULL_FIELDS: &[&str] = &[
        "mode",
        "status",
        "message",
        "observed_height",
        "observed_block_hash",
        "state_dir",
        "service_revisions",
        "healthy_service_revisions",
        "hydrating_service_revisions",
        "degraded_service_revisions",
        "unavailable_service_revisions",
        "apartments",
        "running_apartments",
        "expired_apartments",
    ];
    const UNAVAILABLE_FIELDS: &[&str] = &["mode", "status", "message"];
    let context = "/v1/soracloud/status.service_health";
    let unvalidated = value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    let status = unvalidated
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{context}.status must be a string"))?;
    let fields = if unvalidated.len() == UNAVAILABLE_FIELDS.len() {
        UNAVAILABLE_FIELDS
    } else {
        FULL_FIELDS
    };
    let object = exact_soracloud_status_object(value, context, fields)?;
    if soracloud_status_string(object, "mode", context)? != "embedded_runtime_manager" {
        return Err(format!(
            "{context}.mode is not the canonical V1 runtime mode"
        ));
    }
    soracloud_status_string(object, "message", context)?;
    if fields == UNAVAILABLE_FIELDS {
        if status != "unavailable" {
            return Err(format!(
                "{context} compact form is reserved for unavailable runtime"
            ));
        }
        return Ok(status);
    }
    if !["idle", "healthy", "degraded", "unavailable"].contains(&status) {
        return Err(format!("{context}.status has unknown value `{status}`"));
    }
    for field in [
        "observed_height",
        "service_revisions",
        "healthy_service_revisions",
        "hydrating_service_revisions",
        "degraded_service_revisions",
        "unavailable_service_revisions",
        "apartments",
        "running_apartments",
        "expired_apartments",
    ] {
        soracloud_status_u64(object, field, context)?;
    }
    validate_soracloud_nullable_hash(object, "observed_block_hash", context)?;
    soracloud_status_string(object, "state_dir", context)?;
    Ok(status)
}
fn validate_soracloud_routing(value: &Value) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "configured_lane_count",
        "declared_lane_count",
        "active_lane_count",
        "active_lane_ids",
        "autoscale_capacity_lane_count",
        "autoscale_capacity_lane_ids",
        "dataspace_count",
        "routing_rules",
        "default_lane_id",
        "default_dataspace_id",
    ];
    let context = "/v1/soracloud/status.routing";
    let object = exact_soracloud_status_object(value, context, FIELDS)?;
    for field in [
        "configured_lane_count",
        "declared_lane_count",
        "active_lane_count",
        "autoscale_capacity_lane_count",
        "dataspace_count",
        "routing_rules",
        "default_lane_id",
        "default_dataspace_id",
    ] {
        soracloud_status_u64(object, field, context)?;
    }
    for (count_field, ids_field) in [
        ("active_lane_count", "active_lane_ids"),
        (
            "autoscale_capacity_lane_count",
            "autoscale_capacity_lane_ids",
        ),
    ] {
        let ids = soracloud_status_field(object, ids_field, context)?
            .as_array()
            .ok_or_else(|| format!("{context}.{ids_field} must be an array"))?;
        if !ids.iter().all(|value| value.as_u64().is_some()) {
            return Err(format!(
                "{context}.{ids_field} must contain only nonnegative integers"
            ));
        }
        if soracloud_status_u64(object, count_field, context)?
            != u64::try_from(ids.len()).unwrap_or(u64::MAX)
        {
            return Err(format!(
                "{context}.{count_field} does not match {ids_field}"
            ));
        }
    }
    Ok(())
}
fn validate_soracloud_topology(value: &Value) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "active_capability_adverts",
        "placed_host_count",
        "hosted_replica_count",
        "unavailable_replica_count",
    ];
    let context = "/v1/soracloud/status.hosted_http_topology";
    let object = exact_soracloud_status_object(value, context, FIELDS)?;
    for field in FIELDS {
        soracloud_status_u64(object, field, context)?;
    }
    Ok(())
}
fn validate_soracloud_runtime_pressure(value: &Value) -> Result<(), String> {
    const FULL_FIELDS: &[&str] = &[
        "enabled",
        "state_dir",
        "observed_height",
        "service_revisions",
        "apartments",
        "max_load_factor_bps",
        "reported_pending_mailbox_messages",
        "authoritative_pending_mailbox_messages",
        "bundle_cache_misses",
        "artifact_cache_misses",
    ];
    let context = "/v1/soracloud/status.resource_pressure.runtime";
    let unvalidated = value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    let enabled = unvalidated
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{context}.enabled must be a boolean"))?;
    if !enabled {
        exact_soracloud_status_object(value, context, &["enabled"])?;
        return Ok(());
    }
    let object = exact_soracloud_status_object(value, context, FULL_FIELDS)?;
    soracloud_status_string(object, "state_dir", context)?;
    for field in &FULL_FIELDS[2..] {
        soracloud_status_u64(object, field, context)?;
    }
    Ok(())
}
fn validate_soracloud_resource_pressure(value: &Value) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "queue_active",
        "queue_queued",
        "queue_capacity",
        "queue_saturated",
        "high_load_threshold",
        "high_load",
        "runtime",
    ];
    let context = "/v1/soracloud/status.resource_pressure";
    let object = exact_soracloud_status_object(value, context, FIELDS)?;
    for field in [
        "queue_active",
        "queue_queued",
        "queue_capacity",
        "high_load_threshold",
    ] {
        soracloud_status_u64(object, field, context)?;
    }
    soracloud_status_bool(object, "queue_saturated", context)?;
    soracloud_status_bool(object, "high_load", context)?;
    validate_soracloud_runtime_pressure(soracloud_status_field(object, "runtime", context)?)
}
fn validate_soracloud_failed_admissions(value: &Value) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "available",
        "total",
        "governance_manifest_rejected",
        "sorafs_provider_rejected",
    ];
    let context = "/v1/soracloud/status.failed_admissions";
    let object = exact_soracloud_status_object(value, context, FIELDS)?;
    soracloud_status_bool(object, "available", context)?;
    for field in &FIELDS[1..] {
        soracloud_status_u64(object, field, context)?;
    }
    Ok(())
}
fn validate_soracloud_runtime_snapshot(value: &Value) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "schema_version",
        "observed_height",
        "observed_block_hash",
        "local_peer_id",
        "services",
        "apartments",
    ];
    let context = "/v1/soracloud/status.runtime_manager.snapshot";
    let object = exact_soracloud_status_object(value, context, FIELDS)?;
    if soracloud_status_u64(object, "schema_version", context)? != 1 {
        return Err(format!("{context}.schema_version must equal 1"));
    }
    soracloud_status_u64(object, "observed_height", context)?;
    validate_soracloud_nullable_hash(object, "observed_block_hash", context)?;
    validate_soracloud_nullable_string(object, "local_peer_id", context)?;
    for field in ["services", "apartments"] {
        if !soracloud_status_field(object, field, context)?.is_object() {
            return Err(format!("{context}.{field} must be an object"));
        }
    }
    Ok(())
}
fn validate_soracloud_runtime_manager(value: &Value) -> Result<bool, String> {
    let context = "/v1/soracloud/status.runtime_manager";
    let unvalidated = value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    let available = unvalidated
        .get("available")
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{context}.available must be a boolean"))?;
    if !available {
        exact_soracloud_status_object(value, context, &["available"])?;
        return Ok(false);
    }
    let object =
        exact_soracloud_status_object(value, context, &["available", "state_dir", "snapshot"])?;
    soracloud_status_string(object, "state_dir", context)?;
    validate_soracloud_runtime_snapshot(soracloud_status_field(object, "snapshot", context)?)?;
    Ok(true)
}
fn validate_soracloud_rollout(value: &Value, context: &str) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "rollout_handle",
        "baseline_version",
        "candidate_version",
        "canary_percent",
        "traffic_percent",
        "stage",
        "health_failures",
        "max_health_failures",
        "health_window_secs",
        "created_sequence",
        "updated_sequence",
    ];
    let object = exact_soracloud_status_object(value, context, FIELDS)?;
    soracloud_status_string(object, "rollout_handle", context)?;
    validate_soracloud_nullable_string(object, "baseline_version", context)?;
    soracloud_status_string(object, "candidate_version", context)?;
    for field in [
        "canary_percent",
        "traffic_percent",
        "health_failures",
        "max_health_failures",
        "health_window_secs",
        "created_sequence",
        "updated_sequence",
    ] {
        soracloud_status_u64(object, field, context)?;
    }
    if soracloud_status_u64(object, "canary_percent", context)? > 100
        || soracloud_status_u64(object, "traffic_percent", context)? > 100
    {
        return Err(format!(
            "{context} rollout percentages must be within 0..=100"
        ));
    }
    validate_soracloud_tagged_unit(
        soracloud_status_field(object, "stage", context)?,
        "stage",
        &["Canary", "Promoted", "RolledBack"],
        &format!("{context}.stage"),
    )?;
    Ok(())
}
fn validate_soracloud_network_policy(value: &Value, context: &str) -> Result<(), String> {
    let object = exact_soracloud_status_object(value, context, &["mode", "value"])?;
    let mode = soracloud_status_string(object, "mode", context)?;
    let payload = soracloud_status_field(object, "value", context)?;
    match mode {
        "Open" | "Isolated" if payload.is_null() => Ok(()),
        "Allowlist" if payload.is_array() => Ok(()),
        "Open" | "Isolated" => Err(format!("{context}.value must be null for `{mode}`")),
        "Allowlist" => Err(format!("{context}.value must be an array for `Allowlist`")),
        _ => Err(format!("{context}.mode has unknown variant `{mode}`")),
    }
}
#[expect(
    clippy::too_many_lines,
    reason = "the V1 status revision inventory is intentionally validated in one exact pass"
)]
fn validate_soracloud_revision(value: &Value, context: &str) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "sequence",
        "action",
        "service_version",
        "service_manifest_hash",
        "container_manifest_hash",
        "replicas",
        "execution_plane",
        "route_host",
        "route_path_prefix",
        "base_url",
        "healthcheck_url",
        "public_discovery_content_cid",
        "public_discovery_url",
        "public_discovery_cid_host_url",
        "state_binding_count",
        "state_bindings",
        "lease_volumes",
        "allow_model_inference",
        "allow_model_training",
        "runtime",
        "allow_state_writes",
        "network",
        "cpu_millis",
        "memory_bytes",
        "ephemeral_storage_bytes",
        "max_open_files_per_process",
        "max_tasks",
        "start_grace_secs",
        "stop_grace_secs",
        "healthcheck_path",
        "required_config_names",
        "required_secret_names",
        "config_exports",
        "sandbox_profile_hash",
        "process_generation",
        "process_started_sequence",
        "signed_by",
    ];
    let object = exact_soracloud_status_object(value, context, FIELDS)?;
    soracloud_status_u64(object, "sequence", context)?;
    validate_soracloud_tagged_unit(
        soracloud_status_field(object, "action", context)?,
        "action",
        &[
            "Deploy",
            "Upgrade",
            "Rollback",
            "ConfigMutation",
            "SecretMutation",
            "StateMutation",
            "FheJobRun",
            "FhePolicyRegister",
            "FhePolicyRotate",
            "FhePolicyRevoke",
            "DecryptionRequest",
            "CiphertextQuery",
            "Rollout",
            "LeaseReportingEpochRollover",
        ],
        &format!("{context}.action"),
    )?;
    soracloud_status_string(object, "service_version", context)?;
    for field in [
        "service_manifest_hash",
        "container_manifest_hash",
        "sandbox_profile_hash",
    ] {
        validate_soracloud_hash(
            soracloud_status_field(object, field, context)?,
            &format!("{context}.{field}"),
        )?;
    }
    for field in [
        "replicas",
        "state_binding_count",
        "cpu_millis",
        "memory_bytes",
        "ephemeral_storage_bytes",
        "max_open_files_per_process",
        "max_tasks",
        "start_grace_secs",
        "stop_grace_secs",
        "process_generation",
        "process_started_sequence",
    ] {
        soracloud_status_u64(object, field, context)?;
    }
    validate_soracloud_tagged_unit(
        soracloud_status_field(object, "execution_plane", context)?,
        "execution_plane",
        &["DeterministicService", "HttpService"],
        &format!("{context}.execution_plane"),
    )?;
    let route_host = soracloud_status_field(object, "route_host", context)?;
    if !route_host.is_null() {
        validate_canonical_soracloud_route_host(
            route_host
                .as_str()
                .ok_or_else(|| format!("{context}.route_host must be a string or null"))?,
            &format!("{context}.route_host"),
        )?;
    }
    let route_prefix = soracloud_status_field(object, "route_path_prefix", context)?;
    if !route_prefix.is_null() {
        validate_canonical_soracloud_route_prefix(
            route_prefix
                .as_str()
                .ok_or_else(|| format!("{context}.route_path_prefix must be a string or null"))?,
            &format!("{context}.route_path_prefix"),
        )?;
    }
    for field in [
        "base_url",
        "healthcheck_url",
        "public_discovery_content_cid",
        "public_discovery_url",
        "public_discovery_cid_host_url",
        "healthcheck_path",
    ] {
        validate_soracloud_nullable_string(object, field, context)?;
    }
    for field in ["state_bindings", "lease_volumes", "config_exports"] {
        if !soracloud_status_field(object, field, context)?.is_array() {
            return Err(format!("{context}.{field} must be an array"));
        }
    }
    if soracloud_status_u64(object, "state_binding_count", context)?
        != u64::try_from(
            soracloud_status_field(object, "state_bindings", context)?
                .as_array()
                .expect("state_bindings was validated as an array")
                .len(),
        )
        .unwrap_or(u64::MAX)
    {
        return Err(format!(
            "{context}.state_binding_count does not match state_bindings"
        ));
    }
    for field in [
        "allow_model_inference",
        "allow_model_training",
        "allow_state_writes",
    ] {
        soracloud_status_bool(object, field, context)?;
    }
    validate_soracloud_tagged_unit(
        soracloud_status_field(object, "runtime", context)?,
        "runtime",
        &["Ivm", "Inrou"],
        &format!("{context}.runtime"),
    )?;
    validate_soracloud_network_policy(
        soracloud_status_field(object, "network", context)?,
        &format!("{context}.network"),
    )?;
    validate_soracloud_string_array(object, "required_config_names", context)?;
    validate_soracloud_string_array(object, "required_secret_names", context)?;
    soracloud_status_string(object, "signed_by", context)?;
    Ok(())
}
fn validate_soracloud_lease_epoch_rollover(value: &Value, context: &str) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "schema_version",
        "economic_clock",
        "lease_started_height",
        "previous_reporting_epoch",
        "new_reporting_epoch",
        "reporter_account_id",
        "active_service_version",
        "replica_slot",
        "placement_incarnation",
        "finalized_checkpoint_count",
        "settled_egress_bytes_delta",
        "settled_egress_bytes",
    ];
    let object = exact_soracloud_status_object(value, context, FIELDS)?;
    if soracloud_status_u64(object, "schema_version", context)? != 1 {
        return Err(format!("{context}.schema_version must equal 1"));
    }
    validate_soracloud_tagged_unit(
        soracloud_status_field(object, "economic_clock", context)?,
        "clock",
        &["CanonicalBlockHeight"],
        &format!("{context}.economic_clock"),
    )?;
    for field in [
        "lease_started_height",
        "previous_reporting_epoch",
        "new_reporting_epoch",
        "replica_slot",
        "finalized_checkpoint_count",
    ] {
        soracloud_status_u64(object, field, context)?;
    }
    soracloud_status_string(object, "reporter_account_id", context)?;
    soracloud_status_string(object, "active_service_version", context)?;
    validate_soracloud_hash(
        soracloud_status_field(object, "placement_incarnation", context)?,
        &format!("{context}.placement_incarnation"),
    )?;
    for field in ["settled_egress_bytes_delta", "settled_egress_bytes"] {
        validate_soracloud_u128(
            soracloud_status_field(object, field, context)?,
            &format!("{context}.{field}"),
        )?;
    }
    Ok(())
}
fn validate_soracloud_audit_event(value: &Value, context: &str) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "sequence",
        "action",
        "service_name",
        "from_version",
        "to_version",
        "service_manifest_hash",
        "container_manifest_hash",
        "binding_name",
        "state_key",
        "config_name",
        "secret_name",
        "governance_tx_hash",
        "rollout_handle",
        "policy_name",
        "policy_snapshot_hash",
        "jurisdiction_tag",
        "consent_evidence_hash",
        "break_glass",
        "break_glass_reason",
        "lease_reporting_epoch_rollover",
        "signed_by",
    ];
    let object = exact_soracloud_status_object(value, context, FIELDS)?;
    soracloud_status_u64(object, "sequence", context)?;
    validate_soracloud_tagged_unit(
        soracloud_status_field(object, "action", context)?,
        "action",
        &[
            "Deploy",
            "Upgrade",
            "Rollback",
            "ConfigMutation",
            "SecretMutation",
            "StateMutation",
            "FheJobRun",
            "FhePolicyRegister",
            "FhePolicyRotate",
            "FhePolicyRevoke",
            "DecryptionRequest",
            "CiphertextQuery",
            "Rollout",
            "LeaseReportingEpochRollover",
        ],
        &format!("{context}.action"),
    )?;
    for field in ["service_name", "to_version", "signed_by"] {
        soracloud_status_string(object, field, context)?;
    }
    for field in ["service_manifest_hash", "container_manifest_hash"] {
        validate_soracloud_hash(
            soracloud_status_field(object, field, context)?,
            &format!("{context}.{field}"),
        )?;
    }
    for field in [
        "from_version",
        "binding_name",
        "state_key",
        "config_name",
        "secret_name",
        "rollout_handle",
        "policy_name",
        "jurisdiction_tag",
        "break_glass_reason",
    ] {
        validate_soracloud_nullable_string(object, field, context)?;
    }
    for field in [
        "governance_tx_hash",
        "policy_snapshot_hash",
        "consent_evidence_hash",
    ] {
        validate_soracloud_nullable_hash(object, field, context)?;
    }
    let break_glass = soracloud_status_field(object, "break_glass", context)?;
    if !break_glass.is_null() && !break_glass.is_bool() {
        return Err(format!("{context}.break_glass must be a boolean or null"));
    }
    let rollover = soracloud_status_field(object, "lease_reporting_epoch_rollover", context)?;
    if !rollover.is_null() {
        validate_soracloud_lease_epoch_rollover(
            rollover,
            &format!("{context}.lease_reporting_epoch_rollover"),
        )?;
    }
    Ok(())
}
fn validate_soracloud_service(value: &Value, index: usize) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "service_name",
        "current_version",
        "revision_count",
        "config_generation",
        "secret_generation",
        "config_entry_count",
        "secret_entry_count",
        "quota_class",
        "service_lease_status",
        "lease_expires_height",
        "prepaid_runtime_balance",
        "remaining_runtime_balance",
        "public_discovery_content_cid",
        "public_discovery_url",
        "public_discovery_cid_host_url",
        "latest_revision",
        "active_rollout",
        "last_rollout",
    ];
    let context = format!("/v1/soracloud/status.control_plane.services[{index}]");
    let object = exact_soracloud_status_object(value, &context, FIELDS)?;
    soracloud_status_string(object, "service_name", &context)?;
    soracloud_status_string(object, "current_version", &context)?;
    for field in [
        "revision_count",
        "config_generation",
        "secret_generation",
        "config_entry_count",
        "secret_entry_count",
    ] {
        soracloud_status_u64(object, field, &context)?;
    }
    validate_soracloud_nullable_string(object, "quota_class", &context)?;
    let lease_status = soracloud_status_field(object, "service_lease_status", &context)?;
    if !lease_status.is_null() {
        validate_soracloud_tagged_unit(
            lease_status,
            "status",
            &["Active", "Expired", "Exhausted", "Suspended"],
            &format!("{context}.service_lease_status"),
        )?;
    }
    let lease_height = soracloud_status_field(object, "lease_expires_height", &context)?;
    if !lease_height.is_null() && lease_height.as_u64().is_none() {
        return Err(format!(
            "{context}.lease_expires_height must be a nonnegative integer or null"
        ));
    }
    for field in ["prepaid_runtime_balance", "remaining_runtime_balance"] {
        let value = soracloud_status_field(object, field, &context)?;
        if !value.is_null() && !value.is_string() {
            return Err(format!("{context}.{field} must be a string or null"));
        }
    }
    for field in [
        "public_discovery_content_cid",
        "public_discovery_url",
        "public_discovery_cid_host_url",
    ] {
        validate_soracloud_nullable_string(object, field, &context)?;
    }
    let revision = soracloud_status_field(object, "latest_revision", &context)?;
    if !revision.is_null() {
        validate_soracloud_revision(revision, &format!("{context}.latest_revision"))?;
    }
    for field in ["active_rollout", "last_rollout"] {
        let rollout = soracloud_status_field(object, field, &context)?;
        if !rollout.is_null() {
            validate_soracloud_rollout(rollout, &format!("{context}.{field}"))?;
        }
    }
    Ok(())
}
fn validate_soracloud_control_plane(value: &Value) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "schema_version",
        "service_count",
        "audit_event_count",
        "services",
        "recent_audit_events",
    ];
    let context = "/v1/soracloud/status.control_plane";
    let object = exact_soracloud_status_object(value, context, FIELDS)?;
    if soracloud_status_u64(object, "schema_version", context)? != 1 {
        return Err(format!("{context}.schema_version must equal 1"));
    }
    let services = soracloud_status_field(object, "services", context)?
        .as_array()
        .ok_or_else(|| format!("{context}.services must be an array"))?;
    if soracloud_status_u64(object, "service_count", context)?
        != u64::try_from(services.len()).unwrap_or(u64::MAX)
    {
        return Err(format!("{context}.service_count does not match services"));
    }
    for (index, service) in services.iter().enumerate() {
        validate_soracloud_service(service, index)?;
    }
    let audit_events = soracloud_status_field(object, "recent_audit_events", context)?
        .as_array()
        .ok_or_else(|| format!("{context}.recent_audit_events must be an array"))?;
    if soracloud_status_u64(object, "audit_event_count", context)?
        < u64::try_from(audit_events.len()).unwrap_or(u64::MAX)
    {
        return Err(format!(
            "{context}.audit_event_count is smaller than recent_audit_events"
        ));
    }
    for (index, event) in audit_events.iter().enumerate() {
        validate_soracloud_audit_event(event, &format!("{context}.recent_audit_events[{index}]"))?;
    }
    Ok(())
}
#[expect(
    clippy::too_many_lines,
    reason = "the public V1 status validator keeps the complete fail-closed contract visible"
)]
fn validate_soracloud_status(status: Option<&Value>) -> Result<(), String> {
    const ROOT_FIELDS: &[&str] = &[
        "schema_version",
        "service_health",
        "routing",
        "hosted_http_topology",
        "resource_pressure",
        "failed_admissions",
        "runtime_manager",
        "control_plane",
    ];
    let status = status.ok_or_else(|| "/v1/soracloud/status returned no JSON body".to_owned())?;
    let status = exact_soracloud_status_object(status, "/v1/soracloud/status", ROOT_FIELDS)?;
    if soracloud_status_u64(status, "schema_version", "/v1/soracloud/status")? != 1 {
        return Err("/v1/soracloud/status is not canonical schema version 1".to_owned());
    }
    let health_status = validate_soracloud_service_health(soracloud_status_field(
        status,
        "service_health",
        "/v1/soracloud/status",
    )?)?;
    match health_status {
        "healthy" | "idle" => {}
        other => {
            return Err(format!(
                "/v1/soracloud/status runtime health is `{other}`, expected healthy or idle"
            ));
        }
    }
    validate_soracloud_routing(soracloud_status_field(
        status,
        "routing",
        "/v1/soracloud/status",
    )?)?;
    validate_soracloud_topology(soracloud_status_field(
        status,
        "hosted_http_topology",
        "/v1/soracloud/status",
    )?)?;
    validate_soracloud_resource_pressure(soracloud_status_field(
        status,
        "resource_pressure",
        "/v1/soracloud/status",
    )?)?;
    validate_soracloud_failed_admissions(soracloud_status_field(
        status,
        "failed_admissions",
        "/v1/soracloud/status",
    )?)?;
    if !validate_soracloud_runtime_manager(soracloud_status_field(
        status,
        "runtime_manager",
        "/v1/soracloud/status",
    )?)? {
        return Err("/v1/soracloud/status reports no runtime manager".to_owned());
    }
    validate_soracloud_control_plane(soracloud_status_field(
        status,
        "control_plane",
        "/v1/soracloud/status",
    )?)?;
    let topology = status
        .get("hosted_http_topology")
        .and_then(Value::as_object)
        .ok_or_else(|| "/v1/soracloud/status is missing `hosted_http_topology`".to_owned())?;
    let active_adverts = topology
        .get("active_capability_adverts")
        .and_then(Value::as_u64)
        .ok_or_else(|| "/v1/soracloud/status is missing `active_capability_adverts`".to_owned())?;
    if active_adverts < 4 {
        return Err(format!(
            "/v1/soracloud/status reports {active_adverts} active Inrou host advert(s); expected at least 4"
        ));
    }
    let hosted_replicas = topology
        .get("hosted_replica_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| "/v1/soracloud/status is missing `hosted_replica_count`".to_owned())?;
    if hosted_replicas < 4 {
        return Err(format!(
            "/v1/soracloud/status reports {hosted_replicas} hosted Inrou replica placement(s); expected at least 4"
        ));
    }
    let services = status
        .get("control_plane")
        .and_then(Value::as_object)
        .and_then(|control_plane| control_plane.get("services"))
        .and_then(Value::as_array)
        .ok_or_else(|| "/v1/soracloud/status is missing control-plane services".to_owned())?;
    let has_four_replica_public_inrou_route = services.iter().any(|service| {
        let Some(revision) = service
            .as_object()
            .and_then(|service| service.get("latest_revision"))
            .and_then(Value::as_object)
        else {
            return false;
        };
        revision.get("replicas").and_then(Value::as_u64) == Some(4)
            && revision
                .get("runtime")
                .and_then(|runtime| tagged_enum_name(runtime, "runtime"))
                == Some("Inrou")
            && revision
                .get("execution_plane")
                .and_then(|plane| tagged_enum_name(plane, "execution_plane"))
                == Some("HttpService")
            && revision.get("route_host").and_then(Value::as_str).is_some()
            && revision
                .get("route_path_prefix")
                .and_then(Value::as_str)
                .is_some()
    });
    if !has_four_replica_public_inrou_route {
        return Err(
            "/v1/soracloud/status has no canonical four-replica public HttpService/Inrou route"
                .to_owned(),
        );
    }
    Ok(())
}
fn validate_mcp_protocol_response(payload: Option<&Value>, json_rpc: bool) -> Result<(), String> {
    let payload = payload.ok_or_else(|| "MCP response body is not canonical JSON".to_owned())?;
    let capabilities = if json_rpc {
        if payload.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || payload.get("id").and_then(Value::as_u64) != Some(1)
        {
            return Err("MCP initialize response has a substituted JSON-RPC envelope".to_owned());
        }
        payload
            .get("result")
            .ok_or_else(|| "MCP initialize response omits result".to_owned())?
    } else {
        payload
    };
    if capabilities.get("protocolVersion").and_then(Value::as_str) != Some(MCP_PROTOCOL_VERSION) {
        return Err(format!(
            "MCP response protocolVersion must equal `{MCP_PROTOCOL_VERSION}`"
        ));
    }
    Ok(())
}

fn mcp_tool_names(payload: Option<&Value>) -> Result<Vec<String>, String> {
    let payload = payload.ok_or_else(|| "MCP tools/list body is not canonical JSON".to_owned())?;
    if payload.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || payload.get("id").and_then(Value::as_u64) != Some(1)
    {
        return Err("MCP tools/list response has a substituted JSON-RPC envelope".to_owned());
    }
    let tools = value_path(payload, &["result", "tools"])
        .and_then(Value::as_array)
        .ok_or_else(|| "MCP tools/list response omits its exact tools array".to_owned())?;
    let mut names = Vec::with_capacity(tools.len());
    let mut unique = std::collections::BTreeSet::new();
    for (index, tool) in tools.iter().enumerate() {
        let object = tool
            .as_object()
            .ok_or_else(|| format!("MCP tool {index} is not an object"))?;
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("MCP tool {index} omits a string name"))?;
        if !name.starts_with("iroha.")
            || name.len() > 128
            || name.ends_with('.')
            || name.contains("..")
            || !name.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_')
            })
        {
            return Err(format!(
                "MCP tool {index} has noncanonical Taira name `{name}`"
            ));
        }
        if !unique.insert(name) {
            return Err(format!("MCP tools/list duplicates `{name}`"));
        }
        names.push(name.to_owned());
    }
    Ok(names)
}
fn resolve_canary_signer(config: &Config) -> Result<CanarySigner> {
    let key_pair = config.key_pair.clone();
    let (algorithm, _) = key_pair
        .public_key()
        .try_to_bytes()
        .wrap_err("Taira canary signer public key is malformed")?;
    if algorithm != Algorithm::Ed25519 {
        eyre::bail!("Taira canary signer must use Ed25519");
    }
    let account_id = AccountId::new(key_pair.public_key().clone());
    Ok(CanarySigner {
        account_id,
        key_pair,
    })
}
pub(super) fn canary_alias(public_key: &iroha_crypto::PublicKey) -> String {
    let digest = Sha256::digest(public_key.to_string().as_bytes());
    let suffix = hex::encode(&digest[..8]);
    format!("{CANARY_ALIAS_PREFIX}{suffix}@universal")
}

fn solve_account_faucet_claim(
    public_root: &str,
    account_id: &AccountId,
    expected_network_id: &NetworkId,
) -> Result<AccountFaucetClaimV1> {
    let http = http_client()?;
    let puzzle_url = join_url(public_root, "/v1/accounts/faucet/puzzle")?;
    let puzzle = http_json(&http, reqwest::Method::GET, puzzle_url.as_str(), None)?;
    if puzzle.status != 200 {
        eyre::bail!(
            "faucet puzzle request failed with HTTP {}; no transaction was prepared",
            puzzle.status
        );
    }
    let puzzle = puzzle
        .body
        .as_ref()
        .ok_or_else(|| eyre!("faucet puzzle response was not canonical JSON"))?;
    let claim = solve_faucet_puzzle(&account_id.to_string(), expected_network_id, puzzle)?;
    json::from_value(claim).wrap_err("decode solved faucet claim into its closed V1 schema")
}

fn solve_faucet_puzzle(
    account_id: &str,
    expected_network_id: &NetworkId,
    puzzle: &Value,
) -> Result<Value> {
    validate_exact_faucet_puzzle_shape(puzzle)?;
    let algorithm = required_str(puzzle, "algorithm")?;
    if algorithm != FAUCET_POW_ALGORITHM {
        eyre::bail!(
            "unsupported faucet puzzle algorithm `{algorithm}`; expected `{FAUCET_POW_ALGORITHM}`"
        );
    }
    let network_id = validate_taira_puzzle_identity(puzzle, expected_network_id)?;
    let difficulty_bits = required_u64(puzzle, "difficulty_bits")?;
    if difficulty_bits == 0 {
        eyre::bail!("faucet puzzle difficulty_bits must be positive");
    }
    let mut body = Map::new();
    body.insert("account_id".into(), Value::String(account_id.to_owned()));
    let anchor_height = required_u64(puzzle, "anchor_height")?;
    if anchor_height == 0 {
        eyre::bail!("faucet puzzle anchor_height must be positive");
    }
    let anchor_hash_hex = required_str(puzzle, "anchor_block_hash_hex")?;
    let challenge_salt_hex = required_nullable_str(puzzle, "challenge_salt_hex")?;
    let log_n = u8::try_from(required_u64(puzzle, "scrypt_log_n")?)
        .map_err(|_| eyre!("faucet puzzle scrypt_log_n is too large"))?;
    let r = u32::try_from(required_u64(puzzle, "scrypt_r")?)
        .map_err(|_| eyre!("faucet puzzle scrypt_r is too large"))?;
    let p = u32::try_from(required_u64(puzzle, "scrypt_p")?)
        .map_err(|_| eyre!("faucet puzzle scrypt_p is too large"))?;
    if required_u64(puzzle, "max_anchor_age_blocks")? == 0 {
        eyre::bail!("faucet puzzle max_anchor_age_blocks must be positive");
    }
    let challenge = build_faucet_challenge(
        account_id,
        &network_id,
        anchor_height,
        anchor_hash_hex,
        challenge_salt_hex,
    )?;
    let params = ScryptParams::new(log_n, r, p, 32)
        .map_err(|err| eyre!("invalid faucet scrypt parameters: {err}"))?;
    let difficulty_bits =
        u32::try_from(difficulty_bits).map_err(|_| eyre!("faucet difficulty is too large"))?;
    let nonce = solve_faucet_pow(&challenge, &params, difficulty_bits)?;
    body.insert("pow_anchor_height".into(), Value::from(anchor_height));
    body.insert("pow_nonce_hex".into(), Value::String(hex::encode(nonce)));
    Ok(Value::Object(body))
}
const FAUCET_PUZZLE_V1_FIELDS: [&str; 11] = [
    "algorithm",
    "network_id",
    "chain_discriminant",
    "difficulty_bits",
    "anchor_height",
    "anchor_block_hash_hex",
    "challenge_salt_hex",
    "scrypt_log_n",
    "scrypt_r",
    "scrypt_p",
    "max_anchor_age_blocks",
];
fn validate_exact_faucet_puzzle_shape(puzzle: &Value) -> Result<()> {
    let object = puzzle
        .as_object()
        .ok_or_else(|| eyre!("faucet puzzle response must be an exact V1 object"))?;
    if object.len() != FAUCET_PUZZLE_V1_FIELDS.len()
        || FAUCET_PUZZLE_V1_FIELDS
            .iter()
            .any(|field| !object.contains_key(*field))
    {
        eyre::bail!("faucet puzzle response violates the exact V1 field set");
    }
    Ok(())
}
fn validate_taira_puzzle_identity(
    puzzle: &Value,
    expected_network_id: &NetworkId,
) -> Result<NetworkId> {
    let network_id_literal = required_str(puzzle, "network_id")?;
    let network_id = network_id_literal
        .parse::<NetworkId>()
        .wrap_err("faucet puzzle network_id is not a canonical NetworkId")?;
    if network_id.to_string() != network_id_literal {
        eyre::bail!("faucet puzzle network_id is not canonically encoded");
    }
    if &network_id != expected_network_id {
        eyre::bail!(
            "faucet puzzle network_id `{network_id}` does not match configured network `{expected_network_id}`"
        );
    }
    let chain_discriminant = u16::try_from(required_u64(puzzle, "chain_discriminant")?)
        .map_err(|_| eyre!("faucet puzzle chain_discriminant is too large"))?;
    if chain_discriminant != DEFAULT_CHAIN_DISCRIMINANT {
        eyre::bail!(
            "faucet puzzle chain_discriminant `{chain_discriminant}` does not match Taira `{DEFAULT_CHAIN_DISCRIMINANT}`"
        );
    }
    Ok(network_id)
}
fn required_u64(value: &Value, key: &str) -> Result<u64> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("faucet puzzle missing numeric `{key}`"))
}
fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("faucet puzzle missing string `{key}`"))
}
fn required_nullable_str<'a>(value: &'a Value, key: &str) -> Result<Option<&'a str>> {
    let field = value
        .as_object()
        .and_then(|obj| obj.get(key))
        .ok_or_else(|| eyre!("faucet puzzle missing nullable string `{key}`"))?;
    if field.is_null() {
        return Ok(None);
    }
    field
        .as_str()
        .map(Some)
        .ok_or_else(|| eyre!("faucet puzzle `{key}` must be a string or null"))
}
fn build_faucet_challenge(
    account_id: &str,
    network_id: &NetworkId,
    anchor_height: u64,
    anchor_hash_hex: &str,
    challenge_salt_hex: Option<&str>,
) -> Result<[u8; 32]> {
    // This Torii field is explicitly raw lowercase hex, not the marked `Hash` display form.
    let anchor_hash = decode_exact_lower_hex(anchor_hash_hex, "anchor_block_hash_hex", 32)?;
    let mut hasher = Sha256::new();
    hasher.update(FAUCET_POW_DOMAIN_SEPARATOR);
    hasher.update(network_id.as_bytes());
    hasher.update(account_id.as_bytes());
    hasher.update(anchor_height.to_be_bytes());
    hasher.update(anchor_hash);
    if let Some(salt) = challenge_salt_hex {
        let salt = decode_exact_lower_hex(salt, "challenge_salt_hex", 32)?;
        hasher.update(salt);
    }
    Ok(hasher.finalize().into())
}
fn decode_exact_lower_hex(value: &str, field: &str, byte_length: usize) -> Result<Vec<u8>> {
    if value.len() != byte_length.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        eyre::bail!(
            "faucet puzzle {field} must be an exact lowercase {byte_length}-byte hex string"
        );
    }
    hex::decode(value).wrap_err_with(|| format!("invalid faucet puzzle {field}"))
}
fn solve_faucet_pow(
    challenge: &[u8; 32],
    params: &ScryptParams,
    difficulty_bits: u32,
) -> Result<[u8; 8]> {
    for nonce in 0_u64..(1_u64 << 63) {
        let nonce_bytes = nonce.to_be_bytes();
        let mut digest = [0_u8; 32];
        derive_scrypt(&nonce_bytes, challenge, params, &mut digest)
            .map_err(|err| eyre!("failed faucet scrypt derivation: {err}"))?;
        if leading_zero_bits(&digest) >= difficulty_bits {
            return Ok(nonce_bytes);
        }
    }
    eyre::bail!("faucet PoW nonce space exhausted")
}
fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut total = 0_u32;
    for byte in bytes {
        if *byte == 0 {
            total += 8;
            continue;
        }
        total += byte.leading_zeros();
        break;
    }
    total
}
fn insert_string_metadata(metadata: &mut Metadata, key: &str, value: &str) -> Result<()> {
    metadata.insert(Name::from_str(key)?, IrohaJson::new(value.to_owned()));
    Ok(())
}

fn validate_write_canary_idempotency_key(value: &str) -> Result<String, String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("must be exactly 64 lowercase hexadecimal characters".to_owned());
    }
    Ok(value.to_owned())
}

fn hint_submit_error(err: eyre::Report) -> eyre::Report {
    let text = format!("{err:#}");
    if text.contains("fee intent") || text.contains("fee_payment") {
        eyre!(
            "{text}\nTaira requires the exact signature-bound `FeePaymentIntent` returned by `/v1/fees/quote`; re-quote after any payload or sponsor-program revision change."
        )
    } else if text.contains("route_unavailable") || text.contains("ROUTE_UNRESOLVED") {
        eyre!(
            "{text}\nTaira accepted the request at ingress but no authoritative peer accepted the route; treat this as ingress or lane routing first."
        )
    } else if text.contains("Failed to find asset") {
        eyre!(
            "{text}\nThe canary signer appears unfunded for the fee asset; re-run the command so the faucet path can claim starter funds."
        )
    } else {
        err
    }
}
fn compact_json(value: &Value) -> String {
    json::to_json(value).unwrap_or_else(|_| format!("{value:?}"))
}
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;
    use iroha_i18n::{Bundle, Language, Localizer};
    use std::{
        net::{TcpListener, TcpStream},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    };
    use tempfile::NamedTempFile;
    const TEST_ONBOARDING_TOKEN: &str = "0123456789abcdef0123456789ABCDEF";

    #[test]
    fn prepared_transaction_lifetime_separates_recovery_from_forward_freshness() {
        validate_prepared_transaction_time_window(1_000, 200, 1_300, "fixture")
            .expect("immutable transaction window fits inside its execution lease");
        validate_live_prepared_transaction_freshness(1_000, 200, 1_100, "fixture")
            .expect("fresh transaction remains eligible for a forward effect");

        for (creation, ttl, expiry) in [
            (1_000, 0, 1_300),
            (1_000, 301, 1_300),
            (u64::MAX, 1, u64::MAX),
        ] {
            validate_prepared_transaction_time_window(creation, ttl, expiry, "fixture")
                .expect_err("invalid immutable prepared-transaction window must fail closed");
        }

        validate_live_prepared_transaction_freshness(1_000, 200, 1_200, "fixture")
            .expect_err("an expired transaction must not create a new forward effect");
        validate_live_prepared_transaction_freshness(
            1_000 + PREPARED_TRANSACTION_CLOCK_SKEW_MS + 1,
            200,
            1_000,
            "fixture",
        )
        .expect_err("a future-dated transaction must not create a new forward effect");

        validate_prepared_transaction_time_window(1_000, 200, 1_300, "fixture")
            .expect("durable recovery keeps accepting an expired but immutable valid window");
    }

    #[test]
    fn prepared_fee_payment_requires_the_independent_cli_selection() {
        let selected = FeePaymentIntent::authority(Vec::new(), None);
        validate_expected_prepared_fee_payment(&selected, &selected)
            .expect("the exact selected payer and gas bound are accepted");

        let substituted_gas = FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(1));
        validate_expected_prepared_fee_payment(&selected, &substituted_gas)
            .expect_err("a substituted gas bound must fail closed");
    }

    #[test]
    fn faucet_child_requires_one_exact_independent_policy() {
        let args = fixture_write_canary_args(WriteCanaryOperation::Faucet);
        let policy = args.faucet_policy().expect("canonical faucet policy");
        assert_eq!(
            policy.asset_definition_id().to_string(),
            DEFAULT_GAS_ASSET_ID
        );
        assert_eq!(policy.amount(), &Quantity::from(25_000_u32));

        let mut missing_authority = fixture_write_canary_args(WriteCanaryOperation::Faucet);
        missing_authority.faucet_authority = None;
        missing_authority
            .faucet_policy()
            .expect_err("missing faucet authority must fail closed");

        let mut alias_asset = fixture_write_canary_args(WriteCanaryOperation::Faucet);
        alias_asset.faucet_asset_id = Some("xor#universal".to_owned());
        alias_asset
            .faucet_policy()
            .expect_err("an asset alias must not replace the exact canonical asset definition");

        let mut zero_amount = fixture_write_canary_args(WriteCanaryOperation::Faucet);
        zero_amount.faucet_amount = Some("0".to_owned());
        zero_amount
            .faucet_policy()
            .expect_err("zero faucet amount must fail closed");
    }

    #[derive(clap::Parser, Debug)]
    struct TestTairaCli {
        #[command(subcommand)]
        command: Command,
    }

    #[test]
    fn write_canary_parser_accepts_only_one_exact_child_action() {
        let nonce = "n".repeat(32);
        let phase = "pre_edge";
        let key = write_canary_child_idempotency_key(&nonce, phase, "onboarding");
        let parsed = TestTairaCli::try_parse_from([
            "taira-test".to_owned(),
            "write-canary".to_owned(),
            "--operation".to_owned(),
            "onboarding".to_owned(),
            "--authorization-sha256".to_owned(),
            "ab".repeat(32),
            "--authorization-nonce".to_owned(),
            nonce,
            "--mutation-phase".to_owned(),
            phase.to_owned(),
            "--idempotency-key".to_owned(),
            key,
            "--execution-expires-at-unix-ms".to_owned(),
            u64::MAX.to_string(),
            "--prepare-envelope".to_owned(),
            "--prepared-output-fd".to_owned(),
            "3".to_owned(),
            "--onboarding-token-file".to_owned(),
            "/private/runtime/token".to_owned(),
        ])
        .expect("one-operation prepared mode must parse");
        let Command::WriteCanary(command) = parsed.command else {
            panic!("expected write-canary command");
        };
        assert_eq!(command.operation, WriteCanaryOperation::Onboarding);
        assert_eq!(
            command.prepared_action().expect("prepared action"),
            PreparedEnvelopeAction::Prepare(3)
        );

        for retired in [
            "--recover-only",
            "--write-config",
            "--alias-prefix",
            "--use-config-signer",
        ] {
            let error = TestTairaCli::try_parse_from(["taira-test", "write-canary", retired])
                .expect_err("retired one-shot flags must fail closed");
            assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
        }
        let error = TestTairaCli::try_parse_from(["taira-test", "write-canary"])
            .expect_err("aggregate operation and implicit action must not parse");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn inrou_canary_parser_accepts_only_one_exact_child_action() {
        let nonce = "n".repeat(32);
        let phase = "pre_edge";
        let key = write_canary_child_idempotency_key(&nonce, phase, "inrou_bundle_pin");
        let parsed = TestTairaCli::try_parse_from([
            "taira-test".to_owned(),
            "inrou-canary".to_owned(),
            "--stage-dir".to_owned(),
            "/private/runtime/inrou-stage".to_owned(),
            "--mode".to_owned(),
            "deploy".to_owned(),
            "--operation".to_owned(),
            "bundle-pin".to_owned(),
            "--authorization-sha256".to_owned(),
            "ab".repeat(32),
            "--authorization-nonce".to_owned(),
            nonce.clone(),
            "--mutation-phase".to_owned(),
            phase.to_owned(),
            "--idempotency-key".to_owned(),
            key,
            "--execution-expires-at-unix-ms".to_owned(),
            u64::MAX.to_string(),
            "--prepare-envelope".to_owned(),
            "--prepared-output-fd".to_owned(),
            "3".to_owned(),
            "--prerequisite-envelope-fd".to_owned(),
            "4".to_owned(),
        ])
        .expect("one-operation Inrou prepare mode must parse");
        let Command::InrouCanary(command) = parsed.command else {
            panic!("expected inrou-canary command");
        };
        assert_eq!(command.operation, InrouCanaryOperation::BundlePin);
        assert_eq!(
            command.prepared_action().expect("prepared action"),
            PreparedEnvelopeAction::Prepare(3)
        );
        command.binding().expect("exact child key must bind");

        let error = TestTairaCli::try_parse_from(["taira-test", "inrou-canary", "--recover-only"])
            .expect_err("retired aggregate recovery flag must fail closed");
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn inrou_canary_binding_and_prerequisite_are_child_exact() {
        let nonce = "n".repeat(32);
        let mut args = InrouCanary {
            public_root: DEFAULT_PUBLIC_ROOT.to_owned(),
            stage_dir: PathBuf::from("/private/runtime/inrou-stage"),
            mode: InrouCanaryMode::Deploy,
            operation: InrouCanaryOperation::GuestPin,
            authorization_sha256: "ab".repeat(32),
            authorization_nonce: nonce.clone(),
            mutation_phase: "pre_edge".to_owned(),
            idempotency_key: write_canary_child_idempotency_key(
                &nonce,
                "pre_edge",
                "inrou_guest_pin",
            ),
            execution_expires_at_unix_ms: u64::MAX,
            prepare_envelope: true,
            prepared_output_fd: Some(3),
            submit_prepared_envelope_fd: None,
            recover_prepared_envelope_fd: None,
            prerequisite_envelope_fd: Some(4),
            timeout_secs: 1,
            json: true,
        };
        assert!(args.binding().is_ok());
        assert!(
            args.validate_prerequisite_action(PreparedEnvelopeAction::Prepare(3))
                .is_ok()
        );
        args.idempotency_key =
            write_canary_child_idempotency_key(&nonce, "pre_edge", "inrou_bundle_pin");
        assert!(args.binding().is_err());
        args.prerequisite_envelope_fd = None;
        assert!(
            args.validate_prerequisite_action(PreparedEnvelopeAction::Prepare(3))
                .is_err()
        );
        args.prerequisite_envelope_fd = Some(4);
        assert!(
            args.validate_prerequisite_action(PreparedEnvelopeAction::Recover(3))
                .is_err()
        );
    }

    #[test]
    fn write_canary_prerequisite_policy_is_exact() {
        let mut args = fixture_write_canary_args(WriteCanaryOperation::Onboarding);
        assert!(
            args.validate_prerequisite_action(PreparedEnvelopeAction::Prepare(3))
                .is_ok()
        );
        args.prerequisite_envelope_fd = Some(4);
        assert!(
            args.validate_prerequisite_action(PreparedEnvelopeAction::Prepare(3))
                .is_err()
        );
        args.operation = WriteCanaryOperation::Faucet;
        args.idempotency_key = write_canary_child_idempotency_key(
            &args.authorization_nonce,
            &args.mutation_phase,
            args.operation.mutation_kind(),
        );
        assert!(
            args.validate_prerequisite_action(PreparedEnvelopeAction::Prepare(3))
                .is_ok()
        );
        args.prerequisite_envelope_fd = None;
        assert!(
            args.validate_prerequisite_action(PreparedEnvelopeAction::Prepare(3))
                .is_err()
        );
        args.prerequisite_envelope_fd = Some(4);
        assert!(
            args.validate_prerequisite_action(PreparedEnvelopeAction::Recover(3))
                .is_err()
        );
    }

    #[test]
    fn authenticated_no_op_preparation_reports_proof_required() {
        let semantic_evidence = "ab".repeat(32);
        let (outcome, evidence) =
            initial_prepared_report_state_from_evidence(Some(&semantic_evidence));
        assert_eq!(outcome, "ProofRequired");
        assert_eq!(evidence.as_deref(), Some(semantic_evidence.as_str()));
        assert_eq!(
            initial_prepared_report_state_from_evidence(None),
            ("Prepared", None)
        );
    }

    #[test]
    fn proof_required_applied_state_maps_without_transaction_height() {
        let semantic_evidence = "ab".repeat(32);
        let classification = classify_proof_required_current_state(
            &semantic_evidence,
            AccountOnboardingCurrentStateV1::Applied {
                block_height: NonZeroU64::new(41).expect("nonzero fixture height"),
                block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"proof-required current-state fixture anchor",
                )),
            },
        );
        let PreparedRecoveryClassification::Applied {
            block_height,
            evidence,
        } = classification
        else {
            panic!("applied proof-required current state must classify as applied");
        };
        assert_eq!(block_height, None);
        assert_eq!(evidence, semantic_evidence);
    }

    #[test]
    fn proof_required_mismatch_states_remain_nonterminal() {
        let block_height = NonZeroU64::new(41).expect("nonzero fixture height");
        let block_hash = iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
            b"proof-required mismatch fixture anchor",
        ));
        for (state, expected_kind) in [
            (
                AccountOnboardingCurrentStateV1::AliasAbsent {
                    block_height,
                    block_hash,
                },
                "OnboardingStateAbsent",
            ),
            (
                AccountOnboardingCurrentStateV1::AliasConflict {
                    block_height,
                    block_hash,
                },
                "OnboardingAliasConflict",
            ),
        ] {
            assert_eq!(
                classify_proof_required_current_state(&"ab".repeat(32), state),
                PreparedRecoveryClassification::Pending {
                    terminal_kind: expected_kind.to_owned(),
                }
            );
        }
    }

    #[test]
    fn prepared_recovery_accepts_only_global_state_finality() {
        let response = |kind: &str, scope: &str, resolved_from: &str| {
            PipelineTransactionStatusResponse::new(
                format!("{}b", "a".repeat(63)),
                iroha_torii_shared::PipelineTransactionStatus {
                    kind: kind.to_owned(),
                    block_height: Some(7),
                },
                scope.to_owned(),
                resolved_from.to_owned(),
            )
        };
        assert!(prepared_recovery_status_is_final_applied(&response(
            "Applied", "global", "state"
        )));
        for (scope, resolved_from) in [("global", "cache"), ("global", "queue"), ("local", "state")]
        {
            assert!(!prepared_recovery_status_is_final_applied(&response(
                "Applied",
                scope,
                resolved_from
            )));
        }
        for kind in ["Rejected", "Expired"] {
            assert!(prepared_recovery_status_is_final_failure(&response(
                kind, "global", "state"
            )));
            for (scope, resolved_from) in
                [("global", "cache"), ("global", "queue"), ("local", "state")]
            {
                assert!(!prepared_recovery_status_is_final_failure(&response(
                    kind,
                    scope,
                    resolved_from
                )));
            }
        }
    }

    #[test]
    fn prepared_envelope_rejects_legacy_zero_or_multi_operation_shapes() {
        for operations in [norito::json!([]), norito::json!([{}, {}])] {
            let retired = norito::json!({
                "schema": PREPARED_ENVELOPE_SCHEMA_V1,
                "binding": {
                    "schema": PREPARED_BINDING_SCHEMA_V1,
                    "authorization_sha256": ("ab".repeat(32)),
                    "authorization_nonce": ("n".repeat(32)),
                    "kind": "onboarding",
                    "phase": "pre_edge",
                    "idempotency_key": ("cd".repeat(32)),
                    "execution_expires_at_unix_ms": (u64::MAX)
                },
                "public_root": DEFAULT_PUBLIC_ROOT,
                "chain_id": DEFAULT_CHAIN_ID,
                "network_id": "retired",
                "authority": "retired",
                "operations": operations
            });
            let bytes = json::to_vec(&retired).expect("encode retired aggregate shape");
            let error = json::from_slice::<PreparedMutationEnvelopeV1>(&bytes)
                .expect_err("zero/multi operation envelopes must fail closed");
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn inrou_predecessor_decoder_rejects_unknown_fields_at_every_envelope_layer() {
        let account = AccountId::new(fixture_key_pair(0x45).public_key().clone());
        let fee_payment = FeePaymentIntent::authority(Vec::new(), None);
        let binding = TairaPublicResetMutationBindingV1 {
            schema: PREPARED_BINDING_SCHEMA_V1.to_owned(),
            authorization_sha256: "ab".repeat(32),
            authorization_nonce: "n".repeat(32),
            kind: "write_canary".to_owned(),
            phase: "pre_edge".to_owned(),
            idempotency_key: "cd".repeat(32),
            execution_expires_at_unix_ms: u64::MAX,
        };
        let operation = FinalCanaryPreparedTransactionV1 {
            schema: PREPARED_OPERATION_SCHEMA_V1.to_owned(),
            binding: binding.clone(),
            operation: WRITE_CANARY_OPERATION.to_owned(),
            transaction_hash_hex: "ab".repeat(32),
            signed_transaction_wire_hex: "00".to_owned(),
            signed_transaction_wire_sha256: "cd".repeat(32),
            semantic_hash_hex: "ef".repeat(32),
            fee_payment: fee_payment.clone(),
            fee_quote: FeeQuoteResponse {
                intent: fee_payment,
                observation: iroha_torii_shared::FeeQuoteObservation {
                    ledger_time_ms: 1,
                    next_block_height: 1,
                    route_dataspace_id: iroha::data_model::nexus::DataSpaceId::UNIVERSAL,
                },
                components: Vec::new(),
                capacities: Vec::new(),
                decision: iroha_torii_shared::FeeQuoteDecision::Accepted {
                    debit_source: iroha::data_model::nexus::FeeDebitSource::Account(
                        account.clone(),
                    ),
                    program_revision: None,
                },
            },
        };
        let envelope = PreparedMutationEnvelopeV1 {
            schema: PREPARED_ENVELOPE_SCHEMA_V1.to_owned(),
            binding,
            public_root: DEFAULT_PUBLIC_ROOT.to_owned(),
            chain_id: DEFAULT_CHAIN_ID.to_owned(),
            network_id: "fixture-network".to_owned(),
            authority: account.to_string(),
            operation: PreparedTransactionOperationV1::FinalCanary(operation),
        };
        let exact = canonical_prepared_envelope_bytes(&envelope).expect("canonical predecessor");
        decode_exact_inrou_predecessor_v1(&exact, "write_canary")
            .expect("exact final-canary predecessor");

        for path in [
            &["retired_v0"][..],
            &["binding", "retired_v0"][..],
            &["operation", "retired_v0"][..],
            &["operation", "envelope", "retired_v0"][..],
            &["operation", "envelope", "fee_quote", "retired_v0"][..],
            &[
                "operation",
                "envelope",
                "fee_quote",
                "observation",
                "retired_v0",
            ][..],
        ] {
            let mut unknown = json::to_value(&envelope).expect("encode predecessor fixture");
            let mut object = unknown.as_object_mut().expect("predecessor object");
            for segment in &path[..path.len() - 1] {
                object = object
                    .get_mut(*segment)
                    .and_then(Value::as_object_mut)
                    .expect("nested predecessor object");
            }
            object.insert(
                path[path.len() - 1].to_owned(),
                Value::String("forbidden".to_owned()),
            );
            let mut bytes = json::to_json(&unknown)
                .expect("encode unknown predecessor")
                .into_bytes();
            bytes.push(b'\n');
            assert!(
                decode_exact_inrou_predecessor_v1(&bytes, "write_canary").is_err(),
                "first-release predecessor accepted unknown path {}",
                path.join("."),
            );
        }
    }

    fn test_onboarding_token_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("create onboarding token file");
        file.write_all(TEST_ONBOARDING_TOKEN.as_bytes())
            .expect("write onboarding token file");
        file.flush().expect("flush onboarding token file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(file.path(), fs::Permissions::from_mode(0o600))
                .expect("set onboarding token file mode");
        }
        file
    }

    fn fixture_write_canary_args(operation: WriteCanaryOperation) -> WriteCanary {
        let faucet_authority = AccountId::new(
            KeyPair::try_from_seed(vec![0x43; 32], Algorithm::Ed25519)
                .expect("deterministic faucet policy key")
                .public_key()
                .clone(),
        );
        let authorization_nonce = "n".repeat(32);
        let mutation_phase = "pre_edge".to_owned();
        let idempotency_key = write_canary_child_idempotency_key(
            &authorization_nonce,
            &mutation_phase,
            operation.mutation_kind(),
        );
        WriteCanary {
            public_root: DEFAULT_PUBLIC_ROOT.to_owned(),
            faucet_authority: Some(faucet_authority.to_string()),
            faucet_asset_id: Some(DEFAULT_GAS_ASSET_ID.to_owned()),
            faucet_amount: Some("25000".to_owned()),
            onboarding_token_file: None,
            operation,
            authorization_sha256: "ab".repeat(32),
            authorization_nonce,
            mutation_phase,
            idempotency_key,
            execution_expires_at_unix_ms: u64::MAX,
            prepare_envelope: true,
            prepared_output_fd: Some(3),
            submit_prepared_envelope_fd: None,
            recover_prepared_envelope_fd: None,
            prerequisite_envelope_fd: None,
            json: true,
        }
    }

    #[derive(Clone)]
    struct MockRequest {
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: String,
    }
    impl std::fmt::Debug for MockRequest {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("MockRequest")
                .field("method", &self.method)
                .field("path", &self.path)
                .field(
                    "header_names",
                    &self
                        .headers
                        .iter()
                        .map(|(name, _)| name)
                        .collect::<Vec<_>>(),
                )
                .field("body", &self.body)
                .finish()
        }
    }
    impl MockRequest {
        fn header_values(&self, name: &str) -> Vec<&str> {
            self.headers
                .iter()
                .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
                .collect()
        }
    }
    struct MockResponse {
        status: u16,
        content_type: &'static str,
        headers: Vec<(&'static str, String)>,
        body: String,
    }
    fn fixture_key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair")
    }
    struct TextContext {
        cfg: Config,
        i18n: Localizer,
        lines: Vec<String>,
    }
    impl TextContext {
        fn new() -> Self {
            Self {
                cfg: crate::fallback_config(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
                lines: Vec::new(),
            }
        }
    }
    impl RunContext for TextContext {
        fn config(&self) -> &Config {
            &self.cfg
        }
        fn transaction_metadata(&self) -> Option<&Metadata> {
            None
        }
        fn input_instructions(&self) -> bool {
            false
        }
        fn output_instructions(&self) -> bool {
            false
        }
        fn i18n(&self) -> &Localizer {
            &self.i18n
        }
        fn output_format(&self) -> CliOutputFormat {
            CliOutputFormat::Text
        }
        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: norito::json::JsonSerialize + ?Sized,
        {
            let _value = norito::json::to_value(data)?;
            Ok(())
        }
        fn println(&mut self, data: impl std::fmt::Display) -> Result<()> {
            self.lines.push(data.to_string());
            Ok(())
        }
    }
    impl MockResponse {
        fn json(status: u16, value: Value) -> Self {
            Self {
                status,
                content_type: "application/json",
                headers: Vec::new(),
                body: json::to_json(&value).expect("mock JSON response"),
            }
        }
        fn text(status: u16, body: impl Into<String>) -> Self {
            Self {
                status,
                content_type: "text/plain",
                headers: Vec::new(),
                body: body.into(),
            }
        }
    }
    struct MockHttpServer {
        base_url: String,
        requests: Arc<Mutex<Vec<MockRequest>>>,
        stop: Arc<AtomicBool>,
        handle: thread::JoinHandle<()>,
    }
    fn spawn_mock_http<F>(expected_requests: usize, responder: F) -> MockHttpServer
    where
        F: Fn(&MockRequest) -> MockResponse + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("mock server address");
        listener
            .set_nonblocking(true)
            .expect("set mock server nonblocking");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let server_requests = Arc::clone(&requests);
        let stop = Arc::new(AtomicBool::new(false));
        let server_stop = Arc::clone(&stop);
        let responder = Arc::new(responder);
        let handle = thread::spawn(move || {
            let mut accepted = 0_usize;
            while accepted < expected_requests && !server_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_nonblocking(false)
                            .expect("set accepted mock stream blocking");
                        let request = read_mock_request(&mut stream);
                        let response = responder(&request);
                        server_requests
                            .lock()
                            .expect("requests")
                            .push(request.clone());
                        write_mock_response(&mut stream, response);
                        accepted += 1;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("mock server accept failed: {error}"),
                }
            }
        });
        MockHttpServer {
            base_url: format!("http://{addr}"),
            requests,
            stop,
            handle,
        }
    }
    fn read_mock_request(stream: &mut TcpStream) -> MockRequest {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("mock stream timeout");
        let mut raw = Vec::new();
        let mut buf = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buf).expect("read mock request");
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&buf[..read]);
            if let Some(header_end) = find_header_end(&raw) {
                let headers = String::from_utf8_lossy(&raw[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if raw.len() >= header_end + 4 + content_length {
                    break;
                }
            }
        }
        let header_end = find_header_end(&raw).expect("mock request header end");
        let headers = String::from_utf8_lossy(&raw[..header_end]);
        let request_line = headers.lines().next().expect("request line");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().expect("method").to_owned();
        let path = parts.next().expect("path").to_owned();
        let parsed_headers = headers
            .lines()
            .skip(1)
            .filter_map(|line| {
                let (name, value) = line.split_once(':')?;
                Some((name.trim().to_owned(), value.trim().to_owned()))
            })
            .collect();
        let body = String::from_utf8_lossy(&raw[header_end + 4..]).to_string();
        MockRequest {
            method,
            path,
            headers: parsed_headers,
            body,
        }
    }
    fn find_header_end(raw: &[u8]) -> Option<usize> {
        raw.windows(4).position(|window| window == b"\r\n\r\n")
    }
    fn write_mock_response(stream: &mut TcpStream, response: MockResponse) {
        let reason = match response.status {
            200 => "OK",
            202 => "Accepted",
            307 => "Temporary Redirect",
            400 => "Bad Request",
            401 => "Unauthorized",
            404 => "Not Found",
            503 => "Service Unavailable",
            _ => "OK",
        };
        let body = response.body.as_bytes();
        write!(
            stream,
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            response.status,
            reason,
            response.content_type,
            body.len()
        )
        .expect("write mock response headers");
        for (name, value) in response.headers {
            write!(stream, "{name}: {value}\r\n").expect("write mock response header");
        }
        write!(stream, "\r\n").expect("finish mock response headers");
        stream.write_all(body).expect("write mock response body");
    }
    fn finish_mock(server: MockHttpServer) -> Vec<MockRequest> {
        server.stop.store(true, Ordering::Release);
        server.handle.join().expect("mock server thread");
        Arc::try_unwrap(server.requests)
            .expect("request references")
            .into_inner()
            .expect("requests")
    }
    fn path_only(path: &str) -> &str {
        path.split_once('?').map_or(path, |(path, _)| path)
    }
    fn doctor_mock_response(request: &MockRequest, omit_tool: Option<&str>) -> MockResponse {
        match (request.method.as_str(), path_only(&request.path)) {
            ("GET", "/status") => MockResponse::json(
                200,
                norito::json!({
                    "txs_rejected_recent_5m": 0,
                    "queue_size": 0,
                    "sumeragi": { "tx_queue_saturated": false }
                }),
            ),
            ("GET", "/v1/time/now") => MockResponse::json(
                200,
                norito::json!({
                    "now": 1_785_168_000_000_u64,
                    "offset_ms": 0,
                    "confidence_ms": 1,
                    "sample_count": 3,
                    "peer_count": 3,
                    "enforcement_mode": "reject",
                    "fallback": false,
                    "health": {
                        "healthy": true,
                        "min_samples_ok": true,
                        "offset_ok": true,
                        "confidence_ok": true
                    }
                }),
            ),
            ("GET", "/v1/sumeragi/status") => MockResponse::json(
                401,
                norito::json!({
                    "code": "canonical_authentication_required",
                    "message": "canonical account request authentication is required"
                }),
            ),
            ("GET", "/v1/contracts/state") => {
                MockResponse::json(400, norito::json!({"error": "missing selector"}))
            }
            ("GET", "/v1/pipeline/transactions/status") => {
                MockResponse::json(400, norito::json!({"error": "missing transaction hash"}))
            }
            ("POST", "/v1/musubi/queries/ordered-prefix") => MockResponse::json(
                401,
                norito::json!({
                    "code": "canonical_authentication_required",
                    "message": "canonical account request authentication is required"
                }),
            ),
            ("GET", "/v1/soracloud/status") => MockResponse::json(
                401,
                norito::json!({
                    "code": "canonical_authentication_required",
                    "message": "canonical account request authentication is required"
                }),
            ),
            ("GET", "/v1/mcp") => MockResponse::json(
                200,
                norito::json!({"protocolVersion": MCP_PROTOCOL_VERSION}),
            ),
            ("POST", "/v1/mcp") if request.body.contains("tools/list") => {
                let tools: Vec<Value> = REQUIRED_MCP_TOOLS
                    .iter()
                    .copied()
                    .filter(|name| Some(*name) != omit_tool)
                    .map(|name| norito::json!({ "name": name, "description": "mock" }))
                    .collect();
                MockResponse::json(
                    200,
                    norito::json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "result": { "tools": tools }
                    }),
                )
            }
            ("POST", "/v1/mcp") => MockResponse::json(
                200,
                norito::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {
                        "protocolVersion": MCP_PROTOCOL_VERSION,
                        "capabilities": {}
                    }
                }),
            ),
            ("GET", _) => MockResponse::json(200, norito::json!({"ok": true})),
            _ => MockResponse::text(404, "not found"),
        }
    }
    fn inrou_canary_deployment(mode: &str, version: &str) -> InrouProbeIdentity {
        InrouProbeIdentity {
            service_name: "taira_inrou_canary".to_owned(),
            service_version: version.to_owned(),
            route_host: "taira-inrou-canary.sora".to_owned(),
            route_path_prefix: "/api/v1".to_owned(),
            healthcheck_path: "/health".to_owned(),
            stage_mode: mode.to_owned(),
            container_manifest_hash: Hash::new(b"taira-test-container-manifest").to_string(),
            service_manifest_hash: Hash::new(b"taira-test-service-manifest").to_string(),
        }
    }
    fn inrou_canary_artifact_version(seed: u8) -> String {
        format!("artifact-{}", Hash::new(&[seed; 32]))
    }
    #[expect(
        clippy::too_many_lines,
        reason = "the fixture spells out the complete exact Soracloud V1 response"
    )]
    fn exact_inrou_status(version: &str, action: &str, revision_count: u64) -> Value {
        norito::json!({
            "schema_version": 1,
            "service_health": {
                "mode": "embedded_runtime_manager",
                "status": "healthy",
                "message": "embedded runtime manager reports healthy hosted workloads",
                "observed_height": 1,
                "observed_block_hash": null,
                "state_dir": "/tmp/taira-inrou-runtime",
                "service_revisions": 1,
                "healthy_service_revisions": 1,
                "hydrating_service_revisions": 0,
                "degraded_service_revisions": 0,
                "unavailable_service_revisions": 0,
                "apartments": 0,
                "running_apartments": 0,
                "expired_apartments": 0
            },
            "routing": {
                "configured_lane_count": 1,
                "declared_lane_count": 1,
                "active_lane_count": 1,
                "active_lane_ids": [0],
                "autoscale_capacity_lane_count": 1,
                "autoscale_capacity_lane_ids": [0],
                "dataspace_count": 1,
                "routing_rules": 0,
                "default_lane_id": 0,
                "default_dataspace_id": 0
            },
            "hosted_http_topology": {
                "active_capability_adverts": 4,
                "placed_host_count": 4,
                "hosted_replica_count": 4,
                "unavailable_replica_count": 0
            },
            "resource_pressure": {
                "queue_active": 0,
                "queue_queued": 0,
                "queue_capacity": 1024,
                "queue_saturated": false,
                "high_load_threshold": 1024,
                "high_load": false,
                "runtime": {
                    "enabled": true,
                    "state_dir": "/tmp/taira-inrou-runtime",
                    "observed_height": 1,
                    "service_revisions": 1,
                    "apartments": 0,
                    "max_load_factor_bps": 0,
                    "reported_pending_mailbox_messages": 0,
                    "authoritative_pending_mailbox_messages": 0,
                    "bundle_cache_misses": 0,
                    "artifact_cache_misses": 0
                }
            },
            "failed_admissions": {
                "available": true,
                "total": 0,
                "governance_manifest_rejected": 0,
                "sorafs_provider_rejected": 0
            },
            "runtime_manager": {
                "available": true,
                "state_dir": "/tmp/taira-inrou-runtime",
                "snapshot": {
                    "schema_version": 1,
                    "observed_height": 1,
                    "observed_block_hash": null,
                    "local_peer_id": null,
                    "services": {},
                    "apartments": {}
                }
            },
            "control_plane": {
                "schema_version": 1,
                "service_count": 1,
                "audit_event_count": 1,
                "services": [{
                    "service_name": "taira_inrou_canary",
                    "current_version": version,
                    "revision_count": revision_count,
                    "config_generation": 0,
                    "secret_generation": 0,
                    "config_entry_count": 0,
                    "secret_entry_count": 0,
                    "quota_class": null,
                    "service_lease_status": null,
                    "lease_expires_height": null,
                    "prepaid_runtime_balance": null,
                    "remaining_runtime_balance": null,
                    "public_discovery_content_cid": null,
                    "public_discovery_url": null,
                    "public_discovery_cid_host_url": null,
                    "active_rollout": null,
                    "last_rollout": null,
                    "latest_revision": {
                        "sequence": revision_count,
                        "action": { "action": action, "value": null },
                        "service_version": version,
                        "container_manifest_hash": (Hash::new(b"taira-test-container-manifest")),
                        "service_manifest_hash": (Hash::new(b"taira-test-service-manifest")),
                        "replicas": 4,
                        "execution_plane": {
                            "execution_plane": "HttpService",
                            "value": null
                        },
                        "route_host": "taira-inrou-canary.sora",
                        "route_path_prefix": "/api/v1",
                        "base_url": null,
                        "healthcheck_url": null,
                        "public_discovery_content_cid": null,
                        "public_discovery_url": null,
                        "public_discovery_cid_host_url": null,
                        "state_binding_count": 0,
                        "state_bindings": [],
                        "lease_volumes": [],
                        "allow_model_inference": false,
                        "allow_model_training": false,
                        "runtime": { "runtime": "Inrou", "value": null },
                        "allow_state_writes": false,
                        "network": { "mode": "Open", "value": null },
                        "cpu_millis": 1000,
                        "memory_bytes": 1073741824,
                        "ephemeral_storage_bytes": 1073741824,
                        "max_open_files_per_process": 1024,
                        "max_tasks": 64,
                        "start_grace_secs": 30,
                        "stop_grace_secs": 30,
                        "healthcheck_path": "/health",
                        "required_config_names": [],
                        "required_secret_names": [],
                        "config_exports": [],
                        "sandbox_profile_hash": (Hash::new(b"taira-test-sandbox-profile")),
                        "process_generation": revision_count,
                        "process_started_sequence": revision_count,
                        "signed_by": "taira-test-validator"
                    }
                }],
                "recent_audit_events": [{
                    "sequence": revision_count,
                    "action": { "action": action, "value": null },
                    "service_name": "taira_inrou_canary",
                    "from_version": null,
                    "to_version": version,
                    "service_manifest_hash": (Hash::new(b"taira-test-service-manifest")),
                    "container_manifest_hash": (Hash::new(b"taira-test-container-manifest")),
                    "binding_name": null,
                    "state_key": null,
                    "config_name": null,
                    "secret_name": null,
                    "governance_tx_hash": null,
                    "rollout_handle": null,
                    "policy_name": null,
                    "policy_snapshot_hash": null,
                    "jurisdiction_tag": null,
                    "consent_evidence_hash": null,
                    "break_glass": null,
                    "break_glass_reason": null,
                    "lease_reporting_epoch_rollover": null,
                    "signed_by": "taira-test-validator"
                }]
            }
        })
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one cohesive fail-closed deploy and upgrade contract test"
    )]
    fn exact_inrou_status_requires_distinct_promoted_upgrade() {
        let deployed_version = inrou_canary_artifact_version(0x11);
        let upgraded_version = inrou_canary_artifact_version(0x22);
        let deploy = inrou_canary_deployment("deploy", &deployed_version);
        let deploy_status = exact_inrou_status(&deployed_version, "Deploy", 1);
        assert!(validate_exact_inrou_canary_status(&deploy_status, &deploy).is_ok());
        let mut missing_schema = deploy_status.clone();
        missing_schema
            .as_object_mut()
            .expect("status fixture is an object")
            .remove("schema_version");
        assert!(validate_exact_inrou_canary_status(&missing_schema, &deploy).is_err());
        for missing_field in [
            "active_capability_adverts",
            "placed_host_count",
            "hosted_replica_count",
            "unavailable_replica_count",
        ] {
            let mut missing = deploy_status.clone();
            missing
                .pointer_mut("/hosted_http_topology")
                .and_then(Value::as_object_mut)
                .expect("status fixture has hosted topology")
                .remove(missing_field);
            assert!(
                validate_exact_inrou_canary_status(&missing, &deploy).is_err(),
                "missing {missing_field} must fail closed"
            );
        }
        let mut duplicate_service = deploy_status.clone();
        let services = duplicate_service
            .pointer_mut("/control_plane/services")
            .and_then(Value::as_array_mut)
            .expect("status fixture has services");
        let duplicate = services[0].clone();
        services.push(duplicate);
        assert!(
            validate_exact_inrou_canary_status(&duplicate_service, &deploy).is_err(),
            "duplicate authoritative service rows must fail closed"
        );
        for missing_field in ["active_rollout", "last_rollout"] {
            let mut missing = deploy_status.clone();
            missing
                .pointer_mut("/control_plane/services/0")
                .and_then(Value::as_object_mut)
                .expect("status fixture has one service")
                .remove(missing_field);
            assert!(
                validate_exact_inrou_canary_status(&missing, &deploy).is_err(),
                "missing {missing_field} must fail closed"
            );
        }
        let mut missing_revision_version = deploy_status.clone();
        missing_revision_version
            .pointer_mut("/control_plane/services/0/latest_revision")
            .and_then(Value::as_object_mut)
            .expect("status fixture has a latest revision")
            .remove("service_version");
        assert!(
            validate_exact_inrou_canary_status(&missing_revision_version, &deploy).is_err(),
            "latest revision must carry its exact service version"
        );
        for hash_field in ["container_manifest_hash", "service_manifest_hash"] {
            let mut mismatched = deploy_status.clone();
            *mismatched
                .pointer_mut(&format!(
                    "/control_plane/services/0/latest_revision/{hash_field}"
                ))
                .expect("status fixture has manifest hashes") = Value::from("foreign-hash");
            assert!(
                validate_exact_inrou_canary_status(&mismatched, &deploy).is_err(),
                "a live {hash_field} mismatch must fail closed"
            );
        }
        for (path, retired) in [
            ("/control_plane/services/0/latest_revision/action", "Deploy"),
            ("/control_plane/services/0/latest_revision/runtime", "Inrou"),
            (
                "/control_plane/services/0/latest_revision/execution_plane",
                "HttpService",
            ),
        ] {
            let mut bare = deploy_status.clone();
            *bare
                .pointer_mut(path)
                .expect("status fixture has a tagged enum") = Value::from(retired);
            assert!(
                validate_exact_inrou_canary_status(&bare, &deploy).is_err(),
                "bare-string enum at {path} must fail closed"
            );
        }
        let mismatched_deploy = inrou_canary_deployment("deploy", "1.0.0");
        let mismatched_deploy_status = exact_inrou_status("1.0.0", "Deploy", 1);
        assert!(
            validate_exact_inrou_canary_status(&mismatched_deploy_status, &mismatched_deploy)
                .is_err()
        );

        let upgrade = inrou_canary_deployment("upgrade", &upgraded_version);
        let mut upgrade_status = exact_inrou_status(&upgraded_version, "Upgrade", 3);
        upgrade_status
            .pointer_mut("/control_plane/services/0")
            .and_then(Value::as_object_mut)
            .expect("status fixture has one service")
            .insert(
                "last_rollout".to_owned(),
                norito::json!({
                    "rollout_handle": "taira-inrou-upgrade",
                    "baseline_version": deployed_version.clone(),
                    "candidate_version": upgraded_version.clone(),
                    "canary_percent": 100,
                    "traffic_percent": 100,
                    "stage": { "stage": "Promoted", "value": null },
                    "health_failures": 0,
                    "max_health_failures": 3,
                    "health_window_secs": 30,
                    "created_sequence": 1,
                    "updated_sequence": 2
                }),
            );
        assert!(validate_exact_inrou_canary_status(&upgrade_status, &upgrade).is_ok());

        let mut bare_stage = upgrade_status.clone();
        *bare_stage
            .pointer_mut("/control_plane/services/0/last_rollout/stage")
            .expect("status fixture has a rollout stage") = Value::from("Promoted");
        assert!(
            validate_exact_inrou_canary_status(&bare_stage, &upgrade).is_err(),
            "bare-string rollout stage must fail closed"
        );

        let mut stale = upgrade_status.clone();
        *stale
            .pointer_mut("/control_plane/services/0/last_rollout/candidate_version")
            .expect("status fixture has a rollout candidate") =
            Value::from(deployed_version.clone());
        assert!(validate_exact_inrou_canary_status(&stale, &upgrade).is_err());
        for field in ["rollout_handle", "updated_sequence"] {
            let mut missing = upgrade_status.clone();
            missing
                .pointer_mut("/control_plane/services/0/last_rollout")
                .and_then(Value::as_object_mut)
                .expect("status fixture has a rollout")
                .remove(field);
            assert!(
                validate_exact_inrou_canary_status(&missing, &upgrade).is_err(),
                "missing rollout field {field} must fail closed"
            );
        }
        let mut extra_rollout = upgrade_status.clone();
        extra_rollout
            .pointer_mut("/control_plane/services/0/last_rollout")
            .and_then(Value::as_object_mut)
            .expect("status fixture has a rollout")
            .insert("retired_v0".to_owned(), Value::from(true));
        assert!(validate_exact_inrou_canary_status(&extra_rollout, &upgrade).is_err());
        let mut extra_host = upgrade_status.clone();
        *extra_host
            .pointer_mut("/hosted_http_topology/active_capability_adverts")
            .expect("status fixture has an advert count") = Value::from(5_u64);
        assert!(validate_exact_inrou_canary_status(&extra_host, &upgrade).is_err());
        for (field, invalid) in [
            ("placed_host_count", 3_u64),
            ("unavailable_replica_count", 1_u64),
        ] {
            let mut unavailable = upgrade_status.clone();
            *unavailable
                .pointer_mut(&format!("/hosted_http_topology/{field}"))
                .expect("status fixture has an exact topology count") = Value::from(invalid);
            assert!(
                validate_exact_inrou_canary_status(&unavailable, &upgrade).is_err(),
                "noncanonical topology count {field}={invalid} must fail closed"
            );
        }
    }
    #[test]
    fn tagged_enum_name_requires_exact_tagged_unit_envelope() {
        let canonical = norito::json!({"runtime": "Inrou", "value": null});
        assert_eq!(tagged_enum_name(&canonical, "runtime"), Some("Inrou"));
        for retired in [
            Value::from("Inrou"),
            norito::json!({"runtime": "Inrou"}),
            norito::json!({"runtime": "Inrou", "value": {}}),
            norito::json!({"runtime": "Inrou", "value": null, "legacy": true}),
        ] {
            assert_eq!(
                tagged_enum_name(&retired, "runtime"),
                None,
                "noncanonical tagged enum must fail: {retired:?}"
            );
        }
    }
    #[test]
    fn doctor_soracloud_status_rejects_bare_string_enum_aliases() {
        let canonical = exact_inrou_status(&inrou_canary_artifact_version(0x11), "Deploy", 1);
        validate_soracloud_status(Some(&canonical)).expect("canonical tagged status");
        for (path, retired) in [
            ("/control_plane/services/0/latest_revision/runtime", "Inrou"),
            (
                "/control_plane/services/0/latest_revision/execution_plane",
                "HttpService",
            ),
        ] {
            let mut bare = canonical.clone();
            *bare
                .pointer_mut(path)
                .expect("status fixture has a tagged enum") = Value::from(retired);
            assert!(
                validate_soracloud_status(Some(&bare)).is_err(),
                "doctor must reject bare-string enum alias at {path}"
            );
        }
        for field in [
            "active_capability_adverts",
            "placed_host_count",
            "hosted_replica_count",
            "unavailable_replica_count",
        ] {
            let mut missing = canonical.clone();
            missing
                .pointer_mut("/hosted_http_topology")
                .and_then(Value::as_object_mut)
                .expect("status fixture has hosted HTTP topology")
                .remove(field);
            let error = validate_soracloud_status(Some(&missing))
                .expect_err("doctor must reject a missing authoritative topology count");
            assert!(
                error.contains(field),
                "missing {field} reported the wrong error: {error}"
            );
        }
    }
    #[test]
    fn doctor_soracloud_status_rejects_unknown_and_missing_v1_fields() {
        let canonical = exact_inrou_status(&inrou_canary_artifact_version(0x11), "Deploy", 1);
        let mut extra_root = canonical.clone();
        extra_root
            .as_object_mut()
            .expect("status fixture is an object")
            .insert("retired_v0".to_owned(), Value::from(true));
        assert!(
            validate_soracloud_status(Some(&extra_root))
                .expect_err("unknown root field must fail closed")
                .contains("unknown field `retired_v0`")
        );
        for path in [
            "/service_health",
            "/routing",
            "/hosted_http_topology",
            "/resource_pressure",
            "/resource_pressure/runtime",
            "/failed_admissions",
            "/runtime_manager",
            "/runtime_manager/snapshot",
            "/control_plane",
            "/control_plane/services/0",
            "/control_plane/services/0/latest_revision",
            "/control_plane/recent_audit_events/0",
        ] {
            let mut extra = canonical.clone();
            extra
                .pointer_mut(path)
                .and_then(Value::as_object_mut)
                .unwrap_or_else(|| panic!("status fixture has object at {path}"))
                .insert("retired_v0".to_owned(), Value::from(true));
            let error = validate_soracloud_status(Some(&extra))
                .expect_err("unknown nested field must fail closed");
            assert!(
                error.contains("unknown field `retired_v0`"),
                "unknown field at {path} reported the wrong error: {error}"
            );
        }
        for (path, field) in [
            ("", "routing"),
            ("/service_health", "observed_height"),
            ("/routing", "default_lane_id"),
            ("/hosted_http_topology", "placed_host_count"),
            ("/resource_pressure", "runtime"),
            ("/resource_pressure/runtime", "artifact_cache_misses"),
            ("/failed_admissions", "total"),
            ("/runtime_manager", "snapshot"),
            ("/runtime_manager/snapshot", "services"),
            ("/control_plane", "recent_audit_events"),
            ("/control_plane/services/0", "quota_class"),
            ("/control_plane/services/0/latest_revision", "network"),
            (
                "/control_plane/recent_audit_events/0",
                "lease_reporting_epoch_rollover",
            ),
        ] {
            let mut missing = canonical.clone();
            let object = if path.is_empty() {
                missing.as_object_mut()
            } else {
                missing.pointer_mut(path).and_then(Value::as_object_mut)
            }
            .unwrap_or_else(|| panic!("status fixture has object at {path}"));
            object.remove(field);
            let error = validate_soracloud_status(Some(&missing))
                .expect_err("missing nested field must fail closed");
            assert!(
                error.contains(field),
                "missing {field} at {path} reported the wrong error: {error}"
            );
        }
    }

    #[test]
    fn doctor_soracloud_status_requires_canonical_observed_block_hashes() {
        let canonical = exact_inrou_status(&inrou_canary_artifact_version(0x11), "Deploy", 1);
        for path in [
            "/service_health/observed_block_hash",
            "/runtime_manager/snapshot/observed_block_hash",
        ] {
            let mut present = canonical.clone();
            *present
                .pointer_mut(path)
                .expect("status fixture has observed block hash") =
                Value::from(Hash::new(path.as_bytes()).to_string());
            validate_soracloud_status(Some(&present))
                .expect("canonical observed block hash is accepted");

            for malformed in ["not-a-hash".to_owned(), "10".repeat(32)] {
                let mut invalid = canonical.clone();
                *invalid
                    .pointer_mut(path)
                    .expect("status fixture has observed block hash") = Value::from(malformed);
                validate_soracloud_status(Some(&invalid))
                    .expect_err("noncanonical observed block hash must fail closed");
            }
        }
    }
    #[test]
    fn doctor_soracloud_status_rejects_noncanonical_route_text() {
        let canonical = exact_inrou_status(&inrou_canary_artifact_version(0x11), "Deploy", 1);
        for (field, retired) in [
            ("route_host", " taira-inrou-canary.sora"),
            ("route_host", "TAIRA-INROU-CANARY.SORA"),
            ("route_path_prefix", " /api/v1"),
            ("route_path_prefix", "/api/v1/"),
            ("route_path_prefix", "/api//v1"),
        ] {
            let mut status = canonical.clone();
            *status
                .pointer_mut(&format!(
                    "/control_plane/services/0/latest_revision/{field}"
                ))
                .expect("status fixture has a route field") = Value::from(retired);
            assert!(
                validate_soracloud_status(Some(&status)).is_err(),
                "noncanonical {field} value {retired:?} must fail closed"
            );
        }
    }
    #[test]
    fn write_canary_child_idempotency_keys_are_domain_separated() {
        let nonce = "n".repeat(32);
        let phase = "pre_edge";
        let onboarding = write_canary_child_idempotency_key(&nonce, phase, "onboarding");
        let faucet = write_canary_child_idempotency_key(&nonce, phase, "faucet");
        let final_canary = write_canary_child_idempotency_key(&nonce, phase, "write_canary");
        assert_eq!(onboarding.len(), 64);
        assert_ne!(onboarding, faucet);
        assert_ne!(onboarding, final_canary);
        assert_ne!(faucet, final_canary);

        let mut args = fixture_write_canary_args(WriteCanaryOperation::Onboarding);
        assert!(args.binding().is_ok());
        args.idempotency_key = faucet;
        assert!(args.binding().is_err());
    }

    #[test]
    fn expired_submit_reconciles_known_state_but_bars_absent_effect() {
        let mut binding = fixture_write_canary_args(WriteCanaryOperation::FinalCanary)
            .binding()
            .expect("binding");
        binding.execution_expires_at_unix_ms = 1;
        let applied = PreparedRecoveryClassification::Applied {
            block_height: Some(7),
            evidence: "ab".repeat(32),
        };
        assert!(
            !submit_required_after_classification(&binding, &applied)
                .expect("known state is read-only across expiry")
        );
        let absent = PreparedRecoveryClassification::Absent;
        assert!(submit_required_after_classification(&binding, &absent).is_err());
    }
    #[test]
    fn inrou_canary_rejects_zero_timeout_before_external_work() {
        assert!(validate_inrou_canary_timeout(1).is_ok());
        let error = validate_inrou_canary_timeout(0)
            .expect_err("zero timeout must fail before canary mutation");
        assert!(error.to_string().contains("must be greater than zero"));
    }
    #[test]
    fn inrou_health_identity_requires_exact_v1_shape_and_version() {
        let service_version = inrou_canary_artifact_version(0x22);
        let deployment = inrou_canary_deployment("upgrade", &service_version);
        let canonical = norito::json!({
            "service": "taira_inrou_canary",
            "service_version": service_version.clone(),
            "runtime": "Inrou",
            "replica_slot": 3,
            "identity": "taira_inrou_canary:replica:3"
        });
        assert!(exact_inrou_health_identity(&canonical, &deployment).is_some());

        let mut extra = canonical.clone();
        extra
            .as_object_mut()
            .expect("health fixture is an object")
            .insert("legacy_version".to_owned(), Value::from("retired"));
        assert!(exact_inrou_health_identity(&extra, &deployment).is_none());
        let mut stale = canonical.clone();
        *stale
            .pointer_mut("/service_version")
            .expect("health fixture has a service version") =
            Value::from(inrou_canary_artifact_version(0x11));
        assert!(exact_inrou_health_identity(&stale, &deployment).is_none());
        let mut string_slot = canonical;
        *string_slot
            .pointer_mut("/replica_slot")
            .expect("health fixture has a replica slot") = Value::from("3");
        assert!(exact_inrou_health_identity(&string_slot, &deployment).is_none());
    }
    #[test]
    fn doctor_mock_healthy_flow_reports_ok() {
        let server = spawn_mock_http(14, |request| doctor_mock_response(request, None));
        let report = run_doctor(&server.base_url).expect("doctor report");
        let requests = finish_mock(server);
        assert_eq!(report_status(&report), Some("ok"));
        assert!(
            requests
                .iter()
                .any(|request| request.method == "POST" && request.body.contains("initialize"))
        );
        assert!(
            requests
                .iter()
                .any(|request| request.method == "POST" && request.body.contains("tools/list"))
        );
        assert!(requests.iter().any(|request| {
            request.method == "GET"
                && path_only(&request.path) == "/v1/pipeline/transactions/status"
        }));
        assert!(requests.iter().any(|request| {
            request.method == "GET" && path_only(&request.path) == "/v1/time/now"
        }));
        assert!(requests.iter().any(|request| {
            request.method == "POST"
                && path_only(&request.path) == "/v1/musubi/queries/ordered-prefix"
                && request.body == "{}"
        }));
    }
    #[test]
    fn inrou_status_probe_uses_canonical_account_authentication() {
        let server = spawn_mock_http(1, |request| {
            assert_eq!(request.method, "GET");
            assert_eq!(path_only(&request.path), "/v1/soracloud/status");
            MockResponse::json(200, norito::json!({ "schema_version": 1 }))
        });
        let key_pair = fixture_key_pair(0x41);
        let mut config = crate::fallback_config();
        config.account = AccountId::new(key_pair.public_key().clone());
        config.key_pair = key_pair;
        config.torii_api_url =
            Url::parse(&format!("{}/", server.base_url)).expect("mock Torii URL");
        let client = IrohaClient::new(config);
        let status = account_signed_soracloud_status(&client).expect("signed status response");
        assert_eq!(status.status, 200);
        assert_eq!(status.body, Some(norito::json!({ "schema_version": 1 })));
        let requests = finish_mock(server);
        let request = requests.first().expect("status request");
        for header in [
            "x-iroha-account",
            "x-iroha-signature",
            "x-iroha-timestamp-ms",
            "x-iroha-nonce",
        ] {
            assert_eq!(
                request.header_values(header).len(),
                1,
                "canonical header {header} must appear exactly once"
            );
        }
        assert_eq!(request.header_values("accept"), vec!["application/json"]);
    }
    #[test]
    fn render_report_text_includes_route_check_detail() {
        let mut checks = Vec::new();
        push_check(
            &mut checks,
            "contracts_state",
            400,
            true,
            route_check_detail(&[400]),
        );
        let report = report_value(
            "taira_doctor",
            "ok",
            DEFAULT_PUBLIC_ROOT,
            checks,
            Vec::new(),
            Vec::new(),
            Map::new(),
        )
        .expect("report");
        let mut context = TextContext::new();
        render_report(&mut context, false, &report).expect("render report");
        let output = context.lines.join("\n");
        assert!(
            output
                .contains("contracts_state HTTP 400 (mounted route is expected to return HTTP 400")
        );
    }
    #[test]
    fn configless_report_writer_emits_json_and_text() {
        let report = report_value(
            "taira_doctor",
            "ok",
            DEFAULT_PUBLIC_ROOT,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Map::new(),
        )
        .expect("report");

        let mut json_output = WriterReportOutput {
            write: Vec::new(),
            output_format: CliOutputFormat::Text,
        };
        render_report_to(&mut json_output, true, &report).expect("render configless JSON report");
        let decoded: Value =
            json::from_slice(&json_output.write).expect("decode configless JSON report");
        assert_eq!(decoded, report);
        assert!(json_output.write.ends_with(b"\n"));

        let mut text_output = WriterReportOutput {
            write: Vec::new(),
            output_format: CliOutputFormat::Text,
        };
        render_report_to(&mut text_output, false, &report).expect("render configless text report");
        assert_eq!(
            String::from_utf8(text_output.write).expect("UTF-8 report"),
            format!("taira_doctor: ok ({DEFAULT_PUBLIC_ROOT})\n")
        );
    }
    #[test]
    fn public_status_requires_canonical_nested_queue_saturation() {
        let canonical = norito::json!({
            "sumeragi": { "tx_queue_saturated": false },
            "txs_rejected_recent_5m": 0,
            "queue_size": 0
        });
        validate_public_status(Some(&canonical)).expect("canonical status");

        let flattened = norito::json!({
            "tx_queue_saturated": false,
            "sumeragi": { "tx_queue_saturated": false }
        });
        assert!(
            validate_public_status(Some(&flattened))
                .expect_err("retired flattened status field must fail")
                .contains("retired root")
        );
        let missing = norito::json!({ "sumeragi": {} });
        assert!(validate_public_status(Some(&missing)).is_err());
        assert!(validate_public_status(None).is_err());

        let mut warnings = Vec::new();
        let saturated = norito::json!({
            "sumeragi": { "tx_queue_saturated": true },
            "txs_rejected_recent_5m": 0,
            "queue_size": 0
        });
        collect_status_warnings(Some(&saturated), &mut warnings);
        assert_eq!(warnings, ["public transaction queue reports saturation"]);
    }
    #[test]
    fn time_snapshot_requires_network_time_and_every_health_axis() {
        let healthy = norito::json!({
            "now": 1_u64,
            "offset_ms": 0,
            "confidence_ms": 0_u64,
            "sample_count": 3_u64,
            "peer_count": 3_u64,
            "enforcement_mode": "reject",
            "fallback": false,
            "health": {
                "healthy": true,
                "min_samples_ok": true,
                "offset_ok": true,
                "confidence_ok": true
            }
        });
        validate_time_snapshot(Some(&healthy)).expect("healthy network time");
        for (label, mutation) in [
            ("fallback", "/v1/time/now is using the local-clock fallback"),
            (
                "samples",
                "/v1/time/now field `sample_count` must be a positive integer",
            ),
            ("health", "/v1/time/now health field `healthy` is not true"),
            (
                "enforcement",
                "/v1/time/now is not using fail-closed time enforcement",
            ),
        ] {
            let mut hostile = healthy.clone();
            let object = hostile.as_object_mut().expect("object");
            match label {
                "fallback" => {
                    object.insert("fallback".into(), Value::Bool(true));
                }
                "samples" => {
                    object.insert("sample_count".into(), Value::from(0_u64));
                }
                "health" => {
                    object
                        .get_mut("health")
                        .and_then(Value::as_object_mut)
                        .expect("health")
                        .insert("healthy".into(), Value::Bool(false));
                }
                "enforcement" => {
                    object.insert("enforcement_mode".into(), Value::from("warn"));
                }
                _ => unreachable!(),
            }
            assert_eq!(
                validate_time_snapshot(Some(&hostile)),
                Err(mutation.to_owned())
            );
        }
        assert!(validate_time_snapshot(None).is_err());
    }
    #[test]
    fn write_canary_exit_gate_fails_closed() {
        ensure_write_canary_succeeded(&norito::json!({"status": "ok"}))
            .expect("an explicitly successful canary may exit zero");
        for report in [
            norito::json!({"status": "fail"}),
            norito::json!({"status": "unknown"}),
            norito::json!({}),
        ] {
            let _ = ensure_write_canary_succeeded(&report)
                .expect_err("non-success canary reports must produce a failing process exit");
        }
    }
    #[test]
    fn onboarding_token_validation_is_byte_exact() {
        assert_eq!(
            validate_onboarding_token(TEST_ONBOARDING_TOKEN).expect("valid token"),
            TEST_ONBOARDING_TOKEN
        );
        for malformed in [
            String::new(),
            "T".repeat(31),
            "T".repeat(257),
            format!("{} ", "T".repeat(31)),
            format!("{}é", "T".repeat(31)),
        ] {
            let error = validate_onboarding_token(&malformed)
                .expect_err("malformed onboarding token must fail closed");
            if !malformed.is_empty() {
                assert!(!format!("{error:#}").contains(&malformed));
            }
        }
    }
    #[test]
    fn onboarding_token_file_is_exact_owner_only_regular_and_not_cached() {
        let file = test_onboarding_token_file();
        assert_eq!(
            read_onboarding_token_file(file.path())
                .expect("read token")
                .as_str(),
            TEST_ONBOARDING_TOKEN
        );
        let replacement = "Z".repeat(32);
        fs::write(file.path(), &replacement).expect("replace token bytes");
        assert_eq!(
            read_onboarding_token_file(file.path())
                .expect("read replacement token")
                .as_str(),
            replacement
        );
        fs::write(file.path(), format!("{TEST_ONBOARDING_TOKEN}\n")).expect("write newline token");
        let error = read_onboarding_token_file(file.path())
            .expect_err("newline must not be trimmed from token file");
        assert!(format!("{error:#}").contains("printable ASCII"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt as _, symlink};
            fs::write(file.path(), TEST_ONBOARDING_TOKEN).expect("restore token");
            fs::set_permissions(file.path(), fs::Permissions::from_mode(0o640))
                .expect("set unsafe permissions");
            let error = read_onboarding_token_file(file.path())
                .expect_err("group-readable token file must fail closed");
            assert!(format!("{error:#}").contains("group or other"));
            fs::set_permissions(file.path(), fs::Permissions::from_mode(0o600))
                .expect("restore safe permissions");
            let directory = tempfile::tempdir().expect("token symlink directory");
            let link = directory.path().join("onboarding.token");
            symlink(file.path(), &link).expect("create token symlink");
            let error =
                read_onboarding_token_file(&link).expect_err("token symlink must fail closed");
            assert!(format!("{error:#}").contains("non-symlink"));
        }
        let directory = tempfile::tempdir().expect("token directory");
        let error = read_onboarding_token_file(directory.path())
            .expect_err("directory token path must fail closed");
        assert!(format!("{error:#}").contains("regular non-symlink"));
    }
    #[test]
    fn doctor_mock_required_tool_missing_reports_failure() {
        let missing_tool = REQUIRED_MCP_TOOLS[0];
        let server = spawn_mock_http(14, move |request| {
            doctor_mock_response(request, Some(missing_tool))
        });
        let report = run_doctor(&server.base_url).expect("doctor report");
        let _requests = finish_mock(server);
        let failures = report
            .as_object()
            .and_then(|object| object.get("failures"))
            .and_then(Value::as_array)
            .expect("failures");
        assert_eq!(report_status(&report), Some("fail"));
        assert!(
            failures
                .iter()
                .filter_map(Value::as_str)
                .any(|failure| failure.contains(missing_tool))
        );
    }
    #[test]
    fn doctor_rejects_substituted_mcp_protocol_version() {
        let server = spawn_mock_http(14, |request| {
            if request.method == "GET" && path_only(&request.path) == "/v1/mcp" {
                MockResponse::json(200, norito::json!({"protocolVersion": "2024-11-05"}))
            } else {
                doctor_mock_response(request, None)
            }
        });
        let report = run_doctor(&server.base_url).expect("doctor report");
        let _requests = finish_mock(server);
        assert_eq!(report_status(&report), Some("fail"));
        assert!(
            report
                .get("failures")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .any(|failure| failure.contains("protocolVersion"))
        );
    }
    #[test]
    fn doctor_rejects_non_iroha_or_malformed_mcp_tools() {
        for hostile_tool in [
            norito::json!({"name": "connect.legacy", "description": "retired"}),
            norito::json!({"description": "missing name"}),
            norito::json!({"name": (REQUIRED_MCP_TOOLS[0]), "description": "duplicate"}),
        ] {
            let server = spawn_mock_http(14, move |request| {
                if request.method == "POST"
                    && path_only(&request.path) == "/v1/mcp"
                    && request.body.contains("tools/list")
                {
                    let mut tools: Vec<Value> = REQUIRED_MCP_TOOLS
                        .iter()
                        .map(|name| norito::json!({"name": name, "description": "mock"}))
                        .collect();
                    tools.push(hostile_tool.clone());
                    MockResponse::json(
                        200,
                        norito::json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "result": {"tools": tools}
                        }),
                    )
                } else {
                    doctor_mock_response(request, None)
                }
            });
            let report = run_doctor(&server.base_url).expect("doctor report");
            let _requests = finish_mock(server);
            assert_eq!(report_status(&report), Some("fail"));
        }
    }
    #[test]
    fn submit_failure_hints_cover_invalid_fee_intent_and_route_unavailable() {
        let invalid_fee = hint_submit_error(eyre!("invalid fee_payment intent"));
        assert!(format!("{invalid_fee:#}").contains("/v1/fees/quote"));
        let route = hint_submit_error(eyre!("route_unavailable"));
        assert!(format!("{route:#}").contains("ingress or lane routing"));
    }
    #[test]
    fn leading_zero_bits_counts_prefix() {
        assert_eq!(leading_zero_bits(&[0x00, 0x0f]), 12);
        assert_eq!(leading_zero_bits(&[0x80]), 0);
        assert_eq!(leading_zero_bits(&[0x40]), 1);
    }
    fn faucet_puzzle_fixture(network_id: &NetworkId) -> Value {
        norito::json!({
            "algorithm": FAUCET_POW_ALGORITHM,
            "network_id": (network_id.to_string()),
            "chain_discriminant": DEFAULT_CHAIN_DISCRIMINANT,
            "difficulty_bits": 1,
            "anchor_height": 7,
            "anchor_block_hash_hex": ("11".repeat(32)),
            "challenge_salt_hex": null,
            "scrypt_log_n": 1,
            "scrypt_r": 1,
            "scrypt_p": 1,
            "max_anchor_age_blocks": 16
        })
    }
    #[test]
    fn faucet_challenge_matches_python_fixture_shape() {
        let network_id = crate::fallback_config().network_id;
        let challenge = build_faucet_challenge(
            "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            &network_id,
            7,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect("challenge");
        assert_eq!(challenge.len(), 32);
        assert_ne!(challenge, [0_u8; 32]);
    }

    #[test]
    fn faucet_challenge_rejects_noncanonical_anchor_hash_hex() {
        let network_id = crate::fallback_config().network_id;
        build_faucet_challenge(
            "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            &network_id,
            7,
            &"AA".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect_err("uppercase anchor hash must fail before proof-of-work");
    }
    #[test]
    fn faucet_challenge_matches_v1_preimage_vector() {
        let genesis_hash =
            hex::decode("32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149")
                .expect("decode fixture genesis hash")
                .try_into()
                .expect("fixture genesis hash is exactly 32 bytes");
        let network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::prehashed(genesis_hash)),
        );
        let challenge = build_faucet_challenge(
            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53",
            &network_id,
            68,
            "d5c0016a6345e8ea379da42aab1fdc16ba82756e19e0b63c48c14735e8caf7ef",
            None,
        )
        .expect("V1 faucet challenge");
        assert_eq!(
            hex::encode(challenge),
            "21e547302359214b28f0d1e0b04b6aeaf62a0e597dbad018d93ab0ce6af81a05"
        );
    }
    #[test]
    fn solve_faucet_puzzle_rejects_pre_release_algorithm_label() {
        let network_id = crate::fallback_config().network_id;
        let mut puzzle = faucet_puzzle_fixture(&network_id);
        puzzle.as_object_mut().expect("puzzle object").insert(
            "algorithm".to_owned(),
            Value::from("scrypt-leading-zero-bits-v2"),
        );
        let error = solve_faucet_puzzle(
            "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            &network_id,
            &puzzle,
        )
        .expect_err("pre-release faucet algorithm must fail closed");
        let message = format!("{error:#}");
        assert!(message.contains("scrypt-leading-zero-bits-v2"));
        assert!(message.contains(FAUCET_POW_ALGORITHM));
    }
    #[test]
    fn faucet_challenge_rejects_same_label_different_genesis_replay() {
        let first_network = crate::fallback_config().network_id;
        let second_network =
            NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::new(b"foreign-faucet-genesis"),
            ));
        let account_id = "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
        let first = build_faucet_challenge(
            account_id,
            &first_network,
            7,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect("first challenge");
        let second = build_faucet_challenge(
            account_id,
            &second_network,
            7,
            &"11".repeat(32),
            Some(&"22".repeat(32)),
        )
        .expect("second challenge");
        assert_ne!(first, second);
    }
    #[test]
    fn solve_faucet_puzzle_rejects_zero_difficulty() {
        let network_id = crate::fallback_config().network_id;
        let mut puzzle = faucet_puzzle_fixture(&network_id);
        puzzle
            .as_object_mut()
            .expect("puzzle object")
            .insert("difficulty_bits".to_owned(), Value::from(0_u64));
        let error = solve_faucet_puzzle(
            "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            &network_id,
            &puzzle,
        )
        .expect_err("zero-difficulty faucet puzzle must fail closed");
        assert!(format!("{error:#}").contains("difficulty_bits must be positive"));
    }
    #[test]
    fn solve_faucet_puzzle_requires_the_exact_v1_field_set() {
        let network_id = crate::fallback_config().network_id;
        let canonical = faucet_puzzle_fixture(&network_id);
        validate_exact_faucet_puzzle_shape(&canonical).expect("exact V1 puzzle field set");
        assert_eq!(
            required_nullable_str(&canonical, "challenge_salt_hex")
                .expect("explicit nullable salt"),
            None
        );

        for field in FAUCET_PUZZLE_V1_FIELDS {
            let mut missing = canonical.clone();
            missing
                .as_object_mut()
                .expect("puzzle object")
                .remove(field);
            let error = solve_faucet_puzzle(
                "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                &network_id,
                &missing,
            )
            .expect_err("omitted exact puzzle field must fail closed");
            assert!(format!("{error:#}").contains("exact V1 field set"));
        }

        let mut unknown = canonical.clone();
        unknown
            .as_object_mut()
            .expect("puzzle object")
            .insert("legacy_salt".to_owned(), Value::Null);
        assert!(
            solve_faucet_puzzle(
                "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                &network_id,
                &unknown,
            )
            .is_err(),
            "unknown puzzle fields must fail closed"
        );

        let mut malformed = canonical;
        malformed
            .as_object_mut()
            .expect("puzzle object")
            .insert("challenge_salt_hex".to_owned(), Value::from(false));
        assert!(required_nullable_str(&malformed, "challenge_salt_hex").is_err());
    }
    #[test]
    fn taira_puzzle_identity_requires_the_exact_network_and_discriminant() {
        let network_id = crate::fallback_config().network_id;
        let canonical = norito::json!({
            "network_id": (network_id.to_string()),
            "chain_discriminant": DEFAULT_CHAIN_DISCRIMINANT,
        });
        assert_eq!(
            validate_taira_puzzle_identity(&canonical, &network_id)
                .expect("canonical Taira puzzle identity"),
            network_id
        );

        let foreign_network =
            NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::new(b"foreign-taira-genesis"),
            ));
        let foreign = norito::json!({
            "network_id": (foreign_network.to_string()),
            "chain_discriminant": DEFAULT_CHAIN_DISCRIMINANT,
        });
        let network_error = validate_taira_puzzle_identity(&foreign, &network_id)
            .expect_err("a foreign network identity must fail before publication");
        assert!(format!("{network_error:#}").contains("does not match configured network"));

        let wrong_discriminant = norito::json!({
            "network_id": (network_id.to_string()),
            "chain_discriminant": (DEFAULT_CHAIN_DISCRIMINANT + 1),
        });
        let discriminant_error = validate_taira_puzzle_identity(&wrong_discriminant, &network_id)
            .expect_err("a foreign chain discriminant must fail before publication");
        assert!(format!("{discriminant_error:#}").contains("does not match Taira"));
    }
    #[test]
    fn resolve_canary_signer_derives_account() {
        let key_pair = fixture_key_pair(3);
        let mut config = crate::fallback_config();
        config.key_pair = key_pair.clone();
        let signer = resolve_canary_signer(&config).expect("config signer");
        assert_eq!(
            signer.account_id,
            AccountId::new(key_pair.public_key().clone())
        );
    }
    #[test]
    fn canary_alias_is_one_canonical_key_derived_identity() {
        let key_pair = fixture_key_pair(7);
        let alias = canary_alias(key_pair.public_key());
        assert!(alias.starts_with("tairarolloutcanary"));
        assert!(alias.ends_with("@universal"));
        assert!(
            alias
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '@')
        );
        assert_eq!(alias, canary_alias(key_pair.public_key()));
    }
    #[test]
    fn public_root_requires_an_exact_origin() {
        assert_eq!(
            normalize_root_url(DEFAULT_PUBLIC_ROOT).expect("canonical public root"),
            DEFAULT_PUBLIC_ROOT
        );
        assert_eq!(
            normalize_root_url("http://127.0.0.1:18080").expect("canonical local origin"),
            "http://127.0.0.1:18080"
        );
        for invalid in [
            " https://taira.sora.org",
            "https://user@taira.sora.org",
            "https://taira.sora.org/",
            "https://taira.sora.org/path",
            "https://taira.sora.org?debug=1",
            "https://taira.sora.org#fragment",
            "HTTPS://TAIRA.SORA.ORG",
        ] {
            let _ = normalize_root_url(invalid)
                .expect_err("noncanonical public roots must fail closed");
        }
    }
    #[test]
    fn inrou_canary_requires_exact_taira_client_identity_before_publication() {
        let mut config = crate::fallback_config();
        config.chain = DEFAULT_CHAIN_ID.into();
        config.account_chain_discriminant = DEFAULT_CHAIN_DISCRIMINANT;
        ensure_canonical_taira_client_identity(&config).expect("canonical Taira identity");

        config.chain = "iroha3-taira".into();
        let chain_error = ensure_canonical_taira_client_identity(&config)
            .expect_err("retired chain alias must fail before publication");
        assert!(format!("{chain_error:#}").contains("requires canonical chain"));

        config.chain = DEFAULT_CHAIN_ID.into();
        config.account_chain_discriminant = DEFAULT_CHAIN_DISCRIMINANT + 1;
        let discriminant_error = ensure_canonical_taira_client_identity(&config)
            .expect_err("wrong Taira discriminant must fail before publication");
        assert!(format!("{discriminant_error:#}").contains("requires chain discriminant"));
    }
    #[test]
    fn fixture_key_pair_uses_checked_seed_derivation() {
        assert_eq!(fixture_key_pair(11).algorithm(), Algorithm::Ed25519);
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }
}
