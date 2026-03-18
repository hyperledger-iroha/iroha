---
id: privacy-metrics-pipeline
lang: uz
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet Privacy Metrics Pipeline (SNNet-8)
sidebar_label: Privacy Metrics Pipeline
description: Privacy-preserving telemetry collection for SoraNet relays and orchestrators.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

# SoraNet Maxfiylik ko'rsatkichlari quvur liniyasi

SNNet-8 o'rni ish vaqti uchun maxfiylikdan xabardor telemetriya sirtini taqdim etadi. The
o'rni endi qo'l siqish va davra hodisalarini bir necha daqiqali chelaklarga va
faqat qo'pol Prometheus hisoblagichlarini eksport qiladi, individual sxemalarni saqlaydi
uzib bo'lmaydi, shu bilan birga operatorlarga harakat qilish mumkin bo'lgan ko'rinish beradi.

## Aggregatorga umumiy nuqtai

- Ish vaqtini amalga oshirish `tools/soranet-relay/src/privacy.rs` sifatida yashaydi
  `PrivacyAggregator`.
- Chelaklar devor soati daqiqasiga qarab belgilanadi (`bucket_secs`, standart 60 soniya) va
  chegaralangan halqada saqlanadi (`max_completed_buckets`, standart 120). Kollektor
  aktsiyalar o'zlarining cheklangan to'siqlarini saqlaydi (`max_share_lag_buckets`, standart 12)
  shuning uchun eskirgan Prio oynalari sizib chiqmasdan emas, balki bostirilgan chelaklar sifatida yuviladi
  xotira yoki tiqilib qolgan kollektorlarni maskalash.
