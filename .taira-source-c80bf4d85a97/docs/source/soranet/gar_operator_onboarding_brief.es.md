---
lang: es
direction: ltr
source: docs/source/soranet/gar_operator_onboarding_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a8e9039ada95afb08e568362a34a60f6cf3cddf1e93f7d588bcf9e25a655867
source_last_modified: "2026-01-03T18:08:01.743760+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraNet GAR Operator Onboarding Brief
summary: Step-by-step checklist for operators enabling SNNet-9 compliance policies with jurisdictional attestation digests.
---

# Operator Onboarding Brief (SNNet-9)

This brief gives operators a deterministic path to enable the SNNet-9
compliance policies. It ties together the jurisdictional review, configuration
snippets, validation commands, and evidence capture so rollouts stay auditable.

## Prerequisites

- Read the [GAR Jurisdictional Review](gar_jurisdictional_review.md) and note
  the current attestation digests and next-review dates.
- Pull the canonical opt-out catalogue from
  `governance/compliance/soranet_opt_outs.json`.
- Collect signed PDFs for the jurisdictions that apply to your footprint
  (reference URIs used in `compliance.attestations`).

## Onboarding steps

1. **Assemble config**
   - Start from the sample in the jurisdictional review and merge it with your
     `operator_jurisdictions`.
   - Keep the attestation digests identical to the signed memos; do not trim
     trailing zeros or change casing.
2. **Validate**
   - Run `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`.
   - Run `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`.
   - Optional: run `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>` if you want to attach a suppression budget snapshot alongside the attestation.
3. **Record evidence**
   - Create `artifacts/soranet/compliance/<YYYYMMDD>/` and store:
     - `config.json` (final compliance block),
     - `attestations.json` (list of URIs + digests),
     - validation logs,
     - links to signed PDFs or Norito envelopes.
4. **Activate**
   - Tag the rollout (`gar-opt-out-<date>`) and log the activation window +
     approvers in your GAR logbook.
   - Redeploy orchestrator/SDK clients with the updated config.
   - Verify production logs emit `compliance_jurisdiction_opt_out` / `compliance_blinded_cid_opt_out` where expected.
5. **Close-out**
   - File the evidence bundle with the Governance Council.
   - Schedule the next review dates from the jurisdictional review table.

## Quick checklist

- [ ] `jurisdiction_opt_outs` matches the canonical catalogue.
- [ ] Attestation digests copied exactly as published.
- [ ] Validation commands executed and logs archived.
- [ ] Evidence bundle stored under `artifacts/soranet/compliance/<date>/`.
- [ ] Rollout tag and GAR logbook updated.
- [ ] Next-review reminders scheduled.

## References

- [GAR Compliance Playbook](gar_compliance_playbook.md)
- [GAR Jurisdictional Review](gar_jurisdictional_review.md)
- [Opt-out filing template](templates/gar_opt_out_filing_template.md)
- [SoraFS Orchestrator Configuration](../sorafs/developer/orchestrator.md)
