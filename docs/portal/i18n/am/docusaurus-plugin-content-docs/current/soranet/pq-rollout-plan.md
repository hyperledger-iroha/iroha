---
id: pq-rollout-plan
lang: am
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNNet-16G Post-Quantum Rollout Playbook
sidebar_label: PQ Rollout Plan
description: Operational guide for promoting the SoraNet hybrid X25519+ML-KEM handshake from canary to default across relays, clients, and SDKs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

SNNet-16G የድህረ-ኳንተም ልቀት ለሶራኔት ትራንስፖርት ጨርሷል። የ`rollout_phase` ማዞሪያዎች ኦፕሬተሮች ለያንዳንዱ ወለል ጥሬ JSON/TOML ሳያስተካከሉ የሚወስን ማስተዋወቂያን አሁን ካለው የደረጃ A የጥበቃ መስፈርት እስከ ደረጃ B አብላጫ ሽፋን እና የC ጥብቅ PQ አቀማመጥ እንዲያስተባብሩ ያስችላቸዋል።

ይህ የመጫወቻ መጽሐፍ የሚከተሉትን ያጠቃልላል

- የደረጃ ፍቺዎች እና አዲሱ የማዋቀሪያ ቁልፎች (`sorafs.gateway.rollout_phase` ፣ `sorafs.rollout_phase`) በኮድቤዝ (`crates/iroha_config/src/parameters/actual.rs:2230` ፣ `crates/iroha/src/config/user.rs:251`) ውስጥ ተሽረዋል።
- እያንዳንዱ ደንበኛ ልቀቱን መከታተል እንዲችል ኤስዲኬ እና CLI ባንዲራ ማካሄድ።
- የማስተላለፊያ/የደንበኛ የካናሪ መርሐግብር የሚጠበቁ ነገሮችን እና የማስተዋወቂያውን በር የሚይዙ የአስተዳደር ዳሽቦርዶች (`dashboards/grafana/soranet_pq_ratchet.json`)።
- የጥቅልል መንጠቆዎች እና የእሳት-ቁፋሮ runbook ([PQ ratchet runbook] (./pq-ratchet-runbook.md) ማጣቀሻዎች።

#የደረጃ ካርታ

| `rollout_phase` | ውጤታማ ማንነትን መደበቅ ደረጃ | ነባሪ ውጤት | የተለመደ አጠቃቀም |
|--------------------------------------------|
| `canary` | `anon-guard-pq` (ደረጃ ሀ) | መርከቧ በሚሞቅበት ጊዜ በእያንዳንዱ ወረዳ ቢያንስ አንድ PQ ጠባቂ ጠይቅ። | የመነሻ መስመር እና ቀደምት የካናሪ ሳምንታት። |
| `ramp` | `anon-majority-pq` (ደረጃ B) | ለ PQ ቅብብሎሽ ምርጫ>\u003e ሁለት ሦስተኛ ሽፋን; ክላሲካል ቅብብሎሽ እንደ ውድቀት ይቆያሉ። | ክልል-በ-ክልል ቅብብል ካናሪዎች; የኤስዲኬ ቅድመ እይታ ይቀየራል። |
| `default` | `anon-strict-pq` (ደረጃ ሐ) | PQ-ብቻ ወረዳዎችን ያስፈጽሙ እና የማውረድ ማንቂያዎችን አጥብቁ። | የመጨረሻ ማስተዋወቅ ቴሌሜትሪ እና የአስተዳደር መፈረም እንደተጠናቀቀ። |

አንድ ወለል ግልጽ የሆነ `anonymity_policy` ካዘጋጀ የዚያን አካል ደረጃ ይሽራል። ግልጽውን ደረጃ መተው አሁን ወደ `rollout_phase` እሴት ያስተላልፋል ስለዚህ ኦፕሬተሮች በየአካባቢው ደረጃውን አንድ ጊዜ እንዲገለብጡ እና ደንበኞች እንዲወርሱት ማድረግ ይችላሉ።

## የማዋቀር ማጣቀሻ

### ኦርኬስትራ (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

የኦርኬስትራ ጫኚው የውድቀት ደረጃን በሂደት (`crates/sorafs_orchestrator/src/lib.rs:2229`) ይፈታዋል እና በI18NI0000024X እና I18NI0000025X በኩል ያደርገዋል። ለተዘጋጁ ቅንጣቢዎች `docs/examples/sorafs_rollout_stage_b.toml` እና I18NI0000027X ይመልከቱ።

### ዝገት ደንበኛ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` አሁን የተተነተነውን ደረጃ (`crates/iroha/src/client.rs:2315`) ይመዘግባል ስለዚህ የረዳት ትዕዛዞች (ለምሳሌ `iroha_cli app sorafs fetch`) የአሁኑን ደረጃ ከነባሪ ማንነትን ከመደበቅ ፖሊሲ ጋር ሪፖርት ማድረግ ይችላል።

# አውቶሜሽን

ሁለት `cargo xtask` ረዳቶች የጊዜ ሰሌዳውን ማመንጨት እና አርቲፊክ ቀረጻን በራስ-ሰር ያዘጋጃሉ።

1. ** ክልላዊ የጊዜ ሰሌዳውን ይፍጠሩ **

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   ቆይታዎች `s`፣ `m`፣ `h`፣ ወይም I18NI0000036X ቅጥያዎችን ይቀበላሉ። ትዕዛዙ `artifacts/soranet_pq_rollout_plan.json` እና ከለውጥ ጥያቄ ጋር ሊላክ የሚችል የMarkdown ማጠቃለያ (`artifacts/soranet_pq_rollout_plan.md`) ያወጣል።

2. ** የመሰርሰሪያ ቅርሶችን በፊርማ ይያዙ ***

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   ትዕዛዙ የቀረቡትን ፋይሎች ወደ `artifacts/soranet_pq_rollout/<timestamp>_<label>/` ይገለብጣል፣ ለእያንዳንዱ አርቲፊሻል BLAKE3 መፍጨት ያሰላል እና `rollout_capture.json` ሜታዳታ እና ከክፍያ ጭነት በላይ የ Ed25519 ፊርማ ይጽፋል። አስተዳደር በፍጥነት መያዙን ማረጋገጥ እንዲችል የእሳት-ቁፋሮ ደቂቃዎችን የሚፈርም ተመሳሳይ የግል ቁልፍ ይጠቀሙ።

## ኤስዲኬ እና CLI ባንዲራ ማትሪክስ

| ወለል | ካናሪ (ደረጃ ሀ) | ራምፕ (ደረጃ B) | ነባሪ (ደረጃ ሐ) |
|--------|---
| `sorafs_cli` ማምጣት | `--anonymity-policy stage-a` ወይም ደረጃ ላይ መታመን | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| ኦርኬስትራ ማዋቀር JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| ዝገት ደንበኛ ውቅር (`iroha.toml`) | `rollout_phase = "canary"` (ነባሪ) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` የተፈረመ ትዕዛዞች | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| ጃቫ/አንድሮይድ `GatewayFetchOptions` | `setRolloutPhase("canary")`፣ እንደ አማራጭ `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`፣ እንደ አማራጭ I18NI0000061X | `setRolloutPhase("default")`፣ እንደ አማራጭ I18NI0000063X |
| ጃቫስክሪፕት ኦርኬስትራ ረዳቶች | `rolloutPhase: "canary"` ወይም `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| ስዊፍት `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

ሁሉም ኤስዲኬ በኦርኬስትራ (`crates/sorafs_orchestrator/src/lib.rs:365`) ወደ ተጠቀመበት ተመሳሳይ የመድረክ ተንታኝ ይቀይራል፣ ስለዚህ የድብልቅ ቋንቋ ማሰማራቶች ከተዋቀረው ደረጃ ጋር በመቆለፊያ ደረጃ ይቆያሉ።

## የካናሪ መርሐግብር ማረጋገጫ ዝርዝር

1. ** ቅድመ በረራ (ቲ ሲቀነስ 2 ሳምንታት)**

- ደረጃ አረጋግጥ ቡኒ መውጫ መጠን <1% ካለፉት ሁለት ሳምንታት እና PQ ሽፋን>=70% በክልል (`sorafs_orchestrator_pq_candidate_ratio`)።
   - የካናሪ መስኮትን የሚያጸድቀውን የአስተዳደር ግምገማ ማስገቢያ መርሐግብር ያስይዙ።
   - በመድረክ ላይ `sorafs.gateway.rollout_phase = "ramp"` ያዘምኑ (የኦርኬስትራውን JSON ያርትዑ እና እንደገና ይለማመዱ) እና የማስተዋወቂያ ቧንቧን ያደርቁ።

2. ** ሪሌይ ካናሪ (ቲ ቀን)**

   - `rollout_phase = "ramp"` በኦርኬስትራ እና በተሳታፊ ቅብብሎሽ ማሳያዎች ላይ በማስቀመጥ አንድ ክልል በአንድ ጊዜ ያስተዋውቁ።
   - "የመመሪያ ክስተቶችን በውጤት" እና "Brownout Rate" በPQ Ratchet ዳሽቦርድ (አሁን የልቀት ፓነልን የያዘው) የጥበቃ መሸጎጫ TTLን ለሁለት ጊዜ ይቆጣጠሩ።
   - ለኦዲት ማከማቻ ከሩጫ በፊት እና በኋላ የ `sorafs_cli guard-directory fetch` ቅጽበተ-ፎቶዎችን ይቁረጡ።

3. ** ደንበኛ/ኤስዲኬ ካናሪ (ቲ እና 1 ሳምንት)**

   - `rollout_phase = "ramp"`ን በደንበኛ ውቅሮች ውስጥ ገልብጥ ወይም `stage-b` ለተሰየሙት የኤስዲኬ ስብስቦች ይሽራል።
   - የቴሌሜትሪ ልዩነቶችን ያንሱ (`sorafs_orchestrator_policy_events_total` በ `client_id` እና `region` ተመድቦ) እና ከታቀደው ክስተት መዝገብ ጋር አያይዟቸው።

4. ** ነባሪ ማስተዋወቂያ (ቲ እና 3 ሳምንታት) ***

   - አንዴ የአስተዳደር ስራው ከጠፋ፣ ሁለቱንም ኦርኬስትራ እና የደንበኛ ውቅሮችን ወደ `rollout_phase = "default"` ይቀይሩ እና የተፈረመውን ዝግጁነት ማረጋገጫ ዝርዝር ወደ ተለቀቀው ቅርሶች ያሽከርክሩት።

## የአስተዳደር እና የማስረጃ ዝርዝር

| ደረጃ ለውጥ | የማስተዋወቂያ በር | ማስረጃ ጥቅል | ዳሽቦርዶች እና ማንቂያዎች |
|-------------|
| ካናሪ → ራምፕ *(የደረጃ B ቅድመ እይታ)* | ደረጃ-A ቡኒ መውጫ ፍጥነት <1% በመከታተል ላይ ባሉት 14 ቀናት፣ `sorafs_orchestrator_pq_candidate_ratio` ≥ 0.7 በአንድ ከፍ ባለ ክልል፣ የአርጎን2 ትኬት ማረጋገጫ p95 <50 ms፣ እና የማስተዋወቂያው የአስተዳደር ክፍተት ተይዟል። | `cargo xtask soranet-rollout-plan` JSON/Markdown ጥንድ፣ የተጣመሩ `sorafs_cli guard-directory fetch` ቅጽበተ-ፎቶዎች (በፊት/በኋላ)፣ የተፈረመ `cargo xtask soranet-rollout-capture --label canary` ቅርቅብ እና የካናሪ ደቂቃዎች ማጣቀሻ [PQ ratchet runbook](./pq-ratchet-runbook.md)። | `dashboards/grafana/soranet_pq_ratchet.json` (የመመሪያ ክንውኖች + ቡኒውት ተመን)፣ `dashboards/grafana/soranet_privacy_metrics.json` (SN16 የመቀነስ ሬሾ)፣ የቴሌሜትሪ ማጣቀሻዎች በ`docs/source/soranet/snnet16_telemetry_plan.md`። |
| ራምፕ → ነባሪ *(ደረጃ ሐ ማስፈጸሚያ)* | የ30-ቀን SN16 የቴሌሜትሪ ማቃጠል ተገናኘ፣ `sn16_handshake_downgrade_total` ጠፍጣፋ በመነሻ መስመር፣ I18NI0000097X ዜሮ ደንበኛ ካናሪ፣ እና የፕሮክሲ መቀያየር ልምምዱን ገብቷል። | `sorafs_cli proxy set-mode --mode gateway|direct` ግልባጭ፣ `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` ውፅዓት፣ `sorafs_cli guard-directory verify` ሎግ እና የተፈረመ `cargo xtask soranet-rollout-capture --label default` ጥቅል። | ተመሳሳይ PQ Ratchet ቦርድ እና የ SN16 ቁልቁል ፓነሎች በ `docs/source/sorafs_orchestrator_rollout.md` እና I18NI0000103X ውስጥ ተመዝግበው ይገኛሉ። |
| የአደጋ ጊዜ ዝቅ ማድረግ / ወደ ኋላ ዝግጁነት | የሚቀሰቀሰው የቆጣሪዎች ሲያድጉ፣ የጥበቃ-ማውጫ ማረጋገጫ ሳይሳካ ሲቀር፣ ወይም የ`/policy/proxy-toggle` ቋት መዝገቦች የመቀነስ ክስተቶችን ቀጥለዋል። | የማረጋገጫ ዝርዝር ከ `docs/source/ops/soranet_transport_rollback.md`፣ `sorafs_cli guard-directory import`/`guard-cache prune` ምዝግብ ማስታወሻዎች፣ `cargo xtask soranet-rollout-capture --label rollback`፣ የአደጋ ትኬቶች እና የማሳወቂያ አብነቶች። | `dashboards/grafana/soranet_pq_ratchet.json`፣ `dashboards/grafana/soranet_privacy_metrics.json`፣ እና ሁለቱም የማንቂያ ጥቅሎች (`dashboards/alerts/soranet_handshake_rules.yml`፣ `dashboards/alerts/soranet_privacy_rules.yml`)። |

- እያንዳንዱን ቅርስ በ`artifacts/soranet_pq_rollout/<timestamp>_<label>/` በተፈጠረው `rollout_capture.json` ያከማቹ ስለዚህ የአስተዳደር እሽጎች የውጤት ሰሌዳ፣ የፕሮምቶል ዱካዎች እና የምግብ መፍጫ አካላት ይይዛሉ።
- የSHA256 ዲጀስት የተሰቀሉ ማስረጃዎችን (ደቂቃ ፒዲኤፍ፣ የቀረጻ ቅርቅብ፣ የጥበቃ ቅጽበታዊ ገጽ እይታዎች) ከማስተዋወቂያ ደቂቃዎች ጋር በማያያዝ የፓርላማ ማጽደቆችን ወደ መድረክ ክላስተር ሳይደርሱ እንደገና እንዲጫወቱ ያድርጉ።
- `docs/source/soranet/snnet16_telemetry_plan.md` የቃላቶችን ደረጃ ዝቅ ለማድረግ እና የማስጠንቀቂያ ገደቦችን ለማረጋገጥ የቴሌሜትሪ እቅድን በማስተዋወቂያ ትኬት ውስጥ ያጣቅሱ።

## ዳሽቦርድ እና ቴሌሜትሪ ዝመናዎች

`dashboards/grafana/soranet_pq_ratchet.json` አሁን ከዚህ የመጫወቻ መጽሐፍ ጋር የሚያገናኝ እና የአስተዳደር ግምገማዎች የትኛው ደረጃ ገባሪ እንደሆነ የሚያረጋግጡ የ"የታቀደው እቅድ" ማብራሪያ ፓኔል ይልካል። የፓነል መግለጫውን ወደፊት በማዋቀር ቁልፎች ላይ ከሚደረጉ ለውጦች ጋር በማመሳሰል ያቆዩት።

ለማስጠንቀቅ፣ ነባር ደንቦች የI18NI000001117X መሰየሚያ መጠቀማቸውን ያረጋግጡ ካናሪ እና ነባሪ ደረጃዎች የተለየ የመመሪያ ገደቦችን (`dashboards/alerts/soranet_handshake_rules.yml`) ያስነሳሉ።

## የጥቅልል መንጠቆዎች

### ነባሪ → ራምፕ (ደረጃ C → ደረጃ ለ)

1. ኦርኬስትራውን በ`sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` ዝቅ ያድርጉ (እና በኤስዲኬ አወቃቀሮች ላይ ያለውን ተመሳሳይ ደረጃ ያንጸባርቁ) ስለዚህ ደረጃ B መርከቦችን በስፋት ይቀጥላል።
2. ደንበኞችን በ`sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` በኩል ወደ ደህንነቱ የተጠበቀ የትራንስፖርት ፕሮፋይል ያስገድዱ፣ ግልባጩን በመያዝ የ`/policy/proxy-toggle` የማስተካከያ የስራ ፍሰት ኦዲት ተደርጎ ይቆያል።
3. `cargo xtask soranet-rollout-capture --label rollback-default`ን በ`artifacts/soranet_pq_rollout/` ስር የጠባቂ-ማውጫ ልዩነቶችን፣ የፕሮምቶሉን ውፅዓት እና ዳሽቦርድ ቅጽበታዊ ገጽ እይታዎችን በማህደር ለማስቀመጥ ያሂዱ።

### ራምፕ → ካናሪ (ደረጃ B → ደረጃ ሀ)

1. ከማስታወቂያ በፊት የተቀረጸውን የጠባቂ-ማውጫ ቅጽበታዊ ገጽ እይታ በ`sorafs_cli guard-directory import --guard-directory guards.json` አስመጣ እና `sorafs_cli guard-directory verify` እንደገና አስጀምር ስለዚህ የማውረድ ፓኬቱ ሃሽን ያካትታል።
2. በኦርኬስትራ እና በደንበኛ ውቅሮች ላይ `rollout_phase = "canary"` (ወይም በ`anonymity_policy stage-a` መሻር) ያዋቅሩ፣ በመቀጠል የPQ ratchet drill ከ[PQ ratchet runbook](./pq-ratchet-runbook.md) ዝቅተኛ የቧንቧ መስመር ለማረጋገጥ።
3. አስተዳደርን ከማሳወቁ በፊት የተዘመነውን የPQ Ratchet እና SN16 ቴሌሜትሪ ስክሪፕቶች እና የማንቂያ ውጤቶችን ከክስተቱ መዝገብ ጋር ያያይዙ።

### የጥበቃ ሀዲድ አስታዋሾች- ማመሳከሪያ `docs/source/ops/soranet_transport_rollback.md` በማንኛውም ጊዜ ቅናሽ በሚከሰትበት ጊዜ እና ማንኛውንም ጊዜያዊ ቅነሳን እንደ `TODO:` ንጥል ለክትትል ሥራ በታቀደው መከታተያ ውስጥ ያስገቡ።
- `dashboards/alerts/soranet_handshake_rules.yml` እና `dashboards/alerts/soranet_privacy_rules.yml`ን በ`promtool test rules` ሽፋን ከጥቅልል በፊት እና በኋላ ያቆዩ ስለዚህ የማስጠንቀቂያ ተንሸራታች ከተቀረጸው ጥቅል ጋር ይመዘገባል።