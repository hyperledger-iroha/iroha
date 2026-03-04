---
lang: ka
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2025-12-29T18:16:35.978367+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ზემოქმედების შეფასების ინსტრუმენტი (MINFO‑4b)

საგზაო რუქის მითითება: **MINFO‑4b — ზემოქმედების შეფასების ინსტრუმენტი.**  
მფლობელი: მმართველობის საბჭო / ანალიტიკა

ეს ჩანაწერი ადასტურებს `cargo xtask ministry-agenda impact` ბრძანებას, რომელიც ახლა
აწარმოებს ავტომატური ჰეშ-ოჯახის განსხვავებას, რომელიც საჭიროა რეფერენდუმის პაკეტებისთვის. The
ინსტრუმენტი მოიხმარს დღის წესრიგის დადასტურებულ საბჭოს წინადადებებს, რეესტრის დუბლიკატს და
არასავალდებულო უარმყოფელი/პოლიტიკის სურათი, რათა მიმომხილველებმა ნახონ ზუსტად რომელი
თითის ანაბეჭდები ახალია, რომლებიც ეჯახება არსებულ პოლიტიკას და რამდენი ჩანაწერი
თითოეული ჰაშის ოჯახი ხელს უწყობს.

## შეყვანა

1. ** დღის წესრიგის წინადადებები. ** ერთი ან მეტი ფაილი, რომელიც მოჰყვება
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   გადასცეთ ისინი პირდაპირ `--proposal <path>`-ით ან მიუთითეთ ბრძანება a
   დირექტორია `--proposal-dir <dir>` და ყველა `*.json` ფაილის მეშვეობით ამ გზაზე
   შედის.
2. ** რეესტრის დუბლიკატი (არასავალდებულო). ** JSON ფაილის შესატყვისი
   `docs/examples/ministry/agenda_duplicate_registry.json`. კონფლიქტები არის
   მოხსენებული `source = "duplicate_registry"` ქვეშ.
3. **პოლიტიკის სნეპშოტი (სურვილისამებრ).** მსუბუქი მანიფესტი, რომელიც ჩამოთვლის ყველა
   თითის ანაბეჭდი უკვე აღსრულებულია GAR/მინისტრის პოლიტიკით. მტვირთავი ელის
   სქემა ნაჩვენებია ქვემოთ (იხ
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   სრული ნიმუშისთვის):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

ნებისმიერი ჩანაწერი, რომლის `hash_family:hash_hex` თითის ანაბეჭდი ემთხვევა წინადადების მიზანს
მოხსენებულია `source = "policy_snapshot"`-ით, მითითებულ `policy_id`-ით.

## გამოყენება

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

დამატებითი წინადადებები შეიძლება დაერთოს განმეორებითი `--proposal` დროშებით ან
აწვდის დირექტორიას, რომელიც შეიცავს მთელ რეფერენდუმის ჯგუფს:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

ბრძანება ბეჭდავს გენერირებულ JSON-ს stdout-ზე, როდესაც `--out` გამოტოვებულია.

## გამომავალი

ანგარიში არის ხელმოწერილი არტეფაქტი (ჩაწერეთ იგი რეფერენდუმის პაკეტის ქვეშ
`artifacts/ministry/impact/` დირექტორია) შემდეგი სტრუქტურით:

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

მიამაგრეთ ეს JSON ყველა რეფერენდუმის დოსიეს ნეიტრალურ შეჯამებასთან ერთად
პანელისტებს, ნაფიც მსაჯულებს და მმართველობის დამკვირვებლებს შეუძლიათ დაინახონ აფეთქების ზუსტი რადიუსი
თითოეული წინადადება. გამომავალი არის დეტერმინისტული (დახარისხებული ჰეშის ოჯახის მიხედვით) და უსაფრთხო
CI/runbooks-ში ჩართვა; თუ რეესტრის დუბლიკატი ან პოლიტიკის სურათი იცვლება,
გაიმეორეთ ბრძანება და მიამაგრეთ განახლებული არტეფაქტი კენჭისყრის დაწყებამდე.

> **შემდეგი ნაბიჯი:** შეიტანეთ გენერირებული ზემოქმედების ანგარიში
> [`cargo xtask ministry-panel packet`](referendum_packet.md) ასე რომ
> `ReferendumPacketV1` დოსიე შეიცავს ჰეშ-ოჯახის დაყოფას და
> განხილული წინადადების კონფლიქტების დეტალური სია.