---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/gateway-dns-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: be60ee4d682b2c6ac72bff579521a82ec0c70e2ae7e4e67bf3f08cd86cf41f8c
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Ранбук запуска Gateway и DNS SoraFS

Эта копия в портале отражает канонический ранбук в
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Она фиксирует операционные guardrails для воркстрима Decentralized DNS & Gateway,
чтобы лиды по networking, ops и документации могли отрепетировать стек
автоматизации перед kickoff 2025-03.

## Область и deliverables

- Связать вехи DNS (SF-4) и gateway (SF-5) через репетицию детерминированной
  деривации хостов, релизов каталога resolvers, автоматизации TLS/GAR и
  сбора доказательств.
- Держать kickoff-артефакты (agenda, invite, attendance tracker, snapshot
  телеметрии GAR) синхронизированными с последними назначениями owners.
- Подготовить аудируемый bundle артефактов для governance reviewers: release notes
  каталога resolvers, логи gateway probes, вывод conformance harness и сводку
  Docs/DevRel.

## Роли и ответственности

| Workstream | Ответственности | Требуемые артефакты |
|------------|-----------------|---------------------|
| Networking TL (DNS stack) | Поддерживать детерминированный план хостов, запускать RAD directory releases, публиковать входы телеметрии resolvers. | `artifacts/soradns_directory/<ts>/`, diffs для `docs/source/soradns/deterministic_hosts.md`, RAD metadata. |
| Ops Automation Lead (gateway) | Выполнять drills автоматизации TLS/ECH/GAR, запускать `sorafs-gateway-probe`, обновлять hooks PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, probe JSON, записи в `ops/drill-log.md`. |
| QA Guild & Tooling WG | Запускать `ci/check_sorafs_gateway_conformance.sh`, курировать fixtures, архивировать Norito self-cert bundles. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | Фиксировать minutes, обновлять design pre-read + appendices, публиковать evidence summary в портале. | Обновленные `docs/source/sorafs_gateway_dns_design_*.md` и rollout notes. |

## Входы и prerequisites

- Спецификация детерминированных хостов (`docs/source/soradns/deterministic_hosts.md`) и
  каркас attestation для resolvers (`docs/source/soradns/resolver_attestation_directory.md`).
- Артефакты gateway: operator handbook, TLS/ECH automation helpers,
  guidance по direct-mode и self-cert workflow под `docs/source/sorafs_gateway_*`.
- Tooling: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh` и CI helpers
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secrets: ключ релиза GAR, DNS/TLS ACME credentials, routing key PagerDuty,
  Torii auth token для fetches resolvers.

## Pre-flight checklist

1. Подтвердите участников и agenda, обновив
   `docs/source/sorafs_gateway_dns_design_attendance.md` и разослав текущую
   agenda (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Подготовьте корни артефактов, например
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` и
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Обновите fixtures (GAR manifests, RAD proofs, bundles conformance gateway) и
   убедитесь, что состояние `git submodule` соответствует последнему rehearsal tag.
4. Проверьте secrets (Ed25519 release key, ACME account file, PagerDuty token) и
   соответствие checksums в vault.
5. Проведите smoke-test telemetry targets (Pushgateway endpoint, GAR Grafana board)
   перед drill.

## Шаги репетиции автоматизации

### Детерминированная карта хостов и release каталога RAD

1. Запустите helper детерминированной деривации хостов на предложенном наборе
   manifests и подтвердите отсутствие drift относительно
   `docs/source/soradns/deterministic_hosts.md`.
2. Сгенерируйте bundle каталога resolvers:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Зафиксируйте напечатанный ID каталога, SHA-256 и выходные пути внутри
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` и в minutes kickoff.

### Захват телеметрии DNS

- Tail resolver transparency logs в течение ≥10 минут с
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Экспортируйте метрики Pushgateway и архивируйте NDJSON snapshots рядом с
  директориями run ID.

### Drills автоматизации gateway

1. Выполните TLS/ECH probe:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Запустите conformance harness (`ci/check_sorafs_gateway_conformance.sh`) и
   self-cert helper (`scripts/sorafs_gateway_self_cert.sh`) для обновления
   Norito attestation bundle.
3. Зафиксируйте PagerDuty/Webhook события, чтобы подтвердить end-to-end работу
   пути автоматизации.

### Упаковка доказательств

- Обновите `ops/drill-log.md` с timestamps, участниками и hashes probes.
- Сохраните артефакты в директориях run ID и опубликуйте executive summary
  в minutes Docs/DevRel.
- Сошлитесь на evidence bundle в governance тикете до kickoff review.

## Модерация сессии и передача доказательств

- **Timeline модератора:**
  - T-24 h — Program Management публикует напоминание + snapshot agenda/attendance в `#nexus-steering`.
  - T-2 h — Networking TL обновляет snapshot телеметрии GAR и фиксирует deltas в `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation проверяет готовность probes и записывает активный run ID в `artifacts/sorafs_gateway_dns/current`.
  - Во время звонка — Модератор делится этим ранбуком и назначает live scribe; Docs/DevRel фиксируют action items по ходу.
- **Шаблон minutes:** Скопируйте скелет из
  `docs/source/sorafs_gateway_dns_design_minutes.md` (также отражен в portal bundle)
  и коммитьте заполненный экземпляр на каждую сессию. Включите список участников,
  решения, action items, hashes evidence и открытые риски.
- **Загрузка доказательств:** Заархивируйте `runbook_bundle/` из rehearsal,
  приложите отрендеренный PDF minutes, запишите SHA-256 hashes в minutes + agenda,
  затем уведомите governance reviewer alias после загрузки в
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Снимок доказательств (kickoff марта 2025)

Последние rehearsal/live артефакты, упомянутые в roadmap и minutes, лежат в bucket
`s3://sora-governance/sorafs/gateway_dns/`. Хэши ниже отражают канонический
manifest (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Dry run — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball bundle: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Minutes PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Live workshop — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Pending upload: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel добавит SHA-256 после попадания PDF в bundle.)_

## Связанные материалы

- [Operations playbook для gateway](./operations-playbook.md)
- [План наблюдаемости SoraFS](./observability-plan.md)
- [Трекер децентрализованного DNS и gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)
