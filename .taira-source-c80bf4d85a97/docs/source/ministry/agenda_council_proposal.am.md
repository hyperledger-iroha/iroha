---
lang: am
direction: ltr
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2025-12-29T18:16:35.977493+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የአጀንዳ ምክር ቤት ፕሮፖዛል እቅድ (MINFO-2a)

የመንገድ ካርታ ማጣቀሻ፡ **MINFO-2a — የፕሮፖዛል ቅርጸት አረጋጋጭ።**

የአጀንዳ ምክር ቤት የስራ ፍሰት በዜጎች የቀረቡ ጥቁር መዝገብ እና የፖሊሲ ለውጦችን ያዘጋጃል።
የአስተዳደር ፓነሎች ከመከለሳቸው በፊት የቀረቡት ሀሳቦች። ይህ ሰነድ የ
ቀኖናዊ የክፍያ እቅድ፣ የማስረጃ መስፈርቶች እና የማባዛት ማወቂያ ህጎች
በአዲሱ አረጋጋጭ (`cargo xtask ministry-agenda validate`) ተበላ
ፕሮፖሰሮች JSON ወደ ፖርታሉ ከመስቀላቸው በፊት በአገር ውስጥ ማስገባት ይችላሉ።

## የደመወዝ ጭነት አጠቃላይ እይታ

የአጀንዳ ፕሮፖዛሎች የ`AgendaProposalV1` Norito እቅድን ይጠቀማሉ።
(`iroha_data_model::ministry::AgendaProposalV1`)። መስኮች መቼ እንደ JSON ተቀምጠዋል
በCLI/portal surfaces በኩል ማቅረብ።

| መስክ | አይነት | መስፈርቶች |
|-------|-------|------------|
| `version` | `1` (u16) | `AGENDA_PROPOSAL_VERSION_V1` እኩል መሆን አለበት። |
| `proposal_id` | ሕብረቁምፊ (`AC-YYYY-###`) | የተረጋጋ መለያ; በማረጋገጫ ጊዜ ተፈጻሚነት. |
| `submitted_at_unix_ms` | u64 | ከዩኒክስ ዘመን ጀምሮ ሚሊሰከንዶች። |
| `language` | ሕብረቁምፊ | BCP-47 መለያ (`"en"`፣ `"ja-JP"`፣ ወዘተ)። |
| `action` | enum (`add-to-denylist`, `remove-from-denylist`, `amend-policy`) | የተጠየቀው የሚኒስቴር እርምጃ። |
| `summary.title` | ሕብረቁምፊ | ≤256 ቻርዶች ይመከራል። |
| `summary.motivation` | ሕብረቁምፊ | ለምን እርምጃ ያስፈልጋል. |
| `summary.expected_impact` | ሕብረቁምፊ | ድርጊቱ ተቀባይነት ካገኘ ውጤቶች. |
| `tags[]` | ንዑስ ሆሄያት | አማራጭ የመለያ መለያዎች። የተፈቀዱ እሴቶች፡- `csam`፣ `malware`፣ `fraud`፣ `harassment`፣ `impersonation`፣ `policy-escalation`፣ Norito `spam`. |
| `targets[]` | ዕቃዎች | አንድ ወይም ከዚያ በላይ የሃሽ ቤተሰብ ግቤቶች (ከዚህ በታች ይመልከቱ)። |
| `evidence[]` | ዕቃዎች | አንድ ወይም ከዚያ በላይ የማስረጃ አባሪዎች (ከዚህ በታች ይመልከቱ)። |
| `submitter.name` | ሕብረቁምፊ | የማሳያ ስም ወይም ድርጅት. |
| `submitter.contact` | ሕብረቁምፊ | ኢሜል፣ ማትሪክስ እጀታ ወይም ስልክ; ከህዝባዊ ዳሽቦርዶች የተወሰደ። |
| `submitter.organization` | ሕብረቁምፊ (አማራጭ) | በግምገማ UI ውስጥ የሚታይ። |
| `submitter.pgp_fingerprint` | ሕብረቁምፊ (አማራጭ) | ባለ 40-ሄክስ አቢይ ሆሄ የጣት አሻራ። |
| `duplicates[]` | ሕብረቁምፊዎች | ከዚህ ቀደም ለገቡት የፕሮፖዛል መታወቂያዎች አማራጭ ማጣቀሻዎች። |

### የዒላማ ግቤቶች (`targets[]`)

እያንዳንዱ ኢላማ በፕሮፖዛሉ የተጠቀሰውን የሃሽ ቤተሰብ መፍጨትን ይወክላል።

