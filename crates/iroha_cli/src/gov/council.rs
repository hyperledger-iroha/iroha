//! Council and VRF-related governance helpers.

use super::shared::print_with_summary;
use crate::{
    Run, RunContext,
    json_utils::{json_array, json_object, json_value},
};
use base64::Engine as _;
use eyre::{Result, WrapErr, eyre};
use iroha::client::Client;
use iroha::data_model::{
    ChainId,
    account::AccountId,
    domain::DomainId,
    events::{EventBox, data::prelude::DataEventFilter, prelude::EventFilterBox},
};
use iroha_core::governance::parliament;
use norito::json::Map;

#[derive(clap::Subcommand, Debug)]
pub enum CouncilSubcommand {
    #[command(name = "derive-vrf")]
    DeriveVrf(DeriveVrfArgs),
    #[command(name = "persist")]
    Persist(PersistCouncilArgs),
    #[command(name = "gen-vrf")]
    GenVrf(GenVrfArgs),
    #[command(name = "derive-and-persist")]
    DeriveAndPersist(DeriveAndPersistArgs),
    #[command(name = "replace")]
    Replace(ReplaceCouncilArgs),
}

#[derive(clap::Args, Debug)]
pub struct PersistCouncilArgs {
    /// Committee size to select (top-k by VRF output)
    #[arg(long)]
    pub committee_size: Option<usize>,
    /// Optional number of alternates to keep (defaults to committee size)
    #[arg(long)]
    pub alternate_size: Option<usize>,
    /// Optional epoch override; defaults to `height/TERM_BLOCKS`
    #[arg(long)]
    pub epoch: Option<u64>,
    /// Path to JSON file with candidates: [{ `account_id`, variant: Normal|Small, `pk_b64`, `proof_b64` }, ...]
    #[arg(long, value_name = "PATH")]
    pub candidates_file: std::path::PathBuf,
    /// Authority `AccountId` for signing (e.g., alice@wonderland)
    #[arg(long)]
    pub authority: String,
    /// Private key (hex) for signing
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
}

