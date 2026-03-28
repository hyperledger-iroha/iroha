//! Governance voting and query helpers.

use crate::{
    Run, RunContext,
    json_utils::{json_object, json_value},
};
use clap::ValueEnum;
use eyre::{Result, eyre};
use iroha::client::Client;
use iroha::data_model::account::AccountId;
use iroha::data_model::isi::{
    InstructionBox,
    governance::{CastPlainBallot, CastZkBallot},
};
use iroha_crypto::Hash as CryptoHash;
use norito::{decode_from_bytes, json};

use super::shared::{canonicalize_hex32, print_with_summary};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
/// Voting mode selector for `iroha_cli gov vote`.
pub enum VoteMode {
    /// Automatically detect the referendum mode from the node.
    Auto,
    /// Force plain (non-ZK) voting mode.
    Plain,
    /// Force zero-knowledge voting mode.
    Zk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResolvedVoteMode {
    Plain,
    Zk,
}

fn parse_referendum_mode(value: &norito::json::Value) -> Option<ResolvedVoteMode> {
    let referendum = value.get("referendum")?;
    let mode = referendum.get("mode")?.as_str()?;
    if mode.eq_ignore_ascii_case("zk") {
        Some(ResolvedVoteMode::Zk)
    } else if mode.eq_ignore_ascii_case("plain") {
        Some(ResolvedVoteMode::Plain)
    } else {
        None
    }
}

fn resolve_vote_mode(
    client: &Client,
    referendum_id: &str,
    mode: VoteMode,
) -> Result<ResolvedVoteMode> {
    match mode {
        VoteMode::Plain => Ok(ResolvedVoteMode::Plain),
        VoteMode::Zk => Ok(ResolvedVoteMode::Zk),
        VoteMode::Auto => {
            let value = client.get_gov_referendum_json(referendum_id)?;
            let found = value
                .get("found")
                .and_then(norito::json::Value::as_bool)
                .unwrap_or(false);
            if !found {
                return Err(eyre!(
                    "referendum '{referendum_id}' not found; specify --mode explicitly"
                ));
            }
            parse_referendum_mode(&value).ok_or_else(|| {
                eyre!("referendum '{referendum_id}' does not expose a voting mode; specify --mode")
            })
        }
    }
}

fn hint_present(map: &json::Map, key: &str) -> bool {
    map.get(key)
        .is_some_and(|value| !matches!(value, json::Value::Null))
}

fn ensure_lock_hints_complete(map: &json::Map) -> Result<()> {
    let has_owner = hint_present(map, "owner");
    let has_amount = hint_present(map, "amount");
    let has_duration = hint_present(map, "duration_blocks");
    let any = has_owner || has_amount || has_duration;
    if any && !(has_owner && has_amount && has_duration) {
        return Err(eyre!(
            "lock hints must include owner, amount, duration_blocks"
        ));
    }
    Ok(())
}

fn canonicalize_account_literal(value: &str, field: &str) -> Result<String> {
    let owner = value.trim();
    if owner.is_empty() {
        return Err(eyre!("{field} must be a canonical I105 account id"));
    }
    if owner.contains('@') {
        return Err(eyre!(
            "{field} must not include '@domain'; use canonical I105 only"
        ));
    }

    let parsed = AccountId::parse_encoded(owner)
        .map_err(|err| eyre!("{field} must be a canonical I105 account id: {err}"))?;
    Ok(parsed.canonical().to_owned())
}

fn normalize_public_input_owner(map: &mut json::Map) -> Result<()> {
    let Some(value) = map.get("owner") else {
        return Ok(());
    };
    if matches!(value, json::Value::Null) {
        return Ok(());
    }
    let owner = value
        .as_str()
        .ok_or_else(|| eyre!("owner must be a canonical I105 account id"))?
        .to_owned();
    let canonical = canonicalize_account_literal(&owner, "owner")?;
    map.insert("owner".to_owned(), json::Value::String(canonical));
    Ok(())
}

fn normalize_public_inputs(map: &mut json::Map) -> Result<()> {
    reject_public_input_key(map, "durationBlocks", "duration_blocks")?;
    reject_public_input_key(map, "root_hint_hex", "root_hint")?;
    reject_public_input_key(map, "rootHintHex", "root_hint")?;
    reject_public_input_key(map, "rootHint", "root_hint")?;
    reject_public_input_key(map, "nullifier_hex", "nullifier")?;
    reject_public_input_key(map, "nullifierHex", "nullifier")?;
    canonicalize_public_input_hex(map, "root_hint")?;
    canonicalize_public_input_hex(map, "nullifier")?;
    Ok(())
}

fn canonicalize_public_input_hex(map: &mut json::Map, key: &str) -> Result<()> {
    let Some(value) = map.get_mut(key) else {
        return Ok(());
    };
    if matches!(value, json::Value::Null) {
        return Ok(());
    }
    let raw = value
        .as_str()
        .ok_or_else(|| eyre!("{key} must be 32-byte hex"))?;
    let canonical = canonicalize_hex32(raw).map_err(|_| eyre!("{key} must be 32-byte hex"))?;
    *value = json::Value::String(canonical);
    Ok(())
}

fn reject_public_input_key(map: &json::Map, key: &str, canonical: &str) -> Result<()> {
    if map.contains_key(key) {
        return Err(eyre!(
            "public inputs JSON does not support {key}; use {canonical}"
        ));
    }
    Ok(())
}

#[derive(clap::Args, Debug)]
pub struct VoteArgs {
    #[arg(long, value_name = "REFERENDUM_ID")]
    pub referendum_id: String,
    /// Voting mode override. Defaults to auto-detect via GET /v1/gov/referenda/{id}.
    #[arg(long, value_enum, default_value_t = VoteMode::Auto)]
    pub mode: VoteMode,
    /// Base64-encoded proof for ZK voting mode.
    #[arg(long)]
    pub proof_b64: Option<String>,
    /// Optional JSON file containing public inputs for ZK voting mode.
    #[arg(long, value_name = "PATH")]
    pub public: Option<std::path::PathBuf>,
    /// Owner account id for plain voting mode (canonical I105 account literal; must equal transaction authority).
    #[arg(long)]
    pub owner: Option<String>,
    /// Locked amount for plain voting mode (string to preserve large integers).
    #[arg(long, alias = "lock-amount")]
    pub amount: Option<String>,
    /// Lock duration (in blocks) for plain voting mode.
    #[arg(long, alias = "lock-duration-blocks")]
    pub duration_blocks: Option<u64>,
    /// Ballot direction for plain voting mode: Aye, Nay, or Abstain.
    #[arg(long)]
    pub direction: Option<String>,
    /// Optional 32-byte nullifier hint for ZK ballots (hex).
    #[arg(long = "nullifier")]
    pub nullifier: Option<String>,
}

impl Run for VoteArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let VoteArgs {
            referendum_id,
            mode,
            proof_b64,
            public,
            owner,
            amount,
            duration_blocks,
            direction,
            nullifier,
        } = self;

