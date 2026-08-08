//! Governance deployment CLI helpers.

use super::shared::{
    parse_governance_proposal_id_v1, print_with_summary, resolve_contract_address_target,
};
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
    #[arg(long, conflicts_with = "contract_alias")]
    pub contract_address: Option<String>,
    #[arg(long, conflicts_with = "contract_address")]
    pub contract_alias: Option<String>,
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
        let mut pairs = vec![
            ("code_hash", json_value(&self.code_hash)?),
            ("abi_hash", json_value(&self.abi_hash)?),
            ("abi_version", json_value(&self.abi_version)?),
            ("window", window),
            ("mode", mode_value),
        ];
        match (
            self.contract_address.as_deref(),
            self.contract_alias.as_deref(),
        ) {
            (Some(_), Some(_)) => {
                return Err(eyre!(
                    "exactly one of --contract-address or --contract-alias must be provided"
                ));
            }
            (Some(contract_address), None) => {
                let contract_address: iroha::data_model::smart_contract::ContractAddress =
                    contract_address
                        .parse()
                        .map_err(|err| eyre!("invalid --contract-address: {err}"))?;
                pairs.push(("contract_address", json_value(&contract_address)?));
            }
            (None, Some(contract_alias)) => {
                let contract_alias: iroha::data_model::smart_contract::ContractAlias =
                    contract_alias
                        .parse()
                        .map_err(|err| eyre!("invalid --contract-alias: {err}"))?;
                pairs.push(("contract_alias", json_value(&contract_alias)?));
            }
            (None, None) => {
                return Err(eyre!(
                    "provide exactly one contract target via --contract-address or --contract-alias"
                ));
            }
        }
        let body = json_object(pairs)?;
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
    /// Referendum id (the exact lowercase proposal fingerprint)
    #[arg(
        long,
        value_name = "ID_HEX",
        value_parser = parse_governance_proposal_id_v1
    )]
    pub referendum_id: String,
    /// Proposal id (hex 64)
    #[arg(
        long,
        value_name = "ID_HEX",
        value_parser = parse_governance_proposal_id_v1
    )]
    pub proposal_id: String,
}

fn build_finalize_body(args: &FinalizeArgs) -> Result<norito::json::Value> {
    let referendum_id = parse_governance_proposal_id_v1(&args.referendum_id)
        .map_err(|message| eyre!("invalid referendum_id: {message}"))?;
    let proposal_id = parse_governance_proposal_id_v1(&args.proposal_id)
        .map_err(|message| eyre!("invalid proposal_id: {message}"))?;
    if referendum_id != proposal_id {
        return Err(eyre!("referendum_id must equal proposal_id"));
    }
    json_object(vec![
        ("referendum_id", json_value(&referendum_id)?),
        ("proposal_id", json_value(&proposal_id)?),
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
    #[arg(
        long,
        value_name = "ID_HEX",
        value_parser = parse_governance_proposal_id_v1
    )]
    pub proposal_id: String,
    /// Sign, submit, and wait for this exact server-drafted enactment instruction.
    #[arg(long)]
    pub apply: bool,
}

fn build_enact_body(args: &EnactArgs) -> Result<norito::json::Value> {
    json_object(vec![("proposal_id", json_value(&args.proposal_id)?)])
}

fn decode_enact_instructions(
    response: &norito::json::Value,
    expected_proposal_id: &str,
) -> Result<Vec<InstructionBox>> {
    let entries = response
        .get("tx_instructions")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("governance enactment draft is missing `tx_instructions`"))?;
    if entries.len() != 1 {
        return Err(eyre!(
            "governance enactment draft must contain exactly one instruction"
        ));
    }
    let entry = &entries[0];
    let wire_id = entry
        .get("wire_id")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("governance enactment draft is missing `wire_id`"))?;
    let payload_hex = entry
        .get("payload_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("governance enactment draft is missing `payload_hex`"))?;
    let payload = hex::decode(payload_hex)
        .map_err(|error| eyre!("invalid governance enactment payload hex: {error}"))?;
    let instruction = iroha::data_model::isi::decode_instruction_from_pair(wire_id, &payload)
        .map_err(|error| eyre!("invalid governance enactment instruction: {error}"))?;
    let enactment = instruction
        .as_any()
        .downcast_ref::<iroha::data_model::isi::governance::EnactReferendum>()
        .ok_or_else(|| eyre!("governance enactment draft returned a different instruction"))?;
    if hex::encode(enactment.referendum_id) != expected_proposal_id {
        return Err(eyre!(
            "governance enactment draft proposal id differs from the request"
        ));
    }
    Ok(vec![instruction])
}

