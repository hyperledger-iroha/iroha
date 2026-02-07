---
lang: uz
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2025-12-29T18:16:35.978367+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Ta'sirni baholash vositasi (MINFO‑4b)

Yoʻl xaritasi maʼlumotnomasi: **MINFO‑4b — Taʼsirni baholash vositalari.**  
Egasi: Boshqaruv kengashi / Analytics

Ushbu eslatma hozirda `cargo xtask ministry-agenda impact` buyrug'ini hujjatlashtiradi
referendum paketlari uchun zarur bo'lgan avtomatlashtirilgan xesh-oilaviy farqni ishlab chiqaradi. The
vositasi tasdiqlangan Kun tartibi Kengashi takliflarini, dublikat reestrini va iste'mol qiladi
ixtiyoriy rad etish roʻyxati/siyosat surati, shuning uchun sharhlovchilar aynan qaysi birini koʻrishlari mumkin
barmoq izlari yangi, mavjud siyosat bilan to'qnashgan va qancha yozuvlar
har bir hash oilasi hissa qo'shadi.

## Kirishlar

1. **Kun tartibi takliflari.** Bir yoki bir nechta fayl
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   Ularni `--proposal <path>` bilan aniq o'tkazing yoki buyruqni a ga qarating
   `--proposal-dir <dir>` katalogi va shu yo'l ostidagi har bir `*.json` fayli
   kiritilgan.
2. **Dublikat registr (ixtiyoriy).** JSON fayli mos keladi
   `docs/examples/ministry/agenda_duplicate_registry.json`. Mojarolar
   `source = "duplicate_registry"` ostida xabar qilingan.
3. **Siyosat snapshoti (ixtiyoriy).** Har bir roʻyxatni koʻrsatadigan engil manifest
   barmoq izi allaqachon GAR/Vazirlik siyosati tomonidan kiritilgan. Yuklovchi kutadi
   sxemasi quyida ko'rsatilgan (qarang
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   to'liq namuna uchun):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

`hash_family:hash_hex` barmoq izi taklif maqsadiga mos keladigan har qanday yozuv
havola qilingan `policy_id` bilan `source = "policy_snapshot"` ostida xabar qilingan.

## Foydalanish

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

Qo'shimcha takliflar takroriy `--proposal` bayroqlari orqali yoki
butun referendum to'plamini o'z ichiga olgan katalogni taqdim etish:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

`--out` o'tkazib yuborilganda, buyruq yaratilgan JSON-ni stdout-ga chop etadi.

## Chiqish

Hisobot imzolangan artefaktdir (uni referendum paketi ostida yozib oling
`artifacts/ministry/impact/` katalogi) quyidagi tuzilishga ega:

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

Ushbu JSONni har bir referendum maʼlumotlariga neytral xulosa bilan birga ilova qiling
panelistlar, hakamlar hay'ati va boshqaruv kuzatuvchilari aniq portlash radiusini ko'rishlari mumkin
har bir taklif. Chiqish deterministik (xesh oilasi bo'yicha tartiblangan) va xavfsizdir
CI/runbooks ga kiritish; agar dublikat registr yoki siyosat snapshoti o'zgartirilsa,
buyruqni qayta ishga tushiring va ovoz berish boshlanishidan oldin yangilangan artefaktni biriktiring.

> **Keyingi qadam:** yaratilgan ta'sir hisobotini kiriting
> [`cargo xtask ministry-panel packet`](referendum_packet.md) shuning uchun
> `ReferendumPacketV1` ma'lumotlari ham xesh-oilaviy taqsimotni, ham
> ko'rib chiqilayotgan taklif uchun mojarolarning batafsil ro'yxati.