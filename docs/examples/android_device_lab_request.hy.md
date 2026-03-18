---
lang: hy
direction: ltr
source: docs/examples/android_device_lab_request.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a8e6a4981a11faac56d9b04432773e94fd59f8e2524fa4c552be459291c7c39
source_last_modified: "2025-12-29T18:16:35.068386+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android սարքի լաբորատորիայի ամրագրման հայտի ձևանմուշ

Պատճենել այս ձևանմուշը `_android-device-lab` Jira հերթում ամրագրման ժամանակ
ապարատային. Կցեք հղումներ Buildkite խողովակաշարերին, համապատասխանության արտեֆակտներին և ցանկացած այլ
գործընկեր տոմսեր, որոնք կախված են վազքից:

```
Summary: <Milestone / workload> – <lane(s)> – <date/time UTC>

Milestone / Tracking:
- Roadmap item: AND6 / AND7 / AND8 (choose)
- Related ticket(s): <link to ANDx issue>, <partner-sla reference if any>

Requestor / Contact:
- Primary engineer:
- Backup engineer:
- Slack channel / pager escalation:

Reservation details:
- Lanes required: <pixel8pro-strongbox-a / pixel8a-ci-b / pixel7-fallback / firebase-burst / strongbox-external>
- Desired slot: <YYYY-MM-DD HH:MM UTC> for <duration>
- Workload type: <CI smoke / attestation sweep / chaos rehearsal / partner demo>
- Tooling to run: <scripts/buildkite job names>
- Artefacts produced: <logs, attestation bundles, dashboards>

Dependencies:
- Capacity snapshot reference: link to `android_strongbox_capture_status.md`
- Readiness matrix rows touched: link to `android_strongbox_device_matrix.md`
- Compliance linkage (if any): AND6 checklist row, evidence log ID

Fallback plan:
- If primary slot unavailable, alternate slot is:
- Needs fallback pool / Firebase? (yes/no)
- External StrongBox retainer required? (yes/no – include lead time)

Approvals:
- Hardware Lab Lead:
- Android Foundations TL (when CI lanes impacted):
- Program Lead (if StrongBox retainer invoked):

Post-run checklist:
- Attach Buildkite URL(s):
- Update evidence log row: <ID/date>
- Note deviations/overruns:
```