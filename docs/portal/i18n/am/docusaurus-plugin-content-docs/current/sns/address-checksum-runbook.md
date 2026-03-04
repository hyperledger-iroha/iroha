---
id: address-checksum-runbook
lang: am
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account Address Checksum Incident Runbook
sidebar_label: Checksum incidents
description: Operational response for IH58 (preferred) / compressed (`sora`, second-best) checksum failures (ADDR-7).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
ይህ ገጽ `docs/source/sns/address_checksum_failure_runbook.md` ያንጸባርቃል። አዘምን
መጀመሪያ የምንጭ ፋይሉን፣ ከዚያ ይህን ቅጂ አመሳስል።
::

የቼክሱም አለመሳካቶች ልክ እንደ I18NI0000012X (`ChecksumMismatch`) ላይ
Torii፣ ኤስዲኬዎች እና የኪስ ቦርሳ/አሳሽ ደንበኞች። የ ADDR-6/ADDR-7 የመንገድ ካርታ እቃዎች አሁን
የቼክሰም ማንቂያዎች ወይም ድጋፍ በሚሰጡበት ጊዜ ኦፕሬተሮች ይህንን Runbook እንዲከተሉ ይጠይቃሉ።
ቲኬቶች እሳት.

## ጨዋታውን መቼ እንደሚሮጥ

- ** ማንቂያዎች፡** `AddressInvalidRatioSlo` (የተገለፀው በ
  `dashboards/alerts/address_ingest_rules.yml`) ጉዞዎች እና ማብራሪያዎች ዝርዝር
  `reason="ERR_CHECKSUM_MISMATCH"`.
- ** ቋሚ ተንሸራታች፡** `account_address_fixture_status` I18NT0000000X የጽሑፍ ፋይል ወይም
  Grafana ዳሽቦርድ ለማንኛውም የኤስዲኬ ቅጂ የቼክ ድምር አለመዛመድን ዘግቧል።
- ** የድጋፍ ጭማሪዎች: ** የኪስ ቦርሳ / አሳሽ / ኤስዲኬ ቡድኖች የቼክሰም ስህተቶችን ይጠቅሳሉ ፣ IME
  ሙስና፣ ወይም ክሊፕቦርድ ስካን ከአሁን በኋላ ኮድ መፍታት አይችልም።
- ** በእጅ ምልከታ፡** Torii ምዝግብ ማስታወሻዎች `address_parse_error=checksum_mismatch` ደጋግመው ያሳያሉ
  ለምርት የመጨረሻ ነጥቦች.

ክስተቱ በተለይ የአካባቢ-8/Local-12 ግጭቶች ከሆነ፣ ይከተሉ
በምትኩ `AddressLocal8Resurgence` ወይም `AddressLocal12Collision` የመጫወቻ መጽሐፍት።

## የማስረጃ ዝርዝር

| ማስረጃ | ትዕዛዝ / ቦታ | ማስታወሻ |
|-------|-----------|------|
| Grafana ቅጽበታዊ | `dashboards/grafana/address_ingest.json` | ልክ ያልሆኑ የምክንያት ክፍተቶችን እና የተጎዱ የመጨረሻ ነጥቦችን ይያዙ። |
| የማንቂያ ጭነት | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | የአውድ መለያዎችን እና የጊዜ ማህተሞችን ያካትቱ። |
| ቋሚ ጤና | `artifacts/account_fixture/address_fixture.prom` + Grafana | የኤስዲኬ ቅጂዎች ከ`fixtures/account/address_vectors.json` የተንሳፈፉ መሆናቸውን ያረጋግጣል። |
| PromQL ጥያቄ | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | ለክስተቱ ሰነድ CSV ወደ ውጪ ላክ። |
| መዝገቦች | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (ወይም የምዝግብ ማስታወሻ) | ከማጋራትዎ በፊት PII ን ያጽዱ። |
| ቋሚ ማረጋገጫ | `cargo xtask address-vectors --verify` | ቀኖናዊ ጀነሬተርን ያረጋግጣል እና ቁርጠኛ JSON ይስማማሉ። |
| የኤስዲኬ እኩልነት ማረጋገጫ | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | በማንቂያዎች/ትኬቶች ውስጥ ለተዘገበው ለእያንዳንዱ ኤስዲኬ ያሂዱ። |
| ክሊፕቦርድ/IME ንጽህና | `iroha tools address inspect <literal>` | የተደበቁ ቁምፊዎችን ያገኛል ወይም IME እንደገና ይጽፋል; `address_display_guidelines.md` ጥቀስ። |

#አፋጣኝ ምላሽ

1. ማንቂያውን እውቅና ይስጡ፣ በአደጋው ውስጥ የGrafana ቅጽበታዊ ገጽ እይታዎችን + የ PromQL ውጤትን ያገናኙ
   ክር፣ እና ማስታወሻ I18NT0000009X አውዶች ላይ ተጽዕኖ አሳድሯል።
2. አንጸባራቂ ማስተዋወቂያዎችን አቁም/ኤስዲኬ ልብ የሚነካ አድራሻ መተንተን ይለቃል።
3. የዳሽቦርድ ቅጽበተ-ፎቶዎችን እና የመነጨውን Prometheus የጽሑፍ ፋይል ቅርሶችን ያስቀምጡ
   የክስተቱ አቃፊ (`docs/source/sns/incidents/YYYY-MM/<ticket>/`)።
