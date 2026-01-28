//! Contracts helpers.

use std::{path::PathBuf, str::FromStr};

use base64::Engine as _;
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    client::Client,
    data_model::{
        isi::smart_contract_code::{
            ActivateContractInstance, RegisterSmartContractBytes, RegisterSmartContractCode,
        },
        metadata::Metadata,
        name::Name,
        prelude::*,
        smart_contract::manifest::ContractManifest,
        transaction::{IvmBytecode, TransactionBuilder},
    },
};
use iroha_core::{
    pipeline::overlay::build_overlay_for_transaction_with_accounts,
    smartcontracts::ivm::cache::ProgramSummary,
};
use iroha_crypto::{Hash, KeyPair, PrivateKey};

use crate::{Run, RunContext};

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Contract code helpers
    #[command(subcommand)]
    Code(CodeCommand),
    /// Deploy compiled `.to` code via Torii (POST /v1/contracts/deploy)
    Deploy(DeployArgs),
    /// Deploy bytecode, register manifest, and activate a namespace binding in one transaction
    DeployActivate(DeployActivateArgs),
    /// Contract manifest helpers
    #[command(subcommand)]
    Manifest(ManifestCommand),
    /// Run an offline simulation of IVM bytecode to see the queued ISIs and header metadata
    Simulate(SimulateArgs),
    /// List active contract instances in a namespace (supports filters and pagination)
    Instances(InstancesArgs),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Code(cmd) => cmd.run(context),
            Command::Deploy(args) => args.run(context),
            Command::DeployActivate(args) => args.run(context),
            Command::Manifest(cmd) => cmd.run(context),
            Command::Simulate(args) => args.run(context),
            Command::Instances(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum CodeCommand {
    /// Fetch on-chain contract code bytes by code hash and write to a file
    Get(CodeBytesGetArgs),
}

impl Run for CodeCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            CodeCommand::Get(args) => args.run(context),
        }
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum ManifestCommand {
    /// Fetch on-chain contract manifest by code hash and either print or save (if --out is provided)
    Get(ManifestArgs),
    /// Build a manifest for compiled bytecode (with optional signing)
    Build(BuildManifestArgs),
}

