//! Governance deployment CLI helpers.

use super::shared::{decode_hex32, print_with_summary};
use crate::{
    Run, RunContext,
    json_utils::{json_array, json_object, json_value},
};
use eyre::{Result, eyre};
use iroha::client::Client;
use iroha::data_model::{
    isi::{InstructionBox, SetParameter},
    name::Name,
    parameter::{CustomParameterId, Parameter, custom::CustomParameter},
};

#[derive(clap::Args, Debug)]
pub struct ProposeDeployArgs {
    #[arg(long)]
    pub namespace: String,
    #[arg(long, value_name = "ID")]
    pub contract_id: String,
    #[arg(long)]
    pub code_hash: String,
    #[arg(long)]
    pub abi_hash: String,
    #[arg(long, default_value = "v1")]
    pub abi_version: String,
    /// Optional window lower bound (height)
    #[arg(long)]
    pub window_lower: Option<u64>,
    /// Optional window upper bound (height)
    #[arg(long)]
    pub window_upper: Option<u64>,
    /// Optional voting mode for the referendum: Zk or Plain (defaults to server policy)
    #[arg(long, value_name = "MODE", value_parser = ["Zk", "Plain"])]
    pub mode: Option<String>,
}

impl Run for ProposeDeployArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        // Normalize and validate mode
        let mode_norm = if let Some(mode) = self.mode.as_deref() {
            match mode.to_ascii_lowercase().as_str() {
                "zk" => Some("Zk".to_string()),
                "plain" => Some("Plain".to_string()),
                other => {
                    return Err(eyre!(
                        "invalid --mode '{}'; expected 'Zk' or 'Plain'",
                        other
                    ));
                }
            }
        } else {
            None
        };
        let window = match (self.window_lower, self.window_upper) {
            (Some(lower), Some(upper)) => json_object(vec![
                ("lower", json_value(&lower)?),
                ("upper", json_value(&upper)?),
            ])?,
            _ => norito::json::Value::Null,
        };
        let mode_value = json_value(&mode_norm)?;
        let body = json_object(vec![
            ("namespace", json_value(&self.namespace)?),
            ("contract_id", json_value(&self.contract_id)?),
            ("code_hash", json_value(&self.code_hash)?),
            ("abi_hash", json_value(&self.abi_hash)?),
            ("abi_version", json_value(&self.abi_version)?),
            ("window", window),
            ("mode", mode_value),
        ])?;
        let value = client.post_gov_propose_deploy_json(&body)?;
        let ok = value
            .get("ok")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let pid = value
            .get("proposal_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let summary = Some(format!("deploy propose: ok={ok} proposal_id={pid}"));
        print_with_summary(context, summary, &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct FinalizeArgs {
    /// Referendum id
    #[arg(long)]
    pub referendum_id: String,
    /// Proposal id (hex 64)
    #[arg(long, value_name = "ID_HEX")]
    pub proposal_id: String,
}

fn build_finalize_body(args: &FinalizeArgs) -> Result<norito::json::Value> {
    json_object(vec![
        ("referendum_id", json_value(&args.referendum_id)?),
        ("proposal_id", json_value(&args.proposal_id)?),
    ])
}

impl Run for FinalizeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let body = build_finalize_body(&self)?;
        let value = client.post_gov_finalize_json(&body)?;
        let ok = value
            .get("ok")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let n_instr = value
            .get("tx_instructions")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let summary = Some(format!(
            "finalize: referendum_id={} ok={ok} tx_instrs={n_instr}",
            self.referendum_id
        ));
        print_with_summary(context, summary, &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct EnactArgs {
    /// Proposal id (hex 64)
    #[arg(long, value_name = "ID_HEX")]
    pub proposal_id: String,
    /// Optional preimage hash (hex 64)
    #[arg(long)]
    pub preimage_hash: Option<String>,
    /// Optional window lower bound (height)
    #[arg(long)]
    pub window_lower: Option<u64>,
    /// Optional window upper bound (height)
    #[arg(long)]
    pub window_upper: Option<u64>,
}

impl Run for EnactArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let window = match (self.window_lower, self.window_upper) {
            (Some(lower), Some(upper)) => json_object(vec![
                ("lower", json_value(&lower)?),
                ("upper", json_value(&upper)?),
            ])?,
            _ => norito::json::Value::Null,
        };
        let body = json_object(vec![
            ("proposal_id", json_value(&self.proposal_id)?),
            ("preimage_hash", json_value(&self.preimage_hash)?),
            ("window", window),
        ])?;
        let value = client.post_gov_enact_json(&body)?;
        let ok = value
            .get("ok")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let n_instr = value
            .get("tx_instructions")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let summary = Some(format!(
            "enact: proposal_id={} ok={ok} tx_instrs={n_instr}",
            self.proposal_id
        ));
        print_with_summary(context, summary, &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct ProtectedSetArgs {
    /// Comma-separated namespaces (e.g., apps,system)
    #[arg(long)]
    pub namespaces: String,
}

impl Run for ProtectedSetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        // Build a SetParameter(Custom) instruction for gov_protected_namespaces
        let name: Name = "gov_protected_namespaces".parse()?;
        let id = CustomParameterId(name);
        let arr: Vec<String> = self
            .namespaces
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let json_arr = json_value(&arr)?;
        let custom = CustomParameter::new(id, iroha_primitives::json::Json::from(json_arr));
        let isi = SetParameter::new(Parameter::Custom(custom));
        let boxed: InstructionBox = isi.into();
        let bytes = norito::to_bytes(&boxed)?;
        let (wire_id, payload_bytes) = norito::decode_from_bytes::<(String, Vec<u8>)>(&bytes)?;
        let payload_hex = hex::encode(payload_bytes);
        let tx_instruction = json_object(vec![
            ("wire_id", json_value(&wire_id)?),
            ("payload_hex", json_value(&payload_hex)?),
        ])?;
        let tx_instructions = json_array(vec![tx_instruction])?;
        let out = json_object(vec![
            ("ok", json_value(&true)?),
            ("tx_instructions", tx_instructions),
        ])?;
        let count = self
            .namespaces
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .count();
        let summary = Some(format!("protected set: namespaces_count={count}"));
        print_with_summary(context, summary, &out)
    }
}

#[derive(clap::Args, Debug)]
pub struct ProtectedApplyArgs {
    /// Comma-separated namespaces (e.g., apps,system)
    #[arg(long)]
    pub namespaces: String,
}

impl Run for ProtectedApplyArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let namespaces: Vec<String> = self
            .namespaces
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let value = client.post_gov_protected_set_json(&namespaces)?;
        let ok = value
            .get("ok")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let applied = value
            .get("applied")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let summary = Some(format!("protected apply: ok={ok} applied={applied}"));
        print_with_summary(context, summary, &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct ProtectedGetArgs {}

impl Run for ProtectedGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_gov_protected_namespaces_json()?;
        let found = value
            .get("found")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let count = value
            .get("namespaces")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let summary = Some(format!("protected get: found={found} count={count}"));
        print_with_summary(context, summary, &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct ActivateInstanceArgs {
    #[arg(long)]
    pub namespace: String,
    #[arg(long)]
    pub contract_id: String,
    /// code hash hex (64 chars, 0x optional)
    #[arg(long, value_name = "HEX64")]
    pub code_hash: String,
    /// Submit and wait until committed or rejected
    #[arg(long, default_value_t = false)]
    pub blocking: bool,
}

impl Run for ActivateInstanceArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let Self {
            namespace,
            contract_id,
            code_hash,
            blocking,
        } = self;
        let hash_bytes = decode_hex32(&code_hash)?;
        let code_hash_hex = format!("0x{}", hex::encode(hash_bytes));
        if blocking {
            let instruction =
                iroha::data_model::isi::smart_contract_code::ActivateContractInstance {
                    namespace,
                    contract_id,
                    code_hash: iroha_crypto::Hash::prehashed(hash_bytes),
                };
            let boxed: iroha::data_model::isi::InstructionBox = instruction.into();
            context.finish([boxed])?;
        } else {
            let out = json_object(vec![
                ("action", json_value("ActivateContractInstance")?),
                ("namespace", json_value(&namespace)?),
                ("contract_id", json_value(&contract_id)?),
                ("code_hash", json_value(&code_hash_hex)?),
                (
                    "hint",
                    json_value(
                        "pass --blocking to submit via CLI context; otherwise this is a skeleton payload",
                    )?,
                ),
            ])?;
            context.print_data(&out)?;
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
}

impl Run for InstancesArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_gov_instances_by_ns_filtered_json(
            &self.namespace,
            self.contains.as_deref(),
            self.hash_prefix.as_deref(),
            self.offset,
            self.limit,
            self.order.as_deref(),
        )?;
        let total = value
            .get("total")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let offset = value
            .get("offset")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let limit = value
            .get("limit")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let summary = Some(format!(
            "instance list: namespace={} total={} offset={} limit={}",
            self.namespace, total, offset, limit
        ));
        print_with_summary(context, summary, &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct DeployMetaArgs {
    #[arg(long)]
    pub namespace: String,
    #[arg(long)]
    pub contract_id: String,
    /// Optional validator account IDs (canonical I105 account literals) authorizing the deployment alongside the authority.
    #[arg(long = "approver", value_name = "ACCOUNT")]
    pub approvers: Vec<String>,
}

impl Run for DeployMetaArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let mut pairs = vec![
            ("gov_namespace", json_value(&self.namespace)?),
            ("gov_contract_id", json_value(&self.contract_id)?),
        ];

        if !self.approvers.is_empty() {
            let mut accounts = Vec::with_capacity(self.approvers.len());
            for (idx, raw) in self.approvers.iter().enumerate() {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    return Err(eyre!(format!("--approver[{idx}] must not be blank")));
                }
                let account = crate::resolve_account_id(context, trimmed)
                    .map_err(|err| eyre!("invalid --approver[{idx}] `{trimmed}`: {err}"))?;
                accounts.push(account.to_string());
            }
            pairs.push(("gov_manifest_approvers", json_array(accounts)?));
        }

        let obj = json_object(pairs)?;
        context.print_data(&obj)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blake3::Hasher as Blake3Hasher;
    use iroha::config::Config;
    use iroha::crypto::{Algorithm, KeyPair};
    use iroha::data_model::isi::InstructionBox;
    use iroha::data_model::{
        ChainId,
        account::AccountId,
        metadata::Metadata,
        transaction::{Executable, IvmBytecode},
    };
    use iroha_i18n::{Bundle, Language, Localizer};
    use norito::json::JsonSerialize;
    use url::Url;

    struct TestContext {
        cfg: Config,
        printed: Vec<norito::json::Value>,
        submitted: Option<Vec<InstructionBox>>,
        i18n: Localizer,
    }

    impl TestContext {
        fn new() -> Self {
            let key_pair = KeyPair::from_seed(vec![0u8; 32], Algorithm::Ed25519);
            let account = AccountId::new(key_pair.public_key().clone());
            let cfg = Config {
                chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
                account,
                account_chain_discriminant:
                    iroha_config::parameters::defaults::common::chain_discriminant(),
                key_pair,
                basic_auth: None,
                torii_api_url: Url::parse("http://127.0.0.1/").unwrap(),
                torii_api_version: iroha::config::default_torii_api_version(),
                torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: iroha::config::default_connect_queue_root(),
                soracloud_http_witness_file: None,
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                printed: Vec::new(),
                submitted: None,
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }
    }

    impl RunContext for TestContext {
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

        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            self.printed.push(norito::json::to_value(data)?);
            Ok(())
        }

        fn println(&mut self, _data: impl std::fmt::Display) -> Result<()> {
            Ok(())
        }

        fn submit_with_metadata(
            &mut self,
            instructions: impl Into<Executable>,
            _metadata: Metadata,
            _wait_for_confirmation: bool,
        ) -> Result<()> {
            self.submit(instructions)
        }

        fn submit(&mut self, instructions: impl Into<Executable>) -> Result<()> {
            match instructions.into() {
                Executable::Instructions(list) => {
                    self.submitted = Some(list.into_vec());
                    Ok(())
                }
                Executable::Ivm(IvmBytecode { .. }) => {
                    eyre::bail!("unexpected IVM bytecode submission in test context")
                }
                Executable::IvmProved(_) => {
                    eyre::bail!("unexpected proved IVM submission in test context")
                }
            }
        }
    }

    #[test]
    fn deploy_meta_args_outputs_expected_keys() {
        let mut ctx = TestContext::new();
        let args = DeployMetaArgs {
            namespace: "apps".into(),
            contract_id: "calc.v1".into(),
            approvers: Vec::new(),
        };
        args.run(&mut ctx).expect("deploy-meta run");
        assert_eq!(ctx.printed.len(), 1);
        let value = &ctx.printed[0];
        assert_eq!(
            value.get("gov_namespace").and_then(|v| v.as_str()),
            Some("apps")
        );
        assert_eq!(
            value.get("gov_contract_id").and_then(|v| v.as_str()),
            Some("calc.v1")
        );
        assert!(value.get("gov_manifest_approvers").is_none());
    }

    #[test]
    fn deploy_meta_args_accepts_manifest_approvers() {
        let mut ctx = TestContext::new();
        let validator = sample_account_string("validator");
        let bob = sample_account_string("bob");
        let args = DeployMetaArgs {
            namespace: "apps".into(),
            contract_id: "calc.v1".into(),
            approvers: vec![validator.clone(), format!("   {bob}   ")],
        };
        args.run(&mut ctx).expect("deploy-meta run");
        let value = &ctx.printed[0];
        let approvers = value
            .get("gov_manifest_approvers")
            .and_then(|v| v.as_array())
            .expect("manifest approver array");
        let collected: Vec<_> = approvers
            .iter()
            .map(|entry| entry.as_str().unwrap_or(""))
            .collect();
        assert_eq!(collected, vec![validator, bob]);
    }

    #[test]
    fn deploy_meta_args_rejects_invalid_approver() {
        let mut ctx = TestContext::new();
        let args = DeployMetaArgs {
            namespace: "apps".into(),
            contract_id: "calc.v1".into(),
            approvers: vec!["not-an-id".into()],
        };
        let err = args
            .run(&mut ctx)
            .expect_err("invalid approver should fail");
        assert!(
            err.to_string().contains("invalid --approver[0]"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn deploy_meta_args_rejects_legacy_approver_with_domain_suffix() {
        let mut ctx = TestContext::new();
        let args = DeployMetaArgs {
            namespace: "apps".into(),
            contract_id: "calc.v1".into(),
            approvers: vec!["alice@invalid-domain".into()],
        };
        let err = args
            .run(&mut ctx)
            .expect_err("legacy approver literal should fail");
        assert!(
            err.to_string().contains("must not include '@domain'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn finalize_body_shape() {
        let args = FinalizeArgs {
            referendum_id: "ref-123".to_string(),
            proposal_id: "0x".to_string() + &"aa".repeat(32),
        };
        let body = build_finalize_body(&args).expect("build finalize body");
        let s = norito::json::to_json(&body).expect("serialize body");
        let v: norito::json::Value = norito::json::from_str(&s).expect("roundtrip");
        assert_eq!(v["referendum_id"].as_str(), Some("ref-123"));
        assert_eq!(v["proposal_id"].as_str().unwrap().len(), 66);
    }

    #[test]
    fn activate_instance_skeleton_prints_json() {
        let mut ctx = TestContext::new();
        let args = ActivateInstanceArgs {
            namespace: "apps".to_string(),
            contract_id: "calc.v1".to_string(),
            code_hash: "0x".to_string() + &"ab".repeat(32),
            blocking: false,
        };
        args.run(&mut ctx).expect("run skeleton path");
        assert!(
            ctx.submitted.is_none(),
            "should not submit without --blocking"
        );
        assert_eq!(ctx.printed.len(), 1, "expect single JSON output");
        let value = &ctx.printed[0];
        assert_eq!(value["action"].as_str(), Some("ActivateContractInstance"));
        assert_eq!(value["namespace"].as_str(), Some("apps"));
        assert_eq!(value["contract_id"].as_str(), Some("calc.v1"));
        let expected_hex = format!("0x{}", "ab".repeat(32));
        assert_eq!(value["code_hash"].as_str(), Some(expected_hex.as_str()));
        assert!(value.get("hint").is_some(), "hint field should be present");
    }

    #[test]
    fn activate_instance_blocking_submits_instruction() {
        let mut ctx = TestContext::new();
        let args = ActivateInstanceArgs {
            namespace: "apps".to_string(),
            contract_id: "calc.v1".to_string(),
            code_hash: "0x".to_string() + &"cd".repeat(32),
            blocking: true,
        };
        args.run(&mut ctx).expect("run blocking path");
        assert!(
            ctx.printed.is_empty(),
            "blocking path should not print skeleton"
        );
        let submitted = ctx
            .submitted
            .expect("expected instructions to be submitted");
        assert_eq!(submitted.len(), 1, "exactly one instruction submitted");
        let instruction = &submitted[0];
        let any = instruction.as_any();
        let concret = any
            .downcast_ref::<iroha::data_model::isi::smart_contract_code::ActivateContractInstance>()
            .expect("downcast to ActivateContractInstance");
        assert_eq!(concret.namespace, "apps");
        assert_eq!(concret.contract_id, "calc.v1");
        assert_eq!(
            hex::encode(<[u8; 32]>::from(concret.code_hash)),
            "cd".repeat(32)
        );
    }

    fn sample_account_string(name: &str) -> String {
        let mut hasher = Blake3Hasher::new();
        hasher.update(b"gov-deploy-account");
        hasher.update(name.as_bytes());
        let digest = hasher.finalize();
        let key_pair = KeyPair::from_seed(digest.as_bytes().to_vec(), Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone()).to_string()
    }
}
