---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: операции узла
Название: نوڈ آپریشنز رن بک
Sidebar_label: Нажмите здесь
описание: Torii کے اندر ایمبیڈڈ `sorafs-node` ڈیپلائمنٹ کی کریں۔
---

:::примечание
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_node_ops.md` کی عکاسی کرتا ہے۔ جب تک پرانا Sphinx ڈاکیومنٹیشن سیٹ مکمل طور پر منتقل نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

## جائزہ

Если вам нужен Torii, вы можете использовать `sorafs-node`. توثیق میں
رہنمائی کرتی ہے۔ Результаты поиска по SF-3: закрепить/извлечь
راؤنڈ ٹرپس، ری اسٹارٹ ریکوری, کوٹا ریجیکشن، اور PoR سیمپلنگ۔

## 1. پیشگی تقاضے

- `torii.sorafs.storage` Для получения дополнительной информации:

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

- Доступ к Torii для чтения/записи `data_dir`.
- декларация ریکارڈ ہونے کے بعد `GET /v2/sorafs/capacity/state` کے ذریعے تصدیق
  کریں کہ نوڈ متوقع کپیسٹی اعلان کرتا ہے۔
- Сглаживание в режиме RAW или сглаживание GiB·hour/PoR в режиме реального времени.
  Возможность использования без джиттера Возможность выбора точечных значений

### Интерфейс командной строки (на английском языке)

Конечные точки HTTP и встроенный интерфейс командной строки, а также серверная часть
Проверка работоспособности کر سکتے ہیں۔【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Несоответствие профиля фрагмента Norito Сводки JSON.
Torii проводка, проверка дыма CI, проверка дыма, проверка проводки Torii.
ہیں۔【crates/sorafs_node/tests/cli.rs#L1】

### PoR پروف کی مشق

Используйте PoR-артефакты для Torii для создания артефактов PoR.
پہلے لوکل طور پر ری پلے کر سکتے ہیں۔ CLI и `sorafs-node` для приема и повторного использования
Проверьте наличие ошибок проверки и ошибок проверки HTTP API.
وٹاتی ہے۔

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Сводка JSON выдает کرتی ہے (дайджест манифеста, идентификатор поставщика, дайджест подтверждения,
подсчет проб, необязательный результат вердикта)۔ `--manifest-id=<hex>` فراہم کریں تاکہ
В манифесте есть дайджест, который может быть использован в `--json-out=<path>`.
Краткое изложение, артефакты и доказательства аудита, необходимые для проверки
چاہیں۔ `--verdict` позволяет получить доступ к HTTP API и изменить настройки → Открыть
→ вердикт

Torii Для создания артефактов HTTP можно использовать следующие функции:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Доступ к конечным точкам и проверка дымовых тестов CLI.
зонды шлюза ہم آہنگ رہتے ہیں۔【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Pin → Fetch راؤنڈ ٹرپ

1. манифест + полезная нагрузка
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` کے ذریعے)۔
2. Манифест с кодировкой base64 или кодировкой:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   запрос JSON میں `manifest_b64` اور `payload_b64` شامل ہونا چاہیے۔ کامیاب
   ответ `manifest_id_hex` Дайджест полезной нагрузки واپس کرتی ہے۔
3. закрепленный вариант выбора:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   `data_b64` может декодировать base64, а также использовать байты, необходимые для декодирования.

## 3. ری اسٹارٹ ریکوری ڈرل1. Установите флажок манифеста, чтобы узнать, как его использовать.
2. Torii پروسس (یا پورا نوڈ) ری اسٹارٹ کریں۔
3. принести ریکوئسٹ دوبارہ جمع کریں۔ полезная нагрузка
   дайджест ری اسٹارٹ سے پہلے والی قدر کے مطابق ہو۔
4. `GET /v2/sorafs/storage/state` چیک کریں تاکہ تصدیق کہ `bytes_used` ری کے
   بعد устойчивые проявления کو ظاہر کرتا ہے۔

## 4. کوٹا ریجیکشن ٹیسٹ

1. Установите флажок `torii.sorafs.storage.max_capacity_bytes`, который может быть отключен от сети.
   (مثلاً ایک Manifest کے سائز کے برابر)۔
2. Значок манифеста. ریکوئسٹ کامیاب ہونی چاہیے۔
3. Нажмите на значок манифеста, чтобы узнать, как это сделать. Torii کو ریکوئسٹ
   HTTP `400` کے ساتھ کرنی چاہیے اور ایرر میسج میں `storage capacity exceeded`
   شامل ہونا چاہیے۔
4. ختم ہونے پر نارمل کپیسٹی حد بحال کریں۔

## 5. PoR سیمپلنگ پروب

1. Значок манифеста کریں۔
2. Образец PoR:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Выберите ответ `samples` и подсчитайте количество ответов, которые вы хотите получить.
   доказательство اسٹور شدہ манифест корень کے خلاف ویریفائی ہوتا ہے۔

## 6. آٹومیشن ہکس

- CI/дымовые тесты, необходимые для проверки и проверки:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  جو `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`,
  `por_sampling_returns_verified_proofs` کو کور کرتے ہیں۔
- Если вы хотите, чтобы это произошло:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` или `torii_sorafs_storage_fetch_inflight`
  - `/v2/sorafs/capacity/state` کے ذریعے ظاہر کیے گئے PoR کامیابی/ناکامی کاؤنٹرز
  - `sorafs_node_deal_publish_total{result=success|failure}` کے ذریعے попытки публикации урегулирования

В упражнениях можно использовать различные упражнения, которые можно использовать для приема внутрь.
سکے، ری اسٹارٹس برداشت کرے، مقررہ کوٹاز کا احترام کرے، اور کے وسیع نیٹ ورک
Если вы хотите рекламировать или использовать доказательства PoR, вы можете использовать их.