impl Run for ManifestCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            ManifestCommand::Get(args) => args.run(context),
            ManifestCommand::Build(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct CodeBytesGetArgs {
    /// Hex-encoded 32-byte code hash (0x optional)
    #[arg(long, value_name = "HEX64")]
    pub code_hash: String,
    /// Output path to write the `.to` bytes
    #[arg(long, value_name = "PATH")]
    pub out: PathBuf,
}

impl Run for CodeBytesGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let code_hash = self.code_hash.trim_start_matches("0x");
        let bytes = client.get_contract_code_bytes(code_hash)?;
        std::fs::write(&self.out, &bytes)?;
        context.println(format_args!(
            "Wrote {} bytes to {}",
            bytes.len(),
            self.out.display()
        ))?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct DeployArgs {
    /// Authority account identifier (IH58 (preferred)/sora (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
    #[arg(long)]
    pub authority: String,
    /// Hex-encoded private key for signing
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Path to compiled `.to` file (mutually exclusive with --code-b64)
    #[arg(long, conflicts_with = "code_b64")]
    pub code_file: Option<PathBuf>,
    /// Base64-encoded code (mutually exclusive with --code-file)
    #[arg(long, conflicts_with = "code_file")]
    pub code_b64: Option<String>,
}

impl Run for DeployArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        // Parse authority and key
        let authority =
            crate::resolve_account_id(context, &self.authority).wrap_err("failed to resolve --authority")?;
        let private_key: iroha_crypto::PrivateKey =
            self.private_key.parse().wrap_err("invalid --private-key")?;
        // Obtain base64 code
        let code_b64 = if let Some(p) = self.code_file {
            let bytes = std::fs::read(&p).wrap_err("read --code-file")?;
            base64::engine::general_purpose::STANDARD.encode(bytes)
        } else if let Some(s) = self.code_b64 {
            s
        } else {
            return Err(eyre!("either --code-file or --code-b64 must be provided"));
        };
        let v = client.post_contract_deploy_json(&authority, &private_key, &code_b64)?;
        context.print_data(&v)?;
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct DeployActivateArgs {
    /// Authority account identifier (IH58 (preferred)/sora (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
    #[arg(long)]
    pub authority: String,
    /// Hex-encoded private key for signing and manifest provenance
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Governance namespace to bind (e.g., apps)
    #[arg(long)]
    pub namespace: String,
    /// Contract identifier within the namespace
    #[arg(long, value_name = "ID")]
    pub contract_id: String,
    /// Path to compiled `.to` file (mutually exclusive with --code-b64)
    #[arg(long, conflicts_with = "code_b64")]
    pub code_file: Option<PathBuf>,
    /// Base64-encoded code (mutually exclusive with --code-file)
    #[arg(long, conflicts_with = "code_file")]
    pub code_b64: Option<String>,
    /// Optional path to write the manifest JSON used in the transaction
    #[arg(long, value_name = "PATH")]
    pub manifest_out: Option<PathBuf>,
    /// Preview transaction contents without submitting
    #[arg(long)]
    pub dry_run: bool,
}

impl Run for DeployActivateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let _authority =
            crate::resolve_account_id(context, &self.authority).wrap_err("failed to resolve --authority")?;
        let private_key: PrivateKey = self.private_key.parse().wrap_err("invalid --private-key")?;
        let keypair =
            KeyPair::from_private_key(private_key.clone()).wrap_err("derive keypair failed")?;
        let code = load_code_bytes(self.code_file.clone(), self.code_b64.clone())?;
        let summary = program_summary_from_bytes(&code)?;

        let manifest = ContractManifest {
            code_hash: Some(summary.code_hash),
            abi_hash: Some(summary.abi_hash),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&keypair);

        if let Some(path) = self.manifest_out.as_ref() {
            let json = norito::json::to_json_pretty(&manifest)?;
            std::fs::write(path, json.as_bytes())
                .wrap_err_with(|| format!("write manifest to {}", path.display()))?;
        }

        let instructions: Vec<InstructionBox> = vec![
            RegisterSmartContractBytes {
                code_hash: summary.code_hash,
                code: code.clone(),
            }
            .into(),
            RegisterSmartContractCode {
                manifest: manifest.clone(),
            }
            .into(),
            ActivateContractInstance {
                namespace: self.namespace.clone(),
                contract_id: self.contract_id.clone(),
                code_hash: summary.code_hash,
            }
            .into(),
        ];

        let mut metadata = context
            .transaction_metadata()
            .cloned()
            .unwrap_or_else(Metadata::default);
        metadata.insert(
            Name::from_str("gov_namespace")?,
            iroha_primitives::json::Json::from(self.namespace.as_str()),
        );
        metadata.insert(
            Name::from_str("gov_contract_id")?,
            iroha_primitives::json::Json::from(self.contract_id.as_str()),
        );
        metadata.insert(
            Name::from_str("contract_namespace")?,
            iroha_primitives::json::Json::from(self.namespace.as_str()),
        );
        metadata.insert(
            Name::from_str("contract_id")?,
            iroha_primitives::json::Json::from(self.contract_id.as_str()),
        );

        if self.dry_run {
            let mut preview = norito::json::Map::new();
            preview.insert(
                "code_hash_hex".to_string(),
                norito::json::to_value(&hex::encode(summary.code_hash.as_ref()))?,
            );
            preview.insert(
                "abi_hash_hex".to_string(),
                norito::json::to_value(&hex::encode(summary.abi_hash.as_ref()))?,
            );
            preview.insert(
                "namespace".to_string(),
                norito::json::to_value(&self.namespace)?,
            );
            preview.insert(
                "contract_id".to_string(),
                norito::json::to_value(&self.contract_id)?,
            );
            preview.insert(
                "instruction_count".to_string(),
                norito::json::to_value(&instructions.len())?,
            );
            let preview = norito::json::Value::Object(preview);
            context.print_data(&preview)?;
            return Ok(());
        }

        context.submit_with_metadata(instructions, metadata, true)
    }
}

#[derive(clap::Args, Debug)]
pub struct BuildManifestArgs {
    /// Path to compiled `.to` file (mutually exclusive with --code-b64)
    #[arg(long, conflicts_with = "code_b64")]
    pub code_file: Option<PathBuf>,
    /// Base64-encoded code (mutually exclusive with --code-file)
    #[arg(long, conflicts_with = "code_file")]
    pub code_b64: Option<String>,
    /// Hex-encoded private key for signing the manifest (optional)
    #[arg(long, value_name = "HEX")]
    pub sign_with: Option<String>,
    /// Optional output path; if omitted, prints to stdout
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
}

