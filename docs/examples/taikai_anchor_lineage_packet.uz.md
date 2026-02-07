---
lang: uz
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-12-29T18:16:35.095373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Anchor Lineage paket shabloni (SN13-C)

“Yo‘l xaritasi” bandi **SN13-C — Manifestlar va SoraNS ankorlari** har bir taxallusni talab qiladi
deterministik dalillar to'plamini jo'natish uchun aylanish. Ushbu shablonni o'zingizning ilovangizga nusxalang
ishlab chiqarish artefakt katalogi (masalan
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) va almashtiring
paketni boshqaruvga topshirishdan oldin joy egalari.

## 1. Metadata

| Maydon | Qiymat |
|-------|-------|
| Voqea identifikatori | `<taikai.event.launch-2026-07-10>` |
| Oqim / ko'rsatish | `<main-stage>` |
| Taxallus nom maydoni / nom | `<sora / docs>` |
| Dalillar katalogi | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| Operator bilan aloqa | `<name + email>` |
| GAR / RPT chiptasi | `<governance ticket or GAR digest>` |

## Paket yordamchisi (ixtiyoriy)

Spool artefaktlaridan nusxa oling va oldin JSON (ixtiyoriy imzolangan) xulosasini chiqaring
qolgan bo'limlarni to'ldirish:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

Yordamchi `taikai-anchor-request-*`, `taikai-trm-state-*`,
Taikai spool katalogidan `taikai-lineage-*`, konvertlar va qo'riqchilar
(`config.da_ingest.manifest_store_dir/taikai`) shuning uchun dalillar papkasi allaqachon
quyida havola qilingan aniq fayllarni o'z ichiga oladi.

## 2. Nasl kitobi va maslahat

Diskdagi nasl kitobi va buning uchun yozgan JSON Torii maslahatini ilova qiling.
oyna. Bular to'g'ridan-to'g'ri keladi
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` va
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Artefakt | Fayl | SHA-256 | Eslatmalar |
|----------|------|---------|-------|
| Nasab kitobi | `taikai-trm-state-docs.json` | `<sha256>` | Oldingi manifest dayjestini/oynasini isbotlaydi. |
| Nasab haqida maslahat | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | SoraNS langariga yuklashdan oldin olingan. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Anchor foydali yukni ushlab turish

Torii langar xizmatiga yetkazilgan POST foydali yukini yozib oling. Foydali yuk
`envelope_base64`, `ssm_base64`, `trm_base64` va inline oʻz ichiga oladi
`lineage_hint` ob'ekti; auditlar bu ishorani isbotlash uchun ushbu suratga tayanadi
SoraNS ga yuborildi. Torii endi bu JSONni avtomatik ravishda shunday yozadi
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
Taikai spool katalogida (`config.da_ingest.manifest_store_dir/taikai/`), shuning uchun
operatorlar HTTP jurnallarini qirib tashlash o'rniga uni to'g'ridan-to'g'ri nusxalashlari mumkin.

| Artefakt | Fayl | SHA-256 | Eslatmalar |
|----------|------|---------|-------|
| Anchor POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | Xom soʻrov `taikai-anchor-request-*.json` (Taikai spool) dan koʻchirildi. |

## 4. Manifest dayjestni tan olish

| Maydon | Qiymat |
|-------|-------|
| Yangi manifest dayjesti | `<hex digest>` |
| Oldingi manifest dayjesti (maslahatdan) | `<hex digest>` |
| Oyna boshlanishi / tugashi | `<start seq> / <end seq>` |
| Qabul qilish vaqt tamg'asi | `<ISO8601>` |

Yuqorida qayd etilgan daftarga/maslahat xeshlariga murojaat qiling, shunda sharhlovchilar buni tekshirishlari mumkin
almashtirilgan oyna.

## 5. Ko'rsatkichlar / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` surati: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` dump (taxallus uchun): `<file path + hash>`

Hisoblagichni ko'rsatadigan Prometheus/Grafana eksport yoki `curl` chiqishini taqdim eting
oshirish va bu taxallus uchun `/status` massivi.

## 6. Dalillar katalogi uchun manifest

Dalillar katalogining deterministik manifestini yarating (spool fayllari,
foydali yukni olish, o'lchovlar oniy tasvirlari) shuning uchun boshqaruv har bir xeshni tekshirishi mumkin
arxivni ochish.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Artefakt | Fayl | SHA-256 | Eslatmalar |
|----------|------|---------|-------|
| Dalillar manifest | `manifest.json` | `<sha256>` | Buni boshqaruv paketiga / GARga biriktiring. |

## 7. Tekshirish ro'yxati

- [ ] Nasl kitobi koʻchirildi + xeshlangan.
- [ ] Nasl bo'yicha maslahat ko'chirildi + xeshlangan.
- [ ] Anchor POST foydali yuki yozib olindi va xeshlandi.
- [ ] Manifest dayjest jadvali toʻldirilgan.
- [ ] Koʻrsatkichlar oniy suratlari eksport qilindi (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] Manifest `scripts/repo_evidence_manifest.py` bilan yaratilgan.
- [ ] Xeshlar + aloqa ma'lumotlari bilan boshqaruvga yuklangan paket.

Har bir taxallus aylanishi uchun ushbu shablonni saqlash SoraNS boshqaruvini saqlab qoladi
to'plam qayta ishlab chiqariladi va nasl-nasab haqidagi maslahatlarni bevosita GAR/RPT dalillariga bog'laydi.