---
lang: uz
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2025-12-29T18:16:35.982646+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
translator: machine-google-reviewed
---

> **Qanday foydalaniladi:** bu shablonni har bir mashqdan so'ng darhol `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` ga takrorlang. Fayl nomlarini kichik, tire va Alertmanager tizimiga kiritilgan ma'lumot identifikatori bilan tekislang.

# Red-Team Drill Report - `<SCENARIO NAME>`

- **Matkap ID:** `<YYYYMMDD>-<scenario>`
- **Sana va oyna:** `<YYYY-MM-DD HH:MMZ – HH:MMZ>`
- **Ssenariy sinfi:** `smuggling | bribery | gateway | ...`
- **Operatorlar:** `<names / handles>`
- **Boshqaruv panellari topshirilgandan muzlatilgan:** `<git SHA>`
- **Dalillar to'plami:** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID (ixtiyoriy):** `<cid>`  
- **Tegishli yoʻl xaritasi elementlari:** `MINFO-9`, shuningdek har qanday bogʻlangan chiptalar.

## 1. Maqsadlar va kirish shartlari

- **Asosiy maqsadlar**
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- **Tasdiqlangan shartlar**
  - `emergency_canon_policy.md` versiyasi `<tag>`
  - `dashboards/grafana/ministry_moderation_overview.json` dayjest `<sha256>`
  - Qo'ng'iroq paytida vakolatni bekor qilish: `<name>`

## 2. Bajarish xronologiyasi

| Vaqt tamg'asi (UTC) | Aktyor | Harakat / Buyruq | Natija / Eslatmalar |
|----------------|-------|------------------|----------------|
|  |  |  |  |

> Torii soʻrov identifikatorlari, boʻlak xeshlari, tasdiqlovlarni bekor qilish va Alertmanager havolalarini qoʻshing.

## 3. Kuzatishlar va ko'rsatkichlar

| Metrik | Maqsad | Kuzatilgan | O'tdi/qobiliyatsiz | Eslatmalar |
|--------|--------|----------|-----------|-------|
| Ogohlantirish javobining kechikishi | `<X> min` | `<Y> min` | ✅/⚠️ |  |
| Moderatsiyani aniqlash tezligi | `>= <value>` |  |  |  |
| Gateway anomaliyasini aniqlash | `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `<path>`

## 4. Topilmalar va tuzatish

| Jiddiylik | Topish | Egasi | Maqsadli sana | Holat / Havola |
|----------|---------|-------|-------------|---------------|
| Yuqori |  |  |  |  |

Kalibrlash qanday namoyon boʻlishini hujjatlashtiring, rad etish qoidalari yoki SDK/asboblar oʻzgarishi kerak. GitHub/Jira muammolariga havola va bloklangan/bloklanmagan holatlarni qayd qiling.

## 5. Boshqaruv va tasdiqlash

- **Hodisa komandirining imzosi:** `<name / timestamp>`
- **Boshqaruv kengashining ko'rib chiqish sanasi:** `<meeting id>`
- **Kuzatuv roʻyxati:** `[ ] status.md updated`, `[ ] roadmap row updated`, `[ ] transparency packet annotated`

## 6. Qo'shimchalar

- `[ ] CLI logbook (`logs/.md`)`
- `[ ] Dashboard JSON export`
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

Dalillar toʻplamiga va SoraFS suratiga yuklangandan soʻng har bir biriktirmani `[x]` bilan belgilang.

---

_Oxirgi yangilangan: {{ sana | default("2026-02-20") }}_