| መስክ | መግለጫ | ማረጋገጫ |
|-------|-------------|-----------|
| `label` | ለገምጋሚ አውድ ተስማሚ ስም። | ባዶ ያልሆነ። |
| `hash_family` | Hash ለዪ (`blake3-256`፣ `sha256`፣ ወዘተ)። | ASCII ፊደሎች/አሃዞች/`-_.`፣ ≤48 ቻርሶች። |
| `hash_hex` | በትንሿ ሄክስ የተቀመጠ | ≥16 ባይት (32 hex chars) እና ልክ የሆነ ሄክስ መሆን አለበት። |
| `reason` | የምግብ መፍጫው ለምን መደረግ እንዳለበት አጭር መግለጫ. | ባዶ ያልሆነ። |

አረጋጋጩ የተባዙ `hash_family:hash_hex` ጥንዶችን በተመሳሳይ ውስጥ ውድቅ ያደርጋል
ፕሮፖዛል እና ሪፖርቶች የሚጋጩት ተመሳሳይ የጣት አሻራ በ ውስጥ ካለ
የተባዛ መዝገብ (ከዚህ በታች ይመልከቱ).

### የማስረጃ አባሪዎች (`evidence[]`)

ገምጋሚዎች ደጋፊ አውድ ማምጣት የሚችሉበት የማስረጃ ግቤቶች ሰነድ።| መስክ | አይነት | ማስታወሻ |
|-------|------|------|
| `kind` | enum (`url`፣ `torii-case`፣ `sorafs-cid`፣ `attachment`) | የምግብ መፍጫ መስፈርቶችን ይወስናል. |
| `uri` | ሕብረቁምፊ | HTTP(ኤስ) URL፣ Torii የጉዳይ መታወቂያ፣ ወይም SoraFS URI። |
| `digest_blake3_hex` | ሕብረቁምፊ | ለ `sorafs-cid` እና `attachment` አይነቶች ያስፈልጋል; ለሌሎች አማራጭ። |
| `description` | ሕብረቁምፊ | ለገምጋሚዎች አማራጭ የነጻ ቅጽ ጽሑፍ። |

### የተባዛ መዝገብ

ኦፕሬተሮች እንዳይባዙ ለመከላከል የነባር አሻራዎችን መዝገብ መያዝ ይችላሉ።
ጉዳዮች. አረጋጋጩ የሚከተለውን የ JSON ፋይል ይቀበላል፡-

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

የፕሮፖዛል ኢላማ ከግቤት ጋር ሲዛመድ አረጋጋጩ ካልሆነ በስተቀር ይሰረዛል
`--allow-registry-conflicts` ተለይቷል (ማስጠንቀቂያዎች አሁንም ይወጣሉ)።
[`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) ይጠቀሙ
የተባዛውን በማጣቀሻነት የሚያቋርጥ ለሪፈረንደም ዝግጁ የሆነ ማጠቃለያ ማፍለቅ
የመመዝገቢያ እና የፖሊሲ ቅጽበተ-ፎቶዎች.

## የ CLI አጠቃቀም

ነጠላ ፕሮፖዛል አቅርቡ እና በተባዛ መዝገብ ላይ አረጋግጡት፡-

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

የተባዙ ስኬቶችን ወደ ማስጠንቀቂያዎች ለማውረድ `--allow-registry-conflicts` ይለፉ
ታሪካዊ ኦዲት ማድረግ.

CLI በተመሳሳዩ Norito እቅድ እና የማረጋገጫ ረዳቶች ላይ ተመስርቷል
`iroha_data_model`፣ስለዚህ ኤስዲኬዎች/ፖርታሎች `AgendaProposalV1::validate`ን እንደገና መጠቀም ይችላሉ።
ወጥነት ላለው ባህሪ ዘዴ።

## መደርደር CLI (MINFO-2b)

የመንገድ ካርታ ማጣቀሻ፡ **MINFO-2b — ባለብዙ-ማስገቢያ ምደባ እና የኦዲት መዝገብ።**

የአጀንዳ ምክር ቤት ዝርዝር አሁን የሚተዳደረው በቆራጥነት አደረጃጀት ነው ስለዚህ ዜጎች
እያንዳንዱን ስዕል በተናጥል ኦዲት ማድረግ ይችላል። አዲሱን ትዕዛዝ ተጠቀም፡-

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` - እያንዳንዱን ብቁ አባል የሚገልጽ JSON ፋይል፡-

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  የምሳሌው ፋይል በ
  `docs/examples/ministry/agenda_council_roster.json`. አማራጭ መስኮች (ሚና)
  ድርጅት, ግንኙነት, ሜታዳታ) በ Merkle ቅጠል ውስጥ ተይዘዋል ስለዚህ ኦዲተሮች
  ስዕሉን የመገበውን ዝርዝር ማረጋገጥ ይችላል.