impl Run for BuildManifestArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let code = load_code_bytes(self.code_file.clone(), self.code_b64.clone())?;
        let summary = program_summary_from_bytes(&code)?;
        let mut manifest = ContractManifest {
            code_hash: Some(summary.code_hash),
            abi_hash: Some(summary.abi_hash),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        };
        if let Some(hex_key) = self.sign_with {
            let private: PrivateKey = hex_key.parse().wrap_err("invalid --sign-with")?;
            let kp =
                KeyPair::from_private_key(private).wrap_err("derive signing keypair failed")?;
            manifest = manifest.signed(&kp);
        }
        let rendered = norito::json::to_json_pretty(&manifest)?;
        if let Some(path) = self.out {
            std::fs::write(&path, rendered.as_bytes())
                .wrap_err_with(|| format!("write manifest to {}", path.display()))?;
            context.println(format_args!("Wrote manifest to {}", path.display()))?;
        } else {
            context.println(rendered)?;
        }
        Ok(())
    }
}

fn load_code_bytes(code_file: Option<PathBuf>, code_b64: Option<String>) -> Result<Vec<u8>> {
    if let Some(path) = code_file {
        let bytes = std::fs::read(&path).wrap_err_with(|| format!("read {}", path.display()))?;
        Ok(bytes)
    } else if let Some(s) = code_b64 {
        base64::engine::general_purpose::STANDARD
            .decode(s.as_bytes())
            .wrap_err("decode base64 code payload")
    } else {
        Err(eyre!("either --code-file or --code-b64 must be provided"))
    }
}

