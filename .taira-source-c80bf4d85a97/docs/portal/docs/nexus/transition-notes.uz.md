---
lang: uz
direction: ltr
source: docs/portal/docs/nexus/transition-notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 90869bf46975e3ac40cf606b2ac27597f81e34acbce25261dc9f588e0fac3103
source_last_modified: "2026-01-06T05:56:41.006216+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-transition-notes
title: Nexus transition notes
description: Mirror of `docs/source/nexus_transition_notes.md`, covering Phase B transition evidence, audit schedule, and mitigations.
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus o'tish eslatmalari

Bu jurnal uzoq davom etayotgan **PhaseB — Nexus Transition Foundations** ishini kuzatib boradi.
ko'p qatorli ishga tushirish nazorat ro'yxati tugaguncha. Bu muhim bosqichni to'ldiradi
`roadmap.md` yozuvlari va B1–B4 tomonidan havola qilingan dalillarni bir joyda saqlaydi
shuning uchun boshqaruv, SRE va SDK etakchilari bir xil haqiqat manbasini bo'lishishi mumkin.

## Qo'llanish doirasi va tezligi

- Marshrutlangan kuzatuv tekshiruvlari va telemetriya to'siqlarini (B1/B2) qamrab oladi
  boshqaruv tomonidan tasdiqlangan konfiguratsiya delta to'plami (B3) va ko'p qatorli ishga tushirish
  repetitsiya kuzatuvlari (B4).
- Bu yerda ilgari yashagan vaqtinchalik kadens notasini almashtiradi; 2026 yildan boshlab
  1-chorak auditi batafsil hisobotda joylashgan
  `docs/source/nexus_routed_trace_audit_report_2026q1.md`, bu sahifaga tegishli
  ish jadvali va yumshatilish reestri.
- Har bir marshrutlangan kuzatuv oynasi, boshqaruv ovozi yoki ishga tushirilgandan keyin jadvallarni yangilang
  mashq. Har doim artefaktlar harakatlansa, ushbu sahifadagi yangi joyni aks ettiring
  Shunday qilib, quyi oqim hujjatlari (holat, boshqaruv paneli, SDK portallari) barqaror faylga ulanishi mumkin.
  langar.

## Dalillar surati (2026 yil 1-chorak 2)

| Ish oqimi | Dalil | Ega(lar)i | Holati | Eslatmalar |
|------------|----------|----------|--------|-------|
| **B1 — Marshrutlangan kuzatuv tekshiruvlari** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | ✅ Toʻliq (2026 yil 1-chorak) | Uchta audit oynasi qayd etilgan; `TRACE-CONFIG-DELTA` dan TLS kechikishi 2-chorak qayta ishlanishida yopildi. |
| **B2 — Telemetriya remediatsiyasi va to'siqlar** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | ✅ To'liq | Ogohlantirish to'plami, farqli bot siyosati va OTLP to'plami o'lchami (`nexus.scheduler.headroom` log + Grafana bosh paneli) jo'natildi; ochiq chegirmalar yo'q. |
| **B3 — Delta sozlamalarini tasdiqlash** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | ✅ To'liq | GOV-2026-03-19 ovozi qo'lga olindi; imzolangan toʻplam quyida qayd etilgan telemetriya paketini taʼminlaydi. |
| **B4 — Ko‘p qatorli uchish repetisiyasi** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | ✅ Toʻliq (2026 yil 2-chorak) | 2-chorak kanareykalarni takrorlash TLS kechikishini yumshatishni yopdi; validator manifest + `.sha256` suratga olish uyasi diapazoni 912–936, ish yuki urugʻi `NEXUS-REH-2026Q2` va qayta ishga tushirishdan yozilgan TLS profil xeshi. |

## Har chorakda yo'naltirilgan kuzatuv tekshiruvi jadvali

