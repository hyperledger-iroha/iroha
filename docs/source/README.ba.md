---
lang: ba
direction: ltr
source: docs/source/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7548d481edd33d7e325d22559a5f53f261fa302ffd8710a1626acc4a5705e428
source_last_modified: "2025-12-29T18:16:35.915400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha ВМ + Kotodama Docs индексы

Был индекс IVM, Kotodama өсөн төп проектлау һәм белешмә документтарын, IVM‐беренсе торба өсөн бәйләй. 日本語訳は [`README.ja.md`](./README.ja.md) を参照しくくだい。

- IVM архитектураһы һәм тел картаһы: `../../ivm.md`
- IVM syscall ABI: `ivm_syscalls.md`
- генерацияланған syscall константалары: `ivm_syscalls_generated.md` (йөрөү `make docs-syscalls` яңыртыу өсөн)
- IVM байтакод башы: `ivm_header.md`
- Kotodama грамматика һәм семантика: `kotodama_grammar.md` X
- Kotodama миҫалдары һәм syscall карталары: `kotodama_examples.md`
- Транзакция торбаһы (IVM‐беренсе): `../../new_pipeline.md`
- Torii контракттары API (күстәр): `torii_contracts_api.md`
- Универсаль иҫәп/УАИД операциялары етәксеһе: `universal_accounts_guide.md`
- JSON эҙләү конверты (CLI / инструмент): `query_json.md`
- Norito потоковый модуль һылтанма: `norito_streaming.md`
- АБИ өлгөләре: `samples/runtime_abi_active.md`, `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md` X.
- ZK App API (ҡушымталар, иҫбатлаусы, тауыш биргәндә): `zk_app_api.md`X
- Torii ZK ҡушымталары/довер номиналь китабы: `zk/prover_runbook.md`
- Torii ZK App API операторы етәксеһе (беркетмәһе/губерна; йәшник doc): `../../crates/iroha_torii/docs/zk_app_api.md`
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- ВК/иҫбатлау тормош циклы (реестр, тикшерелгән, телеметрия): `zk/lifecycle.md`
- Torii оператор ярҙамы (күренеш өсөн нөктәләр): `references/operator_aids.md`
- Nexus стандарт-трасса старт: `quickstart/default_lane.md`
- MOCHI етәксеһе quickstart & архитектура: `mochi/index.md`
- JavaScript SDK етәкселәр (тиҙstart, конфигурация, баҫтырыу): `sdk/js/index.md`X
- Свифт SDK паритеты/CI приборҙар таҡталары: `references/ios_metrics.md`
- Идара итеү: `../../gov.md`
- Домендарҙы хуплау (комитеттар, сәйәсәт, раҫлау): `domain_endorsements.md` X
- JDG аттестациялары (офлайн раҫлау инструменттары): `jdg_attestations.md`
- Асыҡлау координацияһы тураһында өндәүҙәр: `coordination_llm_prompts.md`
- Юл картаһы: `../../roadmap.md`
- Docker төҙөүсе һүрәтен ҡулланыу: `docker_build.md`

Ҡулланыу кәңәштәре
- Тышҡы ҡоралдар ярҙамында `examples/`-ла (`koto_compile`, `ivm_run`) миҫалдар төҙөү һәм эшләү):
  - `make examples-run` (һәм `make examples-inspect`, әгәр `ivm_tool` булһа)
- Миҫалдар һәм баш чектар өсөн `integration_tests/tests/` тура эфирҙа интеграция интеграцияһы һынауҙары (ғәҙәттән тыш) тура килә.Торба конфигурацияһы
- Бөтә йөрөү ваҡыты тәртибе `iroha_config` файлдары аша конфигурациялана. Тирә-яҡ мөхит үҙгәртеүселәре операторҙар өсөн ҡулланылмай.
- Һиҙгерлек ғәҙәттәгесә бирелә; күпселек таратыу’т үҙгәрештәр кәрәкмәй.
- `[pipeline]` буйынса тейешле асҡыстар:
  - `dynamic_prepass`: IVM уҡыу ғына рөхсәт итеү йыйылмаларын сығарыу өсөн тик преспетчер (подлуҫ: дөрөҫ).
  - `access_set_cache_enabled`: кэш алынған рөхсәт йыйылмаһы `(code_hash, entrypoint)`; өҙөү өсөн отладка кәңәштәре (подлубливый: дөрөҫ).
  - `parallel_overlay`X: параллель рәүештә өҫтөнә өҫтәүҙәр төҙөү; коммит детерминистик булып ҡала (ғәҙәти: дөрөҫ).
  - `gpu_key_bucket`: `(key, tx_idx, rw_flag)`-та тотороҡло радикс ҡулланып планлаштырыусы өсөн опциональ асҡыс бикет; детерминистик процессор fallback һәр ваҡыт әүҙем (ғәҙәти: ялған).
  - `cache_size`X: донъя IVM алдынан кэш (decoded ағымдары һаҡлана). Ғәҙәттәгесә: 128. Артыу ҡабатлау өсөн ҡабатлау ваҡытын кәметергә мөмкин.

Доктар синхронлаштырыу чектары
- Сыскалл константалары (доктар/сығанаҡ/vm_syscalls_генерацияланған.мд)
  - Регенерация: `make docs-syscalls`
  - Тик тикшерергә: `bash scripts/check_syscalls_doc.sh`
- Syscall ABI таблицаһы (йәшник/вм/докс/syscalls.мд)
  - Тик тикшерергә: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - Яңыртыу генерацияланған бүлек (һәм код docs таблицаһы): `cargo run -p ivm --bin gen_syscalls_doc -- --write` .
- Хәҙерге АБИ таблицалары (йәшниктәр/вм/доктар/пункт_аби.мд һәм ivm.md)
  - Тикшерергә генә: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - Яңыртыу бүлектәре: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM башлыҡ сәйәсәте һәм АБИ хештары (доктар/сығанаҡ/ivm_header.md)
  - Тикшерергә генә: `cargo run -p ivm --bin gen_header_doc -- --check` һәм `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - Яңыртыу бүлектәре: `cargo run -p ivm --bin gen_header_doc -- --write` һәм `cargo run -p ivm --bin gen_abi_hash_doc -- --write`.

CI
- GitHub Actions эш ағымы `.github/workflows/check-docs.yml` был тикшерелгән һәр push/PR эшләй һәм уңышһыҙлыҡҡа осрай, әгәр генерацияланған docs дрейф тормошҡа ашырыу.
- [Идара итеү плейбук] (governance_playbook.md)
