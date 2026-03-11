---
lang: ja
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6609b9628186b58c0441d1ca1594a3030b7f7d81bb2df9c36af3a9d65cdd963d
source_last_modified: "2025-11-15T20:04:53.140162+00:00"
translation_last_reviewed: 2026-01-01
---

# Repo Custodian Acknowledgement Template

repo (bilateral または tri-party) が `RepoAgreement::custodian` で custodian を参照する場合に、このテンプレートを使用します。目的は、資産移動前に custody SLA、ルーティングアカウント、drill 連絡先を記録することです。テンプレートをエビデンスディレクトリ (例: `artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`) にコピーし、placeholders を埋め、`docs/source/finance/repo_ops.md` sec 2.8 で説明される governance packet の一部としてファイルを hash 化してください。

## 1. メタデータ

| Field | Value |
|-------|-------|
| Agreement identifier | `<repo-yyMMdd-XX>` |
| Custodian account id | `<i105...>` |
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

1. *Custody readiness:* "上記の識別子で stage 済み `repo initiate` payload を確認し、sec 2 に記載の SLA の下で collateral を受け入れる準備ができています。"
2. *Rollback commitment:* "インシデントコマンダーの指示があれば、上記の rollback playbook を実行し、`governance/drills/<timestamp>.log` に CLI logs と hashes を提供します。"
3. *Evidence retention:* "acknowledgement、standing instructions、CLI logs を少なくとも `<duration>` 保管し、求めに応じて finance council に提供します。"

以下に署名してください (ガバナンス tracker 経由の電子署名も可)。

| Name | Role | Signature / date |
|------|------|------------------|
| `<custodian ops lead>` | Custodian operator | `<signature>` |
| `<desk lead>` | Desk | `<signature>` |
| `<counterparty>` | Counterparty | `<signature>` |

> 署名後、ファイルを hash 化 (例: `sha256sum custodian_ack_<cust>.md`) し、governance packet の表に digest を記録して、投票時に参照された acknowledgement bytes を reviewer が検証できるようにしてください。