- `RelayConfig::privacy` to'g'ridan-to'g'ri `PrivacyConfig` ga xaritaga tushadi va sozlashni ochib beradi
  tugmalar (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`,
  `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`,
  `expected_shares`). SNNet-8a da ishlab chiqarishning ishlash vaqti standart sozlamalarni saqlaydi
  xavfsiz yig'ish chegaralarini joriy qiladi.
- Runtime modullari voqealarni yoziladigan yordamchilar orqali yozib oladi:
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes` va `record_gar_category`.

## Relay Admin Endpoint

Operatorlar releyning administrator tinglovchisidan xom kuzatishlar uchun so'rov o'tkazishlari mumkin
`GET /privacy/events`. Yakuniy nuqta yangi qator bilan ajratilgan JSON-ni qaytaradi
(`application/x-ndjson`) aks ettirilgan `SoranetPrivacyEventV1` foydali yuklarni o'z ichiga oladi
ichki `PrivacyEventBuffer` dan. Bufer eng yangi voqealarni saqlaydi
`privacy.event_buffer_capacity` yozuvlariga (standart 4096) va to'kiladi
o'qing, shuning uchun qirg'ichlar bo'shliqlarni oldini olish uchun tez-tez so'rov o'tkazishlari kerak. Voqealar qamrab oladi
bir xil qo'l siqish, gaz kelebeği, tasdiqlangan tarmoqli kengligi, faol sxema va GAR signallari
Prometheus hisoblagichlarini quvvatlantiradi, bu esa quyi oqim kollektorlariga arxivlash imkonini beradi
maxfiylik uchun xavfsiz bo'laklar yoki ta'minot xavfsiz yig'ish ish oqimlari.

## O'rni konfiguratsiyasi

Operatorlar o'rni konfiguratsiya faylida maxfiylik telemetriya kadenslarini sozlaydi
`privacy` bo'limi:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Dala standartlari SNNet-8 spetsifikatsiyasiga mos keladi va yuklash vaqtida tasdiqlanadi:

| Maydon | Tavsif | Standart |
|-------|-------------|---------|
| `bucket_secs` | Har bir yig'ish oynasining kengligi (sekundlar). | `60` |
| `min_handshakes` | Bir chelak hisoblagichlarni chiqarishidan oldin minimal hissa qo'shuvchilar soni. | `12` |
| `flush_delay_buckets` | Tugallangan chelaklarni yuvishdan oldin kutish kerak. | `1` |
| `force_flush_buckets` | Biz bostirilgan chelakni chiqarishimizdan oldin maksimal yosh. | `6` |
| `max_completed_buckets` | Saqlangan chelaklar to'plami (cheklanmagan xotirani oldini oladi). | `120` |
| `max_share_lag_buckets` | Bostirishdan oldin kollektor aktsiyalarini saqlash oynasi. | `12` |
| `expected_shares` | Birlashtirishdan oldin kollektor aktsiyalari talab qilinadi. | `2` |
| `event_buffer_capacity` | Administrator oqimi uchun NDJSON voqealari toʻplami. | `4096` |

`force_flush_buckets` sozlamasi `flush_delay_buckets` dan pastroq, nolga teng
chegaralar yoki ushlab turish himoyasini o'chirib qo'yish endi tekshirishni oldini olish uchun muvaffaqiyatsiz tugadi
har bir o'rni telemetriyasini oqizadigan joylashtirishlar.

`event_buffer_capacity` chegarasi `/admin/privacy/events` ni ham chegaralab,
qirg'ichlar cheksiz orqada qolishi mumkin emas.

## Prio kollektor aktsiyalari

SNNet-8a maxfiy umumiy Prio chelaklarini chiqaradigan ikkitomonlama kollektorlarni o'rnatadi. The
orkestrator endi ikkalasi uchun `/privacy/events` NDJSON oqimini tahlil qiladi
`SoranetPrivacyEventV1` yozuvlari va `SoranetPrivacyPrioShareV1` aktsiyalari,
ularni `SoranetSecureAggregator::ingest_prio_share` ga yo'naltirish. Chelaklar chiqaradi
`PrivacyBucketConfig::expected_shares` hissalari kelgandan so'ng, uni aks ettiradi
rele harakati. Ulanishlar chelakni tekislash va gistogramma shakli uchun tasdiqlangan
`SoranetPrivacyBucketMetricsV1` ga birlashtirilishidan oldin. Agar birlashtirilgan bo'lsa
qo'l siqish soni `min_contributors` dan past bo'lsa, chelak eksport qilinadi
`suppressed`, o'rni ichidagi agregatorning harakatini aks ettiradi. Bosilgan
Windows endi operatorlar farqlashi uchun `suppression_reason` yorlig'ini chiqaradi
`insufficient_contributors`, `collector_suppressed`,
`collector_window_elapsed` va `forced_flush_window_elapsed` stsenariylari qachon
telemetriya bo'shliqlarini diagnostika qilish. `collector_window_elapsed` sababi ham yonadi
Prio aktsiyalari `max_share_lag_buckets` dan o'tib ketsa, kollektorlarni tiqilib qoladi
xotirada eskirgan akkumulyatorlarni qoldirmasdan ko'rinadi.

## Torii Yutishning oxirgi nuqtalari

Torii endi ikkita telemetriyaga ega HTTP so'nggi nuqtalarini ochib beradi, shuning uchun o'rni va kollektorlar
Buyurtmali transportni o'rnatmasdan kuzatishlarni yuborishi mumkin:

- `POST /v1/soranet/privacy/event` a qabul qiladi
  `RecordSoranetPrivacyEventDto` foydali yuk. Tanani o'rab oladi a
  `SoranetPrivacyEventV1` va ixtiyoriy `source` yorlig'i. Torii tasdiqlaydi
  faol telemetriya profiliga qarshi so'rov yuboradi, voqeani yozib oladi va javob beradi
  HTTP `202 Accepted` bilan Norito JSON konvertini o'z ichiga olgan
  hisoblangan paqir oynasi (`bucket_start_unix`, `bucket_duration_secs`) va
  o'rni rejimi.
- `POST /v1/soranet/privacy/share` `RecordSoranetPrivacyShareDto` ni qabul qiladi
  foydali yuk. Korpusda `SoranetPrivacyPrioShareV1` va ixtiyoriy bor
  Operatorlar kollektor oqimlarini tekshirishlari uchun `forwarded_by` maslahati. Muvaffaqiyatli
  jo'natmalar HTTP `202 Accepted` bilan Norito JSON konvertini umumlashtiradi
  kollektor, paqir oynasi va bostirish bo'yicha maslahat; tekshirish xatoliklari xaritasi
  deterministik xatolarni qayta ishlashni saqlab qolish uchun telemetriya `Conversion` javobi
  kollektorlar bo'ylab. Orkestratorning voqealar tsikli endi bu ulushlarni o'zi kabi chiqaradi
  Torii Prio akkumulyatorini o'rni chelaklari bilan sinxronlashtirib, so'rov o'tkazgichlari.

Ikkala so'nggi nuqta telemetriya profilini hurmat qiladi: ular `503 xizmatini chiqaradilar
Ko‘rsatkichlar o‘chirilgan bo‘lsa, mavjud emas`. Mijozlar Norito ikkilik faylini yuborishlari mumkin
(`application/x.norito`) yoki Norito JSON (`application/x.norito+json`) jismlari;
server avtomatik ravishda standart Torii orqali formatni muhokama qiladi
ekstraktorlar.

## Prometheus ko'rsatkichlari

Har bir eksport qilingan chelakda `mode` (`entry`, `middle`, `exit`) va
`bucket_start` teglari. Quyidagi metrik oilalar chiqariladi:

| Metrik | Tavsif |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` bilan qoʻl siqish taksonomiyasi. |
| `soranet_privacy_throttles_total{scope}` | `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` bilan gaz kelebeği hisoblagichlari. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Qo‘l siqishlarning umumiy sovutish vaqti. |
| `soranet_privacy_verified_bytes_total` | Ko'r-ko'rona o'lchov dalillaridan tasdiqlangan tarmoqli kengligi. |
| `soranet_privacy_active_circuits_{avg,max}` | Bir chelak uchun o'rtacha va eng yuqori faol davrlar. |
| `soranet_privacy_rtt_millis{percentile}` | RTT foizli taxminlar (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Kategoriya dayjestiga koʻra kalitli boshqaruv boʻyicha harakat hisoboti hisoblagichlari. |
| `soranet_privacy_bucket_suppressed` | Himoyachi chegarasi bajarilmagani uchun chelaklar ushlab qolindi. |
| `soranet_privacy_pending_collectors{mode}` | Kollektor ulushi akkumulyatorlar kombinatsiyasini kutmoqda, o'rni rejimi bo'yicha guruhlangan. |
| `soranet_privacy_suppression_total{reason}` | `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` bilan bosilgan chelak hisoblagichlari, shuning uchun asboblar paneli maxfiylik bo'shliqlarini ko'rsatishi mumkin. |
| `soranet_privacy_snapshot_suppression_ratio` | Oxirgi drenajning bostirilgan/to‘kilgan nisbati (0–1), ogohlantirish byudjetlari uchun foydalidir. |
| `soranet_privacy_last_poll_unixtime` | Eng so'nggi muvaffaqiyatli so'rovning UNIX vaqt tamg'asi (kollektor bo'sh turish ogohlantirishini boshqaradi). |
| `soranet_privacy_collector_enabled` | Maxfiylik kollektori o'chirilgan yoki ishga tushmaganida `0` ga aylanadigan o'lchagich (kollektor o'chirilgan ogohlantirishni boshqaradi). |
| `soranet_privacy_poll_errors_total{provider}` | Relay taxalluslari boʻyicha guruhlangan soʻrovnomadagi nosozliklar (dekodlash xatolari, HTTP xatolari yoki kutilmagan holat kodlari boʻyicha oʻsishlar). |

Kuzatuvsiz chelaklar jim turadi, asboblar panelini tartibsiz saqlaydi
nol bilan to'ldirilgan oynalarni ishlab chiqarish.

## Operatsion qo'llanma

1. **Boshqaruv paneli** – `mode` va `window_start` bo‘yicha guruhlangan yuqoridagi ko‘rsatkichlarni diagramma qiling.
   Kollektor yoki o'rni bilan bog'liq muammolarni yuzaga chiqarish uchun etishmayotgan oynalarni ajratib ko'rsatish. Foydalanish
   `soranet_privacy_suppression_total{reason}` hissa qo'shuvchini farqlash uchun
   bo'shliqlarni tekshirishda kollektor tomonidan boshqariladigan bostirishdan kelib chiqadigan kamchiliklar. Grafana
   aktiv endi ular tomonidan oziqlanadigan maxsus **“Bostirish sabablari (5m)”** panelini yuboradi.
   hisoblagichlar va hisoblaydigan **“Bostirilgan chelak %”** statistikasi
   `sum(soranet_privacy_bucket_suppressed) / count(...)` tanlash uchun shunday
   operatorlar bir qarashda byudjet buzilishini aniqlashlari mumkin. **Kollektor ulushi
   Backlog** seriyasi (`soranet_privacy_pending_collectors`) va **Snapshot
   Bostirish koeffitsienti** stati tiqilib qolgan kollektorlarni va budjet driftini ta'kidlaydi
   avtomatlashtirilgan yugurishlar.
2. **Ogohlantirish** – maxfiylik uchun xavfsiz hisoblagichlardan signallarni haydash: PoW rad etish tezligi,
   sovutish chastotasi, RTT drifti va quvvatni rad etish. Chunki hisoblagichlar
   har bir chelak ichida monotonik, to'g'ridan-to'g'ri tarifga asoslangan qoidalar yaxshi ishlaydi.
3. **Hodisaga javob** – birinchi navbatda jamlangan maʼlumotlarga tayan. Chuqurroq nosozliklarni tuzatishda
   zarur bo'lsa, chelak snapshotlarini takrorlash yoki ko'r-ko'rona tekshirish uchun o'rni so'rang
   xom trafik jurnallarini yig'ish o'rniga o'lchov dalillari.
4. **Ushlab turish** – oshib ketmaslik uchun tez-tez qirib tashlang
   `max_completed_buckets`. Eksportchilar Prometheus chiqishini shunday deb hisoblashlari kerak
   kanonik manba va bir marta yo'naltirilgan mahalliy chelaklarni tashlang.

## Bostirish tahlillari va avtomatlashtirilgan ishga tushirishSNNet-8ni qabul qilish avtomatlashtirilgan kollektorlarning qolishini ko'rsatishga bog'liq
sog'lom va bu bostirish siyosat chegaralarida qoladi (har bir chelakning ≤10%
har qanday 30 daqiqali oyna orqali o'tish). Endi bu eshikni qondirish uchun asboblar kerak edi
daraxt bilan kemalar; operatorlar buni haftalik marosimlariga kiritishlari kerak. Yangi
Grafana bostirish panellari quyidagi PromQL snippetlarini aks ettiradi va qo'ng'iroq vaqtida beradi.
jamoalar qo'lda so'rovlarga qaytishdan oldin jonli ko'rinishga ega.

### Bostirishni ko'rib chiqish uchun PromQL retseptlari

Operatorlar quyidagi PromQL yordamchilarini qulay saqlashlari kerak; ikkalasiga havola qilingan
umumiy Grafana asboblar panelida (`dashboards/grafana/soranet_privacy_metrics.json`)
va Alertmanager qoidalari:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

**“Bostirilgan chelak %”** statistikasi quyida qolishi mumkinligini tasdiqlash uchun nisbat chiqishidan foydalaning
siyosat byudjeti; tez fikr-mulohaza olish uchun boshoq detektorini Alertmanager-ga ulang
hissa qo'shuvchilar soni kutilmaganda pasayganda.

### Oflayn paqir hisoboti CLI

Ish maydoni bir martalik NDJSON uchun `cargo xtask soranet-privacy-report` ni ochib beradi
ushlaydi. Uni bir yoki bir nechta relay administrator eksportiga qarating:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

Yordamchi suratga olishni `SoranetSecureAggregator` orqali uzatadi, a chop etadi
stdout-ga bostirish xulosasi va ixtiyoriy ravishda tuzilgan JSON hisobotini yozadi
`--json-out <path|->` orqali. U jonli kollektor bilan bir xil tugmachalarni hurmat qiladi
(`--bucket-secs`, `--min-contributors`, `--expected-shares` va boshqalar), ijaraga berish
operatorlar triajlashda turli chegaralar ostida tarixiy suratga olishlarni takrorlaydi
masala. JSON-ni Grafana skrinshotlari bilan birga biriktiring, shunda SNNet-8
bostirish tahlillari eshigi tekshirilishi mumkin.

### Birinchi avtomatlashtirilgan nazorat ro'yxati

Boshqaruv hali ham birinchi avtomatlashtirish ishga tushganligini isbotlashni talab qiladi
bostirish byudjeti. Yordamchi endi `--max-suppression-ratio <0-1>` ni qabul qiladi
Bosilgan chelaklar ruxsat etilganidan oshib ketganda, CI yoki operatorlar tezda ishlamay qolishi mumkin
oyna (standart 10%) yoki chelaklar hali mavjud bo'lmaganda. Tavsiya etilgan oqim:

1. NDJSONni o‘rni administratorining so‘nggi nuqtasi(lar)idan hamda orkestratordan eksport qiling
   `/v1/soranet/privacy/event|share` oqimi
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Siyosat byudjeti bilan yordamchini ishga tushiring:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Buyruq kuzatilgan nisbatni chop etadi va byudjet bo'lganda nolga teng bo'lmagan holda chiqadi
   **yoki** dan oshdi, agar chelaklar tayyor bo'lmasa, bu telemetriya tayyor emasligini bildiradi
   hali ishga tushirish uchun ishlab chiqarilgan. Jonli ko'rsatkichlar ko'rsatilishi kerak
   `soranet_privacy_pending_collectors` nolga va
   `soranet_privacy_snapshot_suppression_ratio` bir xil byudjet ostida qoladi
   yugurish bajarilayotganda.
3. Oldin SNNet-8 dalillar to'plami bilan JSON chiqishi va CLI jurnalini arxivlang
   ko'rib chiquvchilar aniq artefaktlarni takrorlashlari uchun transport standartini aylantiring.

## Keyingi qadamlar (SNNet-8a)

- Ikki Prio kollektorlarini integratsiya qiling, ularning ulushini qabul qilish tizimiga ulang
  ish vaqti, shuning uchun o'rni va kollektorlar izchil `SoranetPrivacyBucketMetricsV1` chiqaradi
  foydali yuklar. *(Bajarildi - `ingest_privacy_payload` ga qarang
  `crates/sorafs_orchestrator/src/lib.rs` va qo'shimcha testlar.)*
- Umumiy Prometheus boshqaruv panelini JSON va ogohlantirish qoidalarini e'lon qiling
  bostirish bo'shliqlari, kollektorning sog'lig'i va anonimlik buzilishi. *(Bajarildi - qarang
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml` va tekshirish moslamalari.)*
- da tavsiflangan differensial-maxfiylik kalibrlash artefaktlarini yarating
  `privacy_metrics_dp.md`, shu jumladan takrorlanadigan noutbuklar va boshqaruv
  hazm qiladi. *(Bajarildi — daftar + tomonidan yaratilgan artefaktlar
  `scripts/telemetry/run_privacy_dp.py`; CI o'rami
  `scripts/telemetry/run_privacy_dp_notebook.sh` daftarni orqali amalga oshiradi
  `.github/workflows/release-pipeline.yml` ish jarayoni; boshqaruv dayjesti taqdim etildi
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

Joriy nashr SNNet-8 asosini taqdim etadi: deterministik,
Mavjud Prometheus qirg'ichlariga to'g'ridan-to'g'ri joylashadigan maxfiylik uchun xavfsiz telemetriya
va asboblar paneli. Differentsial maxfiylik kalibrlash artefaktlari joyida, the
chiqarish quvur liniyasi ish jarayoni notebook chiqishlarini yangi, qolganlarini esa saqlaydi
ish birinchi avtomatlashtirilgan ishga tushirishni kuzatish va bostirishni kengaytirishga qaratilgan
ogohlantirish tahlili.