---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8e34d60198b5809cc1a609ccfb27687357b6814acefebbb1f29f328416c16c05
source_last_modified: "2026-07-10T10:11:25+00:00"
translation_last_reviewed: 2026-02-07
id: node-plan
title: SoraFS Node Implementation Plan
sidebar_label: Node Implementation Plan
description: Translate the SF-3 storage roadmap into actionable engineering work with milestones, tasks, and test coverage.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

SF-3 Iroha/Torii jarayonini SoraFS saqlash provayderiga aylantiradigan birinchi ishga tushiriladigan `sorafs-node` qutisini taqdim etadi. Yetkazib berishlarni ketma-ketlashtirishda ushbu rejadan [tugunni saqlash boʻyicha yoʻriqnoma](node-storage.md), [provayderga kirish siyosati](provider-admission-policy.md) va [saqlash hajmi bozori yoʻl xaritasi](storage-capacity-marketplace.md) bilan birga foydalaning.

## Maqsad doirasi (M1 bosqich)

1. **Bo‘lak do‘kon integratsiyasi.** `sorafs_car::ChunkStore`-ni konfiguratsiya qilingan ma’lumotlar katalogida parcha baytlari, manifestlar va PoR daraxtlarini saqlaydigan doimiy backend bilan o‘rab oling.
2. **Gateway so‘nggi nuqtalari.** Torii jarayonida PIN-kodlarni yuborish, bo‘laklarni olish, PoR namunalarini olish va saqlash telemetriyasi uchun Norito HTTP so‘nggi nuqtalarini ko‘rsating.
3. **Santexnika konfiguratsiyasi.** `SoraFsStorage` konfiguratsiya strukturasini (yoqilgan bayroq, sig‘im, kataloglar, parallellik chegaralari) `iroha_config`, `iroha_core` va I18NI0000003X.
4. **Kvota/rejalashtirish.** Operator tomonidan belgilangan disk/parallellik chegaralari va orqa bosim bilan navbat so'rovlarini amalga oshirish.
5. **Telemetriya.** Muvaffaqiyatli pin, bo‘laklarni olish kechikishi, sig‘imdan foydalanish va PoR namuna olish natijalari uchun ko‘rsatkichlar/jurnallarni chiqaring.

## Ish taqsimoti

### A. Kassa va modul tuzilishi

| Vazifa | Ega(lar)i | Eslatmalar |
|------|----------|-------|
| `crates/sorafs_node` modullari bilan yarating: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Saqlash jamoasi | Torii integratsiyasi uchun qayta ishlatiladigan turlarni qayta eksport qilish. |
| `SoraFsStorage` dan xaritalangan `StorageConfig` ni amalga oshiring (foydalanuvchi → haqiqiy → standart). | Saqlash jamoasi / Config WG | Norito/`iroha_config` qatlamlarining deterministik ekanligiga ishonch hosil qiling. |
| `NodeHandle` fasadini Torii pinlarni/oldirishlarni yuborish uchun ishlating. | Saqlash jamoasi | Saqlash ichki qismlarini va asenkron sanitariya-tesisatni o'rab oling. |

### B. Doimiy bo'laklar do'koni

| Vazifa | Ega(lar)i | Eslatmalar |
|------|----------|-------|
| Diskdagi manifest indeksi (`sled`/`sqlite`) bilan `sorafs_car::ChunkStore` diskini o'rashni yarating. | Saqlash jamoasi | Deterministik tartib: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| `ChunkStore::sample_leaves` yordamida PoR metamaʼlumotlarini (64KiB/4KiB daraxtlar) saqlang. | Saqlash jamoasi | Qayta ishga tushirilgandan so'ng takrorlashni qo'llab-quvvatlash; korruptsiyada tezda muvaffaqiyatsizlikka uchraydi. |
| Ishga tushganda yaxlitlikni takrorlashni amalga oshiring (rehash manifestlari, to'liq bo'lmagan pinlarni kesish). | Saqlash jamoasi | Torii blokini takrorlash tugaguniga qadar boshlang. |

### C. Gateway oxirgi nuqtalari

| Endpoint | Behaviour | Tasks |
|----------|-----------|-------|
| `GET /v1/sorafs/pin`, `POST /v1/sorafs/pin/register`, `GET /v1/sorafs/pin/{digest_hex}` | Read the pin registry, register paid manifest pins, and fetch bounded manifest pin details. | Validate chunker profiles, manifest payloads, pin policy, fee receipt context, aliases, and successor links before queueing the signed transaction. |
| `POST /v1/sorafs/storage/pin`, `POST /v1/sorafs/storage/fetch`, `POST /v1/sorafs/storage/token` | Store payload bytes for an approved manifest, fetch content ranges, and issue storage access tokens. | Enforce quotas, token policy, provider capability checks, and scheduler/back-pressure limits. |
| `GET /v1/sorafs/storage/manifest/{manifest_id}`, `GET /v1/sorafs/storage/plan/{manifest_id}`, `GET /v1/sorafs/storage/car/{manifest_id}`, `GET /v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}` | Serve bounded manifest metadata, deterministic chunk plans, CAR bytes, and individual chunk bytes. | Keep readback arrays bounded while preserving total counts and verify digest/path bindings before streaming bytes. |
| `GET /v1/sorafs/storage/peers`, `GET /v1/sorafs/storage/state`, `POST /v1/sorafs/storage/por-sample` | Report peer/storage state and exercise bounded local PoR sampling. | Reuse chunk-store sampling and update telemetry; challenge creation and proof/verdict lifecycle handling remain outside the direct-storage router. |


