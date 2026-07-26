---
lang: kk
direction: ltr
source: docs/source/sdk/swift/readiness/cards/swift_telemetry_redaction_qrc.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d70f1965f042a24c895d4a65bd3973be1541b2a95487e904286b8d43056a07b
source_last_modified: "2025-12-29T18:16:36.078401+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Swift Telemetry Redaction — Quick Reference Card

Use this cheat sheet during incidents, rehearsals, or support escalations. The
full policy lives in `docs/source/sdk/swift/telemetry_redaction.md`; this card
captures the operational knobs that on-call engineers need immediately.

## Override Workflow (10-minute target)
1. Confirm requester has SRE/Support approval.
2. Run `python3 scripts/swift_status_export.py telemetry-override create \
   --reason "<ticket>" --expires-in 24h`.
3. Capture the signed Norito blob + digest from the script output.
4. Update `docs/source/sdk/swift/telemetry_redaction.md` Appendix B and the
   override ledger snippet in the support playbook.
5. Schedule removal (`telemetry-override revoke --id <digest>`) before expiry.

## Salt Rotation Checklist
- First Monday of each quarter at 09:00 UTC.
- Update vault secret + `iroha_config.telemetry.redaction_salt`.
- Run `scripts/swift_collect_redaction_status.py --record-sample`.
- Verify `swift.telemetry.redaction.salt_version` gauge bumps in
  `dashboards/mobile_parity.swift`.
- Log rotation minutes under
  `docs/source/sdk/swift/readiness/archive/<year>-<month>/salt_rotation.md`.

## Dashboards & Alerts
- `dashboards/mobile_parity.swift` → Telemetry block (salt, overrides, exporter
  status).
- `dashboards/mobile_ci.swift` → Buildkite lanes + exporter smoketests.
- Alert routing: #telemetry-mobile (Slack) + PagerDuty “SDK Observability”.

## Incident Artefacts
- Screenshots/logs: `docs/source/sdk/swift/readiness/screenshots/<date>/`.
- Lab/chaos reports: `docs/source/sdk/swift/readiness/labs/swift_telemetry_lab_01.md`.
- Emails & receipts: `docs/source/sdk/swift/readiness/archive/<date>/`.

## Contacts
- DRI: Mei Nakamura (Swift Observability TL)
- Support Escalation: Elias Ortega / Docs rota
- Backup reviewer: LLM (IOS7/IOS8 acting owner)
