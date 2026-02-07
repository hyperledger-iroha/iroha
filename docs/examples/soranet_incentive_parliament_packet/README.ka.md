---
lang: ka
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraNet სარელეო წამახალისებელი პარლამენტის პაკეტი

ეს ნაკრები ასახავს არტეფაქტებს, რომლებიც საჭიროა სორას პარლამენტის დასამტკიცებლად
ავტომატური სარელეო გადახდა (SNNet-7):

- `reward_config.json` - Norito-სერიული ჯილდოს ძრავის კონფიგურაცია, მზად
  გადაყლაპვა `iroha app sorafs incentives service init`-ით. The
  `budget_approval_id` ემთხვევა მმართველობის წუთებში ჩამოთვლილ ჰეშს.
- `shadow_daemon.json` - ბენეფიციარის და ობლიგაციების რუკების მოხმარება განმეორებით
  აღკაზმულობა (`shadow-run`) და წარმოების დემონი.
- `economic_analysis.md` - სამართლიანობის შეჯამება 2025-10 წლებისთვის -> 2025-11
  ჩრდილის სიმულაცია.
- `rollback_plan.md` - ოპერატიული სათამაშო წიგნი ავტომატური გადახდების გამორთვისთვის.
- დამხმარე არტეფაქტები: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## მთლიანობის შემოწმება

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

შეადარეთ დაიჯესტები პარლამენტის ოქმებში დაფიქსირებულ მნიშვნელობებს. გადაამოწმეთ
ჩრდილში გაშვებული ხელმოწერა, როგორც ეს აღწერილია
`docs/source/soranet/reports/incentive_shadow_run.md`.

## პაკეტის განახლება

1. განაახლეთ `reward_config.json`, როდესაც ჯილდოს წონა, საბაზისო გადახდა ან
   დამტკიცების ჰეშის ცვლილება.
2. ხელახლა გაუშვით 60-დღიანი ჩრდილის სიმულაცია, განაახლეთ `economic_analysis.md`
   ახალი აღმოჩენები და ჩაიდინეთ JSON + გამოყოფილი ხელმოწერის წყვილი.
3. განახლებული პაკეტი წარუდგინეთ პარლამენტს ობსერვატორიის დაფასთან ერთად
   ექსპორტი განახლებული დამტკიცების მოთხოვნისას.