- `--slots` - ለመሙላት የምክር ቤት መቀመጫዎች ብዛት.
- `--seed` — 32-ባይት BLAKE3 ዘር (64 ትንሽ ሆክስ ቁምፊዎች) በ ውስጥ ተመዝግቧል
  የአስተዳደር ደቂቃዎች ለእጣው.
- `--out` - አማራጭ የውጤት መንገድ። ሲቀር፣ የJSON ማጠቃለያ ታትሟል
  stdout

### የውጤት ማጠቃለያ

ትዕዛዙ `SortitionSummary` JSON ብሎብ ያወጣል። የናሙና ውፅዓት የሚቀመጠው በ
`docs/examples/ministry/agenda_sortition_summary_example.json`. ቁልፍ መስኮች:

| መስክ | መግለጫ |
|-------|-----------|
| `algorithm` | የመደርደር መለያ (`agenda-sortition-blake3-v1`)። |
| `roster_digest` | የሮስተር ፋይሉን BLAKE3 + SHA-256 ማጭበርበሮች (ኦዲቶች በተመሳሳዩ የአባላት ዝርዝር ውስጥ እንደሚሠሩ ለማረጋገጥ ይጠቅማል)። |
| `seed_hex` / `slots` | ኦዲተሮች ስዕሉን እንደገና ማባዛት እንዲችሉ የCLI ግብአቶችን አስተጋባ። |
| `merkle_root_hex` | የሮስተር መርክል ዛፍ ስር (`hash_node`/`hash_leaf` አጋዥ በ`xtask/src/ministry_agenda.rs`)። |
| `selected[]` | ለእያንዳንዱ ማስገቢያ ግቤቶች፣ ቀኖናዊ አባል ሜታዳታ፣ ብቁ መረጃ ጠቋሚ፣ ኦሪጅናል የስም ዝርዝር መረጃ ጠቋሚ፣ ወሳኙ ስዕል ኢንትሮፒ፣ ቅጠል ሃሽ እና የመርክል ማረጋገጫ ወንድሞች እና እህቶች። |

### ስዕል በማረጋገጥ ላይ1. በ`roster_path` የተጠቀሰውን የስም ዝርዝር ያውጡ እና BLAKE3/SHA-256 ያረጋግጡ
   የምግብ መፍጫዎቹ ከማጠቃለያው ጋር ይጣጣማሉ.
2. CLI ን በተመሳሳዩ ዘር / ቦታዎች / ዝርዝር ውስጥ እንደገና ያሂዱ; የተገኘው `selected[].member_id`
   ትዕዛዙ ከታተመው ማጠቃለያ ጋር መዛመድ አለበት።
3. ለአንድ የተወሰነ አባል፣ ተከታታይ አባል የሆነውን JSON በመጠቀም የመርክል ቅጠልን ያሰሉ።
   (`norito::json::to_vec(&sortition_member)`) እና እያንዳንዱን የማረጋገጫ ሃሽ አጣጥፉ። የመጨረሻው
   መፍጨት `merkle_root_hex` እኩል መሆን አለበት። በምሳሌው ማጠቃለያ ውስጥ ያለው ረዳት ያሳያል
   `eligible_index`፣ `leaf_hash_hex` እና `merkle_proof[]` እንዴት እንደሚጣመር።

እነዚህ ቅርሶች የMINFO-2b መስፈርትን ለተረጋገጠ የዘፈቀደነት ያሟላሉ፣
k-of-m ምርጫ፣ እና በሰንሰለት ላይ ያለው ኤፒአይ እስካልተጣበቀ ድረስ አባሪ-ብቻ የኦዲት ምዝግብ ማስታወሻዎች።

## የማረጋገጫ ስህተት ማጣቀሻ

