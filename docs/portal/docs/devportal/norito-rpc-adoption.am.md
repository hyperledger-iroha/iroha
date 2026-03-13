---
lang: am
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 39cbd5e448c8a868c50401c15466d7159cb08ad7be52dafb9e9dc66d5bba979d
source_last_modified: "2025-12-29T18:16:35.105044+00:00"
translation_last_reviewed: 2026-02-07
id: norito-rpc-adoption
title: Norito-RPC Adoption Schedule
sidebar_label: Norito-RPC adoption
description: Cross-SDK rollout plan, evidence checklist, and automation hooks for roadmap item NRPC-4.
translator: machine-google-reviewed
---

> የቀኖናዊ እቅድ ማስታወሻዎች በ`docs/source/torii/norito_rpc_adoption_schedule.md` ውስጥ ይኖራሉ።  
> ይህ የፖርታል ቅጂ ለኤስዲኬ ደራሲዎች፣ ኦፕሬተሮች እና ገምጋሚዎች የሚጠበቀውን የልቀት ተስፋ ያጠፋል።

# አላማዎች

- እያንዳንዱን ኤስዲኬ (Rust CLI፣ Python፣ JavaScript፣ Swift፣ Android) በሁለትዮሽ Norito-RPC ትራንስፖርት ከ AND4 ምርት መቀያየር በፊት አሰልፍ።
- የደረጃ በሮች፣ የማስረጃ እሽጎች እና የቴሌሜትሪ መንጠቆዎች ወሳኙን ያቆዩ አስተዳደር ልቀቱን ኦዲት ማድረግ ይችላል።
- የመንገድ ካርታ NRPC-4 ከሚጠራው የጋራ ረዳቶች ጋር ቋሚ እና የካናሪ ማስረጃዎችን ለመያዝ ቀላል ያድርጉት።

## ደረጃ የጊዜ መስመር

