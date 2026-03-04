---
id: training-collateral
lang: ka
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Training Collateral
description: Curriculum, localization workflow, and annex evidence capture required by SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> სარკეები `docs/source/sns/training_collateral.md`. გამოიყენეთ ეს გვერდი ბრიფინგის დროს
> რეგისტრატორი, DNS, მეურვე და ფინანსური გუნდები ყოველი სუფიქსის გაშვების წინ.

## 1. სასწავლო გეგმის სურათი

| სიმღერა | მიზნები | წინასწარ წაკითხული |
|-------|------------|-----------|
| რეგისტრატორის ოპერაციები | მანიფესტების გაგზავნა, KPI დაფების მონიტორინგი, შეცდომების ესკალაცია. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS & კარიბჭე | წაისვით გამხსნელის ჩონჩხები, გაიმეორეთ ყინვები/დაბრუნება. | `sorafs/gateway-dns-runbook`, პირდაპირი რეჟიმის პოლიტიკის ნიმუშები. |
| მეურვეები და საბჭო | განახორციელეთ დავები, განაახლეთ მმართველობის დამატებები, ჟურნალის დანართები. | `sns/governance-playbook`, სტიუარდის ქულების ბარათები. |
| ფინანსები და ანალიტიკა | აღბეჭდეთ ARPU/ნაყარი მეტრიკა, გამოაქვეყნეთ დანართების პაკეტები. | `finance/settlement-iso-mapping`, KPI დაფა JSON. |

### მოდულის ნაკადი

1. **M1 — KPI ორიენტაცია (30 წთ): ** Walk suffix ფილტრები, ექსპორტი და გაქცეული
   გაყინვის მრიცხველები. მიწოდება: PDF/CSV კადრები SHA-256 დაიჯესტით.
2. **M2 — მანიფესტის სასიცოცხლო ციკლი (45 წთ):** რეგისტრატორის მანიფესტების შექმნა და დადასტურება,
   გადამწყვეტი ჩონჩხების გენერირება `scripts/sns_zonefile_skeleton.py`-ის საშუალებით. მიწოდება:
   git diff აჩვენებს ჩონჩხს + GAR მტკიცებულებებს.
3. **M3 — დავის წვრთნები (40 წთ):** მეურვის გაყინვის სიმულაცია + გასაჩივრება, გადაღება
   guardian CLI ჟურნალი `artifacts/sns/training/<suffix>/<cycle>/logs/`-ის ქვეშ.
4. **M4 — დანართის გადაღება (25წთ):** დაფის JSON ექსპორტი და გაშვება:

   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   მიწოდება: განახლებული დანართი Markdown + მარეგულირებელი + პორტალის შენიშვნების ბლოკები.

## 2. ლოკალიზაციის სამუშაო პროცესი

- ენები: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, I18NI000000000.
- თითოეული თარგმანი ცხოვრობს წყაროს ფაილის გვერდით
  (`docs/source/sns/training_collateral.<lang>.md`). განაახლეთ `status` +
  `translation_last_reviewed` განახლების შემდეგ.
- აქტივები თითო ენაზე ეკუთვნის
  `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (სლაიდები/, სამუშაო წიგნები/,
  ჩანაწერები/, ჟურნალები/).
- გაუშვით `python3 scripts/sync_docs_i18n.py --lang <code>` ინგლისურის რედაქტირების შემდეგ
  წყარო, რათა მთარგმნელებმა ნახონ ახალი ჰეში.

### მიწოდების ჩამონათვალი

1. განაახლეთ თარგმანის ნამუშევარი (`status: complete`) ლოკალიზების შემდეგ.
2. სლაიდების ექსპორტი PDF-ში და ატვირთეთ თითო ენაზე `slides/` დირექტორიაში.
3. ჩანაწერი ≤10 წთ KPI გავლა; ბმული ენის ნაკვთიდან.
4. ფაილის მართვის ბილეთი მონიშნული `sns-training`, რომელიც შეიცავს სლაიდს/სამუშაო წიგნს
   დაიჯესტები, ჩანაწერი ბმულები და მტკიცებულებები.

## 3. სასწავლო აქტივები

- სლაიდის მონახაზი: `docs/examples/sns_training_template.md`.
- სამუშაო წიგნის შაბლონი: `docs/examples/sns_training_workbook.md` (ერთი დამსწრე).
- მოწვევა + შეხსენებები: `docs/examples/sns_training_invite_email.md`.
- შეფასების ფორმა: `docs/examples/sns_training_eval_template.md` (პასუხები
  დაარქივებულია `artifacts/sns/training/<suffix>/<cycle>/feedback/` ქვეშ).

## 4. განრიგი და მეტრიკა

| ციკლი | ფანჯარა | მეტრიკა | შენიშვნები |
|-------|--------|---------|-------|
| 2026-03 | გამოაქვეყნეთ KPI მიმოხილვა | დასწრება %, დანართი დაიჯესტი შესულია | `.sora` + `.nexus` კოჰორტები |
| 2026-06 | Pre `.dao` GA | ფინანსური მზაობა ≥90% | პოლიტიკის განახლების ჩართვა |
| 2026-09 | გაფართოება | დავის სავარჯიშო <20 წთ, დანართი SLA ≤2 დღე | გასწორება SN-7 წახალისებით |

აღბეჭდეთ ანონიმური გამოხმაურება `docs/source/sns/reports/sns_training_feedback.md`-ში
ასე რომ, შემდგომ კოჰორტებს შეუძლიათ გააუმჯობესონ ლოკალიზაცია და ლაბორატორიები.