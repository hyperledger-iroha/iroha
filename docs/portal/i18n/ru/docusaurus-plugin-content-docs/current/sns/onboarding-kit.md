---
lang: ru
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Метрики SNS и onboarding kit

Пункт дорожной карты **SN-8** включает два обещания:

1. Опубликовать dashboards, которые показывают регистрации, продления, ARPU, споры и окна freeze для `.sora`, `.nexus` и `.dao`.
2. Выпустить onboarding kit, чтобы registrars и stewards могли единообразно подключать DNS, pricing и APIs до запуска любого суффикса.

Эта страница отражает исходную версию
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
чтобы внешние ревьюеры следовали той же процедуре.

## 1. Пакет метрик

### Dashboard Grafana и portal embed

- Импортируйте `dashboards/grafana/sns_suffix_analytics.json` в Grafana (или другой analytics host)
  через стандартный API:

```bash
curl -H "Content-Type: application/json"          -H "Authorization: Bearer ${GRAFANA_TOKEN}"          -X POST https://grafana.sora.net/api/dashboards/db          --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- Тот же JSON используется в iframe на странице портала (см. **SNS KPI Dashboard**).
  При каждом обновлении dashboard выполняйте
  `npm run build && npm run serve-verified-preview` в `docs/portal`,
  чтобы подтвердить синхронизацию Grafana и embed.

### Панели и evidence

| Панель | Метрики | Evidence governance |
|-------|---------|---------------------|
| Регистрации и продления | `sns_registrar_status_total` (success + renewal resolver labels) | Throughput по суффиксу + SLA трекинг. |
| ARPU / net units | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Finance сопоставляет manifests registrar с доходом. |
| Споры и freezes | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Показывает активные freezes, темп арбитража и нагрузку guardian. |
| SLA / error rates | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | Подсвечивает регрессии API до влияния на клиентов. |
| Bulk manifest tracker | `sns_bulk_release_manifest_total`, payment метрики с labels `manifest_id` | Связывает CSV drops с settlement тикетами. |

Экспортируйте PDF/CSV из Grafana (или embed iframe) во время ежемесячного KPI review
и приложите к соответствующей записи annex в
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Stewards также фиксируют SHA-256
экспортированного bundle в `docs/source/sns/reports/` (например,
`steward_scorecard_2026q1.md`), чтобы аудиты могли воспроизвести evidence path.

### Автоматизация annex

Генерируйте файлы annex напрямую из экспорта dashboard, чтобы reviewers получали
единообразный дайджест:

```bash
cargo xtask sns-annex       --suffix .sora       --cycle 2026-03       --dashboard dashboards/grafana/sns_suffix_analytics.json       --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json       --output docs/source/sns/reports/.sora/2026-03.md       --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md       --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- Helper хеширует экспорт, фиксирует UID/tags/panel count и пишет Markdown annex в
  `docs/source/sns/reports/.<suffix>/<cycle>.md` (см. пример `.sora/2026-03`).
- `--dashboard-artifact` копирует экспорт в
  `artifacts/sns/regulatory/<suffix>/<cycle>/` чтобы annex ссылался на канонический путь;
  используйте `--dashboard-label` только если нужен внешний архив.
- `--regulatory-entry` указывает на regulatory memo. Helper вставляет (или заменяет)
  блок `KPI Dashboard Annex`, который фиксирует путь annex, artefact dashboard,
  digest и timestamp для синхронизации evidence.
- `--portal-entry` синхронизирует Docusaurus копию
  (`docs/portal/docs/sns/regulatory/*.md`), чтобы reviewers не сравнивали
  отдельные annex summaries вручную.
- Если пропустить `--regulatory-entry`/`--portal-entry`, приложите файл вручную и
  все равно загрузите PDF/CSV snapshots из Grafana.
