//! Deploy a contract with governance-attributed native registration and one
//! atomic deployment commit.
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::items_after_test_module,
    clippy::needless_borrow,
    clippy::needless_borrows_for_generic_args,
    clippy::redundant_closure_for_method_calls,
    clippy::too_many_lines
)]

use std::{
    fs,
    io::Read as _,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use clap::Parser;
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    account_address::parse_account_address,
    client::Client,
    config::{self, Config},
    data_model::{
        isi::smart_contract_code::{
            CommitContractDeployment, FinalizeSmartContractCodeUpload, RegisterSmartContractCode,
            SMART_CONTRACT_CODE_CHUNK_BYTES, UploadSmartContractCodeChunk,
        },
        metadata::Metadata,
        name::Name,
        prelude::*,
        smart_contract::ContractAlias,
        transaction::{FeePaymentIntent, TransactionBuilder},
    },
};
use iroha_config::parameters::{
    actual::SorafsRolloutPhase,
    defaults::{
        sorafs::gateway::{DEFAULT_ANONYMITY_POLICY, DEFAULT_ROLLOUT_PHASE},
        torii,
    },
};
use iroha_crypto::{Hash, KeyPair, PrivateKey};
use iroha_primitives::json::Json;
use iroha_torii_shared::FeeQuoteResponse;
use iroha_version::codec::EncodeVersioned;
use sorafs_manifest::alias_cache::AliasCachePolicy;
use sorafs_orchestrator::AnonymityPolicy;
use url::Url;
use zeroize::Zeroizing;

const DEFAULT_CHAIN_DISCRIMINANT_TAIRA: u16 = 369;
const MAX_PRIVATE_KEY_FILE_BYTES: u64 = 16 * 1024;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    torii_url: String,
    #[arg(long)]
    chain_id: String,
    #[arg(long)]
    authority: String,
    /// Owner-held mode-0600 regular file containing one exact private-key
    /// literal. Inline key arguments are intentionally unsupported so process
    /// listings cannot expose the signer.
    #[arg(long, value_name = "PATH")]
    private_key_file: PathBuf,
    #[arg(long)]
    code_file: PathBuf,
    #[arg(long)]
    contract_alias: String,
    #[arg(long, default_value_t = DEFAULT_CHAIN_DISCRIMINANT_TAIRA)]
    chain_discriminant: u16,
    /// Canonical Norito JSON selecting the fee payer, sponsor revision, and gas bound.
    /// Every deployment transaction is quoted and signed with exact recommended limits.
    #[arg(long)]
    fee_payment_json: PathBuf,
    #[arg(long = "gov-manifest-approver", value_name = "ACCOUNT")]
    gov_manifest_approvers: Vec<String>,
    #[arg(long, default_value_t = 300_000)]
    status_timeout_ms: u64,
    #[arg(long)]
    transaction_ttl_ms: Option<u64>,
    #[arg(long, default_value_t = 300_000)]
    torii_request_timeout_ms: u64,
    #[arg(long)]
    out_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    emit_only: bool,
    #[arg(long, default_value_t = false)]
    skip_register_bytes: bool,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonDeserialize, norito::derive::JsonSerialize,
)]
#[norito(deny_unknown_fields)]
struct ContractDeploymentStateSnapshot {
    authority: String,
    contract_alias: String,
    deploy_nonce: String,
    dataspace_alias: String,
    dataspace_id: String,
    previous_contract_address: Option<String>,
    observed_block_height: String,
    observed_block_hash: String,
    ledger_time_ms: String,
    chain_discriminant: String,
}

struct ValidatedContractDeploymentState {
    snapshot: ContractDeploymentStateSnapshot,
    deploy_nonce: u64,
    dataspace_id: DataSpaceId,
    previous_contract_address: Option<iroha::data_model::smart_contract::ContractAddress>,
}

#[cfg(test)]
use iroha::data_model::transaction::Executable;

fn default_alias_cache_policy() -> AliasCachePolicy {
    AliasCachePolicy::new(
        Duration::from_secs(torii::SORAFS_ALIAS_POSITIVE_TTL_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_HARD_EXPIRY_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_REVOCATION_TTL_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS),
    )
}

fn default_anonymity_policy() -> AnonymityPolicy {
    AnonymityPolicy::parse(DEFAULT_ANONYMITY_POLICY).unwrap_or(AnonymityPolicy::GuardPq)
}

fn default_rollout_phase() -> SorafsRolloutPhase {
    SorafsRolloutPhase::parse(DEFAULT_ROLLOUT_PHASE).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn make_client(
    torii_url: &str,
    chain_id: &str,
    authority: AccountId,
    chain_discriminant: u16,
    key_pair: KeyPair,
    status_timeout_ms: u64,
    transaction_ttl_ms: Option<u64>,
    torii_request_timeout_ms: u64,
) -> Result<Client> {
    let config = Config {
        chain: chain_id
            .parse::<ChainId>()
            .wrap_err("--chain-id must be canonical")?,
        account: authority,
        account_chain_discriminant: chain_discriminant,
        key_pair,
        basic_auth: None,
        torii_api_url: Url::parse(torii_url).wrap_err("invalid --torii-url")?,
        torii_request_timeout: Duration::from_millis(torii_request_timeout_ms),
        transaction_ttl: transaction_ttl_ms.map_or(
            config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
            Duration::from_millis,
        ),
        transaction_status_timeout: Duration::from_millis(status_timeout_ms),
        transaction_add_nonce: false,
        connect_queue_root: config::default_connect_queue_root(),
        soracloud_http_witness_file: None,
        sorafs_alias_cache: default_alias_cache_policy(),
        sorafs_anonymity_policy: default_anonymity_policy(),
        sorafs_rollout_phase: default_rollout_phase(),
    };
    Ok(Client::new(config))
}

fn insert_string_metadata(
    metadata: &mut Metadata,
    key: &str,
    value: impl Into<String>,
) -> Result<()> {
    metadata.insert(Name::from_str(key)?, Json::new(value.into()));
    Ok(())
}

fn insert_gov_manifest_approvers(metadata: &mut Metadata, approvers: &[String]) -> Result<()> {
    let mut accounts = Vec::new();
    for (index, raw) in approvers.iter().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(eyre!("--gov-manifest-approver[{index}] must not be blank"));
        }
        accounts.push(trimmed.to_owned());
    }
    if !accounts.is_empty() {
        metadata.insert(
            Name::from_str("gov_manifest_approvers")?,
            Json::new(accounts),
        );
    }
    Ok(())
}

fn deployment_transaction_metadata(
    contract_address: &iroha::data_model::smart_contract::ContractAddress,
    gov_manifest_approvers: &[String],
) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    insert_string_metadata(
        &mut metadata,
        "gov_contract_address",
        contract_address.to_string(),
    )?;
    insert_string_metadata(
        &mut metadata,
        "contract_address",
        contract_address.to_string(),
    )?;
    insert_gov_manifest_approvers(&mut metadata, gov_manifest_approvers)?;
    Ok(metadata)
}

