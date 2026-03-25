---
lang: ka
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

SNS საგზაო რუკა აკონტროლებს ყველა დამტკიცებულ სუფიქსს (SN-1/SN-2). ეს გვერდი ასახავს
სიმართლის წყაროს კატალოგი, ასე რომ ოპერატორები აწარმოებენ რეგისტრატორებს, DNS კარიბჭეებს ან საფულეს
Tooling-ს შეუძლია იგივე პარამეტრების ჩატვირთვა სტატუსის დოკუმენტების გაფხეკის გარეშე.

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **მომხმარებლები:** `iroha sns policy`, SNS საბორტო ნაკრები, KPI დაფები და
  DNS/Gateway გამოშვების სკრიპტები ყველა ერთსა და იმავე JSON პაკეტს კითხულობს.
- **სტატუსები:** `active` (რეგისტრაცია დაშვებულია), `paused` (დროებით დახურული),
  `revoked` (გამოცხადებული, მაგრამ ამჟამად არ არის ხელმისაწვდომი).

## კატალოგის სქემა

| ველი | ტიპი | აღწერა |
|-------|------|-------------|
| `suffix` | სიმებიანი | ადამიანის წაკითხვადი სუფიქსი წამყვანი წერტილით. |
| `suffix_id` | `u16` | იდენტიფიკატორი ინახება წიგნში `SuffixPolicyV1::suffix_id`-ში. |
| `status` | inum | `active`, `paused`, ან `revoked`, რომელიც აღწერს გაშვების მზადყოფნას. |
| `steward_account` | სიმებიანი | მეურვეობაზე პასუხისმგებელი ანგარიში (შეესაბამება რეგისტრატორის პოლიტიკას). |
| `fund_splitter_account` | სიმებიანი | ანგარიში, რომელიც იღებს გადახდებს მარშრუტიზაციამდე `fee_split`-ით. |
| `payment_asset_id` | სიმებიანი | აქტივი გამოიყენება ანგარიშსწორებისთვის (`61CtjvNd9T3THAR65GsMVHr82Bjc` საწყისი კოჰორტისთვის). |
| `min_term_years` / `max_term_years` | მთელი რიცხვი | შეიძინეთ ვადის საზღვრები პოლისიდან. |
| `grace_period_days` / `redemption_period_days` | მთელი რიცხვი | დამცავი ფანჯრების განახლება იძულებითი Torii-ით. |
| `referral_cap_bps` | მთელი რიცხვი | მმართველობის მიერ დაშვებული მაქსიმალური რეფერალური ამოკვეთა (საბაზისო პუნქტები). |
| `reserved_labels` | მასივი | მმართველობით დაცული ეტიკეტის ობიექტები `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | მასივი | დონის ობიექტები `label_regex`, `base_price`, `auction_kind` და ხანგრძლივობის საზღვრები. |
| `fee_split` | ობიექტი | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` საბაზისო პუნქტის გაყოფა. |
| `policy_version` | მთელი რიცხვი | მონოტონური მრიცხველი იზრდება, როდესაც მმართველობა ასწორებს პოლიტიკას. |

## მიმდინარე კატალოგი

| სუფიქსი | ID (`hex`) | სტიუარდი | ფონდის გამყოფი | სტატუსი | გადახდის აქტივი | რეფერალური ქუდი (bps) | ვადა (მინ. – მაქს წლები) | მადლი / გამოსყიდვა (დღეები) | ფასების დონეები (რეგექსი → საბაზისო ფასი / აუქციონი) | დაცულია ეტიკეტები | საფასურის გაყოფა (T/S/R/E bps) | პოლიტიკის ვერსია |
|--------|-----------|--------|--------------|--------|------ ---------|-------------------|------------------------|-------- --------------------------------------------------------------- ---|------------------------------------------|---------------|
| `.sora` | `0x0001` | `i105...` | `i105...` | აქტიური | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1 – 5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → i105...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `i105...` | `i105...` | შეჩერებულია | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1 – 3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → i105...`, `guardian → i105...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `i105...` | `i105...` | გაუქმებულია | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1 – 2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON ამონაწერი

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

## ავტომატიზაციის შენიშვნები

1. ჩატვირთეთ JSON სნეპშოტი და გახეხეთ/ხელმოწერეთ იგი ოპერატორებისთვის გავრცელებამდე.
2. რეგისტრატორის ხელსაწყოები უნდა ასახავდეს `suffix_id`, ვადის ლიმიტებს და ფასებს
   კატალოგიდან, როდესაც მოთხოვნა მოხვდება `/v1/sns/*`.
3. DNS/Gateway-ის დამხმარეები კითხულობენ რეზერვირებული ლეიბლის მეტამონაცემებს GAR-ის გენერირებისას
   შაბლონები, რათა DNS პასუხები დარჩეს მართვის კონტროლთან შესაბამისობაში.
4. KPI დანართის სამუშაოების ტეგის საინფორმაციო დაფის ექსპორტი სუფიქსის მეტამონაცემებით, რათა გაფრთხილებები ემთხვეოდეს
   გაშვების მდგომარეობა ჩაწერილია აქ.