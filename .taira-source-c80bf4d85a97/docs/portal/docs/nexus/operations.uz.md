---
lang: uz
direction: ltr
source: docs/portal/docs/nexus/operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dea5098afdf15e92c78aba363b38f3ec8ce2018672a3d34bb1b505e2ee2f5869
source_last_modified: "2025-12-29T18:16:35.144858+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-operations
title: Nexus operations runbook
description: Field-ready summary of the Nexus operator workflow, mirroring `docs/source/nexus_operations.md`.
translator: machine-google-reviewed
---

Ushbu sahifadan tezkor havola sifatida foydalaning
`docs/source/nexus_operations.md`. Bu operatsion nazorat ro'yxatini distillaydi, o'zgartiradi
boshqaruv kancalari va Nexus operatorlari talab qiladigan telemetriya qamrovi talablari
ergash.

## Hayot aylanishini tekshirish ro'yxati

| Bosqich | Amallar | Dalil |
|-------|--------|----------|
| Parvoz oldidan | Chiqarish xeshlarini/imzolarini tekshiring, `profile = "iroha3"`ni tasdiqlang va konfiguratsiya shablonlarini tayyorlang. | `scripts/select_release_profile.py` chiqishi, nazorat summasi jurnali, imzolangan manifest to'plami. |
| Katalogni tekislash | `[nexus]` katalogini, marshrutlash siyosatini va kengash tomonidan chiqarilgan manifest uchun DA chegaralarini yangilang, keyin `--trace-config` ni yozib oling. | `irohad --sora --config … --trace-config` chiqishi bortga chiqish chiptasi bilan saqlanadi. |
| Smoke & cutover | `irohad --sora --config … --trace-config` ni ishga tushiring, CLI tutunini (`FindNetworkStatus`) bajaring, telemetriya eksportini tasdiqlang va qabul qilishni so'rang. | Smoke-test log + Alertmanager tasdiqlash. |
| Barqaror holat | Nazorat paneli/ogohlantirishlarni kuzatib boring, boshqaruv ritmi bo'yicha tugmachalarni aylantiring va manifestlar o'zgarganda konfiguratsiyalar/runbooklarni sinxronlashtiring. | Har chorakda ko'rib chiqish daqiqalari, asboblar panelidagi skrinshotlar, aylanish chiptasi identifikatorlari. |

Batafsil ishga tushirish (kalitlarni almashtirish, marshrutlash shablonlari, profilni chiqarish bosqichlari)
`docs/source/sora_nexus_operator_onboarding.md` ichida qoladi.

## O'zgarishlarni boshqarish

1. **Reliz yangilanishlari** – `status.md`/`roadmap.md` da e'lonlarni kuzatish; biriktir
   har bir reliz PR uchun ishga tushirish nazorat ro'yxati.
2. **Lane manifest o'zgarishlar** – Space Directory va imzolangan paketlarni tasdiqlang
   ularni `docs/source/project_tracker/nexus_config_deltas/` ostida arxivlang.
3. **Konfiguratsiya deltalari** – har bir `config/config.toml` o‘zgarishi chipta talab qiladi
   qatorga/ma'lumotlar maydoniga murojaat qilish. Samarali konfiguratsiyaning tahrirlangan nusxasini saqlang
   tugunlar birlashganda yoki yangilanganda.
4. **Orqaga qaytish matkaplari** – har chorakda to‘xtash/tiklash/tutunlash tartib-qoidalarini takrorlash; jurnal
   `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` ostida natijalar.
5. **Muvofiqlikni tasdiqlash** – xususiy/CBDC yo‘llari muvofiqlikni ro‘yxatdan o‘tkazishni ta’minlashi kerak
   DA siyosati yoki telemetriyani tahrirlash tugmachalarini o'zgartirishdan oldin (qarang
   `docs/source/cbdc_lane_playbook.md`).

## Telemetriya va SLOlar

- Boshqaruv paneli: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, plus
  SDK-ga xos koʻrinishlar (masalan, `android_operator_console.json`).
