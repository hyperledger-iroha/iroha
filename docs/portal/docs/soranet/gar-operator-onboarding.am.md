---
lang: am
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 565d4e8bf0a043b2c83a03ec87a8c71a30da34f56d94a28cad03677963b3e69a
source_last_modified: "2025-12-29T18:16:35.206864+00:00"
translation_last_reviewed: 2026-02-07
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
translator: machine-google-reviewed
---

የSNNet-9 ተገዢነት ውቅረትን ሊደገም በሚችል ለመልቀቅ ይህንን አጭር መግለጫ ይጠቀሙ።
ለኦዲት ተስማሚ ሂደት. ለእያንዳንዱ ኦፕሬተር ከህግ ግምገማ ጋር ያጣምሩት።
ተመሳሳዩን መፈጨት እና የማስረጃ አቀማመጥ ይጠቀማል።

# እርምጃዎች

1. ** ውቅረትን ሰብስብ ***
   - `governance/compliance/soranet_opt_outs.json` አስመጣ።
   - የእርስዎን I18NI0000005X ከታተሙት የማረጋገጫ ማጭበርበሮች ጋር ያዋህዱ
     በ [የህግ ግምገማ] (gar-jurisdictional-review)።
2. ** አረጋግጥ ***
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - አማራጭ፡ I18NI0000008X
3. **ማስረጃ ያዝ**
   - በ `artifacts/soranet/compliance/<YYYYMMDD>/` ስር ያከማቹ:
     - `config.json` (የመጨረሻው ተገዢነት እገዳ)
     - `attestations.json` (URIs + መፈጨት)
     - የማረጋገጫ ምዝግብ ማስታወሻዎች
     - የተፈረሙ ፒዲኤፍ/Norito ፖስታዎች ማጣቀሻዎች
4. **አግብር**
   - ልቀቱን (`gar-opt-out-<date>`) ፣ ኦርኬስትራ/ኤስዲኬ ውቅሮችን እንደገና ማሰማራት ፣
     እና የ `compliance_*` ክስተቶች በተጠበቀው ቦታ በምዝግብ ማስታወሻዎች ውስጥ እንደሚለቀቁ ያረጋግጡ።
5. ** ዝጋ ***
   - የማስረጃውን ጥቅል ከአስተዳደር ምክር ቤት ጋር ያቅርቡ።
   - የማግበር መስኮቱን + አጽዳቂዎችን በ GAR መዝገብ ቤት ውስጥ ያስገቡ።
   - የሚቀጥለውን የግምገማ ቀናት ከዳኝነት ግምገማ ሰንጠረዥ ያቅዱ።

## ፈጣን የፍተሻ ዝርዝር

- [ ] `jurisdiction_opt_outs` ከቀኖናዊው ካታሎግ ጋር ይዛመዳል።
- [ ] የማረጋገጫ ቀመሮች በትክክል ተቀድተዋል።
- [ ] የማረጋገጫ ትዕዛዞች ይሮጣሉ እና በማህደር ተቀምጠዋል።
- [ ] በ`artifacts/soranet/compliance/<date>/` ውስጥ የተከማቸ የማስረጃ ጥቅል።
- [ ] የታቀደ ልቀት + GAR ማስታወሻ ደብተር ተዘምኗል።
- [ ] ቀጣይ ግምገማ አስታዋሾች ተዘጋጅተዋል።

## ይመልከቱ

- [ጋር የፍርድ ውሳኔ](gar-jurisdictional-review)
- [GAR Compliance Playbook (ምንጭ)](../../../source/soranet/gar_compliance_playbook.md)