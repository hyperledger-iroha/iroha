---
lang: ru
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-04T17:06:14.405886+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Краткое руководство

В этом практическом руководстве рассматривается детерминированный профиль чанкера SF-1.
подписание манифеста и поток выборки от нескольких поставщиков, лежащие в основе SoraFS
трубопровод хранения. Соедините его с [глубоким погружением в конвейер манифеста] (manifest-pipeline.md)
для примечаний к проектированию и справочных материалов по флагам CLI.

## Предварительные условия

- Набор инструментов Rust (`rustup update`), рабочая область клонирована локально.
- Необязательно: [сгенерированная OpenSSL пара ключей Ed25519] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  для подписания манифестов.
- Необязательно: Node.js ≥ 18, если вы планируете предварительно просмотреть портал Docusaurus.

Установите `export RUST_LOG=info` и экспериментируйте, чтобы отображать полезные сообщения CLI.

## 1. Обновить детерминированные фикстуры

Восстановите канонические векторы фрагментирования SF-1. Команда также выдает подписанный
конверты манифеста, когда предоставляется `--signing-key`; используйте `--allow-unsigned`
только во время локальной разработки.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Выходы:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (если подписан)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Разбейте полезную нагрузку на части и проверьте план

Используйте `sorafs_chunker` для разбиения произвольного файла или архива:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Ключевые поля:

- `profile`/`break_mask` – подтверждает параметры `sorafs.sf1@1.0.0`.
- `chunks[]` – упорядоченные смещения, длины и дайджесты BLAKE3.

Для более крупных приборов запустите регрессию на основе proptest, чтобы обеспечить потоковую передачу и
пакетное разделение остается синхронизированным:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Создайте и подпишите манифест

Оберните план фрагмента, псевдонимы и подписи управления в манифест, используя
`sorafs-manifest-stub`. Команда ниже демонстрирует однофайловую полезную нагрузку; пройти
путь к каталогу для упаковки дерева (CLI просматривает его лексикографически).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Обзор `/tmp/docs.report.json` для:

- `chunking.chunk_digest_sha3_256` – дайджест SHA3 смещений/длин, соответствует
  чанкерные приспособления.
- `manifest.manifest_blake3` – дайджест BLAKE3, подписанный в конверте манифеста.
- `chunk_fetch_specs[]` – приказал получить инструкции для оркестраторов.

Когда будете готовы предоставить настоящие подписи, добавьте `--signing-key` и `--signer`.
аргументы. Команда проверяет каждую подпись Ed25519 перед записью
конверт.

## 4. Имитация поиска от нескольких поставщиков

Используйте интерфейс командной строки выборки разработчика, чтобы воспроизвести план фрагмента для одного или нескольких
провайдеры. Это идеально подходит для дымовых тестов CI и прототипирования оркестратора.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Утверждения:

- `payload_digest_hex` должен соответствовать отчету манифеста.
- `provider_reports[]` отображает количество успешных/неудачных операций для каждого поставщика.
- Ненулевой `chunk_retry_total` указывает на регулировку противодавления.
- Передайте `--max-peers=<n>`, чтобы ограничить количество поставщиков, запланированных для запуска.
  и сосредоточить моделирование CI на основных кандидатах.
- `--retry-budget=<n>` переопределяет счетчик повторов для каждого фрагмента по умолчанию (3), поэтому вы
  может быстрее выявить регрессию оркестратора при внедрении сбоев.

Добавьте `--expect-payload-digest=<hex>` и `--expect-payload-len=<bytes>` для сбоя.
быстро, когда восстановленная полезная нагрузка отклоняется от манифеста.

## 5. Следующие шаги- **Интеграция управления** – передача дайджеста манифеста и
  `manifest_signatures.json` в рабочий процесс совета, чтобы реестр контактов мог
  Рекламируйте наличие.
- **Согласование реестра** – см. [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  перед регистрацией новых профилей. Автоматизация должна предпочитать канонические дескрипторы
  (`namespace.name@semver`) по числовым идентификаторам.
- **Автоматизация CI** — добавьте приведенные выше команды для освобождения конвейеров, чтобы документировать их,
  приспособления и артефакты публикуют детерминированные манифесты вместе с подписанными
  метаданные.