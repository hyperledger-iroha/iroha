---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Быстрый старт SoraFS

Это практический метод проверки точности определения фрагмента SF-1,
фирма-манифестос и канал рекуперации с несколькими поставщиками, обеспечивающими питание
трубопровод альмасенации SoraFS. Полная комплектация
[глубокий анализ конвейера манифестов](manifest-pipeline.md)
для заметок о дизайне и ссылок на флаги CLI.

## Предыдущие реквизиты

- Toolchain de Rust (`rustup update`), локальное клонирование рабочей области.
- Необязательно: [par de claves Ed25519, созданный с OpenSSL] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  Para Firmar Manifestos.
- Необязательно: Node.js ≥ 18, если вы планируете предварительно визуализировать портал Docusaurus.

Определите `export RUST_LOG=info`, пока проводятся эксперименты для просмотра сообщений об утилитах CLI.

## 1. Актуализация детерминированных светильников

Восстановите канонические векторы фрагментации SF-1. Эль-командо тоже эмитэ
Sobres de Manifico Firmados, когда это соответствует `--signing-key`; США
`--allow-unsigned` соло в течение местного времени.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Салидас:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (в фирме)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Фрагмент полезной нагрузки и проверка плана

Используйте `sorafs_chunker` для фрагментации архива или произвольного архива:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Кампос клава:

- `profile` / `break_mask` – подтверждение параметров `sorafs.sf1@1.0.0`.
- `chunks[]` – смещает орденадо, долготу и обрабатывает BLAKE3 фрагментов.

Для большего количества светильников необходимо выполнить регрессию, чтобы помочь
Гарантия того, что фрагментация в потоковом режиме и все синхронизированы:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Создайте и создайте манифест

Оберните план кусков, псевдонимы и управляющие фирмы в используемом манифесте.
`sorafs-manifest-stub`. Командир рабочего места должен выполнить полезную нагрузку одиночного архива; паса
рута директории для вставки стрелки (CLI lo recorre en orden lexicográfico).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Ревиза `/tmp/docs.report.json` пункт:

- `chunking.chunk_digest_sha3_256` – дайджест SHA3 смещений/долгот, совпадение с лос
  светильники дель чанкер.
- `manifest.manifest_blake3` – дайджест BLAKE3, зафиксированный в объявлении.
- `chunk_fetch_specs[]` – инструкции по восстановлению орденадас для поисковиков.

Если этот список для реальных фирм, а также аргументы `--signing-key` y
`--signer`. Команда Verifica Cada Firma Ed25519 перед написанием книги.

## 4. Симула рекуперации с несколькими поставщиками

Используйте CLI для извлечения фрагментов для воспроизведения плана фрагментов против одного или большего количества
провожатые. Это идеальный вариант для дымовых тестов CI и прототипов организаторов.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Компробационы:- `payload_digest_hex` может совпадать с информацией о манифестах.
- `provider_reports[]` muestra conteos de éxito/fallo por provedor.
- Un `chunk_retry_total`, отличающийся от серо, регулирует противодавление.
- Шаг `--max-peers=<n>` для ограничения количества программных проверок для выброса
  и сопровождайте симуляции CI в главных кандидатах.
- `--retry-budget=<n>`, чтобы записать полученное сообщение из-за дефекта повторного использования фрагмента (3) для
  обнаружен более быстрый регресс оркестадора в случае падения.

Нажмите `--expect-payload-digest=<hex>` и `--expect-payload-len=<bytes>` для быстрого падения
Когда автомобиль реконструируется, он должен стать манифестом.

## 5. Сигиентес Пасос

- **Integración de gobernanza** – канализация сводки заявлений и
  `manifest_signatures.json` в списке советов для того, чтобы можно было создать реестр контактов
  объявить о диспонибилидаде.
- **Переговоры о регистрации** – обратитесь к [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  перед регистратором новых профилей. Автоматизация должна быть предпочтительной для канонических мастеров
  (`namespace.name@semver`) с цифровыми идентификаторами.
- **Автоматизация CI** – передние команды и конвейеры выпуска для того, что
  la documentación, светильники и артефакты, публично объявленные детерминистами, объединенными
  метаданные фирмы.