impl Run for PersistCouncilArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        // Read candidates JSON from file
        let s = std::fs::read_to_string(&self.candidates_file)?;
        let candidates: norito::json::Value = norito::json::from_str(&s)?;
        if !candidates.is_array() {
            return Err(eyre!("candidates file must contain a JSON array"));
        }
        let body = json_object(vec![
            ("committee_size", json_value(&self.committee_size)?),
            ("alternate_size", json_value(&self.alternate_size)?),
            ("epoch", json_value(&self.epoch)?),
            ("candidates", candidates),
            ("authority", json_value(&self.authority)?),
            ("private_key", json_value(&self.private_key)?),
        ])?;
        let value = client.post_gov_council_persist_json(&body)?;
        let epoch = value
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let verified = value
            .get("verified")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let members = value
            .get("members")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let alternates = value
            .get("alternates")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let summary = Some(format!(
            "council persist: epoch={epoch} members={members} alternates={alternates} verified={verified}"
        ));
        print_with_summary(context, summary, &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct GenVrfArgs {
    /// Number of candidates to generate
    #[arg(long, default_value_t = 5)]
    pub count: usize,
    /// Variant: Normal (pk in G1, proof in G2) or Small (pk in G2, proof in G1)
    #[arg(long, value_parser = ["Normal", "Small"], default_value = "Normal")]
    pub variant: String,
    /// Chain id string used for VRF domain separation
    #[arg(long)]
    pub chain_id: String,
    /// Optional seed hex (32 bytes as 64 hex); if omitted, requires --epoch and --beacon-hex
    #[arg(long)]
    pub seed_hex: Option<String>,
    /// Epoch index used when deriving the seed (ignored if --seed-hex is provided)
    #[arg(long)]
    pub epoch: Option<u64>,
    /// Beacon hash hex (32 bytes as 64 hex) to derive the seed (ignored if --seed-hex is provided)
    #[arg(long)]
    pub beacon_hex: Option<String>,
    /// Account id prefix (final id is `${prefix}-${i}@${domain}`)
    #[arg(long, default_value = "node")]
    pub account_prefix: String,
    /// Domain used in generated account ids
    #[arg(long, default_value = "wonderland")]
    pub domain: String,
    /// Output path; if omitted, prints JSON to stdout
    #[arg(long)]
    pub out: Option<std::path::PathBuf>,
    /// Fetch `seed/epoch/chain_id` from /v1/gov/council/audit (overrides --epoch/--beacon-hex when set)
    #[arg(long, default_value_t = false)]
    pub from_audit: bool,
}

impl Run for GenVrfArgs {
    fn run<C: RunContext>(mut self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let seed = self.resolve_seed(context, &client)?;
        let candidates = self.generate_candidates(&seed)?;
        self.output_candidates(context, candidates)
    }
}

impl GenVrfArgs {
    fn resolve_seed<C: RunContext>(
        &mut self,
        context: &mut C,
        client: &Client,
    ) -> Result<[u8; 64]> {
        if self.from_audit {
            self.seed_from_audit(context, client)
        } else if let Some(hex) = self.seed_hex.as_deref() {
            Self::seed_from_flag(hex)
        } else {
            self.seed_from_inputs()
        }
    }

    fn seed_from_audit<C: RunContext>(
        &mut self,
        context: &mut C,
        client: &Client,
    ) -> Result<[u8; 64]> {
        let audit = client.get_gov_council_audit_json(self.epoch)?;
        let seed_hex = audit
            .get("seed_hex")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre!("audit missing seed_hex"))?;
        let chain = audit
            .get("chain_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre!("audit missing chain_id"))?;
        let aud_epoch = audit
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let aud_beacon = audit
            .get("beacon_hex")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        self.chain_id = chain.to_string();
        let chain_id = &self.chain_id;
        context.println(format!(
            "audit: chain_id={chain_id} epoch={aud_epoch} beacon_hex={aud_beacon}"
        ))?;
        let bytes = hex::decode(seed_hex).map_err(|_| eyre!("invalid seed_hex from audit"))?;
        bytes
            .try_into()
            .map_err(|_| eyre!("audit seed must be 64 bytes"))
    }

    fn seed_from_flag(hex: &str) -> Result<[u8; 64]> {
        let bytes = hex::decode(hex.trim_start_matches("0x"))
            .map_err(|_| eyre!("invalid --seed-hex (expect 128 hex chars)"))?;
        bytes
            .try_into()
            .map_err(|_| eyre!("--seed-hex must be 64 bytes"))
    }

    fn seed_from_inputs(&self) -> Result<[u8; 64]> {
        let (epoch, beacon_hex) = match (self.epoch, self.beacon_hex.as_deref()) {
            (Some(e), Some(b)) => (e, b),
            _ => {
                return Err(eyre!(
                    "either --seed-hex or both --epoch and --beacon-hex must be provided"
                ));
            }
        };
        let beacon = hex::decode(beacon_hex.trim_start_matches("0x"))
            .map_err(|_| eyre!("invalid --beacon-hex"))?;
        let beacon: [u8; 32] = beacon
            .try_into()
            .map_err(|_| eyre!("--beacon-hex must be 32 bytes"))?;
        let chain = ChainId::from(self.chain_id.clone());
        Ok(parliament::compute_seed(&chain, epoch, &beacon))
    }

    fn generate_candidates(&self, seed: &[u8; 64]) -> Result<Vec<norito::json::Value>> {
        let chain_bytes = self.chain_id.as_bytes();
        let domain_id: DomainId = self
            .domain
            .parse()
            .map_err(|_| eyre!("invalid --domain value"))?;
        (0..self.count)
            .map(|index| self.build_candidate(seed, chain_bytes, index, &domain_id))
            .collect()
    }

    fn build_candidate(
        &self,
        seed: &[u8; 64],
        chain_bytes: &[u8],
        index: usize,
        domain_id: &DomainId,
    ) -> Result<norito::json::Value> {
        let alias = format!("{}{}@{}", self.account_prefix, index, self.domain);
        let account_seed = iroha_crypto::Hash::new(alias.as_bytes());
        let account_keypair = iroha_crypto::KeyPair::from_seed(
            account_seed.as_ref().to_vec(),
            iroha_crypto::Algorithm::Ed25519,
        );
        let (account_public_key, _) = account_keypair.into_parts();
        let account_id = AccountId::new(domain_id.clone(), account_public_key);
        let account_id_str = account_id.to_string();
        let (variant_label, pk_b64, proof_b64) =
            self.candidate_payload(seed, chain_bytes, index, &account_id, &alias)?;
        let candidate = json_object(vec![
            ("account_id", json_value(&account_id_str)?),
            ("variant", json_value(variant_label)?),
            ("pk_b64", json_value(&pk_b64)?),
            ("proof_b64", json_value(&proof_b64)?),
        ])?;
        Ok(candidate)
    }

    fn candidate_payload(
        &self,
        seed: &[u8; 64],
        chain_bytes: &[u8],
        index: usize,
        account_id: &AccountId,
        alias: &str,
    ) -> Result<(&'static str, String, String)> {
        if self.variant.eq_ignore_ascii_case("Small") {
            Self::build_small_candidate(seed, chain_bytes, index, account_id, alias)
        } else {
            Self::build_normal_candidate(seed, chain_bytes, index, account_id, alias)
        }
    }

    fn build_normal_candidate(
        seed: &[u8; 64],
        chain_bytes: &[u8],
        index: usize,
        account_id: &AccountId,
        alias: &str,
    ) -> Result<(&'static str, String, String)> {
        let mut attempt = 0u32;
        loop {
            let bls_seed =
                iroha_crypto::Hash::new(format!("{alias}|normal|{index}|{attempt}").as_bytes());
            let (pk, sk) = iroha_crypto::BlsNormal::keypair(iroha_crypto::KeyGenOption::UseSeed(
                bls_seed.as_ref().to_vec(),
            ));
            let (public_key, _) = iroha_crypto::KeyPair::from((pk, sk.clone())).into_parts();
            let input = parliament::build_input(seed, account_id);
            let proof_result = std::panic::catch_unwind(|| {
                iroha_crypto::vrf::prove_normal_with_chain(&sk, chain_bytes, &input)
            });
            if let Ok((_output, proof)) = proof_result {
                let (_alg, pk_payload) = public_key.to_bytes();
                let proof_bytes = match proof {
                    iroha_crypto::vrf::VrfProof::SigInG2(bytes) => bytes,
                    _ => unreachable!("Normal uses SigInG2"),
                };
                return Ok((
                    "Normal",
                    base64::engine::general_purpose::STANDARD.encode(pk_payload),
                    base64::engine::general_purpose::STANDARD.encode(proof_bytes),
                ));
            }
            attempt += 1;
            if attempt > 16 {
                return Err(eyre!("failed to derive BLS normal key after retries"));
            }
        }
    }

    fn build_small_candidate(
        seed: &[u8; 64],
        chain_bytes: &[u8],
        index: usize,
        account_id: &AccountId,
        alias: &str,
    ) -> Result<(&'static str, String, String)> {
        let mut attempt = 0u32;
        loop {
            let bls_seed =
                iroha_crypto::Hash::new(format!("{alias}|small|{index}|{attempt}").as_bytes());
            let (pk, sk) = iroha_crypto::BlsSmall::keypair(iroha_crypto::KeyGenOption::UseSeed(
                bls_seed.as_ref().to_vec(),
            ));
            let (public_key, _) = iroha_crypto::KeyPair::from((pk, sk.clone())).into_parts();
            let input = parliament::build_input(seed, account_id);
            let proof_result = std::panic::catch_unwind(|| {
                iroha_crypto::vrf::prove_small_with_chain(&sk, chain_bytes, &input)
            });
            if let Ok((_output, proof)) = proof_result {
                let (_alg, pk_payload) = public_key.to_bytes();
                let proof_bytes = match proof {
                    iroha_crypto::vrf::VrfProof::SigInG1(bytes) => bytes,
                    _ => unreachable!("Small uses SigInG1"),
                };
                return Ok((
                    "Small",
                    base64::engine::general_purpose::STANDARD.encode(pk_payload),
                    base64::engine::general_purpose::STANDARD.encode(proof_bytes),
                ));
            }
            attempt += 1;
            if attempt > 16 {
                return Err(eyre!("failed to derive BLS small key after retries"));
            }
        }
    }

    fn output_candidates<C: RunContext>(
        &self,
        context: &mut C,
        candidates: Vec<norito::json::Value>,
    ) -> Result<()> {
        let count = candidates.len();
        let value = norito::json::Value::Array(candidates);
        if let Some(path) = self.out.as_ref() {
            let json = norito::json::to_json_pretty(&value)?;
            std::fs::write(path, json.as_bytes())?;
            context.println(format!("wrote {count} candidates to {}", path.display()))?;
        } else {
            context.print_data(&value)?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct DeriveAndPersistArgs {
    /// Committee size to select (top-k by VRF output)
    #[arg(long)]
    pub committee_size: Option<usize>,
    /// Optional number of alternates to keep (defaults to committee size)
    #[arg(long)]
    pub alternate_size: Option<usize>,
    /// Optional epoch override; defaults to `height/TERM_BLOCKS` (server-side)
    #[arg(long)]
    pub epoch: Option<u64>,
    /// Path to JSON file with candidates: [{ `account_id`, variant: Normal|Small, `pk_b64`, `proof_b64` }, ...]
    #[arg(long, value_name = "PATH")]
    pub candidates_file: std::path::PathBuf,
    /// Authority `AccountId` for signing (e.g., alice@wonderland)
    #[arg(long)]
    pub authority: String,
    /// Private key (hex) for signing
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
    /// Wait for `CouncilPersisted` event and verify via /v1/gov/council/current
    #[arg(long, default_value_t = false)]
    pub wait: bool,
}

impl Run for DeriveAndPersistArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let candidates = self.load_candidates()?;
        let derive = self.derive_once(&client, &candidates)?;
        let persisted = self.persist_once(&client, &candidates, derive.epoch)?;
        let summary = Some(format!(
            "council derive-and-persist: epoch={} members={} alternates={} verified={}",
            derive.epoch, derive.members, derive.alternates, derive.verified
        ));
        let combined = json_object(vec![
            ("derived", derive.value.clone()),
            ("persisted", persisted),
        ])?;
        print_with_summary(context, summary, &combined)?;
        if self.wait {
            Self::verify_persist(context, &client, derive.epoch)?;
        }
        Ok(())
    }
}

impl DeriveAndPersistArgs {
    fn load_candidates(&self) -> Result<norito::json::Value> {
        let contents =
            std::fs::read_to_string(&self.candidates_file).wrap_err("read --candidates-file")?;
        let value: norito::json::Value =
            norito::json::from_str(&contents).wrap_err("parse candidates JSON")?;
        if !value.is_array() {
            return Err(eyre!("candidates file must contain a JSON array"));
        }
        Ok(value)
    }

    fn derive_once(
        &self,
        client: &Client,
        candidates: &norito::json::Value,
    ) -> Result<DeriveOutcome> {
        let mut pairs = vec![
            ("committee_size", json_value(&self.committee_size)?),
            ("epoch", json_value(&self.epoch)?),
            ("candidates", candidates.clone()),
        ];
        if let Some(alternate_size) = self.alternate_size {
            pairs.push(("alternate_size", json_value(&alternate_size)?));
        }
        let body = json_object(pairs)?;
        let value = client.post_gov_council_derive_vrf_json(&body)?;
        let epoch = value
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let members = value
            .get("members")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let alternates = value
            .get("alternates")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let verified = value
            .get("verified")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        Ok(DeriveOutcome {
            value,
            epoch,
            members,
            alternates,
            verified,
        })
    }

    fn persist_once(
        &self,
        client: &Client,
        candidates: &norito::json::Value,
        epoch: u64,
    ) -> Result<norito::json::Value> {
        let mut pairs = vec![
            ("committee_size", json_value(&self.committee_size)?),
            ("epoch", json_value(&epoch)?),
            ("candidates", candidates.clone()),
            ("authority", json_value(&self.authority)?),
            ("private_key", json_value(&self.private_key)?),
        ];
        if let Some(alternate_size) = self.alternate_size {
            pairs.push(("alternate_size", json_value(&alternate_size)?));
        }
        let body = json_object(pairs)?;
        client.post_gov_council_persist_json(&body)
    }

    fn verify_persist<C: RunContext>(context: &mut C, client: &Client, epoch: u64) -> Result<()> {
        let filters: Vec<EventFilterBox> = vec![DataEventFilter::Any.into()];
        let mut it = client
            .listen_for_events(filters)
            .wrap_err("open events stream")?;
        let mut event_ok = false;
        let start = std::time::Instant::now();
        let max = std::time::Duration::from_secs(30);
        for ev in &mut it {
            let ev = ev.wrap_err("events next")?;
            if let EventBox::Data(event) = ev
                && let iroha::data_model::events::data::DataEvent::Governance(
                    iroha::data_model::events::data::governance::GovernanceEvent::CouncilPersisted(
                        cp,
                    ),
                ) = event.as_ref()
                && cp.epoch == epoch
            {
                event_ok = true;
                break;
            }
            if start.elapsed() > max {
                break;
            }
        }
        let curr = client.get_gov_council_json().wrap_err("council/current")?;
        let curr_epoch = curr
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let curr_members = curr
            .get("members")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let curr_alternates = curr
            .get("alternates")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        context.println(format!(
            "council verify: event={event_ok} current_epoch={curr_epoch} members={curr_members} alternates={curr_alternates}"
        ))?;
        if !event_ok || curr_epoch != epoch {
            return Err(eyre!(
                "verification failed: event_ok={event_ok} current_epoch={curr_epoch} expected_epoch={epoch}"
            ));
        }
        Ok(())
    }
}

struct DeriveOutcome {
    value: norito::json::Value,
    epoch: u64,
    members: usize,
    alternates: usize,
    verified: u64,
}

#[derive(clap::Args, Debug)]
pub struct CouncilArgs {}

impl Run for CouncilArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_gov_council_json()?;
        let epoch = value
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let members_list =
            value
                .get("members")
                .and_then(|v| v.as_array())
                .map_or_else(Vec::new, |arr| {
                    arr.iter()
                        .filter_map(|m| {
                            m.get("account_id")
                                .and_then(|v| v.as_str())
                                .map(ToString::to_string)
                        })
                        .collect()
                });
        let member_count = members_list.len();
        let alternates_list = value
            .get("alternates")
            .and_then(|v| v.as_array())
            .map_or_else(Vec::new, |arr| {
                arr.iter()
                    .filter_map(|m| {
                        m.get("account_id")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string)
                    })
                    .collect()
            });
        let alternate_count = alternates_list.len();
        let members_joined = members_list.join(", ");
        let alternates_joined = alternates_list.join(", ");
        let verified = value
            .get("verified")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let derived_by = value
            .get("derived_by")
            .and_then(norito::json::Value::as_str)
            .unwrap_or("unknown");
        let summary = Some(format!(
            "council: epoch={epoch} members_count={member_count} alternates_count={alternate_count} verified={verified} derived_by={derived_by} members=[{members_joined}] alternates=[{alternates_joined}]"
        ));
        print_with_summary(context, summary, &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct ReplaceCouncilArgs {
    /// Account id of the member to replace (e.g., alice@wonderland)
    #[arg(long)]
    pub missing: String,
    /// Optional epoch override; defaults to the latest persisted epoch
    #[arg(long)]
    pub epoch: Option<u64>,
    /// Authority `AccountId` for signing (e.g., alice@wonderland)
    #[arg(long)]
    pub authority: String,
    /// Private key (hex) for signing
    #[arg(long, value_name = "HEX")]
    pub private_key: String,
}

impl Run for ReplaceCouncilArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let body = json_object(vec![
            ("missing", json_value(&self.missing)?),
            ("epoch", json_value(&self.epoch)?),
            ("authority", json_value(&self.authority)?),
            ("private_key", json_value(&self.private_key)?),
        ])?;
        let value = client.post_gov_council_replace_json(&body)?;
        let epoch = value
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let members = value
            .get("members")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let alternates = value
            .get("alternates")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let replaced = value
            .get("replaced")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let summary = Some(format!(
            "council replace: epoch={epoch} members={members} alternates={alternates} replaced={replaced}"
        ));
        print_with_summary(context, summary, &value)
    }
}