| Trace ID | Oyna (UTC) | Natija | Eslatmalar |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | ✅ O'tish | Navbatga kirish P95 ≤750ms ko'rsatkichidan ancha past bo'lib qoldi. Hech qanday harakat talab qilinmaydi. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | ✅ O'tish | `status.md` ga biriktirilgan OTLP replay xeshlari; SDK diff bot pariteti nol driftni tasdiqladi. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | ✅ Hal qilindi | TLS profilining kechikishi 2-chorak qayta ishga tushirilganda yopildi; `NEXUS-REH-2026Q2` uchun telemetriya toʻplami TLS profil xeshini `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (qarang: `artifacts/nexus/tls_profile_rollout_2026q2/`) va nol stragglerlarni qayd qiladi. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12–10:14 | ✅ O'tish | Ish yuki urug'i `NEXUS-REH-2026Q2`; telemetriya toʻplami + `artifacts/nexus/rehearsals/2026q2/` kun tartibi bilan `artifacts/nexus/rehearsals/2026q1/` (slot diapazoni 912–936) ostida manifest/dijest. |

Kelgusi choraklar yangi qatorlarni qo'shishi va ko'chirishi kerak
jadval oqimdan oshib ketganda ilovaga to'ldirilgan yozuvlar
chorak. Marshrutlangan kuzatuv hisobotlari yoki boshqaruv protokollaridan ushbu bo'limga havola qiling
`#quarterly-routed-trace-audit-schedule` langari yordamida.

## Yumshatish va Ortiqcha ishlar

| Element | Tavsif | Egasi | Maqsad | Holat / Eslatmalar |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` vaqtida kechikib qolgan TLS profilini tarqatishni tugating, qayta ishlash dalillarini oling va yumshatish jurnalini yoping. | @release-eng, @sre-core | 2026 yil 2-chorak marshrutni kuzatish oynasi | ✅ Yopiq — `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` da olingan TLS profil xeshi; Qayta ko'rish hech qanday stragglers yo'qligini tasdiqladi. |
| `TRACE-MULTILANE-CANARY` tayyorgarlik | 2-chorak mashqlarini rejalashtiring, telemetriya to'plamiga moslamalar biriktiring va SDK jabduqlari tasdiqlangan yordamchidan qayta foydalanishiga ishonch hosil qiling. | @telemetry-ops, SDK dasturi | Rejalashtirish qo'ng'irog'i 2026-04-30 | ✅ Tugallangan — kun tartibi `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` da slot/ish yuki metamaʼlumotlari bilan saqlanadi; trekerda qayd qilingan jabduqlardan qayta foydalanish. |
| Telemetriya to'plamini aylantirish | Har bir mashq/chiqarishdan oldin `scripts/telemetry/validate_nexus_telemetry_pack.py` ni ishga tushiring va konfiguratsiya delta kuzatuvchisi yonidagi dayjestlarni qayd qiling. | @telemetry-ops | Har bir reliz nomzodi | ✅ Tugallangan — `telemetry_manifest.json` + `artifacts/nexus/rehearsals/2026q1/` da chiqarilgan `.sha256` (slot diapazoni `912-936`, `NEXUS-REH-2026Q2` urug'i); treker va dalillar indeksiga ko'chirilgan dayjestlar. |

## Config Delta Bundle Integration

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` qoladi
  kanonik farq xulosasi. Yangi `defaults/nexus/*.toml` yoki genezis o'zgarganda
  land, birinchi navbatda o'sha trekerni yangilang, so'ngra bu yerda diqqatga sazovor joylarni aks ettiring.
- Imzolangan konfiguratsiya to'plamlari repetisiya telemetriya paketini ta'minlaydi. Paket, tasdiqlangan
  `scripts/telemetry/validate_nexus_telemetry_pack.py` tomonidan chop etilishi kerak
  konfiguratsiya deltasi dalillari bilan birga operatorlar aniq takrorlashlari mumkin
  B4 davrida ishlatilgan artefaktlar.
- Iroha 2 ta toʻplam yoʻlaksiz qoladi: hozir `nexus.enabled = false` bilan konfiguratsiyalar
  Nexus profili yoqilmagan bo'lsa, qator/ma'lumotlar maydoni/marshrutni bekor qilishni rad etish
  (`--sora`), shuning uchun `nexus.*` qismlarini bir qatorli shablonlardan ajratib oling.
- Boshqaruv ovoz berish jurnalini (GOV-2026-03-19) ham kuzatuvchidan, ham ulangan holda saqlang
  Ushbu eslatma kelajakdagi ovozlar formatni qayta kashf qilmasdan nusxalashi mumkin
  tasdiqlash marosimi.

## Repetitsiya kuzatuvlarini ishga tushiring

- `docs/source/runbooks/nexus_multilane_rehearsal.md` kanareyka rejasini oladi,
  ishtirokchilar ro'yxati va orqaga qaytish bosqichlari; Har safar chiziq bo'ylab yugurish kitobini yangilang
  topologiya yoki telemetriya eksportchilari o'zgaradi.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` har bir artefaktni sanab o'tadi
  9-apreldagi mashq paytida tekshirildi va hozirda 2-chorak tayyorgarlik eslatmalari/kun tartibi mavjud.
  Bir martalik ochish o'rniga, kelajakdagi mashqlarni xuddi shu trekerga qo'shing
  dalillarni monoton holda saqlash uchun kuzatuvchilar.
