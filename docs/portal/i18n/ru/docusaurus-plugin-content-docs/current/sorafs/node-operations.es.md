---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: операции узла
название: Runbook de operaciones del nodo
Sidebar_label: Runbook операций узла
описание: Действительно интегрированное устройство `sorafs-node` вместе с Torii.
---

:::примечание Фуэнте каноника
Эта страница отражает `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Многие из синхронизированных версий должны удалить соединение Сфинкса.
:::

## Резюме

Этот runbook предназначен для операторов и проверки соответствия устройства `sorafs-node`, встроенного в Torii. Этот раздел соответствует прямому контакту с Entregables SF-3: записи захвата/выборки, восстановление по перезапуску, повторение по куоте и muestreo PoR.

## 1. Предварительные условия

- Навыки работы альмасенамиенто на `torii.sorafs.storage`:

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

- Убедитесь, что процесс Torii дает доступ к лекциям/писям по `data_dir`.
- Подтвердите, что номер объявления о возможности отправки через `GET /v2/sorafs/capacity/state` соответствует тому, что было зарегистрировано в заявлении.
- Когда отслеживание становится привычным, приборные панели демонстрируются, как и контадоры GiB·hour/PoR, и становятся жестокими, как вспомогательные устройства, для восстановления тенденций без дрожания и мгновенных значений.

### Удаление из CLI (необязательно)

Экспонирование конечных точек HTTP может обеспечить быструю проверку серверной части хранилища с интегрированным CLI.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Команды вводят резюме Norito JSON и исправляют несоответствия в файлах фрагментов или дайджестов, поэтому у них есть утилиты для дымовой проверки CI перед кабелем Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Ensayo de Pruebas PoR

Оперативники теперь могут воспроизводить артефакты PoR, испускаемые губернаторами для местных форм до субирлос в Torii. CLI повторно использует неправильный маршрут приема `sorafs-node`, чтобы локальные выбросы отображали точные ошибки проверки, которые были переданы API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Команда выдает возобновленный JSON (дайджест манифеста, идентификатор проверяющего, дайджест проверки, списки запросов и необязательный результат проверки). Пропорция `--manifest-id=<hex>` для уверенности в том, что все манифесты совпадают с дайджестом желаемого, и `--json-out=<path>`, когда нужно архивировать резюме с оригинальными артефактами в качестве аудиторских доказательств. Включите `--verdict`, чтобы разрешить все желаемое сообщение → проверка → проверка в автономном режиме перед вызовом API HTTP.

Если Torii активен, можно восстановить потерянные артефакты через HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Конечные точки являются сервисами для встроенного рабочего сервера, а также дымовыми тестами CLI и постоянными датчиками шлюза. sincronizados.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Recorrido Pin → Получить

1. Создайте пакет манифеста + полезная нагрузка (например, с `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Отправка манифеста с базой кодификации64:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```JSON запроса должен содержать контент `manifest_b64` и `payload_b64`. Ответ на выход `manifest_id_hex` и дайджест полезной нагрузки.
3. Восстановление потерянных данных:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Декодировка в base64 на кампо `data_b64` и проверка совпадения с оригинальными байтами.

## 3. Симуляция восстановления после повторного использования

1. Fija al menos un manificesto como arriba.
2. Повторите процесс Torii (полный узел).
3. Повторите запрос на получение. Полезная нагрузка должна быть восстановлена, а переваривание должно совпасть с доблестью, предшествующей повторному использованию.
4. Проверьте `GET /v2/sorafs/storage/state`, чтобы подтвердить, что `bytes_used` отражает сохраняющиеся манифесты по указанному адресу.

## 4. Пруэба-де-рекасо-пор-куота

1. Уменьшите временной интервал `torii.sorafs.storage.max_capacity_bytes` для достижения маленькой доблести (например, для одного манифеста).
2. Fija un manificeto; la solicitud debe tener éxito.
3. Intenta fijar un segundo проявляет подобное таманьо. Torii необходимо выполнить запрос с HTTP `400` и сообщить об ошибке, включающей `storage capacity exceeded`.
4. Восстановите нормальный предел емкости после завершения работы.

## 5. Сонда де муэстрео PoR

1. Fija un manificeto.
2. Solicita un muestreo PoR:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Убедитесь, что ответ содержит `samples` с запрошенным сообщением и что каждый раз проверяется, что он противоречит полученному заявлению.

## 6. Автоматизация

- CI/дымовые тесты могут повторно использовать dirigidas añadidas las comprobaciones en:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  que cubren `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` и `por_sampling_returns_verified_proofs`.
- Приборные панели теперь будут работать следующим образом:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` и `torii_sorafs_storage_fetch_inflight`
  - contadores de éxito/fallo de PoR expuestos через `/v2/sorafs/capacity/state`
  - намерения публикации соглашения через `sorafs_node_deal_publish_total{result=success|failure}`

Вы должны гарантировать, что рабочий альмасенации будет установлен, чтобы сохранить данные, сохранить их, сохранить настройки и сгенерировать проверки PoR до того, как будет объявлено, что они могут быть расширены.