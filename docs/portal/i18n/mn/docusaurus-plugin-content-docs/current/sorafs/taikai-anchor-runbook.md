---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Taikai Anchor Observability Runbook

Энэ портал хуулбар нь каноник runbook-ийг толин тусгадаг
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
SoraFS/SoraNet SN13-C чиглүүлэлтийн манифест (TRM) зангууг давтахдаа үүнийг ашиглана уу.
операторууд дамар олдвор, Prometheus телеметр болон засаглалыг хооронд нь холбож болно
порталын урьдчилан харах бүтцийг орхихгүйгээр нотлох баримт.

## Хамрах хүрээ ба эзэмшигчид

- **Хөтөлбөр:** SN13-C — Taikai манифест ба SoraNS зангуу.
- **Эзэмшигчид:** Media Platform WG, DA Program, Networking TL, Docs/DevRel.
- **Зорилго:** Sev1/Sev2 сэрэмжлүүлэг, телеметрийн тодорхойлогч тоглоомын номоор хангах
  баталгаажуулалт, нотлох баримтуудыг цуглуулах үед Тайкай чиглүүлэлт урагш эргэлддэг
  бусад нэрээр.

## Хурдан эхлэл (Sev1/Sev2)

1. **Дамар олдворуудыг барих** — хамгийн сүүлийнхийг хуулна
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json`, болон
   `taikai-lineage-*.json` файлуудаас
   Ажилчдыг дахин эхлүүлэхийн өмнө `config.da_ingest.manifest_store_dir/taikai/`.
2. **Dump `/status` telemetry** — бичлэг хийх
   Аль манифест цонх болохыг батлах `telemetry.taikai_alias_rotations` массив
   идэвхтэй:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **Хяналтын самбар болон анхааруулгыг шалгах** — ачаалал
   `dashboards/grafana/taikai_viewer.json` (кластер + урсгал шүүлтүүр) болон тэмдэглэл
   ямар нэгэн дүрэм журамд орсон эсэх
   `dashboards/alerts/taikai_viewer_rules.yml` халагдсан (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, SoraFS эрүүл мэндийн баталгааны үйл явдлууд).
4. **Inspect Prometheus** — баталгаажуулахын тулд §“Метрийн лавлагаа” хэсэгт асуулга ажиллуулна уу.
   ingest latency/drift болон alias-rotation тоолуур нь хүлээгдэж буй байдлаар ажиллана. Өсгөх
   хэрэв `taikai_trm_alias_rotations_total` олон цонхны лангуу эсвэл хэрэв
   алдааны тоолуур нэмэгддэг.

## Метрийн лавлагаа

| Метрик | Зорилго |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | Кластер/стрим бүрт CMAF залгих хоцрогдлын гистограмм (зорилтот: p95<750ms, p99<900ms). |
| `taikai_ingest_live_edge_drift_ms` | Кодлогч болон зангууны ажилчдын хоорондох шууд ирмэгийн шилжилт (хуудас p99>1.5 секундын 10 минутын турш). |
| `taikai_ingest_segment_errors_total{reason}` | Шалтгаанаар алдаа тоологч (`decode`, `manifest_mismatch`, `lineage_replay`, …). Аливаа өсөлт нь `TaikaiIngestFailure`-ийг өдөөдөг. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | `/v1/da/ingest` өөр нэрийн шинэ TRM-г хүлээн авах бүрд нэмэгддэг; Эргэлтийн хэмнэлийг баталгаажуулахын тулд `rate()` ашиглана уу. |
| `/status → telemetry.taikai_alias_rotations[]` | `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total`, нотлох баримтын багцын цаг тэмдэг бүхий JSON агшин зураг. |
| `taikai_viewer_*` (буфер, CEK-ийн эргэлтийн нас, PQ эрүүл мэнд, анхааруулга) | CEK эргэлт + PQ хэлхээг зангуу үед эрүүл хэвээр байлгахын тулд үзэгчдийн талын KPI-ууд. |

### PromQL хэсгүүд

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

## Хяналтын самбар ба анхааруулга

- **Grafana үзэгчийн самбар:** `dashboards/grafana/taikai_viewer.json` — p95/p99
  хоцролт, шууд ирмэгийн шилжилт, сегментийн алдаа, CEK-ийн эргэлтийн нас, үзэгчдийн сэрэмжлүүлэг.
- **Grafana кэш самбар:** `dashboards/grafana/taikai_cache.json` — халуун/дулаан/хүйтэн
  нэрийн цонхыг эргүүлэх үед урамшуулал болон QoS татгалзал.
- **Сэрэмжлэгчийн дүрэм:** `dashboards/alerts/taikai_viewer_rules.yml` — дрифт
  пейджинг, залгих алдааны сэрэмжлүүлэг, CEK эргэлтийн хоцрогдол, SoraFS proof-health
  торгууль/хөлжих хугацаа. Үйлдвэрлэлийн кластер бүрт хүлээн авагч байгаа эсэхийг шалгаарай.

## Нотлох баримтын багцыг шалгах хуудас

- Дамрын олдворууд (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>`-г ажиллуулж, хүлээгдэж буй/хүргэсэн дугтуйнуудын гарын үсэг бүхий JSON тооллогыг гаргаж, хүсэлт/SSM/TRM/удам угсааны файлуудыг өрмийн багц руу хуулна. Өгөгдмөл дамар зам нь `torii.toml`-аас `storage/da_manifests/taikai` байна.
- `telemetry.taikai_alias_rotations`-г хамарсан `/status` хормын хувилбар.
- Prometheus нь ослын цонхон дээрх дээрх хэмжигдэхүүнүүдийг (JSON/CSV) экспортлодог.
- Шүүлтүүртэй Grafana дэлгэцийн агшин.
- Холбогдох дүрэмд хамаарах дохиоллын менежерийн ID.
- `docs/examples/taikai_anchor_lineage_packet.md`-г тайлбарласан холбоос
  каноник нотлох баримтын багц.

