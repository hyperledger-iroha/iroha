use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use blake3::Hasher as Blake3Hasher;
use hex::FromHex;
use iroha_data_model::ministry::{AgendaProposalAction, AgendaProposalV1};
use norito::json;
use rand::{
    SeedableRng,
    distr::{Distribution, weighted::WeightedIndex},
};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value as JsonValue};
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use walkdir::WalkDir;

pub enum Command {
    Validate(ValidateOptions),
    Sortition(SortitionOptions),
    Impact(ImpactOptions),
}

#[derive(Clone)]
pub struct ValidateOptions {
    pub proposal_path: PathBuf,
    pub registry_path: Option<PathBuf>,
    pub allow_registry_conflicts: bool,
}

#[derive(Clone)]
pub struct SortitionOptions {
    pub roster_path: PathBuf,
    pub slots: usize,
    pub seed_hex: String,
    pub output_path: Option<PathBuf>,
}

#[derive(Clone, Default)]
pub struct ImpactOptions {
    pub proposal_paths: Vec<PathBuf>,
    pub proposal_dirs: Vec<PathBuf>,
    pub registry_path: Option<PathBuf>,
    pub policy_snapshot_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
}

impl Command {
    pub fn impact(options: ImpactOptions) -> Self {
        Self::Impact(options)
    }
}

pub fn run(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Validate(options) => validate_proposal(options),
        Command::Sortition(options) => {
            let summary = run_sortition(&options)?;
            let json = serde_json::to_string_pretty(&summary)
                .map_err(|err| format!("failed to serialize sortition summary: {err}"))?;
            if let Some(path) = options.output_path {
                fs::write(&path, json.as_bytes()).map_err(|err| {
                    format!(
                        "failed to write sortition summary to {}: {err}",
                        path.display()
                    )
                })?;
                println!(
                    "wrote Agenda Council sortition summary to {}",
                    path.display()
                );
            } else {
                println!("{json}");
            }
            Ok(())
        }
        Command::Impact(options) => run_impact(options),
    }
}

fn run_sortition(options: &SortitionOptions) -> Result<SortitionSummary, Box<dyn Error>> {
    const ALGORITHM: &str = "agenda-sortition-blake3-v1";
    let roster_bytes = fs::read(&options.roster_path).map_err(|err| {
        format!(
            "failed to read roster from `{}`: {err}",
            options.roster_path.display()
        )
    })?;
    let roster: SortitionRoster = serde_json::from_slice(&roster_bytes).map_err(|err| {
        format!(
            "failed to parse roster JSON from `{}`: {err}",
            options.roster_path.display()
        )
    })?;
    if roster.format_version != 1 {
        return Err(format!(
            "unsupported roster format_version {}; expected 1",
            roster.format_version
        )
        .into());
    }

    let seed = parse_seed(&options.seed_hex)?;
    if options.slots == 0 {
        return Err("sortition requires --slots >= 1".into());
    }

    let eligible_members = build_eligible_members(&roster)?;
    if eligible_members.is_empty() {
        return Err("roster contains no eligible members".into());
    }
    if eligible_members.len() < options.slots {
        return Err(format!(
            "requested {} slot(s) but only {} eligible members found",
            options.slots,
            eligible_members.len()
        )
        .into());
    }

    let seed_hex = options.seed_hex.trim().to_ascii_lowercase();
    let roster_digest = digest_roster(&roster_bytes);
    let leaves = hash_leaves(&eligible_members)?;
    let layers = build_merkle_layers(&leaves);
    let merkle_root = layers
        .last()
        .and_then(|layer| layer.first())
        .copied()
        .ok_or("failed to derive Merkle root for roster")?;

    let draws = perform_draws(&eligible_members, options.slots, seed)?;
    let mut selected = Vec::with_capacity(draws.len());
    for draw in draws {
        let member = &eligible_members[draw.member_index];
        let leaf_hash = leaves
            .get(draw.member_index)
            .copied()
            .ok_or("missing leaf hash for selected member")?;
        let proof = build_merkle_proof(&layers, draw.member_index);
        selected.push(SelectedMemberSummary {
            member_id: member.member.member_id.clone(),
            role: member.member.role.clone(),
            organization: member.member.organization.clone(),
            contact: member.member.contact.clone(),
            weight: member.member.weight,
            eligible_index: draw.member_index,
            original_index: member.original_index,
            draw_position: draw.draw_position,
            draw_entropy_hex: hex::encode(draw.entropy),
            leaf_hash_hex: hex::encode(leaf_hash),
            merkle_proof: proof.into_iter().map(hex::encode).collect(),
        });
    }

    Ok(SortitionSummary {
        format_version: 1,
        algorithm: ALGORITHM.to_string(),
        roster_path: options.roster_path.display().to_string(),
        roster_digest,
        seed_hex,
        slots: options.slots,
        eligible_members: eligible_members.len(),
        merkle_root_hex: hex::encode(merkle_root),
        selected,
    })
}

