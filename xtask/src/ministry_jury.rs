use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use blake2::digest::Digest;
use hex::{FromHex, decode as hex_decode};
use iroha_crypto::Blake2b256;
use iroha_data_model::ministry::{
    POLICY_JURY_BALLOT_COMMIT_VERSION_V1, POLICY_JURY_BALLOT_REVEAL_VERSION_V1,
    POLICY_JURY_SORTITION_VERSION_V1, PolicyJuryAssignment, PolicyJuryBallotCommitV1,
    PolicyJuryBallotMode, PolicyJuryBallotRevealV1, PolicyJuryFailoverPlan, PolicyJurySortitionV1,
    PolicyJuryVoteChoice, PolicyJuryWaitlistEntry,
};
use rand::{
    SeedableRng,
    distr::{Distribution, weighted::WeightedIndex},
};
use rand_chacha::ChaCha20Rng;
use rand_core_06::{OsRng as SecureOsRng, RngCore as SecureRngCore};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

pub enum Command {
    Sortition(SortitionOptions),
    BallotCommit(BallotCommitOptions),
    BallotVerify(BallotVerifyOptions),
}

#[derive(Clone)]
pub struct SortitionOptions {
    pub roster_path: PathBuf,
    pub proposal_id: String,
    pub round_id: String,
    pub beacon_hex: String,
    pub committee_size: usize,
    pub waitlist_size: usize,
    pub drawn_at: OffsetDateTime,
    pub waitlist_ttl_hours: u32,
    pub grace_period_secs: u32,
    pub failover_grace_secs: u32,
    pub output_path: Option<PathBuf>,
}

#[derive(Clone)]
pub struct BallotCommitOptions {
    pub proposal_id: String,
    pub round_id: String,
    pub juror_id: String,
    pub choice: PolicyJuryVoteChoice,
    pub nonce: Vec<u8>,
    pub committed_at: OffsetDateTime,
    pub revealed_at: Option<OffsetDateTime>,
    pub output_path: Option<PathBuf>,
    pub reveal_output_path: Option<PathBuf>,
}

#[derive(Clone)]
pub struct BallotVerifyOptions {
    pub commit_path: PathBuf,
    pub reveal_path: PathBuf,
}

pub fn run(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Sortition(options) => {
            let manifest = run_sortition(&options)?;
            let json = norito::json::to_json_pretty(&manifest).map_err(|err| {
                format!("failed to serialize policy jury sortition manifest: {err}")
            })?;
            if let Some(path) = options.output_path {
                fs::write(&path, json.as_bytes()).map_err(|err| {
                    format!(
                        "failed to write policy jury sortition manifest to {}: {err}",
                        path.display()
                    )
                })?;
                println!(
                    "wrote policy jury sortition manifest for {} to {}",
                    options.round_id,
                    path.display()
                );
            } else {
                println!("{json}");
            }
            Ok(())
        }
        Command::BallotCommit(options) => run_ballot_commit(&options),
        Command::BallotVerify(options) => run_ballot_verify(&options),
    }
}