## Хяналтын самбарын толин тусгал болон өрмийн хэмнэл

SN13-C замын зураглалын шаардлагыг хангана гэдэг нь Тайкай гэдгийг нотолж байна гэсэн үг
үзэгч/кэш хяналтын самбар нь зангууны **болон** портал дотор тусгагдсан байдаг
нотлох өрөмдлөг нь урьдчилан таамаглах хэмнэлээр ажилладаг.

1. **Портал толин тусгал.** `dashboards/grafana/taikai_viewer.json` эсвэл
   `dashboards/grafana/taikai_cache.json` өөрчлөлтүүд, доторх дельтануудыг нэгтгэн дүгнэнэ үү
   `sorafs/taikai-monitoring-dashboards` (энэ портал) болон JSON-г анхаарна уу
   портал PR тайлбар дахь шалгах нийлбэр. Шинэ самбар/босгыг онцлон тэмдэглэ
   Шүүгчид удирдаж буй Grafana хавтастай холбогдох боломжтой.
2. **Сар бүрийн дасгал.**
   - Сургалтыг сар бүрийн эхний Мягмар гарагийн 15:00UTC цагт хийгээрэй
     SN13 засаглалын синхрончлолоос өмнө газарддаг.
   - Дамрын олдвор, `/status` телеметр, Grafana дэлгэцийн агшинг дотор нь авах
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - Гүйцэтгэлийг ашиглан бүртгүүлнэ үү
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **Хянаж, нийтлэх.** 48 цагийн дотор анхааруулга/худал эерэг мэдээллийг
   DA Program + NetOps, дагах зүйлсийг өрөмдлөгийн бүртгэлд тэмдэглэж, холбох
   `docs/source/sorafs/runbooks-index.md`-аас засаглалын хувин байршуулах.

Хэрэв хяналтын самбар эсвэл өрмийн аль нэг нь хоцорч байвал SN13-C гарах боломжгүй 🈺; үүнийг хадгал
Каденц эсвэл нотолгооны хүлээлт өөрчлөгдөх бүрт шинэчлэгдсэн хэсгийг шинэчилнэ.

## Хэрэгтэй тушаалууд

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

Taikai болгонд энэ портал хуулбарыг каноник runbook-тэй синхрончилж байлгаарай
бэхэлгээний телеметр, хяналтын самбар эсвэл засаглалын нотлох баримт шаардлагын өөрчлөлт.