---
lang: ru
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
translator: manual
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-11-09T09:04:55.207331+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/norito_demo_contributor.md (Norito SwiftUI Demo Contributor Guide) -->

# Руководство для контрибьюторов SwiftUI‑демо Norito

Этот документ описывает шаги по ручной настройке для запуска SwiftUI‑демо против локального
узла Torii и тестового (mock) ledger’а. Он дополняет `docs/norito_bridge_release.md`,
фокусируясь на повседневных задачах разработки. За более подробным walkthrough по
интеграции Norito‑bridge’а/Connect‑стека в проекты Xcode см.
`docs/connect_swift_integration.md`.

## Настройка окружения

1. Установите Rust‑toolchain, указанный в `rust-toolchain.toml`.
2. Установите Swift 5.7+ и Xcode Command Line Tools на macOS.
3. (Опционально) установите [SwiftLint](https://github.com/realm/SwiftLint) для
   статического анализа.
4. Выполните `cargo build -p irohad`, чтобы убедиться, что узел собирается на вашей
   машине.
5. Скопируйте `examples/ios/NoritoDemoXcode/Configs/demo.env.example` в `.env` и
   адаптируйте значения под ваше окружение. Приложение читает следующие переменные при
   запуске:
   - `TORII_NODE_URL` — базовый REST‑URL (WebSocket‑URL вычисляются из него).
   - `CONNECT_SESSION_ID` — 32‑байтовый идентификатор сессии (base64/base64url).
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — токены, возвращаемые
     `/v2/connect/session`.
   - `CONNECT_CHAIN_ID` — идентификатор цепочки, объявляемый во время control‑handshake’а.
   - `CONNECT_ROLE` — роль по умолчанию, предвыбранная в UI (`app` или `wallet`).
   - Дополнительные helper’ы для ручных тестов: `CONNECT_PEER_PUB_B64`,
     `CONNECT_SHARED_KEY_B64`, `CONNECT_APPROVE_ACCOUNT_ID`,
     `CONNECT_APPROVE_PRIVATE_KEY_B64`, `CONNECT_APPROVE_SIGNATURE_B64`.

## Запуск Torii + mock‑ledger’а

В репозитории есть служебные скрипты, которые запускают узел Torii с in‑memory‑ledger’ом,
предзаполненным demo‑аккаунтами:

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

Скрипт формирует:

- логи узла Torii в `artifacts/torii.log`;
- метрики ledger’а (формат Prometheus) в `artifacts/metrics.prom`;
- клиентские access‑tokens в `artifacts/torii.jwt`.

`start.sh` держит demo‑peer запущенным до нажатия `Ctrl+C`. Он пишет snapshot состояния
готовности в `artifacts/ios_demo_state.json` (единственный источник правды для остальных
артефактов), копирует активный stdout‑лог Torii, периодически опрашивает `/metrics` до
появления Prometheus‑scrape и сериализует сконфигурированные аккаунты в `torii.jwt`
(включая приватные ключи, если они указаны в конфиге). Скрипт принимает `--artifacts` для
переопределения каталога вывода, `--telemetry-profile` для согласования с кастомными
конфигурациями Torii и `--exit-after-ready` для неинтерактивных CI‑job’ов.

Каждая запись в `SampleAccounts.json` поддерживает следующие поля:

- `name` (string, опционально) — сохраняется как metadata‑поле `alias` аккаунта;
- `public_key` (multihash‑строка, обязательно) — используется в качестве
  подписанта (signatory) аккаунта;
- `private_key` (опционально) — сохраняется в `torii.jwt` для генерации клиентских
  credentials;
- `domain` (опционально) — по умолчанию это domain актива, если поле опущено;
- `asset_id` (string, обязательно) — определение актива, которое будет начислено
  на аккаунт;
- `initial_balance` (string, обязательно) — числовой объём актива, который будет
  заминчен на аккаунт.

## Запуск SwiftUI‑демо

1. Соберите XCFramework согласно `docs/norito_bridge_release.md` и добавьте его в проект
   demo (ожидается, что `NoritoBridge.xcframework` лежит в корне проекта).
2. Откройте проект `NoritoDemoXcode` в Xcode.
3. Выберите схему `NoritoDemo` и целевой iOS‑симулятор или устройство.
4. Убедитесь, что файл `.env` подключён через переменные окружения схемы. Заполните
   `CONNECT_*` значениями, возвращёнными `/v2/connect/session`, чтобы UI была
   предзаполнена при запуске.
5. Проверьте настройки аппаратного ускорения: `App.swift` вызывает
   `DemoAccelerationConfig.load().apply()`, чтобы демо подхватывало либо override
   окружения `NORITO_ACCEL_CONFIG_PATH`, либо встроенный файл
   `acceleration.{json,toml}`/`client.{json,toml}`. Удалите/измените эти входные данные,
   если хотите принудительно использовать CPU‑fallback.
6. Соберите и запустите приложение. На домашнем экране будет запрошен Torii‑URL/token,
   если они не заданы через `.env`.
7. Инициируйте сессию «Connect», чтобы подписаться на обновления аккаунта или подтверждать
   запросы.
8. Выполните перевод IRH и посмотрите вывод логов на экране и в логах Torii.

### Тумблеры аппаратного ускорения (Metal / NEON)

`DemoAccelerationConfig` повторяет конфигурацию узла Rust, чтобы разработчики могли
нагружать Metal/NEON‑ветки без жёстко прошитых порогов. Загрузчик ищет конфиг при запуске
в следующем порядке:

1. `NORITO_ACCEL_CONFIG_PATH` (определена в `.env`/аргументах схемы) — абсолютный путь
   или `~`‑расширённый путь к JSON/TOML‑файлу `iroha_config`;
2. встроенные конфиги `acceleration.{json,toml}` или `client.{json,toml}`;
3. если ни один источник не найден, сохраняются дефолтные настройки
   (`AccelerationSettings()`).

Пример `acceleration.toml`:

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

Если поля оставлены пустыми (`nil`), используются значения по умолчанию. Отрицательные
числа игнорируются; отсутствие секции `[accel]` означает детерминированный CPU‑режим.
При выполнении на симуляторе без поддержки Metal bridge остаётся на скалярной ветке, даже
если конфиг просит Metal.

## Интеграционные тесты

- Интеграционные тесты будут располагаться в `Tests/NoritoDemoTests` (будут добавлены по
  мере появления macOS‑CI).
- Тесты поднимают Torii с помощью вышеописанных скриптов и проверяют подписки WebSocket,
  балансы токенов и потоки переводов через Swift‑пакет.
- Логи тестовых прогонов сохраняются в `artifacts/tests/<timestamp>/` вместе с метриками
  и примерными дампами ledger’а.

## Проверки паритета в CI

- Запускайте `make swift-ci` перед отправкой PR, затрагивающего демо или общие fixtures.
  Target выполняет проверки паритета fixtures, валидирует дашборды и формирует
  текстовые сводки локально. В CI тот же workflow опирается на metadata Buildkite
  (`ci/xcframework-smoke:<lane>:device_tag`), чтобы привязать результаты к правильному
  симулятору или StrongBox‑lane; после изменения pipeline’ов или тегов агентов убедитесь,
  что это metadata по‑прежнему присутствует.
- При падении `make swift-ci` следуйте инструкциям из
  `docs/source/swift_parity_triage.md` и изучите вывод `mobile_ci`, чтобы понять, какой
  lane требует регенерации или дополнительной работы по инциденту.

## Устранение неполадок

- Если демо не может подключиться к Torii, проверьте URL узла и настройки TLS.
- Убедитесь, что JWT‑токен (если используется) действителен и не истёк.
- Просмотрите `artifacts/torii.log` на наличие ошибок на стороне сервера.
- Для проблем с WebSocket изучите окно лога клиента или вывод консоли Xcode.