`AgendaProposalV1::validate` `AgendaProposalValidationError` ልዩነቶችን ያወጣል
የደመወዝ ጭነት መሸፈን ካልተሳካ። ከታች ያለው ሰንጠረዥ በጣም የተለመዱትን ያጠቃልላል
ስህተቶች የፖርታል ገምጋሚዎች የCLI ውፅዓት ወደ ተግባራዊ መመሪያ ሊተረጉሙ ይችላሉ።| ስህተት | ትርጉም | ማሻሻያ |
|-------|---------|------------|
| `UnsupportedVersion { expected, found }` | ክፍያ `version` ከአረጋጋጭ ከሚደገፈው ንድፍ ይለያል። | ስሪቱ ከ`expected` ጋር እንዲመሳሰል የቅርብ ጊዜውን የሼማ ጥቅል በመጠቀም JSON ን ያድሱ። |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` ባዶ ነው ወይም በ`AC-YYYY-###` ቅጽ የለም። | በድጋሚ ከማቅረብዎ በፊት በሰነድ የተደገፈውን ቅርጸት በመከተል ልዩ ለዪን ይሙሉ። |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` ዜሮ ነው ወይም ጠፍቷል። | የማስረከቢያውን የጊዜ ማህተም በዩኒክስ ሚሊሰከንዶች ይመዝግቡ። |
| `InvalidLanguageTag { value }` | `language` ትክክለኛ BCP-47 መለያ አይደለም። | እንደ `en`፣ `ja-JP`፣ ወይም ሌላ በBCP-47 የታወቀውን አካባቢ ይጠቀሙ። |
| `MissingSummaryField { field }` | ከ `summary.title`፣ `.motivation`፣ ወይም `.expected_impact` አንዱ ባዶ ነው። | ለተጠቀሰው የማጠቃለያ መስክ ባዶ ያልሆነ ጽሑፍ ያቅርቡ። |
| `MissingSubmitterField { field }` | `submitter.name` ወይም `submitter.contact` ጠፍቷል። | ገምጋሚዎች አቅራቢውን ማግኘት እንዲችሉ የጎደለውን አስገባ ዲበዳታ ያቅርቡ። |
| `InvalidTag { value }` | `tags[]` ግቤት በተፈቀደ ዝርዝሩ ላይ አይደለም። | መለያውን ያስወግዱት ወይም እንደገና ይሰይሙ ከተመዘገቡት እሴቶች (`csam`፣ `malware`፣ ወዘተ)። |
| `MissingTargets` | `targets[]` ድርድር ባዶ ነው። | ቢያንስ አንድ ኢላማ የሃሽ ቤተሰብ ግቤት ያቅርቡ። |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` | የዒላማ ግቤት የ`label` ወይም `reason` መስኮች ይጎድላል። | በድጋሚ ከማቅረቡ በፊት ለጠቋሚው ግቤት አስፈላጊውን መስክ ይሙሉ። |
| `InvalidHashFamily { index, value }` | የማይደገፍ `hash_family` መለያ። | የሃሽ ቤተሰብ ስሞችን ወደ ASCII ፊደላት እና `-_` ገድብ። |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` | ዳይጀስት ልክ ያልሆነ ሄክስ ወይም ከ16 ባይት ያነሰ ነው። | ለተጠቆመው ኢላማ ትንሽ ሆክስ ዳይጀስት (≥32 hex chars) ያቅርቡ። |
| `DuplicateTarget { index, fingerprint }` | የዒላማ መፍጨት ቀደም ሲል የመግቢያ ወይም የመመዝገቢያ የጣት አሻራ ያባዛል። | የተባዙትን ያስወግዱ ወይም ደጋፊ ማስረጃውን ወደ አንድ ኢላማ ያዋህዱ። |
| `MissingEvidence` | ምንም የማስረጃ አባሪዎች አልቀረቡም። | ከመራቢያ ቁሳቁስ ጋር የሚያገናኝ ቢያንስ አንድ የማስረጃ መዝገብ ያያይዙ። |
| `MissingEvidenceUri { index }` | የማስረጃ ግቤት የ`uri` መስክ ጠፍቷል። | ለመረጃ ጠቋሚው ግቤት ሊመጣ የሚችለውን URI ወይም የጉዳይ መለያ ያቅርቡ። |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` | መፈጨትን የሚፈልግ (SoraFS CID ወይም አባሪ) የሚያስፈልገው ማስረጃ ይጎድላል ​​ወይም ልክ ያልሆነ `digest_blake3_hex` አለው። | ለመረጃ ጠቋሚው መግቢያ ባለ 64-ቁምፊ ንዑስ ሆሄ BLAKE3 መፍጨት ያቅርቡ። |

#ምሳሌዎች

- `docs/examples/ministry/agenda_proposal_example.json` - ቀኖናዊ ፣
  lint-clean proposal payload ከሁለት ማስረጃዎች ጋር።
- `docs/examples/ministry/agenda_duplicate_registry.json` - የጀማሪ መዝገብ
  ነጠላ BLAKE3 አሻራ እና ምክንያታዊነት የያዘ።

የፖርታል መሣሪያን ሲያዋህዱ ወይም CI ሲጽፉ እነዚህን ፋይሎች እንደ አብነት ይጠቀሙ
አውቶማቲክ ማቅረቢያዎችን ይፈትሻል.