4. የ `checksum_mismatch` ጭነቶችን የሚያሳዩ የምዝግብ ማስታወሻዎችን ይጎትቱ.
5. የኤስዲኬ ባለቤቶችን (`#sdk-parity`) በናሙና ከሚጫኑ ጭነቶች ጋር ያሳውቁ።

## የስር መንስኤ ማግለል

### ቋሚ ወይም የጄነሬተር ተንሸራታች

- `cargo xtask address-vectors --verify` እንደገና አሂድ; ካልተሳካ እንደገና ማመንጨት.
- `ci/account_fixture_metrics.sh` (ወይም ግለሰብን) ያስፈጽሙ
  `scripts/account_fixture_helper.py check`) ለእያንዳንዱ ኤስዲኬ መጠቅለሉን ለማረጋገጥ
  የቤት ዕቃዎች ከቀኖናዊው JSON ጋር ይዛመዳሉ።

### የደንበኛ ኢንኮድሮች / IME regressions

- ዜሮ ስፋትን ለማግኘት በ `iroha tools address inspect` በኩል በተጠቃሚ የቀረቡ ቃል በቃል ይፈትሹ
  መቀላቀል፣ የቃና ልወጣዎች ወይም የተቆራረጡ ጭነቶች።
- ተሻጋሪ የኪስ ቦርሳ/አሳሽ አብሮ ይፈስሳል
  `docs/source/sns/address_display_guidelines.md` (ሁለት ቅጂ ዒላማዎች፣ ማስጠንቀቂያዎች፣
  የQR አጋዥዎች) የተፈቀደውን UX መከተላቸውን ለማረጋገጥ።

### ይገለጡ ወይም የመመዝገቢያ ጉዳዮች

- የቅርብ ጊዜውን አንጸባራቂ ጥቅል እና እንደገና ለማረጋገጥ `address_manifest_ops.md`ን ይከተሉ
  የአካባቢ-8 መራጮች ዳግም መነሳታቸውን ያረጋግጡ።
  በተጫኑ ጭነቶች ውስጥ ይታያሉ.

### ተንኮል አዘል ወይም የተበላሸ ትራፊክ

- የሚያስከፋ IPs/መተግበሪያ መታወቂያዎችን በTorii ምዝግብ ማስታወሻዎች እና በ`torii_http_requests_total` ያፈርሱ።
- ለደህንነት/ለመንግስት ክትትል ቢያንስ የ24 ሰአታት ምዝግብ ማስታወሻዎችን አቆይ።

## ቅነሳ እና ማገገም

| ሁኔታ | ድርጊቶች |
|-------|--------|
| ቋሚ ተንሸራታች | `fixtures/account/address_vectors.json`ን ያድሱ፣ `cargo xtask address-vectors --verify` እንደገና ያስጀምሩ፣ የኤስዲኬ ቅርቅቦችን ያዘምኑ እና የ`address_fixture.prom` ቅጽበተ-ፎቶዎችን ከቲኬቱ ጋር ያያይዙ። |
| ኤስዲኬ/የደንበኛ መመለሻ | የፋይል ጉዳዮች ቀኖናዊውን የ + I18NI0000044X ውፅዓት እና ከኤስዲኬ ፓሪቲ CI (ለምሳሌ I18NI0000045X) ጀርባ የሚለቀቁትን የሚያመለክቱ ፋይሎች። |
| ተንኮል አዘል ማስገባቶች | የመቃብር ድንጋይ መራጮች ካስፈለገ ደረጃ-ገደብ ወይም አግድ ዳይሬክተሮች፣ ወደ አስተዳደር ከፍ ይበሉ። |

አንዴ ማቃለያዎች ካረፉ፣ ለማረጋገጥ ከላይ ያለውን የPromQL ጥያቄ እንደገና ያሂዱ
`ERR_CHECKSUM_MISMATCH` ቢያንስ በዜሮ (ከ `/tests/*` በስተቀር) ይቆያል
ክስተቱን ከማሳነስ 30 ደቂቃዎች በፊት.

## መዝጋት

1. I18NT0000006X ቅጽበታዊ ገጽ እይታዎች፣ PromQL CSV፣ የምዝግብ ማስታወሻዎች እና `address_fixture.prom`።
2. `status.md` (ADDR ክፍል) እና የመንገድ ካርታ ረድፉን ከመሳሪያ/ዶክመንቶች ጋር ያዘምኑ
   ተለውጧል።
3. ከክስተት በኋላ ማስታወሻዎችን በ `docs/source/sns/incidents/` ስር ያስገቡ አዳዲስ ትምህርቶች
   ብቅ ማለት
4. ሲተገበር የኤስዲኬ ልቀት ማስታወሻዎች የቼክሰም መጠገኛዎችን መጠቀሳቸውን ያረጋግጡ።
5. ማንቂያው ለ24 ሰአት አረንጓዴ መቆየቱን ያረጋግጡ እና የፍተሻ ቼኮች ከዚህ በፊት አረንጓዴ እንደሆኑ ይቆያሉ።
   መፍታት.