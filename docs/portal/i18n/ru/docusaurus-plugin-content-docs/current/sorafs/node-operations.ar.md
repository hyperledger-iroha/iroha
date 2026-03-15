---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: операции узла
Название: دليل تشغيل عمليات العقد
Sidebar_label: Открыть
описание: تحقق من نشر `sorafs-node` المضمّن داخل Torii.
---

:::примечание
Был установлен `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Он был убит в фильме "Сфинкс" в фильме "Сфинкс". القديمة.
:::

## نظرة عامة

Он был установлен в соответствии с `sorafs-node` и установлен Torii. يتطابق
В случае с SF-3: закрепление/выборка, а также выборка в режиме реального времени.
Он был опубликован в PoR.

## 1. Настройки

- В разделе `torii.sorafs.storage`:

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

- Зарегистрирован для Torii и установлен на `data_dir`.
- تحقّق من أن العقدة تعلن السعة المتوقعة عبر `GET /v2/sorafs/capacity/state` بعد
  تسجيل تصريح.
- عند تمكين التنعيم, تعرض لوحات المتابعة عدادات GiB·hour/PoR الخام والمُنعّمة
  Он был убит в 2007 году в Нью-Йорке в Нью-Йорке.

### Открытие интерфейса командной строки (открытие)

Для запуска HTTP-запроса и использования интерфейса командной строки CLI
المرفقة.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Создайте файл Norito JSON и создайте дайджест,
Для создания файла CI в файле Torii.【crates/sorafs_node/tests/cli.rs#L1】

### تمرين إثبات PoR

Он сказал, что в 2017 году он был назначен президентом PoR. قبل
رفعها إلى Torii. Вызов CLI для работы с `sorafs-node`, с помощью `sorafs-node`,
Вы можете подключиться к веб-серверу HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Создать файл JSON (дайджест, дайджест, дайджест, дайджест
العينات, ونتيجة الحكم الاختيارية). وفّر `--manifest-id=<hex>` ضمان تطابق
المانيفست المخزّن عمخزّن عندما تريد أرشفة
Это было сделано для того, чтобы покончить с собой. إدراج `--verdict` يتيح لك تمرين
Перейти к основному контенту → Обмен → Открыть веб-сайт HTTP.

Добавьте Torii для подключения к HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Его персонаж - Дэвид Пэнсон, вице-премьер-министр, Лизетт Спенсер. اختبارات
Настройка CLI и настройка интерфейса متزامنة.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Закрепить Pin → Fetch

1. أنشئ حزمة مانيفست + حمولة (على سبيل المثال عبر
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Создать базу данных base64:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   Используйте JSON для `manifest_b64` и `payload_b64`. تعيد الاستجابة
   `manifest_id_hex` и дайджест-файл `manifest_id_hex`.
3. Варианты действий:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   В base64 используется код `data_b64`, который используется для проверки подлинности.

## 3. Нажмите на кнопку «Получить»

1. Дэнни Мэнсон и его сын Кейна в сериале.
2. Установите флажок Torii (в случае необходимости).
3. أعد إرسال طلب الجلب. Новости и новости журнала "Дайджест"
   المُعاد مع القيمة السابقة لإعادة التشغيل.
4. Установите `GET /v2/sorafs/storage/state` рядом с `bytes_used`.
   Сделайте это в ближайшее время.

## 4. اختبار رفض الحصة1. Установите `torii.sorafs.storage.max_capacity_bytes` для получения дополнительной информации (например,
   Миссис Найт).
2. Дэнни Мэнни Уинстон Он был в Нью-Йорке.
3. Дэниел Торонто Миссисипи Уотсон. Отправьте запрос Torii для HTTP `400`
   Установите флажок `storage capacity exceeded`.
4. Сделайте так, чтобы это произошло.

## 5. فحص أخذ عينات PoR

1. Дэнни Мэнси.
2. Обратите внимание на PoR:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Установите флажок для `samples`, чтобы установить его на место. إثبات
   Он выступил с речью в газете "The Wall Street Journal".

## 6. خطافات الأتمتة

- В разделе CI / в разделе "Информационные материалы":

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  Например, `pin_fetch_roundtrip` и `pin_survives_restart` и `pin_quota_rejection`.
  و`por_sampling_returns_verified_proofs`.
- Сообщение в разделе «Линия»:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` و`torii_sorafs_storage_fetch_inflight`
  - عدادات نجاح/فشل PoR المعروضة عبر `/v2/sorafs/capacity/state`
  - Добавлено в программу `sorafs_node_deal_publish_total{result=success|failure}`.

Он был убит в 2007 году в 1980-х годах.
Он был создан в 1980-х годах, когда он был выбран в качестве посредника. إثباتات PoR
Он был убит в 1980-х годах.