#[derive(Debug, Clone)]
pub struct VrfCandidateArg {
    pub account_id: String,
    pub variant: String,
    pub pk_b64: String,
    pub proof_b64: String,
}

fn parse_candidate_flag(s: &str) -> Result<VrfCandidateArg, String> {
    // Format: account_id,variant,pk_b64,proof_b64
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err("candidate must be 'account_id,variant,pk_b64,proof_b64'".to_string());
    }
    let variant = parts[1].trim();
    let variant_ok = matches!(variant, "Normal" | "normal" | "Small" | "small");
    if !variant_ok {
        return Err("variant must be 'Normal' or 'Small'".to_string());
    }
    Ok(VrfCandidateArg {
        account_id: parts[0].trim().to_string(),
        variant: if variant == "normal" {
            "Normal".into()
        } else if variant == "small" {
            "Small".into()
        } else {
            variant.to_string()
        },
        pk_b64: parts[2].trim().to_string(),
        proof_b64: parts[3].trim().to_string(),
    })
}

#[derive(clap::Args, Debug)]
pub struct DeriveVrfArgs {
    /// Committee size to select
    #[arg(long, value_name = "N")]
    pub committee_size: Option<usize>,
    /// Optional alternates to keep
    #[arg(long, value_name = "N")]
    pub alternate_size: Option<usize>,
    /// Optional epoch override
    #[arg(long)]
    pub epoch: Option<u64>,
    /// Candidate spec: "`account_id,variant,pk_b64,proof_b64`"; repeatable
    #[arg(long = "candidate", value_parser = parse_candidate_flag)]
    pub candidates: Vec<VrfCandidateArg>,
    /// Path to a JSON file with an array of candidates ({`account_id`, variant, `pk_b64`, `proof_b64`})
    #[arg(long, value_name = "PATH")]
    pub candidates_file: Option<std::path::PathBuf>,
}

