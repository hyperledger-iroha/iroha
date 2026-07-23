<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
title: SNS Arbitration Toolkit
summary: Evidence-only case schema and handling boundaries for alias disputes.
---

# Sora Name Service arbitration toolkit

This toolkit is an off-chain case and evidence format. It does not register,
transfer, rebind, freeze, unfreeze, auction, renew, or otherwise mutate an SNS
record. The first release exposes no `sns governance case` command and no
freeze/unfreeze/auction mutation command. Do not present such commands, legacy
Torii routes, or a dashboard export as implemented release behavior.

The canonical case schema is
`docs/examples/sns/arbitration_case_schema.json`; the publication outline is
`docs/examples/sns/arbitration_transparency_report_template.md`. They support
human governance and external case-management tooling only. They are not
Norito transaction payloads.

## 1. Intake

Assign an immutable case ID and record:

- the canonical textual resource and, when known, its expected numeric
  dataspace ID;
- allegation, policy reference, priority, reporter/respondents, and timestamps;
- content-addressed evidence references, hashes, access classification, and
  retention policy; and
- acknowledgement/resolution deadlines and every approved extension.

Keep raw tokens, authorization headers, private keys, and unredacted identity
material outside the case file. A hash or redacted reference is sufficient for
restricted evidence.

## 2. Canonical technical evidence

For a provisioning or billing dispute, collect evidence from the supported
flow:

1. the approved secret-free alias intent;
2. the canonical signed planner request and response;
3. the `AliasTransactionPlanV1` body/hash, ordered framed instructions,
   disposition, exact quote, cap, policy version, payment asset, and anchor;
4. the locally signed ordinary transaction and committed/rejected result;
5. payer and owner ledger balances before/after; and
6. a redacted `AliasSetupReportV1` plus visibility-authorized query results.

A dashboard, application log, or screenshot can provide context but cannot
replace these canonical artifacts. Never infer a charge from the cap: consensus
charges the exact recomputed quote.

## 3. Triage questions

- Did canonical text resolve to the expected `DataSpaceId` in both the plan and
  execution context?
- Was the resource classified before quote validation as `NoOp`, `Repair`,
  `Create`, or conflict?
- Did a conflict return 409 without any partial executable plan?
- Did the local verifier reproduce the plan hash and exact framed bytes?
- Was the complete ordered vector submitted as one transaction?
- Does the ledger debit equal the exact calculated amount, with no second
  charge on replay?
- On rejection, are earlier resource, binding, index, permission, and balance
  writes absent?
- Were restricted reads authorized in the 401 → 403 → 404 order without
  existence leakage?

## 4. Decision and remediation boundary

Record findings, approvals, dissent, effective time, publication state, and the
exact remediation authorized. The case decision itself is not executable.

If a documented registered instruction supports the remediation, create or
plan that instruction through its supported API, obtain the required
signatures, and submit it in a normal transaction separate from the case
record. Renewal uses expiry CAS; rebind and primary-alias changes use their CAS
instructions; auto-renew uses expected revision.

If no registered instruction supports the requested freeze, unfreeze, transfer,
auction, refund, or other action, mark remediation `Blocked`. Do not use raw
domain registration, a storage edit, a removed `/v1/sns` mutation route, or an
undocumented CLI command as a substitute.

## 5. SLA and publication

Teams may adopt local acknowledgement and decision SLAs, but those deadlines
are governance policy rather than consensus behavior. Calculate reports from
the reviewed case files and canonical transaction evidence. Publish only
aggregates and redacted case summaries; filter restricted resources and sealed
attachments before computing public totals.

The repository does not currently provide a supported arbitration submission
CLI, Torii case endpoint, case-event metric contract, or canonical arbitration
Grafana dashboard. Any future automation must be implemented and tested before
its command, route, metric, or dashboard is added to this runbook.

## 6. Minimum case checklist

- [ ] Case schema validated by the repository's approved document validation
      workflow.
- [ ] Secrets and restricted identity evidence are absent or separately sealed.
- [ ] Canonical alias plan/transaction/readiness evidence is attached when
      relevant.
- [ ] Decision identifies a supported typed remediation or explicitly says
      `Blocked`.
- [ ] No removed SNS mutation command or route appears in the execution log.
- [ ] Public report totals are calculated after visibility/redaction filtering.

See [`governance_playbook.md`](./governance_playbook.md),
[`registrar_api.md`](./registrar_api.md), and
[`payment_settlement_plan.md`](./payment_settlement_plan.md) for the operational
surfaces that are implemented.
