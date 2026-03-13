---
lang: ka
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2025-12-29T18:16:35.911952+00:00"
translation_last_reviewed: 2026-02-07
id: staging-manifest-playbook-ka
title: Staging Manifest Playbook
sidebar_label: Staging Manifest Playbook
description: Checklist for enabling the Parliament-ratified chunker profile on staging Torii deployments.
translator: machine-google-reviewed
slug: /sorafs/staging-manifest-playbook-ka
---

:::შენიშვნა კანონიკური წყარო
სარკეები `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. შეინახეთ ორივე ასლი გასწორებული გამოშვებებში.
:::

## მიმოხილვა

ეს სახელმძღვანელო გადის პარლამენტის მიერ რატიფიცირებული ჩუნკერის პროფილის ჩართვას ინსცენირების Torii განლაგების თაობაზე, სანამ დაიწყება ცვლილება წარმოებაში. იგი ვარაუდობს, რომ SoraFS მმართველობის წესდება რატიფიცირებულია და კანონიკური მოწყობილობები ხელმისაწვდომია საცავში.

## 1. წინაპირობები

1. სინქრონიზაცია კანონიკური მოწყობილობებისა და ხელმოწერების შესახებ:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. მოამზადეთ დაშვების კონვერტის დირექტორია, რომელსაც Torii წაიკითხავს გაშვებისას (მაგალითი გზა): `/var/lib/iroha/admission/sorafs`.
3. დარწმუნდით, რომ Torii კონფიგურაცია საშუალებას იძლევა აღმოჩენის ქეში და დაშვების აღსრულება:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. გამოაქვეყნეთ მისაღები კონვერტები

1. დააკოპირეთ დამტკიცებული პროვაიდერის დაშვების კონვერტები დირექტორიაში, მითითებულ `torii.sorafs.discovery.admission.envelopes_dir`-ის მიერ:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. გადატვირთეთ Torii (ან გაგზავნეთ SIGHUP, თუ ჩამტვირთავი შეფუთეთ ფრენის დროს გადატვირთვით).
3. ჩაწერეთ ჟურნალები მისაღები შეტყობინებებისთვის:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. აღმოჩენის გავრცელების დადასტურება

1. განათავსეთ ხელმოწერილი პროვაიდერის სარეკლამო დატვირთვა (Norito ბაიტი) მიერ წარმოებული თქვენი
   პროვაიდერის მილსადენი:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. გამოიკითხეთ აღმოჩენის საბოლოო წერტილი და დაადასტურეთ, რომ რეკლამა გამოჩნდება კანონიკური მეტსახელებით:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   დარწმუნდით, რომ `profile_aliases` მოიცავს `"sorafs.sf1@1.0.0"`, როგორც პირველ ჩანაწერს.

## 4. სავარჯიშო მანიფესტი და დაგეგმეთ საბოლოო წერტილები

1. მიიღეთ მანიფესტის მეტამონაცემები (საჭიროებს ნაკადის ჟეტონს, თუ დაშვება ძალაშია):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. შეამოწმეთ JSON გამომავალი და გადაამოწმეთ:
   - `chunk_profile_handle` არის `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` ემთხვევა დეტერმინიზმის ანგარიშს.
   - `chunk_digests_blake3` შეესაბამება რეგენერირებულ მოწყობილობებს.

## 5. ტელემეტრიული ჩეკები

- დაადასტურეთ, რომ Prometheus აჩვენებს ახალი პროფილის მეტრიკას:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- საინფორმაციო დაფებმა უნდა აჩვენოს დადგმის პროვაიდერი მოსალოდნელი მეტსახელის ქვეშ და შეინახოს ბრუნაუტის მრიცხველები ნულზე, სანამ პროფილი აქტიურია.

## 6. გაშვების მზადყოფნა

1. გადაიღეთ მოკლე ანგარიში URL-ებით, მანიფესტის ID-ით და ტელემეტრიის სნეპშოტით.
2. გააზიარეთ ანგარიში Nexus გაშვების არხში დაგეგმილი წარმოების აქტივაციის ფანჯარასთან ერთად.
3. გადადით წარმოების საკონტროლო სიაზე (ნაწილი 4 `chunker_registry_rollout_checklist.md`-ში), როგორც კი დაინტერესებული მხარეები მოაწერენ ხელს.

ამ სათამაშო წიგნის განახლების უზრუნველყოფა უზრუნველყოფს, რომ ყოველი ბლოკი/დაშვება მიჰყვება იგივე განმსაზღვრელ ნაბიჯებს დადგმასა და წარმოებაში.