fn run_impact(options: ImpactOptions) -> Result<(), Box<dyn Error>> {
    let proposals = load_impact_proposals(&options)?;
    let registry = match &options.registry_path {
        Some(path) => Some(DuplicateRegistry::load(path)?),
        None => None,
    };
    let policy = match &options.policy_snapshot_path {
        Some(path) => Some(PolicySnapshot::load(path)?),
        None => None,
    };
    let report = build_impact_report(&proposals, registry.as_ref(), policy.as_ref());
    let json = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("failed to serialize impact report: {err}"))?;
    if let Some(path) = &options.output_path {
        fs::write(path, json.as_bytes())
            .map_err(|err| format!("failed to write impact report to {}: {err}", path.display()))?;
        println!(
            "wrote Agenda Council impact report for {} proposal(s) to {}",
            report.totals.proposals_analyzed,
            path.display()
        );
    } else {
        println!("{json}");
    }
    Ok(())
}

fn load_impact_proposals(options: &ImpactOptions) -> Result<Vec<LoadedProposal>, Box<dyn Error>> {
    let paths = collect_proposal_paths(options)?;
    let mut proposals = Vec::with_capacity(paths.len());
    for path in paths {
        let proposal = load_proposal(&path)?;
        proposals.push(LoadedProposal {
            proposal,
            source_path: path.display().to_string(),
        });
    }
    Ok(proposals)
}

fn collect_proposal_paths(options: &ImpactOptions) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut paths: BTreeSet<PathBuf> = BTreeSet::new();
    for path in &options.proposal_paths {
        if !path.is_file() {
            return Err(format!("proposal `{}` is not a file", path.display()).into());
        }
        paths.insert(path.clone());
    }
    for dir in &options.proposal_dirs {
        if !dir.is_dir() {
            return Err(
                format!("proposal directory `{}` is not a directory", dir.display()).into(),
            );
        }
        for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            if entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            {
                paths.insert(entry.path().to_path_buf());
            }
        }
    }
    if paths.is_empty() {
        return Err("ministry-agenda impact requires at least one --proposal <path> or --proposal-dir <dir>".into());
    }
    Ok(paths.into_iter().collect())
}

