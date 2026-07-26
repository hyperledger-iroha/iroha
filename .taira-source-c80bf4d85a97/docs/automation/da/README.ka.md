---
lang: ka
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8757f0bf8699b532ece29437af953353526b3201b4b129ebec7d6bf5d224f038
source_last_modified: "2025-12-29T18:16:35.061402+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# მონაცემთა ხელმისაწვდომობის საფრთხის მოდელის ავტომატიზაცია (DA-1)

საგზაო რუკის პუნქტი DA-1 და `status.md` მოითხოვს დეტერმინისტულ ავტომატიზაციის ციკლს, რომელიც
აწარმოებს Norito PDP/PoTR საფრთხის მოდელის შეჯამებებს, რომლებიც გამოჩნდა
`docs/source/da/threat_model.md` და Docusaurus სარკე. ეს დირექტორია
აღბეჭდავს არტეფაქტებს, რომლებიც მითითებულია:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (რომელიც მუშაობს `scripts/docs/render_da_threat_model_tables.py`)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## ნაკადი

1. ** შექმენით ანგარიში **
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON რეზიუმე აღრიცხავს იმიტირებული რეპლიკაციის წარუმატებლობის სიჩქარეს, ცუნკერს
   ზღვრები და ნებისმიერი პოლიტიკის დარღვევა, რომელიც აღმოჩენილია PDP/PoTR აღმართის მიერ
   `integration_tests/src/da/pdp_potr.rs`.
2. ** Markdown ცხრილების რენდირება **
   ```bash
   make docs-da-threat-model
   ```
   ეს გადის `scripts/docs/render_da_threat_model_tables.py` გადასაწერად
   `docs/source/da/threat_model.md` და `docs/portal/docs/da/threat-model.md`.
3. **არტეფაქტის დაარქივება** JSON ანგარიშის (და სურვილისამებრ CLI ჟურნალის) კოპირებით
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. როცა
   მმართველობითი გადაწყვეტილებები ეყრდნობა კონკრეტულ გაშვებას, მოიცავს git commit ჰეშს და
   სიმულატორის თესლი ძმა `<timestamp>-metadata.md`-ში.

## მტკიცებულების მოლოდინი

- JSON ფაილები უნდა დარჩეს <100 KiB, რათა მათ შეძლონ git-ში ცხოვრება. უფრო დიდი აღსრულება
  კვალი მიეკუთვნება გარე მეხსიერებას - მიუთითეთ მათი ხელმოწერილი ჰეში მეტამონაცემებში
  საჭიროების შემთხვევაში შენიშვნა.
- თითოეულ დაარქივებულ ფაილში უნდა იყოს მითითებული თესლი, კონფიგურაციის გზა და სიმულატორის ვერსია
  გამეორებები შეიძლება განმეორდეს ზუსტად DA გამოშვების კარიბჭის აუდიტის დროს.
- დააბრუნეთ დაარქივებული ფაილი `status.md`-დან ან საგზაო რუქის ჩანაწერიდან, როდესაც
  DA-1 მიღების კრიტერიუმები წინ მიიწევს, რაც უზრუნველყოფს მიმომხილველებს გადაამოწმონ
  საწყისი ხაზი აღკაზმულობის ხელახალი გაშვების გარეშე.

## ვალდებულების შეჯერება (სექვენერის გამოტოვება)

გამოიყენეთ `cargo xtask da-commitment-reconcile` DA ingest ქვითრების შესადარებლად
DA ვალდებულების ჩანაწერები, სეკვენსერის გამოტოვების დაჭერა ან ხელყოფა:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- იღებს ქვითრებს Norito ან JSON ფორმაში და ვალდებულებებს
  `SignedBlockWire`, `.norito` ან JSON პაკეტები.
- წარუმატებელია, როდესაც რომელიმე ბილეთი აკლია ბლოკის ჟურნალს ან როდესაც ჰეშები განსხვავდება;
  `--allow-unexpected` უგულებელყოფს მხოლოდ ბლოკირებულ ბილეთებს, როდესაც თქვენ განზრახ ახორციელებთ ფარგლებს
  ქვითრის ნაკრები.
- მიამაგრეთ გამოშვებული JSON მართვის პაკეტებს/Alertmanager-ს გამოტოვებისთვის
  გაფრთხილებები; ნაგულისხმევად არის `artifacts/da/commitment_reconciliation.json`.

## პრივილეგიის აუდიტი (კვარტალური წვდომის მიმოხილვა)

გამოიყენეთ `cargo xtask da-privilege-audit` DA manifest/replay დირექტორიების სკანირებისთვის
(პლუს სურვილისამებრ დამატებითი ბილიკები) გამოტოვებული, არასაცნობარო ან მსოფლიო ჩასაწერად
ჩანაწერები:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- კითხულობს DA შიგთავსის ბილიკებს მოწოდებული Torii კონფიგურაციიდან და ამოწმებს Unix-ს
  ნებართვები, სადაც შესაძლებელია.
- მიუთითებს დაკარგული/არ არის დირექტორია/მსოფლიო-დასაწერი ბილიკები და აბრუნებს არა-ნულოვან გასასვლელს
  კოდი, როდესაც პრობლემები არსებობს.
- მოაწერეთ ხელი და მიამაგრეთ JSON პაკეტი (`artifacts/da/privilege_audit.json` by
  ნაგულისხმევი) პაკეტებისა და დაფების კვარტალური წვდომა-მიმოხილვა.