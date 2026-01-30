---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: testnet-rollout
title: Роллаут testnet SoraNet (SNNet-10)
sidebar_label: Роллаут testnet (SNNet-10)
description: Поэтапный план активации, onboarding kit и telemetry gates для повышений SoraNet testnet.
---

:::note Канонический источник
Эта страница зеркалирует план rollout SNNet-10 в `docs/source/soranet/testnet_rollout_plan.md`. Держите обе копии синхронизированными, пока наследуемые docs не будут выведены из эксплуатации.
:::

SNNet-10 координирует поэтапную активацию anonymity overlay SoraNet по всей сети. Используйте этот план, чтобы перевести roadmap bullet в конкретные deliverables, runbooks и telemetry gates, чтобы каждый operator понимал ожидания до того, как SoraNet станет транспортом по умолчанию.

## Фазы запуска

| Фаза | Таймлайн (цель) | Объем | Обязательные artefacts |
|-------|-------------------|-------|--------------------|
| **T0 - Closed Testnet** | Q4 2026 | 20-50 relays на >=3 ASNs, управляемых core contributors. | Testnet onboarding kit, guard pinning smoke suite, baseline latency + PoW metrics, brownout drill log. |
| **T1 - Public Beta** | Q1 2027 | >=100 relays, guard rotation включен, exit bonding обязателен, SDK betas по умолчанию используют SoraNet с `anon-guard-pq`. | Обновленный onboarding kit, operator verification checklist, directory publishing SOP, telemetry dashboard pack, incident rehearsal reports. |
| **T2 - Mainnet Default** | Q2 2027 (с гейтом на завершение SNNet-6/7/9) | Production сеть по умолчанию SoraNet; включены obfs/MASQUE transports и enforcement PQ ratchet. | Протоколы approval governance, direct-only rollback procedure, downgrade alarms, подписанный success metrics report. |

**Пути пропуска нет** - каждая фаза обязана доставить telemetry и governance artefacts предыдущей стадии до повышения.

## Testnet onboarding kit

Каждый relay operator получает детерминированный пакет со следующими файлами:

| Artefact | Описание |
|----------|-------------|
| `01-readme.md` | Обзор, контакты и таймлайн. |
| `02-checklist.md` | Pre-flight checklist (hardware, network reachability, guard policy verification). |
| `03-config-example.toml` | Минимальная конфигурация relay + orchestrator SoraNet, согласованная с compliance blocks SNNet-9, включая блок `guard_directory`, который pin-ит hash последнего guard snapshot. |
| `04-telemetry.md` | Инструкции по подключению SoraNet privacy metrics dashboards и alert thresholds. |
| `05-incident-playbook.md` | Процедура реакции на brownout/downgrade с матрицей эскалации. |
| `06-verification-report.md` | Шаблон, который оператор заполняет и возвращает после прохождения smoke tests. |

Отрендеренная копия лежит в `docs/examples/soranet_testnet_operator_kit/`. Каждое повышение обновляет kit; номера версий следуют фазе (например, `testnet-kit-vT0.1`).

Для операторов public-beta (T1) краткий onboarding brief в `docs/source/soranet/snnet10_beta_onboarding.md` суммирует prerequisites, telemetry deliverables и workflow подачи, указывая на детерминированный kit и validator helpers.

`cargo xtask soranet-testnet-feed` генерирует JSON feed, который агрегирует promotion window, relay roster, metrics report, drill evidence и attachment hashes, на которые ссылается stage-gate template. Сначала подпишите drill logs и attachments через `cargo xtask soranet-testnet-drill-bundle`, чтобы feed зафиксировал `drill_log.signed = true`.

## Метрики успеха

Переход между фазами гейтится по следующей telemetry, собранной минимум две недели:

- `soranet_privacy_circuit_events_total`: 95% circuits завершаются без brownout или downgrade; оставшиеся 5% ограничены supply PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: <1% fetch sessions в день триггерят brownout вне запланированных drills.
- `soranet_privacy_gar_reports_total`: вариативность в пределах +/-10% от ожидаемого микса GAR категорий; всплески должны быть объяснены утвержденными policy updates.
- Успешность PoW tickets: >=99% в целевом окне 3 s; репортится через `soranet_privacy_throttles_total{scope="congestion"}`.
- Latency (95th percentile) по регионам: <200 ms после полного построения circuits, фиксируется через `soranet_privacy_rtt_millis{percentile="p95"}`.

Dashboard и alert templates находятся в `dashboard_templates/` и `alert_templates/`; отзеркальте их в вашем telemetry repository и добавьте в CI lint checks. Используйте `cargo xtask soranet-testnet-metrics` для генерации governance-ориентированного отчета перед запросом повышения.

Stage-gate submissions должны следовать `docs/source/soranet/snnet10_stage_gate_template.md`, который ссылается на готовую Markdown форму в `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Verification checklist

Операторы должны подписаться под следующим перед входом в каждую фазу:

- ✅ Relay advert подписан текущим admission envelope.
- ✅ Guard rotation smoke test (`tools/soranet-relay --check-rotation`) проходит.
- ✅ `guard_directory` указывает на последний artefact `GuardDirectorySnapshotV2` и `expected_directory_hash_hex` совпадает с committee digest (при старте relay логирует проверенный hash).
- ✅ Метрики PQ ratchet (`sorafs_orchestrator_pq_ratio`) держатся выше целевых порогов для запрошенной стадии.
- ✅ GAR compliance config совпадает с последним tag (см. каталог SNNet-9).
- ✅ Симуляция downgrade alarm (отключить collectors, ожидать alert в течение 5 мин).
- ✅ PoW/DoS drill выполнен с документированными шагами mitigation.

Предзаполненный шаблон включен в onboarding kit. Операторы отправляют завершенный отчет в governance helpdesk перед получением production credentials.

## Governance и отчетность

- **Change control:** promotions требуют approval Governance Council, зафиксированный в council minutes и приложенный к status page.
- **Status digest:** публиковать еженедельные обновления с количеством relays, PQ ratio, инцидентами brownout и открытыми action items (хранится в `docs/source/status/soranet_testnet_digest.md` после старта каденции).
- **Rollbacks:** поддерживать подписанный rollback plan, возвращающий сеть на предыдущую фазу за 30 минут, включая DNS/guard cache invalidation и client communication templates.

## Supporting assets

- `cargo xtask soranet-testnet-kit [--out <dir>]` материализует onboarding kit из `xtask/templates/soranet_testnet/` в целевой каталог (по умолчанию `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` оценивает SNNet-10 success metrics и выдает структурированный pass/fail отчет для governance reviews. Примерный snapshot лежит в `docs/examples/soranet_testnet_metrics_sample.json`.
- Grafana и Alertmanager templates находятся в `dashboard_templates/soranet_testnet_overview.json` и `alert_templates/soranet_testnet_rules.yml`; скопируйте их в telemetry repository или подключите к CI lint checks.
- Шаблон downgrade communication для SDK/portal messaging находится в `docs/source/soranet/templates/downgrade_communication_template.md`.
- Еженедельные status digests должны использовать `docs/source/status/soranet_testnet_weekly_digest.md` как каноническую форму.

Pull requests должны обновлять эту страницу вместе с любыми изменениями artefacts или telemetry, чтобы rollout plan оставался каноническим.