fn canonical_decimal_u64(raw: &str, field: &str) -> Result<u64> {
    let parsed = raw
        .parse::<u64>()
        .wrap_err_with(|| format!("deployment-state `{field}` is not a u64"))?;
    if parsed.to_string() != raw {
        return Err(eyre!(
            "deployment-state `{field}` is not canonical decimal text"
        ));
    }
    Ok(parsed)
}

fn read_contract_deployment_state(
    client: &Client,
    authority: &AccountId,
    contract_alias: &ContractAlias,
    chain_discriminant: u16,
) -> Result<ValidatedContractDeploymentState> {
    let response = client
        .post_contract_deployment_state(contract_alias)
        .wrap_err("failed to read authenticated contract deployment state")?;
    let status = response.status();
    let body = response.into_body();
    if status.as_u16() != 200 {
        return Err(eyre!(
            "contract deployment-state request failed with HTTP {}: {}",
            status,
            std::str::from_utf8(&body).unwrap_or("")
        ));
    }
    let snapshot: ContractDeploymentStateSnapshot = norito::json::from_slice(&body)
        .wrap_err("decode closed contract deployment-state response")?;
    if snapshot.authority != authority.to_string()
        || snapshot.contract_alias != contract_alias.to_string()
    {
        return Err(eyre!(
            "contract deployment-state response does not bind the exact authority and alias"
        ));
    }

    let deploy_nonce = canonical_decimal_u64(&snapshot.deploy_nonce, "deploy_nonce")?;
    if deploy_nonce == u64::MAX {
        return Err(eyre!("contract deployment nonce is exhausted"));
    }
    let dataspace_id = DataSpaceId::new(canonical_decimal_u64(
        &snapshot.dataspace_id,
        "dataspace_id",
    )?);
    let expected_dataspace_alias = if contract_alias.dataspace_segment() == "universal" {
        "universal"
    } else {
        contract_alias.dataspace_segment()
    };
    if snapshot.dataspace_alias != expected_dataspace_alias {
        return Err(eyre!(
            "contract deployment-state response names a different dataspace alias"
        ));
    }
    let response_discriminant =
        canonical_decimal_u64(&snapshot.chain_discriminant, "chain_discriminant")?;
    if response_discriminant != u64::from(chain_discriminant) {
        return Err(eyre!(
            "contract deployment-state chain discriminant differs from the configured client"
        ));
    }
    let observed_height =
        canonical_decimal_u64(&snapshot.observed_block_height, "observed_block_height")?;
    if observed_height == 0 {
        return Err(eyre!(
            "contract deployment-state observed block height must be non-zero"
        ));
    }
    canonical_decimal_u64(&snapshot.ledger_time_ms, "ledger_time_ms")?;
    let observed_hash: iroha_crypto::HashOf<iroha::data_model::block::BlockHeader> = snapshot
        .observed_block_hash
        .parse()
        .wrap_err("deployment-state observed block hash is invalid")?;
    if observed_hash.to_string() != snapshot.observed_block_hash {
        return Err(eyre!(
            "deployment-state observed block hash is not canonical"
        ));
    }

    let previous_contract_address = snapshot
        .previous_contract_address
        .as_deref()
        .map(str::parse::<iroha::data_model::smart_contract::ContractAddress>)
        .transpose()
        .wrap_err("deployment-state previous contract address is invalid")?;
    if let Some(previous) = previous_contract_address.as_ref()
        && (previous.to_string()
            != snapshot
                .previous_contract_address
                .as_deref()
                .expect("present parsed previous address has source")
            || previous
                .dataspace_id()
                .map_err(|error| eyre!(error.to_string()))?
                != dataspace_id)
    {
        return Err(eyre!(
            "deployment-state previous contract address is non-canonical or in another dataspace"
        ));
    }

    Ok(ValidatedContractDeploymentState {
        snapshot,
        deploy_nonce,
        dataspace_id,
        previous_contract_address,
    })
}

#[cfg(unix)]
fn validate_private_key_file_metadata(metadata: &fs::Metadata, path: &Path) -> Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    if metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(eyre!(
            "private-key file {} must be an owner-held mode-0600 single-link regular file",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_private_key_file_metadata(_metadata: &fs::Metadata, path: &Path) -> Result<()> {
    Err(eyre!(
        "private-key file {} cannot be authenticated on this platform",
        path.display()
    ))
}

fn read_private_key_file(path: &Path) -> Result<PrivateKey> {
    let before = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect private-key file {}", path.display()))?;
    if before.file_type().is_symlink() || !before.file_type().is_file() {
        return Err(eyre!(
            "private-key file {} must be a regular non-symlink file",
            path.display()
        ));
    }
    if before.len() == 0 || before.len() > MAX_PRIVATE_KEY_FILE_BYTES {
        return Err(eyre!(
            "private-key file {} must contain 1..={MAX_PRIVATE_KEY_FILE_BYTES} bytes",
            path.display()
        ));
    }
    validate_private_key_file_metadata(&before, path)?;

    #[cfg(unix)]
    let mut file = {
        let descriptor = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )
        .wrap_err_with(|| format!("securely open private-key file {}", path.display()))?;
        fs::File::from(descriptor)
    };
    #[cfg(not(unix))]
    let mut file = fs::OpenOptions::new()
        .read(true)
        .open(path)
        .wrap_err_with(|| format!("open private-key file {}", path.display()))?;

    let opened = file
        .metadata()
        .wrap_err_with(|| format!("inspect opened private-key file {}", path.display()))?;
    if !opened.file_type().is_file() {
        return Err(eyre!(
            "private-key file {} must remain a regular file",
            path.display()
        ));
    }
    validate_private_key_file_metadata(&opened, path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        if opened.dev() != before.dev() || opened.ino() != before.ino() {
            return Err(eyre!(
                "private-key file {} changed during secure open",
                path.display()
            ));
        }
    }

    let maximum_read = MAX_PRIVATE_KEY_FILE_BYTES
        .checked_add(1)
        .expect("bounded private-key file size");
    let mut raw = Zeroizing::new(Vec::with_capacity(
        usize::try_from(before.len()).expect("bounded private-key file length fits usize"),
    ));
    file.by_ref()
        .take(maximum_read)
        .read_to_end(&mut raw)
        .wrap_err_with(|| format!("read private-key file {}", path.display()))?;
    let after = file
        .metadata()
        .wrap_err_with(|| format!("reinspect private-key file {}", path.display()))?;
    validate_private_key_file_metadata(&after, path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        if after.dev() != opened.dev()
            || after.ino() != opened.ino()
            || after.len() != opened.len()
            || after.mtime() != opened.mtime()
            || after.mtime_nsec() != opened.mtime_nsec()
        {
            return Err(eyre!(
                "private-key file {} changed while it was read",
                path.display()
            ));
        }
    }
    if u64::try_from(raw.len()).ok() != Some(opened.len()) {
        return Err(eyre!(
            "private-key file {} changed while it was read",
            path.display()
        ));
    }

    let raw = raw.strip_suffix(b"\n").unwrap_or(raw.as_slice());
    let literal = std::str::from_utf8(raw)
        .map_err(|_| eyre!("private-key file {} must be UTF-8", path.display()))?;
    if literal.is_empty() || literal.trim() != literal || literal.chars().any(char::is_control) {
        return Err(eyre!(
            "private-key file {} must contain one exact private-key literal",
            path.display()
        ));
    }
    literal
        .parse()
        .wrap_err_with(|| format!("parse private-key file {}", path.display()))
}

struct NativeUploadTransactionPlan {
    chunk_count: u32,
    pre_stage: Vec<(String, String, SignedTransaction)>,
    finalize: (String, String, SignedTransaction),
}

struct TransactionSigningContext<'a> {
    chain_id: &'a ChainId,
    authority: &'a AccountId,
    private_key: &'a PrivateKey,
    transaction_ttl: Option<Duration>,
    fee_payment: &'a FeePaymentIntent,
    metadata: &'a Metadata,
}

