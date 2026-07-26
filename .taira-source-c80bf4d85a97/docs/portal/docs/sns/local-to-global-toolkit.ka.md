---
lang: ka
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d4493e69ce57c4f691f368fb13c1bbe96e2c73991dfb39045753b5652d2f10a9
source_last_modified: "2026-01-28T17:11:30.702818+00:00"
translation_last_reviewed: 2026-02-07
title: Local → Global Address Toolkit
translator: machine-google-reviewed
---

ეს გვერდი ასახავს [`docs/source/sns/local_to_global_toolkit.md`](../../../source/sns/local_to_global_toolkit.md)
მონორეპოდან. ის აფუთებს CLI დამხმარეებსა და წიგნებს, რომლებიც საჭიროა საგზაო რუქის პუნქტით **ADDR-5c**.

## მიმოხილვა

- `scripts/address_local_toolkit.sh` ახვევს `iroha` CLI-ს, რომ აწარმოოს:
  - `audit.json` — სტრუქტურირებული გამომავალი `iroha tools address audit --format json`-დან.
  - `normalized.txt` — გადაკეთდა სასურველი i105 / მეორე საუკეთესო შეკუმშული (`sora`) ლიტერალები ლოკალური დომენის ყველა ამომრჩეველისთვის.
- დააწყვილეთ სკრიპტი მისამართების ჩაწერის დაფასთან (`dashboards/grafana/address_ingest.json`)
  და Alertmanager წესები (`dashboards/alerts/address_ingest_rules.yml`) Local-8-ის დასამტკიცებლად
  Local-12 cutover უსაფრთხოა. უყურეთ Local-8 და Local-12 შეჯახების პანელებს პლუს
  `AddressLocal8Resurgence`, `AddressLocal12Collision` და `AddressInvalidRatioSlo` გაფრთხილებები ადრე
  აშკარა ცვლილებების ხელშეწყობა.
- მიმართეთ [Address Display Guidelines] (address-display-guidelines.md) და
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) UX და ინციდენტზე რეაგირების კონტექსტში.

## გამოყენება

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format i105
```

პარამეტრები:

- `--format i105` `i105` გამოსასვლელად i105-ის ნაცვლად.
- `domainless output (default)` შიშველი ლიტერალების გამოსაცემად.
- `--audit-only`, რომ გამოტოვოთ კონვერტაციის ნაბიჯი.
- `--allow-errors` სკანირების გასაგრძელებლად, როდესაც არასწორი რიგები გამოჩნდება (შეესაბამება CLI ქცევას).

სცენარი წერს არტეფაქტის ბილიკებს გაშვების ბოლოს. მიამაგრეთ ორივე ფაილი
თქვენი ცვლილებების მართვის ბილეთი Grafana ეკრანის ანაბეჭდთან ერთად, რომელიც ადასტურებს ნულს
ლოკალური-8 აღმოჩენები და ნულოვანი ლოკალური-12 შეჯახება ≥30 დღის განმავლობაში.

## CI ინტეგრაცია

1. გაუშვით სკრიპტი სპეციალურ სამუშაოში და ატვირთეთ მისი შედეგები.
2. ბლოკი გაერთიანდება, როდესაც `audit.json` აცნობებს ლოკალურ სელექტორებს (`domain.kind = local12`).
   ნაგულისხმევი `true` მნიშვნელობით (მხოლოდ `false`-ზე გადახვევა დეველ/ტესტი კლასტერებზე, როდესაც
   რეგრესიების დიაგნოსტიკა) და დაამატეთ
   `iroha tools address normalize` CI-მდე ასე რეგრესია
   მცდელობები წარუმატებელი ხდება წარმოებამდე.

იხილეთ საწყისი დოკუმენტი დამატებითი დეტალებისთვის, მტკიცებულებათა საკონტროლო სიების ნიმუში და გამოშვების შენიშვნის ფრაგმენტი, რომელიც შეგიძლიათ ხელახლა გამოიყენოთ კლიენტებისთვის შეწყვეტის გამოცხადებისას.