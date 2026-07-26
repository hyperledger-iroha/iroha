---
lang: dz
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

# གནོད་སྐྱོན་བརྟག་དཔྱད་ལག་ཆས་ (MINFO‐‑4b)

ལམ་སྟོན་གཞི་བསྟུན་: **ཨེམ་ཨའི་ཨེཕ་ཨོ་‐༤བི་ — གནོད་སྐྱོན་བརྟག་དཔྱད་ལག་ཆ།**  
ཇོ་བདག་: གཞུང་སྐྱོང་ཚོགས་སྡེ་ / དབྱེ་དཔྱད།

དྲན་འཛིན་འདི་གིས་ ད་ལྟོ་དེ་སྦེ་ `cargo xtask ministry-agenda impact` བརྡ་བཀོད་འདི་ཡིག་ཆ་བཟོཝ་ཨིན།
གིས་ སྤྱིར་བཏང་འོས་འཚམ་གྱི་ཐུམ་སྒྲིལ་ཚུ་གི་དོན་ལུ་ དགོ་པའི་ རང་བཞིན་གྱི་ ཧ་ཤི་བཟའ་ཚང་གི་ ཁྱབ་སྤེལ་འདི་ བཏོནམ་ཨིན། ཚིག༌ཕྲད
ལག་ཆས་ཚུ་ བདེན་དཔྱད་འབད་ཡོད་པའི་ གྲོས་གཞི་ཚོགས་སྡེ་གི་གྲོས་འཆར་དང་ འདྲ་བཤུས་ཐོ་བཀོད་ དེ་ལས་ དང་།
གདམ་ཁ་ཅན་གྱི་ཐོ་ཡིག་མེད་པའི་/སྲིད་དོན་གྱི་པར་བཤུས་འདི་བསྐྱར་ཞིབ་འབད་མི་ཚུ་གིས་ ག་འདི་ ངེས་བདེན་སྦེ་མཐོང་ཚུགས།
མཛུབ་མོ་གི་པར་ཚུ་ གསརཔ་ཨིནམ་ད་ དེ་གིས་ ད་ལྟོ་ཡོད་པའི་སྲིད་བྱུས་དང་ ཁ་ཐུག་རྐྱབ་སྟེ་ ཐོ་བཀོད་ག་དེམ་ཅིག་ཡོདཔ་ཨིན་ན་ཚུ་ཨིན་པས།
ཧ་ཤི་བཟའ་ཚང་རེ་རེ་གིས་ ཕན་གྲོགས་འབདཝ་ཨིན།

## ཨིན་པུཊི་ཚུ།

1. **གྲོས་གཞི་གྲོས་འཆར་ཚུ།།** རྗེས་སུ་འབྲང་བའི་ཡིག་སྣོད་གཅིག་ཡང་ན་མང་།
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   དེ་ཚུ་གསལ་ཏོག་ཏོ་སྦེ་ `--proposal <path>` དང་ཅིག་ཁར་ ཡང་ན་ བརྡ་བཀོད་འདི་ ཅིག་ ལུ་དཔག་དགོ།
   སྣོད་ཐོ་ `--proposal-dir <dir>` དང་ འགྲུལ་ལམ་དེ་གི་འོག་ལུ་ `*.json` ཡིག་སྣོད་རེ་རེ་བཞིན་ བརྒྱུད་དེ་ཨིན།
   འདི་ཚུད་ཡོད།
2. **འདྲ་བཤུས་ཐོ་བཀོད་ (གདམ་ཁ་ཅན་)** ཇེ་ཨེསི་ཨོ་ཨེན་ཡིག་སྣོད་མཐུན་སྒྲིག་འབད་མི།
   `docs/examples/ministry/agenda_duplicate_registry.json`. འཁྲུག་རྩོད་ཚུ་ནི།
   `source = "duplicate_registry"` འོག་ལུ་སྙན་ཞུ་འབད་ཡོདཔ་ཨིན།
