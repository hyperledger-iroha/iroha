---
lang: ka
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-12-29T18:16:35.079313+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS სასწავლო სლაიდის შაბლონი

ეს მარკდაუნის მონახაზი ასახავს სლაიდებს, რომლებსაც ფასილიტატორები უნდა მოერგონ
მათი ენობრივი კოჰორტები. დააკოპირეთ ეს სექციები Keynote/PowerPoint/Google-ში
საჭიროების შემთხვევაში სლაიდები და ლოკალიზებულია პუნქტების, ეკრანის ანაბეჭდების და დიაგრამების ლოკალიზება.

## სათაურის სლაიდი
- პროგრამა: "Sora Name Service inboarding"
- სუბტიტრები: მიუთითეთ სუფიქსი + ციკლი (მაგ., `.sora — 2026‑03`)
- წამყვანები + კუთვნილება

## KPI ორიენტაცია
- სკრინშოტი ან ჩაშენებული `docs/portal/docs/sns/kpi-dashboard.md`
- პუნქტების სია სუფიქსის ფილტრების ახსნით, ARPU ცხრილი, გაყინვის ტრეკერი
- მოწოდებები PDF/CSV ექსპორტისთვის

## მანიფესტი სასიცოცხლო ციკლი
- დიაგრამა: რეგისტრატორი → Torii → მმართველობა → DNS / კარიბჭე
- ნაბიჯების მითითება `docs/source/sns/registry_schema.md`
- მაგალითი მანიფესტი ამონაწერი ანოტაციებით

## სადავო და გაყინული წვრთნები
- ნაკადის დიაგრამა მეურვის ჩარევისთვის
- საკონტროლო სიის მითითება `docs/source/sns/governance_playbook.md`
- მაგალითი გაყინვის ბილეთების ვადები

## დანართის აღება
- ბრძანების ნაწყვეტი, რომელიც აჩვენებს `cargo xtask sns-annex ... --portal-entry ...`
- შეხსენება Grafana JSON არქივისთვის `artifacts/sns/regulatory/<suffix>/<cycle>/` ქვეშ
- ბმული `docs/source/sns/reports/.<suffix>/<cycle>.md`-ზე

## შემდეგი ნაბიჯები
- ტრენინგის გამოხმაურების ბმული (იხ. `docs/examples/sns_training_eval_template.md`)
- Slack/Matrix არხის სახელურები
- მომავალი საეტაპო თარიღები