impl Run for DeriveVrfArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let candidates = self.collect_candidates()?;
        let response = self.invoke_derive(&client, &candidates)?;
        let summary = Self::derive_summary(&response);
        print_with_summary(context, summary, &response)
    }
}

impl DeriveVrfArgs {
    fn collect_candidates(&self) -> Result<norito::json::Value> {
        let mut combined = self.candidates_from_flags()?;
        if let Some(path) = self.candidates_file.as_ref() {
            combined.extend(Self::candidates_from_file(path)?);
        }
        if combined.is_empty() {
            return Err(eyre!(
                "no candidates provided: use --candidate and/or --candidates-file"
            ));
        }
        json_array(combined)
    }

    fn candidates_from_flags(&self) -> Result<Vec<norito::json::Value>> {
        self.candidates
            .iter()
            .map(|c| Self::candidate_json(&c.account_id, &c.variant, &c.pk_b64, &c.proof_b64))
            .collect()
    }

    fn candidates_from_file(path: &std::path::Path) -> Result<Vec<norito::json::Value>> {
        let s = std::fs::read_to_string(path)
            .map_err(|e| eyre!("failed to read --candidates-file: {e}"))?;
        let v: norito::json::Value = norito::json::from_str(&s)
            .map_err(|e| eyre!("--candidates-file is not valid JSON: {e}"))?;
        let arr = v.as_array().ok_or_else(|| {
            eyre!(
                "--candidates-file must be a JSON array of {{account_id,variant,pk_b64,proof_b64}}"
            )
        })?;
        arr.iter()
            .map(Self::candidate_from_json)
            .collect::<Result<Vec<_>>>()
    }

