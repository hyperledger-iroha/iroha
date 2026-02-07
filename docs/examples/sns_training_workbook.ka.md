---
lang: ka
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS სასწავლო სამუშაო წიგნის შაბლონი

გამოიყენეთ ეს სამუშაო წიგნი, როგორც კანონიკური მასალა თითოეული სასწავლო ჯგუფისთვის. ჩანაცვლება
ჩანაცვლების დამფუძნებლები (`<...>`) დამსწრეებისთვის გავრცელებამდე.

## სესიის დეტალები
- სუფიქსი: `<.sora | .nexus | .dao>`
- ციკლი: `<YYYY-MM>`
- ენა: `<ar/es/fr/ja/pt/ru/ur>`
- ფასილიტატორი: `<name>`

## ლაბორატორია 1 - KPI ექსპორტი
1. გახსენით პორტალის KPI დაფა (`docs/portal/docs/sns/kpi-dashboard.md`).
2. გაფილტვრა სუფიქსით `<suffix>` და დროის დიაპაზონით `<window>`.
3. PDF + CSV სნეპშოტების ექსპორტი.
4. ჩაწერეთ SHA-256 ექსპორტირებული JSON/PDF აქ: `______________________`.

## ლაბორატორია 2 - მანიფესტის საბურღი
1. მიიღეთ ნიმუშის მანიფესტი `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json`-დან.
2. დამოწმება `cargo run --bin sns_manifest_check -- --input <file>`-ით.
3. შექმენით გადამწყვეტი ჩონჩხი `scripts/sns_zonefile_skeleton.py`-ით.
4. ჩასვით განსხვავების შეჯამება:
   ```
   <git diff output>
   ```

## ლაბორატორია 3 - დავის სიმულაცია
1. გამოიყენეთ guardian CLI გაყინვის დასაწყებად (საქმის ID `<case-id>`).
2. ჩაწერეთ დავის ჰეში: `______________________`.
3. ატვირთეთ მტკიცებულებათა ჟურნალი `artifacts/sns/training/<suffix>/<cycle>/logs/`-ზე.

## ლაბორატორია 4 — დანართის ავტომატიზაცია
1. გაიყვანეთ Grafana დაფა JSON და დააკოპირეთ `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`-ში.
2. სირბილი:
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
3. ჩასვით დანართის ბილიკი + SHA-256 გამომავალი: `________________________________`.

## უკუკავშირის შენიშვნები
-რა იყო გაუგებარი?
- რომელი ლაბორატორიები მუშაობდნენ დროთა განმავლობაში?
- ხელსაწყოების ხარვეზები შეინიშნება?

დააბრუნეთ დასრულებული სამუშაო წიგნები ფასილიტატორს; ქვეშ ეკუთვნიან
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.