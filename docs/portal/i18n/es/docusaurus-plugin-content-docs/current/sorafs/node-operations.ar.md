---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones de nodo
título: دليل تشغيل عمليات العقد
sidebar_label: تشغيل العقد
descripción: تحقق من نشر `sorafs-node` المضمّن داخل Torii.
---

:::nota المصدر المعتمد
Utilice el código `docs/source/sorafs/runbooks/sorafs_node_ops.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب مجموعة توثيق Sphinx القديمة.
:::

## نظرة عامة

Utilice el conector `sorafs-node` para conectar el conector Torii. يتطابق
كل قسم مباشرةً مع مخرجات SF-3: Pin/fetch, y ما بعد إعادة التشغيل.
ورفض الحصص، وأخذ عينات PoR.

## 1. المتطلبات المسبقة

- Para obtener más información, consulte `torii.sorafs.storage`:

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

- Conecte el cable Torii y conecte el cable/controlador `data_dir`.
- تحقّق من أن العقدة تعلن السعة المتوقعة عبر `GET /v2/sorafs/capacity/state` بعد
  تسجيل تصريح.
- عند تمكين التنعيم، تعرض لوحات المتابعة عدادات GiB·hora/PoR الخام yالمُنعّمة
  لإبراز الاتجاهات الخالية من التذبذب جنبًا إلى جنب مع القيم اللحظية.

### تشغيل تجريبي عبر CLI (اختياري)

قبل إتاحة نقاط النهاية HTTP مكنك إجراء فحص سلامة لخلفية التخزين باستخدام CLI
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

تطبع الأوامر ملخصات Norito JSON y ترفض عدم تطابق ملفات تعريف المقاطع أو digest،
Esta aplicación de CI está disponible en Torii.【crates/sorafs_node/tests/cli.rs#L1】

### تمرين إثبات PoRيمكن للمشغلين الآن إعادة تشغيل آرتيفاكتات PoR الصادرة عن الحوكمة محليًا قبل
رفعها إلى Torii. Utilice la CLI para conectar el dispositivo `sorafs-node` a través de `sorafs-node`.
Los archivos adjuntos están conectados a través de HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

يُصدر الأمر ملخص JSON (resumen المانيفست، معرّف المزوّد، resumen الإثبات، عدد
العينات، ونتيجة الحكم الاختيارية). وفّر `--manifest-id=<hex>` لضمان تطابق
المانيفست المخزّن مع digest التحدي، و`--json-out=<path>` عندما تريد أرشفة
الملخص مع الآرتيفاكتات الأصلية كدليل تدقيق. إدراج `--verdict` يتيح لك تمرين
حلقة التحدي → الإثبات → الحكم كاملةً دون اتصال قبل استدعاء y HTTP.

La configuración Torii está basada en HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

يتم تقديم كلا نقطتي النهاية بواسطة عامل التخزين المضمّن، لذا تبقى اختبارات
【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. جولة Pin → Recuperar

1. أنشئ حزمة مانيفست + حمولة (على سبيل المثال عبر
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Conexión base64:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   Este archivo JSON está escrito en `manifest_b64` y `payload_b64`. تعيد الاستجابة
   الناجحة `manifest_id_hex` وdigest الحمولة.
3. اجلب البيانات المثبتة:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Utilice base64 para `data_b64` y conecte sus archivos.

## 3. تمرين استعادة ما بعد إعادة التشغيل1. ثبّت مانيفستًا واحدًا على الأقل كما في الأعلى.
2. أعد تشغيل عملية Torii (أو العقدة كاملةً).
3. أعد إرسال طلب الجلب. يجب أن تبقى الحمولة قابلة للاسترجاع وأن يتطابق resumen
   المُعاد مع القيمة السابقة لإعادة التشغيل.
4. افحص `GET /v2/sorafs/storage/state` للتأكد من أن `bytes_used` يعكس
   المانيفستات المحفوظة بعد إعادة التشغيل.

## 4. اختبار رفض الحصة

1. اخفض مؤقتًا `torii.sorafs.storage.max_capacity_bytes` إلى قيمة صغيرة (مثل حجم
   مانيفست واحد).
2. ثبّت مانيفستًا واحدًا؛ يجب أن ينجح الطلب.
3. حاول تثبيت مانيفست ثانٍ بحجم مشابه. Está conectado a Torii desde HTTP `400`.
   ورسالة خطأ تحتوي على `storage capacity exceeded`.
4. استعد حد السعة الطبيعي عند الانتهاء.

## 5. فحص أخذ عينات PoR

1. ثبّت مانيفستًا.
2. اطلب عينة PoR:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تحقّق من أن الاستجابة تحتوي على `samples` بعدد العينات المطلوبة وأن كل إثبات
   يصحّ مقابل جذر المانيفست المخزّن.

## 6. خطافات الأتمتة

- يمكن لاختبارات CI / الدخان إعادة استخدام الفحوصات المستهدفة المضافة في:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  Otros tipos de `pin_fetch_roundtrip`, `pin_survives_restart` y `pin_quota_rejection`
  Y `por_sampling_returns_verified_proofs`.
- يجب أن تتابع لوحات المتابعة:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` y `torii_sorafs_storage_fetch_inflight`
  - عدادات نجاح/فشل PoR المعروضة عبر `/v2/sorafs/capacity/state`
  - محاولات نشر التسوية عبر `sorafs_node_deal_publish_total{result=success|failure}`يضمن اتباع هذه التدريبات أن عامل التخزين المضمّن قادر على إدخال البيانات،
PoR
حتمية قبل أن تعلن العقدة السعة للشبكة الأوسع.