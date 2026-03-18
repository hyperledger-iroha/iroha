---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: операции узла
Название: Runbook d’exploitation du nœud
Sidebar_label: Runbook d’exploitation du nœud
описание: Проверка установки `sorafs-node` в Torii.
---

:::note Источник канонический
Эта страница отражена `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Gardez les deux version synchronisées jusqu’à ce que l’ensemble Sphinx soit retire.
:::

## ансамбль

Это руководство по Runbook для операторов для проверки развертывания `sorafs-node`, установленного в Torii. Раздел Chaque соответствует направлению aux livrables SF-3: булавка / выборка, повторение после выдачи, сброс квот и échantillonnage PoR.

## 1. Предварительные условия

- Активировать рабочий склад в `torii.sorafs.storage`:

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

- Убедитесь, что процесс Torii предоставит доступ к лекции/письму по `data_dir`.
- Подтвердите, что нужно объявить о возможности присутствия через `GET /v1/sorafs/capacity/state` для зарегистрированной декларации.
- Lorsque le lissage est activé, les приборные панели выставлены напоказ для fois les compteurs GiB·hour/PoR bruts et lissés afin de mettre en évidence des sans ditter à côté des valeurs Instantanées.

### Выполнение белого интерфейса командной строки (опция)

Прежде чем открывать конечные точки HTTP, вы можете проверить серверную часть с помощью CLI Fournie.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Команды ввода резюме Norito JSON и отказ от расхождений в профиле фрагмента или дайджеста, которые позволяют использовать утилиты для дымовых проверок CI перед подключением Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Повтор предварительного PoR

Операторы, которые могут испытывать дискомфорт, используют локальные артефакты PoR для управления перед телеверсией версии Torii. CLI повторно использует шаблон ввода `sorafs-node`, чтобы отсортировать локальные исполнения, выявляя ошибки проверки, которые обрабатываются API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Команда создания резюме в формате JSON (дайджест манифеста, идентификатор фурниссера, дайджест предварительного просмотра, номера окон, варианты вердикта). Fournissez `--manifest-id=<hex>` для гарантии того, что наличие манифеста соответствует обзору вызова, а `--json-out=<path>` позволяет вам архивировать резюме с исходными артефактами, как до аудита. Включите `--verdict`, чтобы повторить вызов цикла → доказательство → вердикт в локальном приложении API HTTP.

Если Torii по прямой линии, вы можете восстановить артефакты мемов через HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Две конечные точки находятся в сервисе, установленном на складе, после того, как дымовые тесты CLI и зонды шлюза остаются выровненными.

## 2. Булавка-букле → Извлечь

1. Создайте пакет манифест + полезные данные (например, через `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Соберите манифест в base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```Требуемый JSON-файл содержит содержимое `manifest_b64` и `payload_b64`. Ответ на повторный вызов `manifest_id_hex` и дайджест полезной нагрузки.
3. Соберите нарезанные кусочки:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Декодер в base64 по имени `data_b64` и проверка того, что он соответствует исходным октетам.

## 3. Упражнение по повторению после исправления ситуации

1. Épinglez au moins un Manifest comme ci-dessus.
2. Восстановить процесс Torii (или завершить процесс).
3. Отправьте запрос на получение. Полезная нагрузка должна быть восстановлена, а полученный дайджест будет соответствовать возвратной стоимости.
4. Проверьте `GET /v1/sorafs/storage/state`, чтобы убедиться, что `bytes_used` отображает манифесты, сохраняющиеся после перезагрузки.

## 4. Проверка отмены квоты

1. Временное отключение `torii.sorafs.storage.max_capacity_bytes` с неверным значением (например, хвост собственного манифеста).
2. Épinglez un Manifest; la requête doit reussir.
3. Tentez d’épingler un второй манифест аналогичного заявления. Torii повторите запрос с HTTP `400` и сообщение об ошибке, содержащееся `storage capacity exceeded`.
4. После проверки восстановите предел нормальной емкости.

## 5. Sondage d’échantillonnage PoR

1. Épinglez un Manifest.
2. Требуйте échantillon PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Проверьте, что ответ содержит `samples` с требуемым номером и что вы можете быть уверены в том, что вы можете противодействовать расизму в наличии.

## 6. Хуки автоматизации

- Тесты CI / дым могут быть повторно использованы в следующих случаях:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  которые сочетаются `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` и `por_sampling_returns_verified_proofs`.
- Приборные панели doivent suivre:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` и `torii_sorafs_storage_fetch_inflight`
  - les compteurs de succès/échec PoR разоблачает через `/v1/sorafs/capacity/state`
  - предварительные публикации об урегулировании через `sorafs_node_deal_publish_total{result=success|failure}`

Эти учения гарантируют, что работник склада сможет погрузить людей, выживет при повторных браках, соблюдает установленные квоты и генерирует превентивные меры PoR, определяющие, что никто не сможет их восстановить.