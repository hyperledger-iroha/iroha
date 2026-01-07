//! Governance voting and query helpers.

use crate::{
    Run, RunContext,
    json_utils::{json_object, json_value},
};
use clap::ValueEnum;
use eyre::{Result, eyre};
use iroha::client::Client;
use iroha::data_model::isi::{
    InstructionBox,
    governance::{CastPlainBallot, CastZkBallot},
};
use iroha_crypto::Hash as CryptoHash;
use norito::{decode_from_bytes, json};

use super::shared::{canonicalize_hex32, print_with_summary, validate_summary_flags};

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
        .map(|value| !matches!(value, json::Value::Null))
        .unwrap_or(false)
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

fn normalize_public_input_aliases(map: &mut json::Map) -> Result<()> {
    normalize_public_input_alias(map, "durationBlocks", "duration_blocks")?;
    normalize_public_input_alias(map, "nullifierHex", "nullifier_hex")?;
    normalize_public_input_alias(map, "rootHintHex", "root_hint")?;
    normalize_public_input_alias(map, "rootHint", "root_hint")?;
    canonicalize_public_input_hex(map, "root_hint")?;
    canonicalize_public_input_hex(map, "nullifier_hex")?;
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

fn normalize_public_input_alias(
    map: &mut json::Map,
    alias: &str,
    canonical: &str,
) -> Result<()> {
    if !map.contains_key(alias) {
        return Ok(());
    }
    if map.contains_key(canonical) {
        return Err(eyre!(
            "public inputs JSON cannot include both {alias} and {canonical}"
        ));
    }
    if let Some(value) = map.remove(alias) {
        map.insert(canonical.to_owned(), value);
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
    /// Owner account id for plain voting mode (must equal transaction authority).
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
    #[arg(long = "nullifier-hex")]
    pub nullifier_hex: Option<String>,
    /// Print only the compact summary line (suppresses raw JSON)
    #[arg(long, default_value_t = false)]
    pub summary_only: bool,
    /// Suppress the compact summary line (print raw JSON only)
    #[arg(long, default_value_t = false)]
    pub no_summary: bool,
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
            nullifier_hex,
            summary_only,
            no_summary,
        } = self;

        validate_summary_flags(summary_only, no_summary)?;
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
                    nullifier_hex,
                    summary_only,
                    no_summary,
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
                    summary_only,
                    no_summary,
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
    /// Optional owner hint mirrored into public inputs.
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
    #[arg(long = "nullifier-hex")]
    pub nullifier_hex: Option<String>,
    /// Print only the compact summary line (suppresses raw JSON)
    #[arg(long, default_value_t = false)]
    pub summary_only: bool,
    /// Suppress the compact summary line (print raw JSON only)
    #[arg(long, default_value_t = false)]
    pub no_summary: bool,
}

impl Run for VoteZkArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_summary_flags(self.summary_only, self.no_summary)?;
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
        normalize_public_input_aliases(public_obj)?;
        if let Some(owner) = self.owner {
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
        if let Some(nullifier_hex) = self.nullifier_hex {
            let canonical = canonicalize_hex32(&nullifier_hex)?;
            public_obj.insert("nullifier_hex".to_owned(), json_value(&canonical)?);
        }
        ensure_lock_hints_complete(public_obj)?;
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
            prefix: "vote-zk",
            id_label: "election_id",
            id_value: &self.election_id,
            ok,
            accepted: Some(accepted),
            instruction_count: n_instr,
            reason,
            annotation: annotation.as_ref(),
        });
        print_with_summary(
            context,
            Some(summary),
            &value,
            self.summary_only,
            self.no_summary,
        )
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
    /// Print only the compact summary line (suppresses raw JSON)
    #[arg(long, default_value_t = false)]
    pub summary_only: bool,
    /// Suppress the compact summary line (print raw JSON only)
    #[arg(long, default_value_t = false)]
    pub no_summary: bool,
}

impl Run for VotePlainArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_summary_flags(self.summary_only, self.no_summary)?;
        let client: Client = context.client_from_config();
        let body = json_object(vec![
            ("referendum_id", json_value(&self.referendum_id)?),
            ("owner", json_value(&self.owner)?),
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
            prefix: "vote-plain",
            id_label: "referendum_id",
            id_value: &self.referendum_id,
            ok,
            accepted: Some(accepted),
            instruction_count: n_instr,
            reason,
            annotation: annotation.as_ref(),
        });
        print_with_summary(
            context,
            Some(summary),
            &value,
            self.summary_only,
            self.no_summary,
        )
    }
}

#[derive(clap::Args, Debug)]
pub struct ProposalGetArgs {
    #[arg(long, value_name = "ID_HEX")]
    pub id: String,
    /// Print only the compact summary line (suppresses raw JSON)
    #[arg(long, default_value_t = false)]
    pub summary_only: bool,
    /// Suppress the compact summary line (print raw JSON only)
    #[arg(long, default_value_t = false)]
    pub no_summary: bool,
}

