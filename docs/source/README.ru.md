---
lang: ru
direction: ltr
source: docs/source/README.md
status: complete
translator: manual
source_hash: 4a6e55a3232ff38c5c2f45b0a8a3d97471a14603bea75dc2034d7c9c4fb3f862
source_last_modified: "2025-11-10T19:43:50.185052+00:00"
translation_last_reviewed: 2025-11-14
---

# Индекс документации Iroha VM и Kotodama

Этот индекс ссылается на основные проектные и справочные документы по IVM,
Kotodama и конвейеру выполнения, ориентированному на IVM. Японскую версию см.
в [`README.ja.md`](./README.ja.md).

- Архитектура IVM и отображение языковых конструкций: `../../ivm.md`
- ABI системных вызовов IVM: `ivm_syscalls.md`
- Сгенерированные константы syscalls: `ivm_syscalls_generated.md` (для обновления
  выполните `make docs-syscalls`)
- Заголовок байткода IVM: `ivm_header.md`
- Грамматика и семантика Kotodama: `kotodama_grammar.md`
- Примеры Kotodama и соответствия syscalls: `kotodama_examples.md`
- Конвейер транзакций (IVM‑first): `../../new_pipeline.md`
- Контрактный API Torii (манифесты): `torii_contracts_api.md`
- JSON‑конверт для запросов (CLI / инструменты): `query_json.md`
- Справочник по модулю Norito streaming: `norito_streaming.md`
- Примеры ABI среды выполнения: `samples/runtime_abi_active.md`,
  `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- ZK‑API приложения (вложения, prover, подсчёт голосов): `zk_app_api.md`
- Runbook для вложений/prover ZK в Torii: `zk/prover_runbook.md`
- Операторское руководство по ZK App API Torii (вложения/prover; crate‑док):
  `../../crates/iroha_torii/docs/zk_app_api.md`
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- Жизненный цикл VK/proof (реестр, проверка, телеметрия): `zk/lifecycle.md`
- Операторские помощники Torii (эндпоинты для наблюдаемости):
  `references/operator_aids.md`
- Быстрый старт для lane по умолчанию в Nexus: `quickstart/default_lane.md`
- Быстрый старт и архитектура супервизора MOCHI: `mochi/index.md`
- Руководства по JavaScript SDK (quickstart, конфигурация, публикация):
  `sdk/js/index.md`
- Метрики/панели CI для Swift SDK: `references/ios_metrics.md`
- Управление (governance): `../../gov.md`
- Промпты для согласования/уточнений: `coordination_llm_prompts.md`
- Дорожная карта: `../../roadmap.md`
- Использование Docker‑образа сборки: `docker_build.md`

Рекомендации по использованию

- Сборка и запуск примеров в `examples/` с использованием внешних утилит
  (`koto_compile`, `ivm_run`):
  - `make examples-run` (и `make examples-inspect`, если доступен `ivm_tool`)
- Необязательные интеграционные тесты (по умолчанию отключены) для примеров и
  проверки заголовков находятся в `integration_tests/tests/`.

Настройка конвейера

- Всё поведение времени выполнения конфигурируется через файлы `iroha_config`.
  Для операторов переменные окружения не используются.
- Предоставлены разумные значения по умолчанию; большинству развёртываний не
  потребуется настройка.
- Важные ключи в `[pipeline]`:
  - `dynamic_prepass`: включает IVM‑prepass в режиме только чтение для
    построения наборов доступа (по умолчанию true).
  - `access_set_cache_enabled`: кэширует производные наборы доступа по
    `(code_hash, entrypoint)`; отключайте для отладки hints (по умолчанию true).
  - `parallel_overlay`: формирует overlays параллельно; коммит остаётся
    детерминированным (по умолчанию true).
  - `gpu_key_bucket`: опциональная разбивка ключей для prepass планировщика по
    стабильному radix на `(key, tx_idx, rw_flag)`; детерминированный CPU‑путь
    всегда активен (по умолчанию false).
  - `cache_size`: ёмкость глобального кэша пред‑декодирования IVM (для
    декодированных потоков). Значение по умолчанию: 128. Увеличение может
    сократить время декодирования при повторном выполнении.

Проверки синхронизации документации

- Константы syscalls (`docs/source/ivm_syscalls_generated.md`)
  - Регенирация: `make docs-syscalls`
  - Только проверка: `bash scripts/check_syscalls_doc.sh`
- Таблица ABI syscalls (`crates/ivm/docs/syscalls.md`)
  - Только проверка:
    `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - Обновление сгенерированного раздела (и таблицы в код‑доках):
    `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Таблицы pointer‑ABI (`crates/ivm/docs/pointer_abi.md` и `ivm.md`)
  - Только проверка:
    `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - Обновление разделов:
    `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- Политика заголовка IVM и ABI‑hash’и (`docs/source/ivm_header.md`)
  - Только проверка:
    `cargo run -p ivm --bin gen_header_doc -- --check` и
    `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - Обновление разделов:
    `cargo run -p ivm --bin gen_header_doc -- --write` и
    `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI

- Workflow GitHub Actions `.github/workflows/check-docs.yml` запускает эти
  проверки при каждом push/PR и завершится ошибкой, если сгенерированная
  документация перестанет совпадать с реализацией.
- [Руководство по управлению](governance_playbook.md)