- Ogohlantirishlar: `dashboards/alerts/nexus_audit_rules.yml` va Torii/Norito transport
  qoidalar (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Ko'rish uchun ko'rsatkichlar:
  - `nexus_lane_height{lane_id}` - uchta slot uchun nolga o'tish haqida ogohlantirish.
  - `nexus_da_backlog_chunks{lane_id}` - bo'lakka xos chegaralardan yuqorida ogohlantirish
    (standart 64 ommaviy / 8 xususiy).
  - `nexus_settlement_latency_seconds{lane_id}` - P99 900ms dan oshganda ogohlantirish
    (ommaviy) yoki 1200ms (xususiy).
  - `torii_request_failures_total{scheme="norito_rpc"}` - 5 daqiqalik xatolik haqida ogohlantirish
    nisbat > 2%.
  - `telemetry_redaction_override_total` – Sev2 darhol; bekor qilishni ta'minlash
    muvofiqlik chiptalariga ega.
- Telemetriyani tuzatish nazorat ro'yxatini ishga tushiring
  [Nexus telemetriya remediatsiyasi rejasi](./nexus-telemetry-remediation) kamida
  har chorakda va to'ldirilgan shaklni operatsiyalarni tekshirish qaydlariga ilova qiling.

## Hodisa matritsasi

| Jiddiylik | Ta'rif | Javob |
|----------|------------|----------|
| Sev1 | Ma'lumotlar makonini izolyatsiya qilish buzilishi, hisob-kitoblarni to'xtatish > 15 daqiqa yoki boshqaruv ovozining buzilishi. | Sahifa Nexus Birlamchi + Reliz muhandisligi + Muvofiqlik, kirishni muzlatish, artefaktlarni yig'ish, xabarlarni nashr qilish ≤60min, RCA ≤5 ish kuni. |
| Sev2 | Yo‘lak bo‘ylab SLA buzilishi, telemetriyadagi ko‘r nuqta >30 daqiqa, manifestni ishga tushirish muvaffaqiyatsiz tugadi. | Sahifa Nexus Birlamchi + SRE, ≤4 soatni yumshatish, 2 ish kuni ichida kuzatuvlarni fayl. |
| Sev3 | Bloklanmagan drift (hujjatlar, ogohlantirishlar). | Kuzatuvchiga kiring, sprint ichida tuzatishni rejalashtirish. |

Hodisa chiptalarida ta'sirlangan qator/ma'lumotlar maydoni identifikatorlari, manifest xeshlari,
vaqt jadvali, qo'llab-quvvatlovchi ko'rsatkichlar/jurnallar va keyingi vazifalar/egalar.

## Dalillar arxivi

- `artifacts/nexus/<lane>/<date>/` ostida paketlar/manifestlar/temetriya eksportlarini saqlang.
- Har bir nashr uchun tahrirlangan konfiguratsiyalar + `--trace-config` chiqishini saqlang.
- Konfiguratsiya yoki manifest o'zgarishlari paytida kengash bayonnomalarini + imzolangan qarorlarni ilova qiling.
- Nexus koʻrsatkichlariga mos keladigan haftalik Prometheus oniy suratlarini 12 oy davomida saqlang.
- `docs/source/project_tracker/nexus_config_deltas/README.md` da runbook tahrirlarini yozib oling
  shuning uchun auditorlar mas'uliyat qachon o'zgarganligini bilishadi.

## Tegishli material

- Umumiy ko'rinish: [Nexus umumiy ko'rinishi](./nexus-overview)
- Texnik xususiyatlari: [Nexus spetsifikatsiyasi](./nexus-spec)
- Bo'lak geometriyasi: [Nexus chiziqli modeli](./nexus-lane-model)
- O'tish va marshrutlash moslamalari: [Nexus o'tish qaydlari](./nexus-transition-notes)
- Operatorni ishga tushirish: [Sora Nexus operatorni ishga tushirish](./nexus-operator-onboarding)
- Telemetriyani tuzatish: [Nexus telemetriyani tuzatish rejasi](./nexus-telemetry-remediation)