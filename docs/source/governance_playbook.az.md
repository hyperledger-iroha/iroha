---
lang: az
direction: ltr
source: docs/source/governance_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9201c0027f05b1ab2c83fa6b3e1a1e6dad3ff9660a8ed23bac7667408d421ada
source_last_modified: "2026-01-22T14:35:37.551676+00:00"
translation_last_reviewed: 2026-02-07
---

# Governance Playbook

This playbook captures the day-to-day rituals that keep the Sora Network
governance council aligned. It aggregates the authoritative references from the
repository so individual ceremonies can remain concise, while operators always
have a single entry point for the broader process.

## Council Ceremonies

- **Fixture governance** – See [Sora Parliament Fixture Approval](sorafs/signing_ceremony.md)
  for the on-chain approval flow that the Parliament’s Infrastructure Panel now
  follows when reviewing SoraFS chunker updates.
- **Vote tally publication** – Refer to
  [Governance Vote Tally](governance_vote_tally.md) for the step-by-step CLI
  workflow and reporting template.

## Operational Runbooks

- **API integrations** – [Governance API reference](governance_api.md) lists the
  REST/gRPC surfaces exposed by council services, including authentication
  requirements and pagination rules.
- **Telemetry dashboards** – The Grafana JSON definitions under
  `docs/source/grafana_*` define the “Governance Constraints” and “Scheduler
  TEU” boards. Export the JSON into Grafana after each release to stay aligned
  with the canonical layout.

## Data Availability Oversight

### Retention classes

Parliament panels approving DA manifests must reference the enforced retention
policy before voting. The table below mirrors the defaults enforced via
`torii.da_ingest.replication_policy` so reviewers can spot mismatches without
hunting for the source TOML.【docs/source/da/replication_policy.md:1】

| Governance tag | Blob class | Hot retention | Cold retention | Required replicas | Storage class |
|----------------|------------|---------------|----------------|-------------------|---------------|
| `da.taikai.live` | `taikai_segment` | 24 h | 14 d | 5 | `hot` |
| `da.sidecar` | `nexus_lane_sidecar` | 6 h | 7 d | 4 | `warm` |
| `da.governance` | `governance_artifact` | 12 h | 180 d | 3 | `cold` |
| `da.default` | _all other classes_ | 6 h | 30 d | 3 | `warm` |

The Infrastructure Panel should attach the filled template from
`docs/examples/da_manifest_review_template.md` to every ballot so the manifest
digest, retention tag, and Norito artefacts remain linked in the governance
record.

### Signed manifest audit trail

Before a ballot reaches the agenda, council staff must prove that the manifest
bytes under review match the Parliament envelope and the SoraFS artefact. Use
the existing tooling to collect that evidence:

1. Fetch the manifest bundle from Torii (`iroha app da get-blob --storage-ticket <hex>`
   or the equivalent SDK helper) so everyone hashes the same bytes that reached
   the gateways.
2. Run the manifest stub verifier with the signed envelope:
   ```
   cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json \
     --manifest-signatures-in=fixtures/sorafs_chunker/manifest_signatures.json \
     --json-out=/tmp/manifest_report.json
   ```
   This recomputes the BLAKE3 manifest digest, validates the
   `chunk_digest_sha3_256`, and checks every Ed25519 signature embedded in
   `manifest_signatures.json`. See `docs/source/sorafs/manifest_pipeline.md`
   for additional CLI options.
3. Copy the digest, `chunk_digest_sha3_256`, profile handle, and signer list into
   the review template. NOTE: if the verifier reports “profile mismatch” or a
   missing signature, halt the vote and request a corrected envelope.
4. Store the verifier output (or CI artefact from
   `ci/check_sorafs_fixtures.sh`) alongside the Norito `.to` payload so auditors
   can replay the evidence without accessing internal gateways.

The resulting audit pack should let Parliament recreate every hash and signature
check even after the manifest is rotated out of hot storage.

### Review checklist

1. Pull the Parliament-approved manifest envelope (see
   `docs/source/sorafs/signing_ceremony.md`) and record the BLAKE3 digest.
2. Verify the manifest’s `RetentionPolicy` block matches the tag in the table
   above; Torii will reject mismatches, but the council must capture the
   evidence for auditors.【docs/source/da/replication_policy.md:32】
3. Confirm that the submitted Norito payload references the same retention tag
   and blob class that appears in the intake ticket.
4. Attach proof of the policy check (CLI output, `torii.da_ingest.replication_policy`
   dump, or CI artefact) to the review packet so SRE can replay the decision.