fn finish_enact<C: RunContext>(
    context: &mut C,
    proposal_id: &str,
    apply: bool,
    value: &norito::json::Value,
) -> Result<()> {
    if apply {
        let instructions = decode_enact_instructions(value, proposal_id)?;
        return context.submit(instructions);
    }
    let ok = value
        .get("ok")
        .and_then(norito::json::Value::as_bool)
        .unwrap_or(false);
    let n_instr = value
        .get("tx_instructions")
        .and_then(|v| v.as_array())
        .map_or(0, Vec::len);
    let summary = Some(format!(
        "enact: proposal_id={proposal_id} ok={ok} tx_instrs={n_instr}"
    ));
    print_with_summary(context, summary, value)
}

impl Run for EnactArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let body = build_enact_body(&self)?;
        let value = client.post_gov_enact_json(&body)?;
        finish_enact(context, &self.proposal_id, self.apply, &value)
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
pub struct DeployMetaArgs {
    #[arg(long, conflicts_with = "contract_alias")]
    pub contract_address: Option<String>,
    #[arg(long, conflicts_with = "contract_address")]
    pub contract_alias: Option<String>,
    /// Optional validator account IDs (canonical I105 account literals) authorizing the deployment alongside the authority.
    #[arg(long = "approver", value_name = "ACCOUNT")]
    pub approvers: Vec<String>,
}

