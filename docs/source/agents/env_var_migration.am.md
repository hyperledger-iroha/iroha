---
lang: am
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-05T09:28:11.999442+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Env → የስደት መከታተያ አዋቅር

ይህ መከታተያ ምርትን የሚጋፈጡ አካባቢ-ተለዋዋጭ መቀያየሪያዎችን ወለል ብሎ ያሳያል
በ `docs/source/agents/env_var_inventory.{json,md}` እና የታሰበው ፍልሰት
ወደ `iroha_config` (ወይም ግልጽ ዲቪ/ሙከራ-ብቻ ወሰን) የሚወስድ መንገድ።


ማሳሰቢያ፡ `ci/check_env_config_surface.sh` አሁን አዲስ **ምርት** env
`ENV_CONFIG_GUARD_ALLOW=1` ካልሆነ በስተቀር ሺምስ ከ `AGENTS_BASE_REF` አንፃር ይታያል
አዘጋጅ; መሻርን ከመጠቀምዎ በፊት ሆን ተብሎ የተጨመሩትን እዚህ ይመዝግቡ።

## የተጠናቀቁ ፍልሰቶች- **IVM ABI መርጦ መውጣት** - ተወግዷል `IVM_ALLOW_NON_V1_ABI`; አቀናባሪው አሁን ውድቅ ያደርጋል
  v1 ያልሆኑ ኤቢአይዎች ያለምንም ቅድመ ሁኔታ የስህተቱን ዱካ የሚጠብቅ የክፍል ሙከራ።
- **IVM የማረሚያ ባነር env shim** - `IVM_SUPPRESS_BANNER` env መርጦ መውጣትን ተወ።
  ባነር ማፈን በፕሮግራማዊ አቀናባሪ በኩል ይገኛል።