    fn candidate_from_json(value: &norito::json::Value) -> Result<norito::json::Value> {
        let obj = value
            .as_object()
            .ok_or_else(|| eyre!("candidate must be a JSON object"))?;
        let account_id = Self::extract_string(obj, "account_id")?;
        let mut variant = Self::extract_string(obj, "variant")?;
        match variant.as_str() {
            "Normal" | "Small" => {}
            "normal" => variant = "Normal".to_string(),
            "small" => variant = "Small".to_string(),
            other => {
                return Err(eyre!("invalid variant '{other}': expected Normal|Small"));
            }
        }
        let pk_b64 = Self::extract_string(obj, "pk_b64")?;
        let proof_b64 = Self::extract_string(obj, "proof_b64")?;
        Self::candidate_json(&account_id, &variant, &pk_b64, &proof_b64)
    }

    fn extract_string(obj: &Map, key: &str) -> Result<String> {
        obj.get(key)
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
            .ok_or_else(|| eyre!("candidate missing string field '{key}'"))
    }

    fn candidate_json(
        account_id: &str,
        variant: &str,
        pk_b64: &str,
        proof_b64: &str,
    ) -> Result<norito::json::Value> {
        json_object(vec![
            ("account_id", json_value(account_id)?),
            ("variant", json_value(variant)?),
            ("pk_b64", json_value(pk_b64)?),
            ("proof_b64", json_value(proof_b64)?),
        ])
    }

