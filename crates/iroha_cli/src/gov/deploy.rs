//! Governance deployment CLI helpers.
use super::shared::{
    parse_governance_proposal_id_v1, print_with_summary, resolve_contract_address_target,
};
use crate::{
    Run, RunContext,
    json_utils::{json_array, json_object, json_value},
};
use eyre::{Result, eyre};
use iroha::client::{
    Client, GovernanceAtWindowV1, GovernanceDeployContractDraftRequestV1,
    GovernanceEnactDraftRequestV1, GovernanceFinalizeDraftRequestV1,
    validate_governance_deploy_draft_response_v1, validate_governance_enact_draft_response_v1,
    validate_governance_finalize_draft_response_v1,
};
use iroha::data_model::{
    isi::{InstructionBox, SetParameter, governance::VotingMode},
    name::Name,
    parameter::{CustomParameterId, Parameter, custom::CustomParameter},
    smart_contract::{ContractAddress, ContractAlias, manifest::ManifestProvenance},
};
const MANIFEST_PROVENANCE_MAX_BYTES: u64 = 64 * 1024;
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
    /// Exact first-release governed contract ABI.
    #[arg(long, default_value = "1", value_parser = ["1"])]
    pub abi_version: String,
    /// Inclusive referendum window lower block height.
    #[arg(long)]
    pub window_lower: u64,
    /// Inclusive referendum window upper block height.
    #[arg(long)]
    pub window_upper: u64,
    /// Optional voting mode for the referendum: Zk or Plain (defaults to server policy)
    #[arg(long, value_name = "MODE", value_parser = ["Zk", "Plain"])]
    pub mode: Option<String>,
    /// JSON file containing only the public `signer` and `signature` provenance fields.
    #[arg(long, visible_alias = "manifest-provenance", value_name = "PATH")]
    pub manifest_provenance_file: std::path::PathBuf,
    /// Sign, submit, and wait for this exact server-drafted proposal instruction.
    #[arg(long)]
    pub apply: bool,
}
fn read_manifest_provenance(path: &std::path::Path) -> Result<ManifestProvenance> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        eyre!(
            "failed to inspect --manifest-provenance-file `{}`: {error}",
            path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(eyre!(
            "--manifest-provenance-file `{}` must be a regular file",
            path.display()
        ));
    }
    if metadata.len() > MANIFEST_PROVENANCE_MAX_BYTES {
        return Err(eyre!(
            "--manifest-provenance-file exceeds the {}-byte limit",
            MANIFEST_PROVENANCE_MAX_BYTES
        ));
    }
    let bytes = std::fs::read(path).map_err(|error| {
        eyre!(
            "failed to read --manifest-provenance-file `{}`: {error}",
            path.display()
        )
    })?;
    norito::json::from_slice(&bytes).map_err(|error| {
        eyre!(
            "invalid --manifest-provenance-file `{}`: {error}",
            path.display()
        )
    })
}
impl Run for ProposeDeployArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let mode = match self.mode.as_deref() {
            Some("Zk") => Some(VotingMode::Zk),
            Some("Plain") => Some(VotingMode::Plain),
            Some(other) => return Err(eyre!("invalid --mode `{other}`; expected Zk or Plain")),
            None => None,
        };
        let contract_address = self
            .contract_address
            .as_deref()
            .map(str::parse::<ContractAddress>)
            .transpose()
            .map_err(|error| eyre!("invalid --contract-address: {error}"))?;
        let contract_alias = self
            .contract_alias
            .as_deref()
            .map(str::parse::<ContractAlias>)
            .transpose()
            .map_err(|error| eyre!("invalid --contract-alias: {error}"))?;
        let resolved_contract_address = resolve_contract_address_target(
            &client,
            self.contract_address.as_deref(),
            self.contract_alias.as_deref(),
        )?;
        let request = GovernanceDeployContractDraftRequestV1 {
            contract_address,
            contract_alias,
            abi_version: self.abi_version,
            code_hash: self.code_hash,
            abi_hash: self.abi_hash,
            window: GovernanceAtWindowV1 {
                lower: self.window_lower,
                upper: self.window_upper,
            },
            mode,
            manifest_provenance: read_manifest_provenance(&self.manifest_provenance_file)?,
        };
        request
            .validate()
            .map_err(|error| eyre!("invalid governed deployment request: {error}"))?;
        let response =
            client.post_governance_deploy_draft_v1(&request, &resolved_contract_address)?;
        let instruction = validate_governance_deploy_draft_response_v1(
            &response,
            &request,
            &resolved_contract_address,
        )?;
        if self.apply {
            return context.submit(vec![instruction]);
        }
        let value = norito::json::to_value(&response)?;
        let summary = Some(format!(
            "deploy propose: ok={} proposal_id={}",
            response.ok, response.proposal_id
        ));
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
    /// Sign, submit, and wait for this exact server-drafted finalization instruction.
    #[arg(long)]
    pub apply: bool,
}
fn build_finalize_request(args: &FinalizeArgs) -> Result<GovernanceFinalizeDraftRequestV1> {
    let referendum_id = parse_governance_proposal_id_v1(&args.referendum_id)
        .map_err(|message| eyre!("invalid referendum_id: {message}"))?;
    let proposal_id = parse_governance_proposal_id_v1(&args.proposal_id)
        .map_err(|message| eyre!("invalid proposal_id: {message}"))?;
    if referendum_id != proposal_id {
        return Err(eyre!("referendum_id must equal proposal_id"));
    }
    Ok(GovernanceFinalizeDraftRequestV1 {
        referendum_id,
        proposal_id,
    })
}
impl Run for FinalizeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let request = build_finalize_request(&self)?;
        let response = client.post_governance_finalize_draft_v1(&request)?;
        let instruction = validate_governance_finalize_draft_response_v1(&response, &request)?;
        if self.apply {
            return context.submit(vec![instruction]);
        }
        let n_instr = response.tx_instructions.len();
        let summary = Some(format!(
            "finalize: referendum_id={} ok={} tx_instrs={n_instr}",
            self.referendum_id, response.ok
        ));
        let value = norito::json::to_value(&response)?;
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
fn finish_enact<C: RunContext>(
    context: &mut C,
    request: &GovernanceEnactDraftRequestV1,
    apply: bool,
    response: &iroha::client::GovernanceEnactDraftResponseV1,
) -> Result<()> {
    let instruction = validate_governance_enact_draft_response_v1(response, request)?;
    if apply {
        return context.submit(vec![instruction]);
    }
    let n_instr = response.tx_instructions.len();
    let summary = Some(format!(
        "enact: proposal_id={} ok={} tx_instrs={n_instr}",
        request.proposal_id, response.ok
    ));
    let value = norito::json::to_value(response)?;
    print_with_summary(context, summary, &value)
}
impl Run for EnactArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let request = GovernanceEnactDraftRequestV1 {
            proposal_id: self.proposal_id,
        };
        let response = client.post_governance_enact_draft_v1(&request)?;
        finish_enact(context, &request, self.apply, &response)
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
            apply: false,
        };
        let body = build_finalize_request(&args).expect("build finalize body");
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
                apply: false,
            },
            FinalizeArgs {
                referendum_id: "aa".repeat(32),
                proposal_id: "bb".repeat(32),
                apply: false,
            },
        ] {
            let _error = build_finalize_request(&args)
                .expect_err("invalid finalization ids must fail locally");
        }
    }
    #[test]
    fn enact_body_matches_strict_torii_dto() {
        let args = EnactArgs {
            proposal_id: "ab".repeat(32),
            apply: false,
        };
        let body = GovernanceEnactDraftRequestV1 {
            proposal_id: args.proposal_id.clone(),
        };
        let value = norito::json::to_value(&body).expect("encode enact request");
        let object = value.as_object().expect("enact body object");
        assert_eq!(object.len(), 1);
        assert_eq!(
            object.get("proposal_id").and_then(|value| value.as_str()),
            Some(args.proposal_id.as_str())
        );
        assert!(object.get("preimage_hash").is_none());
        assert!(object.get("window").is_none());
    }
    fn enact_draft_response() -> (
        GovernanceEnactDraftRequestV1,
        iroha::client::GovernanceEnactDraftResponseV1,
    ) {
        use iroha::data_model::governance::types::{
            AbiVersion, AtWindow, ContractAbiHash, ContractCodeHash, DeployContractProposal,
            ProposalKind,
        };
        let proposal_kind = ProposalKind::DeployContract(DeployContractProposal {
            contract_address: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                .parse()
                .expect("contract address"),
            code_hash_hex: ContractCodeHash::new([0x11; 32]),
            abi_hash_hex: ContractAbiHash::new([0x22; 32]),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        });
        let proposal_id = proposal_kind.fingerprint();
        let referendum_window = AtWindow {
            lower: 10,
            upper: 20,
        };
        let instruction: InstructionBox = iroha::data_model::isi::governance::EnactReferendum {
            referendum_id: proposal_id,
            preimage_hash: proposal_id,
            at_window: referendum_window,
        }
        .into();
        let draft = iroha::client::GovernanceInstructionDraftV1::from_instruction(&instruction)
            .expect("build enact draft instruction");
        let request = GovernanceEnactDraftRequestV1 {
            proposal_id: hex::encode(proposal_id),
        };
        let response = iroha::client::GovernanceEnactDraftResponseV1 {
            ok: true,
            proposal_id: request.proposal_id.clone(),
            proposal_kind,
            referendum_window,
            tx_instructions: vec![draft],
        };
        (request, response)
    }
    #[test]
    fn enact_defaults_to_draft_only() {
        let (request, response) = enact_draft_response();
        let mut context = TestContext::new();
        finish_enact(&mut context, &request, false, &response).expect("render enact draft");
        assert!(context.submitted.is_none());
        assert_eq!(context.printed.len(), 1);
    }
    #[test]
    fn enact_apply_decodes_and_submits_the_exact_native_instruction() {
        let (request, response) = enact_draft_response();
        let proposal_id = hex::decode(&request.proposal_id).expect("proposal id");
        let proposal_id: [u8; 32] = proposal_id.try_into().expect("32-byte proposal id");
        let mut context = TestContext::new();
        finish_enact(&mut context, &request, true, &response).expect("apply exact enact draft");
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
