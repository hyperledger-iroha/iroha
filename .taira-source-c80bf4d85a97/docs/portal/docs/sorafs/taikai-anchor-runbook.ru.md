---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 50261b1f3173cd3916b29c81e85cc92ed8c14c38a0e0296be38397fe9b5c0596
source_last_modified: "2025-11-21T18:08:23.480735+00:00"
translation_last_reviewed: 2026-01-30
---

# Ранбук наблюдаемости анкора Taikai

Эта копия в портале отражает канонический ранбук в
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
Используйте её при репетициях анкеров SN13-C routing-manifest (TRM), чтобы
операторы SoraFS/SoraNet могли сопоставлять spool-артефакты, телеметрию Prometheus
и governance-доказательства, не выходя из portal preview.

## Область и владельцы

- **Программа:** SN13-C — Taikai manifests и SoraNS anchors.
- **Владельцы:** Media Platform WG, DA Program, Networking TL, Docs/DevRel.
- **Цель:** Обеспечить детерминированный playbook для алертов Sev 1/Sev 2,
  валидации телеметрии и сбора доказательств, пока Taikai routing manifests
  последовательно продвигаются по alias.

## Quickstart (Sev 1/Sev 2)

1. **Зафиксируйте spool-артефакты** — скопируйте последние файлы
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json` и
   `taikai-lineage-*.json` из
   `config.da_ingest.manifest_store_dir/taikai/` перед перезапуском воркеров.
2. **Снимите `/status` телеметрию** — сохраните массив
   `telemetry.taikai_alias_rotations`, чтобы подтвердить активное окно манифеста:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **Проверьте dashboards и алерты** — загрузите
   `dashboards/grafana/taikai_viewer.json` (фильтры cluster + stream) и отметьте,
   сработали ли правила в
   `dashboards/alerts/taikai_viewer_rules.yml` (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, события PoR health SoraFS).
4. **Проверьте Prometheus** — выполните запросы из раздела "Metric reference",
   чтобы подтвердить корректность ingest latency/drift и счётчиков rotation.
   Эскалируйте, если `taikai_trm_alias_rotations_total` замирает на нескольких
   окнах или растут счётчики ошибок.

## Справочник метрик

| Метрика | Назначение |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | Гистограмма CMAF ingest latency по cluster/stream (цель: p95 < 750 ms, p99 < 900 ms). |
| `taikai_ingest_live_edge_drift_ms` | Live-edge drift между encoder и anchor воркерами (paging при p99 > 1.5 s в течение 10 мин). |
| `taikai_ingest_segment_errors_total{reason}` | Счётчики ошибок по причине (`decode`, `manifest_mismatch`, `lineage_replay`, …). Любой рост триггерит `TaikaiIngestFailure`. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | Увеличивается при каждом принятии нового TRM через `/v1/da/ingest`; используйте `rate()` для проверки каденции ротации. |
| `/status → telemetry.taikai_alias_rotations[]` | JSON snapshot с `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total` и timestamps для evidence bundles. |
| `taikai_viewer_*` (rebuffer, возраст CEK rotation, PQ health, alerts) | KPI viewer-стороны, подтверждающие здоровье CEK rotation + PQ circuits во время anchors. |

### PromQL snippets

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## Дашборды и алерты

- **Grafana viewer board:** `dashboards/grafana/taikai_viewer.json` — p95/p99 latency,
  live-edge drift, segment errors, возраст CEK rotation, viewer alerts.
- **Grafana cache board:** `dashboards/grafana/taikai_cache.json` — hot/warm/cold
  promotions и QoS denials при смене alias windows.
- **Alertmanager rules:** `dashboards/alerts/taikai_viewer_rules.yml` — paging по drift,
  предупреждения ingest failure, lag CEK rotation и PoR health penalties/cooldowns
  для SoraFS. Убедитесь, что receivers настроены для каждого production cluster.

## Checklist evidence bundle

- Spool-артефакты (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- Запустите `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>`
  чтобы сформировать подписанный JSON-инвентарь pending/delivered envelopes и
  скопировать файлы request/SSM/TRM/lineage в bundle drill. Путь spool по умолчанию —
  `storage/da_manifests/taikai` из `torii.toml`.
- Snapshot `/status` с `telemetry.taikai_alias_rotations`.
- Экспорт Prometheus (JSON/CSV) для метрик выше на окне инцидента.
- Скриншоты Grafana с видимыми фильтрами.
- ID Alertmanager, относящиеся к соответствующим срабатываниям.
- Ссылка на `docs/examples/taikai_anchor_lineage_packet.md`, описывающую канонический
  evidence packet.

## Зеркалирование дашбордов и cadence drill

Чтобы выполнить требование SN13-C, нужно доказать, что дашборды Taikai viewer/cache
отражены в портале **и** что drill evidence для anchoring выполняется с
предсказуемой периодичностью.

1. **Portal mirroring.** При изменении `dashboards/grafana/taikai_viewer.json` или
   `dashboards/grafana/taikai_cache.json` суммируйте изменения в
   `sorafs/taikai-monitoring-dashboards` (этот портал) и фиксируйте JSON checksums
   в описании PR портала. Выделяйте новые панели/пороги, чтобы reviewers могли
   сопоставить их с управляемой папкой Grafana.
2. **Ежемесячный drill.**
   - Запускайте drill в первый вторник каждого месяца в 15:00 UTC, чтобы evidence
     попадали до SN13 governance sync.
   - Сохраняйте spool-артефакты, `/status` телеметрию и Grafana screenshots в
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - Логируйте выполнение через
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **Review & publish.** В течение 48 часов проверяйте alerts/false positives с
   DA Program + NetOps, фиксируйте follow-up в drill log и добавляйте ссылку на
   загрузку governance bucket из `docs/source/sorafs/runbooks-index.md`.

Если dashboards или drills отстают, SN13-C не может выйти из 🈺; держите этот
раздел актуальным при изменении cadence или требований к evidence.

## Полезные команды

```bash
# Snapshot телеметрии ротации alias в каталог артефактов
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# Список spool-entries для конкретного alias/event
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Просмотр причин TRM mismatch из spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Держите этот портал-экземпляр синхронизированным с каноническим ранбуком, когда
меняются Taikai anchoring telemetry, dashboards или governance evidence требования.
