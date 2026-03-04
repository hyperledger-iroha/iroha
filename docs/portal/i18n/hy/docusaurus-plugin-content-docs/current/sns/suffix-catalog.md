---
lang: hy
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

# Sora Անվան ծառայության վերջածանցների կատալոգ

SNS ճանապարհային քարտեզը հետևում է յուրաքանչյուր հաստատված վերջածանցին (SN-1/SN-2): Այս էջը արտացոլում է
ճշմարտության աղբյուրի կատալոգ, որպեսզի օպերատորները գործարկեն գրանցիչներ, DNS դարպասներ կամ դրամապանակ
գործիքավորումը կարող է բեռնել նույն պարամետրերը՝ առանց կարգավիճակի փաստաթղթերը քերելու:

- **Պատկեր.** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Սպառողներ.
  DNS/Gateway թողարկման սկրիպտները բոլորն էլ կարդում են նույն JSON փաթեթը:
- **Կարգավիճակները.** `active` (գրանցումները թույլատրվում են), `paused` (ժամանակավորապես փակված),
  `revoked` (հայտարարված է, բայց ներկայումս հասանելի չէ):

## Կատալոգի սխեման

| Դաշտային | Տեսակ | Նկարագրություն |
|-------|------|-------------|
| `suffix` | լարային | Մարդկանց համար ընթեռնելի վերջածանց՝ առաջատար կետով: |
| `suffix_id` | `u16` | Նույնացուցիչը պահված է `SuffixPolicyV1::suffix_id`-ում: |
| `status` | enum | `active`, `paused` կամ `revoked`, որոնք նկարագրում են գործարկման պատրաստությունը: |
| `steward_account` | լարային | Կառավարման համար պատասխանատու հաշիվ (համապատասխանում է ռեգիստրի քաղաքականության կեռիկներին): |
| `fund_splitter_account` | լարային | Հաշիվ, որը վճարումներ է ստանում `fee_split`-ով երթուղուց առաջ: |
| `payment_asset_id` | լարային | Հաշվարկի համար օգտագործվող ակտիվը (`xor#sora` սկզբնական խմբի համար): |
| `min_term_years` / `max_term_years` | ամբողջ թիվ | Գնեք ժամկետի սահմանները քաղաքականությունից: |
| `grace_period_days` / `redemption_period_days` | ամբողջ թիվ | Անվտանգության ապակիների նորացում, որն ուժի մեջ է մտնում Torii-ի կողմից: |
| `referral_cap_bps` | ամբողջ թիվ | Կառավարման կողմից թույլատրված առավելագույն ուղղորդում (հիմնական միավորներ): |
| `reserved_labels` | զանգված | Կառավարման կողմից պաշտպանված պիտակի օբյեկտներ `{label, assigned_to, release_at_ms, note}`: |
| `pricing` | զանգված | Շերտի օբյեկտներ `label_regex`, `base_price`, `auction_kind` և տևողության սահմաններով: |
| `fee_split` | օբյեկտ | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` բազային կետի բաժանում. |
| `policy_version` | ամբողջ թիվ | Միապաղաղ հաշվիչը ավելանում է, երբ կառավարումը խմբագրում է քաղաքականությունը: |

## Ընթացիկ կատալոգ

| վերջածանց | ID (`hex`) | Ստյուարդ | Ֆոնդերի բաժանիչ | Կարգավիճակը | Վճարային ակտիվ | Ուղղորդման գլխարկ (bps) | Ժամկետ (min – առավելագույն տարի) | Շնորհք / Փրկագին (օրեր) | Գնագոյացման մակարդակներ (regex → բազային գին / աճուրդ) | Պահպանված պիտակներ | Վճարի բաժանում (T/S/R/E bps) | Քաղաքականության տարբերակ |
|--------|-----------|-------------------------|--------|------ ---------|----------------------------------------------|-------- --------------------------------------------------------------- ---|--------------------------------------------------------------|
| `.sora` | `0x0001` | `ih58...` | `ih58...` | Ակտիվ | `xor#sora` | 500 | 1 – 5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → ih58...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `ih58...` | `ih58...` | Դադարեցված | `xor#sora` | 300 | 1 – 3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → ih58...`, `guardian → ih58...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `ih58...` | `ih58...` | Չեղյալ համարված | `xor#sora` | 0 | 1 – 2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON հատված

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "ih58...",
      "payment_asset_id": "xor#sora",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "xor#sora", "amount": 120},
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

## Ավտոմատացման նշումներ

1. Բեռնեք JSON նկարը և հաշեք/ստորագրեք այն նախքան օպերատորներին բաժանելը:
2. Գրանցող սարքավորումը պետք է բացահայտի `suffix_id`-ը, ժամկետի սահմանները և գնագոյացումը
   կատալոգից, երբ հարցումը հասնում է `/v1/sns/*`-ին:
3. DNS/Gateway-ի օգնականները GAR ստեղծելիս կարդում են վերապահված պիտակի մետատվյալները
   ձևանմուշներ, որպեսզի DNS-ի պատասխանները համահունչ մնան կառավարման վերահսկման հետ:
4. KPI հավելվածի աշխատատեղերի պիտակի վահանակը արտահանվում է վերջածանցով մետատվյալներով, որպեսզի ծանուցումները համապատասխանեն
   գործարկման վիճակը գրանցված է այստեղ: