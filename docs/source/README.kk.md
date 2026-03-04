---
lang: kk
direction: ltr
source: docs/source/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7548d481edd33d7e325d22559a5f53f261fa302ffd8710a1626acc4a5705e428
source_last_modified: "2025-12-29T18:16:35.915400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha VM + Kotodama құжаттар индексі

Бұл индекс IVM, Kotodama және IVM-бірінші құбыр желісінің негізгі жобалау және анықтамалық құжаттарын байланыстырады. 訳は [`README.ja.md`](./README.ja.md) を参照してください。

- IVM архитектурасы және тіл картасы: `../../ivm.md`
- IVM ABI жүйелік қоңырауы: `ivm_syscalls.md`
- Жасалған жүйелік қоңырау тұрақтылары: `ivm_syscalls_generated.md` (жаңарту үшін `make docs-syscalls` іске қосыңыз)
- IVM байт коды тақырыбы: `ivm_header.md`
- Kotodama грамматика және семантика: `kotodama_grammar.md`
- Kotodama мысалдары мен жүйені салыстыру: `kotodama_examples.md`
- Транзакция құбыры (IVM‑бірінші): `../../new_pipeline.md`
- Torii Contracts API (манифесттер): `torii_contracts_api.md`
- Әмбебап тіркелгі/UAID операциялық нұсқаулығы: `universal_accounts_guide.md`
- JSON сұрау конверті (CLI / құрал): `query_json.md`
- Norito ағындық модулінің анықтамасы: `norito_streaming.md`
- Орындалу уақытының ABI үлгілері: `samples/runtime_abi_active.md`, `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- ZK App API (тіркемелер, дәлелдеу, дауыс саны): `zk_app_api.md`
- Torii ZK тіркемелері/prover runbook: `zk/prover_runbook.md`
- Torii ZK App API операторының нұсқаулығы (тіркемелер/провер; жәшік құжаты): `../../crates/iroha_torii/docs/zk_app_api.md`
- VK/proof өмірлік циклі (тізілім, тексеру, телеметрия): `zk/lifecycle.md`
- Torii Операторға көмекші құралдар (көріну үшін соңғы нүктелер): `references/operator_aids.md`
- Nexus әдепкі жолақты жылдам іске қосу: `quickstart/default_lane.md`
- MOCHI супервайзері жылдам іске қосу және архитектурасы: `mochi/index.md`
- JavaScript SDK нұсқаулықтары (жылдам іске қосу, конфигурациялау, жариялау): `sdk/js/index.md`
- Swift SDK паритеті/CI бақылау тақталары: `references/ios_metrics.md`
- Басқару: `../../gov.md`
- Доменді растау (комитеттер, саясаттар, тексеру): `domain_endorsements.md`
- JDG аттестациялары (офлайн тексеру құралы): `jdg_attestations.md`
- Түсіндіруді үйлестіру нұсқаулары: `coordination_llm_prompts.md`
- Жол картасы: `../../roadmap.md`
- Docker құрастырушы кескінін пайдалану: `docker_build.md`

Қолдану бойынша кеңестер
- Сыртқы құралдарды (`koto_compile`, `ivm_run`) пайдаланып `examples/` ішінде мысалдарды құрастырыңыз және іске қосыңыз:
  - `make examples-run` (және `ivm_tool` болса, `make examples-inspect`)
- Мысалдар мен тақырыпты тексеруге арналған қосымша біріктіру сынақтары (әдепкі бойынша еленбейді) `integration_tests/tests/`.Құбыр конфигурациясы
- Барлық орындау уақытының әрекеті `iroha_config` файлдары арқылы конфигурацияланады. Ортаның айнымалы мәндері операторлар үшін пайдаланылмайды.
- Ақылға қонымды әдепкілер қамтамасыз етілген; орналастырулардың көпшілігі өзгерістерді қажет етпейді.
- `[pipeline]` астында сәйкес кілттер:
  - `dynamic_prepass`: кіру жиындарын алу үшін IVM тек оқуға арналған алдын ала өтуді қосыңыз (әдепкі: шын).
  - `access_set_cache_enabled`: `(code_hash, entrypoint)` үшін кэштен алынған қол жеткізу жиындары; кеңестерді жөндеуді өшіру (әдепкі: шын).
  - `parallel_overlay`: қабаттасуды параллель құрастыру; commit детерминистік болып қалады (әдепкі: шын).
  - `gpu_key_bucket`: `(key, tx_idx, rw_flag)` жүйесіндегі тұрақты радиксті пайдаланып жоспарлаушы алдын ала өту үшін қосымша кілттерді шелектеу; детерминирленген процессордың қалпына келуі әрқашан белсенді (әдепкі: жалған).
  - `cache_size`: жаһандық IVM алдын ала декодтау кэшінің сыйымдылығы (декодталған ағындар сақталады). Әдепкі: 128. Көбейту қайталанатын орындалулар үшін декодтау уақытын қысқартуы мүмкін.

Docs синхрондауды тексеру
- Жүйелік қоңырау тұрақтылары (docs/source/ivm_syscalls_generated.md)
  - Қайта құру: `make docs-syscalls`
  - Тек тексеру: `bash scripts/check_syscalls_doc.sh`
- Syscall ABI кестесі (crates/ivm/docs/syscalls.md)
  - Тек тексеру: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - Жасалған бөлімді жаңарту (және кодтық құжаттар кестесі): `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Pointer‑ABI кестелері (crates/ivm/docs/pointer_abi.md және ivm.md)
  - Тек тексеру: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - Жаңарту бөлімдері: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM тақырып саясаты және ABI хэштері (docs/source/ivm_header.md)
  - Тек тексеру: `cargo run -p ivm --bin gen_header_doc -- --check` және `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - Жаңарту бөлімдері: `cargo run -p ivm --bin gen_header_doc -- --write` және `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI
- GitHub Actions жұмыс процесі `.github/workflows/check-docs.yml` бұл тексерулерді әрбір push/PR кезінде іске қосады және жасалған құжаттар іске асырудан ауытқып кетсе, сәтсіз болады.
- [Басқару кітабы](governance_playbook.md)