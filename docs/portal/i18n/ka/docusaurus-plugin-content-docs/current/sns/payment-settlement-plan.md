---
id: payment-settlement-plan
lang: ka
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Payment & Settlement Plan
sidebar_label: Payment & settlement plan
description: Playbook for routing SNS registrar revenue, reconciling steward/treasury splits, and producing evidence bundles.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> კანონიკური წყარო: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

საგზაო რუკის ამოცანა **SN-5 — გადახდის და ანგარიშსწორების სერვისი** შემოაქვს დეტერმინისტიკას
გადახდის ფენა Sora Name Service-ისთვის. ყოველი რეგისტრაცია, განახლება ან თანხის დაბრუნება
უნდა ასხივოს სტრუქტურირებული Norito დატვირთვა, რათა ხაზინას, სტიუარდებმა და მმართველობამ შეძლონ
გაიმეორეთ ფინანსური ნაკადები ცხრილების გარეშე. ეს გვერდი ასუფთავებს სპეციფიკას
პორტალის აუდიტორიისთვის.

## შემოსავლის მოდელი

- საბაზისო გადასახადი (`gross_fee`) გამომდინარეობს რეგისტრატორის ფასების მატრიციდან.  
- ხაზინა იღებს `gross_fee × 0.70`, სტიუარდები იღებენ დარჩენილ მინუსს
  რეფერალური ბონუსები (10%).  
- არჩევითი შეფერხებები საშუალებას აძლევს მმართველობას შეაჩეროს სტიუარდის გადახდა დავების დროს.  
- დასახლების შეკვრა ავლენს `ledger_projection` ბლოკს ბეტონთან
  `Transfer` ISI-ები, რათა ავტომატიზაციამ შეძლოს XOR მოძრაობების განთავსება პირდაპირ Torii-ში.

## სერვისები და ავტომატიზაცია

| კომპონენტი | დანიშნულება | მტკიცებულება |
|-----------|---------|----------|
| `sns_settlementd` | ვრცელდება პოლიტიკა, ხელს აწერს პაკეტებს, ზედაპირებს `/v1/sns/settlements`. | JSON პაკეტი + ჰეში. |
| ანგარიშსწორების რიგი & მწერალი | Idempotent queue + ledger წარმდგენი, რომელსაც მართავს `iroha_cli app sns settlement ledger`. | შეფუთვის ჰეში ↔ tx ჰეშის მანიფესტი. |
| შერიგების სამუშაო | ყოველდღიური განსხვავება + ყოველთვიური ამონაწერი `docs/source/sns/reports/`-ის ქვეშ. | Markdown + JSON დაიჯესტი. |
| თანხის დაბრუნების მაგიდა | მთავრობის მიერ დამტკიცებული თანხის დაბრუნება `/settlements/{id}/refund`-ის მეშვეობით. | `RefundRecordV1` + ბილეთი. |

CI დამხმარეები ასახავს ამ ნაკადებს:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## დაკვირვება და მოხსენება

- დაფები: `dashboards/grafana/sns_payment_settlement.json` სახაზინო vs
  სტიუარდის ჯამები, რეფერალური გადახდები, რიგის სიღრმე და თანხის დაბრუნების შეყოვნება.
- გაფრთხილებები: `dashboards/alerts/sns_payment_settlement_rules.yml` მონიტორები მოლოდინშია
  ასაკი, შერიგების წარუმატებლობა და წიგნის დრიფტი.
- განცხადებები: ყოველდღიური დაიჯესტები (`settlement_YYYYMMDD.{json,md}`) გადადის ყოველთვიურად
  ანგარიშები (`settlement_YYYYMM.md`), რომლებიც აიტვირთება როგორც Git-ში, ასევე The
  მართვის ობიექტების მაღაზია (`s3://sora-governance/sns/settlements/<period>/`).
- მმართველობის პაკეტები აერთიანებს საინფორმაციო დაფებს, CLI ჟურნალებს და დამტკიცებებს საბჭოს წინაშე
  ხელმოწერა.

## გაშვების ჩამონათვალი

1. ციტატის პროტოტიპი + წიგნის დამხმარეები და გადაიღეთ დადგმის ნაკრები.
2. გაუშვით `sns_settlementd` რიგით + ჩაწერით, მავთულის დაფებით და ვარჯიშით
   გაფრთხილების ტესტები (`promtool test rules ...`).
3. მიწოდება თანხის დაბრუნების დამხმარე პლუს ყოველთვიური ამონაწერის შაბლონი; სარკეში არტეფაქტები
   `docs/portal/docs/sns/reports/`.
4. ჩაატარეთ პარტნიორის რეპეტიცია (დასახლებების მთელი თვე) და დაიჭირეთ
   მმართველობის ხმის მონიშვნა SN-5 დასრულებულად.

სქემის ზუსტი განმარტებებისთვის მიმართეთ წყაროს დოკუმენტს, ღია
კითხვები და მომავალი ცვლილებები.