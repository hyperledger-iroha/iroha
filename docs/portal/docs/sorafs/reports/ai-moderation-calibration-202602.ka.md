---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6066e49abf73a788490f9d6a738d90376b13dde4150509e81431508ffaaaf916
source_last_modified: "2025-12-29T18:16:35.199423+00:00"
translation_last_reviewed: 2026-02-07
title: AI Moderation Calibration Report (2026-02)
summary: Baseline calibration dataset, thresholds, and scoreboard for the first MINFO-1 governance release.
translator: machine-google-reviewed
---

# AI მოდერაციის კალიბრაციის ანგარიში - 2026 წლის თებერვალი

ეს ანგარიში შეფუთავს **MINFO-1**-ის საინაუგურაციო კალიბრაციის არტეფაქტებს. The
მონაცემთა ნაკრები, მანიფესტი და ქულების დაფა მომზადდა 2026-02-05, განხილული იქნა ორგანიზაციის მიერ
სამინისტროს საბჭო 2026-02-10 წწ
`912044`.

## მონაცემთა ნაკრების მანიფესტი

- **მონაცემთა მითითება:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **სლაგ:** `ai-moderation-calibration-202602`
- **ჩანაწერები:** manifest 480, ნაწილი 12,800, მეტამონაცემები 920, აუდიო 160
- **ლეიბლის ნაზავი:** უსაფრთხო 68%, საეჭვო 19%, ესკალაცია 13%
- **არტეფაქტის დაიჯესტი:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **დისტრიბუცია:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

სრული მანიფესტი ცხოვრობს `docs/examples/ai_moderation_calibration_manifest_202602.json`-ში
და შეიცავს მმართველობის ხელმოწერას და მორბენალ ჰეშს, რომელიც აღბეჭდილია გამოშვებისას
დრო.

## შედეგების შეჯამება

კალიბრაცია ჩატარდა opset 17-ით და დეტერმინისტული სათესლე მილსადენით. The
სრული დაფა JSON (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
აღრიცხავს ჰეშებსა და ტელემეტრიულ დისჯესტებს; ქვემოთ მოყვანილი ცხრილი ხაზს უსვამს ყველაზე მეტს
მნიშვნელოვანი მეტრიკა.

| მოდელი (ოჯახი) | ბრიერი | ECE | AUROC | სიზუსტე@კარანტინი | Recall@Escalate |
| -------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 უსაფრთხოება (ხედვა) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B უსაფრთხოება (მულტიმოდალური) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| აღქმის ანსამბლი (აღქმადი) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

კომბინირებული მეტრიკა: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. განაჩენი
კალიბრაციის ფანჯარაში განაწილება იყო 91.2%, საკარანტინო 6.8%,
ესკალაცია 2.0%, ემთხვევა მანიფესტში დაფიქსირებულ პოლიტიკის მოლოდინებს
შეჯამება. ცრუ დადებითი ჩამორჩენა დარჩა ნულზე, ხოლო დრიფტის ქულა (7.1%)
დაეცა 20%-იანი გაფრთხილების ზღვარს.

## ზღურბლები და შესვლა

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- მართვის მოძრაობა: `MINFO-2026-02-07`
- ხელი მოაწერა `ministry-council-seat-03`-ს `2026-02-10T11:33:12Z`-ზე

CI ინახავდა ხელმოწერილ პაკეტს `artifacts/ministry/ai_moderation/2026-02/`-ში
მოდერაციის მორბენალ ბინარებთან ერთად. მანიფესტის დაიჯესტი და ანგარიშის დაფა
ზემოთ მოყვანილი ჰეშები უნდა იყოს მითითებული აუდიტისა და აპელაციების დროს.

## დაფები და გაფრთხილებები

მოდერაციის SRE-ებმა უნდა შემოიტანონ Grafana დაფა
`dashboards/grafana/ministry_moderation_overview.json` და შესატყვისი
Prometheus გაფრთხილების წესები `dashboards/alerts/ministry_moderation_rules.yml`-ში
(ტესტი დაფარვა მოქმედებს `dashboards/alerts/tests/ministry_moderation_rules.test.yml`-ის ქვეშ).
ეს არტეფაქტები ავრცელებენ სიგნალიზაციას საკვების გადაყლაპვის, დრეიფტის და კარანტინის შესახებ
რიგის ზრდა, რომელიც აკმაყოფილებს მონიტორინგის მოთხოვნებს
[AI Moderation Runner Specification] (../../ministry/ai-moderation-runner.md).