fn run_sortition(options: &SortitionOptions) -> Result<PolicyJurySortitionV1, Box<dyn Error>> {
    let roster_bytes = fs::read(&options.roster_path).map_err(|err| {
        format!(
            "failed to read policy jury roster `{}`: {err}",
            options.roster_path.display()
        )
    })?;
    let roster: JuryRoster = serde_json::from_slice(&roster_bytes).map_err(|err| {
        format!(
            "failed to parse policy jury roster JSON from `{}`: {err}",
            options.roster_path.display()
        )
    })?;
    roster.validate()?;

    if options.committee_size == 0 {
        return Err("policy jury sortition requires --committee-size >= 1".into());
    }
    let total_required = options
        .committee_size
        .checked_add(options.waitlist_size)
        .ok_or("committee + waitlist size overflow")?;

    let eligible = roster.eligible_jurors()?;
    if eligible.len() < total_required {
        return Err(format!(
            "roster contains {} eligible juror(s) but the draw requires {total_required}",
            eligible.len()
        )
        .into());
    }

    let beacon = parse_beacon(&options.beacon_hex)?;
    let draws = perform_draws(&eligible, total_required, beacon)?;

    let drawn_at_ms = unix_millis(options.drawn_at)?;
    let waitlist_expiry_ms = unix_millis(calculate_waitlist_expiry(
        options.drawn_at,
        options.waitlist_ttl_hours,
    )?)?;
    let snapshot_digest = digest_roster(&roster_bytes);

    let default_grace = roster
        .default_grace_period_secs
        .unwrap_or(options.grace_period_secs);
    let assignments = draws
        .iter()
        .take(options.committee_size)
        .enumerate()
        .map(|(slot, juror)| {
            let candidate = &eligible[juror.index];
            let grace = candidate.member.grace_period_secs.unwrap_or(default_grace);
            let failover = if slot < options.waitlist_size {
                Some(PolicyJuryFailoverPlan {
                    waitlist_rank: u32::try_from(slot).unwrap_or(u32::MAX) + 1,
                    escalate_after_secs: options.failover_grace_secs,
                })
            } else {
                None
            };
            PolicyJuryAssignment {
                slot: u32::try_from(slot).unwrap_or(u32::MAX),
                juror_id: candidate.member.juror_id.clone(),
                pop_identity: candidate.member.pop_identity.clone(),
                grace_period_secs: grace,
                failover,
            }
        })
        .collect();

    let waitlist: Vec<PolicyJuryWaitlistEntry> = draws
        .iter()
        .skip(options.committee_size)
        .enumerate()
        .map(|(index, juror)| {
            let candidate = &eligible[juror.index];
            PolicyJuryWaitlistEntry {
                rank: u32::try_from(index).unwrap_or(u32::MAX) + 1,
                juror_id: candidate.member.juror_id.clone(),
                pop_identity: candidate.member.pop_identity.clone(),
                expires_at_unix_ms: waitlist_expiry_ms,
            }
        })
        .collect();

    let manifest = PolicyJurySortitionV1 {
        version: POLICY_JURY_SORTITION_VERSION_V1,
        proposal_id: options.proposal_id.clone(),
        round_id: options.round_id.clone(),
        drawn_at_unix_ms: drawn_at_ms,
        pop_snapshot_digest_blake2b_256: snapshot_digest,
        randomness_beacon: beacon,
        committee_size: u32::try_from(options.committee_size).unwrap_or(u32::MAX),
        assignments,
        waitlist,
    };
    manifest
        .validate()
        .map_err(|err| format!("generated sortition manifest failed validation: {err}"))?;
    Ok(manifest)
}

fn run_ballot_commit(options: &BallotCommitOptions) -> Result<(), Box<dyn Error>> {
    let (commit, reveal) = build_ballot_pair(options)?;
    write_json_artifact(
        &commit,
        options.output_path.as_deref(),
        "policy jury ballot commit",
    )?;
    if let Some(path) = options.reveal_output_path.as_deref() {
        write_json_artifact(&reveal, Some(path), "policy jury ballot reveal")?;
    }
    Ok(())
}

fn run_ballot_verify(options: &BallotVerifyOptions) -> Result<(), Box<dyn Error>> {
    let commit_bytes = fs::read(&options.commit_path).map_err(|err| {
        format!(
            "failed to read ballot commitment `{}`: {err}",
            options.commit_path.display()
        )
    })?;
    let reveal_bytes = fs::read(&options.reveal_path).map_err(|err| {
        format!(
            "failed to read ballot reveal `{}`: {err}",
            options.reveal_path.display()
        )
    })?;
    let commit: PolicyJuryBallotCommitV1 = norito::json::from_slice(&commit_bytes)
        .map_err(|err| format!("failed to parse ballot commitment JSON: {err}"))?;
    let reveal: PolicyJuryBallotRevealV1 = norito::json::from_slice(&reveal_bytes)
        .map_err(|err| format!("failed to parse ballot reveal JSON: {err}"))?;
    commit
        .verify_reveal(&reveal)
        .map_err(|err| format!("ballot verification failed: {err}"))?;
    println!(
        "verified ballot reveal for juror {} in round {}",
        commit.juror_id, commit.round_id
    );
    Ok(())
}

