---
lang: uz
direction: ltr
source: docs/source/ministry/reports/2026-Q3-template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f313d8010f2a7174c90f51dea512bcab6eb4a207df9199f28a7352944cb43c8b
source_last_modified: "2025-12-29T18:16:35.981653+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency Report — 2026 Q3 (Template)
summary: Scaffold for the MINFO-8 quarterly transparency packet; replace all tokens before publication.
quarter: 2026-Q3
translator: machine-google-reviewed
---

<!--
  Usage:
    1. Copy this file when drafting a new quarter (e.g., 2026-Q3 → 2026-Q3.md).
    2. Replace every {{TOKEN}} marker and remove instructional callouts.
    3. Attach supporting artefacts (data appendix, CSVs, manifest, Grafana export) under artifacts/ministry/transparency/<YYYY-Q>/.
-->

# Ijroiya xulosasi

> Moderatsiyaning aniqligi, apellyatsiya natijalari, rad etish roʻyxati va gʻaznachilikning muhim jihatlari haqida bir paragrafdan iborat xulosani taqdim eting. Chiqarish T+14 muddatga to'g'ri keladimi yoki yo'qligini eslatib o'ting.

## Chorak ko'rib chiqilmoqda

### E'tiborga molik
- {{HIGHLIGHT_1}}
- {{HIGHLIGHT_2}}
- {{HIGHLIGHT_3}}

### Xatarlar va kamaytirish choralari

