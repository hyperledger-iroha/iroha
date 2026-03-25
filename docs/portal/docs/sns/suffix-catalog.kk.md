---
lang: kk
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

# Sora Name Service Suffix Catalog

SNS жол картасы әрбір бекітілген жұрнақ (SN-1/SN-2) қадағалайды. Бұл бет бейнені көрсетеді
операторлар тіркеушілерді, DNS шлюздерін немесе әмиянды басқаратын ақиқат көзі каталогы
құралдар күй құжаттарын сызып алмастан бірдей параметрлерді жүктей алады.

- **Лездік сурет:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Тұтынушылар:** `iroha sns policy`, SNS қосу жинақтары, KPI бақылау тақталары және
  DNS/Gateway шығарылым сценарийлерінің барлығы бірдей JSON бумасын оқиды.
- **Күйлер:** `active` (тіркеуге рұқсат етілген), `paused` (уақытша жабық),
  `revoked` (жарияланған, бірақ қазір қолжетімді емес).

## Каталог схемасы

| Өріс | |түрі Сипаттама |
|-------|------|-------------|
| `suffix` | жол | Бастауыш нүктесі бар адам оқитын жұрнақ. |
| `suffix_id` | `u16` | Идентификатор `SuffixPolicyV1::suffix_id` журналында сақталған. |
| `status` | enum | `active`, `paused` немесе `revoked` ұшыру дайындығын сипаттайды. |
| `steward_account` | жол | Басқаруға жауапты есептік жазба (тіркеуші саясатының ілгектеріне сәйкес келеді). |
| `fund_splitter_account` | жол | `fee_split` бойынша маршруттау алдында төлемдерді қабылдайтын тіркелгі. |
| `payment_asset_id` | жол | Есеп айырысу үшін пайдаланылған актив (бастапқы когорта үшін `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `min_term_years` / `max_term_years` | бүтін | Саясаттан сатып алу мерзімі шектеулері. |
| `grace_period_days` / `redemption_period_days` | бүтін | Жаңарту қауіпсіздік терезелері Torii арқылы бекітілген. |
| `referral_cap_bps` | бүтін | Басқару рұқсат берген ең көп жолдама (негізгі ұпай). |
| `reserved_labels` | массив | Басқарумен қорғалған белгі нысандары `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | массив | `label_regex`, `base_price`, `auction_kind` және ұзақтық шектері бар деңгейлі нысандар. |
| `fee_split` | нысан | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` негізгі нүктені бөлу. |
| `policy_version` | бүтін | Басқару саясатты өзгерткен сайын монотонды санауыш ұлғаяды. |

## Ағымдағы каталог

| Суффикс | ID (`hex`) | Стюард | Қорды бөлуші | Күй | Төлем активі | Референциялық шек (bps) | Мерзімі (min – max жылдар) | Рақымдылық / Өтеу (күндер) | Баға деңгейлері (regex → негізгі баға / аукцион) | Сақталған белгілер | Төлемді бөлу (T/S/R/E bps) | Саясат нұсқасы |
|--------|------------|---------|---------------|--------|---------------|--------------------|--------------------------|--------------------------|------------------------------------|
| `.sora` | `0x0001` | `i105...` | `i105...` | Белсенді | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1 – 5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → i105...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `i105...` | `i105...` | Кідіртілген | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1 – 3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → i105...`, `guardian → i105...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `i105...` | `i105...` | Күші жойылды | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1 – 2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON үзіндісі

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

## Автоматтандыру туралы ескертпелер

1. Операторларға таратпастан бұрын JSON суретін жүктеңіз және оған хэш/қол қойыңыз.
2. Тіркеушінің құралдары `suffix_id`, мерзім шектеулері мен бағаларды қамтуы керек.
   сұраныс `/v1/sns/*` соққанда каталогтан.
3. DNS/Gateway көмекшілері GAR жасау кезінде сақталған белгі метадеректерін оқиды
   үлгілер, сондықтан DNS жауаптары басқаруды басқару элементтерімен сәйкес келеді.
4. KPI қосымша тапсырмаларының тег бақылау тақтасы суффикс метадеректерімен экспортталады, осылайша ескертулер сәйкес келеді
   іске қосу күйі осында жазылған.