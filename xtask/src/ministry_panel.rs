use std::{
    collections::BTreeMap,
    convert::TryFrom,
    error::Error,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use eyre::{Context, Result, ensure, eyre};
use hex::encode as hex_encode;
use iroha_data_model::{
    ministry::{
        AgendaEvidenceAttachment, AgendaProposalV1, REFERENDUM_PACKET_VERSION_V1,
        REVIEW_PANEL_SUMMARY_VERSION_V1, ReferendumImpactConflict, ReferendumImpactConflictSource,
        ReferendumImpactHashFamily, ReferendumImpactSummary, ReferendumPacketV1,
        ReferendumPanelist, ReferendumSortitionEvidence, ReviewPanelAiEvidence,
        ReviewPanelCitation, ReviewPanelCitationKind, ReviewPanelHighlight, ReviewPanelOverview,
        ReviewPanelStanceCount, ReviewPanelSummaryV1, ReviewPanelVolunteerReference,
    },
    sorafs::moderation::{MODERATION_REPRO_MANIFEST_VERSION_V1, ModerationReproManifestV1},
};
use norito::json::{JsonDeserialize, Map as JsonMap, Value};
use serde::de::DeserializeOwned;

#[cfg(test)]
use crate::ministry_agenda::{
    ConflictDetail, HashFamilyImpact, ImpactTotals, SelectedMemberSummary, SortitionDigestSummary,
};
use crate::ministry_agenda::{
    ConflictSource, ImpactReport, ProposalImpactSummary, SortitionSummary,
};

type ParseResult<T> = std::result::Result<T, String>;

pub enum Command {
    Synthesize(SynthesizeOptions),
    Packet(PacketOptions),
}

#[derive(Clone)]
pub struct SynthesizeOptions {
    pub proposal_path: PathBuf,
    pub volunteer_path: PathBuf,
    pub ai_manifest_path: PathBuf,
    pub output_path: PathBuf,
    pub panel_round_id: String,
    pub language_override: Option<String>,
    pub generated_at_unix_ms: Option<u64>,
}

#[derive(Clone)]
pub struct PacketOptions {
    pub synth: SynthesizeOptions,
    pub sortition_summary_path: PathBuf,
    pub impact_report_path: PathBuf,
    pub output_path: PathBuf,
    pub summary_out_path: Option<PathBuf>,
}

pub fn run(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Synthesize(options) => synthesize(options).map_err(|err| err.into()),
        Command::Packet(options) => packet(options).map_err(|err| err.into()),
    }
}

pub fn synthesize_summary(options: &SynthesizeOptions) -> Result<ReviewPanelSummaryV1> {
    validate_panel_round(&options.panel_round_id)?;

    let proposal: AgendaProposalV1 = load_json(&options.proposal_path, "proposal")?;
    proposal
        .validate()
        .map_err(|err| eyre!("proposal validation failed: {err}"))?;

    let manifest: ModerationReproManifestV1 = load_json(&options.ai_manifest_path, "AI manifest")?;
    ensure!(
        manifest.body.schema_version == MODERATION_REPRO_MANIFEST_VERSION_V1,
        "AI manifest uses unsupported schema version {}",
        manifest.body.schema_version
    );

    let volunteer_raw: Value = load_json(&options.volunteer_path, "volunteer dataset")?;
    let briefs = parse_volunteer_briefs(volunteer_raw, &proposal.proposal_id)?;
    let filtered: Vec<VolunteerBrief> = briefs
        .into_iter()
        .filter(|brief| !brief.off_topic)
        .collect();
    ensure!(
        !filtered.is_empty(),
        "volunteer dataset contains no on-topic submissions"
    );

    build_review_panel_summary(options, &proposal, &manifest, &filtered)
}

fn synthesize(options: SynthesizeOptions) -> Result<()> {
    let summary = synthesize_summary(&options)?;
    write_summary(&summary, &options.output_path)?;
    Ok(())
}

fn packet(options: PacketOptions) -> Result<()> {
    let packet = build_packet(&options)?;
    if let Some(summary_out) = &options.summary_out_path {
        write_summary(&packet.review_summary, summary_out)?;
    }
    write_packet(&packet, &options.output_path)?;
    println!(
        "wrote referendum packet to {}",
        options.output_path.display()
    );
    Ok(())
}

fn build_packet(options: &PacketOptions) -> Result<ReferendumPacketV1> {
    let summary = synthesize_summary(&options.synth)?;
    let proposal: AgendaProposalV1 =
        load_json(&options.synth.proposal_path, "proposal for packet")?;
    let sortition: SortitionSummary =
        load_serde_json(&options.sortition_summary_path, "sortition summary")?;
    ensure!(
        sortition.format_version == 1,
        "unsupported sortition summary format version {}",
        sortition.format_version
    );
    let impact: ImpactReport = load_serde_json(&options.impact_report_path, "impact report")?;
    let sortition_evidence = build_sortition_evidence(&sortition)?;
    let panelists = build_panelists(&sortition)?;
    let impact_summary = build_referendum_impact_summary(&impact, &proposal.proposal_id)?;
    Ok(ReferendumPacketV1 {
        version: REFERENDUM_PACKET_VERSION_V1,
        proposal,
        review_summary: summary,
        sortition: sortition_evidence,
        panelists,
        impact_summary,
    })
}

