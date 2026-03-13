---
lang: am
direction: ltr
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2025-12-29T18:16:35.927510+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# አንድሮይድ ኤስዲኬ ደህንነት ዒላማ - ETSI EN 319 401 አሰላለፍ

| መስክ | ዋጋ |
|-------|------|
| የሰነድ ስሪት | 0.1 (2026-02-12) |
| ወሰን | አንድሮይድ ኤስዲኬ (በ`java/iroha_android/` እና ደጋፊ ስክሪፕቶች/ሰነዶች ስር ያሉ የደንበኛ ቤተ-መጻሕፍት) |
| ባለቤት | ተገዢነት እና ህጋዊ (ሶፊያ Martins) |
| ገምጋሚዎች | የአንድሮይድ ፕሮግራም መሪ፣ የመልቀቅ ምህንድስና፣ SRE አስተዳደር |

## 1. የ TOE መግለጫ

የግምገማ ዒላማ (TOE) የአንድሮይድ ኤስዲኬ ቤተ-መጽሐፍት ኮድ (`java/iroha_android/src/main/java`)፣ የማዋቀሪያው ገጽ (`ClientConfig` + Norito መውሰጃ) እና በNorito እና በ`roadmap.md` ለወሳኝ ክንውኖች AND72.

ዋና ክፍሎች:

1. **የማዋቀር መግቢያ** — `ClientConfig` ክሮች Torii የመጨረሻ ነጥቦች፣ TLS ፖሊሲዎች፣ ድጋሚዎች እና የቴሌሜትሪ መንጠቆዎች ከተፈጠረው `iroha_config` አንጸባራቂ እና የማይለወጥ ድህረ-ጅምርን (I10000108) ያስገድዳል።
2. **የቁልፍ አስተዳደር/ StrongBox** — በሃርድዌር የተደገፈ ፊርማ በ`SystemAndroidKeystoreBackend` እና `AttestationVerifier` በኩል ይተገበራል፣ በ`docs/source/sdk/android/key_management.md` ፖሊሲዎች ተመዝግቧል። የማረጋገጫ ቀረጻ/ማረጋገጫ `scripts/android_keystore_attestation.sh` እና የ CI አጋዥ `scripts/android_strongbox_attestation_ci.sh` ይጠቀማል።
3. **ቴሌሜትሪ እና ማሻሻያ** — በ`docs/source/sdk/android/telemetry_redaction.md` ላይ በተገለጸው የተጋራ እቅድ፣የሃሺድ ባለስልጣናትን ወደ ውጭ በመላክ፣የተጨመቁ የመሳሪያ መገለጫዎችን እና በድጋፍ ፕሌይቡክ የተተገበረውን የኦዲት መንጠቆዎችን ይሽራል።
4. **ኦፕሬሽኖች runbooks** — `docs/source/android_runbook.md` (የኦፕሬተር ምላሽ) እና `docs/source/android_support_playbook.md` (SLA + escalation) የTOEን የአሠራር አሻራ በቆራጥነት መሻር፣ ትርምስ ልምምዶች እና የማስረጃ ቀረጻዎችን ያጠነክራል።
5. **የተለቀቀው ፕሮቬንሽን** — Gradle-based ግንቦች በ`docs/source/sdk/android/developer_experience_plan.md` እና በ AND6 ተገዢነት ማረጋገጫ ዝርዝር ውስጥ እንደተያዙት CycloneDX ተሰኪን እና ሊባዙ የሚችሉ የግንባታ ባንዲራዎችን ይጠቀማሉ። የተለቀቁ ቅርሶች በ`docs/source/release/provenance/android/` ውስጥ ተፈርመዋል እና ተሻገሩ።

## 2. ንብረቶች እና ግምት

