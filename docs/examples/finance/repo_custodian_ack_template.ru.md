---
lang: ru
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6609b9628186b58c0441d1ca1594a3030b7f7d81bb2df9c36af3a9d65cdd963d
source_last_modified: "2025-11-15T20:04:53.140162+00:00"
translation_last_reviewed: 2026-01-01
---

# Шаблон подтверждения custodians repo

Используйте этот шаблон, когда repo (двусторонний или tri-party) ссылается на custodians через `RepoAgreement::custodian`. Цель — зафиксировать SLA custody, routing accounts и контакты для drill до перемещения активов. Скопируйте шаблон в каталог доказательств (например
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`), заполните placeholders и захешируйте файл как часть governance packet, описанного в
`docs/source/finance/repo_ops.md` sec 2.8.

## 1. Метаданные

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

1. *Custody readiness:* "Мы проверили staged payload `repo initiate` с указанными идентификаторами и готовы принять collateral в рамках SLA, указанного в sec 2."
2. *Rollback commitment:* "Мы выполним указанный выше rollback playbook по указанию incident commander и предоставим CLI logs и hashes в `governance/drills/<timestamp>.log`."
3. *Evidence retention:* "Мы сохраним acknowledgement, standing instructions и CLI logs как минимум на `<duration>` и предоставим их финансовому совету по запросу."

Подпишите ниже (электронные подписи допустимы при маршрутизации через governance tracker).

| Name | Role | Signature / date |
|------|------|------------------|
| `<custodian ops lead>` | Custodian operator | `<signature>` |
| `<desk lead>` | Desk | `<signature>` |
| `<counterparty>` | Counterparty | `<signature>` |

> После подписи захешируйте файл (пример: `sha256sum custodian_ack_<cust>.md`) и запишите digest в таблицу governance packet, чтобы reviewers могли проверить acknowledgement bytes, использованные при голосовании.