fn build_ballot_pair(
    options: &BallotCommitOptions,
) -> Result<(PolicyJuryBallotCommitV1, PolicyJuryBallotRevealV1), Box<dyn Error>> {
    ensure_nonce_len(&options.nonce)?;
    let reveal_at = options.revealed_at.unwrap_or(options.committed_at);
    let reveal = PolicyJuryBallotRevealV1 {
        version: POLICY_JURY_BALLOT_REVEAL_VERSION_V1,
        proposal_id: options.proposal_id.clone(),
        round_id: options.round_id.clone(),
        juror_id: options.juror_id.clone(),
        choice: options.choice,
        nonce: options.nonce.clone(),
        revealed_at_unix_ms: unix_millis(reveal_at)?,
        #[cfg(feature = "zk-ballot")]
        zk_proof_uris: Vec::new(),
    };
    let commit = PolicyJuryBallotCommitV1 {
        version: POLICY_JURY_BALLOT_COMMIT_VERSION_V1,
        proposal_id: options.proposal_id.clone(),
        round_id: options.round_id.clone(),
        juror_id: options.juror_id.clone(),
        commitment_blake2b_256: reveal.compute_commitment(),
        committed_at_unix_ms: unix_millis(options.committed_at)?,
        mode: PolicyJuryBallotMode::Plaintext,
    };
    commit
        .verify_reveal(&reveal)
        .map_err(|err| format!("generated ballot failed verification: {err}"))?;
    Ok((commit, reveal))
}

fn write_json_artifact<T>(value: &T, path: Option<&Path>, label: &str) -> Result<(), Box<dyn Error>>
where
    T: norito::json::JsonSerialize + ?Sized,
{
    let json = norito::json::to_json_pretty(value)
        .map_err(|err| format!("failed to serialize {label}: {err}"))?;
    if let Some(target) = path {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create parent directory for {}: {err}",
                    target.display()
                )
            })?;
        }
        fs::write(target, json.as_bytes())
            .map_err(|err| format!("failed to write {label} to {}: {err}", target.display()))?;
        println!("wrote {label} to {}", target.display());
    } else {
        println!("{json}");
    }
    Ok(())
}

fn unix_millis(timestamp: OffsetDateTime) -> Result<u64, Box<dyn Error>> {
    let nanos = timestamp.unix_timestamp_nanos();
    if nanos < 0 {
        return Err("policy jury sortition timestamps must be >= Unix epoch".into());
    }
    Ok(u64::try_from(nanos / 1_000_000)
        .map_err(|err| format!("failed to convert timestamp to milliseconds: {err}"))?)
}

fn calculate_waitlist_expiry(
    drawn_at: OffsetDateTime,
    ttl_hours: u32,
) -> Result<OffsetDateTime, Box<dyn Error>> {
    let duration = Duration::hours(i64::from(ttl_hours));
    drawn_at
        .checked_add(duration)
        .ok_or_else(|| "waitlist TTL caused overflow when computing expiry timestamp".into())
}

fn parse_beacon(input: &str) -> Result<[u8; 32], Box<dyn Error>> {
    let trimmed = input.trim();
    if trimmed.len() != 64 {
        return Err(format!(
            "policy jury beacon must be 64 hex characters (32 bytes), got length {}",
            trimmed.len()
        )
        .into());
    }
    <[u8; 32]>::from_hex(trimmed)
        .map_err(|err| format!("failed to decode randomness beacon hex: {err}").into())
}