| ንብረት | መግለጫ | የደህንነት አላማ |
|--------
| ውቅረት ይገለጣል | Norito-የተወሰደ `ClientConfig` ቅጽበታዊ ገጽ እይታዎች ከመተግበሪያዎች ጋር ተሰራጭተዋል። | በእረፍት ጊዜ ትክክለኛነት፣ ታማኝነት እና ሚስጥራዊነት። |
| የመፈረሚያ ቁልፎች | በ StrongBox/TEE አቅራቢዎች በኩል የሚፈጠሩ ወይም የሚገቡ ቁልፎች። | StrongBox ምርጫ፣ የማረጋገጫ ምዝግብ ማስታወሻ፣ ምንም ቁልፍ ወደ ውጭ መላክ የለም። |
| ቴሌሜትሪ ዥረቶች | ከኤስዲኬ መሣሪያ ወደ ውጭ የሚላኩ የOTLP ዱካዎች/ምዝግብ ማስታወሻዎች/ሜትሪዎች። | ስም ማጥፋት (የተሳሳቁ ባለስልጣናት)፣ PII የተቀነሰ፣ ኦዲትን ይሽራል። |
| ደብተር መስተጋብር | Norito ጭነቶች፣ የመግቢያ ሜታዳታ፣ Torii የአውታረ መረብ ትራፊክ። | የጋራ ማረጋገጫ፣ መልሶ ማጫወት የሚቋቋሙ ጥያቄዎች፣ ቆራጥ ሙከራዎች። |

ግምቶች፡-

- የሞባይል ስርዓተ ክወና መደበኛ ማጠሪያ + SELinux ያቀርባል; StrongBox መሳሪያዎች የጉግልን ቁልፍ ጌታ በይነገጽን ተግባራዊ ያደርጋሉ።
- የኦፕሬተሮች አቅርቦት Torii የመጨረሻ ነጥቦችን ከTLS የምስክር ወረቀቶች ጋር በካውንስል-ታመኑ CAs የተፈረመ።
- ወደ ማቨን ከመታተሙ በፊት መሠረተ ልማትን እንደገና ሊባዙ የሚችሉ የግንባታ መስፈርቶችን ይገንቡ።

## 3. ማስፈራሪያዎች እና መቆጣጠሪያዎች| ስጋት | ቁጥጥር | ማስረጃ |
|--------|---------|----------|
| የተበላሸ ውቅር ይገለጣል | `ClientConfig` ከማመልከትዎ በፊት ማኒፌክቶችን (hash + schema) ያረጋግጣል እና በ`android.telemetry.config.reload` በኩል ዳግም መጫንን መዝግቧል። | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. |
| የመፈረሚያ ቁልፎች ስምምነት | በስትሮንግቦክስ የሚፈለጉ ፖሊሲዎች፣ የማረጋገጫ ማሰሪያዎች እና የመሣሪያ-ማትሪክስ ኦዲቶች ተንሸራታችነትን ይለያሉ፤ በእያንዳንዱ ክስተት የተመዘገበውን ይሽራል። | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. |
| በቴሌሜትሪ ውስጥ PII መፍሰስ | Blake2b-hashed ባለሥልጣኖች፣ በባልኬት የታሸጉ የመሣሪያ መገለጫዎች፣ የአገልግሎት አቅራቢዎች መቅረት፣ መግባትን መሻር። | `docs/source/sdk/android/telemetry_redaction.md`; Playbook §8ን ይደግፉ። |
| በ Torii RPC ላይ እንደገና ያጫውቱ ወይም ይቀንሱ | `/v2/pipeline` ጥያቄ ገንቢ TLS መሰካትን፣ የጩኸት ሰርጥ ፖሊሲን ያስፈጽማል እና በጀቶችን በሃሽድ ባለስልጣን አውድ እንደገና ይሞክሩ። | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java`; `docs/source/sdk/android/networking.md` (የታቀደ)። |
| ያልተፈረሙ ወይም የማይባዙ ልቀቶች | CycloneDX SBOM + Sigstore ማረጋገጫዎች በ AND6 የማረጋገጫ ዝርዝር; የሚለቀቁ RFCs በ`docs/source/release/provenance/android/` ውስጥ ማስረጃ ያስፈልጋቸዋል። | `docs/source/sdk/android/developer_experience_plan.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. |
| ያልተሟላ ክስተት አያያዝ | Runbook + playbook መሻርን፣ ትርምስ ልምምዶችን እና የማሳደግ ዛፍን ይገልፃል። ቴሌሜትሪ መሻር የተፈረመ Norito ጥያቄዎችን ይፈልጋል። | `docs/source/android_runbook.md`; `docs/source/android_support_playbook.md`. |

## 4. የግምገማ ተግባራት