impl Run for ProposalGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_summary_flags(self.summary_only, self.no_summary)?;
        let client: Client = context.client_from_config();
        let value = client.get_gov_proposal_json(&self.id)?;
        if !self.no_summary {
            let found = value
                .get("found")
                .and_then(norito::json::Value::as_bool)
                .unwrap_or(false);
            let _ = context.println(format!("proposal-get: id={} found={}", self.id, found));
        }
        if !self.summary_only {
            context.print_data(&value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::prelude::AccountId;
    use std::str::FromStr;

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
        const SIGNATORY: &str =
            "ed25519:ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C";
        let owner_input = format!("{SIGNATORY}@wonderland");
        let owner = AccountId::from_str(&owner_input).expect("valid account id");
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
            prefix: "vote-plain",
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
        let ballot = CastZkBallot {
            election_id: "ref-zk".to_string(),
            proof_b64: "AAA=".to_string(),
            public_inputs_json: format!(
                r#"{{"owner":"bob@wonderland","amount":"100","duration_blocks":256,"direction":"Nay","nullifier_hex":"{nullifier}"}}"#
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
        assert_eq!(summary.owner.as_deref(), Some("bob@wonderland"));
        assert_eq!(summary.amount.as_deref(), Some("100"));
        assert_eq!(summary.duration_blocks.as_deref(), Some("256"));
        assert_eq!(summary.direction.as_deref(), Some("Nay"));
        assert_eq!(summary.nullifier_hex.as_deref(), Some(nullifier.as_str()));

        let line = format_vote_summary(&VoteSummaryInput {
            prefix: "vote-zk",
            id_label: "election_id",
            id_value: "ref-zk",
            ok: true,
            accepted: Some(false),
            instruction_count: 1,
            reason: "",
            annotation: Some(&summary),
        });
        assert!(line.contains("fingerprint="));
        assert!(line.contains("owner=bob@wonderland"));
        assert!(line.contains("nullifier_hex="));

        let instr = value
            .get("tx_instructions")
            .and_then(json::Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(json::Value::as_object)
            .expect("instruction object");
        assert!(instr.contains_key("payload_fingerprint_hex"));
        assert_eq!(
            instr.get("owner").and_then(json::Value::as_str),
            Some("bob@wonderland")
        );
        assert_eq!(
            instr.get("nullifier_hex").and_then(json::Value::as_str),
            Some(nullifier.as_str())
        );
    }

    #[test]
    fn lock_hints_require_complete_triplet() {
        let mut map = json::Map::new();
        map.insert(
            "owner".to_string(),
            json::Value::String("alice@wonderland".to_string()),
        );
        assert!(ensure_lock_hints_complete(&map).is_err());
        map.insert(
            "amount".to_string(),
            json::Value::String("10".to_string()),
        );
        assert!(ensure_lock_hints_complete(&map).is_err());
        map.insert("duration_blocks".to_string(), json::Value::from(64u64));
        assert!(ensure_lock_hints_complete(&map).is_ok());
    }

    #[test]
    fn normalize_public_input_aliases_maps_keys() {
        let mut map = json::Map::new();
        let root_raw = format!("0x{}", "Aa".repeat(32));
        let nullifier_raw = format!("blake2b32:{}", "BB".repeat(32));
        let root_expected = "aa".repeat(32);
        let nullifier_expected = "bb".repeat(32);
        map.insert("durationBlocks".to_string(), json::Value::from(64u64));
        map.insert(
            "rootHintHex".to_string(),
            json::Value::String(root_raw),
        );
        map.insert(
            "nullifierHex".to_string(),
            json::Value::String(nullifier_raw),
        );
        normalize_public_input_aliases(&mut map).expect("normalize aliases");
        assert!(map.contains_key("duration_blocks"));
        assert!(map.contains_key("root_hint"));
        assert!(map.contains_key("nullifier_hex"));
        assert!(!map.contains_key("durationBlocks"));
        assert!(!map.contains_key("rootHintHex"));
        assert!(!map.contains_key("nullifierHex"));
        assert_eq!(
            map.get("root_hint").and_then(json::Value::as_str),
            Some(root_expected.as_str())
        );
        assert_eq!(
            map.get("nullifier_hex").and_then(json::Value::as_str),
            Some(nullifier_expected.as_str())
        );
    }

    #[test]
    fn normalize_public_input_aliases_rejects_conflicts() {
        let mut map = json::Map::new();
        map.insert("rootHint".to_string(), json::Value::String("aa".repeat(32)));
        map.insert(
            "root_hint".to_string(),
            json::Value::String("bb".repeat(32)),
        );
        let err = normalize_public_input_aliases(&mut map).expect_err("alias conflict");
        assert!(err.to_string().contains("rootHint"));
    }

    #[test]
    fn normalize_public_input_aliases_rejects_invalid_hex() {
        let mut map = json::Map::new();
        map.insert(
            "root_hint".to_string(),
            json::Value::String("not-hex".to_string()),
        );
        let err = normalize_public_input_aliases(&mut map).expect_err("invalid hex");
        assert!(err.to_string().contains("root_hint"));
    }
}

#[derive(clap::Args, Debug)]
pub struct LocksGetArgs {
    #[arg(long)]
    pub referendum_id: String,
    /// Print only the compact summary line (suppresses raw JSON)
    #[arg(long, default_value_t = false)]
    pub summary_only: bool,
    /// Suppress the compact summary line (print raw JSON only)
    #[arg(long, default_value_t = false)]
    pub no_summary: bool,
}

impl Run for LocksGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_summary_flags(self.summary_only, self.no_summary)?;
        let client: Client = context.client_from_config();
        let value = client.get_gov_locks_json(&self.referendum_id)?;
        if !self.no_summary {
            let found = value
                .get("found")
                .and_then(norito::json::Value::as_bool)
                .unwrap_or(false);
            let voters = value
                .get("locks")
                .and_then(|v| v.get("locks"))
                .and_then(norito::json::Value::as_object)
                .map_or(0, norito::json::Map::len);
            let referendum_id = &self.referendum_id;
            let _ = context.println(format!(
                "locks-get: referendum={referendum_id} found={found} voters={voters}"
            ));
        }
        if !self.summary_only {
            context.print_data(&value)?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct UnlockStatsArgs {
    /// Print only the compact summary line (suppresses raw JSON)
    #[arg(long, default_value_t = false)]
    pub summary_only: bool,
    /// Suppress the compact summary line (print raw JSON only)
    #[arg(long, default_value_t = false)]
    pub no_summary: bool,
}

impl Run for UnlockStatsArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_summary_flags(self.summary_only, self.no_summary)?;
        let client: Client = context.client_from_config();
        let value = client.get_gov_unlock_stats_json()?;
        if !self.no_summary {
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
            let _ = context.println(format!(
                "unlock-stats: height={height} expired={expired} refs={refs} last_sweep={last}"
            ));
        }
        if !self.summary_only {
            context.print_data(&value)?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct ReferendumGetArgs {
    #[arg(long = "referendum-id", alias = "id")]
    pub referendum_id: String,
    /// Print only the compact summary line (suppresses raw JSON)
    #[arg(long, default_value_t = false)]
    pub summary_only: bool,
    /// Suppress the compact summary line (print raw JSON only)
    #[arg(long, default_value_t = false)]
    pub no_summary: bool,
}

impl Run for ReferendumGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_summary_flags(self.summary_only, self.no_summary)?;
        let client: Client = context.client_from_config();
        let value = client.get_gov_referendum_json(&self.referendum_id)?;
        if !self.no_summary {
            let found = value
                .get("found")
                .and_then(norito::json::Value::as_bool)
                .unwrap_or(false);
            let referendum_id = &self.referendum_id;
            let _ = context.println(format!("referendum-get: id={referendum_id} found={found}"));
        }
        if !self.summary_only {
            context.print_data(&value)?;
        }
        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct TallyGetArgs {
    #[arg(long = "referendum-id", alias = "id")]
    pub referendum_id: String,
    /// Print only the compact summary line (suppresses raw JSON)
    #[arg(long, default_value_t = false)]
    pub summary_only: bool,
    /// Suppress the compact summary line (print raw JSON only)
    #[arg(long, default_value_t = false)]
    pub no_summary: bool,
}

impl Run for TallyGetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        validate_summary_flags(self.summary_only, self.no_summary)?;
        let client: Client = context.client_from_config();
        let value = client.get_gov_tally_json(&self.referendum_id)?;
        if !self.no_summary {
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
            let _ = context.println(format!(
                "tally: id={} approve={} reject={} abstain={}",
                self.referendum_id, approve, reject, abstain
            ));
        }
        if !self.summary_only {
            context.print_data(&value)?;
        }
        Ok(())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct VoteInstructionSummary {
    fingerprint_hex: Option<String>,
    owner: Option<String>,
    amount: Option<String>,
    duration_blocks: Option<String>,
    direction: Option<String>,
    nullifier_hex: Option<String>,
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
                if let Some(nullifier_val) = hints.get("nullifier_hex") {
                    let nullifier = stringify_json_value(nullifier_val);
                    map.insert(
                        "nullifier_hex".to_owned(),
                        json::Value::String(nullifier.clone()),
                    );
                    summary.nullifier_hex.get_or_insert(nullifier);
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
        if let Some(nullifier) = &summary.nullifier_hex {
            parts.push(format!("nullifier_hex={nullifier}"));
        }
    }
    if !input.reason.is_empty() {
        let reason = input.reason;
        parts.push(format!("reason=\"{reason}\""));
    }
    parts.join(" ")
}