    fn invoke_derive(
        &self,
        client: &Client,
        candidates: &norito::json::Value,
    ) -> Result<norito::json::Value> {
        let mut pairs: Vec<(String, norito::json::Value)> = Vec::new();
        pairs.push((
            "committee_size".to_string(),
            json_value(&self.committee_size)?,
        ));
        if let Some(alternate_size) = self.alternate_size {
            pairs.push(("alternate_size".to_string(), json_value(&alternate_size)?));
        }
        if let Some(epoch) = self.epoch {
            pairs.push(("epoch".to_string(), json_value(&epoch)?));
        }
        pairs.push(("candidates".to_string(), candidates.clone()));
        let body = json_object(pairs)?;
        client.post_gov_council_derive_vrf_json(&body)
    }

    fn derive_summary(value: &norito::json::Value) -> Option<String> {
        let epoch = value.get("epoch")?.as_u64()?;
        let verified = value
            .get("verified")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let members =
            value
                .get("members")
                .and_then(|v| v.as_array())
                .map_or_else(Vec::new, |arr| {
                    arr.iter()
                        .filter_map(|m| m.get("account_id").and_then(|v| v.as_str()))
                        .map(ToString::to_string)
                        .collect::<Vec<String>>()
                });
        let members_joined = members.join(", ");
        let alternates = value
            .get("alternates")
            .and_then(|v| v.as_array())
            .map_or_else(Vec::new, |arr| {
                arr.iter()
                    .filter_map(|m| m.get("account_id").and_then(|v| v.as_str()))
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
            });
        let alternates_joined = alternates.join(", ");
        Some(format!(
            "council derive-vrf: epoch={epoch} verified={verified} members=[{members_joined}] alternates=[{alternates_joined}]"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_candidate_flag_ok() {
        let s = "alice@wonderland,normal,UEtCQjQ=,UFJPT0Y=";
        let c = parse_candidate_flag(s).expect("parse ok");
        assert_eq!(c.account_id, "alice@wonderland");
        assert_eq!(c.variant, "Normal");
        assert_eq!(c.pk_b64, "UEtCQjQ=");
        assert_eq!(c.proof_b64, "UFJPT0Y=");
    }

    #[test]
    fn parse_candidate_flag_invalid_variant() {
        let err = parse_candidate_flag("bob@wonderland,Other,pk,proof").unwrap_err();
        assert!(
            err.contains("variant"),
            "expected error about variant, got {err}"
        );
    }
}