fn build_impact_report(
    proposals: &[LoadedProposal],
    registry: Option<&DuplicateRegistry>,
    policy: Option<&PolicySnapshot>,
) -> ImpactReport {
    let registry_index = registry
        .map(DuplicateRegistry::build_index)
        .unwrap_or_default();
    let policy_index = policy.map(PolicySnapshot::build_index).unwrap_or_default();

    let mut summaries = Vec::with_capacity(proposals.len());
    let mut totals = ImpactTotals::default();
    let mut total_family_counters: BTreeMap<String, HashFamilyCounters> = BTreeMap::new();

    for loaded in proposals {
        totals.proposals_analyzed += 1;
        let mut proposal_family_counters: BTreeMap<String, HashFamilyCounters> = BTreeMap::new();
        let mut conflicts = Vec::new();
        let mut registry_conflicts = 0usize;
        let mut policy_conflicts = 0usize;

        for target in &loaded.proposal.targets {
            totals.targets_analyzed += 1;
            let family_key = target.hash_family.to_lowercase();
            let fingerprint = target.fingerprint_key();
            let registry_hits = registry_index.get(&fingerprint);
            let policy_hits = policy_index.get(&fingerprint);

            let proposal_entry = proposal_family_counters
                .entry(family_key.clone())
                .or_default();
            proposal_entry.targets += 1;

            let total_entry = total_family_counters.entry(family_key).or_default();
            total_entry.targets += 1;

            if let Some(entries) = registry_hits {
                let hit_count = entries.len();
                if hit_count > 0 {
                    proposal_entry.registry_conflicts += hit_count;
                    total_entry.registry_conflicts += hit_count;
                    registry_conflicts += hit_count;
                    totals.registry_conflicts += hit_count;
                    for item in entries {
                        conflicts.push(ConflictDetail {
                            source: ConflictSource::DuplicateRegistry,
                            hash_family: target.hash_family.clone(),
                            hash_hex: target.hash_hex.clone(),
                            reference: item.proposal_id.clone(),
                            note: item.note.clone(),
                        });
                    }
                }
            }

            if let Some(entries) = policy_hits {
                let hit_count = entries.len();
                if hit_count > 0 {
                    proposal_entry.policy_conflicts += hit_count;
                    total_entry.policy_conflicts += hit_count;
                    policy_conflicts += hit_count;
                    totals.policy_conflicts += hit_count;
                    for item in entries {
                        conflicts.push(ConflictDetail {
                            source: ConflictSource::PolicySnapshot,
                            hash_family: target.hash_family.clone(),
                            hash_hex: target.hash_hex.clone(),
                            reference: item.policy_id.clone(),
                            note: item.note.clone(),
                        });
                    }
                }
            }
        }

        let hash_families = proposal_family_counters
            .into_iter()
            .map(|(hash_family, counters)| HashFamilyImpact {
                hash_family,
                targets: counters.targets,
                registry_conflicts: counters.registry_conflicts,
                policy_conflicts: counters.policy_conflicts,
            })
            .collect();

        summaries.push(ProposalImpactSummary {
            proposal_id: loaded.proposal.proposal_id.clone(),
            action: format_action(loaded.proposal.action).to_string(),
            total_targets: loaded.proposal.targets.len(),
            source_path: loaded.source_path.clone(),
            hash_families,
            conflicts,
            registry_conflicts,
            policy_conflicts,
        });
    }

    let aggregated_hash_families = total_family_counters
        .into_iter()
        .map(|(hash_family, counters)| HashFamilyImpact {
            hash_family,
            targets: counters.targets,
            registry_conflicts: counters.registry_conflicts,
            policy_conflicts: counters.policy_conflicts,
        })
        .collect();

    totals.hash_families = aggregated_hash_families;

    ImpactReport {
        format_version: 1,
        generated_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .expect("Rfc3339 formatting is infallible"),
        proposals: summaries,
        totals,
    }
}

fn format_action(action: AgendaProposalAction) -> &'static str {
    match action {
        AgendaProposalAction::AddToDenylist => "add-to-denylist",
        AgendaProposalAction::RemoveFromDenylist => "remove-from-denylist",
        AgendaProposalAction::AmendPolicy => "amend-policy",
    }
}

struct LoadedProposal {
    proposal: AgendaProposalV1,
    source_path: String,
}

