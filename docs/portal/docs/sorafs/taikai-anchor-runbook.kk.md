---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 50261b1f3173cd3916b29c81e85cc92ed8c14c38a0e0296be38397fe9b5c0596
source_last_modified: "2025-12-29T18:16:35.204852+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Taikai Anchor Бақылау кітабы

Бұл портал көшірмесі канондық жұмыс кітабын көрсетеді
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
Оны SoraFS/SoraNet үшін SN13-C маршруттық манифест (TRM) анкерлерін қайталау кезінде пайдаланыңыз.
операторлар спул артефактілерін, Prometheus телеметриясын және басқаруды корреляциялай алады
порталды алдын ала қарау құрастыруынан шықпай дәлелдеңіз.

## Ауқым және иелері

- **Бағдарлама:** SN13-C — Taikai манифесттері және SoraNS анкерлері.
- **Иелері:** Media Platform WG, DA Program, Networking TL, Docs/DevRel.
- **Мақсат:** Sev1/Sev2 ескертулері, телеметрия үшін детерминирленген ойын кітапшасын қамтамасыз ету
  валидация және Taikai маршрутизациясы алға жылжыған кезде дәлелдерді түсіру
  бүркеншік аттар арқылы.

## Жылдам бастау (Sev1/Sev2)

1. **Спуль артефактілерін түсіру** — соңғысын көшіріңіз
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json`, және
   `taikai-lineage-*.json` файлдары
   Жұмысшыларды қайта іске қоспас бұрын `config.da_ingest.manifest_store_dir/taikai/`.
2. **Dump `/status` телеметрия** — жазу
   `telemetry.taikai_alias_rotations` массиві қай манифест терезесі екенін дәлелдеу үшін
   белсенді:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **Бақылау тақталары мен ескертулерді тексеру** — жүктеу
   `dashboards/grafana/taikai_viewer.json` (кластер + ағын сүзгілері) және ескертпе
   қандай да бір ережелер бар ма
   `dashboards/alerts/taikai_viewer_rules.yml` іске қосылды (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, SoraFS денсаулықты тексеру оқиғалары).
4. **Inspect Prometheus** — растау үшін §«Метрика сілтемесі» ішіндегі сұрауларды іске қосыңыз
   кідіріс/дрейфті қабылдау және бүркеншік аттың айналу есептегіштері күтілгендей әрекет етеді. Көтеру
   `taikai_trm_alias_rotations_total` бірнеше терезелер үшін тоқтаса немесе егер
   қате есептегіштері көбейеді.

## Метрика сілтемесі

| метрикалық | Мақсаты |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | Кластерге/ағынға CMAF қабылдау кешігу гистограммасы (мақсат: p95<750ms, p99<900ms). |
| `taikai_ingest_live_edge_drift_ms` | Кодер мен якорь жұмысшылары арасындағы тікелей жиектердің ауытқуы (10 минут ішінде p99>1,5 с беттерінде). |
| `taikai_ingest_segment_errors_total{reason}` | Себеп бойынша қате есептегіштері (`decode`, `manifest_mismatch`, `lineage_replay`, …). Кез келген өсу `TaikaiIngestFailure` іске қосылады. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | `/v1/da/ingest` бүркеншік ат үшін жаңа TRM қабылдаған сайын өседі; айналу каденциясын тексеру үшін `rate()` пайдаланыңыз. |
| `/status → telemetry.taikai_alias_rotations[]` | `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total` және дәлелдер жинақтарына арналған уақыт белгілері бар JSON суреті. |
| `taikai_viewer_*` (ребуфер, CEK айналу жасы, PQ денсаулығы, ескертулер) | Зәкірлер кезінде CEK айналуын және PQ тізбектерінің сау болып қалуын қамтамасыз ету үшін қараушы жағындағы KPI көрсеткіштері. |

### PromQL үзінділері

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

## Бақылау тақталары және ескертулер

- **Grafana қарау тақтасы:** `dashboards/grafana/taikai_viewer.json` — p95/p99
  кідіріс, тікелей жиектен ауытқу, сегмент қателері, CEK айналу жасы, қараушы ескертулері.
- **Grafana кэш тақтасы:** `dashboards/grafana/taikai_cache.json` — ыстық/жылы/суық
  бүркеншік атын терезелер айналдырғанда, жылжытулар және QoS бас тартулары.
- **Alertmanager ережелері:** `dashboards/alerts/taikai_viewer_rules.yml` — дрейф
  пейджинг, қабылдау сәтсіздігі туралы ескертулер, CEK айналу кешігуі және SoraFS денсаулықты тексеру
  айыппұлдар/салқындату. Әрбір өндірістік кластер үшін қабылдағыштардың болуын қамтамасыз етіңіз.

## Дәлелдер топтамасының бақылау тізімі

- Спуль артефактілері (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- Күтудегі/жеткізілген хатқалталардың қол қойылған JSON түгендеуін шығару және сұрау/SSM/TRM/линия файлдарын бұрғылау бумасына көшіру үшін `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>` іске қосыңыз. Әдепкі катушка жолы `torii.toml` бастап `storage/da_manifests/taikai` болып табылады.
- `telemetry.taikai_alias_rotations` қамтитын `/status` суреті.
- Оқиға терезесінің үстіндегі көрсеткіштер үшін Prometheus экспорттары (JSON/CSV).
- Сүзгілері көрінетін Grafana скриншоттары.
- Тиісті ереже өрттеріне сілтеме жасайтын Alertmanager идентификаторлары.
- сипаттайтын `docs/examples/taikai_anchor_lineage_packet.md` сілтемесі
  канондық дәлелдер пакеті.

## Бақылау тақтасының шағылыстыруы және бұрғылау каденциясы

SN13-C жол картасының талаптарын қанағаттандыру Taikai екенін дәлелдеуді білдіреді
қарау құралы/кэш бақылау тақталары порталдың ішінде **және** якорьде көрсетіледі
дәлелді жаттығулар болжамды каденцияда орындалады.

1. **Порталды көшіру.** Кез келген уақытта `dashboards/grafana/taikai_viewer.json` немесе
   `dashboards/grafana/taikai_cache.json` өзгереді, дельталарды қорытындылаңыз
   `sorafs/taikai-monitoring-dashboards` (осы портал) және JSON-ға назар аударыңыз
   порталдың PR сипаттамасындағы бақылау сомасы. Жаңа панельдерді/шектілерді бөлектеңіз
   шолушылар басқарылатын Grafana қалтасымен байланыстыра алады.
2. **Ай сайынғы жаттығу.**
   - Дәлел ретінде жаттығуды әр айдың бірінші сейсенбісінде 15:00UTC-те орындаңыз
     SN13 басқару синхрондауына дейін жерлер.
   - Спуль артефактілерін, `/status` телеметриясын және ішіндегі Grafana скриншоттарын түсіріңіз.
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - Орындауды тіркеңіз
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **Қарау және жариялау.** 48 сағат ішінде ескертулерді/жалған позитивтерді қарап шығыңыз.
   DA Program + NetOps, бұрғылау журналына кейінгі элементтерді жазып, байланыстырыңыз
   `docs/source/sorafs/runbooks-index.md` ішінен басқару шелегін жүктеп салу.

Бақылау тақталары немесе бұрғылар артта қалса, SN13-C шыға алмайды 🈺; осыны сақта
бөлім каденс немесе дәлел күтулері өзгерген сайын жаңартылады.

## Пайдалы пәрмендер

```bash
# Snapshot alias rotation telemetry to an artefact directory
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# List spool entries for a specific alias/event
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Inspect TRM mismatch reasons from the spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Taikai кез келген уақытта осы портал көшірмесін канондық жұмыс кітабымен синхрондаңыз
анкерлік телеметрия, бақылау тақталары немесе басқару дәлелдері талаптары өзгереді.