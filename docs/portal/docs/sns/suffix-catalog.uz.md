---
lang: uz
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffd062b69b97f11e5baa0ae82256c87cb76600982d599e1953573c1944112f51
source_last_modified: "2026-01-22T16:26:46.520176+00:00"
translation_last_reviewed: 2026-02-07
title: Sora Name Service Suffix Catalog
sidebar_label: Suffix catalog
description: Canonical allowlist of SNS suffixes, stewards, and pricing knobs for `.sora`, `.nexus`, and `.dao`.
translator: machine-google-reviewed
---

# Sora nomi xizmati qo'shimchalari katalogi

SNS yo'l xaritasi har bir tasdiqlangan qo'shimchani (SN-1/SN-2) kuzatib boradi. Ushbu sahifa aks ettiradi
ro'yxatga oluvchilar, DNS shlyuzlari yoki hamyonni boshqaradigan operatorlar uchun haqiqat manbasi katalogi
asboblar holat hujjatlarini qirib tashlamasdan bir xil parametrlarni yuklashi mumkin.

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Iste'molchilar:** `iroha sns policy`, SNS ishga tushirish to'plamlari, KPI asboblar paneli va
  DNS/Gateway reliz skriptlarining barchasi bir xil JSON to'plamini o'qiydi.
- **Holatlar:** `active` (roʻyxatdan oʻtishga ruxsat berilgan), `paused` (vaqtinchalik yopiq),
  `revoked` (e'lon qilingan, ammo hozircha mavjud emas).

## Katalog sxemasi

| Maydon | Tur | Tavsif |
|-------|------|-------------|
| `suffix` | string | Boshlovchi nuqtali odam o‘qiy oladigan qo‘shimcha. |
| `suffix_id` | `u16` | `SuffixPolicyV1::suffix_id` daftarida saqlangan identifikator. |
| `status` | enum | `active`, `paused` yoki `revoked` ishga tushirishga tayyorligini tavsiflaydi. |
| `steward_account` | string | Boshqaruv uchun mas'ul hisob (registrator siyosati ilgaklariga mos keladi). |
| `fund_splitter_account` | string | `fee_split` bo'yicha marshrutlashdan oldin to'lovlarni qabul qiluvchi hisob. |
| `payment_asset_id` | string | Hisoblash uchun foydalanilgan aktiv (dastlabki kogorta uchun `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `min_term_years` / `max_term_years` | butun | Siyosatdan sotib olish muddati chegaralari. |
| `grace_period_days` / `redemption_period_days` | butun | Yangilanish xavfsizligi oynalari Torii tomonidan qo'llaniladi. |
| `referral_cap_bps` | butun | Boshqaruv tomonidan ruxsat etilgan maksimal tavsiyanomalar (asosiy nuqtalar). |
| `reserved_labels` | massiv | Boshqaruv tomonidan himoyalangan yorliq obyektlari `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | massiv | `label_regex`, `base_price`, `auction_kind` va davomiylik chegaralari bilan darajali obyektlar. |
| `fee_split` | ob'ekt | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` bazaviy nuqta bo'linishi. |
| `policy_version` | butun | Boshqaruv siyosatni tahrir qilganda monotonik hisoblagich ortadi. |

## Joriy katalog

| Suffiks | ID (`hex`) | Styuard | Fond ajratuvchi | Holati | To'lov aktivi | Yo'naltiruvchi chegara (bps) | Muddat (min – maks yillar) | Inoyat / To'lov (kunlar) | Narxlash darajalari (regex → asosiy narx / auktsion) | Zaxiralangan teglar | To'lovni taqsimlash (T/S/R/E bps) | Siyosat versiyasi |
|--------|------------|---------|---------------|--------|---------------|--------------------|--------------------------|--------------------------|--------------------------|-----------------------------------|
| `.sora` | `0x0001` | `i105...` | `i105...` | Faol | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1 – 5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → i105...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `i105...` | `i105...` | To'xtatildi | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1 – 3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → i105...`, `guardian → i105...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `i105...` | `i105...` | Bekor qilingan | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1 – 2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON parchasi

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "i105...",
      "payment_asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc", "amount": 120},
          "auction_kind": "vickrey_commit_reveal",
          "min_duration_years": 1,
          "max_duration_years": 5
        }
      ],
      "...": "see docs/examples/sns/suffix_catalog_v1.json for the full record"
    }
  ]
}
```

## Avtomatlashtirish bo'yicha eslatmalar

1. Operatorlarga tarqatishdan oldin JSON snapshotini yuklang va uni hashlang/imzolang.
2. Registrator asboblari `suffix_id`, muddat chegaralari va narxlarni ko'rsatishi kerak
   Agar so'rov `/v1/sns/*` bo'lsa, katalogdan.
3. DNS/Gateway yordamchilari GAR yaratishda zahiradagi yorliqli metamaʼlumotlarni oʻqiydi
   shablonlari, shuning uchun DNS javoblari boshqaruv elementlari bilan mos keladi.
4. KPI ilovasi ish yorliqlari yorlig'i boshqaruv panelidagi qo'shimcha metama'lumotlar bilan eksport qilinadi, shuning uchun ogohlantirishlar mos keladi
   ishga tushirish holati bu yerda qayd etilgan.