---
id: pq-ratchet-runbook
lang: ka
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
:::

## მიზანი

ეს სახელმძღვანელო ხელმძღვანელობს ცეცხლის საბურღი თანმიმდევრობას SoraNet-ის დადგმული პოსტ-კვანტური (PQ) ანონიმურობის პოლიტიკისთვის. ოპერატორები იმეორებენ როგორც დაწინაურებას (სტადია A -> სტადია B -> სტადია C) და კონტროლირებადი დაქვეითება უკან B/A სტადიაზე, როდესაც PQ მიწოდება შემცირდება. საბურღი ამოწმებს ტელემეტრიის კაუჭებს (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) და აგროვებს არტეფაქტებს ინციდენტების რეპეტიციის ჟურნალისთვის.

## წინაპირობები

- უახლესი `sorafs_orchestrator` ორობითი შესაძლებლობების წონით (შეასრულეთ `docs/source/soranet/reports/pq_ratchet_validation.md`-ში ნაჩვენები საბურღი მითითებით ან მის შემდეგ).
- წვდომა Prometheus/Grafana დასტაზე, რომელიც ემსახურება `dashboards/grafana/soranet_pq_ratchet.json`.
- ნომინალური მცველის დირექტორიის სურათი. მიიღეთ და გადაამოწმეთ ასლი ვარჯიშის დაწყებამდე:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

თუ წყაროს დირექტორია აქვეყნებს მხოლოდ JSON-ს, ხელახლა დაშიფვრეთ იგი Norito ორობითად `soranet-directory build`-ით, სანამ როტაციის დამხმარეები გაუშვით.

- დააფიქსირეთ მეტამონაცემები და წინასწარი სტადიის გამცემის როტაციის არტეფაქტები CLI-ით:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- შეცვალეთ ფანჯარა, რომელიც დამტკიცებულია ქსელის და დაკვირვების გამოძახების გუნდების მიერ.

## სარეკლამო ნაბიჯები

1. **სცენის აუდიტი**

   ჩაწერეთ საწყისი ეტაპი:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   ველით `anon-guard-pq` აქციამდე.

2. **გადადით B სტადიაზე (უმრავლესობის PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - დაელოდეთ >=5 წუთს მანიფესტების განახლებას.
   - Grafana-ში (`SoraNet PQ Ratchet Drill` დაფა) დაადასტურეთ „პოლიტიკის მოვლენები“ პანელი აჩვენებს `outcome=met`-ს `stage=anon-majority-pq`-ისთვის.
   - გადაიღეთ ეკრანის ანაბეჭდი ან პანელი JSON და მიამაგრეთ ინციდენტების ჟურნალში.

3. **C სტადიაზე დაწინაურება (მკაცრი PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - შეამოწმეთ `sorafs_orchestrator_pq_ratio_*` ჰისტოგრამის ტენდენცია 1.0-მდე.
   - დაადასტურეთ, რომ ბრაუნაუტ მრიცხველი ბრტყელი რჩება; წინააღმდეგ შემთხვევაში, მიჰყევით დაქვეითების ნაბიჯებს.

## დაქვეითება / ბრუნაუტ საბურღი

1. ** სინთეტიკური PQ დეფიციტის გამოწვევა **

   გამორთეთ PQ რელეები სათამაშო მოედნის გარემოში მცველის კატალოგის მხოლოდ კლასიკურ ჩანაწერებზე შეჭრით, შემდეგ გადატვირთეთ ორკესტრის ქეში:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **დააკვირდით ბრუნაუტ ტელემეტრიას**

   - საინფორმაციო დაფა: პანელის "Brownout Rate" მწვერვალები 0-ზე მაღლა.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` უნდა მოახსენოს `anonymity_outcome="brownout"` `anonymity_reason="missing_majority_pq"`-თან.

3. **დაქვეითება B/სტადიაზე A **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   თუ PQ მიწოდება ჯერ კიდევ არასაკმარისია, გადადით `anon-guard-pq`-ზე. სავარჯიშო დასრულდება მას შემდეგ, რაც ბრუნაუტ მრიცხველები დაფიქსირდება და აქციები ხელახლა იქნება შესაძლებელი.

4. ** დაცვის დირექტორიის აღდგენა **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## ტელემეტრია და არტეფაქტები

- **დაფა:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus გაფრთხილებები:** დარწმუნდით, რომ `sorafs_orchestrator_policy_events_total` გაფრთხილება დარჩება კონფიგურირებული SLO-ის ქვემოთ (<5% ნებისმიერი 10 წუთიანი ფანჯარაში).
- **ინციდენტების ჟურნალი:** დაურთეთ გადაღებული ტელემეტრიის ფრაგმენტები და ოპერატორის შენიშვნები `docs/examples/soranet_pq_ratchet_fire_drill.log`-ში.
- **ხელმოწერილი გადაღება:** გამოიყენეთ `cargo xtask soranet-rollout-capture`, რომ დააკოპიროთ საბურღი ჟურნალი და ანგარიშის დაფა `artifacts/soranet_pq_rollout/<timestamp>/`-ში, გამოთვალოთ BLAKE3-ის დაჯესტები და შექმნათ ხელმოწერილი `rollout_capture.json`.

მაგალითი:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

მიამაგრეთ გენერირებული მეტამონაცემები და ხელმოწერა მართვის პაკეტს.

## დაბრუნება

თუ საბურღი აღმოაჩენს PQ-ს რეალურ დეფიციტს, დარჩით A სტადიაზე, აცნობეთ Networking TL-ს და მიამაგრეთ შეგროვებული მეტრიკა და დაცვის დირექტორიას განსხვავება ინციდენტების ტრეკერს. გამოიყენეთ მცველის კატალოგის ექსპორტი, რომელიც ადრე იყო აღბეჭდილი ნორმალური სერვისის აღსადგენად.

:::tip რეგრესიის დაფარვა
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` უზრუნველყოფს ამ საბურღი სინთეზურ ვალიდაციას.
:::