- **IVM መሸጎጫ/መጠን** - ባለ ክር መሸጎጫ/ማሳያ/ጂፒዩ መጠን
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`፣
  `accel.max_gpus`) እና የአሂድ ጊዜ env shims ተወግዷል። አስተናጋጆች አሁን ይደውሉ
  `ivm::ivm_cache::configure_limits` እና `ivm::zk::set_prover_threads`፣ ሙከራዎች ጥቅም ላይ ይውላሉ
  የኢንቪ መሻር ፋንታ `CacheLimitsGuard`።
- ** የወረፋ ሥርን ያገናኙ *** - `connect.queue.root` ታክሏል (ነባሪ፡-
  `~/.iroha/connect`) ወደ ደንበኛው ማዋቀር እና በ CLI ውስጥ ፈትለው እና
  JS ምርመራዎች. የጄኤስ ረዳቶች አወቃቀሩን (ወይም ግልጽ የሆነ `rootDir`) እና
  በ `allowEnvOverride` በኩል በዴቭ/ሙከራ `IROHA_CONNECT_QUEUE_ROOT` ብቻ ያክብሩ።
  አብነቶች አንጓውን ይመዘግባሉ ስለዚህ ኦፕሬተሮች ከአሁን በኋላ env መሻር አያስፈልጋቸውም።
- **Izanami አውታረ መረብ መርጦ መግባት** — ግልጽ የሆነ `allow_net` CLI/config ባንዲራ ታክሏል ለ
  የ Izanami ትርምስ መሳሪያ; አሁን ለማሄድ `allow_net=true`/`--allow-net` እና
- **IVM ባነር ድምፅ** — `IROHA_BEEP` env shim በማዋቀር በሚነዳ ተተካ
  `ivm.banner.{show,beep}` መቀያየሪያዎች (ነባሪ፡ እውነት/እውነት)። የጅምር ባነር/ቢፕ
  የወልና አሁን በምርት ውስጥ ብቻ ውቅር ያነባል; ዴቭ/ሙከራ አሁንም ክብርን ይገነባል።
  በእጅ ለመቀያየር env መሻር።
- ** DA spool መሻር (ሙከራዎች ብቻ) *** - የ `IROHA_DA_SPOOL_DIR` መሻር አሁን ነው
  ከ `cfg(test)` ረዳቶች በስተጀርባ የታጠረ; የምርት ኮድ ሁል ጊዜ ስፖሉን ያመነጫል።
  ከማዋቀር መንገድ.
- ** ክሪፕቶ ውስጣዊ ነገሮች *** - ተተክቷል `IROHA_DISABLE_SM_INTRINSICS` /
  `IROHA_ENABLE_SM_INTRINSICS` በማዋቀር የሚመራ
  `crypto.sm_intrinsics` ፖሊሲ (`auto`/`force-enable`/`force-disable`) እና
  የ `IROHA_SM_OPENSSL_PREVIEW` ጠባቂውን አስወግዷል. አስተናጋጆች ፖሊሲውን በ
  ጅምር፣ አግዳሚ ወንበሮች/ሙከራዎች በ`CRYPTO_SM_INTRINSICS` እና በOpenSSL በኩል መርጠው መግባት ይችላሉ።
  ቅድመ እይታ አሁን የሚያከብረው የውቅር ባንዲራውን ብቻ ነው።
  Izanami ቀድሞውንም `--allow-net`/የቀጠለ ውቅረት ይፈልጋል፣ እና ሙከራዎች አሁን በ ላይ ይተማመናሉ።
  ከድባብ env መቀያየር ይልቅ ያ ቋጠሮ።
- ** FastPQ GPU ማስተካከያ *** - `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}` ታክሏል።
  የማዋቀር ቁልፎች (ነባሪዎች፡ `None`/`None`/`false`/`false`/`false`) እና በCLI ትንተና በኩል ፈትኑዋቸው።
  `FASTPQ_METAL_*`/`FASTPQ_DEBUG_*` ሺምስ አሁን እንደ ዴቭ/ሙከራ ውድቀት እና
  አንዴ ውቅረት ሲጫኑ ችላ ይባላሉ (ውቅሩ ሳይስተካከሉ በሚቀርባቸው ጊዜም ቢሆን)። ሰነዶች/እቃዎች ነበሩ።
  ስደትን ለመጠቆም ታደሰ።【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  (`IVM_DECODE_TRACE`፣ `IVM_DEBUG_WSV`፣ `IVM_DEBUG_COMPACT`፣ `IVM_DEBUG_INVALID`፣
  `IVM_DEBUG_REGALLOC`፣ `IVM_DEBUG_METAL_ENUM`፣ `IVM_DEBUG_METAL_SELFTEST`፣
  `IVM_FORCE_METAL_ENUM`፣ `IVM_FORCE_METAL_SELFTEST_FAIL`፣ `IVM_FORCE_CUDA_SELFTEST_FAIL`፣
  `IVM_DISABLE_METAL`፣ `IVM_DISABLE_CUDA`) አሁን በጋራ ከማረሚያ/ሙከራ ግንባታዎች ጀርባ ተዘግተዋል
  ረዳት ስለዚህ የምርት ሁለትዮሽዎች ለአካባቢያዊ ምርመራዎች ቁልፎችን በሚጠብቁበት ጊዜ ችላ ይላቸዋል። ኢንቨስት
  የ dev/የሙከራ-ብቻ ወሰንን ለማንፀባረቅ ቆጠራ እንደገና ተፈጠረ።- ** FASTPQ ቋሚ ዝመናዎች *** - `FASTPQ_UPDATE_FIXTURES` አሁን በ FASTPQ ውህደት ውስጥ ብቻ ነው የሚታየው
  ፈተናዎች; የምርት ምንጮች የኢንቪ መቀያየርን አያነቡም እና እቃው የሙከራ-ብቻውን ያንፀባርቃል
  ስፋት.
- **የኢንቬንቶሪ እድሳት + ወሰን ማወቂያ** — የኢንቪ ቆጠራ መሣሪያ አሁን `build.rs` ፋይሎችን እንደ መለያ ይሰጣል
  ወሰን ይገንቡ እና `#[cfg(test)]`/የመዋሃድ ማሰሪያ ሞጁሎችን ይከታተላል ስለዚህ ለሙከራ ብቻ ይቀየራል (ለምሳሌ፣
  `IROHA_TEST_*`፣ `IROHA_RUN_IGNORED`) እና CUDA የግንባታ ባንዲራዎች ከምርት ብዛት ውጭ ይታያሉ።
  ኢንቬንቶሪ በዲሴምበር 07፣ 2025 (518 refs / 144 vars) የኤንቭ-ውቅር ጠባቂ ልዩነትን አረንጓዴ ለማቆየት ተፈጠረ።
