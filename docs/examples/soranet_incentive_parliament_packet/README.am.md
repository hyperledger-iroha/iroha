---
lang: am
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የሶራኔት ሪሌይ ማበረታቻ የፓርላማ ፓኬት

ይህ ጥቅል በሶራ ፓርላማ ለማጽደቅ የሚያስፈልጉትን ቅርሶች ይይዛል
ራስ-ሰር የማስተላለፊያ ክፍያዎች (SNNet-7)፦

- `reward_config.json` - I18NT0000000X-ተከታታይ የሽልማት ሞተር ውቅር፣ ዝግጁ
  በ `iroha app sorafs incentives service init` ለመዋሃድ. የ
  `budget_approval_id` በአስተዳደር ደቂቃዎች ውስጥ ከተዘረዘረው ሃሽ ጋር ይዛመዳል።
- `shadow_daemon.json` - ተጠቃሚ እና ቦንድ ካርታ በድጋሚ አጫውት ጥቅም ላይ ይውላል
  መታጠቂያ (`shadow-run`) እና የምርት ዴሞን.
- `economic_analysis.md` - ለ2025-10 -> 2025-11 የፍትሃዊነት ማጠቃለያ
  ጥላ ማስመሰል.
- `rollback_plan.md` - አውቶማቲክ ክፍያዎችን ለማሰናከል የሚሰራ የመጫወቻ መጽሐፍ።
- የሚደግፉ ቅርሶች፡- `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`፣
  `dashboards/grafana/soranet_incentives.json`፣
  `dashboards/alerts/soranet_incentives_rules.yml`.

## የታማኝነት ማረጋገጫዎች

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

የምግብ መፍጫ ስርዓቱን በፓርላማ ደቂቃዎች ውስጥ ከተመዘገቡት እሴቶች ጋር ያወዳድሩ። አረጋግጥ
በ ውስጥ እንደተገለፀው የጥላ አሂድ ፊርማ
`docs/source/soranet/reports/incentive_shadow_run.md`.

## ፓኬጁን በማዘመን ላይ

1. ሽልማቱ በሚመዘንበት ጊዜ፣ የመሠረታዊ ክፍያ ወይም የ `reward_config.json` አድስ
   ማጽደቅ የሃሽ ለውጥ.
2. የ60-ቀን ጥላ ማስመሰልን እንደገና አሂድ፣ `economic_analysis.md` በ
   አዲስ ግኝቶችን እና JSON + የተነጠለ ፊርማ ጥንድ ያድርጉ።
3. የተዘመነውን ጥቅል ከኦብዘርቫቶሪ ዳሽቦርድ ጋር ለፓርላማ ያቅርቡ
   የታደሰ ፈቃድ ሲፈልጉ ወደ ውጭ መላክ።