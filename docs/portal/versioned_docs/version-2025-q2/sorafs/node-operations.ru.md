---
lang: ru
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T15:38:30.655980+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: node-operations-ru
slug: /sorafs/node-operations-ru
---

:::обратите внимание на канонический источник
Зеркала `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Обе копии должны быть согласованы между выпусками.
:::

## Обзор

Этот модуль Runbook помогает операторам проверить встроенное развертывание `sorafs-node` внутри Torii. Каждый раздел напрямую связан с результатами SF-3: круговые обходы закрепления/извлечения, перезапуск восстановления, отклонение квоты и выборка PoR.

## 1. Предварительные условия

- Включите работника хранилища в `torii.sorafs.storage`:

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

- Убедитесь, что процесс Torii имеет доступ для чтения/записи к `data_dir`.
- Убедитесь, что узел объявляет ожидаемую мощность через `GET /v1/sorafs/capacity/state` после записи объявления.
- Если сглаживание включено, на информационных панелях отображаются как необработанные, так и сглаженные счетчики ГиБ·час/PoR, чтобы выделить тенденции без дрожания наряду с спотовыми значениями.

### Пробный прогон CLI (необязательно)

Прежде чем предоставлять доступ к конечным точкам HTTP, вы можете проверить работоспособность серверной части хранилища с помощью прилагаемого интерфейса командной строки.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Команды печатают сводки Norito JSON и отклоняют несоответствия профилей фрагментов или дайджестов, что делает их полезными для дымовых проверок CI перед подключением Torii.【crates/sorafs_node/tests/cli.rs#L1】

Как только Torii станет активным, вы сможете получить те же артефакты через HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Обе конечные точки обслуживаются встроенным обработчиком хранилища, поэтому дымовые тесты CLI и зонды шлюза остаются синхронизированными.

## 2. Пин → Получить туда и обратно

1. Создайте пакет манифест + полезная нагрузка (например, с помощью `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Отправьте манифест в кодировке Base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON запроса должен содержать `manifest_b64` и `payload_b64`. Успешный ответ возвращает `manifest_id_hex` и дайджест полезной нагрузки.
3. Получите закрепленные данные:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-декодируйте поле `data_b64` и убедитесь, что оно соответствует исходным байтам.

## 3. Перезапустите упражнение по восстановлению

1. Закрепите хотя бы один манифест, как указано выше.
2. Перезапустите процесс Torii (или весь узел).
3. Повторно отправьте запрос на получение. Полезные данные по-прежнему должны быть доступны для извлечения, а возвращаемый дайджест должен соответствовать значению перед перезапуском.
4. Проверьте `GET /v1/sorafs/storage/state`, чтобы убедиться, что `bytes_used` отражает сохраненные манифесты после перезагрузки.

## 4. Тест отклонения квоты

1. Временно уменьшите `torii.sorafs.storage.max_capacity_bytes` до небольшого значения (например, до размера одного манифеста).
2. Закрепите один манифест; запрос должен быть успешным.
3. Попытайтесь закрепить второй манифест аналогичного размера. Torii должен отклонить запрос с HTTP `400` и сообщением об ошибке, содержащим `storage capacity exceeded`.
4. По завершении восстановите нормальный предел емкости.

## 5. Зонд отбора проб PoR

1. Закрепите манифест.
2. Запросите образец PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Убедитесь, что ответ содержит `samples` с запрошенным счетчиком и что каждое подтверждение проверяется на соответствие сохраненному корню манифеста.

## 6. Хуки автоматизации

- CI/дымовые тесты могут повторно использовать целевые проверки, добавленные в:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```который охватывает `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` и `por_sampling_returns_verified_proofs`.
- Панели мониторинга должны отслеживать:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` и `torii_sorafs_storage_fetch_inflight`
  - Счетчики успехов/неуспехов PoR отображаются через `/v1/sorafs/capacity/state`.
  - Попытки публикации расчетов через `sorafs_node_deal_publish_total{result=success|failure}`.

Выполнение этих упражнений гарантирует, что работник встроенного хранилища сможет принимать данные, выдерживать перезапуски, соблюдать настроенные квоты и генерировать детерминированные доказательства PoR, прежде чем узел объявит емкость более широкой сети.
