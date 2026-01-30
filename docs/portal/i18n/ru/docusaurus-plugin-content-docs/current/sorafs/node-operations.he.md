---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 97293510578f35ed968f89ba34524ac1e3b8ded1e20b4b416bbe9577947a2274
source_last_modified: "2025-11-14T04:43:21.864850+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Держите обе копии синхронизированными, пока устаревший набор документации Sphinx не будет выведен из обращения.
:::

## Обзор

Этот ранбук проводит операторов через проверку встроенного деплоя `sorafs-node` внутри
Torii. Каждый раздел напрямую соответствует deliverables SF-3: циклам pin/fetch,
восстановлению после перезапуска, отказу по квоте и PoR-сэмплингу.

## 1. Предварительные условия

- Включите storage worker в `torii.sorafs.storage`:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Убедитесь, что процесс Torii имеет доступ на чтение/запись к `data_dir`.
- Подтвердите, что узел объявляет ожидаемую ёмкость через
  `GET /v1/sorafs/capacity/state` после записи декларации.
- При включённом сглаживании дашборды показывают как сырые, так и сглаженные
  счётчики GiB·hour/PoR, чтобы подчёркивать тренды без джиттера рядом с
  мгновенными значениями.

### Пробный запуск CLI (опционально)

Перед открытием HTTP эндпоинтов можно провести sanity-check backend хранения с
помощью поставляемого CLI.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

Команды выводят Norito JSON-резюме и отвергают несовпадения профиля чанков или
digest, что делает их полезными для CI smoke-проверок перед подключением Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Репетиция PoR-доказательства

Операторы теперь могут локально воспроизводить артефакты PoR, выпущенные governance,
прежде чем загружать их в Torii. CLI использует тот же путь ingest `sorafs-node`, так
что локальные прогоны показывают те же ошибки валидации, которые вернул бы HTTP API.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Команда выводит JSON-резюме (digest манифеста, id провайдера, digest доказательства,
количество сэмплов, опциональный результат verdict). Укажите `--manifest-id=<hex>`,
чтобы убедиться, что сохранённый манифест совпадает с digest вызова, и
`--json-out=<path>`, когда нужно архивировать резюме вместе с исходными артефактами
как доказательство для аудита. Добавление `--verdict` позволяет отрепетировать полный
цикл вызов → доказательство → вердикт в офлайне до обращения к HTTP API.

После запуска Torii можно получить те же артефакты через HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Оба эндпоинта обслуживаются встроенным storage worker, поэтому CLI smoke-тесты и
пробы gateway остаются синхронизированными.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Цикл Pin → Fetch

1. Сформируйте пакет manifest + payload (например, с помощью
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Отправьте manifest в кодировке base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON запроса должен содержать `manifest_b64` и `payload_b64`. Успешный ответ
   возвращает `manifest_id_hex` и digest payload.
3. Получите закреплённые данные:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Декодируйте поле `data_b64` из base64 и проверьте, что оно совпадает с исходными байтами.

## 3. Тренировка восстановления после перезапуска

1. Закрепите как минимум один manifest, как описано выше.
2. Перезапустите процесс Torii (или весь узел).
3. Повторно отправьте запрос fetch. Payload должен по-прежнему извлекаться, а digest в
   ответе должен совпасть с предшествующим перезапуску.
4. Проверьте `GET /v1/sorafs/storage/state`, чтобы убедиться, что `bytes_used` отражает
   сохранённые manifests после перезагрузки.

## 4. Тест отказа по квоте

1. Временно снизьте `torii.sorafs.storage.max_capacity_bytes` до малого значения
   (например, размера одного manifest).
2. Закрепите один manifest; запрос должен пройти успешно.
3. Попробуйте закрепить второй manifest сопоставимого размера. Torii должен отклонить
   запрос с HTTP `400` и сообщением об ошибке, содержащим `storage capacity exceeded`.
4. По завершении восстановите обычный предел ёмкости.

## 5. Проба PoR-сэмплинга

1. Закрепите manifest.
2. Запросите PoR-выборку:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Проверьте, что в ответе есть `samples` с запрошенным количеством и что каждое
   доказательство валидируется относительно корня сохранённого манифеста.

## 6. Хуки автоматизации

- CI / smoke-тесты могут повторно использовать целевые проверки, добавленные в:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  которые покрывают `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`
  и `por_sampling_returns_verified_proofs`.
- Дашборды должны отслеживать:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` и `torii_sorafs_storage_fetch_inflight`
  - счётчики успехов/неудач PoR, публикуемые через `/v1/sorafs/capacity/state`
  - попытки публикации settlement через `sorafs_node_deal_publish_total{result=success|failure}`

Следование этим упражнениям гарантирует, что встроенный storage worker способен
ингестировать данные, переживать перезапуски, соблюдать заданные квоты и генерировать
детерминированные PoR-доказательства до того, как узел объявит ёмкость широкой сети.
