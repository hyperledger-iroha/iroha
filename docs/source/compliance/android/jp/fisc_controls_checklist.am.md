---
lang: am
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2025-12-29T18:16:35.928660+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC የደህንነት ቁጥጥሮች ማረጋገጫ ዝርዝር - አንድሮይድ ኤስዲኬ

| መስክ | ዋጋ |
|-------|------|
| ስሪት | 0.1 (2026-02-12) |
| ወሰን | አንድሮይድ ኤስዲኬ + ከዋኝ መሣሪያ በጃፓን የፋይናንስ ማሰማራቶች ውስጥ ጥቅም ላይ የዋለ |
| ባለቤቶች | ተገዢነት እና ህጋዊ (ዳንኤል ፓርክ)፣ የአንድሮይድ ፕሮግራም መሪ |

## ማትሪክስ ይቆጣጠሩ

| FISC ቁጥጥር | የትግበራ ዝርዝር | ማስረጃ / ዋቢ | ሁኔታ |
|-------------|------------|-----------|--------|
| ** የስርዓት ውቅር ታማኝነት *** | `ClientConfig` አንጸባራቂ hashingን፣ የንድፍ ማረጋገጫን እና ተነባቢ-ብቻ የአሂድ ጊዜ መዳረሻን ያስፈጽማል። የማዋቀር ዳግም መጫን አለመሳካቶች `android.telemetry.config.reload` ክስተቶችን በመሮጫ መጽሐፍ ውስጥ ያሰፍራሉ። | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ ተተግብሯል |
| ** የመዳረሻ ቁጥጥር እና ማረጋገጫ** | ኤስዲኬ የTorii TLS ፖሊሲዎችን እና `/v1/pipeline` የተፈረሙ ጥያቄዎችን ያከብራል፤ ከዋኝ የስራ ፍሰቶች ማመሳከሪያ ፕሌይ ደብተር §4–5ን ከፍ ለማድረግ ይደግፉ እና ጌቲንግን በተፈረመ Norito ቅርሶች። | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (የስራ ፍሰትን ይሽሩ)። | ✅ ተተግብሯል |
| ** የክሪፕቶግራፊክ ቁልፍ አስተዳደር** | በስትሮንግቦክስ የተመረጡ አቅራቢዎች፣ የማረጋገጫ ማረጋገጫ እና የመሣሪያ ማትሪክስ ሽፋን የKMS ተገዢነትን ያረጋግጣሉ። የማረጋገጫ ታጥቆ ውጤቶች በ`artifacts/android/attestation/` ስር ተቀምጠው በዝግጁነት ማትሪክስ ውስጥ ተከታትለዋል። | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ ተተግብሯል |
| ** መመዝገብ፣ መከታተል እና ማቆየት** | የቴሌሜትሪ ማሻሻያ ፖሊሲ ሚስጥራዊነት ያለው መረጃን ይይዛል፣ የመሣሪያ ባህሪያትን ያስቀምጣል እና ማቆየትን ያስፈጽማል (7/30/90/365-ቀን መስኮቶች)። Playbookን ይደግፉ §8 የዳሽቦርድ ገደቦችን ይገልፃል; በ `telemetry_override_log.md` ውስጥ ተመዝግቧል ። | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ ተተግብሯል |
| **ኦፕሬሽን እና ለውጥ አስተዳደር** | GA የመቁረጥ ሂደት (የድጋፍ ጨዋታ መጽሐፍ §7.2) እና `status.md` ዝማኔዎችን የመልቀቂያ ዝግጁነት ይከታተሉ። በ`docs/source/compliance/android/eu/sbom_attestation.md` በኩል የተገናኘ የማስረጃ መልቀቅ (SBOM፣ Sigstore ጥቅሎች)። | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ ተተግብሯል |
| **የአደጋ ምላሽ እና ሪፖርት ማድረግ** | ፕሌይቡክ የክብደት ማትሪክስን፣ የ SLA ምላሽ መስኮቶችን እና የማክበር ማሳወቂያ ደረጃዎችን ይገልጻል። ቴሌሜትሪ ይሽራል + ትርምስ ልምምዶች ከአብራሪዎች በፊት መራባትን ያረጋግጣሉ። | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ ተተግብሯል |
| ** የውሂብ ነዋሪነት / አካባቢያዊነት ** | የቴሌሜትሪ ሰብሳቢዎች ለጄፒ ማሰማራቶች በተፈቀደው የቶኪዮ ክልል ውስጥ ይሰራሉ። የ StrongBox ማረጋገጫ ቅርቅቦች በክልል ውስጥ የተከማቹ እና ከአጋር ትኬቶች የተጠቀሱ። የአካባቢ እቅድ ከቤታ (AND5) በፊት በጃፓን የሚገኙ ሰነዶችን ያረጋግጣል። | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 በሂደት ላይ (የአካባቢ አቀማመጥ በመካሄድ ላይ) |

## የገምጋሚ ማስታወሻዎች

- የጋላክሲ ኤስ23/S24 መሣሪያ-ማትሪክስ ግቤቶችን ከተቆጣጠረው አጋር መሳፈር በፊት ያረጋግጡ (የዝግጁነት ሰነድ ረድፎችን `s23-strongbox-a`፣ `s24-strongbox-a` ይመልከቱ)።
- በጄፒ ማሰማራቶች ውስጥ ያሉ የቴሌሜትሪ ሰብሳቢዎች በDPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`) ላይ የተገለጸውን ተመሳሳይ የማቆየት/የመሻር አመክንዮ መተግበራቸውን ያረጋግጡ።
- የባንክ አጋሮች ይህንን የማረጋገጫ ዝርዝር አንዴ ከገመገሙ የውጭ ኦዲተሮች ማረጋገጫን ይያዙ።