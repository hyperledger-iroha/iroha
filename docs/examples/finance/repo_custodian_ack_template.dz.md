---
lang: dz
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c52d7f2c5ec9dc4cda81895561bc1261659935c94bf3f7febb0867f4981fe616
source_last_modified: "2026-01-22T16:26:46.472177+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo Custodian Acknowledgement Template

Use this template when a repo (bilateral or tri-party) references a custodian
via `RepoAgreement::custodian`. The goal is to record the custody SLA, routing
accounts, and drill contacts before assets move. Copy the template into your
evidence directory (for example
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`), fill the
placeholders, and hash the file as part of the governance packet described in
`docs/source/finance/repo_ops.md` §2.8.

## 1. Metadata

| Field | Value |
|-------|-------|
| Agreement identifier | `<repo-yyMMdd-XX>` |
| Custodian account id | `<ih58...>` |
| Prepared by / date | `<custodian ops lead>` |
| Desk contacts acknowledged | `<desk lead + counterparty>` |
| Evidence directory | ``artifacts/finance/repo/<slug>/`` |

## 2. Custody Scope

- **Collateral definitions received:** `<list of asset definition ids>`
- **Cash leg currency / settlement rail:** `<xor#sora / other>`
- **Custody window:** `<start/end timestamps or SLA summary>`
- **Standing instructions:** `<hash + path to standing instruction document>`
- **Automation prerequisites:** `<scripts, configs, or runbooks custodian will invoke>`

## 3. Routing & Monitoring

| Item | Value |
|------|-------|
| Custody wallet / ledger account | `<asset ids or ledger path>` |
| Monitoring channel | `<Slack/phone/on-call rotation>` |
| Drill contact | `<primary + backup>` |
| Required alerts | `<PagerDuty service, Grafana board, etc.>` |

## 4. Statements

1. *Custody readiness:* “We reviewed the staged `repo initiate` payload with the
   identifiers above and are prepared to accept collateral under the SLA listed
   in §2.”
2. *Rollback commitment:* “We will execute the rollback playbook named above if
   directed by the incident commander, and will provide CLI logs plus hashes in
   `governance/drills/<timestamp>.log`.”
3. *Evidence retention:* “We will keep the acknowledgement, standing
   instructions, and CLI logs for at least `<duration>` and provide them to the
   finance council upon request.”

Sign below (electronic signatures acceptable when routed through the governance
tracker).

| Name | Role | Signature / date |
|------|------|------------------|
| `<custodian ops lead>` | Custodian operator | `<signature>` |
| `<desk lead>` | Desk | `<signature>` |
| `<counterparty>` | Counterparty | `<signature>` |

> Once signed, hash the file (example: `sha256sum custodian_ack_<cust>.md`) and
> record the digest in the governance packet table so reviewers can verify the
> acknowledgement bytes referenced during the vote.
