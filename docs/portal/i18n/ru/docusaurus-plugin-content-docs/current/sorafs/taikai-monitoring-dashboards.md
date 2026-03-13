---
id: taikai-monitoring-dashboards
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

Готовность Taikai routing-manifest (TRM) зависит от двух Grafana boards и их
сопутствующих alerts. Эта страница отражает ключевые моменты из
`dashboards/grafana/taikai_viewer.json`, `dashboards/grafana/taikai_cache.json` и
`dashboards/alerts/taikai_viewer_rules.yml`, чтобы reviewers могли следить за
прогрессом без клонирования репозитория.

## Viewer dashboard (`taikai_viewer.json`)

- **Live edge и latency:** Панели визуализируют p95/p99 гистограммы задержки
  (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`) по
  cluster/stream. Следите за p99 > 900 ms или drift > 1.5 s (триггерит alert
  `TaikaiLiveEdgeDrift`).
- **Ошибки сегментов:** Разбивает `taikai_ingest_segment_errors_total{reason}`
  чтобы показать decode failures, попытки lineage replay или manifest mismatches.
  Прикладывайте скриншоты к инцидентам SN13-C, когда панель выходит выше
  зоны “warning”.
- **Здоровье viewer и CEK:** Панели на основе метрик `taikai_viewer_*` отслеживают
  возраст ротации CEK, mix PQ guard, rebuffer counts и сводки alert'ов. Панель CEK
  фиксирует SLA ротации, который governance проверяет перед одобрением новых aliases.
- **Снимок телеметрии aliases:** Таблица `/status → telemetry.taikai_alias_rotations`
  находится прямо на board, чтобы операторы подтверждали digests manifest перед
  прикреплением governance evidence.

## Cache dashboard (`taikai_cache.json`)

- **Нагрузка по tier:** Панели строят `sorafs_taikai_cache_{hot,warm,cold}_occupancy`
  и `sorafs_taikai_cache_promotions_total`. Используйте их, чтобы увидеть, не
  перегружает ли ротация TRM отдельные tiers.
- **QoS denials:** `sorafs_taikai_qos_denied_total` всплывает, когда cache pressure
  приводит к throttling; отмечайте drill log всякий раз, когда rate отличается от нуля.
- **Egress utilisation:** Помогает подтвердить, что SoraFS exits успевают за
  Taikai viewers при ротации CMAF окон.

## Alerts и сбор evidence

- Paging rules находятся в `dashboards/alerts/taikai_viewer_rules.yml` и
  совпадают один к одному с панелями выше (`TaikaiLiveEdgeDrift`,
  `TaikaiIngestFailure`, `TaikaiCekRotationLag`, proof-health warnings). Убедитесь,
  что все production clusters подключены к Alertmanager.
- Snapshots/скриншоты, снятые во время drills, должны храниться в
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` рядом со spool files и JSON `/status`.
  Используйте `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`
  чтобы добавить выполнение в общий drill log.
- При изменении dashboard включайте SHA-256 digest JSON файла в описание PR
  портала, чтобы аудиторы могли сопоставить управляемую Grafana папку с версией
  репозитория.

## Checklist evidence bundle

Ревью SN13-C ожидают, что каждый drill или инцидент доставляет один и тот же набор
артефактов из Taikai anchor runbook. Собирайте их в порядке ниже, чтобы bundle был
готов к governance review:

1. Скопируйте самые свежие `taikai-anchor-request-*.json`,
   `taikai-trm-state-*.json` и `taikai-lineage-*.json` из
   `config.da_ingest.manifest_store_dir/taikai/`. Эти spool артефакты подтверждают
   активный routing manifest (TRM) и lineage window. Хелпер
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   скопирует spool files, сгенерирует hashes и при необходимости подпишет summary.
2. Запишите вывод `/v2/status`, отфильтрованный до
   `.telemetry.taikai_alias_rotations[]`, и сохраните его рядом со spool files.
   Reviewers сравнивают `manifest_digest_hex` и границы окна с копией spool state.
3. Экспортируйте Prometheus snapshots для метрик выше и сделайте скриншоты
   viewer/cache dashboards с нужными фильтрами cluster/stream. Поместите raw JSON/CSV
   и скриншоты в папку артефактов.
4. Включите Alertmanager incident IDs (если есть), которые ссылаются на правила из
   `dashboards/alerts/taikai_viewer_rules.yml`, и отметьте, были ли они auto-closed
   после исчезновения условия.

Храните все в `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`, чтобы аудит drills и
governance review SN13-C могли забирать один архив.

## Cadence drills и логирование

- Запускайте Taikai anchor drill в первый вторник каждого месяца в 15:00 UTC.
  График поддерживает свежие evidence перед синхронизацией governance SN13.
- После сбора артефактов добавьте выполнение в общий ledger через
  `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`. Хелпер эмитит
  JSON entry, требуемый `docs/source/sorafs/runbooks-index.md`.
- Ссылайтесь на архивированные артефакты в записи runbook index и эскалируйте
  failed alerts или регрессии dashboard в течение 48 часов через канал Media
  Platform WG/SRE.
- Храните набор скриншотов drill summary (latency, drift, errors, CEK rotation,
  cache pressure) вместе со spool bundle, чтобы операторы могли показать точное
  поведение dashboards во время репетиции.

См. [Taikai Anchor Runbook](./taikai-anchor-runbook.md) для полного Sev 1
процесса и evidence checklist. Эта страница фиксирует только dashboard-специфику,
которая требуется SN13-C перед выходом из 🈺.