impl Run for DeployMetaArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let contract_address = resolve_contract_address_target(
            &client,
            self.contract_address.as_deref(),
            self.contract_alias.as_deref(),
        )?;
        let mut pairs = vec![("gov_contract_address", json_value(&contract_address)?)];

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
            let key_pair = fixture_key_pair(vec![0xA5; 32]);
            let account = AccountId::new(key_pair.public_key().clone());
            let cfg = Config {
                chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
                network_id:
                    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
                        .parse()
                        .expect("network id"),
                account,
                account_chain_discriminant:
                    iroha_config::parameters::defaults::common::chain_discriminant(),
                key_pair,
                basic_auth: None,
                torii_api_url: Url::parse("http://127.0.0.1/").unwrap(),
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

    fn fixture_key_pair(seed: Vec<u8>) -> KeyPair {
        KeyPair::try_from_seed(seed, Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair")
    }

    #[test]
    fn fixture_key_pair_uses_checked_seed_derivation() {
        assert_eq!(
            fixture_key_pair(vec![0xA6; 32]).algorithm(),
            Algorithm::Ed25519
        );
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
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
                Executable::ContractCall(_) => {
                    eyre::bail!("unexpected contract-call submission in test context")
                }
                Executable::Ivm(IvmBytecode { .. }) => {
                    eyre::bail!("unexpected IVM bytecode submission in test context")
                }
                Executable::IvmProved(_) => {
                    eyre::bail!("unexpected proved IVM submission in test context")
                }
                Executable::Batch(_) => {
                    eyre::bail!("unexpected mixed executable batch submission in test context")
                }
            }
        }
    }

    #[test]
    fn deploy_meta_args_outputs_expected_keys() {
        let mut ctx = TestContext::new();
        let args = DeployMetaArgs {
            contract_address: Some(
                "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw".into(),
            ),
            contract_alias: None,
            approvers: Vec::new(),
        };
        args.run(&mut ctx).expect("deploy-meta run");
        assert_eq!(ctx.printed.len(), 1);
        let value = &ctx.printed[0];
        assert_eq!(
            value.get("gov_contract_address").and_then(|v| v.as_str()),
            Some("irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw")
        );
        assert!(value.get("gov_manifest_approvers").is_none());
    }

    #[test]
    fn deploy_meta_args_accepts_manifest_approvers() {
        let mut ctx = TestContext::new();
        let validator = sample_account_string("validator");
        let bob = sample_account_string("bob");
        let args = DeployMetaArgs {
            contract_address: Some(
                "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw".into(),
            ),
            contract_alias: None,
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
            contract_address: Some(
                "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw".into(),
            ),
            contract_alias: None,
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
            contract_address: Some(
                "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw".into(),
            ),
            contract_alias: None,
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
        let proposal_id = "aa".repeat(32);
        let args = FinalizeArgs {
            referendum_id: proposal_id.clone(),
            proposal_id: proposal_id.clone(),
        };
        let body = build_finalize_body(&args).expect("build finalize body");
        let s = norito::json::to_json(&body).expect("serialize body");
        let v: norito::json::Value = norito::json::from_str(&s).expect("roundtrip");
        assert_eq!(v["referendum_id"].as_str(), Some(proposal_id.as_str()));
        assert_eq!(v["proposal_id"].as_str(), Some(proposal_id.as_str()));
    }

    #[test]
    fn finalize_body_rejects_noncanonical_or_mismatched_ids() {
        for args in [
            FinalizeArgs {
                referendum_id: "ref-123".to_owned(),
                proposal_id: "aa".repeat(32),
            },
            FinalizeArgs {
                referendum_id: "aa".repeat(32),
                proposal_id: "bb".repeat(32),
            },
        ] {
            build_finalize_body(&args).expect_err("invalid finalization ids must fail locally");
        }
    }

    #[test]
    fn enact_body_matches_strict_torii_dto() {
        let args = EnactArgs {
            proposal_id: "ab".repeat(32),
            apply: false,
        };
        let body = build_enact_body(&args).expect("build enact body");
        let object = body.as_object().expect("enact body object");
        assert_eq!(object.len(), 1);
        assert_eq!(
            object.get("proposal_id").and_then(|value| value.as_str()),
            Some(args.proposal_id.as_str())
        );
        assert!(object.get("preimage_hash").is_none());
        assert!(object.get("window").is_none());
    }

    fn enact_draft_response(proposal_id: [u8; 32]) -> norito::json::Value {
        use iroha::data_model::isi::{Instruction, frame_instruction_payload};

        let instruction: InstructionBox = iroha::data_model::isi::governance::EnactReferendum {
            referendum_id: proposal_id,
            preimage_hash: proposal_id,
            at_window: iroha::data_model::governance::types::AtWindow {
                lower: 10,
                upper: 20,
            },
        }
        .into();
        let wire_id = Instruction::id(&*instruction);
        let payload = Instruction::dyn_encode(&*instruction);
        let framed = frame_instruction_payload(wire_id, &payload).expect("frame enact instruction");
        let tx_instruction = json_object(vec![
            ("wire_id", json_value(&wire_id).expect("encode wire id")),
            (
                "payload_hex",
                json_value(&hex::encode(framed)).expect("encode framed payload"),
            ),
        ])
        .expect("build enact draft instruction");
        let tx_instructions =
            json_array(vec![tx_instruction]).expect("build enact draft instruction list");
        json_object(vec![
            ("ok", json_value(&true).expect("encode draft status")),
            ("tx_instructions", tx_instructions),
        ])
        .expect("build enact draft response")
    }

    #[test]
    fn enact_defaults_to_draft_only() {
        let proposal_id = [0xAB; 32];
        let response = enact_draft_response(proposal_id);
        let mut context = TestContext::new();
        finish_enact(&mut context, &hex::encode(proposal_id), false, &response)
            .expect("render enact draft");
        assert!(context.submitted.is_none());
        assert_eq!(context.printed.len(), 1);
    }

    #[test]
    fn enact_apply_decodes_and_submits_the_exact_native_instruction() {
        let proposal_id = [0xAC; 32];
        let response = enact_draft_response(proposal_id);
        let mut context = TestContext::new();
        finish_enact(&mut context, &hex::encode(proposal_id), true, &response)
            .expect("apply exact enact draft");
        let submitted = context.submitted.expect("submitted instructions");
        assert_eq!(submitted.len(), 1);
        let enactment = submitted[0]
            .as_any()
            .downcast_ref::<iroha::data_model::isi::governance::EnactReferendum>()
            .expect("submitted EnactReferendum");
        assert_eq!(enactment.referendum_id, proposal_id);
        assert!(context.printed.is_empty());
    }

    fn sample_account_string(name: &str) -> String {
        let mut hasher = Blake3Hasher::new();
        hasher.update(b"gov-deploy-account");
        hasher.update(name.as_bytes());
        let digest = hasher.finalize();
        let key_pair = fixture_key_pair(digest.as_bytes().to_vec());
        AccountId::new(key_pair.public_key().clone()).to_string()
    }
}
