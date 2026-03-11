---
lang: am
direction: ltr
source: docs/source/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7548d481edd33d7e325d22559a5f53f261fa302ffd8710a1626acc4a5705e428
source_last_modified: "2025-12-29T18:16:35.915400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha ቪኤም + Kotodama ሰነዶች መረጃ ጠቋሚ

ይህ ኢንዴክስ የ IVM፣ Kotodama እና IVM-የመጀመሪያውን የቧንቧ መስመር ዋና ዲዛይን እና ማጣቀሻ ሰነዶችን ያገናኛል። 日本語訳は [`README.ja.md`](./README.ja.md) を参照してください。

- IVM አርክቴክቸር እና የቋንቋ ካርታ፡ `../../ivm.md`
- IVM syscall ABI፡ `ivm_syscalls.md`
- የመነጩ syscall ቋሚዎች፡ `ivm_syscalls_generated.md` (ለማደስ `make docs-syscalls` ን ያሂዱ)
- IVM ባይትኮድ ራስጌ፡ `ivm_header.md`
- Kotodama ሰዋሰው እና ትርጓሜ፡ `kotodama_grammar.md`
- Kotodama ምሳሌዎች እና syscall ካርታዎች፡ `kotodama_examples.md`
- የግብይት ቧንቧ መስመር (IVM-መጀመሪያ): `../../new_pipeline.md`
- Torii ኮንትራቶች ኤፒአይ (መግለጫዎች)፡ `torii_contracts_api.md`
- ሁለንተናዊ መለያ/UAID ኦፕሬሽኖች መመሪያ፡ `universal_accounts_guide.md`
- JSON መጠይቅ ፖስታ (CLI / tooling): `query_json.md`
- Norito የዥረት ሞጁል ማጣቀሻ፡ `norito_streaming.md`
- የሩጫ ጊዜ ABI ናሙናዎች፡ `samples/runtime_abi_active.md`፣ `samples/runtime_abi_hash.md`፣ `samples/find_active_abi_versions.md`
- ZK መተግበሪያ API (አባሪዎች፣ prover፣ የድምጽ መጠን): `zk_app_api.md`
- Torii ZK አባሪዎች/prover runbook: `zk/prover_runbook.md`
- Torii ZK መተግበሪያ ኤፒአይ ኦፕሬተር መመሪያ (አባሪዎች/ፕሮቨር፣ crate doc)፡ `../../crates/iroha_torii/docs/zk_app_api.md`
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- ቪኬ/ማስረጃ የሕይወት ዑደት (መዝገብ ፣ ማረጋገጫ ፣ ቴሌሜትሪ): `zk/lifecycle.md`
- Torii ኦፕሬተር ኤድስ (የታይነት የመጨረሻ ነጥቦች)፡ `references/operator_aids.md`
- Nexus ነባሪ መስመር ፈጣን ጅምር፡ `quickstart/default_lane.md`
- MOCHI ሱፐርቫይዘር ፈጣን ጅምር እና አርክቴክቸር፡ `mochi/index.md`
- ጃቫ ስክሪፕት ኤስዲኬ መመሪያዎች (ፈጣን ጅምር፣ ውቅር፣ ማተም): `sdk/js/index.md`
- ስዊፍት ኤስዲኬ እኩልነት/CI ዳሽቦርዶች፡ `references/ios_metrics.md`
- አስተዳደር: `../../gov.md`
- የጎራ ድጋፍ (ኮሚቴዎች፣ ፖሊሲዎች፣ ማረጋገጫዎች)፡ `domain_endorsements.md`
- የጄዲጂ ማረጋገጫዎች (ከመስመር ውጭ የማረጋገጫ መሳሪያ)፡ `jdg_attestations.md`
- የማብራሪያ ማስተባበሪያ ጥያቄዎች: `coordination_llm_prompts.md`
- የመንገድ ካርታ: `../../roadmap.md`
- Docker ግንበኛ ምስል አጠቃቀም፡ `docker_build.md`

