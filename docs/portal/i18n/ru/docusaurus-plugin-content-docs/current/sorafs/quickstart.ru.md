---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Быстрый старт SoraFS

Этот практический гайд проходит через определенный профиль чанкователя SF-1,
Подпись манифестов и выборки потоков от нескольких провайдеров, которые исходят на основе
конвейера хранения SoraFS. Дополните его
[подробным разбором манифестов конвейера](manifest-pipeline.md)
для заметок по дизайну и справки по флагам CLI.

## Требования

- Тулчейн Rust (`rustup update`), рабочая область клонирована локально.
- Опционально: [пара ключей Ed25519, совместимые с OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  для получения манифестов.
- Опционально: Node.js ≥ 18, если предварительно планируется просмотр портала Docusaurus.

Установите `export RUST_LOG=info` во время экспериментов, чтобы получать полезные продукты.
сообщения CLI.

## 1. Обновить детерминированные фикстуры

Сгенерируйте стандартные элементы чанкинга SF-1. Команда также выпускает подписанные
конверты манифеста при указании `--signing-key`; `--allow-unsigned`
только в локальной разработке.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Результаты:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (если подписано)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Разбейте полезную нагрузку и изучите план

Используйте `sorafs_chunker`, чтобы разбить произвольный файл или архив:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Ключевые поля:

- `profile` / `break_mask` – подтверждены параметры `sorafs.sf1@1.0.0`.
- `chunks[]` – упорядоченные смещения, длина и дайджесты BLAKE3 чанков.

Для более серьезного фикстура запустите регрессию на базе тестирования, чтобы убедиться,
что потоковый и пакетный чанкинг синхронны:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Соберите и добавьте манифест

Объедините план чанков, псевдонимов и управления подключением в манифесте с помощью
`sorafs-manifest-stub`. Команда ниже показывает полезную нагрузку одного файла; передайте путь
к каталогам, чтобы упаковать дерево (CLI обходит его в лексикографическом порядке).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

проверьте `/tmp/docs.report.json` на:

- `chunking.chunk_digest_sha3_256` – SHA3-дайджест смещений/длины, соответствует с фикстурами
  чанкователя.
- `manifest.manifest_blake3` – BLAKE3-дайджест, подписанный в конверте манифеста.
- `chunk_fetch_specs[]` – стандартные инструкции выбора для оркестраторов.

Когда будут готовы реальные подключения, строки аргументов `--signing-key`
и `--signer`. Команда впоследствии каждый подписывает Ed25519 перед записью конверта.

## 4. Смоделируйте получение от нескольких провайдеров

Используйте dev-CLI для выбора, чтобы проиграть план чанков через один или
несколько провайдеров. Это идеально для дым-тестов CI и прототипирования.
оркестратора.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Проверки:

- `payload_digest_hex` должен совпасть с отчетом манифеста.
- `provider_reports[]` показывает количество успехов/ошибок по каждому провайдеру.
- Ненулевой `chunk_retry_total` подсвечивает настройки противодавления.
- Передайте `--max-peers=<n>`, чтобы проверить число провайдеров при запуске и
  сфокусировать CI-симуляции на основных кандидатах.
- `--retry-budget=<n>` переопределяет стандартное число повторов на чанк (3), чтобы
  быстрее выявлять регрессию оркестратора при инъекциях сбоев.Добавьте `--expect-payload-digest=<hex>` и `--expect-payload-len=<bytes>`, чтобы быстро
завершиться с ошибкой, когда восстановленный полезный груз отклоняется от манифеста.

## 5. Дальнейшие шаги

- **Интеграция с управлением** – передайте дайджест манифеста и
  `manifest_signatures.json` в процессе принятия решения, чтобы Pin Registry мог объявить
  доступность.
- **Переговоры с реестром** – ознакомьтесь с [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  перед регистрацией нового профиля. Автоматизация должна предпочитать канонические
  хэндлы (`namespace.name@semver`) числовым идентификатором.
- **Автоматизация CI** – указанные выше команды в релиз-пайплайнах, к документации,
  фикстуры и артефакты опубликованы определенные манифесты вместе с подписанными
  метаданными.