impl TransactionSigningContext<'_> {
    fn sign(
        &self,
        instructions: impl IntoIterator<Item = InstructionBox>,
    ) -> Result<SignedTransaction> {
        let mut builder = TransactionBuilder::new(
            self.chain_id.clone(),
            self.authority.clone(),
            self.fee_payment.clone(),
        );
        if let Some(transaction_ttl) = self.transaction_ttl {
            builder.set_ttl(transaction_ttl);
        }
        builder
            .with_metadata(self.metadata.clone())
            .with_instructions(instructions)
            .try_sign(self.private_key)
            .wrap_err("failed to sign instruction transaction")
    }
}

fn native_upload_report(
    plan: &NativeUploadTransactionPlan,
    skip_register_bytes: bool,
) -> norito::json::Value {
    let register_bytes_stage_tx_hashes = if skip_register_bytes {
        Vec::new()
    } else {
        plan.pre_stage
            .iter()
            .map(|(_, _, transaction)| transaction.hash().to_string())
            .collect::<Vec<_>>()
    };
    let register_bytes_tx_hash = (!skip_register_bytes).then(|| plan.finalize.2.hash().to_string());

    norito::json!({
        "register_bytes_tx_strategy": ("native_chunks"),
        "register_bytes_chunk_size": (u64::try_from(SMART_CONTRACT_CODE_CHUNK_BYTES)
            .expect("public contract chunk size fits u64")),
        "register_bytes_chunk_count": (plan.chunk_count),
        "register_bytes_stage_tx_hashes": (register_bytes_stage_tx_hashes),
        "register_bytes_tx_hash": (register_bytes_tx_hash),
    })
}

fn deployment_transaction_sequence(
    skip_register_bytes: bool,
    register_plans: Vec<(String, String, SignedTransaction)>,
    register_manifest_tx: SignedTransaction,
    commit_deployment_tx: SignedTransaction,
) -> Vec<(String, String, SignedTransaction)> {
    let mut planned = if skip_register_bytes {
        Vec::new()
    } else {
        register_plans
    };
    planned.push((
        "register_manifest".to_owned(),
        "register-manifest".to_owned(),
        register_manifest_tx,
    ));
    planned.push((
        "commit_deployment".to_owned(),
        "commit-deployment".to_owned(),
        commit_deployment_tx,
    ));
    planned
}

#[allow(clippy::too_many_arguments)]
fn build_commit_deployment_transaction(
    signing: &TransactionSigningContext<'_>,
    expected_deploy_nonce: u64,
    contract_address: iroha::data_model::smart_contract::ContractAddress,
    code_hash: Hash,
    contract_alias: ContractAlias,
    expected_previous_contract_address: Option<iroha::data_model::smart_contract::ContractAddress>,
) -> Result<SignedTransaction> {
    signing.sign([InstructionBox::from(CommitContractDeployment {
        expected_deploy_nonce,
        contract_address,
        code_hash,
        contract_alias,
        lease_expiry_ms: None,
        expected_previous_contract_address,
    })])
}

