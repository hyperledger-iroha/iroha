---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Быстрый старт SoraFS

Этот практический гайд проходит через детерминированный профиль чанкователя SF-1,
подпись манифестов и поток выборки от нескольких провайдеров, которые лежат в основе
конвейера хранения SoraFS. Дополните его
[подробным разбором конвейера манифестов](manifest-pipeline.md)
для заметок по дизайну и справки по флагам CLI.

## Требования

- Тулчейн Rust (`rustup update`), workspace клонирован локально.
- Опционально: [пара ключей Ed25519, совместимая с OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  для подписи манифестов.
- Опционально: Node.js ≥ 18, если планируете предварительно просматривать портал Docusaurus.

Установите `export RUST_LOG=info` во время экспериментов, чтобы получать полезные
сообщения CLI.

## 1. Обновить детерминированные фикстуры

Сгенерируйте канонические векторы чанкинга SF-1. Команда также выпускает подписанные
конверты манифеста при указании `--signing-key`; используйте `--allow-unsigned`
только в локальной разработке.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Результаты:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (если подписано)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Разбейте payload и изучите план

Используйте `sorafs_chunker`, чтобы разбить произвольный файл или архив:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Ключевые поля:

- `profile` / `break_mask` – подтверждает параметры `sorafs.sf1@1.0.0`.
- `chunks[]` – упорядоченные смещения, длины и дайджесты BLAKE3 чанков.

Для более крупных фикстур запустите регрессию на базе proptest, чтобы убедиться,
что потоковый и пакетный чанкинг остаются синхронными:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Соберите и подпишите манифест

Объедините план чанков, алиасы и подписи управления в манифест с помощью
`sorafs-manifest-stub`. Команда ниже показывает payload одного файла; передайте путь
к директории, чтобы упаковать дерево (CLI обходит его в лексикографическом порядке).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Проверьте `/tmp/docs.report.json` на:

- `chunking.chunk_digest_sha3_256` – SHA3-дайджест смещений/длин, совпадает с фикстурами
  чанкователя.
- `manifest.manifest_blake3` – BLAKE3-дайджест, подписанный в конверте манифеста.
- `chunk_fetch_specs[]` – упорядоченные инструкции выборки для оркестраторов.

Когда будете готовы предоставить реальные подписи, добавьте аргументы `--signing-key`
и `--signer`. Команда проверяет каждую подпись Ed25519 перед записью конверта.

## 4. Смоделируйте получение от нескольких провайдеров

Используйте dev-CLI для выборки, чтобы проиграть план чанков через одного или
нескольких провайдеров. Это идеально для smoke-тестов CI и прототипирования
оркестратора.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Проверки:

- `payload_digest_hex` должен совпадать с отчетом манифеста.
- `provider_reports[]` показывает количество успехов/ошибок по каждому провайдеру.
- Ненулевой `chunk_retry_total` подсвечивает настройки back-pressure.
- Передайте `--max-peers=<n>`, чтобы ограничить число провайдеров в запуске и
  сфокусировать CI-симуляции на основных кандидатах.
- `--retry-budget=<n>` переопределяет стандартное число повторов на чанк (3), чтобы
  быстрее выявлять регрессии оркестратора при инъекции сбоев.

Добавьте `--expect-payload-digest=<hex>` и `--expect-payload-len=<bytes>`, чтобы быстро
завершаться с ошибкой, когда восстановленный payload отклоняется от манифеста.

## 5. Дальнейшие шаги

- **Интеграция с управлением** – передайте дайджест манифеста и
  `manifest_signatures.json` в процесс совета, чтобы Pin Registry мог объявить
  доступность.
- **Переговоры с реестром** – ознакомьтесь с [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  перед регистрацией новых профилей. Автоматизация должна предпочитать канонические
  хэндлы (`namespace.name@semver`) числовым ID.
- **Автоматизация CI** – добавьте команды выше в release-пайплайны, чтобы документация,
  фикстуры и артефакты публиковали детерминированные манифесты вместе с подписанными
  метаданными.
