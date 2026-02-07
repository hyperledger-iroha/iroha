---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات العقدة
العنوان: Runbook de Operations del nodo
Sidebar_label: دليل عمليات العقدة
الوصف: تم التحقق من صحة الجزء المتكامل من `sorafs-node` في Torii.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/runbooks/sorafs_node_ops.md`. استمر في العمل على الإصدارات المتزامنة حتى تتقاعد مجموعة أبو الهول.
:::

## السيرة الذاتية

دليل التشغيل هذا موجه إلى المشغلين في التحقق من صحة تخطي `sorafs-node` المدمج في Torii. كل قسم يتوافق مباشرة مع العناصر القابلة للربط SF-3: تسجيلات الدبوس/الجلب، والاسترداد من خلال الاستعادة، والاسترداد من خلال الجلد، ومصدر الطاقة.

## 1. المتطلبات الأساسية

- تأهيل عامل التخزين في `torii.sorafs.storage`:

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

- تأكد من أن العملية Torii ستتيح الوصول إلى القراءة/الكتابة إلى `data_dir`.
- تأكد من أن العقدة تعلن عن السعة المنتهية عبر `GET /v1/sorafs/capacity/state` عندما يتم تسجيل إعلان.
- عندما يتم تأهيل الملحق، تعرض لوحات المعلومات قدرًا كبيرًا من وحدات التحكم GiB·hour/PoR كالمساندة لتسليط الضوء على الاتجاهات دون اهتزاز إلى جانب القيم الفورية.

### تنفيذ مباشر لـ CLI (اختياري)

قبل توسيع نقاط نهاية HTTP، يمكنك إجراء التحقق السريع من تخزين الواجهة الخلفية باستخدام CLI المتكامل.

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
```تقوم الأوامر بطباعة ملخصات Norito JSON وإعادة ضبط التناقضات في ملف تعريف الجزء أو الملخص، مما يجعل الأدوات المستخدمة لفحص الدخان في CI قبل توصيل Torii.[crates/sorafs_node/tests/cli.rs#L1]

### Ensayo de pruebas PoR

يمكن للمشغلين الآن إعادة إنتاج المنتجات المصنّعة من خلال إدارة شكل محلي قبل إرسالها إلى Torii. تعيد واجهة سطر الأوامر (CLI) استخدام نفس مسار الإدخال `sorafs-node`، لكي توضح عمليات التنفيذ المحلية بالضبط أخطاء التحقق من الصحة التي تحول API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

يصدر الأمر استئنافًا JSON (ملخص البيان ومعرف المورد وملخص الاختبار وعدد البيانات ونتيجة الحكم الاختياري). النسبة `--manifest-id=<hex>` للتأكد من أن البيان المُخزن يتزامن مع ملخص التحدي، و`--json-out=<path>` عندما تريد حفظ السيرة الذاتية بالمصنوعات الأصلية كأدلة سمعية. بما في ذلك `--verdict`، يمكنك تجربة كل التحديات → الاختبار → التحقق من عدم الاتصال بالإنترنت قبل الاتصال بـ API HTTP.

بمجرد أن يكون Torii نشطًا، يمكنك استرداد نفس المصنوعات عبر HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

يتم استخدام نقاط النهاية المتعددة من قبل عامل التخزين المضمن، كما أن اختبارات الدخان لـ CLI ومسبار البوابة متزامنة بشكل دائم.## 2. دبوس Recorrido → جلب

1. أنشئ حزمة بيانات + حمولة (على سبيل المثال `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. إرسال البيان إلى قاعدة التدوين 64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   يجب أن يحتوي طلب JSON على `manifest_b64` و`payload_b64`. يتم الرد مرة أخرى على `manifest_id_hex` وملخص الحمولة.
3. استرداد البيانات الموضحة:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   قم بفك تشفير النطاق `data_b64` في الأساس 64 والتحقق من تطابق وحدات البايت الأصلية.

## 3. محاكاة عملية الاسترداد

1. قدم بيانًا على الأقل لكيفية وصولك.
2. استكمال العملية Torii (العقدة كاملة).
3. قم بإعادة طلب الجلب. يجب أن تكون الحمولة قابلة للاسترداد ويجب أن تتزامن الحمولة مع القيمة السابقة للتجديد.
4. افحص `GET /v1/sorafs/storage/state` للتأكد من أن `bytes_used` يعكس البيانات المستمرة أثناء الإنشاء.

## 4. تجربة الاسترداد من قبل cuota

1. قم بتقليل `torii.sorafs.storage.max_capacity_bytes` مؤقتًا إلى قيمة صغيرة (على سبيل المثال حجم بيان واحد فقط).
2. تقديم بيان؛ يجب أن يكون الطلب ناجحًا.
3. نية تقديم بيان ثانٍ بحجم مماثل. يجب على Torii إعادة الطلب باستخدام HTTP `400` ورسالة خطأ تتضمن `storage capacity exceeded`.
4. قم باستعادة الحد الأقصى من القدرة الطبيعية حتى النهاية.

## 5. Sonda de muestreo PoR

1. قم بتقديم بيان.
2. طلب ​​عرض تقديمي:```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. تحقق من أن الرد يحتوي على `samples` مع المحتوى المطلوب وأن كل اختبار صحيح مقابل مصدر البيان المخزن.

## 6. عمليات الأتمتة

- يمكن لاختبارات CI / الدخان إعادة استخدام الاختبارات الموجهة الجديدة في:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  هذا هو الحجم `pin_fetch_roundtrip` و`pin_survives_restart` و`pin_quota_rejection` و`por_sampling_returns_verified_proofs`.
- يجب متابعة لوحات المعلومات:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` و`torii_sorafs_storage_fetch_inflight`
  - مقاومات النجاح/سقوط PoR Expuestos عبر `/v1/sorafs/capacity/state`
  - أهداف نشر التسوية عبر `sorafs_node_deal_publish_total{result=success|failure}`

تضمن متابعة هذه التمارين أن يتمكن عامل التخزين من إدخال البيانات، وحفظ الموارد، واستئناف الحصص التي تم تكوينها، وإجراء اختبارات حول مدى تحديد العقدة قبل أن تعلن العقدة عن قدرتها على زيادة اتساعها.