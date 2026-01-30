---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Трекер приглашений preview

Этот трекер фиксирует каждую волну preview портала документации, чтобы владельцы DOCS-SORA и ревьюеры governance могли видеть активную когорту, кто утвердил приглашения и какие артефакты еще требуют внимания. Обновляйте его при отправке, отзыве или переносе приглашений, чтобы аудитный след оставался в репозитории.

## Статус волн

| Волна | Когорта | Issue трекера | Approver(s) | Статус | Целевое окно | Примечания |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Core maintainers** | Maintainers Docs + SDK, валидирующие checksum flow | `DOCS-SORA-Preview-W0` (GitHub/ops tracker) | Lead Docs/DevRel + Portal TL | Завершено | Q2 2025 недели 1-2 | Приглашения отправлены 2025-03-25, телеметрия была зеленой, итоговый summary опубликован 2025-04-08. |
| **W1 - Partners** | Операторы SoraFS, интеграторы Torii под NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + Governance liaison | Завершено | Q2 2025 неделя 3 | Приглашения 2025-04-12 -> 2025-04-26, все восемь партнеров подтвердили; evidence в [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) и digest выхода в [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Community** | Кураторский community waitlist (<=25 за раз) | `DOCS-SORA-Preview-W2` | Lead Docs/DevRel + Community manager | Завершено | Q3 2025 неделя 1 (tentative) | Приглашения 2025-06-15 -> 2025-06-29, телеметрия зеленая весь период; evidence + findings в [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Beta cohorts** | Finance/observability beta + SDK partner + ecosystem advocate | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + Governance liaison | Завершено | Q1 2026 неделя 8 | Приглашения 2026-02-18 -> 2026-02-28; digest + portal data через волну `preview-20260218` (см. [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Примечание: связывайте каждую issue трекера с соответствующими preview request tickets и архивируйте их в проекте `docs-portal-preview`, чтобы approvals оставались доступными.

## Активные задачи (W0)

- Обновлены preflight артефакты (запуск GitHub Actions `docs-portal-preview` 2025-03-24, descriptor проверен через `scripts/preview_verify.sh` с тегом `preview-2025-03-24`).
- Зафиксированы baseline телеметрии (`docs.preview.integrity`, snapshot `TryItProxyErrors` сохранен в issue W0).
- Outreach текст зафиксирован через [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) с preview tag `preview-2025-03-24`.
- Записаны intake запросы для первых пяти maintainers (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Первые пять приглашений отправлены 2025-03-25 10:00-10:20 UTC после семи дней зеленой телеметрии; acknowledgements сохранены в `DOCS-SORA-Preview-W0`.
- Мониторинг телеметрии + office hours для хоста (ежедневные check-ins до 2025-03-31; checkpoint log ниже).
- Собраны midpoint feedback / issues и отмечены `docs-preview/w0` (см. [W0 digest](./preview-feedback/w0/summary.md)).
- Опубликован summary волны + подтверждения выхода (exit bundle 2025-04-08; см. [W0 digest](./preview-feedback/w0/summary.md)).
- W3 beta wave отслежена; будущие волны планируются после governance review.

## Резюме волны W1 partners

- Legal & governance approvals. Addendum partners подписан 2025-04-05; approvals загружены в `DOCS-SORA-Preview-W1`.
- Telemetry + Try it staging. Change ticket `OPS-TRYIT-147` выполнен 2025-04-06 с Grafana snapshots `docs.preview.integrity`, `TryItProxyErrors`, и `DocsPortal/GatewayRefusals`.
- Artefact + checksum prep. Bundle `preview-2025-04-12` проверен; logs descriptor/checksum/probe сохранены в `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Invite roster + dispatch. Все восемь partner requests (`DOCS-SORA-Preview-REQ-P01...P08`) одобрены; приглашения отправлены 2025-04-12 15:00-15:21 UTC, acknowledgements зафиксированы по ревьюеру.
- Feedback instrumentation. Ежедневные office hours + checkpoints телеметрии; см. [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md).
- Final roster/exit log. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) теперь фиксирует timestamps invite/ack, telemetry evidence, quiz exports и pointers артефактов на 2025-04-26, чтобы governance могла воспроизвести волну.

## Invite log - W0 core maintainers

| Reviewer ID | Role | Request ticket | Invite sent (UTC) | Expected exit (UTC) | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Portal maintainer | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Active | Подтвердил verification checksum; фокус на nav/sidebar review. |
| sdk-rust-01 | Rust SDK lead | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Active | Тестирует SDK recipes + Norito quickstarts. |
| sdk-js-01 | JS SDK maintainer | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Active | Валидирует Try it console + ISO flows. |
| sorafs-ops-01 | SoraFS operator liaison | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Active | Аудитит SoraFS runbooks + orchestration docs. |
| observability-01 | Observability TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Active | Ревьюит telemetry/incident appendices; отвечает за Alertmanager coverage. |

Все приглашения ссылаются на один и тот же `docs-portal-preview` artefact (run 2025-03-24, tag `preview-2025-03-24`) и transcript проверки в `DOCS-SORA-Preview-W0`. Любые добавления/паузы нужно фиксировать и в таблице выше, и в issue трекера перед переходом к следующей волне.

## Checkpoint log - W0

| Date (UTC) | Activity | Notes |
| --- | --- | --- |
| 2025-03-26 | Baseline telemetry review + office hours | `docs.preview.integrity` + `TryItProxyErrors` оставались зелеными; office hours подтвердили завершение checksum verification. |
| 2025-03-27 | Midpoint feedback digest posted | Summary сохранен в [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); две minor nav issues отмечены `docs-preview/w0`, инцидентов нет. |
| 2025-03-31 | Final week telemetry spot check | Последние office hours перед выходом; ревьюеры подтвердили оставшиеся задачи, алертов не было. |
| 2025-04-08 | Exit summary + invite closures | Подтверждены завершенные обзоры, временный доступ отозван, findings архивированы в [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker обновлен перед W1. |

## Invite log - W1 partners

| Reviewer ID | Role | Request ticket | Invite sent (UTC) | Expected exit (UTC) | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | SoraFS operator (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Completed | Orchestrator ops feedback delivered 2025-04-20; exit ack 15:05 UTC. |
| sorafs-op-02 | SoraFS operator (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Completed | Rollout comments logged in `docs-preview/w1`; exit ack 15:10 UTC. |
| sorafs-op-03 | SoraFS operator (US) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Completed | Dispute/blacklist edits filed; exit ack 15:12 UTC. |
| torii-int-01 | Torii integrator | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Completed | Try it auth walkthrough accepted; exit ack 15:14 UTC. |
| torii-int-02 | Torii integrator | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Completed | RPC/OAuth doc comments logged; exit ack 15:16 UTC. |
| sdk-partner-01 | SDK partner (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Completed | Preview integrity feedback merged; exit ack 15:18 UTC. |
| sdk-partner-02 | SDK partner (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Completed | Telemetry/redaction review done; exit ack 15:22 UTC. |
| gateway-ops-01 | Gateway operator | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Completed | Gateway DNS runbook comments filed; exit ack 15:24 UTC. |

## Checkpoint log - W1

| Date (UTC) | Activity | Notes |
| --- | --- | --- |
| 2025-04-12 | Invite dispatch + artefact verification | All eight partners emailed with `preview-2025-04-12` descriptor/archive; acknowledgements stored in tracker. |
| 2025-04-13 | Telemetry baseline review | `docs.preview.integrity`, `TryItProxyErrors`, and `DocsPortal/GatewayRefusals` dashboards green; office hours confirmed checksum verification completed. |
| 2025-04-18 | Mid-wave office hours | `docs.preview.integrity` remained green; two doc nits logged under `docs-preview/w1` (nav wording + Try it screenshot). |
| 2025-04-22 | Final telemetry spot check | Proxy + dashboards healthy; no new issues raised, noted in tracker ahead of exit. |
| 2025-04-26 | Exit summary + invite closures | All partners confirmed review completion, invites revoked, evidence archived in [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## W3 beta cohort recap

- Invites sent 2026-02-18 with checksum verification + acknowledgements logged the same day.
- Feedback collected under `docs-preview/20260218` with governance issue `DOCS-SORA-Preview-20260218`; digest + summary generated via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Access revoked 2026-02-28 after the final telemetry check; tracker + portal tables updated to show W3 as completed.

## Invite log - W2 community

| Reviewer ID | Role | Request ticket | Invite sent (UTC) | Expected exit (UTC) | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Community reviewer (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Completed | Ack 16:06 UTC; focusing on SDK quickstarts; exit confirmed 2025-06-29. |
| comm-vol-02 | Community reviewer (Governance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Completed | Governance/SNS review done; exit confirmed 2025-06-29. |
| comm-vol-03 | Community reviewer (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Completed | Norito walkthrough feedback logged; exit ack 2025-06-29. |
| comm-vol-04 | Community reviewer (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Completed | SoraFS runbook review done; exit ack 2025-06-29. |
| comm-vol-05 | Community reviewer (Accessibility) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Completed | Accessibility/UX notes shared; exit ack 2025-06-29. |
| comm-vol-06 | Community reviewer (Localization) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Completed | Localization feedback logged; exit ack 2025-06-29. |
| comm-vol-07 | Community reviewer (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Completed | Mobile SDK doc checks delivered; exit ack 2025-06-29. |
| comm-vol-08 | Community reviewer (Observability) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Completed | Observability appendix review done; exit ack 2025-06-29. |

## Checkpoint log - W2

| Date (UTC) | Activity | Notes |
| --- | --- | --- |
| 2025-06-15 | Invite dispatch + artefact verification | `preview-2025-06-15` descriptor/archive shared with 8 community reviewers; acknowledgements stored in tracker. |
| 2025-06-16 | Telemetry baseline review | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` dashboards green; Try it proxy logs show community tokens active. |
| 2025-06-18 | Office hours & issue triage | Collected two suggestions (`docs-preview/w2 #1` tooltip wording, `#2` localization sidebar) - both routed to Docs. |
| 2025-06-21 | Telemetry check + doc fixes | Docs addressed `docs-preview/w2 #1/#2`; dashboards still green, no incidents. |
| 2025-06-24 | Final week office hours | Reviewers confirmed remaining feedback submissions; no alert fire. |
| 2025-06-29 | Exit summary + invite closures | Acks recorded, preview access revoked, telemetry snapshots + artefacts archived (see [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Office hours & issue triage | Two documentation suggestions logged under `docs-preview/w1`; no incidents or alerts triggered. |

## Reporting hooks

- Each Wednesday, update the tracker table above plus the active invite issue with a short status note (invites sent, active reviewers, incidents).
- When a wave closes, append the feedback summary path (for example, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) and link it from `status.md`.
- If any pause criteria from the [preview invite flow](./preview-invite-flow.md) trigger, add the remediation steps here before resuming invites.
