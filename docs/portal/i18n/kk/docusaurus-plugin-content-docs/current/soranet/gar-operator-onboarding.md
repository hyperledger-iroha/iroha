---
lang: kk
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
---

Use this brief to roll out the SNNet-9 compliance configuration with a repeatable,
audit-friendly process. Pair it with the jurisdictional review so every operator
uses the same digests and evidence layout.

## Steps

1. **Assemble config**
   - Import `governance/compliance/soranet_opt_outs.json`.
   - Merge your `operator_jurisdictions` with the attestation digests published
     in the [jurisdictional review](gar-jurisdictional-review).
2. **Validate**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - Optional: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Capture evidence**
   - Store under `artifacts/soranet/compliance/<YYYYMMDD>/`:
     - `config.json` (final compliance block)
     - `attestations.json` (URIs + digests)
     - validation logs
     - references to signed PDFs/Norito envelopes
4. **Activate**
   - Tag the rollout (`gar-opt-out-<date>`), redeploy orchestrator/SDK configs,
     and confirm `compliance_*` events emit in logs where expected.
5. **Close out**
   - File the evidence bundle with Governance Council.
   - Log the activation window + approvers in the GAR logbook.
   - Schedule the next-review dates from the jurisdictional review table.

## Quick checklist

- [ ] `jurisdiction_opt_outs` matches the canonical catalogue.
- [ ] Attestation digests copied exactly.
- [ ] Validation commands run and archived.
- [ ] Evidence bundle stored in `artifacts/soranet/compliance/<date>/`.
- [ ] Rollout tag + GAR logbook updated.
- [ ] Next-review reminders set.

## See also

- [GAR Jurisdictional Review](gar-jurisdictional-review)
- [GAR Compliance Playbook (source)](../../../source/soranet/gar_compliance_playbook.md)