fn program_summary_from_bytes(bytes: &[u8]) -> Result<ProgramSummary> {
    let parsed = ivm::ProgramMetadata::parse(bytes)
        .map_err(|err| eyre!("failed to parse IVM metadata: {err}"))?;
    let code_hash = Hash::new(&bytes[parsed.header_len..]);
    let metadata = parsed.metadata;
    let policy = match metadata.abi_version {
        1 => ivm::SyscallPolicy::AbiV1,
        v => {
            return Err(eyre!(
                "unsupported abi_version {v}; expected 1 for the first release"
            ));
        }
    };
    let abi_hash = Hash::prehashed(ivm::syscalls::compute_abi_hash(policy));
    let meta_hash = Hash::new(metadata.encode());
    Ok(ProgramSummary {
        metadata,
        code_hash,
        abi_hash,
        meta_hash,
        header_len: parsed.header_len,
        code_offset: parsed.code_offset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, ExposedPrivateKey};
    use iroha_i18n::{Bundle, Language, Localizer};
    use url::Url;

    fn minimal_program() -> Vec<u8> {
        let meta = ivm::ProgramMetadata {
            max_cycles: 1,
            ..ivm::ProgramMetadata::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        program
    }

    #[test]
    fn program_summary_reports_hashes() {
        let program = minimal_program();
        let parsed = ivm::ProgramMetadata::parse(&program).expect("parse");
        let expected_code_hash = Hash::new(&program[parsed.header_len..]);
        let summary = program_summary_from_bytes(&program).expect("summary");
        assert_eq!(summary.code_hash, expected_code_hash);
        assert_eq!(
            summary.abi_hash,
            Hash::prehashed(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
        );
    }

    #[test]
    fn simulate_emits_gas_limit_metadata_key() {
        let key_pair = KeyPair::from_seed(vec![1u8; 32], Algorithm::Ed25519);
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            key_pair.public_key().clone(),
        );
        let mut ctx = TestContext::new(authority.clone());
        let program = minimal_program();
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);
        let private_key = ExposedPrivateKey(key_pair.private_key().clone()).to_string();
        let args = SimulateArgs {
            authority: authority.to_string(),
            private_key,
            code_file: None,
            code_b64: Some(code_b64),
            gas_limit: 42,
            namespace: None,
            contract_id: None,
        };
        args.run(&mut ctx).expect("simulate");
        let output = ctx.take_output().expect("output");
        let metadata_keys = output
            .get("metadata_keys")
            .and_then(norito::json::Value::as_array)
            .expect("metadata_keys");
        let has_gas_limit = metadata_keys
            .iter()
            .any(|value| value.as_str() == Some("gas_limit"));
        assert!(has_gas_limit, "metadata_keys missing gas_limit: {metadata_keys:?}");
    }

    struct TestContext {
        cfg: iroha::config::Config,
        output: Option<norito::json::Value>,
        i18n: Localizer,
    }

    impl TestContext {
        fn new(account: AccountId) -> Self {
            let key_pair = KeyPair::from_seed(vec![0u8; 32], Algorithm::Ed25519);
            let cfg = iroha::config::Config {
                chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
                account,
                key_pair,
                basic_auth: None,
                torii_api_url: Url::parse("http://127.0.0.1/").unwrap(),
                torii_api_version: iroha::config::default_torii_api_version(),
                torii_api_min_proof_version:
                    iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION.to_string(),
                torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: iroha::config::default_connect_queue_root(),
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                output: None,
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }

        fn take_output(&mut self) -> Option<norito::json::Value> {
            self.output.take()
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
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

        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: norito::json::JsonSerialize + ?Sized,
        {
            self.output = Some(norito::json::to_value(data)?);
            Ok(())
        }

        fn println(&mut self, _data: impl std::fmt::Display) -> Result<()> {
            Ok(())
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct SimulateArgs {
    /// Authority account identifier (IH58 (preferred)/sora (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
    #[arg(long)]
    pub authority: String,
    /// Hex-encoded private key used to sign the simulated transaction
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Path to compiled `.to` file (mutually exclusive with --code-b64)
    #[arg(long, conflicts_with = "code_b64")]
    pub code_file: Option<PathBuf>,
    /// Base64-encoded code (mutually exclusive with --code-file)
    #[arg(long, conflicts_with = "code_file")]
    pub code_b64: Option<String>,
    /// Required `gas_limit` metadata to include in the simulated transaction
    #[arg(long)]
    pub gas_limit: u64,
    /// Optional contract namespace metadata for call-time binding checks
    #[arg(long)]
    pub namespace: Option<String>,
    /// Optional contract identifier metadata for call-time binding checks
    #[arg(long)]
    pub contract_id: Option<String>,
}

impl Run for SimulateArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let authority =
            crate::resolve_account_id(context, &self.authority).wrap_err("failed to resolve --authority")?;
        let private_key: PrivateKey = self.private_key.parse().wrap_err("invalid --private-key")?;
        let code = load_code_bytes(self.code_file.clone(), self.code_b64.clone())?;
        let summary = program_summary_from_bytes(&code)?;

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("gas_limit")?,
            iroha_primitives::json::Json::from(self.gas_limit),
        );
        if let Some(ns) = self.namespace.as_ref() {
            metadata.insert(
                Name::from_str("contract_namespace")?,
                iroha_primitives::json::Json::from(ns.as_str()),
            );
        }
        if let Some(cid) = self.contract_id.as_ref() {
            metadata.insert(
                Name::from_str("contract_id")?,
                iroha_primitives::json::Json::from(cid.as_str()),
            );
        }

        let chain_id = context.config().chain.clone();
        let tx = TransactionBuilder::new(chain_id, authority.clone())
            .with_metadata(metadata.clone())
            .with_executable(Executable::Ivm(IvmBytecode::from_compiled(code.clone())))
            .sign(&private_key);

        let decoded = ivm::ivm_cache::IvmCache::decode_stream(&code[summary.code_offset..])
            .map_err(|err| eyre!("instruction decode failed: {err}"))?;
        let decoded_bytes: u64 = decoded.iter().map(|op| u64::from(op.len)).sum::<u64>();

        let overlay =
            build_overlay_for_transaction_with_accounts(&tx, std::slice::from_ref(&authority))
                .map_err(|err| eyre!("simulation overlay failed: {err}"))?;

        let instruction_ids: Vec<String> =
            overlay.instructions().map(|i| i.id().to_string()).collect();
        let metadata_keys: Vec<String> = metadata
            .iter()
            .map(|(name, _)| name.as_ref().to_string())
            .collect();
        let mut summary_json = norito::json::Map::new();
        summary_json.insert(
            "code_hash_hex".to_string(),
            norito::json::to_value(&hex::encode(summary.code_hash.as_ref()))?,
        );
        summary_json.insert(
            "abi_hash_hex".to_string(),
            norito::json::to_value(&hex::encode(summary.abi_hash.as_ref()))?,
        );
        summary_json.insert(
            "abi_version".to_string(),
            norito::json::to_value(&summary.metadata.abi_version)?,
        );
        summary_json.insert(
            "max_cycles".to_string(),
            norito::json::to_value(&summary.metadata.max_cycles)?,
        );
        summary_json.insert(
            "decoded_instructions".to_string(),
            norito::json::to_value(&decoded.len())?,
        );
        summary_json.insert(
            "decoded_code_bytes".to_string(),
            norito::json::to_value(&decoded_bytes)?,
        );
        summary_json.insert(
            "queued_instruction_count".to_string(),
            norito::json::to_value(&overlay.instruction_count())?,
        );
        summary_json.insert(
            "instruction_ids".to_string(),
            norito::json::to_value(&instruction_ids)?,
        );
        summary_json.insert(
            "metadata_keys".to_string(),
            norito::json::to_value(&metadata_keys)?,
        );
        let summary_json = norito::json::Value::Object(summary_json);
        context.print_data(&summary_json)?;
        Ok(())
    }
}

// Unified Manifest handling supersedes earlier subcommands

#[derive(clap::Args, Debug)]
pub struct ManifestArgs {
    /// Hex-encoded 32-byte code hash (0x optional)
    #[arg(long, value_name = "HEX64")]
    pub code_hash: String,
    /// Optional output path; if provided, writes JSON manifest to file, otherwise prints to stdout
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
}

impl Run for ManifestArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let code_hash = self.code_hash.trim_start_matches("0x");
        let v = client.get_contract_manifest_json(code_hash)?;
        if let Some(p) = self.out {
            let s = norito::json::to_json_pretty(&v)?;
            std::fs::write(&p, s.as_bytes())?;
            context.println(format_args!("Wrote manifest to {}", p.display()))?;
        } else {
            context.print_data(&v)?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct InstancesArgs {
    /// Namespace to list (e.g., apps)
    #[arg(long, value_name = "NS")]
    pub namespace: String,
    /// Filter: `contract_id` substring (case-sensitive)
    #[arg(long)]
    pub contains: Option<String>,
    /// Filter: code hash hex prefix (lowercase)
    #[arg(long)]
    pub hash_prefix: Option<String>,
    /// Pagination offset
    #[arg(long)]
    pub offset: Option<u32>,
    /// Pagination limit
    #[arg(long)]
    pub limit: Option<u32>,
    /// Order: `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
    #[arg(long)]
    pub order: Option<String>,
    /// Render as a table instead of raw JSON
    #[arg(long)]
    pub table: bool,
    /// When rendering a table, truncate the code hash (first 12 hex chars with ellipsis)
    #[arg(long)]
    pub short_hash: bool,
}

impl Run for InstancesArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_contracts_instances_by_ns_filtered_json(
            &self.namespace,
            self.contains.as_deref(),
            self.hash_prefix.as_deref(),
            self.offset,
            self.limit,
            self.order.as_deref(),
        )?;
        if self.table {
            // Pretty table renderer
            use norito::json::Value;
            let ns = value.get("namespace").and_then(Value::as_str).unwrap_or("");
            let total = value.get("total").and_then(Value::as_u64).unwrap_or(0);
            let offset = value.get("offset").and_then(Value::as_u64).unwrap_or(0);
            let limit = value.get("limit").and_then(Value::as_u64).unwrap_or(0);
            let rows = value
                .get("instances")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            // Compute widths
            let mut w_ns = "Namespace".len();
            let mut w_id = "Contract ID".len();
            let mut w_hash = "Code Hash".len();
            w_ns = w_ns.max(ns.len());
            let mut items: Vec<(String, String)> = Vec::new();
            for row in rows {
                if let Value::Object(m) = row {
                    let cid = m
                        .get("contract_id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let full_hash = m
                        .get("code_hash_hex")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let hash = if self.short_hash && full_hash.len() > 12 {
                        format!("{}…", &full_hash[..12])
                    } else {
                        full_hash
                    };
                    w_id = w_id.max(cid.len());
                    w_hash = w_hash.max(hash.len());
                    items.push((cid, hash));
                }
            }
            let sep = format!(
                "+-{:-<ns$}-+-{:-<id$}-+-{:-<hash$}-+",
                "",
                "",
                "",
                ns = w_ns,
                id = w_id,
                hash = w_hash
            );
            // Header
            context.println(format!(
                "Namespace: {ns}  (total={total}, offset={offset}, limit={limit})"
            ))?;
            context.println(&sep)?;
            context.println(format!(
                "| {namespace:<ns_width$} | {contract:<id_width$} | {hash_label:<hash_width$} |",
                namespace = "Namespace",
                contract = "Contract ID",
                hash_label = "Code Hash",
                ns_width = w_ns,
                id_width = w_id,
                hash_width = w_hash
            ))?;
            context.println(&sep)?;
            // Rows
            let ns_width = w_ns;
            let id_width = w_id;
            let hash_width = w_hash;
            for (cid, hash) in items {
                context.println(format!(
                    "| {ns:<ns_width$} | {cid:<id_width$} | {hash:<hash_width$} |"
                ))?;
            }
            context.println(&sep)?;
        } else {
            context.print_data(&value)?;
        }
        Ok(())
    }
}