fn load_json<T>(path: &Path, label: &str) -> Result<T>
where
    T: JsonDeserialize,
{
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read {label} from {}", path.display()))?;
    norito::json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {label} JSON at {}", path.display()))
}

fn load_serde_json<T>(path: &Path, label: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read {label} from {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {label} JSON at {}", path.display()))
}

fn build_sortition_evidence(summary: &SortitionSummary) -> Result<ReferendumSortitionEvidence> {
    Ok(ReferendumSortitionEvidence {
        algorithm: summary.algorithm.clone(),
        seed_hex: summary.seed_hex.clone(),
        merkle_root_hex: summary.merkle_root_hex.clone(),
        roster_digest_blake3_hex: summary.roster_digest.blake3_hex.clone(),
        roster_digest_sha256_hex: summary.roster_digest.sha256_hex.clone(),
        slots: to_u32(summary.slots, "sortition slots")?,
        eligible_members: to_u32(summary.eligible_members, "eligible members")?,
    })
}

fn build_panelists(summary: &SortitionSummary) -> Result<Vec<ReferendumPanelist>> {
    summary
        .selected
        .iter()
        .map(|member| {
            Ok(ReferendumPanelist {
                member_id: member.member_id.clone(),
                role: member.role.clone(),
                organization: member.organization.clone(),
                contact: member.contact.clone(),
                weight: member.weight,
                draw_position: to_u32(member.draw_position, "draw position")?,
                eligible_index: to_u32(member.eligible_index, "eligible index")?,
                original_index: to_u32(member.original_index, "original index")?,
                draw_entropy_hex: member.draw_entropy_hex.clone(),
                leaf_hash_hex: member.leaf_hash_hex.clone(),
                merkle_proof: member.merkle_proof.clone(),
            })
        })
        .collect()
}

fn build_referendum_impact_summary(
    report: &ImpactReport,
    proposal_id: &str,
) -> Result<ReferendumImpactSummary> {
    let entry: &ProposalImpactSummary = report
        .proposals
        .iter()
        .find(|proposal| proposal.proposal_id == proposal_id)
        .ok_or_else(|| eyre!("impact report missing proposal `{proposal_id}`"))?;

    let hash_families = entry
        .hash_families
        .iter()
        .map(|family| {
            Ok(ReferendumImpactHashFamily {
                hash_family: family.hash_family.clone(),
                targets: to_u32(family.targets, "hash family targets")?,
                registry_conflicts: to_u32(
                    family.registry_conflicts,
                    "hash family registry conflicts",
                )?,
                policy_conflicts: to_u32(family.policy_conflicts, "hash family policy conflicts")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let conflicts = entry
        .conflicts
        .iter()
        .map(|conflict| ReferendumImpactConflict {
            source: convert_conflict_source(conflict.source),
            hash_family: conflict.hash_family.clone(),
            hash_hex: conflict.hash_hex.clone(),
            reference: conflict.reference.clone(),
            note: conflict.note.clone(),
        })
        .collect();

    Ok(ReferendumImpactSummary {
        report_generated_at: report.generated_at.clone(),
        total_targets: to_u32(entry.total_targets, "proposal targets")?,
        registry_conflicts: to_u32(entry.registry_conflicts, "proposal registry conflicts")?,
        policy_conflicts: to_u32(entry.policy_conflicts, "proposal policy conflicts")?,
        hash_families,
        conflicts,
    })
}

fn convert_conflict_source(source: ConflictSource) -> ReferendumImpactConflictSource {
    match source {
        ConflictSource::DuplicateRegistry => ReferendumImpactConflictSource::DuplicateRegistry,
        ConflictSource::PolicySnapshot => ReferendumImpactConflictSource::PolicySnapshot,
    }
}

fn to_u32(value: usize, label: &str) -> Result<u32> {
    u32::try_from(value).map_err(|_| eyre!("`{label}` value {value} exceeds u32 range"))
}

#[cfg(test)]
mod parse_tests {
    use iroha_data_model::{
        ministry::{
            AGENDA_PROPOSAL_VERSION_V1, AgendaEvidenceAttachment, AgendaEvidenceKind,
            AgendaProposalAction, AgendaProposalSubmitter, AgendaProposalSummary,
            AgendaProposalTarget, AgendaProposalV1,
        },
        sorafs::moderation::{
            MODERATION_REPRO_MANIFEST_VERSION_V1, ModerationModelFingerprintV1,
            ModerationReproBodyV1, ModerationReproManifestV1, ModerationSeedMaterialV1,
            ModerationThresholdsV1,
        },
    };
    use norito::json::{JsonSerialize, Value as NoritoValue};
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn packet_command_emits_referendum_packet() {
        let tmp = TempDir::new().expect("tempdir");
        let proposal_path = tmp.path().join("proposal.json");
        write_norito_json(&proposal_path, &sample_proposal());
        let volunteer_path = tmp.path().join("volunteer.json");
        let volunteer_dataset = sample_volunteer_dataset();
        write_norito_json_value(&volunteer_path, &volunteer_dataset);
        let manifest_path = tmp.path().join("manifest.json");
        write_norito_json(&manifest_path, &sample_manifest());
        let sortition_path = tmp.path().join("sortition.json");
        serde_write(&sortition_path, &sample_sortition_summary());
        let impact_path = tmp.path().join("impact.json");
        serde_write(&impact_path, &sample_impact_report());
        let packet_path = tmp.path().join("packet.json");
        let summary_path = tmp.path().join("summary.json");

        let options = PacketOptions {
            synth: SynthesizeOptions {
                proposal_path: proposal_path.clone(),
                volunteer_path: volunteer_path.clone(),
                ai_manifest_path: manifest_path.clone(),
                output_path: PathBuf::from("-"),
                panel_round_id: "RP-2026-05".into(),
                language_override: None,
                generated_at_unix_ms: Some(1_780_000_000_000),
            },
            sortition_summary_path: sortition_path.clone(),
            impact_report_path: impact_path.clone(),
            output_path: packet_path.clone(),
            summary_out_path: Some(summary_path.clone()),
        };

        packet(options).expect("packet workflow runs");

        assert!(summary_path.exists(), "summary output must be written");

        let packet_bytes = fs::read(&packet_path).expect("packet read");
        let packet: ReferendumPacketV1 =
            norito::json::from_slice(&packet_bytes).expect("packet decode");
        assert_eq!(packet.proposal.proposal_id, "AC-2026-001");
        assert_eq!(packet.review_summary.panel_round_id, "RP-2026-05");
        assert_eq!(packet.panelists.len(), 2);
        assert_eq!(packet.impact_summary.hash_families.len(), 1);
    }

    fn write_norito_json<T>(path: &Path, value: &T)
    where
        T: JsonSerialize,
    {
        let bytes =
            norito::json::to_vec_pretty(value).expect("serialize Norito-compatible structure");
        fs::write(path, bytes).expect("write Norito json");
    }

    fn write_norito_json_value(path: &Path, value: &NoritoValue) {
        let bytes = norito::json::to_vec_pretty(value).expect("serialize Norito value");
        fs::write(path, bytes).expect("write value json");
    }

    fn serde_write<T>(path: &Path, value: &T)
    where
        T: serde::Serialize,
    {
        let bytes = serde_json::to_vec_pretty(value).expect("serialize serde value");
        fs::write(path, bytes).expect("write serde json");
    }

    fn sample_proposal() -> AgendaProposalV1 {
        AgendaProposalV1 {
            version: AGENDA_PROPOSAL_VERSION_V1,
            proposal_id: "AC-2026-001".into(),
            submitted_at_unix_ms: 1_780_000_000_000,
            language: "en".into(),
            action: AgendaProposalAction::AddToDenylist,
            summary: AgendaProposalSummary {
                title: "Remove malicious hash".into(),
                motivation: "Repeated citizen reports".into(),
                expected_impact: "Removes abusive payload".into(),
            },
            tags: vec!["csam".into()],
            targets: vec![AgendaProposalTarget {
                label: "evidence-entry".into(),
                hash_family: "blake3-256".into(),
                hash_hex: "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d".into(),
                reason: "Flagged by moderators".into(),
            }],
            evidence: vec![AgendaEvidenceAttachment {
                kind: AgendaEvidenceKind::Url,
                uri: "https://evidence.example.org/ac-2026-001".into(),
                digest_blake3_hex: None,
                description: Some("Incident report".into()),
            }],
            submitter: AgendaProposalSubmitter {
                name: "Citizen 42".into(),
                contact: "citizen42@example.org".into(),
                organization: None,
                pgp_fingerprint: None,
            },
            duplicates: vec![],
        }
    }

    fn sample_manifest() -> ModerationReproManifestV1 {
        ModerationReproManifestV1 {
            body: ModerationReproBodyV1 {
                schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
                manifest_id: [0x11; 16],
                manifest_digest: [0x22; 32],
                runner_hash: [0x33; 32],
                runtime_version: "sorafs-ai-runner 0.5.0".into(),
                issued_at_unix: 1_780_000_000,
                seed_material: ModerationSeedMaterialV1 {
                    domain_tag: "ai-runner".into(),
                    seed_version: 1,
                    run_nonce: [0x44; 32],
                },
                thresholds: ModerationThresholdsV1 {
                    quarantine: 7200,
                    escalate: 3100,
                },
                models: vec![ModerationModelFingerprintV1 {
                    model_id: [0x55; 16],
                    artifact_digest: [0x66; 32],
                    weights_digest: [0x77; 32],
                    opset: 17,
                    weight: Some(5000),
                }],
                notes: Some("Test manifest".into()),
            },
            signatures: Vec::new(),
        }
    }

    fn sample_sortition_summary() -> SortitionSummary {
        SortitionSummary {
            format_version: 1,
            algorithm: "agenda-sortition-blake3-v1".into(),
            roster_path: "docs/examples/ministry/agenda_council_roster.json".into(),
            roster_digest: SortitionDigestSummary {
                blake3_hex: "7c6f6cbfbcb8eec595655ffb5f84355a84772b65d741f6a53fb966ab0c4b5858"
                    .into(),
                sha256_hex: "1ecf1fd54548617b7329627221dfe9af0ef23e704cd9c4235d2ff8ce6f8c2cdf"
                    .into(),
            },
            seed_hex: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            slots: 2,
            eligible_members: 3,
            merkle_root_hex: "c9c5d48ebf297d73195dd73cdca3b02283d47fac5beb8a2f0f4cf7d4cf5a3b77"
                .into(),
            selected: vec![
                SelectedMemberSummary {
                    member_id: "citizen:ada".into(),
                    role: Some("citizen".into()),
                    organization: Some("Artemis Cooperative".into()),
                    contact: Some("ada@example.org".into()),
                    weight: 2,
                    eligible_index: 0,
                    original_index: 0,
                    draw_position: 0,
                    draw_entropy_hex:
                        "a7e8ee16b74a6ed6c5043621f0ee82e9a2bd69882bff572b7fc608bec3e9a9ef".into(),
                    leaf_hash_hex:
                        "4a807d728c14b2a0f1cb42a8e96ad0b7d5e2f7d37973062dbf8f5eeeed2d2aeb".into(),
                    merkle_proof: vec![
                        "ed7941cb505dbb09c059bdec9f2acefe8237b3356f5b10d2b3a5c51c88629875".into(),
                        "d5850df94543f1ef9e2cb84660b1c3d93b5a2f8bd92afc6f7cc0260f2fa7089b".into(),
                    ],
                },
                SelectedMemberSummary {
                    member_id: "citizen:bea".into(),
                    role: Some("observer".into()),
                    organization: Some("Inspectorate".into()),
                    contact: None,
                    weight: 1,
                    eligible_index: 2,
                    original_index: 2,
                    draw_position: 1,
                    draw_entropy_hex:
                        "4c5e7a0f36de9974ee5d8b848b0f916d3c2a39b38a4a48dcdc2805326b0bd708".into(),
                    leaf_hash_hex:
                        "a0ffca1a26d57fd65b746d4b97a2fc836df7c5e353788f62c7568b5c4b0a5d88".into(),
                    merkle_proof: vec![
                        "7eb0ec24bbce9215ea4ed6f0f9059ecb8ecde25d3672f7c002848631186c34b4".into(),
                        "d5850df94543f1ef9e2cb84660b1c3d93b5a2f8bd92afc6f7cc0260f2fa7089b".into(),
                    ],
                },
            ],
        }
    }

    fn sample_impact_report() -> ImpactReport {
        ImpactReport {
            format_version: 1,
            generated_at: "2026-03-31T12:00:00Z".into(),
            proposals: vec![ProposalImpactSummary {
                proposal_id: "AC-2026-001".into(),
                action: "add-to-denylist".into(),
                total_targets: 1,
                source_path: "docs/examples/ministry/agenda_proposal_example.json".into(),
                hash_families: vec![HashFamilyImpact {
                    hash_family: "blake3-256".into(),
                    targets: 1,
                    registry_conflicts: 1,
                    policy_conflicts: 0,
                }],
                conflicts: vec![ConflictDetail {
                    source: ConflictSource::DuplicateRegistry,
                    hash_family: "blake3-256".into(),
                    hash_hex: "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d"
                        .into(),
                    reference: "AC-2025-014".into(),
                    note: Some("Already quarantined".into()),
                }],
                registry_conflicts: 1,
                policy_conflicts: 0,
            }],
            totals: ImpactTotals {
                proposals_analyzed: 1,
                targets_analyzed: 1,
                registry_conflicts: 1,
                policy_conflicts: 0,
                hash_families: vec![HashFamilyImpact {
                    hash_family: "blake3-256".into(),
                    targets: 1,
                    registry_conflicts: 1,
                    policy_conflicts: 0,
                }],
            },
        }
    }

    fn sample_volunteer_dataset() -> NoritoValue {
        NoritoValue::Array(vec![
            volunteer_entry(
                "support-1",
                "support",
                "https://evidence.example.org/support",
            ),
            volunteer_entry("oppose-1", "oppose", "https://evidence.example.org/oppose"),
        ])
    }

    fn volunteer_entry(id: &str, stance: &str, citation: &str) -> NoritoValue {
        let summary_title = format!("Summary {id}");
        let claim_id = format!("{id}-F1");
        let claim_text = format!("Claim from {id}");
        norito::json!({
            "brief_id": id,
            "proposal_id": "AC-2026-001",
            "language": "en",
            "stance": stance,
            "submitted_at": "2026-03-01T12:00:00Z",
            "author": {
                "name": "Citizen Volunteer",
                "organization": "Sora Commons",
                "contact": "volunteer@example.org",
                "no_conflicts_certified": true
            },
            "summary": {
                "title": summary_title,
                "abstract": "Details about the claim",
                "requested_action": "Act now"
            },
            "fact_table": [{
                "claim_id": claim_id,
                "claim": claim_text,
                "status": "corroborated",
                "impact": ["governance"],
                "citations": [citation],
                "evidence_digest": null
            }],
            "disclosures": [],
            "moderation": {
                "off_topic": false,
                "tags": []
            }
        })
    }
}

fn write_summary(summary: &ReviewPanelSummaryV1, path: &Path) -> Result<()> {
    let bytes = norito::json::to_vec_pretty(summary)
        .context("failed to serialise review panel summary to JSON")?;
    if path == Path::new("-") {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(&bytes)
            .context("failed to write review summary to stdout")?;
        stdout.write_all(b"\n")?;
    } else {
        fs::write(path, bytes)
            .with_context(|| format!("failed to write review summary to {}", path.display()))?;
    }
    Ok(())
}

fn write_packet(packet: &ReferendumPacketV1, path: &Path) -> Result<()> {
    let bytes = norito::json::to_vec_pretty(packet)
        .context("failed to serialise referendum packet to JSON")?;
    if path == Path::new("-") {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(&bytes)
            .context("failed to write referendum packet to stdout")?;
        stdout.write_all(b"\n")?;
    } else {
        fs::write(path, bytes)
            .with_context(|| format!("failed to write referendum packet to {}", path.display()))?;
    }
    Ok(())
}

fn parse_volunteer_briefs(value: Value, expected_proposal_id: &str) -> Result<Vec<VolunteerBrief>> {
    let entries = value
        .as_array()
        .ok_or_else(|| eyre!("volunteer dataset must be a JSON array"))?;
    let mut briefs = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let brief = VolunteerBrief::from_value(entry, expected_proposal_id)
            .map_err(|err| eyre!("invalid volunteer brief at index {index}: {err}"))?;
        briefs.push(brief);
    }
    Ok(briefs)
}

fn build_review_panel_summary(
    options: &SynthesizeOptions,
    proposal: &AgendaProposalV1,
    manifest: &ModerationReproManifestV1,
    briefs: &[VolunteerBrief],
) -> Result<ReviewPanelSummaryV1> {
    let mut warnings = Vec::new();
    let stats = aggregate_stance_stats(briefs);
    ensure!(
        stats.contains_key(&Stance::Support) && stats.contains_key(&Stance::Oppose),
        "volunteer dataset must include both support and oppose stances"
    );

    let manifest_hex = hex_encode(manifest.body.manifest_id);
    let highlights = build_highlights(
        briefs,
        &manifest_hex,
        proposal.evidence.first(),
        &mut warnings,
    )?;
    ensure!(
        highlights.iter().any(|h| h.stance == "support"),
        "failed to generate a support highlight with citations"
    );
    ensure!(
        highlights.iter().any(|h| h.stance == "oppose"),
        "failed to generate an oppose highlight with citations"
    );

    warnings.extend(
        briefs
            .iter()
            .filter(|brief| {
                brief
                    .fact_rows
                    .iter()
                    .any(|row| row.citations.is_empty())
            })
            .map(|brief| {
                format!(
                    "brief `{}` contained fact rows without citations; they were excluded from highlights",
                    brief.id
                )
            }),
    );

    let stance_distribution = build_stance_distribution(&stats);
    let total_briefs: u32 = stance_distribution
        .iter()
        .map(|entry| entry.brief_count)
        .sum();
    let total_rows: u32 = stance_distribution
        .iter()
        .map(|entry| entry.fact_row_count)
        .sum();
    let overview = ReviewPanelOverview {
        title: format!("Review Panel Summary — {}", proposal.proposal_id),
        neutral_summary: format!(
            "Panel reviewed {total_briefs} volunteer briefs (support {support}, oppose {oppose}, context {context}) covering {total_rows} fact rows; AI manifest {manifest_hex} ({runtime}) enforced quarantine {quarantine}/10_000 and escalate {escalate}/10_000.",
            support = stats
                .get(&Stance::Support)
                .map(|s| s.brief_count)
                .unwrap_or(0),
            oppose = stats
                .get(&Stance::Oppose)
                .map(|s| s.brief_count)
                .unwrap_or(0),
            context = stats
                .get(&Stance::Context)
                .map(|s| s.brief_count)
                .unwrap_or(0),
            runtime = manifest.body.runtime_version,
            quarantine = manifest.body.thresholds.quarantine,
            escalate = manifest.body.thresholds.escalate,
        ),
        decision_context: format!(
            "Policy jury will evaluate {} target(s) and {} evidence attachment(s); highlights below summarise the balanced record reviewed by the panel.",
            proposal.targets.len(),
            proposal.evidence.len()
        ),
    };

    let volunteer_references = briefs
        .iter()
        .map(|brief| ReviewPanelVolunteerReference {
            brief_id: brief.id.clone(),
            stance: brief.stance.as_str().into(),
            language: brief.language.clone(),
            fact_rows: brief.fact_rows.len() as u32,
            cited_rows: brief
                .fact_rows
                .iter()
                .filter(|row| !row.citations.is_empty())
                .count() as u32,
        })
        .collect();

    Ok(ReviewPanelSummaryV1 {
        version: REVIEW_PANEL_SUMMARY_VERSION_V1,
        proposal_id: proposal.proposal_id.clone(),
        panel_round_id: options.panel_round_id.clone(),
        language: options
            .language_override
            .clone()
            .unwrap_or_else(|| proposal.language.clone()),
        generated_at_unix_ms: options.generated_at_unix_ms.unwrap_or_else(current_unix_ms),
        overview,
        stance_distribution,
        highlights,
        ai_manifest: ReviewPanelAiEvidence {
            manifest_id: manifest.body.manifest_id,
            runtime_version: manifest.body.runtime_version.clone(),
            issued_at_unix: manifest.body.issued_at_unix,
            quarantine_threshold: manifest.body.thresholds.quarantine,
            escalate_threshold: manifest.body.thresholds.escalate,
        },
        volunteer_references,
        warnings,
    })
}

fn build_highlights(
    briefs: &[VolunteerBrief],
    manifest_hex: &str,
    proposal_evidence: Option<&AgendaEvidenceAttachment>,
    warnings: &mut Vec<String>,
) -> Result<Vec<ReviewPanelHighlight>> {
    let mut highlights = Vec::new();
    for stance in [Stance::Support, Stance::Oppose, Stance::Context] {
        let mut rows = Vec::new();
        for brief in briefs.iter().filter(|brief| brief.stance == stance) {
            for row in &brief.fact_rows {
                if row.citations.is_empty() {
                    warnings.push(format!(
                        "fact `{}` in brief `{}` missing citations; skipping",
                        row.id, brief.id
                    ));
                    continue;
                }
                rows.push(HighlightSource {
                    stance,
                    brief_title: brief.title.clone(),
                    row: row.clone(),
                });
            }
        }
        rows.sort_by(|a, b| {
            b.row
                .citations
                .len()
                .cmp(&a.row.citations.len())
                .then_with(|| a.row.id.cmp(&b.row.id))
        });
        for (index, entry) in rows.into_iter().take(2).enumerate() {
            let mut citations = Vec::new();
            for citation in &entry.row.citations {
                citations.push(ReviewPanelCitation {
                    kind: ReviewPanelCitationKind::VolunteerFact,
                    reference_id: entry.row.id.clone(),
                    uri: Some(citation.clone()),
                });
            }
            citations.push(ReviewPanelCitation {
                kind: ReviewPanelCitationKind::AiManifest,
                reference_id: manifest_hex.to_string(),
                uri: None,
            });
            if let Some(evidence) = proposal_evidence {
                citations.push(ReviewPanelCitation {
                    kind: ReviewPanelCitationKind::ProposalEvidence,
                    reference_id: evidence.uri.clone(),
                    uri: Some(evidence.uri.clone()),
                });
            }
            highlights.push(ReviewPanelHighlight {
                id: format!("{}-{}", entry.stance.as_str(), index + 1),
                stance: entry.stance.as_str().into(),
                claim_id: entry.row.id.clone(),
                statement: format!(
                    "{} — status `{}`; impacts {} (brief `{}`).",
                    entry.row.claim,
                    entry.row.status,
                    if entry.row.impact.is_empty() {
                        "unspecified".into()
                    } else {
                        entry.row.impact.join(", ")
                    },
                    entry.brief_title
                ),
                citations,
            });
        }
    }
    Ok(highlights)
}

fn build_stance_distribution(stats: &BTreeMap<Stance, StanceStats>) -> Vec<ReviewPanelStanceCount> {
    let mut out = Vec::new();
    for stance in [Stance::Support, Stance::Oppose, Stance::Context] {
        if let Some(entry) = stats.get(&stance) {
            out.push(ReviewPanelStanceCount {
                stance: stance.as_str().into(),
                brief_count: entry.brief_count,
                fact_row_count: entry.fact_rows,
            });
        }
    }
    out
}

fn aggregate_stance_stats(briefs: &[VolunteerBrief]) -> BTreeMap<Stance, StanceStats> {
    let mut stats: BTreeMap<Stance, StanceStats> = BTreeMap::new();
    for brief in briefs {
        let entry = stats.entry(brief.stance).or_default();
        entry.brief_count += 1;
        entry.fact_rows += brief.fact_rows.len() as u32;
    }
    stats
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn validate_panel_round(value: &str) -> Result<()> {
    ensure!(
        value.starts_with("RP-"),
        "panel round id must start with RP-"
    );
    ensure!(
        value.len() > 3 && value[3..].chars().all(|c| c.is_ascii_digit() || c == '-'),
        "panel round id `{value}` must include a numeric suffix (e.g., RP-2026-05)"
    );
    Ok(())
}

#[derive(Clone, Debug)]
struct VolunteerBrief {
    id: String,
    stance: Stance,
    language: String,
    title: String,
    off_topic: bool,
    fact_rows: Vec<FactRow>,
}

impl VolunteerBrief {
    fn from_value(value: &Value, expected_proposal_id: &str) -> ParseResult<Self> {
        let map = value
            .as_object()
            .ok_or_else(|| "volunteer brief must be an object".to_string())?;
        let brief_id = require_string(map, "brief_id")?;
        let proposal_id = require_string(map, "proposal_id")?;
        if proposal_id != expected_proposal_id {
            return Err(format!(
                "brief `{brief_id}` references proposal `{proposal_id}` but `{expected_proposal_id}` was expected"
            ));
        }
        let language = require_string(map, "language")?;
        let stance_raw = require_string(map, "stance")?;
        let stance = Stance::parse(&stance_raw)?;
        let summary_obj = map
            .get("summary")
            .and_then(Value::as_object)
            .ok_or_else(|| "summary must be an object".to_string())?;
        let title = require_string(summary_obj, "title")?;
        let fact_rows_value = map
            .get("fact_table")
            .ok_or_else(|| "fact_table field missing".to_string())?;
        let fact_rows = parse_fact_rows(fact_rows_value)?;
        if fact_rows.is_empty() {
            return Err(format!(
                "brief `{brief_id}` fact_table must contain at least one entry"
            ));
        }
        let off_topic = map
            .get("moderation")
            .and_then(Value::as_object)
            .and_then(|obj| obj.get("off_topic"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(Self {
            id: brief_id,
            stance,
            language,
            title,
            off_topic,
            fact_rows,
        })
    }
}

#[derive(Clone, Debug)]
struct FactRow {
    id: String,
    claim: String,
    status: String,
    impact: Vec<String>,
    citations: Vec<String>,
}

fn parse_fact_rows(value: &Value) -> ParseResult<Vec<FactRow>> {
    let rows = value
        .as_array()
        .ok_or_else(|| "fact_table must be an array".to_string())?;
    let mut out = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let map = row
            .as_object()
            .ok_or_else(|| format!("fact_table[{index}] must be an object"))?;
        let claim_id = require_string(map, "claim_id")?;
        let claim = require_string(map, "claim")?;
        let status = require_string(map, "status")?;
        let impact = map
            .get("impact")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("fact `{claim_id}` impact must be an array"))?
            .iter()
            .map(|entry| {
                entry
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| format!("fact `{claim_id}` impact entries must be strings"))
            })
            .collect::<ParseResult<Vec<_>>>()?;
        let citations =
            map.get("citations")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("fact `{claim_id}` citations must be an array"))?
                .iter()
                .map(|entry| {
                    entry.as_str().map(|s| s.to_string()).ok_or_else(|| {
                        format!("fact `{claim_id}` citation entries must be strings")
                    })
                })
                .collect::<ParseResult<Vec<_>>>()?;
        out.push(FactRow {
            id: claim_id,
            claim,
            status,
            impact,
            citations,
        });
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Stance {
    Support,
    Oppose,
    Context,
}

impl Stance {
    fn parse(raw: &str) -> ParseResult<Self> {
        match raw.trim().to_lowercase().as_str() {
            "support" => Ok(Self::Support),
            "oppose" => Ok(Self::Oppose),
            "context" => Ok(Self::Context),
            other => Err(format!("stance `{other}` is not supported")),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Support => "support",
            Self::Oppose => "oppose",
            Self::Context => "context",
        }
    }
}

#[derive(Clone)]
struct HighlightSource {
    stance: Stance,
    brief_title: String,
    row: FactRow,
}

#[derive(Default)]
struct StanceStats {
    brief_count: u32,
    fact_rows: u32,
}

fn require_string(map: &JsonMap, key: &str) -> ParseResult<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("field `{key}` missing or not a string"))
}

#[cfg(test)]
mod tests {
    use iroha_data_model::ministry::{
        AGENDA_PROPOSAL_VERSION_V1, AgendaEvidenceAttachment, AgendaEvidenceKind,
        AgendaProposalAction, AgendaProposalSubmitter, AgendaProposalSummary, AgendaProposalTarget,
        AgendaProposalV1,
    };

    use super::*;

    fn sample_proposal() -> AgendaProposalV1 {
        AgendaProposalV1 {
            version: AGENDA_PROPOSAL_VERSION_V1,
            proposal_id: "AC-2026-001".into(),
            submitted_at_unix_ms: 1_780_000_000_000,
            language: "en".into(),
            action: AgendaProposalAction::AddToDenylist,
            summary: AgendaProposalSummary {
                title: "Sample proposal".into(),
                motivation: "Sample motivation".into(),
                expected_impact: "Sample impact".into(),
            },
            tags: vec!["csam".into()],
            targets: vec![AgendaProposalTarget {
                label: "Sample target".into(),
                hash_family: "blake3-256".into(),
                hash_hex: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcd".into(),
                reason: "Reason".into(),
            }],
            evidence: vec![AgendaEvidenceAttachment {
                kind: AgendaEvidenceKind::Url,
                uri: "https://example.invalid/evidence".into(),
                digest_blake3_hex: None,
                description: Some("Example evidence".into()),
            }],
            submitter: AgendaProposalSubmitter {
                name: "Citizen".into(),
                contact: "citizen@example.org".into(),
                organization: None,
                pgp_fingerprint: None,
            },
            duplicates: vec![],
        }
    }

    fn sample_manifest() -> ModerationReproManifestV1 {
        use iroha_data_model::sorafs::moderation::{
            ModerationModelFingerprintV1, ModerationReproBodyV1, ModerationReproSignatureV1,
            ModerationSeedMaterialV1, ModerationThresholdsV1,
        };
        ModerationReproManifestV1 {
            body: ModerationReproBodyV1 {
                schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
                manifest_id: [0x11; 16],
                manifest_digest: [0x22; 32],
                runner_hash: [0x33; 32],
                runtime_version: "sorafs-ai-runner 0.5.0".into(),
                issued_at_unix: 1_780_000_000,
                seed_material: ModerationSeedMaterialV1 {
                    domain_tag: "ai-runner".into(),
                    seed_version: 1,
                    run_nonce: [0x44; 32],
                },
                thresholds: ModerationThresholdsV1 {
                    quarantine: 7800,
                    escalate: 3200,
                },
                models: vec![ModerationModelFingerprintV1 {
                    model_id: [0x55; 16],
                    artifact_digest: [0x66; 32],
                    weights_digest: [0x77; 32],
                    opset: 17,
                    weight: Some(5_000),
                }],
                notes: None,
            },
            signatures: vec![ModerationReproSignatureV1 {
                role: "council".into(),
                public_key: iroha_crypto::PublicKey::from_hex(
                    iroha_crypto::Algorithm::Ed25519,
                    "87FDCACF58B891947600B0C37795CADB5A2AE6DE612338FDA9489AB21CE427BA",
                )
                .expect("public key"),
                signature: iroha_crypto::SignatureOf::from_signature(
                    iroha_crypto::Signature::from_hex(hex_encode([0xAA; 64])).expect("signature"),
                ),
            }],
        }
    }

    fn sample_brief(stance: Stance, id: &str, claim_suffix: &str) -> VolunteerBrief {
        VolunteerBrief {
            id: id.into(),
            stance,
            language: "en".into(),
            title: format!("Brief {id}"),
            off_topic: false,
            fact_rows: vec![FactRow {
                id: format!("VB-{claim_suffix}"),
                claim: format!("Claim {claim_suffix}"),
                status: "corroborated".into(),
                impact: vec!["governance".into()],
                citations: vec![format!("https://example.invalid/{claim_suffix}")],
            }],
        }
    }

    #[test]
    fn volunteer_parser_rejects_mismatched_proposal_id() {
        let value = norito::json!([{
            "brief_id": "VB-1",
            "proposal_id": "AC-OTHER",
            "language": "en",
            "stance": "support",
            "submitted_at": "2026-04-01T00:00:00Z",
            "summary": {"title": "Sample", "abstract": "a", "requested_action": "publish"},
            "fact_table": [{
                "claim_id": "VB-1-F1",
                "claim": "Example",
                "status": "corroborated",
                "impact": ["governance"],
                "citations": ["https://example.invalid"]
            }]
        }]);
        let err = parse_volunteer_briefs(value, "AC-2026-001")
            .expect_err("parser must reject mismatched proposal");
        assert!(
            err.to_string().contains("references proposal `AC-OTHER`"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn summary_requires_balanced_stances() {
        let options = SynthesizeOptions {
            proposal_path: PathBuf::new(),
            volunteer_path: PathBuf::new(),
            ai_manifest_path: PathBuf::new(),
            output_path: PathBuf::new(),
            panel_round_id: "RP-2026-05".into(),
            language_override: None,
            generated_at_unix_ms: Some(1),
        };
        let proposal = sample_proposal();
        let manifest = sample_manifest();
        let briefs = vec![sample_brief(Stance::Support, "VB-1", "1")];
        let err = build_review_panel_summary(&options, &proposal, &manifest, &briefs)
            .expect_err("imbalanced stances must fail");
        assert!(
            err.to_string()
                .contains("include both support and oppose stances")
        );
    }

    #[test]
    fn summary_includes_highlights_and_citations() {
        let options = SynthesizeOptions {
            proposal_path: PathBuf::new(),
            volunteer_path: PathBuf::new(),
            ai_manifest_path: PathBuf::new(),
            output_path: PathBuf::new(),
            panel_round_id: "RP-2026-05".into(),
            language_override: None,
            generated_at_unix_ms: Some(1),
        };
        let proposal = sample_proposal();
        let manifest = sample_manifest();
        let briefs = vec![
            sample_brief(Stance::Support, "VB-1", "1"),
            sample_brief(Stance::Oppose, "VB-2", "2"),
        ];
        let summary = build_review_panel_summary(&options, &proposal, &manifest, &briefs)
            .expect("summary builds");
        assert_eq!(summary.highlights.len(), 2);
        assert!(summary.highlights.iter().all(|highlight| {
            highlight
                .citations
                .iter()
                .any(|citation| citation.kind == ReviewPanelCitationKind::AiManifest)
        }));
    }
}