| ደረጃ | መስኮት | ወሰን | መውጫ መስፈርት |
|-------|--------|-------|
| ** P0 - የላብራቶሪ እኩልነት *** | Q22025 | Rust CLI + Python smoke suites `/v2/norito-rpc` በCI ውስጥ ያካሂዳሉ፣ JS አጋዥ የዩኒት ፈተናዎችን ያልፋል፣ አንድሮይድ ሞክ ታጥቆ ባለሁለት መጓጓዣዎችን ይሠራል። | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` እና `javascript/iroha_js/test/noritoRpcClient.test.js` አረንጓዴ በ CI; አንድሮይድ መታጠቂያ ወደ I18NI0000023X ተሽሯል። |
| ** P1 - የኤስዲኬ ቅድመ እይታ *** | Q32025 | የጋራ መገልገያ ቅርቅብ ተመዝግቦ ገብቷል፣ `scripts/run_norito_rpc_fixtures.sh --sdk <label>` መዝገቦች ምዝግብ ማስታወሻዎች + JSON በI18NI0000025X፣ አማራጭ Norito የትራንስፖርት ባንዲራዎች በኤስዲኬ ናሙናዎች ውስጥ ተጋልጠዋል። | የማሳያ አንጸባራቂ ተፈርሟል፣ README ዝማኔዎች የመርጦ መግቢያ አጠቃቀምን ያሳያሉ፣ የስዊፍት ቅድመ እይታ API ከ IOS2 ባንዲራ ጀርባ ይገኛል። |
| ** P2 - ደረጃ / AND4 ቅድመ እይታ *** | Q12026 | የደረጃ Torii ገንዳዎች I18NT0000002Xን፣ የአንድሮይድ AND4 ቅድመ እይታ ደንበኞችን እና የስዊፍት IOS2 ፓሪቲ ስብስቦችን ከሁለትዮሽ ትራንስፖርት ነባሪ ይመርጣሉ፣ ቴሌሜትሪ ዳሽቦርድ `dashboards/grafana/torii_norito_rpc_observability.json` ተሞልቷል። | `docs/source/torii/norito_rpc_stage_reports.md` ካናሪውን፣ `scripts/telemetry/test_torii_norito_rpc_alerts.sh` ማለፊያዎችን ይይዛል፣ የአንድሮይድ ሞክ ማሰሪያ መልሶ ማጫወት የስኬት/የስህተት ጉዳዮችን ይይዛል። |
| ** P3 - ምርት GA *** | Q42026 | Norito ለሁሉም ኤስዲኬዎች ነባሪ መጓጓዣ ይሆናል። JSON እንደ ቡኒ ውድቀት ሆኖ ይቆያል። የተለቀቁ ስራዎች የእኩልነት ቅርሶችን በእያንዳንዱ መለያ ያከማቹ። | የፍተሻ ዝርዝር ቅርቅቦችን ይልቀቁ Norito የጭስ ውፅዓት ለ Rust/JS/Python/Swift/Android; ለNorito vs JSON የስህተት መጠን SLOs የማንቂያ ገደቦች ተፈጻሚ ሆነዋል። `status.md` እና የተለቀቁ ማስታወሻዎች የ GA ማስረጃን ይጠቅሳሉ። |

## ኤስዲኬ መላኪያዎች እና CI መንጠቆዎች

- ** Rust CLI & Integation Harness *** - `iroha_cli pipeline` የጭስ ሙከራዎችን አንድ ጊዜ Norito መጓጓዣን ለማስገደድ `cargo xtask norito-rpc-verify` መሬቶችን ያራዝሙ። በ`cargo test -p integration_tests -- norito_streaming` (ላብራቶሪ) እና `cargo xtask norito-rpc-verify` (ስቴጅንግ/ጂኤ)፣ በ`artifacts/norito_rpc/` ስር ያሉ ቅርሶችን ጠብቅ።
- **Python ኤስዲኬ** - የሚለቀቀውን ጭስ (`python/iroha_python/scripts/release_smoke.sh`) ወደ Norito RPC ነባሪ፣ `run_norito_rpc_smoke.sh` እንደ CI መግቢያ ነጥብ ያቆይ እና በ`python/iroha_python/README.md` ውስጥ እኩል አያያዝን ያቅርቡ። CI ኢላማ: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- ** ጃቫ ስክሪፕት ኤስዲኬ *** - `NoritoRpcClient` ን ያረጋጋል፣ የአስተዳደር/ጥያቄ ረዳቶች Norito በ `toriiClientConfig.transport.preferred === "norito_rpc"` ጊዜ ነባሪ ይፍቀዱ እና ከጫፍ እስከ ጫፍ ናሙናዎችን በ `javascript/iroha_js/recipes/` ይቅረጹ። CI ከመታተሙ በፊት `npm test` እና dockerized `npm run test:norito-rpc` ሥራን ማስኬድ አለበት። የፕሮቬንሽን ሰቀላዎች I18NT0000009X የጭስ ማውጫ በ `javascript/iroha_js/artifacts/` ስር።
- ** ፈጣን ኤስዲኬ *** - ከ IOS2 ባንዲራ በስተጀርባ ያለውን የNorito ድልድይ ማጓጓዣን ሽቦ ያድርጉ ፣ የቋሚውን ትክክለኛነት ያንፀባርቁ እና የግንኙነት / Norito የፓርቲ ስብስብ በ `docs/source/sdk/swift/index.md` ውስጥ በተጠቀሰው የBuildkite መስመሮች ውስጥ መሄዱን ያረጋግጡ።
- **አንድሮይድ ኤስዲኬ** – የ AND4 ቅድመ እይታ ደንበኞች እና የማስመሰያው Torii መታጠቂያ Norito ተቀብለዋል፣ በ `docs/source/sdk/android/networking.md` ውስጥ በድጋሚ በመሞከር/በኋላ አጥፋ ቴሌሜትሪ ተመዝግቧል። መታጠቂያው መገልገያዎችን ከሌሎች ኤስዲኬዎች ጋር በI18NI0000047X በኩል ያጋራል።

## ማስረጃ እና አውቶማቲክ

- `scripts/run_norito_rpc_fixtures.sh` ይጠቀልላል `cargo xtask norito-rpc-verify`፣ stdout/stderr ን ይይዛል እና `fixtures.<sdk>.summary.json` ያወጣል ስለዚህ የኤስዲኬ ባለቤቶች ከ`status.md` ጋር የሚያያይዘው ቆራጥ አርቴፋክት አላቸው። የCI ጥቅሎችን ንፁህ ለማድረግ `--sdk <label>` እና `--out artifacts/norito_rpc/<stamp>/` ይጠቀሙ።
- `cargo xtask norito-rpc-verify` schema hash parity (`fixtures/norito_rpc/schema_hashes.json`) ያስፈጽማል እና Torii `X-Iroha-Error-Code: schema_mismatch` ከመለሰ አልተሳካም። እያንዳንዱን ውድቀት ለማረም ከJSON የኋላ ቀረጻ ጋር ያጣምሩ።
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` እና `dashboards/grafana/torii_norito_rpc_observability.json` ለNRPC-2 የማንቂያ ውል ይገልፃሉ። ከእያንዳንዱ ዳሽቦርድ አርትዖት በኋላ ስክሪፕቱን ያሂዱ እና የ`promtool` ውፅዓት በካናሪ ጥቅል ውስጥ ያከማቹ።
- `docs/source/runbooks/torii_norito_rpc_canary.md` የዝግጅት እና የምርት ልምምዶችን ይገልፃል; ቋሚ ሃሽ ወይም የማንቂያ በሮች ሲቀየሩ ያዘምኑት።

## የገምጋሚ ማረጋገጫ ዝርዝር

የNRPC-4 ወሳኝ ደረጃ ላይ ምልክት ከማድረግዎ በፊት፣ ያረጋግጡ፡-

1. የቅርብ ጊዜ የቋሚ ቅርቅብ hashes `fixtures/norito_rpc/schema_hashes.json` እና ተዛማጅ CI artefact በ `artifacts/norito_rpc/<stamp>/` ተመዝግቧል።
2. ኤስዲኬ README / portal docs የJSON ውድቀትን እንዴት ማስገደድ እንደሚቻል ይገልፃሉ እና የNorito የትራንስፖርት ነባሪ ይጥቀሱ።
3. የቴሌሜትሪ ዳሽቦርዶች ባለሁለት-ቁልል ስህተት-ተመን ፓነሎችን የማንቂያ ማገናኛዎች ያሳያሉ፣ እና የ Alertmanager dry run (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) ከመከታተያው ጋር ተያይዟል።
4. እዚህ ያለው የጉዲፈቻ መርሃ ግብር ከመከታተያ ግቤት (`docs/source/torii/norito_rpc_tracker.md`) እና የመንገድ ካርታው (NRPC-4) ከተመሳሳይ ማስረጃ ጥቅል ጋር ይዛመዳል።

በጊዜ ሰሌዳው ላይ በዲሲፕሊን መቆየቱ-የኤስዲኬ ባህሪን ሊተነበይ የሚችል እና የአስተዳደር ቁጥጥር Norito-RPC ያለአንዳች ጥያቄ መቀበል ያስችላል።