- OTLP kollektor parchalarini va Grafana eksportlarini nashr qilish (qarang: `docs/source/telemetry.md`)
  har safar eksportchining paketlar bo'yicha yo'riqnomasi o'zgarganda; 1-chorak yangilanishi uni to'sib qo'ydi
  Bo'shliqlar haqida ogohlantirishlarning oldini olish uchun partiya hajmi 256 ta namunaga.
- Ko'p qatorli CI/test dalillari hozirda mavjud
  `integration_tests/tests/nexus/multilane_pipeline.rs` va ostida ishlaydi
  `Nexus Multilane Pipeline` ish jarayoni
  (`.github/workflows/integration_tests_multilane.yml`), nafaqaga chiqqanlarni almashtirish
  `pytests/nexus/test_multilane_pipeline.py` ma'lumotnomasi; uchun xeshni saqlang
  `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) sinxronlashtirilgan
  mashq to'plamlarini yangilashda treker bilan.

## Runtime Lane Lifecycle

- Runtime line lifecycles rejalari endi ma'lumotlar maydoni ulanishlarini tasdiqlaydi va qachon to'xtatiladi
  Kura/darajali saqlash moslashuvi bajarilmaydi, bu esa katalogni o'zgarishsiz qoldiradi. The
  yordamchilar nafaqaga chiqqan yo'llar uchun keshlangan yo'lak relelarini kesadi, shuning uchun birlashma kitobi sintezi
  eskirgan dalillarni qayta ishlatmaydi.
- Nexus konfiguratsiya/hayot tsikli yordamchilari orqali rejalarni qo'llang (`State::apply_lane_lifecycle`,
  `Queue::apply_lane_lifecycle`) yo'llarni qayta ishga tushirmasdan qo'shish/o'chirish; marshrutlash,
  TEU suratlari va manifest registrlari muvaffaqiyatli rejadan keyin avtomatik ravishda qayta yuklanadi.
- Operatorga ko'rsatma: reja muvaffaqiyatsizlikka uchraganida, etishmayotgan ma'lumotlar bo'shliqlari yoki saqlash joylarini tekshiring
  yaratib bo'lmaydigan ildizlar (darajali sovuq ildiz/Kura yo'li kataloglari). ni tuzating
  qo'llab-quvvatlash yo'llari va qayta urinib ko'ring; Muvaffaqiyatli rejalar yo'lak/ma'lumotlar maydoni telemetriyasini qayta chiqaradi
  diff shuning uchun asboblar paneli yangi topologiyani aks ettiradi.

## NPoS telemetriyasi va orqa bosim dalillari

PhaseB-ning ishga tushirish-mashq retrosi deterministik telemetriyani so'radi
NPoS yurak stimulyatori va g'iybat qatlamlari o'zlarining orqa bosimida qolishlarini isbotlang
chegaralar. Integratsiya jabduqlari da
`integration_tests/tests/sumeragi_npos_performance.rs` ularni mashq qiladi
stsenariylar va JSON xulosalarini chiqaradi (`sumeragi_baseline_summary::<scenario>::…`)
yangi ko'rsatkichlar tushganda. Uni mahalliy sifatida ishga tushiring:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` yoki
Yuqori kuchlanishli topologiyalarni o'rganish uchun `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R`; the
sukut bo'yicha B4 da ishlatiladigan 1s/`k=3` kollektor profilini aks ettiradi.

