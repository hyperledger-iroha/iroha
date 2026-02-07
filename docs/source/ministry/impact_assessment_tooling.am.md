---
lang: am
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

# የተፅዕኖ ግምገማ መሳሪያ (MINFO-4b)

የመንገድ ካርታ ማጣቀሻ፡ **MINFO‑4b — የተፅዕኖ ግምገማ መሳሪያ።**  
ባለቤት፡ የአስተዳደር ምክር ቤት / ትንታኔ

ይህ ማስታወሻ አሁን የ `cargo xtask ministry-agenda impact` ትዕዛዝን ይመዘግባል።
ለሪፈረንደም እሽጎች የሚያስፈልገውን አውቶሜትድ የሃሽ-ቤተሰብ ልዩነት ያዘጋጃል። የ
መሳሪያ የተረጋገጠ የአጀንዳ ምክር ቤት ፕሮፖዛል፣ የተባዛ መዝገብ እና
ገምጋሚዎች የትኛውን በትክክል ማየት እንዲችሉ የአማራጭ መካድ/የመመሪያ ቅጽበታዊ ገጽ እይታ
የጣት አሻራዎች አዲስ ናቸው፣ ከነባሩ ፖሊሲ ጋር ይጋጫሉ፣ እና ስንት ግቤቶች
እያንዳንዱ ሃሽ ቤተሰብ አስተዋፅዖ ያደርጋል።

## ግብዓቶች

1. **የአጀንዳ ፕሮፖዛል።** አንድ ወይም ከዚያ በላይ የሆኑ ፋይሎች ይከተላሉ
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md)።
   በ `--proposal <path>` በግልፅ ያሳልፏቸው ወይም ትዕዛዙን በ ሀ
   ማውጫ በ`--proposal-dir <dir>` እና በእያንዳንዱ የ`*.json` ፋይል በዚያ መንገድ
   ተካቷል.
2. ** የተባዛ መዝገብ (አማራጭ)።** የJSON ፋይል ተዛማጅ
   `docs/examples/ministry/agenda_duplicate_registry.json`. ግጭቶች ናቸው።
   በ `source = "duplicate_registry"` ስር ተዘግቧል.
3. **የመመሪያ ቅጽበታዊ ገጽ እይታ (አማራጭ)።** ሁሉንም የሚዘረዝር ቀላል ክብደት ያለው መግለጫ
   የጣት አሻራ አስቀድሞ በGAR/ሚኒስቴር ፖሊሲ ተፈጻሚ ነው። ጫኚው ይጠብቃል።
   ከታች የሚታየው እቅድ (ተመልከት
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   ለሙሉ ናሙና)

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

የ `hash_family:hash_hex` አሻራው ከፕሮፖዛል ኢላማ ጋር የሚዛመድ ማንኛውም ግቤት ነው።
በ `source = "policy_snapshot"` በተጠቀሰው `policy_id` ተዘግቧል።

##አጠቃቀም

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

ተጨማሪ ሀሳቦችን በተደጋገሙ `--proposal` ባንዲራዎች ወይም በ
አጠቃላይ የሪፈረንደም ስብስብ የያዘ ማውጫ ማቅረብ፡-

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

ትዕዛዙ `--out` ሲቀር የፈጠረውን JSON ወደ stdout ያትማል።

## ውጤት

ሪፖርቱ የተፈረመበት ቅርስ ነው (በህዝበ ውሳኔ ፓኬት ስር ይቅዱት።
`artifacts/ministry/impact/` ማውጫ) ከሚከተለው መዋቅር ጋር

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

ይህንን JSON ከገለልተኛ ማጠቃለያ ጋር ለእያንዳንዱ የሪፈረንደም ዶሴ ያያይዙት።
ተወያዮች፣ ዳኞች እና የአስተዳደር ታዛቢዎች ትክክለኛውን ፍንዳታ ራዲየስ ማየት ይችላሉ።
እያንዳንዱ ፕሮፖዛል. ውጤቱ የሚወስነው (በሃሽ ቤተሰብ የተደረደረ) እና ደህንነቱ የተጠበቀ ነው።
በ CI / runbooks ውስጥ ያካትቱ; የተባዛው መዝገብ ቤት ወይም የፖሊሲ ቅጽበታዊ ገጽ እይታ ከተቀየረ፣
ድምጹ ከመከፈቱ በፊት ትዕዛዙን እንደገና ያስጀምሩ እና የታደሰውን ቅርስ ያያይዙ።

> ** ቀጣዩ ደረጃ፡** የተፈጠረውን የተፅዕኖ ሪፖርት ይመግቡ
> [`cargo xtask ministry-panel packet`](referendum_packet.md) ስለዚህ
> `ReferendumPacketV1` ዶሴ ሁለቱንም የሃሽ-ቤተሰብ መፈራረስ እና
> እየተገመገመ ላለው ሀሳብ ዝርዝር የግጭት ዝርዝር።