| Xavf | Ta'sir | Yumshatish | Egasi | Holati |
|------|--------|------------|-------|--------|
| {{RISK_1}} | {{Ta'sir}} | {{Yumsarlashtirish}} | {{Egasi}} | {{Status}} |
| {{RISK_2}} | {{Ta'sir}} | {{Yumsarlashtirish}} | {{Egasi}} | {{Status}} |

## Ko'rsatkichlarga umumiy nuqtai

Barcha ko'rsatkichlar DP sanitarizatori ishlagandan keyin `ministry_transparency_builder` (Norito to'plami) dan kelib chiqadi. Quyida havola qilingan tegishli CSV tilimlarini biriktiring.

### AI moderatsiyasining aniqligi

| Model profili | Hudud | FP darajasi (maqsadli) | FN kursi (maqsadli) | Drift va kalibrlash | Namuna hajmi | Eslatmalar |
|-------------|--------|------------------|------------------|----------------------|-------------|-------|
| {{profil}} | {{mintaqa}} | {{fp_rate}} ({{fp_target}}) | {{fn_rate}} ({{fn_target}}) | {{drift}} | {{namunalar}} | {{eslatmalar}} |

### Apellyatsiya va panel faoliyati

| Metrik | Qiymat | SLA maqsadi | Trend va Q-1 | Eslatmalar |
|--------|-------|------------|--------------|-------|
| Qabul qilingan murojaatlar | {{apellyatsiyalar_qabul qilingan}} | {{sla}} | {{delta}} | {{eslatmalar}} |
| O'rtacha ruxsat vaqti | {{median_resolution}} | {{sla}} | {{delta}} | {{eslatmalar}} |
| Qaytarilish darajasi | {{reversal_stavka}} | {{target}} | {{delta}} | {{eslatmalar}} |
| Paneldan foydalanish | {{panel_foydalanish}} | {{target}} | {{delta}} | {{eslatmalar}} |

### Rad etish ro'yxati va Favqulodda Canon

| Metrik | Hisob | DP shovqini (e) | Favqulodda bayroqlar | TTL muvofiqligi | Eslatmalar |
|--------|-------|--------------|-----------------|----------------|-------|
| Xesh qo'shimchalari | {{qo'shimchalar}} | {{epsilon_counts}} | {{bayroqlar}} | {{ttl_status}} | {{eslatmalar}} |
| Xeshni olib tashlash | {{olib tashlashlar}} | {{epsilon_counts}} | {{bayroqlar}} | {{ttl_status}} | {{eslatmalar}} |
| Canon chaqiruvlari | {{canon_invocations}} | yo'q | {{bayroqlar}} | {{ttl_status}} | {{eslatmalar}} |

### G'aznachilik harakati

| Oqim | Miqdori (MINFO) | Manbaga havola | Eslatmalar |
|------|----------------|-----------------|-------|
| Depozitlarga murojaat qilish | {{summa}} | {{tx_ref}} | {{eslatmalar}} |
| Panel mukofotlari | {{summa}} | {{tx_ref}} | {{eslatmalar}} |
| Operatsion xarajatlar | {{summa}} | {{tx_ref}} | {{eslatmalar}} |

### Ko'ngillilar va tarqatish signallari

| Metrik | Qiymat | Maqsad | Eslatmalar |
|--------|-------|--------|-------|
| Ko'ngillilar brifinglari chop etildi | {{qiymat}} | {{target}} | {{eslatmalar}} |
| Yoriladigan tillar | {{qiymat}} | {{target}} | {{eslatmalar}} |
| Boshqaruv boʻyicha seminarlar boʻlib oʻtdi | {{qiymat}} | {{target}} | {{eslatmalar}} |

## Differensial maxfiylik va sanitariya

Dezinfektsiyalash jarayonini sarhisob qiling va RNG majburiyatini kiriting.

- Sanitizer ishi: `{{CI_JOB_URL}}`
- DP parametrlari: e={{epsilon_total}}, d={{delta_total}}
- RNG majburiyati: `{{blake3_seed_commitment}}`
- Chelaklar bosilgan: {{suppressed_buckets}}
- QA sharhlovchisi: {{reviewer}}

`artifacts/ministry/transparency/{{Quarter}}/dp_report.json` ni biriktiring va har qanday qo'lda aralashuvlarga e'tibor bering.## Ma'lumotlar qo'shimchalari

| Artefakt | Yo'l | SHA-256 | SoraFS ga yuklanganmi? | Eslatmalar |
|----------|------|---------|---------------------|-------|
| Xulosa PDF | `artifacts/ministry/transparency/{{Quarter}}/summary.pdf` | {{xesh}} | {{Ha/Yo'q}} | {{eslatmalar}} |
| Norito ma'lumotlar ilovasi | `artifacts/ministry/transparency/{{Quarter}}/data/appendix.norito` | {{xesh}} | {{Ha/Yo'q}} | {{eslatmalar}} |
| Metrics CSV to'plami | `artifacts/ministry/transparency/{{Quarter}}/data/csv/` | {{xesh}} | {{Ha/Yo'q}} | {{eslatmalar}} |
| Grafana eksport | `dashboards/grafana/ministry_transparency_overview.json` | {{xesh}} | {{Ha/Yo'q}} | {{eslatmalar}} |
| Ogohlantirish qoidalari | `dashboards/alerts/ministry_transparency_rules.yml` | {{xesh}} | {{Ha/Yo'q}} | {{eslatmalar}} |
| Manifest kelib chiqishi | `artifacts/ministry/transparency/{{Quarter}}/manifest.json` | {{xesh}} | {{Ha/Yo'q}} | {{eslatmalar}} |
| Manifest imzosi | `artifacts/ministry/transparency/{{Quarter}}/manifest.json.sig` | {{xesh}} | {{Ha/Yo'q}} | {{eslatmalar}} |

## Nashr metamaʼlumotlari

| Maydon | Qiymat |
|-------|-------|
| Chiqarish choragi | {{chorak}} |
| Chiqarish vaqt tamg'asi (UTC) | {{vaqt tamg'asi}} |
| SoraFS CID | `{{cid}}` |
| Boshqaruv ovozi ID | {{vote_id}} |
| Manifest dayjesti (`blake2b`) | `{{manifest_digest}}` |
| Git commit / teg | `{{git_rev}}` |
| Egasini chiqarish | {{egasi}} |

## Tasdiqlashlar

| Rol | Ism | Qaror | Vaqt tamg'asi | Eslatmalar |
|------|------|----------|-----------|-------|
| Vazirlikning kuzatuv qobiliyati TL | {{name}} | ✅/⚠️ | {{vaqt tamg'asi}} | {{eslatmalar}} |
| Boshqaruv kengashi bilan aloqa | {{name}} | ✅/⚠️ | {{vaqt tamg'asi}} | {{eslatmalar}} |
| Docs/Comms Lead | {{name}} | ✅/⚠️ | {{vaqt tamg'asi}} | {{eslatmalar}} |

## O'zgarishlar jurnali va kuzatuvlar

- {{CHANGELOG_ITEM_1}}
- {{CHANGELOG_ITEM_2}}

### Harakat elementlarini oching

| Element | Egasi | Muddati | Holati | Eslatmalar |
|------|-------|-----|--------|-------|
| {{Harakat}} | {{Egasi}} | {{Muddati}} | {{Status}} | {{Eslatmalar}} |

### Aloqa

- Asosiy kontakt: {{contact_name}} (`{{chat_handle}}`)
- Eskalatsiya yo'li: {{escalation_details}}
- Tarqatish roʻyxati: {{mailing_list}}

_Shablon versiyasi: 2026-03-25. Tarkibiy o'zgarishlarni amalga oshirishda qayta ko'rib chiqish sanasini yangilang._