        let client: Client = context.client_from_config();
        let resolved = resolve_vote_mode(&client, &referendum_id, mode)?;

        match resolved {
            ResolvedVoteMode::Zk => {
                let proof = proof_b64.ok_or_else(|| {
                    eyre!("--proof-b64 is required for Zk voting; provide it or set --mode plain")
                })?;
                let args = VoteZkArgs {
                    election_id: referendum_id,
                    proof_b64: proof,
                    public,
                    owner,
                    amount,
                    duration_blocks,
                    direction,
                    nullifier,
                };
                args.run(context)
            }
            ResolvedVoteMode::Plain => {
                let owner = owner.ok_or_else(|| {
                    eyre!(
                        "--owner is required for plain voting; provide it or set --mode zk with proof"
                    )
                })?;
                let amount = amount.ok_or_else(|| {
                    eyre!(
                        "--amount is required for plain voting; provide it or set --mode zk with proof"
                    )
                })?;
                let duration_blocks = duration_blocks.ok_or_else(|| {
                    eyre!(
                        "--duration-blocks is required for plain voting; provide it or set --mode zk with proof"
                    )
                })?;
                let direction = direction.ok_or_else(|| {
                    eyre!(
                        "--direction is required for plain voting; provide it or set --mode zk with proof"
                    )
                })?;
                let args = VotePlainArgs {
                    referendum_id,
                    owner,
                    amount,
                    duration_blocks,
                    direction,
                };
                args.run(context)
            }
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct VoteZkArgs {
    #[arg(long)]
    pub election_id: String,
    #[arg(long)]
    pub proof_b64: String,
    /// Path to a JSON file with additional public inputs (optional)
    #[arg(long)]
    pub public: Option<std::path::PathBuf>,
    /// Optional owner hint mirrored into public inputs (canonical I105 account literal).
    #[arg(long)]
    pub owner: Option<String>,
    /// Optional lock amount hint mirrored into public inputs.
    #[arg(long, alias = "lock-amount")]
    pub amount: Option<String>,
    /// Optional lock duration hint mirrored into public inputs.
    #[arg(long, alias = "lock-duration-blocks")]
    pub duration_blocks: Option<u64>,
    /// Optional direction hint mirrored into public inputs.
    #[arg(long)]
    pub direction: Option<String>,
    /// Optional 32-byte nullifier hint derived from the proof commitment.
    #[arg(long = "nullifier")]
    pub nullifier: Option<String>,
}

impl Run for VoteZkArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let mut public = if let Some(p) = self.public {
            let s = std::fs::read_to_string(&p)?;
            norito::json::from_str(&s)?
        } else {
            norito::json::Value::Object(norito::json::Map::new())
        };
        let public_obj = public
            .as_object_mut()
            .ok_or_else(|| eyre!("public inputs JSON must be an object"))?;
        normalize_public_inputs(public_obj)?;
        if let Some(owner) = self.owner {
            let owner = canonicalize_account_literal(&owner, "--owner")?;
            public_obj.insert("owner".to_owned(), json_value(&owner)?);
        }
        if let Some(amount) = self.amount {
            public_obj.insert("amount".to_owned(), json_value(&amount)?);
        }
        if let Some(duration) = self.duration_blocks {
            public_obj.insert("duration_blocks".to_owned(), json_value(&duration)?);
        }
        if let Some(direction) = self.direction {
            public_obj.insert("direction".to_owned(), json_value(&direction)?);
        }
        if let Some(nullifier) = self.nullifier {
            let canonical = canonicalize_hex32(&nullifier)?;
            public_obj.insert("nullifier".to_owned(), json_value(&canonical)?);
        }
        ensure_lock_hints_complete(public_obj)?;
        normalize_public_input_owner(public_obj)?;
        let body = json_object(vec![
            ("election_id", json_value(&self.election_id)?),
            ("proof_b64", json_value(&self.proof_b64)?),
            ("public", public),
        ])?;
        let mut value = client.post_gov_ballot_zk_json(&body)?;
        let annotation = annotate_vote_instructions(&mut value)?;
        let ok = value
            .get("ok")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let accepted = value
            .get("accepted")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let reason = value.get("reason").and_then(|v| v.as_str()).unwrap_or("");
        let n_instr = value
            .get("tx_instructions")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let summary = format_vote_summary(&VoteSummaryInput {
            prefix: "vote zk",
            id_label: "election_id",
            id_value: &self.election_id,
            ok,
            accepted: Some(accepted),
            instruction_count: n_instr,
            reason,
            annotation: annotation.as_ref(),
        });
        print_with_summary(context, Some(summary), &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct VotePlainArgs {
    #[arg(long)]
    pub referendum_id: String,
    #[arg(long)]
    pub owner: String,
    #[arg(long)]
    pub amount: String,
    #[arg(long)]
    pub duration_blocks: u64,
    #[arg(long)]
    pub direction: String,
}

impl Run for VotePlainArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let owner = canonicalize_account_literal(&self.owner, "--owner")?;
        let body = json_object(vec![
            ("referendum_id", json_value(&self.referendum_id)?),
            ("owner", json_value(&owner)?),
            ("amount", json_value(&self.amount)?),
            ("duration_blocks", json_value(&self.duration_blocks)?),
            ("direction", json_value(&self.direction)?),
        ])?;
        let mut value = client.post_gov_ballot_plain_json(&body)?;
        let annotation = annotate_vote_instructions(&mut value)?;
        let ok = value
            .get("ok")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let accepted = value
            .get("accepted")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let reason = value.get("reason").and_then(|v| v.as_str()).unwrap_or("");
        let n_instr = value
            .get("tx_instructions")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let summary = format_vote_summary(&VoteSummaryInput {
            prefix: "vote plain",
            id_label: "referendum_id",
            id_value: &self.referendum_id,
            ok,
            accepted: Some(accepted),
            instruction_count: n_instr,
            reason,
            annotation: annotation.as_ref(),
        });
        print_with_summary(context, Some(summary), &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct ProposalGetArgs {
    #[arg(long, value_name = "ID_HEX")]
    pub id: String,
}

impl Run for ProposalGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_gov_proposal_json(&self.id)?;
        let found = value
            .get("found")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let summary = Some(format!("proposal get: id={} found={found}", self.id));
        print_with_summary(context, summary, &value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    #[test]
    fn parse_referendum_mode_detects_plain() {
        let value = norito::json!({
            "found": true,
            "referendum": {
                "mode": "Plain"
            }
        });
        assert_eq!(parse_referendum_mode(&value), Some(ResolvedVoteMode::Plain));
    }

    #[test]
    fn parse_referendum_mode_detects_zk_case_insensitive() {
        let value = norito::json!({
            "found": true,
            "referendum": {
                "mode": "zK"
            }
        });
        assert_eq!(parse_referendum_mode(&value), Some(ResolvedVoteMode::Zk));
    }

    #[test]
    fn parse_referendum_mode_missing_returns_none() {
        let value = norito::json!({
            "found": true,
            "referendum": {
                "status": "Open"
            }
        });
        assert_eq!(parse_referendum_mode(&value), None);
    }

    #[test]
    fn annotate_vote_plain_populates_summary() {
        let owner = ALICE_ID.clone();
        let owner_str = owner.to_string();
        let ballot = CastPlainBallot {
            referendum_id: "ref-plain".to_string(),
            owner,
            amount: 42,
            duration_blocks: 64,
            direction: 0,
        };
        let instruction: InstructionBox = InstructionBox::from(ballot);
        let bytes = norito::to_bytes::<InstructionBox>(&instruction).expect("encode instruction");
        let payload_hex = hex::encode(bytes);
        let mut value = norito::json!({
            "ok": true,
            "accepted": true,
            "tx_instructions": [{
                "wire_id": "CastPlainBallot",
                "payload_hex": payload_hex,
            }]
        });

        let summary = annotate_vote_instructions(&mut value)
            .expect("annotation result")
            .expect("summary present");

        assert!(
            summary
                .fingerprint_hex
                .as_ref()
                .is_some_and(|s| !s.is_empty())
        );
        assert_eq!(summary.owner.as_deref(), Some(owner_str.as_str()));
        assert_eq!(summary.amount.as_deref(), Some("42"));
        assert_eq!(summary.duration_blocks.as_deref(), Some("64"));
        assert_eq!(summary.direction.as_deref(), Some("Aye"));

        let line = format_vote_summary(&VoteSummaryInput {
            prefix: "vote plain",
            id_label: "referendum_id",
            id_value: "ref-plain",
            ok: true,
            accepted: Some(true),
            instruction_count: 1,
            reason: "",
            annotation: Some(&summary),
        });
        assert!(line.contains("fingerprint="));
        assert!(line.contains(&format!("owner={owner_str}")));
        assert!(line.contains("direction=Aye"));

        let instr = value
            .get("tx_instructions")
            .and_then(json::Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(json::Value::as_object)
            .expect("instruction object");
        assert!(instr.contains_key("payload_fingerprint_hex"));
        assert_eq!(
            instr.get("owner").and_then(json::Value::as_str),
            Some(owner_str.as_str())
        );
        assert_eq!(
            instr.get("direction").and_then(json::Value::as_str),
            Some("Aye")
        );
    }

    #[test]
    fn annotate_vote_zk_populates_hints() {
        let nullifier = "aa".repeat(32);
        let owner = BOB_ID.to_string();
        let ballot = CastZkBallot {
            election_id: "ref-zk".to_string(),
            proof_b64: "AAA=".to_string(),
            public_inputs_json: format!(
                r#"{{"owner":"{owner}","amount":"100","duration_blocks":256,"direction":"Nay","nullifier":"{nullifier}"}}"#
            ),
        };
        let instruction: InstructionBox = InstructionBox::from(ballot);
        let bytes = norito::to_bytes::<InstructionBox>(&instruction).expect("encode instruction");
        let payload_hex = hex::encode(bytes);
        let mut value = norito::json!({
            "ok": true,
            "accepted": false,
            "tx_instructions": [{
                "wire_id": "CastZkBallot",
                "payload_hex": payload_hex,
            }]
        });

        let summary = annotate_vote_instructions(&mut value)
            .expect("annotation result")
            .expect("summary present");

        assert!(
            summary
                .fingerprint_hex
                .as_ref()
                .is_some_and(|s| !s.is_empty())
        );
        assert_eq!(summary.owner.as_deref(), Some(owner.as_str()));
        assert_eq!(summary.amount.as_deref(), Some("100"));
        assert_eq!(summary.duration_blocks.as_deref(), Some("256"));
        assert_eq!(summary.direction.as_deref(), Some("Nay"));
        assert_eq!(summary.nullifier.as_deref(), Some(nullifier.as_str()));

        let line = format_vote_summary(&VoteSummaryInput {
            prefix: "vote zk",
            id_label: "election_id",
            id_value: "ref-zk",
            ok: true,
            accepted: Some(false),
            instruction_count: 1,
            reason: "",
            annotation: Some(&summary),
        });
        assert!(line.contains("fingerprint="));
        assert!(line.contains(&format!("owner={owner}")));
        assert!(line.contains("nullifier="));

        let instr = value
            .get("tx_instructions")
            .and_then(json::Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(json::Value::as_object)
            .expect("instruction object");
        assert!(instr.contains_key("payload_fingerprint_hex"));
        assert_eq!(
            instr.get("owner").and_then(json::Value::as_str),
            Some(owner.as_str())
        );
        assert_eq!(
            instr.get("nullifier").and_then(json::Value::as_str),
            Some(nullifier.as_str())
        );
    }

    #[test]
    fn lock_hints_require_complete_triplet() {
        let mut map = json::Map::new();
        let owner = ALICE_ID.to_string();
        map.insert("owner".to_string(), json::Value::String(owner));
        assert!(ensure_lock_hints_complete(&map).is_err());
        map.insert("amount".to_string(), json::Value::String("10".to_string()));
        assert!(ensure_lock_hints_complete(&map).is_err());
        map.insert("duration_blocks".to_string(), json::Value::from(64u64));
        assert!(ensure_lock_hints_complete(&map).is_ok());
    }

    #[test]
    fn normalize_public_inputs_canonicalizes_hex() {
        let mut map = json::Map::new();
        let root_raw = format!("0x{}", "Aa".repeat(32));
        let nullifier_raw = format!("blake2b32:{}", "BB".repeat(32));
        let root_expected = "aa".repeat(32);
        let nullifier_expected = "bb".repeat(32);
        map.insert("root_hint".to_string(), json::Value::String(root_raw));
        map.insert("nullifier".to_string(), json::Value::String(nullifier_raw));
        normalize_public_inputs(&mut map).expect("normalize");
        assert_eq!(
            map.get("root_hint").and_then(json::Value::as_str),
            Some(root_expected.as_str())
        );
        assert_eq!(
            map.get("nullifier").and_then(json::Value::as_str),
            Some(nullifier_expected.as_str())
        );
    }

    #[test]
    fn normalize_public_inputs_rejects_deprecated_keys() {
        let mut map = json::Map::new();
        map.insert(
            "nullifier_hex".to_string(),
            json::Value::String("aa".repeat(32)),
        );
        let err = normalize_public_inputs(&mut map).expect_err("deprecated key");
        assert!(err.to_string().contains("nullifier_hex"));
    }

    #[test]
    fn normalize_public_inputs_rejects_invalid_hex() {
        let mut map = json::Map::new();
        map.insert(
            "root_hint".to_string(),
            json::Value::String("not-hex".to_string()),
        );
        let err = normalize_public_inputs(&mut map).expect_err("invalid hex");
        assert!(err.to_string().contains("root_hint"));
    }

    #[test]
    fn public_inputs_reject_noncanonical_owner() {
        let owner = ALICE_ID.clone();
        let address_hex = owner.to_canonical_hex().expect("canonical hex");
        let noncanonical = format!("{address_hex}@hbl.dataspace");
        let mut map = json::Map::new();
        map.insert("owner".to_string(), json::Value::String(noncanonical));
        let err = normalize_public_input_owner(&mut map).expect_err("noncanonical owner");
        assert!(err.to_string().contains("must not include '@domain'"));
    }

    #[test]
    fn public_inputs_allow_implicit_owner_without_selector_resolver() {
        use iroha_crypto::{Algorithm, KeyPair};

        let key_pair = KeyPair::from_seed(vec![0x55; 32], Algorithm::Ed25519);
        let owner = AccountId::new(key_pair.public_key().clone()).to_string();
        let mut map = json::Map::new();
        map.insert("owner".to_string(), json::Value::String(owner));
        assert!(
            normalize_public_input_owner(&mut map).is_ok(),
            "implicit owner should be accepted when selector resolution is unavailable"
        );
    }

    #[test]
    fn public_inputs_reject_compressed_owner() {
        use iroha_crypto::{Algorithm, KeyPair};

        let owner = KeyPair::from_seed(vec![5; 32], Algorithm::Ed25519)
            .public_key()
            .to_string();
        let mut map = json::Map::new();
        map.insert("owner".to_string(), json::Value::String(owner));
        let err = normalize_public_input_owner(&mut map).expect_err("compressed owner");
        assert!(err.to_string().contains("canonical I105 account id"));
    }
}

#[derive(clap::Args, Debug)]
pub struct LocksGetArgs {
    #[arg(long)]
    pub referendum_id: String,
}

impl Run for LocksGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_gov_locks_json(&self.referendum_id)?;
        let found = value
            .get("found")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let voters = value
            .get("locks")
            .and_then(|v| v.get("locks"))
            .and_then(norito::json::Value::as_object)
            .map_or(0, norito::json::Map::len);
        let summary = Some(format!(
            "locks get: referendum={} found={found} voters={voters}",
            self.referendum_id
        ));
        print_with_summary(context, summary, &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct UnlockStatsArgs {}

impl Run for UnlockStatsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_gov_unlock_stats_json()?;
        let height = value
            .get("height_current")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let expired = value
            .get("expired_locks_now")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let refs = value
            .get("referenda_with_expired")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let last = value
            .get("last_sweep_height")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let summary = Some(format!(
            "unlock stats: height={height} expired={expired} refs={refs} last_sweep={last}"
        ));
        print_with_summary(context, summary, &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct ReferendumGetArgs {
    #[arg(long = "referendum-id", alias = "id")]
    pub referendum_id: String,
}

impl Run for ReferendumGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_gov_referendum_json(&self.referendum_id)?;
        let found = value
            .get("found")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let summary = Some(format!(
            "referendum get: id={} found={found}",
            self.referendum_id
        ));
        print_with_summary(context, summary, &value)
    }
}

#[derive(clap::Args, Debug)]
pub struct TallyGetArgs {
    #[arg(long = "referendum-id", alias = "id")]
    pub referendum_id: String,
}

impl Run for TallyGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client: Client = context.client_from_config();
        let value = client.get_gov_tally_json(&self.referendum_id)?;
        let approve = value
            .get("approve")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let reject = value
            .get("reject")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let abstain = value
            .get("abstain")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let summary = Some(format!(
            "tally get: id={} approve={} reject={} abstain={}",
            self.referendum_id, approve, reject, abstain
        ));
        print_with_summary(context, summary, &value)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct VoteInstructionSummary {
    fingerprint_hex: Option<String>,
    owner: Option<String>,
    amount: Option<String>,
    duration_blocks: Option<String>,
    direction: Option<String>,
    nullifier: Option<String>,
}

fn annotate_vote_instructions(value: &mut json::Value) -> Result<Option<VoteInstructionSummary>> {
    let Some(instructions) = value
        .get_mut("tx_instructions")
        .and_then(json::Value::as_array_mut)
    else {
        return Ok(None);
    };

    let mut summary = VoteInstructionSummary::default();
    let mut saw_relevant = false;

    for entry in instructions {
        let Some(map) = entry.as_object_mut() else {
            continue;
        };
        let Some(payload_hex) = map.get("payload_hex").and_then(json::Value::as_str) else {
            continue;
        };
        let payload_bytes = decode_payload_hex(payload_hex)?;
        let fingerprint = CryptoHash::new(&payload_bytes).to_string();
        map.insert(
            "payload_fingerprint_hex".to_owned(),
            json::Value::String(fingerprint.clone()),
        );
        if summary.fingerprint_hex.is_none() {
            summary.fingerprint_hex = Some(fingerprint);
        }

        let instruction: InstructionBox = decode_from_bytes(&payload_bytes)
            .map_err(|err| eyre!("failed to decode instruction skeleton: {err}"))?;

        if let Some(plain) = instruction.as_any().downcast_ref::<CastPlainBallot>() {
            saw_relevant = true;
            let owner_str = plain.owner.to_string();
            map.insert("owner".to_owned(), json::Value::String(owner_str.clone()));
            summary.owner.get_or_insert(owner_str);

            let amount_str = plain.amount.to_string();
            map.insert("amount".to_owned(), json::Value::String(amount_str.clone()));
            summary.amount.get_or_insert(amount_str);

            let duration_str = plain.duration_blocks.to_string();
            map.insert(
                "duration_blocks".to_owned(),
                json::Value::String(duration_str.clone()),
            );
            summary.duration_blocks.get_or_insert(duration_str);

            let direction = direction_label_from_u8(plain.direction).to_string();
            map.insert(
                "direction".to_owned(),
                json::Value::String(direction.clone()),
            );
            summary.direction.get_or_insert(direction);
        } else if let Some(zk) = instruction.as_any().downcast_ref::<CastZkBallot>() {
            saw_relevant = true;
            if let Ok(hints_value) = json::from_str::<json::Value>(&zk.public_inputs_json)
                && let Some(hints) = hints_value.as_object()
            {
                if let Some(owner_val) = hints.get("owner") {
                    let owner = stringify_json_value(owner_val);
                    map.insert("owner".to_owned(), json::Value::String(owner.clone()));
                    summary.owner.get_or_insert(owner);
                }
                if let Some(amount_val) = hints.get("amount") {
                    let amount = stringify_json_value(amount_val);
                    map.insert("amount".to_owned(), json::Value::String(amount.clone()));
                    summary.amount.get_or_insert(amount);
                }
                if let Some(duration_val) = hints.get("duration_blocks") {
                    let duration = stringify_json_value(duration_val);
                    map.insert(
                        "duration_blocks".to_owned(),
                        json::Value::String(duration.clone()),
                    );
                    summary.duration_blocks.get_or_insert(duration);
                }
                if let Some(direction_val) = hints.get("direction") {
                    let direction = direction_from_hint_value(direction_val)
                        .unwrap_or_else(|| stringify_json_value(direction_val));
                    map.insert(
                        "direction".to_owned(),
                        json::Value::String(direction.clone()),
                    );
                    summary.direction.get_or_insert(direction);
                }
                if let Some(nullifier_val) = hints.get("nullifier") {
                    let nullifier = stringify_json_value(nullifier_val);
                    map.insert(
                        "nullifier".to_owned(),
                        json::Value::String(nullifier.clone()),
                    );
                    summary.nullifier.get_or_insert(nullifier);
                }
            }
        }
    }

    if summary.fingerprint_hex.is_some() || saw_relevant {
        Ok(Some(summary))
    } else {
        Ok(None)
    }
}

fn decode_payload_hex(hex_str: &str) -> Result<Vec<u8>> {
    let trimmed = hex_str.trim();
    let cleaned = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    hex::decode(cleaned).map_err(|err| eyre!("failed to decode payload hex: {err}"))
}

fn direction_label_from_u8(code: u8) -> &'static str {
    match code {
        0 => "Aye",
        1 => "Nay",
        2 => "Abstain",
        _ => "Unknown",
    }
}

fn direction_from_hint_value(value: &json::Value) -> Option<String> {
    fn from_u64(n: u64) -> String {
        u8::try_from(n).map_or_else(
            |_| n.to_string(),
            |code| direction_label_from_u8(code).to_string(),
        )
    }
    fn from_i64(n: i64) -> String {
        u8::try_from(n).map_or_else(
            |_| n.to_string(),
            |code| direction_label_from_u8(code).to_string(),
        )
    }

    value
        .as_str()
        .map(ToString::to_string)
        .or_else(|| value.as_u64().map(from_u64))
        .or_else(|| value.as_i64().map(from_i64))
}

fn stringify_json_value(value: &json::Value) -> String {
    match value {
        json::Value::String(s) => s.clone(),
        json::Value::Number(n) => match n {
            json::Number::U64(v) => v.to_string(),
            json::Number::I64(v) => v.to_string(),
            json::Number::F64(v) => v.to_string(),
        },
        json::Value::Bool(b) => b.to_string(),
        json::Value::Null => "null".to_owned(),
        _ => norito::json::to_json(value).unwrap_or_else(|_| "<invalid json>".to_owned()),
    }
}

struct VoteSummaryInput<'a> {
    prefix: &'a str,
    id_label: &'a str,
    id_value: &'a str,
    ok: bool,
    accepted: Option<bool>,
    instruction_count: usize,
    reason: &'a str,
    annotation: Option<&'a VoteInstructionSummary>,
}

fn format_vote_summary(input: &VoteSummaryInput<'_>) -> String {
    let prefix = input.prefix;
    let id_label = input.id_label;
    let id_value = input.id_value;
    let ok = input.ok;
    let mut parts = vec![format!("{prefix}: {id_label}={id_value} ok={ok}")];
    if let Some(acc) = input.accepted {
        parts.push(format!("accepted={acc}"));
    }
    let instrs = input.instruction_count;
    parts.push(format!("instrs={instrs}"));
    if let Some(summary) = input.annotation {
        if let Some(fp) = &summary.fingerprint_hex {
            parts.push(format!("fingerprint={fp}"));
        }
        if let Some(owner) = &summary.owner {
            parts.push(format!("owner={owner}"));
        }
        if let Some(amount) = &summary.amount {
            parts.push(format!("amount={amount}"));
        }
        if let Some(duration) = &summary.duration_blocks {
            parts.push(format!("duration_blocks={duration}"));
        }
        if let Some(direction) = &summary.direction {
            parts.push(format!("direction={direction}"));
        }
        if let Some(nullifier) = &summary.nullifier {
            parts.push(format!("nullifier={nullifier}"));
        }
    }
    if !input.reason.is_empty() {
        let reason = input.reason;
        parts.push(format!("reason=\"{reason}\""));
    }
    parts.join(" ")
}
