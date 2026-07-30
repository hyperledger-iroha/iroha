<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo Custodian Acknowledgement Template

Use this supplementary operations template when a tri-party repo references a
custodian via `RepoAgreement::custodian`. Ledger authorization comes only from
the custodian's owner-signed exact maturity `CanExecuteSettlement` Grant; this
document cannot replace it. The template records the custody SLA, exact balance
scope, and contacts before assets move. Copy the template into your
evidence directory (for example
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`), fill the
placeholders, and hash the file as supplementary evidence for
`docs/source/finance/repo_ops.md`.

## 1. Metadata

| Field | Value |
|-------|-------|
| Agreement identifier | `<repo-yyMMdd-XX>` |
| Custodian account id | `<i105...>` |
| Prepared by / date | `<custodian ops lead>` |
| Desk contacts acknowledged | `<desk lead + counterparty>` |
| Evidence directory | ``artifacts/finance/repo/<slug>/`` |

## 2. Custody Scope

- **Collateral definitions received:** `<list of asset definition ids>`
- **Exact collateral custody AssetId:** `<definition + custodian + dataspace scope>`
- **Maturity consent intent hash:** `<RepoIsi::maturity_intent_hash()>`
- **Grant transaction / finality receipt:** `<paths + hashes>`
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
2. *Maturity release:* “We issued the exact owner-signed maturity permission
   recorded above and will preserve the consent-selected collateral scope until
   the immutable agreement settles. We understand that early unwind and
   caller-selected substitution are not protocol operations.”
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
