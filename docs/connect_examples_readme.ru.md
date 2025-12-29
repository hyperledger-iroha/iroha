---
lang: ru
direction: ltr
source: docs/connect_examples_readme.md
status: complete
translator: manual
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-11-02T04:40:28.804038+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/connect_examples_readme.md (Iroha Connect Examples) -->

## Примеры Iroha Connect (Rust‑приложение / кошелёк)

Запустите два Rust‑примера end‑to‑end против узла Torii.

Требования
- Узел Torii с включённой поддержкой `connect` по адресу `http://127.0.0.1:8080`.
- Установленный инструментариум Rust (stable).
- Python 3.9+ с установленным пакетом `iroha-python` (для CLI‑helper’а ниже).

Примеры
- Пример приложения: `crates/iroha_torii_shared/examples/connect_app.rs`
- Пример кошелька: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI‑helper: `python -m iroha_python.examples.connect_flow`

Порядок запуска
1) Терминал A — приложение (печатает `sid` + токены, подключает WS, отправляет
   `SignRequestTx`):

    cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   Пример вывода:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS connected
    app: sent SignRequestTx
    (waiting for reply)

2) Терминал B — кошелёк (подключается, используя `token_wallet`, и отвечает `SignResultOk`):

    cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   Пример вывода:

    wallet: connected WS
    wallet: SignRequestTx len=3 at seq 1
    wallet: sent SignResultOk

3) Терминал приложения печатает результат:

    app: got SignResultOk algo=ed25519 sig=deadbeef

  Используйте новый helper `connect_norito_decode_envelope_sign_result_alg` (и Swift /
  Kotlin‑обёртки в этой папке), чтобы получить строку алгоритма при декодировании payload’а.

Заметки
- В примерах демонстрационные эфемерные ключи выводятся из `sid`, так что приложение и
  кошелёк автоматически интероперабельны. Не используйте такой подход в проде.
- SDK жёстко обеспечивает AEAD‑привязку (AAD) и использование `seq` как nonce; control‑фреймы
  после подтверждения должны быть зашифрованы.
- Для Swift‑клиентов смотрите `docs/connect_swift_integration.md` /
  `docs/connect_swift_ios.md` и проверяйте с помощью `make swift-ci`, чтобы телеметрия
  dashboards оставалась согласованной с Rust‑примерами, а метаданные Buildkite
  (`ci/xcframework-smoke:<lane>:device_tag`) оставались валидными.
- Использование Python CLI‑helper’а:

    ```bash
    python -m iroha_python.examples.connect_flow \
      --base-url http://127.0.0.1:8080 \
      --sid demo-session \
      --chain-id dev-chain \
      --auth-token admin-token \
      --app-name "Demo App" \
      --frame-output connect-open.hex \
      --frame-json-output connect-open.json \
      --status-json-output connect-status.json
    ```

  CLI выводит типизированную информацию о сессии, формирует snapshot статуса Connect и
  эмитирует Norito‑закодированный frame `ConnectControlOpen`. Флаг `--send-open` отправляет
  payload обратно в Torii; `--frame-output-format binary` позволяет записывать «сырые»
  байты; `--frame-json-output` формирует JSON‑blob, удобный для base64; а
  `--status-json-output` полезен, когда нужен типизированный snapshot для автоматизации.
  Также можно загрузить метаданные приложения из JSON‑файла через
  `--app-metadata-file metadata.json`, содержащий поля `name`, `url` и `icon_hash`
  (см. `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`). Новый
  шаблон можно сгенерировать командой
  `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json`.
  Для запусков только ради телеметрии можно полностью пропустить создание сессии через
  `--status-only` и, при необходимости, выгрузить JSON через
  `--status-json-output status.json`.

