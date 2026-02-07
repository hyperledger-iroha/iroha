---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: операции узла
Название: Ранбук операций узла
Sidebar_label: Ранбук операций узла
описание: Проверка встроенного деплоя `sorafs-node` внутри Torii.
---

:::note Канонический источник
На этой странице отражено `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Держите копии синхронизированными, пока набор документации Sphinx не будет выведен из обращения.
:::

## Обзор

Этот ранбук проводится операторами через проверку встроенного деплоя `sorafs-node` внутри.
Torii. Каждый раздел напрямую соответствует результатам SF-3: циклам pin/fetch,
восстановлению после перезапуска, отказа по квоте и PoR-сэмплингу.

## 1. Предварительные условия

- Включите работник хранилища в `torii.sorafs.storage`:

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
  `GET /v1/sorafs/capacity/state` после регистрации деклараций.
- При включенном поглаживании приборной панели выглядят как сырые, такие и сглаженные.
  счётчики GiB·hour/PoR, чтобы подчёркивать тренды без джиттера рядом с
  мгновенными значениями.

### Пробный запуск CLI (опционально)

Перед открытием HTTP-эндпоинтов можно провести проверку работоспособности серверной части хранения.
с помощью поставляемого CLI.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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
дайджест, что делает их ключи для CI Smoke-проверок перед подключением Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Повторение PoR-доказательств

Операторы теперь могут локально воспроизводить артефакты PoR, выпущенные системы управления,
прежде чем загрузить их в Torii. CLI использует тот же путь приема `sorafs-node`, так
что локальные прогоны показывают те же ошибки валидации, которые возвращаются через HTTP API.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Команда выводит JSON-резюме (дайджест манифеста, id провайдера, дайджест доказательств,
количество сэмплов, опциональный результат вердикта). Укажите `--manifest-id=<hex>`,
чтобы убедиться, что сохранённый манифест соответствует вызову дайджеста, и
`--json-out=<path>`, когда необходимо заархивировать резюме вместе с исходными документами.
как доказательство для аудита. Добавление `--verdict` Позволяет отрепетировать полный
цикл вызова → доказательство → вердикт в оффлайне для обращения к HTTP API.

После запуска Torii можно получить те же артефакты через HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Оба эндпоинта обслуживаются рабочей частью хранилища, поэтому проводятся дымовые тесты CLI и
пробы шлюза синхронизированными.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Цикл Pin → Fetch

1. Сформируйте пакет манифест + полезная нагрузка (например, с помощью
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Отформатируйте манифест в кодировке base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   Запрос JSON должен поддерживать `manifest_b64` и `payload_b64`. Успешный ответ
   возвращает `manifest_id_hex` и переваривает полезную нагрузку.
3. Получите закреплённые данные:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```Декодируйте поле `data_b64` из base64 и проверьте, что оно соответствует исходным байтам.

## 3. Тренировка восстановления после перезапуска

1. Закрепите как минимум один манифест, как указано выше.
2. Перезапустите процесс Torii (или весь узел).
3. Повторно отредактировать запрос выборки. Полезная нагрузка должна по-прежнему извлекаться, переваривать в
   ответе должен совпасть с предшествующим перезапуском.
4. Проверьте `GET /v1/sorafs/storage/state`, чтобы убедиться, что `bytes_used` отражает
   сохранённые манифесты после перезагрузки.

## 4. Тест отказ по квоте

1. Временно снизьте `torii.sorafs.storage.max_capacity_bytes` до балетных значений.
   (например, манифест одного размера).
2. Закрепите один манифест; запрос должен пройти успешно.
3. Попробуйте закрепить второй манифест контурного размера. Torii должен отклонить
   запрос с HTTP `400` и сообщением о Нужен, содержание `storage capacity exceeded`.
4. По завершении восстановления обычного предела ёмкости.

##5. Проба PoR-сэмплинга

1. Закрепите манифест.
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

3. Проверить, что в ответе есть `samples` с запрошенным количеством и что каждый
   Доказательство валидируется относительно сохранённого манифеста.

## 6. Автоматизация Хуки

- CI/дымовые тесты могут повторно использовать целевые проверки, добавленные в:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  которые раскрывают `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`
  и `por_sampling_returns_verified_proofs`.
- Дашборды должны следить:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` и `torii_sorafs_storage_fetch_inflight`
  - счётчики успехов/неудач PoR, публикуемые через `/v1/sorafs/capacity/state`
  - попытка проведения расчетов через `sorafs_node_deal_publish_total{result=success|failure}`

Следование этим упражнениям гарантирует, что встроенный складской работник научился
поглощать данные, переживать перезапуски, соблюдать заданные квоты и сокращать
определенные PoR-доказательства до того, как узел объявит ёмкость внешней сети.