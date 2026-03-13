---
id: observability-plan
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Observability & SLO Plan
sidebar_label: Observability & SLOs
description: Telemetry schema, dashboards, and error-budget policy for SoraFS gateways, nodes, and the multi-source orchestrator.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

## Maqsadlar
- Shlyuzlar, tugunlar va ko'p manbali orkestr uchun o'lchovlar va tuzilgan hodisalarni aniqlang.
- Grafana asboblar paneli, ogohlantirish chegaralari va tekshirish ilgaklarini taqdim eting.
- Xato-byudjet va xaos-mashq siyosatlari bilan bir qatorda SLO maqsadlarini belgilang.

## Metrik katalog

### Gateway sirtlari

| Metrik | Tur | Yorliqlar | Eslatmalar |
|--------|------|--------|-------|
| `sorafs_gateway_active` | O'lchagich (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel` orqali chiqariladi; so'nggi nuqta/usul birikmasi bo'yicha parvozdagi HTTP operatsiyalarini kuzatib boradi. |
| `sorafs_gateway_responses_total` | Hisoblagich | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI000000040X, I18NI0000004100 | Har bir tugallangan shlyuz so'rovi bir marta oshiriladi; `result` ∈ {`success`, `error`, `dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Gistogramma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI000000053X, I18NI0000005400 |05 Shlyuz javoblari uchun birinchi baytgacha kechikish vaqti; Prometheus `_bucket/_sum/_count` sifatida eksport qilinadi. |
| `sorafs_gateway_proof_verifications_total` | Hisoblagich | `profile_version`, `result`, `error_code` | Tasdiqlash natijalari soʻrov vaqtida olingan (`result` ∈ {`success`, `failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Gistogramma | `profile_version`, `result`, `error_code` | PoR tushumlari uchun tekshirish kechikish taqsimoti. |
| `telemetry::sorafs.gateway.request` | Strukturaviy hodisa | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Loki/Tempo korrelyatsiyasi boʻyicha har bir soʻrov yakunida chiqarilgan tizimli jurnal. |

`telemetry::sorafs.gateway.request` hodisalari `endpoint`, `method`, `variant`, `status`, I18NI000000000 va I18NI00000000002 va I18NI0000000000101010102 va I18NI0000000000101010802 va `endpoint`, `endpoint`, I18NI0000000000001 va I18NI00000000001 uchun yuzaki tuzilgan foydali yuklarga ega OTEL hisoblagichlarini aks ettiradi. Loki/Tempo korrelyatsiyasi, asboblar paneli SLO kuzatish uchun OTLP seriyasini iste'mol qiladi.

### Salomatlikni isbotlovchi telemetriya

| Metrik | Tur | Yorliqlar | Eslatmalar |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Hisoblagich | `provider_id`, `trigger`, `penalty` | Har safar `RecordCapacityTelemetry` `SorafsProofHealthAlert` chiqarganda oshadi. `trigger` PDP/PoTR/Ikkala nosozlikni ajratadi, `penalty` esa garov haqiqatan ham sovitish natijasida kesilganmi yoki bostirilganmi yoki yoʻqligini aniqlaydi. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | O'lchagich | `provider_id` | Eng so'nggi PDP/PoTR hisoblari, qoidabuzar telemetriya oynasida xabar qilingan, shuning uchun jamoalar provayderlar siyosatdan qanchalik oshib ketganini aniqlashlari mumkin. |
| `torii_sorafs_proof_health_penalty_nano` | O'lchagich | `provider_id` | Oxirgi ogohlantirishda nano-XOR miqdori qisqartirildi (sovutish muddati amalga oshirilganda nolga teng). |
| `torii_sorafs_proof_health_cooldown` | O'lchagich | `provider_id` | Mantiqiy o'lchagich (`1` = ogohlantirish sovutish vaqti bilan bosiladi) keyingi ogohlantirishlar vaqtincha o'chirilganda paydo bo'ladi. |
| `torii_sorafs_proof_health_window_end_epoch` | O'lchagich | `provider_id` | Operatorlar Norito artefaktlariga nisbatan korrelyatsiya qilishlari uchun ogohlantirish bilan bog'langan telemetriya oynasi uchun qayd etilgan davr. |

Ushbu tasmalar endi Taikai tomoshabin asboblar panelining sog'lig'ini tekshirish qatoriga quvvat beradi
(`dashboards/grafana/taikai_viewer.json`), CDN operatorlariga jonli ko'rinish beradi
ogohlantirish hajmlari, PDP/PoTR trigger aralashmasi, jarimalar va sovutish holatiga
provayder.

Xuddi shu ko'rsatkichlar endi Taikai tomoshabinlari ogohlantirish qoidalarini qo'llab-quvvatlaydi:
`SorafsProofHealthPenalty` har doim yonadi
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` ga oshadi
oxirgi 15 daqiqa, `SorafsProofHealthCooldown` esa, agar
provayder besh daqiqa davomida sovutish rejimida qoladi. Ikkala ogohlantirish ham mavjud
`dashboards/alerts/taikai_viewer_rules.yml`, shuning uchun SRElar darhol kontekstni oladi
PoR/PoTR qoidalari kuchayganda.

### Orkestr yuzalari

| Metrik / Voqealar | Tur | Yorliqlar | Ishlab chiqaruvchi | Eslatmalar |
|----------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | O'lchagich | `manifest_id`, `region` | `FetchMetricsCtx` | Seanslar hozirda parvozda. |
| `sorafs_orchestrator_fetch_duration_ms` | Gistogramma | `manifest_id`, `region` | `FetchMetricsCtx` | Millisekundlarda davomiylik gistogrammasi; 1ms→30s chelaklar. |
| `sorafs_orchestrator_fetch_failures_total` | Hisoblagich | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Sabablari: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Hisoblagich | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Qayta urinish sabablarini ajratadi (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Hisoblagich | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Seans darajasidagi o'chirib qo'yish/muvaffaqiyatsizlik ko'rsatkichlarini yozib oladi. |
| `sorafs_orchestrator_chunk_latency_ms` | Gistogramma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | O'tkazuvchanlik/SLO tahlili uchun har bir bo'lak olish kechikish taqsimoti (ms). |
| `sorafs_orchestrator_bytes_total` | Hisoblagich | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Manifest/provayderga yetkazib berilgan baytlar; PromQL da `rate()` orqali o'tkazish qobiliyatini oling. |
| `sorafs_orchestrator_stalls_total` | Hisoblagich | `manifest_id`, `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` dan ortiq bo'laklarni sanaydi. |
| `telemetry::sorafs.fetch.lifecycle` | Strukturaviy hodisa | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, I18NI0000016010, I18NI0000016010 `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Norito JSON foydali yuki bilan ishning ishlash muddati (boshlash/tugatish) aks ettiriladi. |
| `telemetry::sorafs.fetch.retry` | Strukturaviy hodisa | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Har bir provayderning qayta urinish seriyasi uchun chiqariladi; `attempts` qo'shimcha takroriy urinishlarni hisoblaydi (≥1). |
| `telemetry::sorafs.fetch.provider_failure` | Strukturaviy hodisa | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Provayder muvaffaqiyatsizlik chegarasini kesib o'tganda paydo bo'ladi. |
| `telemetry::sorafs.fetch.error` | Strukturaviy hodisa | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Loki/Splunk yutish uchun qulay bo'lgan terminal nosozlik rekordi. |
| `telemetry::sorafs.fetch.stall` | Strukturaviy hodisa | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Bo'lakning kechikishi sozlangan qopqoqni buzganida ko'tariladi (ko'zgular hisoblagichlarni to'xtatadi). |

### Tugun / replikatsiya yuzalari

| Metrik | Tur | Yorliqlar | Eslatmalar |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Gistogramma | `provider_id` | Xotiradan foydalanish foizining OTEL gistogrammasi (`_bucket/_sum/_count` sifatida eksport qilingan). |
| `sorafs_node_por_success_total` | Hisoblagich | `provider_id` | Muvaffaqiyatli PoR namunalari uchun monotonik hisoblagich, rejalashtiruvchi suratlaridan olingan. |
| `sorafs_node_por_failure_total` | Hisoblagich | `provider_id` | Muvaffaqiyatsiz PoR namunalari uchun monotonik hisoblagich. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | O'lchagich | `provider` | Ishlatilgan baytlar uchun mavjud Prometheus o'lchagichlari, navbat chuqurligi, PoR parvoz soni. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | O'lchagich | `provider` | Imkoniyatlar panelida provayder sig‘imi/ish vaqti muvaffaqiyati haqidagi ma’lumotlar paydo bo‘ldi. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | O'lchagich | `provider`, `manifest` | `/v2/sorafs/por/ingestion/{manifest}` so'rovi o'tkazilganda, "PoR Stalls" paneli/ogohlantirishni ta'minlovchi orqaga qo'yish chuqurligi va yig'ilgan nosozlik hisoblagichlari eksport qilinadi. |

### Ta'mirlash va SLA

| Metrik | Tur | Yorliqlar | Eslatmalar |
|--------|------|--------|-------|
| `sorafs_repair_tasks_total` | Hisoblagich | `status` | Ta'mirlash vazifasini o'tkazish uchun OTEL hisoblagichi. |
| `sorafs_repair_latency_minutes` | Gistogramma | `outcome` | OTEL gistogrammasining ta'mirlash muddati kechikishi. |
| `sorafs_repair_queue_depth` | Gistogramma | `provider` | Har bir provayder uchun navbatda turgan vazifalarning OTEL gistogrammasi (snapshot uslubi). |
| `sorafs_repair_backlog_oldest_age_seconds` | Gistogramma | — | Navbatdagi eng eski vazifa yoshining OTEL gistogrammasi (sekundlar). |
| `sorafs_repair_lease_expired_total` | Hisoblagich | `outcome` | Ijara muddati tugashi uchun OTEL hisoblagichi (`requeued`/`escalated`). |
| `sorafs_repair_slash_proposals_total` | Hisoblagich | `outcome` | Slash taklifiga o'tish uchun OTEL hisoblagichi. |
| `torii_sorafs_repair_tasks_total` | Hisoblagich | `status` | Vazifaga o'tish uchun Prometheus hisoblagichi. |
| `torii_sorafs_repair_latency_minutes_bucket` | Gistogramma | `outcome` | Ta'mirlash muddati kechikishi uchun Prometheus gistogrammasi. |
| `torii_sorafs_repair_queue_depth` | O'lchagich | `provider` | Har bir provayder uchun navbatda turgan vazifalar uchun Prometheus o'lchagich. |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | O'lchagich | — | Navbatdagi eng eski vazifa yoshi uchun Prometheus o'lchagich (sekundlar). |
| `torii_sorafs_repair_lease_expired_total` | Hisoblagich | `outcome` | Prometheus ijara muddati tugashi uchun hisoblagich. |
| `torii_sorafs_slash_proposals_total` | Hisoblagich | `outcome` | Slash taklifiga o'tish uchun Prometheus hisoblagichi. |

Boshqaruv auditi JSON metamaʼlumotlari taʼmirlash telemetriya belgilarini aks ettiradi (`status`, `ticket_id`, `manifest`, taʼmirlash hodisalari boʻyicha `provider`; `outcome` slash koʻrsatkichlari va slash korrespondentlari boʻyicha audit). deterministik tarzda.

### Saqlash va GC| Metrik | Tur | Yorliqlar | Eslatmalar |
|--------|------|--------|-------|
| `sorafs_gc_runs_total` | Hisoblagich | `result` | O'rnatilgan tugun tomonidan chiqarilgan GC tozalash uchun OTEL hisoblagichi. |
| `sorafs_gc_evictions_total` | Hisoblagich | `reason` | OTEL hisoblagichi sabab bo'yicha guruhlangan ko'chirilgan manifestlar uchun. |
| `sorafs_gc_bytes_freed_total` | Hisoblagich | `reason` | Bo'shatilgan baytlar uchun OTEL hisoblagichi sabab bo'yicha guruhlangan. |
| `sorafs_gc_blocked_total` | Hisoblagich | `reason` | Faol ta'mirlash yoki siyosat bilan bloklangan ko'chirishlar uchun OTEL hisoblagichi. |
| `torii_sorafs_gc_runs_total` | Hisoblagich | `result` | GC tozalash uchun Prometheus hisoblagichi (muvaffaqiyat/xato). |
| `torii_sorafs_gc_evictions_total` | Hisoblagich | `reason` | Prometheus sabablarga ko'ra guruhlangan ko'chirilgan manifestlar uchun hisoblagich. |
| `torii_sorafs_gc_bytes_freed_total` | Hisoblagich | `reason` | Prometheus sabab bo'yicha guruhlangan bo'shatilgan baytlar uchun hisoblagich. |
| `torii_sorafs_gc_blocked_total` | Hisoblagich | `reason` | Prometheus sabablarga ko'ra guruhlangan bloklangan ko'chirishlar uchun hisoblagich. |
| `torii_sorafs_gc_expired_manifests` | O'lchagich | — | GC tekshiruvlari tomonidan kuzatilgan muddati oʻtgan manifestlarning joriy soni. |
| `torii_sorafs_gc_oldest_expired_age_seconds` | O'lchagich | — | Eng qadimgi muddati o'tgan manifestning soniyalarida yoshi (ushlab turish inoyatidan keyin). |

### Kelishuv

| Metrik | Tur | Yorliqlar | Eslatmalar |
|--------|------|--------|-------|
| `sorafs.reconciliation.runs_total` | Hisoblagich | `result` | OTEL hisoblagichi yarashuv suratlari uchun. |
| `sorafs.reconciliation.divergence_total` | Hisoblagich | — | OTEL har bir yugurishdagi farqlar hisoblagichi. |
| `torii_sorafs_reconciliation_runs_total` | Hisoblagich | `result` | Kelishuvlar uchun Prometheus hisoblagichi. |
| `torii_sorafs_reconciliation_divergence_count` | O'lchagich | — | Kelishuv hisobotida kuzatilgan so'nggi farqlar soni. |

### O'z vaqtida olish isboti (PoTR) va SLA bo'laklari

| Metrik | Tur | Yorliqlar | Ishlab chiqaruvchi | Eslatmalar |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Gistogramma | `tier`, `provider` | PoTR koordinatori | Millisekundlarda oxirgi muddatning sustligi (ijobiy = bajarildi). |
| `sorafs_potr_failures_total` | Hisoblagich | `tier`, `provider`, `reason` | PoTR koordinatori | Sabablari: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Hisoblagich | `provider`, `manifest_id`, `reason` | SLA monitor | Bo'lak yetkazib berish SLO (kechikish, muvaffaqiyat darajasi) o'tkazib yuborilganda ishga tushiriladi. |
| `sorafs_chunk_sla_violation_active` | O'lchagich | `provider`, `manifest_id` | SLA monitor | Mantiqiy o'lchagich (0/1) faol buzilish oynasida almashtirildi. |

## SLO maqsadlari

- Gateway ishonchsiz mavjudligi: **99,9%** (HTTP 2xx/304 javoblar).
- Ishonchsiz TTFB P95: issiq daraja ≤120ms, issiq daraja ≤300ms.
- Muvaffaqiyatni isbotlash darajasi: kuniga ≥99,5%.
- Orkestrning muvaffaqiyati (bo'lakni to'ldirish): ≥99%.

## Boshqaruv paneli va ogohlantirish

1. **Gateway Observability** (`dashboards/grafana/sorafs_gateway_observability.json`) — OTEL koʻrsatkichlari orqali ishonchsiz mavjudlik, TTFB P95, rad etish buzilishi va PoR/PoTR nosozliklarini kuzatib boradi.
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`) — ko‘p manbali yuklanish, qayta urinishlar, provayderning nosozliklari va to‘xtab qolish holatlarini qamrab oladi.
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`) — `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` va I18NI00000002 orqali anonimlashtirilgan reley paqirlari, bostirish oynalari va kollektor holatini diagrammalaydi.
4. **Capacity Health** (`dashboards/grafana/sorafs_capacity_health.json`) — provayder boʻsh joyini hamda taʼmirlash SLA koʻtarilishini, provayder tomonidan taʼmirlash navbatining chuqurligini va GC tozalash/evakuatsiya/baytlar boʻshatilgan/bloklangan sabablar/muddati oʻtgan-manifest yoshi va yarashuv ajralish suratlarini kuzatib boradi.

Ogohlantirishlar to'plami:

- `dashboards/alerts/sorafs_gateway_rules.yml` - shlyuz mavjudligi, TTFB, nosozlikni isbotlash.
- `dashboards/alerts/sorafs_fetch_rules.yml` — orkestrdagi nosozliklar/qayta urinishlar/to'xtashlar; `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` va `dashboards/alerts/tests/soranet_policy_rules.test.yml` orqali tasdiqlangan.
- `dashboards/alerts/sorafs_capacity_rules.yml` - sig'im bosimi va ta'mirlash SLA/ortiqcha ro'yxat/lizing muddati tugashi haqida ogohlantirishlar va saqlashni tekshirish uchun GC to'xtab turish/bloklangan/xato haqida ogohlantirishlar.
- `dashboards/alerts/soranet_privacy_rules.yml` — maxfiylik darajasini pasaytirish, bostirish signallari, kollektorning boʻsh turishini aniqlash va oʻchirilgan kollektor ogohlantirishlari (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` - `sorafs_orchestrator_brownouts_total` ga ulangan anonimlik signallari.
- `dashboards/alerts/taikai_viewer_rules.yml` - Taikai tomoshabinining drift/ingest/CEK kechikish signallari hamda `torii_sorafs_proof_health_*` tomonidan quvvatlangan yangi SoraFS sog'lig'ini isbotlovchi jarima/sovutish haqida ogohlantirishlar.

## Kuzatuv strategiyasi

- OpenTelemetry-ni oxirigacha qabul qiling:
  - Shlyuzlar so'rov identifikatorlari, manifest dayjestlari va token xeshlari bilan izohlangan OTLP oraliqlarini (HTTP) chiqaradi.
  - Orkestrator olish urinishlari uchun oraliqlarni eksport qilish uchun `tracing` + `opentelemetry` dan foydalanadi.
  - PoR muammolari va saqlash operatsiyalari uchun o'rnatilgan SoraFS tugunlarining eksport oralig'i. Barcha komponentlar `x-sorafs-trace` orqali tarqaladigan umumiy iz identifikatoriga ega.
- `SorafsFetchOtel` orkestr ko'rsatkichlarini OTLP gistogrammalariga o'tkazadi, `telemetry::sorafs.fetch.*` hodisalari log-markazli backendlar uchun engil JSON foydali yuklarini ta'minlaydi.
- Kollektorlar: OTEL kollektorlarini Prometheus/Loki/Tempo bilan birga boshqaring (Tempo afzal). Jaeger API eksportchilari ixtiyoriy bo'lib qoladi.
- Yuqori kardinallik operatsiyalaridan namuna olish kerak (muvaffaqiyat yo'llari uchun 10%, muvaffaqiyatsizliklar uchun 100%).

## TLS telemetriya muvofiqlashtirish (SF-5b)

- Metrik hizalama:
  - TLS avtomatlashtirish kemalari `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` va `sorafs_gateway_tls_ech_enabled`.
  - Ushbu o'lchagichlarni TLS/Sertifikatlar paneli ostidagi Gateway Overview boshqaruv paneliga qo'shing.
- Ogohlantirish aloqasi:
  - TLS amal qilish muddati tugashi haqida ogohlantirishlar (≤14 kun qolgan) SLO ishonchsiz mavjudligi bilan bog'liq.
  - ECH o'chirilishi TLS va mavjudlik panellariga havola qiluvchi ikkinchi darajali ogohlantirishni chiqaradi.
- Quvur liniyasi: TLS avtomatlashtirish ishi shlyuz ko'rsatkichlari bilan bir xil Prometheus stekiga eksport qiladi; SF-5b bilan muvofiqlashtirish birlashtirilgan asboblarni ta'minlaydi.

## Metrik nomlash va yorliq qoidalari

- Metrik nomlar Torii va shlyuz tomonidan ishlatiladigan mavjud `torii_sorafs_*` yoki `sorafs_*` prefikslariga mos keladi.
- Yorliqlar to'plami standartlashtirilgan:
  - `result` → HTTP natijasi (`success`, `refused`, `failed`).
  - `reason` → rad etish/xato kodi (`unsupported_chunker`, `timeout` va boshqalar).
  - `status` → ta'mirlash vazifasi holati (`queued`, `in_progress`, `completed`, `failed`, `escalated`).
  - `outcome` → taʼmirlash ijarasi yoki kechikish natijasi (`requeued`, `escalated`, `completed`, `failed`).
  - `provider` → olti burchakli kodlangan provayder identifikatori.
  - `manifest` → kanonik manifest dayjesti (yuqori kardinallik bilan kesilgan).
  - `tier` → deklarativ darajadagi teglar (`hot`, `warm`, `archive`).
- Telemetriya emissiya nuqtalari:
  - Gateway ko'rsatkichlari `torii_sorafs_*` ostida ishlaydi va `crates/iroha_core/src/telemetry.rs` konventsiyalarini qayta ishlatadi.
  - Orkestrator manifest dayjesti, ish identifikatori, mintaqa va provayder identifikatorlari bilan belgilangan `sorafs_orchestrator_*` ko'rsatkichlarini va `telemetry::sorafs.fetch.*` hodisalarini (hayot tsikli, qayta urinish, provayder xatosi, xatolik, to'xtab turish) chiqaradi.
  - Tugunlar yuzasi `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` va `torii_sorafs_por_*`.
- Umumiy Prometheus nomlash hujjatida metrik katalogni, shu jumladan yorliqning asosiy kutilmalarini (provayder/yuqori chegaralarni ko'rsatadi) ro'yxatdan o'tkazish uchun Observability bilan muvofiqlashtiring.

## Ma'lumot uzatish liniyasi

- Kollektorlar OTLP ni Prometheus (metrikalar) va Loki/Tempo (jurnallar/izlar) ga eksport qilib, har bir komponent bilan birga joylashadilar.
- Ixtiyoriy eBPF (Tetragon) shlyuzlar/tugunlar uchun past darajadagi kuzatuvni boyitadi.
- Torii va o'rnatilgan tugunlar uchun `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` dan foydalaning; orkestr `install_sorafs_fetch_otlp_exporter` qo'ng'iroq qilishda davom etmoqda.

## Tasdiqlash ilgaklari

- Prometheus ogohlantirish qoidalari toʻxtab turish koʻrsatkichlari va maxfiylikni bostirish tekshiruvlari bilan bloklangan holatda qolishini taʼminlash uchun CI vaqtida `scripts/telemetry/test_sorafs_fetch_alerts.sh` ni ishga tushiring.
- Grafana asboblar panelini versiya nazorati ostida saqlang (`dashboards/grafana/`) va panellar o'zgarganda skrinshotlarni/havolalarni yangilang.
- Chaos `scripts/telemetry/log_sorafs_drill.sh` orqali jurnal natijalarini mashq qiladi; validation leverages `scripts/telemetry/validate_drill_log.sh` ([Operations Playbook](operations-playbook.md) ga qarang).