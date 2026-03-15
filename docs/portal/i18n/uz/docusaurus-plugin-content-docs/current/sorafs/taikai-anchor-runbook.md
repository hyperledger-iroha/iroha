---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Taikai Anchor kuzatuv kitobi

Ushbu portal nusxasi kanonik ish kitobini aks ettiradi
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
SoraFS/SoraNet uchun SN13-C marshrutlash-manifest (TRM) langarlarini takrorlashda foydalaning
operatorlar spool artefaktlari, Prometheus telemetriyasi va boshqaruvni o'zaro bog'lashlari mumkin
portalni oldindan ko'rish qurilishidan chiqmasdan dalillar.

## Qo'llanish doirasi va egalari

- **Dastur:** SN13-C — Taikai manifestlari va SoraNS langarlari.
- **Egalari:** Media platformasi WG, DA dasturi, Networking TL, Docs/DevRel.
- **Maqsad:** Sev1/Sev2 ogohlantirishlari, telemetriya uchun deterministik o'yin kitobini taqdim eting
  tekshirish va dalillarni to'plash, Taikai marshrutlash oldinga siljiydi
  taxalluslar bo'ylab.

## Tez boshlash (Sev1/Sev2)

1. **Spool artefaktlarini suratga olish** — eng oxirgi nusxasini oling
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json` va
   `taikai-lineage-*.json` fayllari
   Ishchilarni qayta ishga tushirishdan oldin `config.da_ingest.manifest_store_dir/taikai/`.
2. **Dump `/status` telemetriya** — yozib oling
   Qaysi manifest oynasi ekanligini isbotlash uchun `telemetry.taikai_alias_rotations` massivi
   faol:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **Boshqaruv paneli va ogohlantirishlarni tekshiring** — yuklash
   `dashboards/grafana/taikai_viewer.json` (klaster + oqim filtrlari) va eslatma
   qoidalar mavjudmi
   `dashboards/alerts/taikai_viewer_rules.yml` ishga tushirildi (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, SoraFS sog'liqni saqlash hodisalari).
4. **Inspect Prometheus** — tasdiqlash uchun §“Metrik maʼlumotnoma” boʻlimidagi soʻrovlarni bajaring.
   ingest kechikish/drift va taxallus-rotatsion hisoblagichlar kutilganidek ishlaydi. Eskalatsiya
   agar `taikai_trm_alias_rotations_total` bir nechta derazalar uchun to'xtasa yoki agar
   xato hisoblagichlari ko'payadi.

## Metrik ma'lumotnoma

| Metrik | Maqsad |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | Klaster/oqim uchun CMAF qabul qilish kechikish gistogrammasi (maqsad: p95<750ms, p99<900ms). |
| `taikai_ingest_live_edge_drift_ms` | Kodlovchi va langar ishchilari o'rtasida jonli chekka o'tish (sahifalar p99>1,5 soniyada 10 daqiqa). |
| `taikai_ingest_segment_errors_total{reason}` | Sabablari bo'yicha hisoblagichlar xatosi (`decode`, `manifest_mismatch`, `lineage_replay`, …). Har qanday o'sish `TaikaiIngestFailure` ni keltirib chiqaradi. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | `/v1/da/ingest` taxallus uchun yangi TRM qabul qilganda oshadi; aylanish kadansini tekshirish uchun `rate()` dan foydalaning. |
| `/status → telemetry.taikai_alias_rotations[]` | JSON surati `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total` va dalillar toʻplamlari uchun vaqt belgilari. |
| `taikai_viewer_*` (rebufer, CEK aylanish yoshi, PQ salomatligi, ogohlantirishlar) | CEK aylanishini ta'minlash uchun tomoshabin tomonidagi KPIlar + PQ davrlari langar paytida sog'lom bo'lib qoladi. |

### PromQL parchalari

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

## Boshqaruv paneli va ogohlantirishlar

- **Grafana tomosha qilish paneli:** `dashboards/grafana/taikai_viewer.json` — p95/p99
  kechikish, jonli chekka drift, segment xatolar, CEK aylanish yoshi, tomoshabin ogohlantirishlari.
- **Grafana kesh kartasi:** `dashboards/grafana/taikai_cache.json` — issiq/issiq/sovuq
  taxallus oynalari aylantirilganda reklama aktsiyalari va QoS rad etishlari.
- **Ogohlantirish boshqaruvchisi qoidalari:** `dashboards/alerts/taikai_viewer_rules.yml` — drift
  peyjing, qabul qilishda xatolik haqida ogohlantirishlar, CEK aylanish kechikishi va SoraFS sog'lig'ini tekshirish
  jarimalar/sovutish vaqti. Har bir ishlab chiqarish klasteri uchun qabul qiluvchilar mavjudligiga ishonch hosil qiling.

## Dalillar to'plamining nazorat ro'yxati

- Spool artefaktlari (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- Kutilayotgan/etkazib berilgan konvertlarning imzolangan JSON inventarini chiqarish uchun `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>` dasturini ishga tushiring va soʻrov/SSM/TRM/nasil fayllarini matkap toʻplamiga nusxalash. Standart spool yoʻli `torii.toml` dan `storage/da_manifests/taikai`.
- `telemetry.taikai_alias_rotations` ni qamrab oluvchi `/status` surati.
- Voqea oynasi ustidagi yuqoridagi ko'rsatkichlar uchun Prometheus eksporti (JSON/CSV).
- Filtrlar ko'rinadigan Grafana skrinshotlari.
- Tegishli qoida yong'inlariga havola qiluvchi Alertmanager identifikatorlari.
- tavsiflovchi `docs/examples/taikai_anchor_lineage_packet.md` havolasi
  kanonik dalillar to'plami.

## Boshqaruv panelini aks ettirish va matkap kadansi

SN13-C yo'l xaritasi talabini qondirish Taikai ekanligini isbotlashni anglatadi
tomoshabin/kesh boshqaruv paneli langar **va** portalida aks ettiriladi
dalil mashqlari bashorat qilinadigan kadansda ishlaydi.

1. **Portalni aks ettirish.** `dashboards/grafana/taikai_viewer.json` yoki
   `dashboards/grafana/taikai_cache.json` o'zgarishlar, deltalarni umumlashtiring
   `sorafs/taikai-monitoring-dashboards` (ushbu portal) va JSONga e'tibor bering
   portalning PR tavsifidagi nazorat summalari. Shunday qilib, yangi panellar/eshiklarni ajratib ko'rsatish
   sharhlovchilar boshqariladigan Grafana papkasi bilan bog'lanishi mumkin.
2. **Oylik mashq.**
   - Mashqni har oyning birinchi seshanba kuni soat 15:00 UTC da bajaring
     SN13 boshqaruv sinxronlashdan oldin erlar.
   - Spool artefaktlarini, `/status` telemetriyasini va ichidagi Grafana skrinshotlarini oling.
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - bilan ijroni qayd qiling
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **Ko‘rib chiqing va chop eting.** 48 soat ichida ogohlantirishlar/noto‘g‘ri pozitivlarni ko‘rib chiqing.
   DA dasturi + NetOps, burg'ulash jurnaliga keyingi narsalarni yozib oling va bog'lang
   `docs/source/sorafs/runbooks-index.md` dan boshqaruv paqirini yuklash.

Agar asboblar paneli yoki matkaplar orqada qolsa, SN13-C chiqa olmaydi 🈺; buni saqlang
kadans yoki dalillarni kutish o'zgarganda bo'lim yangilanadi.

## Foydali buyruqlar

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

Taikai qachon bu portal nusxasini kanonik runbook bilan sinxronlashtirib turing
ankraj telemetriyasi, asboblar paneli yoki boshqaruv dalillari talablari o'zgaradi.