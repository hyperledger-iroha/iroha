---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ad407370f375e45f0143f082b33a5ea61698825c8cd92dac402f656fb0f61a2
source_last_modified: "2026-01-22T16:26:46.524254+00:00"
translation_last_reviewed: 2026-02-07
id: dispute-revocation-runbook
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
translator: machine-google-reviewed
---

:::შენიშვნა კანონიკური წყარო
:::

## მიზანი

ეს სახელმძღვანელო ხელმძღვანელობს მმართველობით ოპერატორებს SoraFS სიმძლავრის დავების შეტანის, გაუქმების კოორდინაციისა და მონაცემთა ევაკუაციის განმსაზღვრელ დასრულებაში.

## 1. შეაფასეთ ინციდენტი

- **ტრიგერის პირობები:** SLA დარღვევის გამოვლენა (uptime/PoR უკმარისობა), რეპლიკაციის ნაკლებობა ან ბილინგის შეუთანხმებლობა.
- **დაადასტურეთ ტელემეტრია:** გადაიღეთ `/v1/sorafs/capacity/state` და `/v1/sorafs/capacity/telemetry` კადრები პროვაიდერისთვის.
- **აცნობეთ დაინტერესებულ მხარეებს:** შენახვის გუნდს (პროვაიდერის ოპერაციები), მმართველობის საბჭო (გადაწყვეტილების ორგანო), დაკვირვებადობა (დაფის განახლებები).

## 2. მოამზადეთ მტკიცებულებათა ნაკრები

1. შეაგროვეთ ნედლეული არტეფაქტები (ტელემეტრია JSON, CLI ჟურნალები, აუდიტორის ჩანაწერები).
2. ნორმალიზება დეტერმინისტულ არქივში (მაგალითად, ტარბოლი); ჩანაწერი:
   - BLAKE3-256 დაიჯესტი (`evidence_digest`)
   - მედიის ტიპი (`application/zip`, `application/jsonl` და ა.შ.)
   - ჰოსტინგის URI (ობიექტის საცავი, SoraFS პინი, ან Torii-წვდომა ბოლო წერტილი)
3. შეინახეთ ნაკრები მმართველობის მტკიცებულებების შეგროვების თაიგულში ჩაწერის ერთხელ წვდომით.

## 3. დავის შეტანა

1. შექმენით სპეციფიკაცია JSON `sorafs_manifest_stub capacity dispute`-ისთვის:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. გაუშვით CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<katakana-i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. გადახედეთ `dispute_summary.json` (დაადასტურეთ სახეობა, მტკიცებულება დაიჯესტი, დროის ანაბეჭდები).
4. გაგზავნეთ მოთხოვნა JSON-ზე Torii `/v1/sorafs/capacity/dispute` მმართველობითი ტრანზაქციის რიგის მეშვეობით. დააფიქსირეთ `dispute_id_hex` პასუხის მნიშვნელობა; ის ამაგრებს შემდგომი გაუქმების ქმედებებს და აუდიტის ანგარიშებს.

## 4. ევაკუაცია და გაუქმება

1. **Grace window:** აცნობეთ პროვაიდერს მოსალოდნელი გაუქმების შესახებ; ნებადართულია ჩამაგრებული მონაცემების ევაკუაცია, როდესაც პოლიტიკა ნებადართულია.
2. **შექმენით `ProviderAdmissionRevocationV1`:**
   - გამოიყენეთ `sorafs_manifest_stub provider-admission revoke` დამტკიცებული მიზეზით.
   - გადაამოწმეთ ხელმოწერები და გაუქმების დაიჯესტი.
3. **გამოაქვეყნეთ გაუქმება:**
   - გაგზავნეთ გაუქმების მოთხოვნა Torii-ზე.
   - დარწმუნდით, რომ პროვაიდერის რეკლამები დაბლოკილია (მოველით `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` ასვლას).
4. **განაახლეთ საინფორმაციო დაფები:** მონიშნეთ პროვაიდერი, როგორც გაუქმებული, მიუთითეთ დავის ID და დააკავშირეთ მტკიცებულებათა ნაკრები.

## 5. სიკვდილის შემდგომი და შემდგომი დაკვირვება

- ჩაწერეთ ვადები, ძირეული მიზეზი და გამოსწორების ქმედებები მმართველობის ინციდენტების ტრეკერში.
- განსაზღვრეთ რესტიტუცია (ფსონის შემცირება, საკომისიოს უკან დაბრუნება, კლიენტის თანხის დაბრუნება).
- დოკუმენტური სწავლება; განაახლეთ SLA ზღვრები ან საჭიროების შემთხვევაში მონიტორინგის სიგნალიზაცია.

## 6. საცნობარო მასალები

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (დავების განყოფილება)
- `docs/source/sorafs/provider_admission_policy.md` (გაუქმების სამუშაო პროცესი)
- დაკვირვებადობის დაფა: `SoraFS / Capacity Providers`

## საკონტროლო სია

- [ ] მტკიცებულებათა ნაკრები დაჭერილია და ჰეშირებულია.
- [ ] დავის დატვირთვა დამოწმებულია ადგილობრივად.
- [ ] Torii დავის ტრანზაქცია მიღებულია.
- [ ] გაუქმება შესრულებულია (თუ დამტკიცდება).
- [ ] Dashboards/Runbooks განახლებულია.
- [ ] მოკვდავი შეტანილია მმართველ საბჭოში.