Santexnika ish vaqti `sorafs_node::por` orqali PoR o'zaro ta'sirini o'tkazadi: treker har bir `PorChallengeV1`, `PorProofV1` va `AuditVerdictV1`ni qayd qiladi, shuning uchun `CapacityMeter` ko'rsatkichlari o'zgarmasligini aks ettiradi. Torii mantiq.【crates/sorafs_node/src/scheduler.rs#L147】

Amalga oshirish bo'yicha eslatmalar:

- `norito::json` foydali yuklari bilan Torii Axum stekidan foydalaning.
- Javoblar uchun Norito sxemalarini qo'shing (`PinResultV1`, `FetchErrorV1`, telemetriya tuzilmalari).

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` endi kechikish chuqurligini va eng qadimgi davrni/muddatni va
  tomonidan quvvatlangan har bir provayder uchun eng so'nggi muvaffaqiyat/muvaffaqiyat vaqt belgilari
  `sorafs_node::NodeHandle::por_ingestion_status` va Torii qayd qiladi
  `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` uchun o'lchagichlar asboblar paneli.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:18 83】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Reja tuzuvchi va kvota ijrosi

| Vazifa | Tafsilotlar |
|------|---------|
| Disk kvotasi | Diskdagi baytlarni kuzatish; `max_capacity_bytes` dan oshib ketganda yangi pinlarni rad eting. Kelajakdagi siyosatlar uchun chiqarish ilgaklarini taqdim eting. |
| Parametrni olish | Global semafor (`max_parallel_fetches`) va SF-2d diapazonidan kelib chiqqan holda har bir provayder byudjeti. |
| Pin navbat | Ajoyib qabul qilish ishlarini cheklash; navbat chuqurligi uchun Norito holat so'nggi nuqtalarini ko'rsatish. |
| PoR kadansi | `por_sample_interval_secs` tomonidan boshqariladigan fon ishchisi. |

### E. Telemetriya va jurnallar

Ko'rsatkichlar (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (`result` yorliqlari bilan gistogramma)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Jurnallar / voqealar:

- Boshqaruvni qabul qilish uchun tuzilgan Norito telemetriyasi (`StorageTelemetryV1`).
- Foydalanish > 90% yoki PoR etishmovchiligi chegara chegarasidan oshib ketganda ogohlantiradi.

### F. Sinov strategiyasi

1. **Birlik testlari.** Bo'laklarni saqlash barqarorligi, kvota hisoblari, rejalashtiruvchi o'zgarmaslar (qarang: `crates/sorafs_node/src/scheduler.rs`).  
2. **Integratsiya testlari** (`crates/sorafs_node/tests`). PIN → aylanma safarni olib keling, qayta tiklashni qayta ishga tushiring, kvotani rad etish, PoR namunalarini tekshirish.  
3. **Torii integratsiya testlari.** Xotira yoqilgan holda Toriini ishga tushiring, HTTP so‘nggi nuqtalarini `assert_cmd` orqali ishlating.  
4. **Xaos yo'l xaritasi.** Kelajakdagi mashqlar diskning charchashi, sekin IO, provayderni olib tashlashni taqlid qiladi.

## Bog'liqlar

- SF-2b qabul qilish siyosati - tugunlar reklamadan oldin kirish konvertlarini tekshirishini ta'minlang.  
- SF-2c sig'im bozori - telemetriyani quvvat deklaratsiyasiga bog'lang.  
- SF-2d reklama kengaytmalari - mavjud bo'lganda diapazon qobiliyatini + oqim byudjetlarini iste'mol qiladi.

## Muhim bosqichdan chiqish mezonlari

- `cargo run -p sorafs_node --example pin_fetch` mahalliy moslamalarga qarshi ishlaydi.  
- Torii exposes the current `/v1/sorafs/pin*` and `/v1/sorafs/storage/*` route surface and passes integration tests.
- Hujjatlar ([tugunni saqlash bo'yicha qo'llanma](node-storage.md)) standart konfiguratsiya + CLI misollari bilan yangilangan; operator runbook mavjud.  
- Staging asboblar panelida ko'rinadigan telemetriya; sig'imning to'yinganligi va PoR nosozliklari uchun sozlangan ogohlantirishlar.

## Hujjatlar va Ops yetkazib berish

- Konfiguratsiya standartlari, CLI-dan foydalanish va nosozliklarni bartaraf etish bosqichlari bilan [tugunni saqlash ma'lumotnomasini](node-storage.md) yangilang.  
- SF-3 rivojlanishi davomida [tugun operatsiyalari ish kitobini](node-operations.md) amalga oshirish bilan bir xilda saqlang.  
- Keep API reference for `/v1/sorafs/pin*` and `/v1/sorafs/storage/*` endpoints aligned with the OpenAPI manifest.