fn digest_roster(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2b256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn perform_draws(
    members: &[EligibleJuror],
    picks: usize,
    beacon: [u8; 32],
) -> Result<Vec<DrawRef>, Box<dyn Error>> {
    let mut rng = ChaCha20Rng::from_seed(beacon);
    let mut remaining: Vec<usize> = (0..members.len()).collect();
    let mut draws = Vec::with_capacity(picks);
    for _ in 0..picks {
        let weights: Vec<u64> = remaining
            .iter()
            .map(|idx| members[*idx].member.weight)
            .collect();
        let dist = WeightedIndex::new(&weights)
            .map_err(|err| format!("invalid roster weights for policy jury sortition: {err}"))?;
        let pick = dist.sample(&mut rng);
        let roster_index = remaining.swap_remove(pick);
        draws.push(DrawRef {
            index: roster_index,
        });
    }
    Ok(draws)
}

#[derive(Deserialize)]
struct JuryRoster {
    format_version: u32,
    jurors: Vec<JuryCandidate>,
    #[serde(default)]
    default_grace_period_secs: Option<u32>,
}

impl JuryRoster {
    fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.format_version != 1 {
            return Err(format!(
                "unsupported policy jury roster format_version {}; expected 1",
                self.format_version
            )
            .into());
        }
        if self.jurors.is_empty() {
            return Err("policy jury roster contains no juror entries".into());
        }
        Ok(())
    }

    fn eligible_jurors(&self) -> Result<Vec<EligibleJuror>, Box<dyn Error>> {
        let mut members = Vec::new();
        for (index, juror) in self.jurors.iter().cloned().enumerate() {
            if !juror.eligible {
                continue;
            }
            if juror.weight == 0 {
                return Err(format!(
                    "policy jury roster juror `{}` has zero weight; weights must be >= 1",
                    juror.juror_id
                )
                .into());
            }
            members.push(EligibleJuror {
                member: juror,
                original_index: index,
            });
        }
        Ok(members)
    }
}

#[derive(Clone, Deserialize)]
struct JuryCandidate {
    juror_id: String,
    pop_identity: String,
    #[serde(default = "default_weight")]
    weight: u64,
    #[serde(default = "default_true")]
    eligible: bool,
    #[serde(default)]
    grace_period_secs: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    #[serde(rename = "metadata")]
    metadata: Option<JsonValue>,
}

#[derive(Clone)]
struct EligibleJuror {
    member: JuryCandidate,
    #[allow(dead_code)]
    original_index: usize,
}

struct DrawRef {
    index: usize,
}

const fn default_weight() -> u64 {
    1
}

const fn default_true() -> bool {
    true
}

pub fn parse_vote_choice(value: &str) -> Result<PolicyJuryVoteChoice, Box<dyn Error>> {
    match value.trim().to_lowercase().as_str() {
        "approve" => Ok(PolicyJuryVoteChoice::Approve),
        "reject" => Ok(PolicyJuryVoteChoice::Reject),
        "abstain" => Ok(PolicyJuryVoteChoice::Abstain),
        other => Err(format!(
            "unknown policy jury vote choice `{other}` (expected approve|reject|abstain)"
        )
        .into()),
    }
}

pub fn parse_ballot_timestamp(label: &str, value: &str) -> Result<OffsetDateTime, Box<dyn Error>> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|err| format!("failed to parse {label} timestamp `{value}`: {err}").into())
}

pub fn parse_nonce_hex_or_random(value: Option<&str>) -> Result<Vec<u8>, Box<dyn Error>> {
    if let Some(raw) = value {
        let trimmed = raw.trim();
        if trimmed.len() % 2 != 0 {
            return Err(format!(
                "policy jury nonce must contain an even number of hex characters, received `{trimmed}`"
            )
            .into());
        }
        let bytes =
            hex_decode(trimmed).map_err(|err| format!("failed to decode nonce hex: {err}"))?;
        ensure_nonce_len(&bytes)?;
        return Ok(bytes);
    }
    let mut nonce = vec![0u8; 32];
    let mut rng = SecureOsRng;
    SecureRngCore::fill_bytes(&mut rng, &mut nonce);
    Ok(nonce)
}

