---
lang: ur
direction: rtl
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6609b9628186b58c0441d1ca1594a3030b7f7d81bb2df9c36af3a9d65cdd963d
source_last_modified: "2025-11-15T20:04:53.140162+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/finance/repo_custodian_ack_template.md کا اردو ترجمہ -->

# Repo Custodian Acknowledgement Template

جب کوئی repo (bilateral یا tri-party) `RepoAgreement::custodian` کے ذریعے custodian کو reference کرے تو یہ template استعمال کریں۔ مقصد یہ ہے کہ assets move ہونے سے پہلے custody SLA، routing accounts، اور drill contacts ریکارڈ ہوں۔ template کو اپنے evidence directory میں کاپی کریں (مثال کے طور پر
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`)، placeholders بھریں، اور فائل کا hash `docs/source/finance/repo_ops.md` sec 2.8 میں بیان کردہ governance packet کے حصے کے طور پر نکالیں۔

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

1. *Custody readiness:* "ہم نے staged `repo initiate` payload کو اوپر والے identifiers کے ساتھ review کیا ہے اور sec 2 میں درج SLA کے تحت collateral قبول کرنے کے لئے تیار ہیں۔"
2. *Rollback commitment:* "incident commander کی ہدایت پر ہم اوپر والا rollback playbook چلائیں گے، اور `governance/drills/<timestamp>.log` میں CLI logs اور hashes فراہم کریں گے۔"
3. *Evidence retention:* "ہم acknowledgement، standing instructions، اور CLI logs کو کم از کم `<duration>` تک محفوظ رکھیں گے اور ضرورت پر finance council کو فراہم کریں گے۔"

نیچے دستخط کریں (governance tracker کے ذریعے routed ہونے پر electronic signatures قابل قبول ہیں).

| Name | Role | Signature / date |
|------|------|------------------|
| `<custodian ops lead>` | Custodian operator | `<signature>` |
| `<desk lead>` | Desk | `<signature>` |
| `<counterparty>` | Counterparty | `<signature>` |

> دستخط کے بعد، فائل کا hash نکالیں (مثال: `sha256sum custodian_ack_<cust>.md`) اور digest کو governance packet table میں ریکارڈ کریں تاکہ reviewers ووٹ کے دوران referenced acknowledgement bytes کو verify کر سکیں.

</div>
