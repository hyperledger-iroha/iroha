---
lang: ru
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-04T10:50:53.604570+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Разделение → Конвейер манифеста

В этом дополнении к краткому руководству отслеживается сквозной конвейер, который превращается в сырой
байт в манифесты Norito, подходящие для реестра контактов SoraFS. Содержание
адаптировано из [`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
обратитесь к этому документу за канонической спецификацией и журналом изменений.

## 1. Детерминированный фрагмент

SoraFS использует профиль SF-1 (`sorafs.sf1@1.0.0`): прокрутку на основе FastCDC.
хеш с минимальным размером фрагмента 64 КБ, целевым размером 256 КБ, максимальным размером 512 КБ и
`0x0000ffff` маска разрыва. Профиль зарегистрирован в
`sorafs_manifest::chunker_registry`.

### Помощники ржавчины

- `sorafs_car::CarBuildPlan::single_file` – генерирует смещения, длины и
  BLAKE3 обрабатывает данные при подготовке метаданных CAR.
- `sorafs_car::ChunkStore` – передает полезные данные в потоковом режиме, сохраняет метаданные фрагментов и
  выводит дерево выборки доказательства возможности восстановления (PoR) размером 64 КБ/4 КБ.
- `sorafs_chunker::chunk_bytes_with_digests` – помощник библиотеки для обоих интерфейсов командной строки.

### Инструменты CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON содержит упорядоченные смещения, длины и дайджесты фрагментов. Настойчиво
планируйте при создании манифестов или спецификаций выборки оркестратора.

### Свидетели преступления

`ChunkStore` предоставляет доступ к `--por-proof=<chunk>:<segment>:<leaf>` и
`--por-sample=<count>`, чтобы аудиторы могли запрашивать детерминированные наборы свидетелей. Пара
эти флаги с `--por-proof-out` или `--por-sample-out` для записи JSON.

## 2. Обернуть манифест

`ManifestBuilder` объединяет метаданные фрагмента с вложениями управления:

- Корневой CID (dag-cbor) и обязательства CAR.
- Доказательства псевдонимов и заявления о возможностях поставщика.
- Подписи совета и дополнительные метаданные (например, идентификаторы сборок).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Важные результаты:

- `payload.manifest` – байты манифеста, закодированные в Norito.
- `payload.report.json` – сводка, читаемая человеком/автоматом, включая
  `chunk_fetch_specs`, `payload_digest_hex`, дайджесты CAR и метаданные псевдонимов.
- `payload.manifest_signatures.json` – Конверт, содержащий манифест BLAKE3.
  дайджест, дайджест SHA3 фрагментного плана и отсортированные подписи Ed25519.

Используйте `--manifest-signatures-in` для проверки конвертов, поставляемых внешними поставщиками.
подписавшим сторонам, прежде чем выписывать их обратно, и `--chunker-profile-id` или
`--chunker-profile=<handle>`, чтобы заблокировать выбор реестра.

## 3. Опубликовать и закрепить

1. **Представление руководству** – предоставьте дайджест манифеста и подпись.
   конверт в совет, чтобы булавку можно было принять. Внешние аудиторы должны
   сохраните дайджест SHA3 плана фрагментов вместе с дайджестом манифеста.
2. **Пин-полезные данные** – загрузите указанный архив CAR (и необязательный индекс CAR).
   в манифесте реестра контактов. Убедитесь, что манифест и CAR разделяют
   тот же корневой CID.
3. **Запись телеметрии**. Сохраняйте отчет JSON, свидетели PoR и любую выборку.
   метрики в артефактах выпуска. Эти записи поступают на информационные панели операторов и
   помогите воспроизвести проблемы без загрузки больших полезных данных.

## 4. Моделирование выборки от нескольких провайдеров

`пробег груза -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`— `#<concurrency>` увеличивает параллелизм между поставщиками (`#4` выше).
- `@<weight>` настраивает смещение планирования; по умолчанию 1.
- `--max-peers=<n>` ограничивает количество поставщиков, запланированных для запуска, когда
  discovery yields more candidates than desired.
- `--expect-payload-digest` и `--expect-payload-len` защищают от бесшумной работы
  коррупция.
- `--provider-advert=name=advert.to` проверяет возможности поставщика перед
  используя их в симуляции.
- `--retry-budget=<n>` переопределяет счетчик повторов для каждого блока (по умолчанию: 3), поэтому CI
  может быстрее выявить регрессию при тестировании сценариев отказа.

`fetch_report.json` отображает агрегированные показатели (`chunk_retry_total`,
`provider_failure_rate` и т. д.), подходящий для утверждений и наблюдаемости CI.

## 5. Обновления и управление реестром

Предлагая новые профили чанкеров:

1. Создайте дескриптор в `sorafs_manifest::chunker_registry_data`.
2. Обновить `docs/source/sorafs/chunker_registry.md` и соответствующие уставы.
3. Восстановите фикстуры (`export_vectors`) и запишите подписанные манифесты.
4. Предоставить отчет о соблюдении устава с подписями руководства.

Автоматизация должна предпочитать канонические дескрипторы (`namespace.name@semver`) и падать