3. **སྲིད་དོན་གྱི་པར་ལེན་ (གདམ་ཁ་ཅན་)** ལྗིད་ཚད་འཇམ་པོ་ཅིག་གིས་ ག་ར་ཐོ་བཀོད་འབདཝ་ཨིན།
   མཛུབ་མོ་གི་པར་རིས་འདི་ ཇི་ཨར་/ལྷན་ཁག་སྲིད་བྱུས་ཀྱིས་ བསྟར་སྤྱོད་འབད་ཡོདཔ་ཨིན། མངོན་གསལ་པ་གིས་རེ་བ་བསྐྱེདཔ་ཨིན།
   གཤམ་གསལ་གྱི་ལས་འཆར་ (se) དང་།
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   དཔེ་ཚད་ཆ་ཚང་ཅིག་གི་དོན་ལུ་):

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

གྲོས་འཆར་དམིགས་ཚད་དང་མཐུན་པའི་ `hash_family:hash_hex` མཛུབ་མོ་གི་པར་རིས།
`source = "policy_snapshot"` འོག་ལུ་ གཞི་བསྟུན་ `policy_id` གི་ཐོག་ལས་ སྙན་ཞུ་འབད་ཡོདཔ་ཨིན།

## ལག་ལེན།

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

གྲོས་འཆར་ཁ་སྐོང་ཚུ་ བསྐྱར་ལོག་འབད་དེ་ `--proposal` དར་ཆ་ ཡང་ན་ 1.
འོས་འཚམ་གྱི་ཚད་གཞི་ཆ་མཉམ་ཡོད་པའི་སྣོད་ཐོ་ཅིག་བཀྲམ་སྤེལ་འབད་ནི།

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

བརྡ་བཀོད་འདི་གིས་ `--out` བཏོན་བཏང་པའི་སྐབས་ stdout ལུ་ བརྡ་བཀོད་འབདཝ་ཨིན།

## ཐོན་འབྲས་

སྙན་ཞུ་འདི་ མཚན་རྟགས་བཀོད་ཡོད་པའི་ དངོས་པོ་ཅིག་ཨིན།
འོག་གི་གཞི་བཀོད་དང་གཅིག་ཁར་ `artifacts/ministry/impact/` སྣོད་ཐོ་།

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

འདི་ཡང་ བར་ལམ་གྱི་བཅུད་བསྡུས་དང་གཅིག་ཁར་ འོས་འདེམས་ཀྱི་ཡིག་ཆ་ག་ར་ལུ་ JSON མཉམ་སྦྲགས་འབད།
ཚོགས་ཆུང་དང་ཁྲིམས་དཔོན་ དེ་ལས་ གཞུང་སྐྱོང་ལྟ་རྟོག་པ་ཚུ་གིས་ འགས་རྫས་ཀྱི་སྒོར་ཕྱེད་བར་ཐིག་འདི་ མཐོང་ཚུགས།
གྲོས་འཆར། ཐོན་འབྲས་འདི་ གཏན་འབེབས་ (ཧེཤ་བཟའ་ཚང་གིས་ དབྱེ་སེལ་འབད་ཡོདཔ་) དང་ ཉེན་སྲུང་ཡོདཔ་ཨིན།
CI/runbooks ནང་ཚུད་ཡོད། ཐོ་བཀོད་འདྲ་བཤུས་ཡང་ན་ སྲིད་བྱུས་ཀྱི་ པར་ཚུ་བསྒྱུར་བཅོས་འབད་བ་ཅིན་ ལུ།
བཀའ་རྒྱ་འདི་ ལོག་སྟེ་ར་ ཚོགས་རྒྱན་མ་ཕྱེ་བའི་ཧེ་མ་ བསྐྱར་གསོ་འབད་བའི་ ཅ་ཆས་ཚུ་ མཉམ་སྦྲགས་འབད།

> **ཤུལ་མམ་གྱི་རིམ་པ་:** བཟོ་བཏོན་འབད་ཡོད་པའི་གནོད་པ་སྙན་ཞུ་འདི་ ནང་ལུ།
> [I`cargo xtask ministry-panel packet`](referendum_packet.md) དེ་འབདཝ་ལས་
> `ReferendumPacketV1` ཡིག་ཆ་ནང་ ཧེཤ་བཟའ་ཚང་གི་ བརྡབ་གསིག་དང་ གཉིས་ཀ་ཡོད།
> བསྐྱར་ཞིབ་འབད་བའི་བསྒང་ཡོད་མི་ གྲོས་འཆར་གྱི་དོན་ལུ་ ཁ་གསལ་གྱི་འཁྲུག་རྩོད་ཀྱི་ཐོ་ཡིག།