5. Record planned subsidy taps or rent adjustments when the proposal depends on
   `docs/source/sorafs_reserve_rent_plan.md`.

### Escalation matrix

| Request type | Owning panel | Evidence to attach | Deadlines & telemetry | References |
|--------------|--------------|--------------------|-----------------------|------------|
| Subsidy / rent adjustment | Infrastructure + Treasury | Filled DA packet, rent delta from `reserve_rentd`, updated reserve projection CSV, council vote minutes | Note rent impact before submitting the Treasury update; include rolling 30 d buffer telemetry so Finance can reconcile within the next settlement window | `docs/source/sorafs_reserve_rent_plan.md`, `docs/examples/da_manifest_review_template.md` |
| Moderation takedown / compliance action | Moderation + Compliance | Compliance ticket (`ComplianceUpdateV1`), proof tokens, signed manifest digest, appeal status | Follow the gateway compliance SLA (acknowledge within 24 h, full removal ≤72 h). Attach `TransparencyReportV1` excerpt showing the action. | `docs/source/sorafs_gateway_compliance_plan.md`, `docs/source/sorafs_moderation_panel_plan.md` |
| Emergency freeze / rollback | Parliament moderation panel | Prior approval packet, new freeze order, rollback manifest digest, incident log | Publish freeze notice immediately and schedule the rollback referendum within the next governance slot; include buffer saturation + DA replication telemetry to justify the emergency. | `docs/source/sorafs/signing_ceremony.md`, `docs/source/sorafs_moderation_panel_plan.md` |

Use the table when triaging intake tickets so every panel receives the exact
artefacts required to execute its mandate.

### Reporting deliverables

Every DA-10 decision must ship with the following artefacts (attach them to the
Governance DAG entry referenced in the vote):

- The completed Markdown packet from
  `docs/examples/da_manifest_review_template.md` (now including signature and
  escalation sections).
- The signed Norito manifest (`.to`) plus the `manifest_signatures.json` envelope
  or CI verifier logs that prove the fetch digest.
- Any transparency updates triggered by the action:
  - `TransparencyReportV1` delta for takedowns or compliance-driven freezes.
  - Rent/reserve ledger delta or `ReserveSummaryV1` snapshot for subsidies.
- Links to telemetry snapshots collected during the review (replication depth,
  buffer headroom, moderation backlog) so observers can cross-check conditions
  after the fact.

## Moderation & Escalation

Gateway takedowns, subsidy clawbacks, or DA freezes follow the compliance
pipeline described in `docs/source/sorafs_gateway_compliance_plan.md` and the
appeal tooling in `docs/source/sorafs_moderation_panel_plan.md`. Panels should:

1. Log the originating compliance ticket (`ComplianceUpdateV1` or
   `ModerationAppealV1`) and attach the associated proof tokens.【docs/source/sorafs_gateway_compliance_plan.md:20】
2. Confirm whether the request invokes the moderation appeal path (citizen panel
   vote) or an emergency Parliament freeze; both flows must cite the manifest
   digest and retention tag captured in the new template.【docs/source/sorafs_moderation_panel_plan.md:1】
3. Enumerate escalation deadlines (appeal commit/reveal windows, emergency
   freeze duration) and state which council or panel owns the follow-up.
4. Capture the telemetry snapshot (buffer headroom, moderation backlog) used to
   justify the action so downstream audits can match the decision to the live
   state.

The compliance and moderation panels must sync their weekly transparency reports
with the settlement router operators so takedowns and subsidies affect the same
set of manifests.

## Reporting Templates

All DA-10 reviews now require a signed Markdown packet. Copy
`docs/examples/da_manifest_review_template.md`, populate the manifest metadata,
retention verification table, and panel vote summary, then pin the completed
document (plus referenced Norito/JSON artefacts) to the Governance DAG entry.
Panels should link the packet in the governance minutes so future takedowns or
subsidy renewals can cite the original manifest digest without re-running the
entire ceremony.

## Incident & Revocation Workflow

Emergency actions now happen on-chain. When a fixture release needs to be
rolled back, file a governance ticket and open a Parliament revert proposal
pointing at the previously approved manifest digest. The Infrastructure Panel
handles the vote, and once finalized the Nexus runtime publishes the rollback
event that downstream clients consume. No local JSON artefacts are required.

## Keeping the Playbook Current

- Update this file whenever a new governance-facing runbook lands in the
  repository.
- Cross-link new ceremonies here so the council index remains discoverable.
- If a referenced document moves (for example, a new SDK path), update the link
  as part of the same pull request to avoid stale pointers.
