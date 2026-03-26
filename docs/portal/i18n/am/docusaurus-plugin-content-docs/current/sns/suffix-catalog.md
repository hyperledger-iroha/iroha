---
lang: am
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Name Service Suffix Catalog
sidebar_label: Suffix catalog
description: Canonical allowlist of SNS suffixes, stewards, and pricing knobs for `.sora`, `.nexus`, and `.dao`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# የሶራ ስም አገልግሎት ቅጥያ ካታሎግ

የኤስኤንኤስ ፍኖተ ካርታ እያንዳንዱን የጸደቀ ቅጥያ (SN-1/SN-2) ይከታተላል። ይህ ገጽ የሚያንጸባርቀው
የእውነት ምንጭ ካታሎግ ስለዚህ ኦፕሬተሮች ሬጅስትራሮችን፣ የዲ ኤን ኤስ መግቢያ መንገዶችን ወይም የኪስ ቦርሳን ያካሂዳሉ
መሳሪያ ማድረግ የሁኔታ ሰነዶችን ሳይሰርዝ ተመሳሳይ መለኪያዎችን ሊጭን ይችላል።

- ** ቅጽበታዊ እይታ:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- ** ሸማቾች፡** I18NI0000004X፣ የኤስኤንኤስ የመሳፈሪያ ኪቶች፣ የ KPI ዳሽቦርዶች እና
  ዲ ኤን ኤስ/የጌትዌይ መልቀቂያ ስክሪፕቶች ሁሉም አንድ አይነት የJSON ቅርቅብ ያነባሉ።
- ** ሁኔታዎች:** `active` (ምዝገባዎች ተፈቅደዋል)፣ `paused` (ለጊዜው የተዘጋ)፣
  `revoked` (የታወጀ ግን በአሁኑ ጊዜ አይገኝም)።

## ካታሎግ እቅድ

| መስክ | አይነት | መግለጫ |
|-------|-------|-----------|
| `suffix` | ሕብረቁምፊ | ሰው ሊነበብ የሚችል ቅጥያ ከመሪ ነጥብ ጋር። |
| `suffix_id` | `u16` | መለያ በ`SuffixPolicyV1::suffix_id` ውስጥ ተከማችቷል። |
| `status` | enum | `active`፣ `paused`፣ ወይም `revoked` የማስጀመሪያ ዝግጁነትን የሚገልጽ። |
| `steward_account` | ሕብረቁምፊ | ለመጋቢነት ኃላፊነት ያለው መለያ (ተዛማጆች የመዝጋቢ ፖሊሲ መንጠቆዎች)። |
| `fund_splitter_account` | ሕብረቁምፊ | በ`fee_split` ከመሄዱ በፊት ክፍያዎችን የሚቀበል መለያ። |
| `payment_asset_id` | ሕብረቁምፊ | ለመቋቋሚያ ጥቅም ላይ የዋለ ንብረት (`61CtjvNd9T3THAR65GsMVHr82Bjc` ለመጀመሪያው ቡድን)። |
| `min_term_years` / `max_term_years` | ኢንቲጀር | ከፖሊሲው የግዢ ጊዜ ገደቦች. |
| `grace_period_days` / `redemption_period_days` | ኢንቲጀር | በTorii የተተገበሩ የእድሳት ደህንነት መስኮቶች። |
| `referral_cap_bps` | ኢንቲጀር | በአስተዳደር የሚፈቀደው ከፍተኛው የሪፈራል ቀረጻ (መሰረታዊ ነጥቦች)። |
| `reserved_labels` | ድርድር | በመንግስት የተጠበቁ የመለያ እቃዎች `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | ድርድር | የደረጃ ቁሶች ከ I18NI0000029X፣ `base_price`፣ `auction_kind` እና የቆይታ ወሰኖች። |
| `fee_split` | እቃ | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` መሠረት-ነጥብ መለያየት። |
| `policy_version` | ኢንቲጀር | አስተዳደር ፖሊሲውን በሚያስተካክልበት ጊዜ ሞኖቶኒክ ቆጣሪ ይጨምራል። |

## የአሁኑ ካታሎግ

| ቅጥያ | መታወቂያ (`hex`) | መጋቢ | ፈንድ መከፋፈያ | ሁኔታ | የክፍያ ንብረት | ሪፈራል ካፕ (bps) | ጊዜ (ደቂቃ-  ከፍተኛ ዓመታት) | ጸጋ / ቤዛ (ቀናት) | የዋጋ አሰጣጥ ደረጃዎች (regex → መነሻ ዋጋ / ጨረታ) | የተያዙ መለያዎች | ክፍያ ተከፍሎ (T/S/R/E bps) | የፖሊሲ ስሪት |
|--------|------------|------------|-----------|----------|-----------|------------|
| `.sora` | `0x0001` | `soraカタカナ...` | `soraカタカナ...` | ንቁ | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1 – 5 | 30/60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → soraカタカナ...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `soraカタカナ...` | `soraカタカナ...` | ባለበት ቆሟል | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1 – 3 | 15/30 | `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → soraカタカナ...`, `guardian → soraカタカナ...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `soraカタカナ...` | `soraカタカナ...` | ተሽሯል | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1 – 2 | 30/30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON የተቀነጨበ

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "soraカタカナ...",
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

## ራስ-ሰር ማስታወሻዎች

1. ለኦፕሬተሮች ከማሰራጨትዎ በፊት የJSON ቅጽበተ-ፎቶውን ይጫኑ እና ሃሽ/ይፈርሙ።
2. የመመዝገቢያ መሣሪያ `suffix_id`፣ የጊዜ ገደቦች እና የዋጋ አወጣጥ ላይ መታየት አለበት።
   ጥያቄ `/v1/sns/*` ሲደርስ ከካታሎግ።
3. የዲ ኤን ኤስ/ጌትዌይ አጋዦች GAR በሚያመነጩበት ጊዜ የተያዘውን መለያ ሜታዳታ ያንብቡ
   የዲ ኤን ኤስ ምላሾች ከአስተዳደር መቆጣጠሪያዎች ጋር እንዲጣጣሙ አብነቶች።
4. KPI annex jobs ዳሽቦርድ ወደ ውጭ የሚላኩ ማስታወቂያዎች በቅጥያ ሜታዳታ ታግ ያደርጉታል ስለዚህም ማንቂያዎች ከዚህ ጋር ይዛመዳሉ
   የማስጀመሪያ ሁኔታ እዚህ ተመዝግቧል።