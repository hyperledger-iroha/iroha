---
lang: kk
direction: ltr
source: docs/source/ministry/referendum_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 922d972376b67a2f8c0c03ded95db6576e16e229e4bcb62d920b0ffda49c93ac
source_last_modified: "2025-12-29T18:16:35.980526+00:00"
translation_last_reviewed: 2026-02-07
title: Referendum Packet Workflow (MINFO-4)
summary: Produce the complete referendum dossier (`ReferendumPacketV1`) combining the proposal, neutral summary, sortition artefacts, and impact report.
---

# Referendum Packet Workflow (MINFO-4)

Roadmap item **MINFO-4 — In Review panel & referendum synthesizer** is now
fulfilled by the new `ReferendumPacketV1` Norito schema plus the CLI helpers
described below. The workflow bundles every artefact required for policy-jury
votes into a single JSON document so governance, auditors, and transparency
portals can replay the evidence deterministically.

## Inputs

1. **Agenda proposal** — same JSON used for `cargo xtask ministry-agenda validate`.
2. **Volunteer briefs** — the curated dataset produced after linting via
   `cargo xtask ministry-transparency volunteer-validate`.
3. **AI moderation manifest** — governance-signed `ModerationReproManifestV1`.
4. **Sortition summary** — deterministic artefact emitted by
   `cargo xtask ministry-agenda sortition`. The JSON follows
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md) so governance can
   reproduce the POP snapshot digest and waitlist/failover wiring.
5. **Impact report** — hash-family/report generated via
   `cargo xtask ministry-agenda impact`.

## CLI usage

```bash
cargo xtask ministry-panel packet \
  --proposal artifacts/ministry/proposals/AC-2026-001.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --sortition artifacts/ministry/agenda_sortition_2026Q1.json \
  --impact artifacts/ministry/impact/AC-2026-001.json \
  --summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/referendum_packets/AC-2026-001.json
```

The `packet` subcommand runs the neutral-summary synthesizer (MINFO-4a), reuses
the existing volunteer fixtures, and enriches the output with:

- `ReferendumSortitionEvidence` — algorithm, seed, and roster digests from the
  sortition artefact.
- `ReferendumPanelist[]` — each selected council member plus the Merkle proof
  needed to audit their draw.
- `ReferendumImpactSummary` — per-hash-family totals and conflict listings from
  the impact report.

Use `--summary-out` when you still need the standalone `ReviewPanelSummaryV1`
file; otherwise the packet embeds the summary under `review_summary`.

## Output structure

`ReferendumPacketV1` lives in
`crates/iroha_data_model/src/ministry/mod.rs` and is available across SDKs.
Key sections include:

- `proposal` — the original `AgendaProposalV1` object.
- `review_summary` — the balanced summary emitted by MINFO-4a.
- `sortition` / `panelists` — reproducible proofs for the seated council.
- `impact_summary` — duplicate/policy conflict evidence per hash family.

See `docs/examples/ministry/referendum_packet_example.json` for a full sample.
Attach the generated packet to every referendum dossier alongside the signed AI
manifest and transparency artefacts referenced by the highlights section.