fn build_native_upload_transaction_plan(
    signing: &TransactionSigningContext<'_>,
    code_hash: Hash,
    code: &[u8],
) -> Result<NativeUploadTransactionPlan> {
    if code.is_empty() {
        return Err(eyre!("contract artifact must not be empty"));
    }
    let canonical_code_hash = ivm::contract_code_hash(code);
    if code_hash != canonical_code_hash {
        return Err(eyre!(
            "contract code hash does not match the canonical artifact hash"
        ));
    }
    let total_size = u64::try_from(code.len())
        .wrap_err("contract artifact length does not fit the upload descriptor")?;
    let chunk_count_usize = code.len().div_ceil(SMART_CONTRACT_CODE_CHUNK_BYTES);
    let chunk_count = u32::try_from(chunk_count_usize)
        .wrap_err("contract upload chunk count does not fit u32")?;
    let mut pre_stage = Vec::with_capacity(chunk_count_usize.saturating_sub(1));

    for (index, chunk) in code.chunks(SMART_CONTRACT_CODE_CHUNK_BYTES).enumerate() {
        let chunk_index =
            u32::try_from(index).wrap_err("contract upload index does not fit u32")?;
        let upload = UploadSmartContractCodeChunk {
            code_hash,
            total_size,
            chunk_index,
            chunk_count,
            chunk: chunk.to_vec(),
        };
        let is_final = index + 1 == chunk_count_usize;
        let instructions = if is_final {
            vec![
                InstructionBox::from(upload),
                InstructionBox::from(FinalizeSmartContractCodeUpload {
                    code_hash,
                    total_size,
                    chunk_count,
                }),
            ]
        } else {
            vec![InstructionBox::from(upload)]
        };
        let tx = signing.sign(instructions)?;
        if is_final {
            return Ok(NativeUploadTransactionPlan {
                chunk_count,
                pre_stage,
                finalize: (
                    "register_bytes_finalize".to_owned(),
                    "register-bytes-finalize".to_owned(),
                    tx,
                ),
            });
        }
        let ordinal = index + 1;
        pre_stage.push((
            format!("register_bytes_chunk_{ordinal:04}_of_{chunk_count_usize:04}"),
            format!("register-bytes-chunk-{ordinal:04}-of-{chunk_count_usize:04}"),
            tx,
        ));
    }

    Err(eyre!("contract upload plan did not contain a final chunk"))
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;

    fn checked_ivm_contract_deploy_ed25519_key_fixture() -> KeyPair {
        KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519)
            .expect("generate checked IVM contract deploy fixture key")
    }

    fn test_fee_payment() -> FeePaymentIntent {
        FeePaymentIntent::authority(
            Vec::new(),
            Some(NonZeroU64::new(1_000_000).expect("nonzero test gas limit")),
        )
    }

    fn private_key_file_fixture(contents: &str) -> Result<tempfile::NamedTempFile> {
        use std::io::Write as _;

        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(contents.as_bytes())?;
        file.flush()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            fs::set_permissions(file.path(), fs::Permissions::from_mode(0o600))?;
        }
        Ok(file)
    }

    #[test]
    fn private_key_file_accepts_one_exact_literal_with_terminal_newline() -> Result<()> {
        let expected = checked_ivm_contract_deploy_ed25519_key_fixture();
        let exposed = iroha_crypto::ExposedPrivateKey(expected.private_key().clone()).to_string();
        let file = private_key_file_fixture(&format!("{exposed}\n"))?;

        let actual = read_private_key_file(file.path())?;

        assert_eq!(
            KeyPair::from(actual).public_key(),
            expected.public_key(),
            "the file parser must preserve the exact private key"
        );
        Ok(())
    }

    #[test]
    fn private_key_file_rejects_surrounding_whitespace_without_echoing_secret() -> Result<()> {
        let secret = "secret-material-that-must-not-appear-in-errors";
        let file = private_key_file_fixture(&format!(" {secret}\n"))?;

        let error = read_private_key_file(file.path()).expect_err("whitespace must be rejected");
        let message = error.to_string();

        assert!(message.contains("one exact private-key literal"));
        assert!(!message.contains(secret));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn private_key_file_rejects_non_mode_0600_permissions() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let file = private_key_file_fixture("not-inspected-after-mode-check\n")?;
        fs::set_permissions(file.path(), fs::Permissions::from_mode(0o400))?;

        let error = read_private_key_file(file.path()).expect_err("mode 0400 must fail");

        assert!(error.to_string().contains("mode-0600"));
        Ok(())
    }

    #[test]
    fn clap_surface_does_not_accept_inline_private_keys() {
        let parsed = Args::try_parse_from([
            "ivm-contract-deploy",
            "--torii-url",
            "http://127.0.0.1:8080",
            "--chain-id",
            "localnet",
            "--authority",
            "authority",
            "--private-key",
            "must-not-be-accepted",
            "--code-file",
            "contract.to",
            "--contract-alias",
            "contract::universal",
            "--fee-payment-json",
            "fee.json",
        ]);

        assert!(
            parsed.is_err(),
            "inline private keys must not be a CLI option"
        );
    }

    #[test]
    fn ivm_contract_deploy_fixture_uses_checked_ed25519_key_generation() {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let actual = key_pair
            .public_key()
            .try_algorithm()
            .expect("IVM contract deploy fixture key advertises a valid algorithm");

        assert_eq!(actual, iroha_crypto::Algorithm::Ed25519);
    }

    #[test]
    fn deployment_state_decimal_fields_must_be_canonical() -> Result<()> {
        assert_eq!(canonical_decimal_u64("17", "fixture")?, 17);
        for invalid in ["", "017", "+17", " 17", "17 "] {
            assert!(
                canonical_decimal_u64(invalid, "fixture").is_err(),
                "non-canonical decimal `{invalid}` must fail"
            );
        }
        Ok(())
    }

    #[test]
    fn deployment_state_snapshot_rejects_unknown_fields() {
        let payload = br#"{
            "authority":"authority",
            "contract_alias":"deploy::universal",
            "deploy_nonce":"0",
            "dataspace_alias":"universal",
            "dataspace_id":"0",
            "previous_contract_address":null,
            "observed_block_height":"1",
            "observed_block_hash":"hash",
            "ledger_time_ms":"1",
            "chain_discriminant":"369",
            "retired_hint":"must-fail"
        }"#;

        let result = norito::json::from_slice::<ContractDeploymentStateSnapshot>(payload);

        assert!(
            result.is_err(),
            "deployment-state DTO must reject unknown compatibility fields"
        );
    }

    #[test]
    fn clap_surface_rejects_independent_deployment_cas_inputs() {
        for retired in [
            ["--dataspace-id", "20"],
            [
                "--expected-previous-contract-address",
                "caller-controlled-address",
            ],
        ] {
            let mut arguments = vec![
                "ivm-contract-deploy",
                "--torii-url",
                "http://127.0.0.1:8080",
                "--chain-id",
                "localnet",
                "--authority",
                "authority",
                "--private-key-file",
                "private.key",
                "--code-file",
                "contract.to",
                "--contract-alias",
                "contract::universal",
                "--fee-payment-json",
                "fee.json",
            ];
            arguments.extend(retired);

            assert!(
                Args::try_parse_from(arguments).is_err(),
                "deployment CAS state must come only from the authenticated snapshot"
            );
        }
    }

    #[test]
    fn transaction_signing_context_checked_helper_verifies() -> Result<()> {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::of(key_pair.public_key().clone());
        let chain_id = ChainId::from("ivm-contract-deploy-instruction-sign-test");
        let fee_payment = test_fee_payment();
        let metadata = Metadata::default();
        let signing = TransactionSigningContext {
            chain_id: &chain_id,
            authority: &authority,
            private_key: key_pair.private_key(),
            transaction_ttl: None,
            fee_payment: &fee_payment,
            metadata: &metadata,
        };

        let tx = signing.sign(Vec::<InstructionBox>::new())?;

        tx.verify_signature()
            .wrap_err("verify IVM deploy instruction helper signature")?;
        assert_eq!(tx.authority(), &authority);
        Ok(())
    }

    #[test]
    fn final_deployment_transaction_is_one_native_atomic_commit() -> Result<()> {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::of(key_pair.public_key().clone());
        let chain_id = ChainId::from("ivm-contract-deploy-native-commit-test");
        let fee_payment = test_fee_payment();
        let metadata = Metadata::default();
        let contract_address = iroha::data_model::smart_contract::ContractAddress::derive(
            &chain_id,
            &authority,
            11,
            DataSpaceId::UNIVERSAL,
        )
        .map_err(|error| eyre!(error.to_string()))?;
        let contract_alias: ContractAlias = "atomic_deploy::universal".parse()?;
        let code_hash = Hash::new(b"reviewed-contract-artifact");
        let transaction = build_commit_deployment_transaction(
            &TransactionSigningContext {
                chain_id: &chain_id,
                authority: &authority,
                private_key: key_pair.private_key(),
                transaction_ttl: None,
                fee_payment: &fee_payment,
                metadata: &metadata,
            },
            11,
            contract_address.clone(),
            code_hash,
            contract_alias.clone(),
            None,
        )?;

        let Executable::Instructions(instructions) = transaction.instructions() else {
            panic!("native contract deployment commit must use instructions");
        };
        assert_eq!(instructions.len(), 1);
        let commit = instructions[0]
            .as_any()
            .downcast_ref::<CommitContractDeployment>()
            .expect("final deployment transaction must be the native atomic commit");
        assert_eq!(commit.expected_deploy_nonce, 11);
        assert_eq!(commit.contract_address, contract_address);
        assert_eq!(commit.code_hash, code_hash);
        assert_eq!(commit.contract_alias, contract_alias);
        assert!(commit.lease_expiry_ms.is_none());
        assert!(commit.expected_previous_contract_address.is_none());
        assert!(
            instructions[0]
                .as_any()
                .downcast_ref::<iroha::data_model::isi::SetKeyValueBox>()
                .is_none(),
            "generic metadata writes cannot advance the reserved deployment nonce"
        );
        Ok(())
    }

    #[test]
    fn one_chunk_contract_registration_uses_upload_and_finalize() -> Result<()> {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::of(key_pair.public_key().clone());
        let code = vec![1, 2, 3, 4];
        let plan = build_native_upload_transaction_plan(
            &TransactionSigningContext {
                chain_id: &ChainId::from("ivm-contract-deploy-native-register-test"),
                authority: &authority,
                private_key: key_pair.private_key(),
                transaction_ttl: None,
                fee_payment: &test_fee_payment(),
                metadata: &Metadata::default(),
            },
            ivm::contract_code_hash(&code),
            &code,
        )?;

        assert_eq!(plan.chunk_count, 1);
        assert!(plan.pre_stage.is_empty());
        assert_eq!(plan.finalize.1, "register-bytes-finalize");
        let Executable::Instructions(instructions) = plan.finalize.2.instructions() else {
            panic!("native upload plan must use instruction transactions");
        };
        assert_eq!(instructions.len(), 2);
        let upload = instructions[0]
            .as_any()
            .downcast_ref::<UploadSmartContractCodeChunk>()
            .expect("first instruction uploads the only chunk");
        assert_eq!(upload.chunk_index, 0);
        assert_eq!(upload.chunk_count, 1);
        assert_eq!(upload.total_size, u64::try_from(code.len())?);
        assert_eq!(upload.code_hash, ivm::contract_code_hash(&code));
        assert_eq!(upload.chunk, code);
        let finalize = instructions[1]
            .as_any()
            .downcast_ref::<FinalizeSmartContractCodeUpload>()
            .expect("second instruction finalizes the only chunk");
        assert_eq!(finalize.code_hash, upload.code_hash);
        assert_eq!(finalize.total_size, upload.total_size);
        assert_eq!(finalize.chunk_count, upload.chunk_count);
        let report = native_upload_report(&plan, false);
        assert!(
            report["register_bytes_stage_tx_hashes"]
                .as_array()
                .expect("one-chunk stage hashes are an array")
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn native_upload_plan_rejects_empty_artifact() {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::of(key_pair.public_key().clone());
        let result = build_native_upload_transaction_plan(
            &TransactionSigningContext {
                chain_id: &ChainId::from("ivm-contract-deploy-empty-register-test"),
                authority: &authority,
                private_key: key_pair.private_key(),
                transaction_ttl: None,
                fee_payment: &test_fee_payment(),
                metadata: &Metadata::default(),
            },
            Hash::new(b""),
            &[],
        );
        let error = match result {
            Ok(_) => panic!("an empty artifact cannot form a native upload"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("must not be empty"));
    }

    #[test]
    fn native_upload_plan_rejects_noncanonical_code_hash() {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::of(key_pair.public_key().clone());
        let code = [0x01, 0x02, 0x03];
        let result = build_native_upload_transaction_plan(
            &TransactionSigningContext {
                chain_id: &ChainId::from("ivm-contract-deploy-wrong-hash-test"),
                authority: &authority,
                private_key: key_pair.private_key(),
                transaction_ttl: None,
                fee_payment: &test_fee_payment(),
                metadata: &Metadata::default(),
            },
            Hash::new(b"not-the-canonical-artifact-hash"),
            &code,
        );
        let error = match result {
            Ok(_) => panic!("a mismatched code hash cannot form a native upload"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("canonical artifact hash"));
    }

    #[test]
    fn exact_chunk_boundary_uses_one_final_transaction() -> Result<()> {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::of(key_pair.public_key().clone());
        let code = vec![0x91; SMART_CONTRACT_CODE_CHUNK_BYTES];
        let plan = build_native_upload_transaction_plan(
            &TransactionSigningContext {
                chain_id: &ChainId::from("ivm-contract-deploy-boundary-register-test"),
                authority: &authority,
                private_key: key_pair.private_key(),
                transaction_ttl: None,
                fee_payment: &test_fee_payment(),
                metadata: &Metadata::default(),
            },
            ivm::contract_code_hash(&code),
            &code,
        )?;

        assert_eq!(plan.chunk_count, 1);
        assert!(plan.pre_stage.is_empty());
        let Executable::Instructions(instructions) = plan.finalize.2.instructions() else {
            panic!("native upload plan must use instructions");
        };
        let upload = instructions[0]
            .as_any()
            .downcast_ref::<UploadSmartContractCodeChunk>()
            .expect("final transaction starts with the only upload");
        assert_eq!(upload.chunk.len(), SMART_CONTRACT_CODE_CHUNK_BYTES);
        assert_eq!(upload.total_size, u64::try_from(code.len())?);
        Ok(())
    }

    #[test]
    fn multi_mib_upload_plan_is_bounded_ordered_and_stable() -> Result<()> {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::of(key_pair.public_key().clone());
        let code = (0..(3 * 1024 * 1024 + 17))
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let chain_id = ChainId::from("ivm-contract-deploy-large-native-register-test");
        let contract_address = iroha::data_model::smart_contract::ContractAddress::derive(
            &chain_id,
            &authority,
            11,
            DataSpaceId::UNIVERSAL,
        )
        .map_err(|error| eyre!(error.to_string()))?;
        let metadata =
            deployment_transaction_metadata(&contract_address, &[authority.to_string()])?;
        let plan = build_native_upload_transaction_plan(
            &TransactionSigningContext {
                chain_id: &chain_id,
                authority: &authority,
                private_key: key_pair.private_key(),
                transaction_ttl: Some(Duration::from_secs(30)),
                fee_payment: &test_fee_payment(),
                metadata: &metadata,
            },
            ivm::contract_code_hash(&code),
            &code,
        )?;
        let expected_count = code.len().div_ceil(SMART_CONTRACT_CODE_CHUNK_BYTES);
        assert_eq!(usize::try_from(plan.chunk_count)?, expected_count);
        assert_eq!(plan.pre_stage.len(), expected_count - 1);
        assert_eq!(
            plan.pre_stage.first().map(|(_, slug, _)| slug.as_str()),
            Some("register-bytes-chunk-0001-of-0049")
        );
        assert_eq!(plan.finalize.1, "register-bytes-finalize");

        let transactions = plan
            .pre_stage
            .iter()
            .map(|(_, _, tx)| tx)
            .chain(std::iter::once(&plan.finalize.2))
            .collect::<Vec<_>>();
        let mut rebuilt = Vec::with_capacity(code.len());
        for (expected_index, transaction) in transactions.iter().enumerate() {
            assert_eq!(transaction.metadata(), &metadata);
            assert!(
                transaction.encode_versioned().len()
                    < iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_TX_GOSSIP.get(),
                "chunk transaction {expected_index} exceeded the default gossip frame"
            );
            let Executable::Instructions(instructions) = transaction.instructions() else {
                panic!("native upload plan must not emit IVM executables");
            };
            let upload = instructions[0]
                .as_any()
                .downcast_ref::<UploadSmartContractCodeChunk>()
                .expect("every code-bearing transaction starts with one upload");
            assert_eq!(usize::try_from(upload.chunk_index)?, expected_index);
            assert_eq!(upload.chunk_count, plan.chunk_count);
            assert_eq!(upload.total_size, u64::try_from(code.len())?);
            assert_eq!(upload.code_hash, ivm::contract_code_hash(&code));
            let expected_chunk_len = code
                .len()
                .saturating_sub(expected_index * SMART_CONTRACT_CODE_CHUNK_BYTES)
                .min(SMART_CONTRACT_CODE_CHUNK_BYTES);
            assert_eq!(upload.chunk.len(), expected_chunk_len);
            assert!(upload.chunk.len() < code.len());
            rebuilt.extend_from_slice(&upload.chunk);
            assert_eq!(
                instructions.len(),
                usize::from(expected_index + 1 == expected_count) + 1
            );
            if expected_index + 1 == expected_count {
                let finalize = instructions[1]
                    .as_any()
                    .downcast_ref::<FinalizeSmartContractCodeUpload>()
                    .expect("last upload transaction must finalize");
                assert_eq!(finalize.code_hash, upload.code_hash);
                assert_eq!(finalize.total_size, upload.total_size);
                assert_eq!(finalize.chunk_count, upload.chunk_count);
            }
        }
        assert_eq!(rebuilt, code);
        Ok(())
    }

    #[test]
    fn native_upload_json_reports_exact_hash_roles_and_skip_semantics() -> Result<()> {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::of(key_pair.public_key().clone());
        let code = vec![0x35; 2 * SMART_CONTRACT_CODE_CHUNK_BYTES + 1];
        let plan = build_native_upload_transaction_plan(
            &TransactionSigningContext {
                chain_id: &ChainId::from("ivm-contract-deploy-json-test"),
                authority: &authority,
                private_key: key_pair.private_key(),
                transaction_ttl: None,
                fee_payment: &test_fee_payment(),
                metadata: &Metadata::default(),
            },
            ivm::contract_code_hash(&code),
            &code,
        )?;

        let report = native_upload_report(&plan, false);
        let fields = report.as_object().expect("upload report is an object");
        assert_eq!(fields.len(), 5);
        assert!(!fields.contains_key("direct_register_bytes_tx_size"));
        assert_eq!(
            fields["register_bytes_tx_strategy"].as_str(),
            Some("native_chunks")
        );
        assert_eq!(
            fields["register_bytes_chunk_size"].as_u64(),
            Some(u64::try_from(SMART_CONTRACT_CODE_CHUNK_BYTES)?)
        );
        assert_eq!(fields["register_bytes_chunk_count"].as_u64(), Some(3));
        let expected_finalize_hash = plan.finalize.2.hash().to_string();
        assert_eq!(
            fields["register_bytes_tx_hash"].as_str(),
            Some(expected_finalize_hash.as_str())
        );
        let expected_stage_hashes = plan
            .pre_stage
            .iter()
            .map(|(_, _, transaction)| transaction.hash().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            fields["register_bytes_stage_tx_hashes"]
                .as_array()
                .expect("stage hashes are an array")
                .iter()
                .map(|value| value.as_str().expect("stage hash is a string"))
                .collect::<Vec<_>>(),
            expected_stage_hashes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );

        let skipped = native_upload_report(&plan, true);
        let skipped = skipped.as_object().expect("skipped report is an object");
        assert_eq!(skipped.len(), 5);
        assert_eq!(
            skipped["register_bytes_tx_strategy"].as_str(),
            Some("native_chunks")
        );
        assert_eq!(skipped["register_bytes_chunk_count"].as_u64(), Some(3));
        assert!(skipped["register_bytes_tx_hash"].is_null());
        assert!(
            skipped["register_bytes_stage_tx_hashes"]
                .as_array()
                .expect("skipped stage hashes are an array")
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn skip_registration_omits_all_uploads_and_emit_order_is_stable() -> Result<()> {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::of(key_pair.public_key().clone());
        let chain = ChainId::from("ivm-contract-deploy-sequence-test");
        let code = vec![0x7a; SMART_CONTRACT_CODE_CHUNK_BYTES + 1];
        let fee_payment = test_fee_payment();
        let metadata = Metadata::default();
        let signing = TransactionSigningContext {
            chain_id: &chain,
            authority: &authority,
            private_key: key_pair.private_key(),
            transaction_ttl: None,
            fee_payment: &fee_payment,
            metadata: &metadata,
        };
        let upload =
            build_native_upload_transaction_plan(&signing, ivm::contract_code_hash(&code), &code)?;
        let mut uploads = upload.pre_stage;
        uploads.push(upload.finalize);
        let empty_transaction = || signing.sign(Vec::<InstructionBox>::new());
        let sequence = deployment_transaction_sequence(
            false,
            uploads,
            empty_transaction()?,
            empty_transaction()?,
        );
        assert_eq!(
            sequence
                .iter()
                .map(|(_, slug, _)| slug.as_str())
                .collect::<Vec<_>>(),
            vec![
                "register-bytes-chunk-0001-of-0002",
                "register-bytes-finalize",
                "register-manifest",
                "commit-deployment",
            ]
        );
        let output = tempfile::tempdir()?;
        let written = sequence
            .iter()
            .map(|(_, slug, transaction)| write_tx(output.path(), slug, transaction))
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(
            written
                .iter()
                .map(|(path, _)| path.file_name().and_then(std::ffi::OsStr::to_str).unwrap())
                .collect::<Vec<_>>(),
            vec![
                "register-bytes-chunk-0001-of-0002.norito",
                "register-bytes-finalize.norito",
                "register-manifest.norito",
                "commit-deployment.norito",
            ]
        );
        for ((_, _, transaction), (path, byte_len)) in sequence.iter().zip(&written) {
            let encoded = transaction.encode_versioned();
            assert_eq!(*byte_len, encoded.len());
            assert_eq!(fs::read(path)?, encoded);
        }

        let skipped = deployment_transaction_sequence(
            true,
            sequence.into_iter().take(2).collect(),
            empty_transaction()?,
            empty_transaction()?,
        );
        assert_eq!(
            skipped
                .iter()
                .map(|(_, slug, _)| slug.as_str())
                .collect::<Vec<_>>(),
            vec!["register-manifest", "commit-deployment"]
        );
        Ok(())
    }

    #[test]
    fn every_real_deployment_transaction_carries_identical_governance_metadata() -> Result<()> {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::of(key_pair.public_key().clone());
        let chain = ChainId::from("ivm-contract-deploy-metadata-test");
        let contract_address = iroha::data_model::smart_contract::ContractAddress::derive(
            &chain,
            &authority,
            7,
            DataSpaceId::UNIVERSAL,
        )
        .map_err(|error| eyre!(error.to_string()))?;
        let approvers = vec![authority.to_string()];
        let metadata = deployment_transaction_metadata(&contract_address, &approvers)?;
        let fee_payment = test_fee_payment();
        let code = vec![0x44; SMART_CONTRACT_CODE_CHUNK_BYTES + 1];
        let signing = TransactionSigningContext {
            chain_id: &chain,
            authority: &authority,
            private_key: key_pair.private_key(),
            transaction_ttl: None,
            fee_payment: &fee_payment,
            metadata: &metadata,
        };
        let upload =
            build_native_upload_transaction_plan(&signing, ivm::contract_code_hash(&code), &code)?;
        let mut transactions = upload
            .pre_stage
            .into_iter()
            .map(|(_, _, transaction)| transaction)
            .chain(std::iter::once(upload.finalize.2))
            .collect::<Vec<_>>();
        let code_hash = ivm::contract_code_hash(&code);
        let manifest = iroha::data_model::smart_contract::manifest::ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash),
            abi_hash: Some(Hash::new(b"metadata-test-abi")),
            compiler_fingerprint: None,
            features_bitmap: Some(0),
            access_set_hints: None,
            entrypoints: None,
            states: None,
            error_codes: None,
            kotoba: None,
            provenance: None,
        }
        .try_signed(&key_pair)
        .wrap_err("sign metadata-test manifest")?;
        transactions
            .push(signing.sign([InstructionBox::from(RegisterSmartContractCode { manifest })])?);
        transactions.push(build_commit_deployment_transaction(
            &signing,
            7,
            contract_address,
            code_hash,
            "metadata_test::universal".parse()?,
            None,
        )?);

        for transaction in &transactions {
            assert_eq!(transaction.metadata(), &metadata);
            assert_eq!(transaction.fee_payment_intent(), &fee_payment);
            assert!(transaction.metadata().get("gas_limit").is_none());
            assert!(transaction.metadata().get("gas_asset_id").is_none());
            for key in [
                "gov_contract_address",
                "contract_address",
                "gov_manifest_approvers",
            ] {
                assert!(
                    transaction.metadata().get(&Name::from_str(key)?).is_some(),
                    "missing governance key {key}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn deployment_metadata_rejects_blank_governance_approvers() {
        let key_pair = checked_ivm_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::of(key_pair.public_key().clone());
        let contract_address = iroha::data_model::smart_contract::ContractAddress::derive(
            &ChainId::from("ivm-contract-deploy-metadata-validation-test"),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive test address");

        let error = deployment_transaction_metadata(&contract_address, &["  ".to_owned()])
            .expect_err("blank governance approver must fail before signing");
        assert!(error.to_string().contains("must not be blank"));
    }
}

fn quote_and_resign_transaction(
    client: &Client,
    draft: &SignedTransaction,
    requested_fee_payment: &FeePaymentIntent,
) -> Result<(SignedTransaction, FeeQuoteResponse)> {
    let mut payload = draft.payload().clone();
    let quote = client
        .quote_fees(&payload)
        .wrap_err("failed to quote exact contract-deployment transaction fees")?;
    if !requested_fee_payment.has_same_payer_and_gas_bound(&quote.intent) {
        return Err(eyre!(
            "fee quote changed the selected payer, sponsor revision, or gas bound; refusing to sign"
        ));
    }
    payload.fee_payment = quote.intent.clone();
    let transaction = client
        .try_sign_transaction_payload(payload)
        .wrap_err("failed to sign exact quoted contract-deployment payload")?;
    Ok((transaction, quote))
}

fn write_tx(out_dir: &Path, stem: &str, tx: &SignedTransaction) -> Result<(PathBuf, usize)> {
    fs::create_dir_all(out_dir)
        .wrap_err_with(|| format!("create output directory {}", out_dir.display()))?;
    let path = out_dir.join(format!("{stem}.norito"));
    let bytes = tx.encode_versioned();
    fs::write(&path, &bytes).wrap_err_with(|| format!("write {}", path.display()))?;
    Ok((path, bytes.len()))
}

fn main() -> Result<()> {
    let args = Args::parse();

    let fee_payment_bytes = fs::read(&args.fee_payment_json)
        .wrap_err_with(|| format!("read {}", args.fee_payment_json.display()))?;
    let fee_payment: FeePaymentIntent = norito::json::from_slice(&fee_payment_bytes)
        .wrap_err_with(|| format!("parse {}", args.fee_payment_json.display()))?;
    fee_payment
        .validate()
        .wrap_err("invalid fee payment intent")?;

    let authority = parse_account_address(&args.authority, Some(args.chain_discriminant))
        .wrap_err("failed to parse --authority as canonical account address")?
        .address
        .to_account_id()
        .map_err(|err| eyre!(err.to_string()))
        .wrap_err("failed to decode --authority")?;
    let private_key = read_private_key_file(&args.private_key_file)?;
    let key_pair = KeyPair::from(private_key.clone());
    let client = make_client(
        &args.torii_url,
        &args.chain_id,
        authority.clone(),
        args.chain_discriminant,
        key_pair.clone(),
        args.status_timeout_ms,
        args.transaction_ttl_ms,
        args.torii_request_timeout_ms,
    )?;

    let contract_alias: ContractAlias = args
        .contract_alias
        .parse()
        .wrap_err("failed to parse --contract-alias")?;
    let deployment_state = read_contract_deployment_state(
        &client,
        &authority,
        &contract_alias,
        args.chain_discriminant,
    )?;
    let deploy_nonce = deployment_state.deploy_nonce;
    let dataspace_id = deployment_state.dataspace_id;
    let expected_previous_contract_address = deployment_state.previous_contract_address.clone();
    let next_nonce = deploy_nonce
        .checked_add(1)
        .ok_or_else(|| eyre!("deploy nonce overflow"))?;
    let contract_address = iroha::data_model::smart_contract::ContractAddress::derive(
        &client.chain,
        &authority,
        deploy_nonce,
        dataspace_id,
    )
    .map_err(|err| eyre!(err.to_string()))
    .wrap_err("failed to derive contract address")?;

    let code =
        fs::read(&args.code_file).wrap_err_with(|| format!("read {}", args.code_file.display()))?;
    let verified = ivm::verify_contract_artifact(&code)
        .map_err(|err| eyre!("verify contract artifact: {err}"))?;
    let manifest = verified
        .manifest
        .try_signed(&key_pair)
        .wrap_err("failed to sign contract manifest")?;
    let code_hash = verified.code_hash;
    let transaction_ttl = args.transaction_ttl_ms.map(Duration::from_millis);
    // Registration and atomic commit are one governance operation. Bind every
    // transaction in the sequence to the same contract address and approver
    // set so protected-lane admission cannot observe a partially attributed
    // deployment.
    let tx_metadata =
        deployment_transaction_metadata(&contract_address, &args.gov_manifest_approvers)?;
    let signing = TransactionSigningContext {
        chain_id: &client.chain,
        authority: &authority,
        private_key: &private_key,
        transaction_ttl,
        fee_payment: &fee_payment,
        metadata: &tx_metadata,
    };
    let upload_plan = build_native_upload_transaction_plan(&signing, code_hash, &code)?;
    let mut fee_quotes = Vec::new();
    let upload_plan = if args.skip_register_bytes {
        upload_plan
    } else {
        let NativeUploadTransactionPlan {
            chunk_count,
            pre_stage,
            finalize,
        } = upload_plan;
        let mut quoted_pre_stage = Vec::with_capacity(pre_stage.len());
        for (name, slug, transaction) in pre_stage {
            let (transaction, quote) =
                quote_and_resign_transaction(&client, &transaction, &fee_payment)?;
            fee_quotes.push(quote);
            quoted_pre_stage.push((name, slug, transaction));
        }
        let (finalize_name, finalize_slug, finalize_transaction) = finalize;
        let (finalize_transaction, finalize_quote) =
            quote_and_resign_transaction(&client, &finalize_transaction, &fee_payment)?;
        fee_quotes.push(finalize_quote);
        NativeUploadTransactionPlan {
            chunk_count,
            pre_stage: quoted_pre_stage,
            finalize: (finalize_name, finalize_slug, finalize_transaction),
        }
    };
    let upload_report = native_upload_report(&upload_plan, args.skip_register_bytes);
    let NativeUploadTransactionPlan {
        pre_stage,
        finalize,
        ..
    } = upload_plan;
    let mut register_plans = pre_stage;
    register_plans.push(finalize);
    let register_manifest_tx = signing.sign([InstructionBox::from(RegisterSmartContractCode {
        manifest: manifest.clone(),
    })])?;
    let (register_manifest_tx, register_manifest_quote) =
        quote_and_resign_transaction(&client, &register_manifest_tx, &fee_payment)?;
    fee_quotes.push(register_manifest_quote);

    let commit_deployment_tx = build_commit_deployment_transaction(
        &signing,
        deploy_nonce,
        contract_address.clone(),
        code_hash,
        contract_alias.clone(),
        expected_previous_contract_address.clone(),
    )?;
    let (commit_deployment_tx, commit_deployment_quote) =
        quote_and_resign_transaction(&client, &commit_deployment_tx, &fee_payment)?;
    let commit_deployment_fee_payment = commit_deployment_quote.intent.clone();
    fee_quotes.push(commit_deployment_quote);

    let register_manifest_tx_hash = register_manifest_tx.hash();
    let commit_deployment_tx_hash = commit_deployment_tx.hash();
    let planned_txs = deployment_transaction_sequence(
        args.skip_register_bytes,
        register_plans,
        register_manifest_tx,
        commit_deployment_tx,
    );
    let written = if let Some(out_dir) = args.out_dir.as_deref() {
        Some(
            planned_txs
                .iter()
                .map(|(name, slug, tx)| Ok((name.as_str(), write_tx(out_dir, slug, tx)?)))
                .collect::<Result<Vec<_>>>()?,
        )
    } else {
        None
    };
    if !args.emit_only {
        for (name, _, tx) in &planned_txs {
            eprintln!("submitting {name} hash={}", tx.hash());
            client.submit_transaction_blocking(tx)?;
        }
    }

    let code_hash_hex = hex::encode(<[u8; 32]>::from(code_hash));
    let payload_digest_hex = hex::encode(blake3::hash(&code).as_bytes());
    let operation_status = if args.emit_only {
        "prepared"
    } else {
        "committed"
    };
    let deployment_state_snapshot = norito::json::to_value(&deployment_state.snapshot)
        .wrap_err("encode authenticated deployment-state snapshot")?;
    let authority_literal = authority.to_string();
    let contract_subject_account = contract_address.subject_id().to_string();
    let operation_receipt = norito::json!({
        "operation_kind": ("contract_deploy"),
        "status": (operation_status),
        "transport": ("ivm-contract-deploy-helper"),
        "torii_url": (args.torii_url.clone()),
        "chain_id": (args.chain_id.clone()),
        "authority": (authority_literal.clone()),
        "chain_discriminant": (args.chain_discriminant),
        "dataspace": (dataspace_id.to_string()),
        "contract_alias": (contract_alias.to_string()),
        "contract_address": (contract_address.to_string()),
        "contract_subject_account": (contract_subject_account.clone()),
        "code_hash_hex": (code_hash_hex.clone()),
        "abi_hash_hex": (Option::<String>::None),
        "tx_hash_hex": (commit_deployment_tx_hash.to_string()),
        "entrypoint": (Option::<String>::None),
        "entrypoint_hash_hex": (Option::<String>::None),
        "gas_limit": (commit_deployment_fee_payment.gas_limit().map(std::num::NonZeroU64::get)),
        "gas_used": (Option::<u64>::None),
        "fee_payment": (commit_deployment_fee_payment),
        "fee_quotes": (fee_quotes.clone()),
        "payload_digest_hex": (payload_digest_hex),
        "deployment_state": (deployment_state_snapshot.clone()),
    });
    let result = norito::json!({
        "ok": true,
        "submitted": (!args.emit_only),
        "torii_url": (args.torii_url),
        "chain_id": (args.chain_id),
        "authority": (authority_literal),
        "chain_discriminant": (args.chain_discriminant),
        "dataspace": (dataspace_id.to_string()),
        "contract_alias": (contract_alias),
        "contract_address": (contract_address),
        "contract_subject_account": (contract_subject_account),
        "deploy_nonce": (deploy_nonce),
        "next_deploy_nonce": (next_nonce),
        "code_hash_hex": (code_hash_hex.clone()),
        "register_manifest_tx_hash": (register_manifest_tx_hash),
        "commit_deployment_tx_hash": (commit_deployment_tx_hash),
        "expected_previous_contract_address": (expected_previous_contract_address),
        "deployment_state": (deployment_state_snapshot),
        "fee_quotes": (fee_quotes),
        "operation_receipt": (operation_receipt),
        "terminal_kind": (if args.emit_only { "Prepared" } else { "Committed" }),
        "final": (if args.emit_only {
            norito::json!({
                "kind": ("Prepared"),
                "hash": (commit_deployment_tx_hash),
            })
        } else {
            norito::json!({
                "kind": ("Committed"),
                "hash": (commit_deployment_tx_hash),
            })
        }),
    });
    let mut result = result
        .as_object()
        .cloned()
        .ok_or_else(|| eyre!("expected object"))?;
    let norito::json::Value::Object(upload_report) = upload_report else {
        unreachable!("native upload report is always an object");
    };
    result.extend(upload_report);
    if let Some(written) = written {
        let files = written
            .into_iter()
            .map(|(name, (path, size))| {
                norito::json!({
                    "name": (name),
                    "path": (path.display().to_string()),
                    "size": (size as u64),
                })
            })
            .collect();
        result.insert("files".to_owned(), norito::json::Value::Array(files));
    }
    println!(
        "{}",
        norito::json::to_json_pretty(&norito::json::Value::Object(result))?
    );
    Ok(())
}