fn ensure_nonce_len(bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if bytes.len() < 16 {
        return Err(format!(
            "policy jury nonce must contain at least 16 bytes, got {}",
            bytes.len()
        )
        .into());
    }
    Ok(())
}

pub fn parse_drawn_at(value: &str) -> Result<OffsetDateTime, Box<dyn Error>> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|err| format!("failed to parse --drawn-at timestamp `{value}`: {err}").into())
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn parse_beacon_validates_length() {
        assert!(parse_beacon(&"ff".repeat(31)).is_err());
        let beacon = parse_beacon(&"ab".repeat(32)).expect("valid beacon");
        assert_eq!(beacon[0], 0xAB);
    }

    #[test]
    fn run_sortition_generates_manifest() {
        let temp = NamedTempFile::new().expect("temp file");
        fs::write(
            temp.path(),
            r#"{
                "format_version": 1,
                "jurors": [
                    {"juror_id": "citizen:ada", "pop_identity": "pop:ada"},
                    {"juror_id": "citizen:ben", "pop_identity": "pop:ben"},
                    {"juror_id": "citizen:chai", "pop_identity": "pop:chai"},
                    {"juror_id": "citizen:dina", "pop_identity": "pop:dina"}
                ]
            }"#,
        )
        .expect("write roster");
        let options = SortitionOptions {
            roster_path: temp.path().to_path_buf(),
            proposal_id: "AC-2026-042".into(),
            round_id: "PJ-2026-02".into(),
            beacon_hex: "0102030405060708090a0b0c0d0e0f100102030405060708090a0b0c0d0e0f10".into(),
            committee_size: 2,
            waitlist_size: 1,
            drawn_at: OffsetDateTime::UNIX_EPOCH,
            waitlist_ttl_hours: 24,
            grace_period_secs: 900,
            failover_grace_secs: 600,
            output_path: None,
        };
        let manifest = run_sortition(&options).expect("manifest");
        assert_eq!(manifest.assignments.len(), 2);
        assert_eq!(manifest.waitlist.len(), 1);
        assert_eq!(manifest.proposal_id, "AC-2026-042");
        assert_eq!(manifest.round_id, "PJ-2026-02");
        manifest.validate().expect("manifest valid");
    }

    #[test]
    fn parse_vote_choice_supports_variants() {
        assert!(matches!(
            parse_vote_choice("approve").expect("choice"),
            PolicyJuryVoteChoice::Approve
        ));
        assert!(matches!(
            parse_vote_choice("Reject").expect("choice"),
            PolicyJuryVoteChoice::Reject
        ));
        assert!(matches!(
            parse_vote_choice("ABSTAIN").expect("choice"),
            PolicyJuryVoteChoice::Abstain
        ));
        assert!(parse_vote_choice("maybe").is_err());
    }

    #[test]
    fn build_ballot_pair_matches_commitment() {
        let options = BallotCommitOptions {
            proposal_id: "AC-2026-001".into(),
            round_id: "PJ-2026-02".into(),
            juror_id: "citizen:ada".into(),
            choice: PolicyJuryVoteChoice::Approve,
            nonce: vec![0u8; 32],
            committed_at: OffsetDateTime::UNIX_EPOCH,
            revealed_at: None,
            output_path: None,
            reveal_output_path: None,
        };
        let (commit, reveal) = build_ballot_pair(&options).expect("ballot pair");
        assert_eq!(commit.commitment_blake2b_256, reveal.compute_commitment());
        commit.verify_reveal(&reveal).expect("ballot verifies");
    }
}