#[derive(Default)]
struct HashFamilyCounters {
    targets: usize,
    registry_conflicts: usize,
    policy_conflicts: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ImpactReport {
    pub(crate) format_version: u32,
    pub(crate) generated_at: String,
    pub(crate) proposals: Vec<ProposalImpactSummary>,
    pub(crate) totals: ImpactTotals,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub(crate) struct ImpactTotals {
    pub(crate) proposals_analyzed: usize,
    pub(crate) targets_analyzed: usize,
    pub(crate) registry_conflicts: usize,
    pub(crate) policy_conflicts: usize,
    #[serde(default)]
    pub(crate) hash_families: Vec<HashFamilyImpact>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ProposalImpactSummary {
    pub(crate) proposal_id: String,
    pub(crate) action: String,
    pub(crate) total_targets: usize,
    pub(crate) source_path: String,
    pub(crate) hash_families: Vec<HashFamilyImpact>,
    pub(crate) conflicts: Vec<ConflictDetail>,
    pub(crate) registry_conflicts: usize,
    pub(crate) policy_conflicts: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct HashFamilyImpact {
    pub(crate) hash_family: String,
    pub(crate) targets: usize,
    pub(crate) registry_conflicts: usize,
    pub(crate) policy_conflicts: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ConflictDetail {
    pub(crate) source: ConflictSource,
    pub(crate) hash_family: String,
    pub(crate) hash_hex: String,
    pub(crate) reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) note: Option<String>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConflictSource {
    DuplicateRegistry,
    PolicySnapshot,
}

fn validate_proposal(options: ValidateOptions) -> Result<(), Box<dyn Error>> {
    let proposal = load_proposal(&options.proposal_path)?;
    proposal
        .validate()
        .map_err(|err| format!("agenda proposal validation failed: {err}"))?;

    if let Some(path) = options.registry_path.as_ref() {
        let registry = DuplicateRegistry::load(path)?;
        registry.check_conflicts(&proposal, options.allow_registry_conflicts)?;
    }

    println!(
        "agenda proposal `{}` validated ({} target(s), {} evidence attachment(s))",
        proposal.proposal_id,
        proposal.targets.len(),
        proposal.evidence.len()
    );
    Ok(())
}

fn load_proposal(path: &Path) -> Result<AgendaProposalV1, Box<dyn Error>> {
    let bytes = fs::read(path).map_err(|err| {
        format!(
            "failed to read agenda proposal from {}: {err}",
            path.display()
        )
    })?;
    let proposal: AgendaProposalV1 = json::from_slice(&bytes).map_err(|err| {
        format!(
            "failed to decode AgendaProposalV1 from {}: {err}",
            path.display()
        )
    })?;
    Ok(proposal)
}

#[derive(Deserialize)]
struct SortitionRoster {
    format_version: u32,
    members: Vec<SortitionMember>,
}

#[derive(Clone, Deserialize, Serialize)]
struct SortitionMember {
    member_id: String,
    #[serde(default = "default_weight")]
    weight: u64,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    organization: Option<String>,
    #[serde(default)]
    contact: Option<String>,
    #[serde(default = "default_true")]
    eligible: bool,
    #[serde(flatten)]
    extra: BTreeMap<String, JsonValue>,
}

#[derive(Clone)]
struct EligibleMember {
    member: SortitionMember,
    original_index: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SortitionSummary {
    pub(crate) format_version: u32,
    pub(crate) algorithm: String,
    pub(crate) roster_path: String,
    pub(crate) roster_digest: SortitionDigestSummary,
    pub(crate) seed_hex: String,
    pub(crate) slots: usize,
    pub(crate) eligible_members: usize,
    pub(crate) merkle_root_hex: String,
    pub(crate) selected: Vec<SelectedMemberSummary>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SortitionDigestSummary {
    pub(crate) blake3_hex: String,
    pub(crate) sha256_hex: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SelectedMemberSummary {
    pub(crate) member_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) organization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) contact: Option<String>,
    pub(crate) weight: u64,
    pub(crate) eligible_index: usize,
    pub(crate) original_index: usize,
    pub(crate) draw_position: usize,
    pub(crate) draw_entropy_hex: String,
    pub(crate) leaf_hash_hex: String,
    pub(crate) merkle_proof: Vec<String>,
}

struct DrawResult {
    member_index: usize,
    draw_position: usize,
    entropy: [u8; 32],
}

#[derive(Deserialize)]
struct DuplicateRegistry {
    #[serde(default)]
    entries: Vec<DuplicateRegistryEntry>,
}

impl DuplicateRegistry {
    fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let raw = fs::read_to_string(path)?;
        let registry: Self = serde_json::from_str(&raw).map_err(|err| {
            format!(
                "failed to parse duplicate registry JSON from {}: {err}",
                path.display()
            )
        })?;
        Ok(registry)
    }

    fn check_conflicts(
        &self,
        proposal: &AgendaProposalV1,
        allow_conflicts: bool,
    ) -> Result<(), Box<dyn Error>> {
        if self.entries.is_empty() {
            return Ok(());
        }
        let fingerprints: BTreeSet<String> = proposal
            .targets
            .iter()
            .map(|target| target.fingerprint_key())
            .collect();
        let mut conflicts = Vec::new();
        for entry in &self.entries {
            let fingerprint = entry.fingerprint();
            if fingerprints.contains(&fingerprint) {
                conflicts.push((&entry.proposal_id, fingerprint, &entry.note));
            }
        }
        if conflicts.is_empty() {
            return Ok(());
        }
        if allow_conflicts {
            for (proposal_id, fingerprint, note) in conflicts {
                if let Some(note) = note {
                    eprintln!(
                        "warning: target fingerprint `{fingerprint}` already present in proposal `{proposal_id}` ({note})"
                    );
                } else {
                    eprintln!(
                        "warning: target fingerprint `{fingerprint}` already present in proposal `{proposal_id}`"
                    );
                }
            }
            return Ok(());
        }
        let mut message = String::from("proposal conflicts with existing registry entries:\n");
        for (proposal_id, fingerprint, note) in conflicts {
            if let Some(note) = note {
                message.push_str(&format!(
                    "- fingerprint `{fingerprint}` already tracked by `{proposal_id}` ({note})\n"
                ));
            } else {
                message.push_str(&format!(
                    "- fingerprint `{fingerprint}` already tracked by `{proposal_id}`\n"
                ));
            }
        }
        Err(message.into())
    }

    fn build_index(&self) -> BTreeMap<String, Vec<RegistryConflictRef>> {
        let mut map: BTreeMap<String, Vec<RegistryConflictRef>> = BTreeMap::new();
        for entry in &self.entries {
            map.entry(entry.fingerprint())
                .or_default()
                .push(RegistryConflictRef {
                    proposal_id: entry.proposal_id.clone(),
                    note: entry.note.clone(),
                });
        }
        map
    }
}

fn parse_seed(input: &str) -> Result<[u8; 32], Box<dyn Error>> {
    let trimmed = input.trim();
    if trimmed.len() != 64 {
        return Err(format!(
            "sortition seed must be 64 hex characters (32 bytes), got length {}",
            trimmed.len()
        )
        .into());
    }
    let bytes = <[u8; 32]>::from_hex(trimmed)
        .map_err(|err| format!("failed to decode sortition seed hex: {err}"))?;
    Ok(bytes)
}

fn build_eligible_members(roster: &SortitionRoster) -> Result<Vec<EligibleMember>, Box<dyn Error>> {
    let mut eligible = Vec::new();
    for (index, member) in roster.members.iter().cloned().enumerate() {
        if !member.eligible {
            continue;
        }
        if member.weight == 0 {
            return Err(format!(
                "roster member `{}` has zero weight; weights must be >= 1",
                member.member_id
            )
            .into());
        }
        eligible.push(EligibleMember {
            member,
            original_index: index,
        });
    }
    Ok(eligible)
}

fn digest_roster(bytes: &[u8]) -> SortitionDigestSummary {
    let mut sha256 = Sha256::new();
    sha256.update(bytes);
    SortitionDigestSummary {
        blake3_hex: hex::encode(blake3::hash(bytes).as_bytes()),
        sha256_hex: hex::encode(sha256.finalize()),
    }
}

fn hash_leaves(members: &[EligibleMember]) -> Result<Vec<[u8; 32]>, Box<dyn Error>> {
    members
        .iter()
        .map(|entry| {
            let json = serde_json::to_vec(&entry.member).map_err(|err| {
                format!(
                    "failed to serialize roster member `{}`: {err}",
                    entry.member.member_id
                )
            })?;
            Ok(hash_leaf(&json))
        })
        .collect()
}

fn perform_draws(
    members: &[EligibleMember],
    slots: usize,
    seed: [u8; 32],
) -> Result<Vec<DrawResult>, Box<dyn Error>> {
    let mut rng = ChaCha20Rng::from_seed(seed);
    let mut remaining_indices: Vec<usize> = (0..members.len()).collect();
    let mut draws = Vec::with_capacity(slots);
    for draw_position in 0..slots {
        let weights: Vec<u64> = remaining_indices
            .iter()
            .map(|idx| members[*idx].member.weight)
            .collect();
        let dist = WeightedIndex::new(&weights)
            .map_err(|err| format!("invalid roster weights for sortition: {err}"))?;
        let pick = dist.sample(&mut rng);
        let roster_index = remaining_indices.swap_remove(pick);
        let entropy = draw_entropy(seed, draw_position, &members[roster_index].member.member_id);
        draws.push(DrawResult {
            member_index: roster_index,
            draw_position,
            entropy,
        });
    }
    Ok(draws)
}

fn hash_leaf(payload: &[u8]) -> [u8; 32] {
    let mut hasher = Blake3Hasher::new();
    hasher.update(b"agenda-sortition-leaf-v1");
    hasher.update(payload);
    *hasher.finalize().as_bytes()
}

fn hash_node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Blake3Hasher::new();
    hasher.update(b"agenda-sortition-node-v1");
    hasher.update(left);
    hasher.update(right);
    *hasher.finalize().as_bytes()
}

fn build_merkle_layers(leaves: &[[u8; 32]]) -> Vec<Vec<[u8; 32]>> {
    let mut layers = Vec::new();
    if leaves.is_empty() {
        return layers;
    }
    layers.push(leaves.to_vec());
    while layers.last().map_or(0, Vec::len) > 1 {
        let previous = layers.last().expect("layer exists");
        let mut next = Vec::with_capacity(previous.len().div_ceil(2));
        for chunk in previous.chunks(2) {
            let node = if chunk.len() == 2 {
                hash_node(&chunk[0], &chunk[1])
            } else {
                hash_node(&chunk[0], &chunk[0])
            };
            next.push(node);
        }
        layers.push(next);
    }
    layers
}

fn build_merkle_proof(layers: &[Vec<[u8; 32]>], mut index: usize) -> Vec<[u8; 32]> {
    let mut proof = Vec::new();
    if layers.is_empty() {
        return proof;
    }
    for layer in layers.iter() {
        if layer.len() == 1 {
            break;
        }
        let is_right = index % 2 == 1;
        let sibling_index = if is_right { index - 1 } else { index + 1 };
        let sibling = if sibling_index < layer.len() {
            layer[sibling_index]
        } else {
            layer[index]
        };
        proof.push(sibling);
        index /= 2;
    }
    proof
}

fn draw_entropy(seed: [u8; 32], draw_position: usize, member_id: &str) -> [u8; 32] {
    let mut hasher = Blake3Hasher::new();
    hasher.update(b"agenda-sortition-draw-v1");
    hasher.update(&seed);
    hasher.update(&(draw_position as u64).to_be_bytes());
    hasher.update(member_id.as_bytes());
    *hasher.finalize().as_bytes()
}

const fn default_weight() -> u64 {
    1
}

const fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct DuplicateRegistryEntry {
    hash_family: String,
    hash_hex: String,
    proposal_id: String,
    #[serde(default)]
    note: Option<String>,
}

impl DuplicateRegistryEntry {
    fn fingerprint(&self) -> String {
        format!(
            "{}:{}",
            self.hash_family.to_lowercase(),
            self.hash_hex.to_lowercase()
        )
    }
}

struct RegistryConflictRef {
    proposal_id: String,
    note: Option<String>,
}

#[derive(Deserialize)]
struct PolicySnapshot {
    #[serde(default)]
    entries: Vec<PolicySnapshotEntry>,
}

impl PolicySnapshot {
    fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let raw = fs::read_to_string(path).map_err(|err| {
            format!(
                "failed to read policy snapshot from {}: {err}",
                path.display()
            )
        })?;
        let snapshot: Self = serde_json::from_str(&raw).map_err(|err| {
            format!(
                "failed to parse policy snapshot JSON from {}: {err}",
                path.display()
            )
        })?;
        Ok(snapshot)
    }

    fn build_index(&self) -> BTreeMap<String, Vec<PolicyConflictRef>> {
        let mut map: BTreeMap<String, Vec<PolicyConflictRef>> = BTreeMap::new();
        for entry in &self.entries {
            map.entry(entry.fingerprint())
                .or_default()
                .push(PolicyConflictRef {
                    policy_id: entry.policy_id.clone(),
                    note: entry.note.clone(),
                });
        }
        map
    }
}

#[derive(Deserialize)]
struct PolicySnapshotEntry {
    hash_family: String,
    hash_hex: String,
    policy_id: String,
    #[serde(default)]
    note: Option<String>,
}

impl PolicySnapshotEntry {
    fn fingerprint(&self) -> String {
        format!(
            "{}:{}",
            self.hash_family.to_lowercase(),
            self.hash_hex.to_lowercase()
        )
    }
}

struct PolicyConflictRef {
    policy_id: String,
    note: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use hex::FromHex;
    use iroha_data_model::ministry::{
        AGENDA_PROPOSAL_VERSION_V1, AgendaEvidenceAttachment, AgendaEvidenceKind,
        AgendaProposalSubmitter, AgendaProposalSummary, AgendaProposalTarget,
    };
    use tempfile::NamedTempFile;

    use super::*;

    const TEST_ROSTER: &str = r#"{
        "format_version": 1,
        "members": [
            {"member_id": "citizen:ada", "weight": 2, "role": "citizen"},
            {"member_id": "citizen:bob", "weight": 1, "role": "citizen"},
            {"member_id": "citizen:cath", "weight": 1, "role": "observer"},
            {"member_id": "citizen:dora", "weight": 3, "role": "citizen"},
            {"member_id": "citizen:erin", "weight": 1, "role": "citizen", "eligible": false}
        ]
    }"#;

    const TEST_SEED: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SAMPLE_PROPOSAL: &str = r#"{
  "version": 1,
  "proposal_id": "AC-2026-001",
  "submitted_at_unix_ms": 1770000000000,
  "language": "en",
  "action": "add-to-denylist",
  "summary": {
    "title": "Add newly discovered CSAM hash cluster",
    "motivation": "Citizen reviewers and moderators flagged recurring uploads referencing the same digest family across two operators.",
    "expected_impact": "Quarantining this family prevents known abusive media from resurfacing while appeals are processed."
  },
  "tags": [
    "csam",
    "policy-escalation"
  ],
  "targets": [
    {
      "label": "Incident sample A",
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "reason": "Matched quarantine queue entry QA-2026-041 for two separate providers."
    },
    {
      "label": "Incident sample B",
      "hash_family": "blake3-256",
      "hash_hex": "18f47f8da6ccbf81a902f13b64527bd2f7cb8d7797a8ff72dbe1f16b4fcb9a2e",
      "reason": "Derived from the same capture session with identical metadata."
    }
  ],
  "evidence": [
    {
      "kind": "sorafs-cid",
      "uri": "sorafs://bafybeiaxqzryhkzwhd5vnc5e2uqeglplkwdkpx3wt2bnjbetb6levgbewi/manifest.car",
      "digest_blake3_hex": "6f90754d74fe89ae79f2c0017655bc908eb7f98f249c3ba1dc5c0bd1d8861e58",
      "description": "Encrypted CAR bundle quarantined by the GAR."
    },
    {
      "kind": "url",
      "uri": "https://incidents.sora.example/cases/CSAM-2026-041",
      "description": "Incident timeline + moderator review log."
    },
    {
      "kind": "torii-case",
      "uri": "CASE-2026-04-118",
      "description": "Torii governance ticket cross-referencing the same hashes."
    }
  ],
  "submitter": {
    "name": "Citizen Reviewer 42",
    "contact": "citizen42@example.org",
    "organization": "Wonderland Watch",
    "pgp_fingerprint": "A2B3C4D5E6F70123456789ABCDEF0123456789AB"
  },
  "duplicates": [
    "AC-2025-014"
  ]
}"#;

    #[test]
    fn load_proposal_parses_sample_payload() {
        let mut file = NamedTempFile::new().expect("temp file");
        file.write_all(SAMPLE_PROPOSAL.as_bytes())
            .expect("write proposal");
        let proposal = load_proposal(file.path()).expect("load proposal");
        assert_eq!(proposal.proposal_id, "AC-2026-001");
        assert_eq!(
            proposal.action,
            iroha_data_model::ministry::AgendaProposalAction::AddToDenylist
        );
        assert_eq!(proposal.evidence.len(), 3);
    }

    #[test]
    fn sortition_is_deterministic() {
        let roster_file = write_roster(TEST_ROSTER);
        let options = SortitionOptions {
            roster_path: roster_file.path().to_path_buf(),
            slots: 2,
            seed_hex: TEST_SEED.to_string(),
            output_path: None,
        };
        let summary = run_sortition(&options).expect("sortition");
        assert_eq!(summary.selected.len(), 2);

        let repeat = run_sortition(&options).expect("sortition repeat");
        let initial: Vec<_> = summary
            .selected
            .iter()
            .map(|entry| entry.member_id.clone())
            .collect();
        let rerun: Vec<_> = repeat
            .selected
            .iter()
            .map(|entry| entry.member_id.clone())
            .collect();
        assert_eq!(initial, rerun);
    }

    #[test]
    fn merkle_proofs_round_trip_to_root() {
        let roster_file = write_roster(TEST_ROSTER);
        let options = SortitionOptions {
            roster_path: roster_file.path().to_path_buf(),
            slots: 3,
            seed_hex: TEST_SEED.to_string(),
            output_path: None,
        };
        let summary = run_sortition(&options).expect("sortition");

        let roster: SortitionRoster =
            serde_json::from_str(TEST_ROSTER).expect("parse roster fixture");
        let eligible = build_eligible_members(&roster).expect("eligible");
        let leaves = hash_leaves(&eligible).expect("hash leaves");
        let layers = build_merkle_layers(&leaves);
        let root = layers.last().unwrap()[0];
        assert_eq!(hex::encode(root), summary.merkle_root_hex);

        for selected in summary.selected {
            let mut index = selected.eligible_index;
            let mut computed = leaves[index];
            for proof_hex in &selected.merkle_proof {
                let sibling_bytes = <[u8; 32]>::from_hex(proof_hex).expect("decode proof hex");
                if index % 2 == 0 {
                    computed = hash_node(&computed, &sibling_bytes);
                } else {
                    computed = hash_node(&sibling_bytes, &computed);
                }
                index /= 2;
            }
            assert_eq!(hex::encode(computed), summary.merkle_root_hex);
        }
    }

    #[test]
    fn impact_report_summarises_conflicts() {
        let proposal_a = sample_loaded_proposal(
            "AC-2026-001",
            vec![("blake3-256", "abcd"), ("blake3-256", "deadbeef")],
            "proposal-a.json",
        );
        let proposal_b =
            sample_loaded_proposal("AC-2026-002", vec![("sha256", "ffff")], "proposal-b.json");
        let registry = DuplicateRegistry {
            entries: vec![DuplicateRegistryEntry {
                hash_family: "blake3-256".into(),
                hash_hex: "abcd".into(),
                proposal_id: "AC-2025-014".into(),
                note: Some("Handled by prior referendum".into()),
            }],
        };
        let policy = PolicySnapshot {
            entries: vec![PolicySnapshotEntry {
                hash_family: "sha256".into(),
                hash_hex: "ffff".into(),
                policy_id: "denylist-2026-03-entry-1".into(),
                note: None,
            }],
        };

        let report = build_impact_report(&[proposal_a, proposal_b], Some(&registry), Some(&policy));

        assert_eq!(report.totals.proposals_analyzed, 2);
        assert_eq!(report.totals.targets_analyzed, 3);
        assert_eq!(report.totals.registry_conflicts, 1);
        assert_eq!(report.totals.policy_conflicts, 1);
        assert_eq!(report.proposals[0].conflicts.len(), 1);
        assert_eq!(report.proposals[1].conflicts.len(), 1);

        let families: Vec<_> = report
            .totals
            .hash_families
            .iter()
            .map(|entry| (entry.hash_family.as_str(), entry.targets))
            .collect();
        assert_eq!(families, vec![("blake3-256", 2), ("sha256", 1)]);
    }

    fn sample_loaded_proposal(
        proposal_id: &str,
        hashes: Vec<(&str, &str)>,
        source: &str,
    ) -> LoadedProposal {
        let targets = hashes
            .into_iter()
            .enumerate()
            .map(|(index, (family, digest))| AgendaProposalTarget {
                label: format!("target-{index}"),
                hash_family: family.to_string(),
                hash_hex: digest.to_string(),
                reason: "sample".into(),
            })
            .collect();

        let proposal = AgendaProposalV1 {
            version: AGENDA_PROPOSAL_VERSION_V1,
            proposal_id: proposal_id.into(),
            submitted_at_unix_ms: 1,
            language: "en".into(),
            action: AgendaProposalAction::AddToDenylist,
            summary: AgendaProposalSummary {
                title: "Example".into(),
                motivation: "Testing impact summaries".into(),
                expected_impact: "Improves moderation accuracy".into(),
            },
            tags: vec!["csam".into()],
            targets,
            evidence: vec![AgendaEvidenceAttachment {
                kind: AgendaEvidenceKind::Url,
                uri: "https://example.test/evidence".into(),
                digest_blake3_hex: None,
                description: Some("Sample evidence".into()),
            }],
            submitter: AgendaProposalSubmitter {
                name: "Tester".into(),
                contact: "tester@example.org".into(),
                organization: None,
                pgp_fingerprint: None,
            },
            duplicates: Vec::new(),
        };

        LoadedProposal {
            proposal,
            source_path: source.into(),
        }
    }

    fn write_roster(contents: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("temp file");
        file.write_all(contents.as_bytes()).expect("write roster");
        file
    }
}
