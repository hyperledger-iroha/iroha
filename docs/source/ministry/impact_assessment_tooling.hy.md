---
lang: hy
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

# Ազդեցության գնահատման գործիքակազմ (MINFO‑4b)

Ճանապարհային քարտեզի հղում՝ **MINFO-4b — Ազդեցության գնահատման գործիքակազմ։**  
Սեփականատեր՝ Կառավարման խորհուրդ / Վերլուծություն

Այս նշումը փաստում է `cargo xtask ministry-agenda impact` հրամանը, որն այժմ
արտադրում է ավտոմատացված հեշ-ընտանեկան տարբերությունը, որն անհրաժեշտ է հանրաքվեի փաթեթների համար: Այն
գործիքը սպառում է վավերացված օրակարգային խորհրդի առաջարկները, կրկնօրինակ գրանցամատյանը և
կամընտիր հերքող/քաղաքականության նկար, որպեսզի վերանայողները կարողանան հստակ տեսնել, թե որն է
մատնահետքերը նոր են, որոնք հակասում են գործող քաղաքականությանը և քանի գրառում
յուրաքանչյուր հաշ ընտանիք նպաստում է:

## Մուտքեր

1. **Օրակարգային առաջարկներ.** Հաջորդող մեկ կամ մի քանի ֆայլ
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md):
   Հստակ փոխանցեք դրանք `--proposal <path>`-ով կամ ուղղեք հրամանը a
   գրացուցակը `--proposal-dir <dir>`-ի և այդ ճանապարհի տակ գտնվող յուրաքանչյուր `*.json` ֆայլի միջոցով
   ներառված է։
2. **Կրկնօրինակեք ռեեստրը (ըստ ցանկության):** JSON ֆայլ, որը համապատասխանում է
   `docs/examples/ministry/agenda_duplicate_registry.json`. Հակամարտություններ են
   հաղորդում է `source = "duplicate_registry"`-ի ներքո:
3. **Քաղաքականության լուսանկար (ըստ ցանկության):** Թեթև մանիֆեստ, որը թվարկում է ամեն
   մատնահետք արդեն իսկ կիրառվել է ԳԱՐ/Նախարարության քաղաքականության կողմից: Բեռնիչը ակնկալում է
   ստորև ներկայացված սխեման (տես
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   ամբողջական նմուշի համար):

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

Ցանկացած գրառում, որի `hash_family:hash_hex` մատնահետքը համընկնում է առաջարկի թիրախին
հաղորդվում է `source = "policy_snapshot"`-ի ներքո՝ նշված `policy_id`-ով:

## Օգտագործում

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

Լրացուցիչ առաջարկները կարող են կցվել կրկնվող `--proposal` դրոշների միջոցով կամ
տրամադրելով գրացուցակ, որը պարունակում է հանրաքվեի ամբողջական փաթեթ.

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

Հրամանը տպում է ստեղծված JSON-ը stdout-ի համար, երբ `--out`-ը բաց է թողնվում:

## Ելք

Զեկույցը ստորագրված արտեֆակտ է (այն գրանցեք հանրաքվեի փաթեթի տակ
`artifacts/ministry/impact/` գրացուցակ) հետևյալ կառուցվածքով.

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

Կցեք այս JSON-ը յուրաքանչյուր հանրաքվեի դոսյեին չեզոք ամփոփագրի հետ միասին
Քվեարկության մասնակիցները, երդվյալ ատենակալները և կառավարման դիտորդները կարող են տեսնել պայթյունի ճշգրիտ շառավիղը
յուրաքանչյուր առաջարկ: Արդյունքը դետերմինիստական է (տեսակավորված ըստ հեշ ընտանիքի) և անվտանգ
ներառել CI/runbooks-ում; եթե կրկնօրինակ գրանցամատյանը կամ քաղաքականության պատկերը փոխվում է,
կրկնեք հրամանը և կցեք թարմացված արտեֆակտը մինչև քվեարկության բացումը:

> **Հաջորդ քայլը.** մուտքագրեք ստեղծված ազդեցության հաշվետվությունը
> [`cargo xtask ministry-panel packet`](referendum_packet.md), ուստի
> `ReferendumPacketV1` դոսյեն պարունակում է և՛ հեշ-ընտանիքի բաժանումը, և՛
> քննարկվող առաջարկի մանրամասն կոնֆլիկտների ցանկ: