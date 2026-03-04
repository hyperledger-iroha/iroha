---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: операции узла
Название: Runbook de Operacoes do no
Sidebar_label: Runbook de Operacoes не делает
описание: Действителен имплантат `sorafs-node` вместо Torii.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Mantenha ambas, как синхронизированные версии, ели то, что было связано со Сфинксом, оставшимся в прошлом.
:::

## Визао гераль

Этот Runbook предназначен для проверки имплантации `sorafs-node` без установки Torii. В этот момент происходит прямое подключение к SF-3: циклическое соединение/выборка, восстановление после повторного вызова, возврат по квоте и блокировка PoR.

## 1. Предварительные требования

- Навыки работы с хранилищем `torii.sorafs.storage`:

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

- Гарантия, что процесс Torii является доступом/записью на `data_dir`.
- Подтвердите, что не было объявлено о возможности выполнения, через `GET /v1/sorafs/capacity/state`, когда было объявлено о регистрации.
- Когда или сглаживание становится привычным, приборные панели подвергаются жестоким работам в час/час и проверяются, чтобы исчезнуть тенденция к дрожанию или мгновенным значениям.

### Пробный запуск CLI (необязательно)

Прежде чем экспортировать конечные точки HTTP, вы можете подтвердить или использовать серверную часть хранилища с помощью интерфейса командной строки.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Команды импортируют резюме Norito JSON и отбирают расхождения в профилях фрагментов или дайджестах, Tornando-os используются для дымовых проверок CI перед подключением к Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Ensaio de prova PoR

Операторы могут воспроизвести артефакты PoR, испускаемые местными властями до отправки-лос-ао Torii. При повторном использовании CLI или вводе запроса `sorafs-node` порт выдает локальные экспоненциальные ошибки проверки, которые возвращаются API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Команда выдает резюме в формате JSON (дайджест манифеста, идентификатор проверки, дайджест подтверждения, заражение амостра и дополнительная проверка). Forneca `--manifest-id=<hex>` гарантирует, что декларация соответствует соответствующему обзору, а также `--json-out=<path>`, когда вы хотите получить резюме с оригинальными артефактами как аудиторские доказательства. Включите `--verdict`, чтобы разрешить полный переход → проверка → проверка в автономном режиме перед запуском API HTTP.

После того, как Torii был активен, можно восстановить данные об артефактах через HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Конечные точки всех серверов работают с работающим хранилищем, портируют дымовые тесты CLI и постоянно синхронизируемые сигналы шлюза.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Циклический контакт → Извлечь

1. Получите пакет манифеста + полезные данные (например, `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Зависть от манифеста с кодировкой base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   Требуемый JSON-код содержит `manifest_b64` и `payload_b64`. Ответ на успешный возврат `manifest_id_hex` и дайджест полезной нагрузки.
3. Устранение неполадок:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```Декодируйте номер `data_b64` в base64 и проверьте соответствие исходным байтам.

## 3. Упражнение по восстановлению сил после повторного использования

1. Fixe pelo menos um Manifest como acima.
2. Имя процесса Torii (или нет внутреннего).
3. Повторно принесите запрос. Полезная нагрузка должна продолжать распределяться, а переваривание реторнадо должно совпадать с предшествующей доблестью и досрочным завершением.
4. Проверьте `GET /v1/sorafs/storage/state`, чтобы подтвердить, что `bytes_used` отображает системные манифесты, сохраняющиеся после перезагрузки.

## 4. Проверка соблюдения квоты

1. Временное изменение `torii.sorafs.storage.max_capacity_bytes` для вашего достоинства (например, таманьо де ум единый манифест).
2. Исправить манифест; requisicao deve ter sucesso.
3. Зафиксируйте второе нажатие на следующую кнопку. O Torii должен получить запрос с HTTP `400` и сообщить об ошибке `storage capacity exceeded`.
4. Восстановите или установите нормальный предел емкости перед завершением работы.

## 5. Сонда де амстрагем PoR

1. Исправить манифест.
2. Попросите uma amostra PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Убедитесь, что вы получили ответ на вызов `samples` как запрос на заражение и каждый раз доказываете, что вы противодействуете обнаружению явной угрозы.

## 6. Ганчо де автоматакао

- CI/дымовые тесты можно повторно использовать в качестве проверяемых дополнительных направлений:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  que cobrem `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` и `por_sampling_returns_verified_proofs`.
- Панели мониторинга разрабатываются совместно:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` и `torii_sorafs_storage_fetch_inflight`
  - contadores de sucesso/falha PoR expostos через `/v1/sorafs/capacity/state`
  - предварительные публикации об урегулировании через `sorafs_node_deal_publish_total{result=success|failure}`

Вы должны гарантировать, что работник хранилища будет введен в эксплуатацию, сохранится в исходном состоянии, сохранит заданные квоты и убедитесь, что PoR детерминирован перед тем, как не будет объявлено о возможности использования большего количества ресурсов.