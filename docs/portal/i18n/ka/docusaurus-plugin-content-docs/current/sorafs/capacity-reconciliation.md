---
id: capacity-reconciliation
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Reconciliation
description: Nightly workflow for matching capacity fee ledgers to XOR transfer exports.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

საგზაო რუქის პუნქტი **SF-2c** ავალდებულებს, რომ ხაზინა დაამტკიცოს შესაძლებლობების საკომისიო წიგნი
ემთხვევა ყოველ ღამე შესრულებულ XOR გადარიცხვებს. გამოიყენეთ
`scripts/telemetry/capacity_reconcile.py` დამხმარე შედარება
`/v1/sorafs/capacity/state` სნეპშოტი შესრულებული გადაცემის პარტიასთან და
გამოსცემს Prometheus ტექსტური ფაილის მეტრიკას Alertmanager-ისთვის.

## წინაპირობები
- სიმძლავრის მდგომარეობის სნეპშოტი (`fee_ledger` ჩანაწერები) ექსპორტირებულია Torii-დან.
- ლეჯერის ექსპორტი იმავე ფანჯრისთვის (JSON ან NDJSON `provider_id_hex`-ით,
  `kind` = ანგარიშსწორება/ჯარიმა და `amount_nano`).
- გზა node_exporter ტექსტური ფაილების შემგროვებლისკენ, თუ გსურთ გაფრთხილებები.

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- გასასვლელი კოდები: `0` სუფთა მატჩზე, `1` როცა ანგარიშსწორებები/ჯარიმები აკლია
  ან ზედმეტად გადახდილი, `2` არასწორ შეყვანებზე.
- მიამაგრეთ JSON რეზიუმე + ჰეშები სახაზინო პაკეტს
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- როდესაც `.prom` ფაილი დაეშვება ტექსტის ფაილების კოლექციონერში, გაფრთხილება
  `SoraFSCapacityReconciliationMismatch` (იხ
  `dashboards/alerts/sorafs_capacity_rules.yml`) ისვრის, როცა არ არის,
  ზედმეტად გადახდილი, ან პროვაიდერის მოულოდნელი გადარიცხვები გამოვლინდა.

## გამომავალი
- თითო პროვაიდერის სტატუსები ანგარიშსწორებისა და ჯარიმების განსხვავებებით.
- ლიანდაგების სახით ექსპორტირებული ჯამები:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## მოსალოდნელი დიაპაზონი და ტოლერანტობა
- შერიგება ზუსტია: მოსალოდნელი და რეალური ანგარიშსწორება/ჯარიმის ნანო უნდა შეესაბამებოდეს ნულოვანი ტოლერანტობით. ნებისმიერი არანულოვანი განსხვავება უნდა იყოს გვერდის ოპერატორები.
- CI ამაგრებს 30-დღიან გაჟღენთვას ტევადობის საკომისიო წიგნისთვის (ტესტი `capacity_fee_ledger_30_day_soak_deterministic`) `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`-ზე. განაახლეთ დაიჯესტი მხოლოდ მაშინ, როდესაც იცვლება ფასების ან გაგრილების სემანტიკა.
- გაჟღენთის პროფილში (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) ჯარიმები ნულზე რჩება; წარმოებამ უნდა გამოიტანოს ჯარიმები მხოლოდ უტილიზაციის/გამოყენების/PoR სართულების დარღვევის შემთხვევაში და პატივს სცემს კონფიგურირებულ გაგრილებას თანმიმდევრულ დახრილობამდე.