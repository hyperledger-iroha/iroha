---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f447417d0021136734e1bf4b7dafa722115c2e690aebdfdefab1e8988df4d094
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Чанкинг SoraFS → Pipeline манифестов

Этот материал дополняет quickstart и описывает полный пайплайн, который превращает сырые
байты в манифесты Norito, пригодные для Pin Registry SoraFS. Текст адаптирован из
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
обращайтесь к этому документу за канонической спецификацией и журналом изменений.

## 1. Детерминированный чанкинг

SoraFS использует профиль SF-1 (`sorafs.sf1@1.0.0`): роллинг-хэш, вдохновленный FastCDC, с
минимальным размером чанка 64 KiB, целевым 256 KiB, максимальным 512 KiB и маской разрыва
`0x0000ffff`. Профиль зарегистрирован в `sorafs_manifest::chunker_registry`.

### Хелперы Rust

- `sorafs_car::CarBuildPlan::single_file` – Выдает смещения, длины и BLAKE3-дайджесты чанков
  при подготовке метаданных CAR.
- `sorafs_car::ChunkStore` – Стримит payloads, сохраняет метаданные чанков и выводит дерево
  выборки Proof-of-Retrievability (PoR) 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Библиотечный helper, лежащий под обеими CLI.

### Инструменты CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON содержит упорядоченные смещения, длины и дайджесты чанков. Сохраняйте план при сборке
манифестов или спецификаций выборки для оркестратора.

### Свидетели PoR

`ChunkStore` предоставляет `--por-proof=<chunk>:<segment>:<leaf>` и `--por-sample=<count>`,
чтобы аудиторы могли запрашивать детерминированные наборы свидетелей. Сочетайте эти флаги с
`--por-proof-out` или `--por-sample-out`, чтобы записать JSON.

## 2. Обернуть манифест

`ManifestBuilder` объединяет метаданные чанков с вложениями governance:

- Корневой CID (dag-cbor) и коммитменты CAR.
- Доказательства alias и claims возможностей провайдеров.
- Подписи совета и опциональные метаданные (например, build IDs).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Важные выходные данные:

- `payload.manifest` – Norito-кодированные байты манифеста.
- `payload.report.json` – Сводка для людей/автоматизации, включая `chunk_fetch_specs`,
  `payload_digest_hex`, дайджесты CAR и метаданные alias.
- `payload.manifest_signatures.json` – Конверт, содержащий BLAKE3-дайджест манифеста,
  SHA3-дайджест плана чанков и отсортированные подписи Ed25519.

Используйте `--manifest-signatures-in`, чтобы проверить конверты от внешних подписантов перед
перезаписью, и `--chunker-profile-id` или `--chunker-profile=<handle>` для фиксации выбора
реестра.

## 3. Публикация и pinning

1. **Отправка в governance** – Передайте дайджест манифеста и конверт подписей совету, чтобы
   pin мог быть принят. Внешним аудиторам следует хранить SHA3-дайджест плана чанков рядом с
   дайджестом манифеста.
2. **Пиннинг payloads** – Загрузите архив CAR (и опциональный индекс CAR), указанный в
   манифесте, в Pin Registry. Убедитесь, что манифест и CAR используют один и тот же корневой CID.
3. **Запись телеметрии** – Сохраните JSON-отчет, свидетелей PoR и любые метрики fetch в
   релизных артефактах. Эти записи питают операторские дашборды и помогают воспроизводить
   проблемы без загрузки больших payloads.

## 4. Симуляция выборки от нескольких провайдеров

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` увеличивает параллелизм на провайдера (`#4` выше).
- `@<weight>` настраивает смещение планирования; по умолчанию 1.
- `--max-peers=<n>` ограничивает число провайдеров, запланированных на запуск, когда
  обнаружение возвращает больше кандидатов, чем нужно.
- `--expect-payload-digest` и `--expect-payload-len` защищают от тихой порчи данных.
- `--provider-advert=name=advert.to` проверяет возможности провайдера перед использованием
  в симуляции.
- `--retry-budget=<n>` переопределяет число повторов на чанк (по умолчанию: 3), чтобы CI
  быстрее выявляла регрессии при тестировании отказов.

`fetch_report.json` выводит агрегированные метрики (`chunk_retry_total`, `provider_failure_rate`,
и т. д.), подходящие для CI-ассертов и наблюдаемости.

## 5. Обновления реестра и governance

При предложении новых профилей chunker:

1. Подготовьте дескриптор в `sorafs_manifest::chunker_registry_data`.
2. Обновите `docs/source/sorafs/chunker_registry.md` и связанные чартеры.
3. Перегенерируйте фикстуры (`export_vectors`) и зафиксируйте подписанные манифесты.
4. Отправьте отчет о соответствии чартеру с подписями governance.

Автоматизации следует предпочитать канонические handles (`namespace.name@semver`) и
возвращаться к числовым ID только при необходимости обратной совместимости.