የአጠቃቀም ምክሮች
- ውጫዊ መሳሪያዎችን (`koto_compile`፣ `ivm_run`) በመጠቀም ምሳሌዎችን በ`examples/` ይገንቡ እና ያሂዱ።
  - `make examples-run` (እና `make examples-inspect` ካለ)
- የአማራጭ ውህደት ሙከራዎች (በነባሪነት ችላ የተባሉ) ምሳሌዎች እና የርዕስ ፍተሻዎች በቀጥታ በ`integration_tests/tests/`።የቧንቧ መስመር ውቅር
- ሁሉም የሩጫ ጊዜ ባህሪ በ`iroha_config` ፋይሎች ነው የተዋቀረው። የአካባቢ ተለዋዋጮች ለኦፕሬተሮች ጥቅም ላይ አይውሉም.
- አስተዋይ ነባሪዎች ቀርበዋል; አብዛኛዎቹ ማሰማራት ለውጦች አያስፈልጋቸውም።
- ተዛማጅ ቁልፎች በ `[pipeline]`:
  - `dynamic_prepass`፡ የመዳረሻ ስብስቦችን ለማግኘት IVM ንባብ-ብቻ ፕሪፓስን አንቃ (ነባሪ፡ እውነት)።
  - `access_set_cache_enabled`: መሸጎጫ የተገኘ የመዳረሻ ስብስቦች በ `(code_hash, entrypoint)`; ፍንጮችን ለማረም አሰናክል (ነባሪ፡ እውነት)።
  - `parallel_overlay`: ተደራቢዎችን በትይዩ መገንባት; ቁርጠኝነት ይቀራል (ነባሪ፡ እውነት)።
  - `gpu_key_bucket`: በ `(key, tx_idx, rw_flag)` ላይ የተረጋጋ ራዲክስ በመጠቀም የጊዜ ሰሌዳ አስማሚው አማራጭ ቁልፍ ባልዲ; የሚወስን የሲፒዩ ውድቀት ሁል ጊዜ ንቁ ነው (ነባሪ፡ ሐሰት)።
  - `cache_size`፡ የዓለማቀፉ IVM ቅድመ-ዲኮድ መሸጎጫ አቅም (የተለያዩ ዥረቶች)። ነባሪ፡ 128. መጨመር ለተደጋጋሚ ግድያዎች ጊዜን መፍታት ይቀንሳል።

ሰነዶች የማመሳሰል ፍተሻዎች
- ሲስካል ቋሚዎች (ሰነዶች/ምንጭ/ivm_syscals_generated.md)
  - እንደገና ማመንጨት: `make docs-syscalls`
  - ብቻ ያረጋግጡ: `bash scripts/check_syscalls_doc.sh`
- Syscall ABI ሰንጠረዥ (crates/ivm/docs/syscalls.md)
  - ብቻ ያረጋግጡ: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - የመነጨውን ክፍል (እና የኮድ ሰነዶች ሰንጠረዥ) ያዘምኑ፡ `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- ጠቋሚ-ABI ሠንጠረዦች (ሳጥኖች/ivm/docs/pointer_abi.md እና ivm.md)
  - ብቻ ያረጋግጡ: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - ክፍሎችን አዘምን: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM ራስጌ ፖሊሲ እና ABI hashes (docs/source/ivm_header.md)
  - ቼክ ብቻ፡ `cargo run -p ivm --bin gen_header_doc -- --check` እና `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - ክፍሎችን ያዘምኑ፡ `cargo run -p ivm --bin gen_header_doc -- --write` እና `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

ሲ.አይ
- GitHub Actions የስራ ፍሰት `.github/workflows/check-docs.yml` እነዚህን ቼኮች በእያንዳንዱ ግፊት/PR ላይ ያካሂዳል እና የመነጩ ሰነዶች ከትግበራው ከተሳፉ አይሳካም።
- [የመንግስት መጫወቻ መጽሐፍ](governance_playbook.md)