1. **የዲዛይን ግምገማ** — Compliance + SRE ያንን ውቅረት፣ ቁልፍ አስተዳደር፣ ቴሌሜትሪ እና የመልቀቂያ መቆጣጠሪያዎችን ለ ETSI ደህንነት አላማዎች ያረጋግጡ።
2. ** የትግበራ ፍተሻዎች *** - ራስ-ሰር ሙከራዎች፡-
   - `scripts/android_strongbox_attestation_ci.sh` በማትሪክስ ውስጥ ለተዘረዘረው ለእያንዳንዱ የስትሮንግቦክስ መሣሪያ የተያዙ ጥቅሎችን ያረጋግጣል።
   - `scripts/check_android_samples.sh` እና የሚተዳደር መሣሪያ CI የናሙና መተግበሪያዎች የ`ClientConfig`/የቴሌሜትሪ ውሎችን ያከብራሉ።
3. ** የክዋኔ ማረጋገጫ *** - በየሩብ ጊዜ ትርምስ ልምምዶች በ `docs/source/sdk/android/telemetry_chaos_checklist.md` (የማደስ + ልምምዶችን መሻር)።
4. **የማስረጃ ማቆየት** - በ `docs/source/compliance/android/` (በዚህ አቃፊ) ስር የተከማቹ እና ከ`status.md` የተጠቀሱ ቅርሶች።

## 5. ETSI EN 319 401 ካርታ ስራ| EN 319 401 አንቀጽ | የኤስዲኬ ቁጥጥር |
|--------|-----------|
| 7.1 የደህንነት ፖሊሲ | በዚህ የደህንነት ዒላማ + የድጋፍ Playbook ውስጥ ተመዝግቧል። |
| 7.2 ድርጅታዊ ደህንነት | RACI + በጥሪ ላይ ባለቤትነት በድጋፍ Playbook §2 ውስጥ። |
| 7.3 የንብረት አስተዳደር | የማዋቀር፣ ቁልፍ እና የቴሌሜትሪ ንብረት አላማዎች ከላይ በ§2 የተገለጹት። |
| 7.4 የመዳረሻ መቆጣጠሪያ | StrongBox ፖሊሲዎች + የተፈረሙ Norito ቅርሶችን የሚፈልግ የስራ ፍሰት ይሽራል። |
| 7.5 ምስጠራ መቆጣጠሪያዎች | ቁልፍ ማመንጨት፣ ማከማቻ እና የምስክርነት መስፈርቶች ከ AND2 (ቁልፍ አስተዳደር መመሪያ)። |
| 7.6 ክወናዎች ደህንነት | ቴሌሜትሪ ሃሽንግ፣ ትርምስ ልምምዶች፣ የክስተቶች ምላሽ እና የማስረጃ መልቀቅ። |
| 7.7 የመገናኛ ደህንነት | `/v2/pipeline` TLS ፖሊሲ + ሃሽድ ባለስልጣናት (የቴሌሜትሪ ማሻሻያ ሰነድ)። |
| 7.8 የስርዓት ማግኛ / ልማት | በAND5/AND6 ዕቅዶች ውስጥ ሊባዙ የሚችሉ የግራድል ግንባታዎች፣ SBOMs እና የፕሮቬንሽን በሮች። |
| 7.9 የአቅራቢዎች ግንኙነት | Buildkite + Sigstore ማረጋገጫዎች ከሶስተኛ ወገን ጥገኝነት SBoms ጋር ተመዝግበዋል። |
| 7.10 ክስተት አስተዳደር | Runbook/Playbook ማሳደግ፣ ምዝግብ ማስታወሻን መሻር፣ ቴሌሜትሪ አለመሳካት ቆጣሪዎች። |

## 6. ጥገና

- ኤስዲኬ አዲስ ክሪፕቶግራፊክ ስልተ ቀመሮችን፣ የቴሌሜትሪ ምድቦችን ሲያስተዋውቅ ወይም የራስ ሰር ለውጦችን በሚለቀቅበት ጊዜ ይህን ሰነድ ያዘምኑ።
- በ`docs/source/compliance/android/evidence_log.csv` ውስጥ የተፈረሙ ቅጂዎችን ከSHA-256 ዳይጀስትስ እና ከገምጋሚ ማቋረጥ ጋር ያገናኙ።