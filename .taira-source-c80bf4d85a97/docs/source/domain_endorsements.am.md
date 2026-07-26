---
lang: am
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የጎራ ድጋፍ

የጎራ ድጋፍ ኦፕሬተሮች በኮሚቴ በተፈረመ መግለጫ መሠረት ጎራ እንዲፈጠሩ እና እንደገና እንዲጠቀሙ ያስችላቸዋል። የድጋፍ ክፍያው በሰንሰለት ላይ የተመዘገበ Norito ነገር ነው ስለዚህ ደንበኞቻቸው የትኛውን ጎራ እና መቼ እንደመሰከሩ ኦዲት ማድረግ ይችላሉ።

## የመጫኛ ቅርፅ

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: ቀኖናዊ ጎራ መለያ
- `committee_id`፡ ሰው ሊነበብ የሚችል የኮሚቴ መለያ
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`፡ የቁመቶች አግድ ትክክለኛነት
- `scope`፡ የአማራጭ የመረጃ ቦታ እና አማራጭ `[block_start, block_end]` መስኮት (ያካተተ) ይህ የተቀበለው የማገጃ ቁመት መሸፈን አለበት
- `signatures`፡ ከ`body_hash()` በላይ ፊርማዎች (በ`signatures = []` የተረጋገጠ)
- `metadata`፡ አማራጭ Norito ሜታዳታ (የፕሮፖዛል መታወቂያዎች፣ የኦዲት ማያያዣዎች፣ ወዘተ)

## ማስፈጸም

- Nexus ሲነቃ እና `nexus.endorsement.quorum > 0`፣ ወይም የጎራ ፖሊሲ እንደ አስፈላጊነቱ ጎራውን ሲያመለክት ድጋፍ ያስፈልጋል።
- ማረጋገጫ የጎራ/መግለጫ ሃሽ ማሰርን፣ ሥሪትን፣ የማገጃ መስኮትን፣ የውሂብ ቦታ አባልነትን፣ የአገልግሎት ማብቂያ ጊዜን እና የኮሚቴ ምልአተ ጉባኤን ያስፈጽማል። ፈራሚዎች ከ`Endorsement` ሚና ጋር የቀጥታ የጋራ ስምምነት ቁልፎች ሊኖራቸው ይገባል። ድጋሚ ማጫወት በ`body_hash` ውድቅ ተደርጓል።
- ከጎራ ምዝገባ ጋር የተያያዙ ድጋፎች የሜታዳታ ቁልፍን `endorsement` ይጠቀማሉ። ተመሳሳዩ የማረጋገጫ መንገድ በ`SubmitDomainEndorsement` መመሪያ ጥቅም ላይ ይውላል፣ ይህም አዲስ ጎራ ሳይመዘገብ ለኦዲት የተደረጉ ድጋፎችን ይመዘግባል።

## ኮሚቴዎች እና ፖሊሲዎች

- ኮሚቴዎች በሰንሰለት ላይ መመዝገብ ይችላሉ (`RegisterDomainCommittee`) ወይም ከውቅረት ነባሪዎች (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`፣ id = `default`)።
- የየጎራ ፖሊሲዎች በ`SetDomainEndorsementPolicy` (የኮሚቴ መታወቂያ፣ `max_endorsement_age`፣ `required` ባንዲራ) ተዋቅረዋል። በማይኖርበት ጊዜ፣ Nexus ነባሪዎች ጥቅም ላይ ይውላሉ።

## CLI ረዳቶች

- ድጋፍን ይገንቡ/ይፈርሙ (የ Norito JSON ለ stdout ውጤቶች)

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- ማረጋገጫ ያቅርቡ;

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- አስተዳደርን ማስተዳደር;
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

የማረጋገጫ አለመሳካቶች የተረጋጉ የስህተት ሕብረቁምፊዎችን ይመለሳሉ (የኮረም አለመዛመድ፣ የቆየ/ያለፈበት ድጋፍ፣ የወሰን አለመዛመድ፣ ያልታወቀ የውሂብ ቦታ፣ የጎደለ ኮሚቴ)።