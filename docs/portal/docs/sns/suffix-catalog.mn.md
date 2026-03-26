---
lang: mn
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

# Сора нэрийн үйлчилгээний дагавар каталог

SNS замын зураг нь батлагдсан дагавар бүрийг (SN-1/SN-2) хянадаг. Энэ хуудас нь
Үнэний эх сурвалжийн каталог, ингэснээр операторууд бүртгэгч, DNS гарц эсвэл түрийвч ажиллуулдаг.
хэрэгсэл нь статусын баримтыг хусахгүйгээр ижил параметрүүдийг ачаалах боломжтой.

- **Ажийн зураг:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Хэрэглэгчид:** `iroha sns policy`, SNS залгах иж бүрдэл, KPI хяналтын самбар болон
  DNS/Gateway хувилбарын скриптүүд бүгд ижил JSON багцыг уншдаг.
- ** Статус:** `active` (бүртгүүлэхийг зөвшөөрсөн), `paused` (түр хаалгатай),
  `revoked` (зарласан боловч одоогоор байхгүй).

## Каталогийн схем

| Талбай | Төрөл | Тодорхойлолт |
|-------|------|-------------|
| `suffix` | мөр | Хүн унших боломжтой тэргүүлэх цэгтэй дагавар. |
| `suffix_id` | `u16` | `SuffixPolicyV1::suffix_id`-д дэвтэрт хадгалагдсан танигч. |
| `status` | тоо | `active`, `paused`, эсвэл `revoked` хөөргөхөд бэлэн байдлыг дүрсэлсэн. |
| `steward_account` | мөр | Удирдах ажлыг хариуцах данс (бүртгүүлэгчийн бодлогын дэгээтэй таарч байна). |
| `fund_splitter_account` | мөр | `fee_split` дагуу чиглүүлэхээс өмнө төлбөр хүлээн авдаг данс. |
| `payment_asset_id` | мөр | Төлбөр тооцоонд ашигласан хөрөнгө (анхны бүлэгт `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `min_term_years` / `max_term_years` | бүхэл тоо | Бодлогоос худалдан авах хугацааны хязгаар. |
| `grace_period_days` / `redemption_period_days` | бүхэл тоо | Шинэчлэх аюулгүй байдлын цонхыг Torii хэрэгжүүлсэн. |
| `referral_cap_bps` | бүхэл тоо | Засаглалын зөвшөөрөгдсөн хамгийн их лавлагаа (үндсэн оноо). |
| `reserved_labels` | массив | Засаглалаар хамгаалагдсан шошгоны объектууд `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | массив | `label_regex`, `base_price`, `auction_kind`, үргэлжлэх хугацааны хязгаар бүхий түвшний объектууд. |
| `fee_split` | объект | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` үндсэн цэгийн хуваагдал. |
| `policy_version` | бүхэл тоо | Удирдлага нь бодлогыг засах бүрт монотон тоолуур нэмэгддэг. |

## Одоогийн каталог

| дагавар | ID (`hex`) | Даамал | Сан хуваагч | Статус | Төлбөрийн хөрөнгө | Referral cap (bps) | Хугацаа (мин – макс жил) | Нигүүлсэл / гэтэлгэл (өдөр) | Үнийн шатлал (regex → үндсэн үнэ / дуудлага худалдаа) | Хадгалагдсан шошго | Төлбөрийг хуваах (T/S/R/E bps) | Бодлогын хувилбар |
|--------|------------|---------|---------------|--------|---------------|--------------------|-------------------------|--------------------------|------------------------------------|
| `.sora` | `0x0001` | `<i105-account-id>` | `<i105-account-id>` | Идэвхтэй | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1 – 5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → <i105-account-id>` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `<i105-account-id>` | `<i105-account-id>` | Түр зогссон | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1 – 3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → <i105-account-id>`, `guardian → <i105-account-id>` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `<i105-account-id>` | `<i105-account-id>` | Хүчингүй болгосон | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1 – 2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON ишлэл

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "<i105-account-id>",
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

## Автоматжуулалтын тэмдэглэл

1. Операторуудад түгээхээсээ өмнө JSON агшин зуурын зургийг ачаалж, хэш / гарын үсэг зурна уу.
2. Бүртгүүлэгчийн хэрэгсэл нь `suffix_id`, хугацааны хязгаарлалт, үнэ зэргийг тусгасан байх ёстой.
   хүсэлт `/v1/sns/*` хүрэх бүрт каталогоос.
3. DNS/Gateway туслахууд GAR үүсгэх үед нөөцлөгдсөн шошгоны мета өгөгдлийг уншдаг
   загварууд нь DNS хариултууд нь засаглалын хяналттай нийцдэг.
4. KPI хавсралтын ажлын шошгоны хяналтын самбарыг дагавар мета өгөгдөлтэй экспортлох тул анхааруулга нь дараахтай таарч байна
   хөөргөх төлөвийг энд тэмдэглэв.