- ** P2P ቶፖሎጂ env shim መልቀቂያ ጠባቂ *** - `IROHA_P2P_TOPOLOGY_UPDATE_MS` አሁን መወሰኛ ቀስቅሷል
  የጅምር ስህተት በመልቀቂያ ግንባታዎች (ማስጠንቀቅ-በማረሚያ/ሙከራ ላይ ብቻ) ስለዚህ የምርት አንጓዎች የሚታመኑት
  `network.peer_gossip_period_ms`. የኢንቪው ክምችት ጠባቂውን እና የ
  የዘመነ ክላሲፋየር አሁን `cfg!`-የተጠበቁ መቀያየርን እንደ ማረም/ሙከራ ይሸፍናል።

## ከፍተኛ ቅድሚያ የሚሰጠው ፍልሰት (የምርት መንገዶች)

- _ምንም (የእቅድ ክምችት በ cfg ታድሷል!/ማረሚያ ማግኘቱ፤ env-config guard green from P2P shim hardening)

## ዴቭ/ሙከራ-ብቻ ወደ አጥር ይቀየራል።

- የአሁኑ መጥረግ (ታህሳስ 07፣ 2025)፡-ግንባታ-ብቻ CUDA ባንዲራዎች (`IVM_CUDA_*`) እንደ `build` እና
  የመታጠቂያ መቀየሪያዎች (`IROHA_TEST_*`, `IROHA_RUN_IGNORED`, `IROHA_SKIP_BIND_CHECKS`) አሁን ይመዝገቡ እንደ
  `test`/`debug` በዕቃው ውስጥ (`cfg!` የተጠበቁ ሺምስን ጨምሮ)። ተጨማሪ አጥር አያስፈልግም;
  የወደፊት ተጨማሪዎችን ከ `cfg(test)`/ቤንች-ብቻ ረዳቶች ከ TODO ምልክቶች ጋር ጊዜያዊ ሲሆኑ ያቆዩ።

## የግንባታ ጊዜ ኢንቨስ (እንደሆነ ይውጡ)

- ጭነት/ባህሪ ኢንቪስ (`CARGO_*`፣ `OUT_DIR`፣ `DOCS_RS`፣ `PROFILE`፣ `CUDA_HOME`፣
  `CUDA_PATH`፣ `JSONSTAGE1_CUDA_ARCH`፣ `FASTPQ_SKIP_GPU_BUILD`፣ ወዘተ) ይቀራሉ
  የግንባታ-ስክሪፕት ስጋቶች እና ለሩጫ ጊዜ ማዋቀር ከወሰን ውጪ ናቸው።

## ቀጣይ ድርጊቶች

1) አዲስ የምርት env shims ለመያዝ ከውቅረት-የገጽታ ዝመናዎች በኋላ `make check-env-config-surface` ን ያሂዱ
   ቀደም ብሎ እና የንዑስ ስርዓት ባለቤቶችን/ኢታኤዎችን ይመድቡ።  
2) ከእያንዳንዱ መጥረግ በኋላ እቃውን ያድሱ (`make check-env-config-surface`)
   መከታተያው ከአዲስ የጥበቃ ሀዲዶች ጋር ተስተካክሎ ይቆያል እና env-config guard diff ከድምፅ ነፃ ሆኖ ይቆያል።