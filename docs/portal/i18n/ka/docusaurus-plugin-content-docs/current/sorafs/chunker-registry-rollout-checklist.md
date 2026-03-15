---
id: chunker-registry-rollout-checklist
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
:::

# SoraFS რეესტრის გამოქვეყნების საკონტროლო სია

ეს ჩამონათვალი ასახავს ნაბიჯებს, რომლებიც საჭიროა ახალი chunker პროფილის გასაუმჯობესებლად ან
პროვაიდერის დაშვების ნაკრები განხილვიდან წარმოებამდე მმართველობის შემდეგ
ქარტია რატიფიცირებულია.

> **ფარგლები:** ვრცელდება ყველა გამოშვებაზე, რომელიც იცვლება
> `sorafs_manifest::chunker_registry`, პროვაიდერის დაშვების კონვერტები, ან
> კანონიკური მოწყობილობების შეკვრა (`fixtures/sorafs_chunker/*`).

## 1. წინასწარი ფრენის დადასტურება

1. განაახლეთ მოწყობილობები და გადაამოწმეთ დეტერმინიზმი:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. დაადასტურეთ დეტერმინიზმის ჰეშები
   `docs/source/sorafs/reports/sf1_determinism.md` (ან შესაბამისი პროფილი
   ანგარიში) ემთხვევა რეგენერირებული არტეფაქტებს.
3. დარწმუნდით, რომ `sorafs_manifest::chunker_registry` შედგენილია
   `ensure_charter_compliance()` გაშვებით:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. განაახლეთ წინადადების დოსიე:
   - `docs/source/sorafs/proposals/<profile>.json`
   - საბჭოს ოქმის ჩანაწერი `docs/source/sorafs/council_minutes_*.md` ქვეშ
   - დეტერმინიზმის ანგარიში

## 2. მმართველობის ხელმოწერა

1. წარუდგინეთ Tooling სამუშაო ჯგუფის ანგარიში და წინადადების დაიჯესტი Sora-ს
   პარლამენტის ინფრასტრუქტურის პანელი.
2. ჩაწერეთ დამტკიცების დეტალები
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. გამოაქვეყნეთ პარლამენტის მიერ ხელმოწერილი კონვერტი ნიშანთან ერთად:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. გადაამოწმეთ, რომ კონვერტი ხელმისაწვდომია მმართველობის მოპოვების დამხმარის მეშვეობით:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. დადგმის გაშვება

იხილეთ [მანიფესტის დადგმის სათამაშო წიგნი] (./staging-manifest-playbook)
ამ ნაბიჯების დეტალური აღწერა.

1. განათავსეთ Torii `torii.sorafs` აღმოჩენით და დაშვებით
   აღსრულება ჩართულია (`enforce_admission = true`).
2. გადაიტანეთ დამტკიცებული პროვაიდერის დაშვების კონვერტები დადგმის რეესტრში
   დირექტორია მითითებულ `torii.sorafs.discovery.admission.envelopes_dir`-ის მიერ.
3. შეამოწმეთ, რომ პროვაიდერის რეკლამები ვრცელდება აღმოჩენის API-ით:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. განახორციელეთ მანიფესტის/გეგმის საბოლოო წერტილები მმართველობის სათაურებით:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. დაადასტურეთ ტელემეტრიის დაფები (`torii_sorafs_*`) და გაფრთხილების წესები აცნობეთ
   ახალი პროფილი შეცდომების გარეშე.

## 4. წარმოების გავრცელება

1. გაიმეორეთ დადგმის ნაბიჯები წარმოების Torii კვანძების წინააღმდეგ.
2. გამოაცხადეთ აქტივაციის ფანჯარა (თარიღი/დრო, საშეღავათო პერიოდი, დაბრუნების გეგმა).
   ოპერატორი და SDK არხები.
3. გააერთიანეთ გამოშვების PR, რომელიც შეიცავს:
   - განახლებული მოწყობილობები და კონვერტი
   - დოკუმენტაციის ცვლილებები (წესდების მითითებები, დეტერმინიზმის ანგარიში)
   - საგზაო რუკა/სტატუსის განახლება
4. მონიშნეთ გამოშვება და დაარქივეთ ხელმოწერილი არტეფაქტები წარმოშობისთვის.

## 5. გამოშვების შემდგომი აუდიტი

1. დააფიქსირეთ საბოლოო მეტრიკა (აღმოჩენების რაოდენობა, წარმატების მაჩვენებელი, შეცდომა
   ჰისტოგრამები) გაშვებიდან 24 საათის შემდეგ.
2. განაახლეთ `status.md` მოკლე მიმოხილვით და ბმული დეტერმინიზმის ანგარიშთან.
3. შეიტანეთ ნებისმიერი შემდგომი დავალება (მაგ. პროფილის საავტორო დამატებითი ინსტრუქცია) აქ
   `roadmap.md`.