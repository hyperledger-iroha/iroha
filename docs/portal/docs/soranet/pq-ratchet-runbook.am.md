---
lang: am
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 370733f000ffa7022cab18931c2697031a225d2ce3ae83382896c0d61f9fe6e2
source_last_modified: "2026-01-05T09:28:11.912569+00:00"
translation_last_reviewed: 2026-02-07
id: pq-ratchet-runbook
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

#ዓላማ

ይህ Runbook ለሶራኔት የድህረ-ኳንተም (PQ) ስም-አልባ ፖሊሲ የእሳት-ቁፋሮ ቅደም ተከተል ይመራል። ኦፕሬተሮች ሁለቱንም ማስተዋወቅ ይለማመዳሉ (ደረጃ A -> ደረጃ B -> ደረጃ ሐ) እና የPQ አቅርቦት ሲቀንስ ቁጥጥር የሚደረግበት ዝቅጠት ወደ ደረጃ B/A ይመለሳሉ። መሰርሰሪያው የቴሌሜትሪ መንጠቆዎችን (`sorafs_orchestrator_policy_events_total`፣ `sorafs_orchestrator_brownouts_total`፣ `sorafs_orchestrator_pq_ratio_*`) ያረጋግጣል እና ለአደጋው የመልመጃ መዝገብ ቅርሶችን ይሰበስባል።

## ቅድመ ሁኔታዎች

- የቅርብ ጊዜ `sorafs_orchestrator` ባለ ሁለትዮሽ አቅም-መመዘን (በ `docs/source/soranet/reports/pq_ratchet_validation.md` ላይ የሚታየውን መሰርሰሪያ ማጣቀሻ ላይ ወይም በኋላ ቁርጠኝነት).
- ወደ I18NT0000000X/I18NT0000002X ቁልል የሚያገለግለው `dashboards/grafana/soranet_pq_ratchet.json` መድረስ።
- የስም ጠባቂ ማውጫ ቅጽበተ ፎቶ። ከስልጠናው በፊት አንድ ቅጂ አምጥተው ያረጋግጡ፡-

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

የምንጭ ማውጫው JSON ን ብቻ የሚያትመው ከሆነ የማዞሪያ አጋዥዎችን ከማስኬድዎ በፊት ወደ Norito ሁለትዮሽ በI18NI0000020X እንደገና ኮድ ያድርጉት።

- ሜታዳታ እና የቅድመ-ደረጃ ሰጭ ማሽከርከር ቅርሶችን በCLI ይያዙ፡

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- በኔትወርክ የጸደቀውን መስኮት እና በጥሪ ቡድኖች ታዛቢነት ይቀይሩ።

## የማስተዋወቂያ እርምጃዎች

1. **የደረጃ ኦዲት**

   የመነሻውን ደረጃ ይመዝግቡ;

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   ከማስተዋወቅዎ በፊት `anon-guard-pq` ይጠብቁ።

2. ** ወደ ደረጃ B (አብዛኛዎቹ PQ) ያስተዋውቁ**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - አንጸባራቂዎች እስኪታደሱ ድረስ >=5 ደቂቃ ይጠብቁ።
   - በ Grafana (`SoraNet PQ Ratchet Drill` ዳሽቦርድ) የ "የመመሪያ ዝግጅቶች" ፓነል `outcome=met` ለ I18NI0000024X ያሳያል።
   - ቅጽበታዊ ገጽ እይታ ወይም ፓነል JSON ያንሱ እና ከአደጋ ምዝግብ ማስታወሻ ጋር አያይዘው።

3. ** ወደ ደረጃ C (ጥብቅ PQ) ያስተዋውቁ**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - የ `sorafs_orchestrator_pq_ratio_*` ሂስቶግራም አዝማሚያ ወደ 1.0 ያረጋግጡ።
   - የቡናውት ቆጣሪ ጠፍጣፋ መቆየቱን ያረጋግጡ; አለበለዚያ የማውረድ ደረጃዎችን ይከተሉ.

## የማውረድ/የማቅለል መሰርሰሪያ

1. ** ሰው ሰራሽ የፒኪው እጥረት መፍጠር**

   በመጫወቻ ስፍራው አካባቢ የጥበቃ ማውጫውን ወደ ክላሲካል ግቤቶች ብቻ በመቁረጥ የPQ ማሰራጫዎችን ያሰናክሉ እና የኦርኬስትራ መሸጎጫውን እንደገና ይጫኑ፡

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. ** ብራውን የወጣ ቴሌሜትሪ ይመልከቱ ***

   - ዳሽቦርድ፡ የፓነል "Brownout Rate" ከ0 በላይ ከፍ ይላል።
   - PromQL: I18NI0000026X
   - `sorafs_fetch` `anonymity_outcome="brownout"` በ `anonymity_reason="missing_majority_pq"` ሪፖርት ማድረግ አለበት።

3. ** ወደ ደረጃ ለ / ደረጃ A ዝቅ ያድርጉ **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   የPQ አቅርቦት አሁንም በቂ ካልሆነ፣ ወደ `anon-guard-pq` ዝቅ ያድርጉ። መሰርሰሪያው ሲጠናቀቅ ብራውን ውጭ ቆጣሪዎች ሲቀመጡ እና ማስተዋወቂያዎች እንደገና ሊተገበሩ ይችላሉ።

4. ** የጥበቃ ማውጫን እነበረበት መልስ **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## ቴሌሜትሪ እና ቅርሶች

- ** ዳሽቦርድ: *** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus ማንቂያዎች፡** `sorafs_orchestrator_policy_events_total` ቡኒውት ማንቂያ ከተዋቀረው SLO በታች መቆየቱን ያረጋግጡ (በማንኛውም የ10 ደቂቃ መስኮት ላይ <5%)።
- **የአደጋ መዝገብ፡** የተያዙትን የቴሌሜትሪ ቅንጥቦችን እና የኦፕሬተር ማስታወሻዎችን ወደ `docs/examples/soranet_pq_ratchet_fire_drill.log` ጨምር።
- **የተፈረመ ቀረጻ:** የመሰርሰሪያ መዝገብ እና የውጤት ሰሌዳ ወደ `artifacts/soranet_pq_rollout/<timestamp>/` ለመቅዳት፣ BLAKE3 ዳይጀስትቶችን ለማስላት እና የተፈረመ `cargo xtask soranet-rollout-capture` ይጠቀሙ።

ምሳሌ፡-

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

የተፈጠረውን ሜታዳታ እና ፊርማ ከአስተዳደር ፓኬት ጋር ያያይዙ።

## ወደኋላ መመለስ

መሰርሰሪያው እውነተኛ የPQ እጥረቶችን ካወቀ፣ በደረጃ A ላይ ይቆዩ፣ ለኔትወርክ ኤል.ኤል. ያሳውቁ እና የተሰበሰቡትን መለኪያዎች እና የጥበቃ ማውጫ ልዩነቱን ከተፈጠረው መከታተያ ጋር ያያይዙ። መደበኛ አገልግሎትን ወደነበረበት ለመመለስ ቀደም ሲል የተቀረጸውን የጥበቃ ማውጫ ተጠቀም።

:: ጫፍ ሪግሬሽን ሽፋን
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` ይህን መሰርሰሪያ ሰው ሠራሽ ማረጋገጫ ይሰጣል።
::