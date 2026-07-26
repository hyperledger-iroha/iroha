---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c3d1e36d99e18b5986e911a6b240393a92140324142f9edb778d2f966b1712e
source_last_modified: "2026-01-05T09:28:11.909605+00:00"
translation_last_reviewed: 2026-02-07
id: runbooks-index
title: Operator Runbooks Index
description: Canonical entry point for the migrated SoraFS operator runbooks.
sidebar_label: Runbook Index
translator: machine-google-reviewed
---

> ასახავს მფლობელის დავთარს, რომელიც ცხოვრობს `docs/source/sorafs/runbooks/`-ის ქვეშ.
> ყოველი ახალი SoraFS ოპერაციების სახელმძღვანელო უნდა იყოს მიბმული აქ, როგორც კი ის გამოქვეყნდება
> პორტალის აშენება.

გამოიყენეთ ეს გვერდი, რათა გადაამოწმოთ, რომელმა წიგნებმა დაასრულეს მიგრაცია
წყაროს გზა და პორტალის ასლი, რათა მიმომხილველებმა პირდაპირ სასურველზე გადახტეს
სახელმძღვანელო ბეტა გადახედვისას.

## ბეტა წინასწარი გადახედვის ჰოსტი

DocOps ტალღამ ახლა დააწინაურა მიმომხილველის მიერ დამტკიცებული ბეტა წინასწარი გადახედვის მასპინძელი
`https://docs.iroha.tech/`. როდესაც მიუთითებს ოპერატორებს ან მიმომხილველებს მიგრაციაზე
runbook, მითითება, რომ ჰოსტის სახელი, რათა მათ განახორციელონ checksum-gated პორტალი
სნეპშოტი. გამოქვეყნების/დაბრუნების პროცედურები ცოცხალია
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Runbook | მფლობელ(ებ)ი | პორტალი ასლი | წყარო |
|---------|----------|------------|--------|
| Gateway & DNS kickoff | ქსელის TL, ოპერაციების ავტომატიზაცია, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS ოპერაციების სათამაშო წიგნი | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| შესაძლებლობების შეჯერება | ხაზინა / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| პინი რეესტრის ოპერაციები | ინსტრუმენტები WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| კვანძის ოპერაციების ჩამონათვალი | შენახვის გუნდი, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| დავის და გაუქმების სახელმძღვანელო | მმართველობის საბჭო | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| მანიფესტის დადგმა სათამაშო წიგნი | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| ტაიკაის წამყვანის დაკვირვებადობა | მედია პლატფორმა WG / DA პროგრამა / ქსელში TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## შემოწმების სია

- [x] პორტალის აშენების ბმულები ამ ინდექსზე (გვერდითა ზოლის ჩანაწერი).
- [x] ყოველი მიგრირებული წიგნში ჩამოთვლილია კანონიკური წყაროს გზა რეცენზენტების შესანარჩუნებლად
  გასწორებულია დოკუმენტის განხილვისას.
- [x] DocOps-ის წინასწარი გადახედვის მილსადენის ბლოკები ერწყმის, როდესაც ჩამოთვლილი runbook აკლია
  პორტალის გამომავალიდან.

მომავალი მიგრაციები (მაგ., ახალი ქაოსური წვრთნები ან მმართველობის დანართები) უნდა დაემატოს ა
გადადით ზემოთ ცხრილში და განაახლეთ ჩაშენებული DocOps საკონტროლო სია
`docs/examples/docs_preview_request_template.md`.