- Для регулярных экспортов перечислите пары suffix/cycle в
  `docs/source/sns/regulatory/annex_jobs.json` и запустите
  `python3 scripts/run_sns_annex_jobs.py --verbose`. Helper проходит по каждой записи,
  копирует экспорт dashboard (по умолчанию `dashboards/grafana/sns_suffix_analytics.json`)
  и обновляет annex блок в каждом regulatory memo (и, при наличии, portal memo) за один проход.
- Выполните `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (или `make check-sns-annex`), чтобы подтвердить сортировку/дедупликацию списка, наличие маркера `sns-annex` в каждом memo и существование annex stub. Helper пишет `artifacts/sns/annex_schedule_summary.json` рядом с locale/hash summary для governance пакетов.
Это убирает ручной copy/paste и сохраняет согласованность evidence SN-8, защищая от drift расписания, маркеров и локализации в CI.

## 2. Компоненты onboarding kit

### Suffix wiring

- Registry schema + selector rules:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  и [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- DNS skeleton helper:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  и rehearsal flow в
  [gateway/DNS runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- Для каждого registrar launch добавляйте короткую заметку в
  `docs/source/sns/reports/` с примерами selectors, GAR proofs и DNS hashes.

### Pricing cheatsheet

| Длина label | Base fee (USD equiv) |
|--------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6-9 | $12 |
| 10+ | $8 |

Suffix coefficients: `.sora` = 1.0x, `.nexus` = 0.8x, `.dao` = 1.3x.  
Term multipliers: 2-year -5%, 5-year -12%; grace window = 30 days, redemption
= 60 days (20% fee, min $5, max $200). Фиксируйте согласованные отклонения в
registrar ticket.

### Premium auctions vs renewals

1. **Premium pool** -- sealed-bid commit/reveal (SN-3). Отслеживайте bids через
   `sns_premium_commit_total` и публикуйте manifest в
   `docs/source/sns/reports/`.
2. **Dutch reopen** -- после истечения grace + redemption запускайте 7-day Dutch sale
   с 10x, уменьшающейся на 15% в день. Помечайте manifests `manifest_id`, чтобы
   dashboard показывал прогресс.
3. **Renewals** -- мониторьте `sns_registrar_status_total{resolver="renewal"}` и
   фиксируйте autorenew checklist (notifications, SLA, fallback payment rails)
   в registrar ticket.

### Developer APIs и automation

- API contracts: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Bulk helper и CSV schema:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- Пример команды:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv       --ndjson artifacts/sns/releases/2026q2/requests.ndjson       --submission-log artifacts/sns/releases/2026q2/submissions.log       --submit-torii-url https://torii.sora.net       --submit-token-file ~/.config/sora/tokens/registrar.token
```

Включайте manifest ID (вывод `--submission-log`) в KPI dashboard filter,
чтобы finance могла reconciliar revenue panels по release.

### Evidence bundle

1. Registrar ticket с контактами, scope suffix и payment rails.
2. DNS/resolver evidence (zonefile skeletons + GAR proofs).
3. Pricing worksheet + governance-approved overrides.
4. API/CLI smoke-test artefacts (`curl` samples, CLI transcripts).
5. KPI dashboard screenshot + CSV export, приложенные к monthly annex.

## 3. Launch checklist

| Step | Owner | Artefact |
|------|-------|----------|
| Dashboard imported | Product Analytics | Grafana API response + dashboard UID |
| Portal embed validated | Docs/DevRel | `npm run build` logs + preview screenshot |
| DNS rehearsal complete | Networking/Ops | `sns_zonefile_skeleton.py` outputs + runbook log |
| Registrar automation dry run | Registrar Eng | `sns_bulk_onboard.py` submissions log |
| Governance evidence filed | Governance Council | Annex link + SHA-256 of exported dashboard |

Завершите checklist перед активацией registrar или suffix. Подписанный
bundle закрывает gate SN-8 и дает аудиторам единый reference при
проверке запусков marketplace.
