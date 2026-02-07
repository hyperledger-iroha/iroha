---
lang: ka
direction: ltr
source: docs/source/ministry/referendum_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 922d972376b67a2f8c0c03ded95db6576e16e229e4bcb62d920b0ffda49c93ac
source_last_modified: "2025-12-29T18:16:35.980526+00:00"
translation_last_reviewed: 2026-02-07
title: Referendum Packet Workflow (MINFO-4)
summary: Produce the complete referendum dossier (`ReferendumPacketV1`) combining the proposal, neutral summary, sortition artefacts, and impact report.
translator: machine-google-reviewed
---

# რეფერენდუმის პაკეტის სამუშაო პროცესი (MINFO-4)

საგზაო რუკის პუნქტი **MINFO-4 — მიმოხილვის პანელში და რეფერენდუმის სინთეზატორი** არის ახლა
შესრულებულია ახალი `ReferendumPacketV1` Norito სქემით პლუს CLI დამხმარეებით
ქვემოთ აღწერილი. სამუშაო პროცესი აერთიანებს ყველა არტეფაქტს, რომელიც საჭიროა პოლიტიკის ჟიურისთვის
ხმას აძლევს ერთ JSON დოკუმენტს მმართველობა, აუდიტორები და გამჭვირვალობა
პორტალებს შეუძლიათ მტკიცებულებების განმსაზღვრელი გამეორება.

## შეყვანა

1. ** დღის წესრიგის წინადადება ** — იგივე JSON გამოიყენება `cargo xtask ministry-agenda validate`-ისთვის.
2. **მოხალისეების ბრიფინგები** — კურირებული მონაცემთა ნაკრები, რომელიც წარმოიქმნება ლინტების მეშვეობით
   `cargo xtask ministry-transparency volunteer-validate`.
3. **AI მოდერაციის მანიფესტი** — მმართველობის ხელმოწერილი `ModerationReproManifestV1`.
4. **დახარისხების შეჯამება** — დეტერმინისტული არტეფაქტი, რომელიც გამოუშვა
   `cargo xtask ministry-agenda sortition`. JSON მოყვება
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md), რათა მმართველობამ შეძლოს
   რეპროდუცირება POP სნეპშოტის დაიჯესტი და ლოდინის სიის/შეცდომის გაყვანილობა.
5. **გავლენის ანგარიში ** — ჰეშ-ოჯახი/ანგარიში გენერირებული მეშვეობით
   `cargo xtask ministry-agenda impact`.

## CLI გამოყენება

```bash
cargo xtask ministry-panel packet \
  --proposal artifacts/ministry/proposals/AC-2026-001.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --sortition artifacts/ministry/agenda_sortition_2026Q1.json \
  --impact artifacts/ministry/impact/AC-2026-001.json \
  --summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/referendum_packets/AC-2026-001.json
```

`packet` ქვებრძანება აწარმოებს ნეიტრალურ შემაჯამებელ სინთეზატორს (MINFO-4a), ხელახლა იყენებს
არსებული მოხალისეთა მოწყობილობები და ამდიდრებს გამომუშავებას:

- `ReferendumSortitionEvidence` - ალგორითმი, თესლი და დისჯესტები
  დახარისხების არტეფაქტი.
- `ReferendumPanelist[]` - საბჭოს თითოეული არჩეული წევრი პლუს Merkle-ის მტკიცებულება
  საჭიროა მათი გათამაშების აუდიტი.
- `ReferendumImpactSummary` — თითო ჰეშ-ოჯახის ჯამები და კონფლიქტების სიები
  ზემოქმედების ანგარიში.

გამოიყენეთ `--summary-out`, როდესაც ჯერ კიდევ გჭირდებათ დამოუკიდებელი `ReviewPanelSummaryV1`
ფაილი; წინააღმდეგ შემთხვევაში, პაკეტში ჩაშენებულია შეჯამება `review_summary` ქვეშ.

## გამომავალი სტრუქტურა

`ReferendumPacketV1` ცხოვრობს
`crates/iroha_data_model/src/ministry/mod.rs` და ხელმისაწვდომია SDK-ებში.
ძირითადი სექციები მოიცავს:

- `proposal` — ორიგინალური `AgendaProposalV1` ობიექტი.
- `review_summary` — MINFO-4a-ს მიერ გამოშვებული დაბალანსებული რეზიუმე.
- `sortition` / `panelists` — განმეორებადი მტკიცებულებები მჯდომარე საბჭოსთვის.
- `impact_summary` — დუბლიკატი/პოლიტიკის კონფლიქტის მტკიცებულება ჰეშის ოჯახზე.

იხილეთ `docs/examples/ministry/referendum_packet_example.json` სრული ნიმუშისთვის.
მიამაგრეთ გენერირებული პაკეტი ყველა რეფერენდუმის დოსიეს ხელმოწერილი AI-სთან ერთად
მანიფესტი და გამჭვირვალობის არტეფაქტები, რომლებიც მითითებულია მაჩვენებლების განყოფილებაში.