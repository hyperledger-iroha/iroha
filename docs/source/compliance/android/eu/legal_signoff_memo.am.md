---
lang: am
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2025-12-29T18:16:35.926037+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 የአውሮፓ ህብረት ህጋዊ የመግቢያ ማስታወሻ አብነት

ይህ ማስታወሻ በፍኖተ ካርታ ንጥል **AND6** የተጠየቀውን የህግ ግምገማ ከቀዳሚው በፊት ይመዘግባል
EU (ETSI/GDPR) የጥበብ እሽግ ለተቆጣጠሪዎች ገብቷል። ምክሩ መዝለል አለበት።
ይህ አብነት በእያንዳንዱ ልቀት፣ ከታች ያሉትን መስኮች ይሙሉ እና የተፈረመውን ቅጂ ያከማቹ
በማስታወሻው ውስጥ ከተጠቀሱት የማይለወጡ ቅርሶች ጎን ለጎን።

## ማጠቃለያ

- ** መልቀቅ / ባቡር: ** `<e.g., 2026.1 GA>`
- ** የግምገማ ቀን:** `<YYYY-MM-DD>`
- ** ምክር / ገምጋሚ: ** `<name + organisation>`
- ** ወሰን: *** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- ** የተቆራኙ ቲኬቶች: ** `<governance or legal issue IDs>`

## የአርቴፍክት ማረጋገጫ ዝርዝር

| Artefact | SHA-256 | አካባቢ / አገናኝ | ማስታወሻ |
|-------|--------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + የአስተዳደር መዝገብ | የመልቀቂያ መለያዎችን እና የማስፈራሪያ ሞዴል ማስተካከያዎችን ያረጋግጡ። |
| `gdpr_dpia_summary.md` | `<hash>` | ተመሳሳይ ማውጫ / የትርጉም መስተዋቶች | የማሻሻያ ፖሊሲ ማጣቀሻዎች ከ`sdk/android/telemetry_redaction.md` ጋር መዛመዱን ያረጋግጡ። |
| `sbom_attestation.md` | `<hash>` | ተመሳሳይ ማውጫ + ኮሲንግ ጥቅል በማስረጃ ባልዲ | CycloneDX + የፕሮቬንሽን ፊርማዎችን ያረጋግጡ። |
| የማስረጃ መዝገብ ረድፍ | `<hash>` | `docs/source/compliance/android/evidence_log.csv` | የረድፍ ቁጥር `<n>` |
| የመሣሪያ-ላብራቶሪ የአደጋ ጊዜ ጥቅል | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` | ከዚህ ልቀት ጋር የተያያዘ ያልተሳካ ልምምዱን ያረጋግጣል። |

> ፓኬቱ ተጨማሪ ፋይሎችን ከያዘ ተጨማሪ ረድፎችን ያያይዙ (ለምሳሌ፡ ግላዊነት
> አባሪዎች ወይም DPIA ትርጉሞች)። እያንዳንዱ ቅርስ የማይለወጥ መጠቀስ አለበት።
> ሰቀላ ዒላማ እና Buildkite ሥራ ያፈራው.

## ግኝቶች እና ልዩነቶች

- `None.` *(የቀሩ ስጋቶችን በሚሸፍነው በጥይት ዝርዝር ይተኩ
  ይቆጣጠራል፣ ወይም የሚፈለጉ የክትትል እርምጃዎች።)*

## ማጽደቅ

- ** ውሳኔ: ** `<Approved / Approved with conditions / Blocked>`
- ** ፊርማ / የጊዜ ማህተም: ** `<digital signature or email reference>`
- ** ተከታይ ባለቤቶች: ** `<team + due date for any conditions>`

የመጨረሻውን ማስታወሻ ወደ የአስተዳደር ማስረጃ ባልዲ ይስቀሉ፣ SHA-256ን ይቅዱ
`docs/source/compliance/android/evidence_log.csv`፣ እና የሰቀላ ዱካውን ወደ ውስጥ ያገናኙ
`status.md`. ውሳኔው "ታግዷል" ከሆነ ወደ ብአዴን6 መሪነት ይሂዱ
ኮሚቴ እና ሰነድ የማሻሻያ እርምጃዎች በሁለቱም የመንገድ ካርታ ትኩስ ዝርዝር እና በ
የመሣሪያ-ላብራቶሪ የድንገተኛነት መዝገብ.