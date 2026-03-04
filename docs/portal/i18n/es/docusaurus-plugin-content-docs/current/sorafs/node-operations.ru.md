---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones de nodo
título: Ранбук операций узла
sidebar_label: Ранбук операций узла
descripción: Проверка встроенного деплоя `sorafs-node` внутри Torii.
---

:::nota Канонический источник
Esta página está escrita `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Deje copias sincronizadas de los documentos originales de Sphinx y no los guarde.
:::

## Objeto

Este rango proporciona a los operadores el proceso de implementación de la computadora `sorafs-node`
Torii. Каждый раздел напрямую соответствует entregables SF-3: циклам pin/fetch,
восстановлению после перезапуска, отказу по квоте и PoR-сэмплингу.

## 1. Предварительные условия

- Включите trabajador de almacenamiento en `torii.sorafs.storage`:

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

- Tenga en cuenta que el proceso Torii está descargado en el archivo/descargado con `data_dir`.
- Подтвердите, что узел объявляет ожидаемую ёмкость через
  `GET /v1/sorafs/capacity/state` después de записи декларации.
- При включённом сглаживании дашборды показывают как сырые, так и сглаженные
  счётчики GiB·hour/PoR, чтобы подчёркивать тренды без джиттера рядом с
  мгновенными значениями.

### Пробный запуск CLI (opcional)

Antes de cerrar los componentes HTTP, se pueden implementar mejoras en el backend de control de cordura
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
```Los comandos que aparecen en el archivo Norito JSON y eliminan el perfil de usuario no deseado
digest, esta es una de las opciones del programa de humo CI antes del archivo Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Репетиция PoR-доказательства

Los operadores pueden controlar localmente los artefactos PoR, la gobernanza generalizada,
Antes de descargarlo en Torii. CLI utiliza para poner ingest `sorafs-node`, como
Estos programas locales utilizan las funciones válidas basadas en HTTP API.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Команда выводит JSON-резюме (resumen de manifiesto, id провайдера, resumen de documentación,
количество сэмплов, опциональный результат veredicto). Utilice `--manifest-id=<hex>`,
чтобы убедиться, что сохранённый манифест совпадает с digest вызова, и
`--json-out=<path>`, когда нужно архивировать резюме вместе с исходными артефактами
как доказательство для аудита. Добавление `--verdict` позволяет отрепетировать полный
Haga clic en → доказательство → вердикт в офлайне до обращения к HTTP API.

Después de la entrada Torii, puede buscar estos artefactos en HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Оба эндпоинта обслуживаются встроенным trabajador de almacenamiento, поэтому CLI pruebas de humo y
пробы gateway остаются синхронизированными.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Цикл Pin → Recuperar1. Formar el manifiesto del paquete + carga útil (por ejemplo, con el nombre de
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Отправьте manifest в кодировке base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON se almacena para almacenar `manifest_b64` y `payload_b64`. Успешный ответ
   возвращает `manifest_id_hex` y resumen de carga útil.
3. Póngase en contacto con nosotros:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Conecte el polo `data_b64` en base64 y pruebe, que se adapta a este tipo de baterías.

## 3. Тренировка восстановления после перезапуска

1. Закрепите как minimum один manifest, как описано выше.
2. Перезапустите процесс Torii (или весь узел).
3. Повторно отправьте запрос buscar. Carga útil para una descripción general, un resumen en
   ответе должен совпасть с предшествующим перезапуску.
4. Pruebe `GET /v1/sorafs/storage/state`, чтобы убедиться, что `bytes_used` отражает
   сохранённые se manifiesta после перезагрузки.

## 4. Тест отказа по квоте

1. Временно снизьте `torii.sorafs.storage.max_capacity_bytes` до малого значения
   (например, размера одного manifiesto).
2. Закрепите один manifiesto; запрос должен пройти успешно.
3. Haga clic en el manifiesto de almacenamiento del programa. Torii должен отклонить
   запрос с HTTP `400` y сообщением об ошибке, содержащим `storage capacity exceeded`.
4. Po завершении восстановите обычный предел ёмкости.

## 5. Проба PoR-сэмплинга

1. Закрепите manifiesto.
2. Запросите PoR-выборку:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```3. Pruebe, qué en el exterior está `samples` с запрошенным количеством y что каждое
   доказательство валидируется относительно корня сохранённого манифеста.

## 6. Хуки автоматизации

- Las pruebas de CI/humo pueden utilizarse principalmente en:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  которые покрывают `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`
  y `por_sampling_returns_verified_proofs`.
- Los tableros de mando están seleccionados:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` y `torii_sorafs_storage_fetch_inflight`
  - счётчики успехов/неудач PoR, публикуемые через `/v1/sorafs/capacity/state`
  - попытки публикации liquidación через `sorafs_node_deal_publish_total{result=success|failure}`

Следование этим упражнениям гарантирует, что встроенный способен del trabajador de almacenamiento
ингестировать данные, переживать перезапуски, соблюдать заданные квоты и генерировать
детерминированные PoR-доказательства до того, как узел объявит ёмкость широкой сети.