| Ssenariy / test | Qamrash | Asosiy telemetriya |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Dalillar to'plamini ketma-ketlashtirishdan oldin EMA kechikish konvertlarini, navbat chuqurliklarini va ortiqcha jo'natish o'lchagichlarini yozib olish uchun repetisiya bloklash vaqti bilan 12 raundni bloklang. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Qabulni kechiktirish aniq boshlanishini va navbat sig'im/to'yinganlik hisoblagichlarini eksport qilishini ta'minlash uchun tranzaksiya navbatini to'ldiradi. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Sozlangan ±125‰ diapazoni bajarilganligini isbotlamaguncha yurak stimulyatori jitter namunalarini oladi va taym-autlarni ko'ring. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Seanslar va bayt hisoblagichlari ko'tarilish, orqaga chekinish va do'konni ko'paytirmasdan joylashishni ko'rsatish uchun katta RBC yuklarini yumshoq/qattiq do'kon chegaralariga suradi. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Ortiqcha jo‘natish nisbati o‘lchagichlari va maqsadli kollektorlar taymerlari oldinga siljishi uchun kuchlar qayta uzatiladi, bu esa so‘ralgan retro telemetriyaning uchigacha simli ekanligini isbotlaydi. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Orqaga qolgan monitorlar foydali yuklarni jimgina tushirish o'rniga nosozliklarni ko'tarishini tekshirish uchun aniq intervalgacha bo'laklarni tushiradi. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |JSON chiziqlarini Prometheus qirqish bilan birga jabduqlar chop eting.
boshqaruv teskari bosimni isbotlashni so'raganda, yugurish paytida qo'lga olinadi
signallar takroriy topologiyaga mos keladi.

## Tekshirish roʻyxatini yangilash

1. Yangi marshrutlangan oynalarni qo'shing va chorak aylanayotganda eskilarini o'chirib qo'ying.
2. Har bir Alertmanager kuzatuvidan keyin yumshatish jadvalini yangilang, hatto
   harakat chiptani yopishdir.
3. Konfiguratsiya deltalari o'zgarganda, treker, ushbu eslatma va telemetriyani yangilang
   bir xil tortish so'rovida to'plami hazm qilish ro'yxati.
4. Kelajakdagi yoʻl xaritasi holati uchun har qanday yangi repetitsiya/temetriya artefaktlarini bu yerga bogʻlang
   yangilanishlar tarqoq vaqtinchalik eslatmalar o'rniga bitta hujjatga murojaat qilishi mumkin.

## Dalillar indeksi

| Aktiv | Manzil | Eslatmalar |
|-------|----------|-------|
| Yo'naltirilgan kuzatuv auditi hisoboti (2026 yil 1-chorak) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | PhaseB1 dalillari uchun kanonik manba; `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` ostida portal uchun aks ettirilgan. |
| Delta kuzatuvchisini sozlash | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Unda TRACE-CONFIG-DELTA farqlari xulosalari, sharhlovchining bosh harflari va GOV-2026-03-19 ovoz berish jurnali mavjud. |
| Telemetriyani tuzatish rejasi | `docs/source/nexus_telemetry_remediation_plan.md` | B2 ga bog'langan ogohlantirish to'plami, OTLP to'plami o'lchami va eksport byudjeti to'siqlarini hujjatlashtiradi. |
| Ko'p qatorli repetitsiya kuzatuvchisi | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 9-apreldagi mashq artefaktlari, validator manifest/dijest, 2-chorak tayyorgarlik eslatmalari/kun tartibi va orqaga qaytarish dalillari roʻyxati. |
| Telemetriya toʻplami manifest/dijest (oxirgi) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Slot diapazoni 912–936, `NEXUS-REH-2026Q2` urugʻi va boshqaruv toʻplamlari uchun artefakt xeshlarini yozib oladi. |
| TLS profil manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | 2-chorak takrorlash vaqtida olingan tasdiqlangan TLS profilining xesh; marshrutlangan-iz qo'shimchalarida keltiring. |
| TRACE-MULTILANE-CANARY kun tartibi | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | 2-chorak repetisiyasi uchun rejalashtirish eslatmalari (oyna, slot oralig'i, ish yuki urug'i, harakat egalari). |
| Repetitsiya runbook |ni ishga tushiring `docs/source/runbooks/nexus_multilane_rehearsal.md` | Sahnalashtirish uchun operatsion nazorat ro'yxati → bajarish → orqaga qaytarish; yo'lak topologiyasi yoki eksportchi yo'riqnomasi o'zgarganda yangilang. |
| Telemetriya paketi tekshiruvi | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 retro tomonidan havola qilingan CLI; to'plam o'zgarganda treker bilan birga arxiv dayjestlari. |
| Ko'p qatorli regressiya | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Ko'p qatorli konfiguratsiyalar uchun `nexus.enabled = true` ni isbotlaydi, Sora katalogi xeshlarini saqlaydi va artefakt dayjestlarini nashr etishdan oldin `ConfigLaneRouter` orqali qatorli mahalliy Kura/birlashtirish jurnali yo'llarini (`blocks/lane_{id:03}_{slug}`) taqdim etadi. |