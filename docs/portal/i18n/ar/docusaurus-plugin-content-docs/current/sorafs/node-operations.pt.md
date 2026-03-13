---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات العقدة
العنوان: Runbook de Operacoes do no
Sidebar_label: Runbook de Operacoes لا يفعل ذلك
الوصف: قم بالتحقق من غرسة `sorafs-node` في Torii.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sorafs/runbooks/sorafs_node_ops.md`. استمر في العمل كآيات متزامنة حتى يتراجع أبو الهول.
:::

## فيساو جيرال

دليل التشغيل هذا هو مشغلي التحقق من صحة المزروعة `sorafs-node` المضمنة في Torii. يتم الاتصال بكل مكان مباشرة من خلال SF-3: ciclos pin/fetch، والاسترداد بعد الاستعادة، والطلب من خلال الحصة، والتخلص من PoR.

## 1. المتطلبات المسبقة

- إمكانية استخدام عامل التخزين في `torii.sorafs.storage`:

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

- يضمن أن العملية Torii ستصل إلى القراءة/الكتابة على `data_dir`.
- تأكد من عدم الإعلان عن السعة المتوقعة عبر `GET /v2/sorafs/capacity/state` عندما يتم الإعلان عن التسجيل.
- عندما يتم تأهيل الصقل، تعرض لوحات المعلومات وحدات تحكم GiB·hour/PoR كبيرة ومساندة للتخلص من الاتجاهات دون الاهتزاز مع وجود قيم لحظية.

### التشغيل الجاف لـ CLI (اختياري)

قبل تصدير نقاط النهاية HTTP، يمكن التحقق من صحة الواجهة الخلفية للتخزين مع حظر CLI.

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
```تقوم الأوامر بطباعة السيرة الذاتية Norito JSON وتخصيص الاختلافات في ملف تعريف القطعة أو الملخص، وتستخدم أدوات لفحص الدخان من CI قبل إجراء الأسلاك Torii.[crates/sorafs_node/tests/cli.rs#L1]

### Ensaio de prova PoR

يمكن للمشغلين الآن إنتاج منتجات مصنوعة من خلال الإدارة المحلية قبل إرسالها إلى Torii. تعيد واجهة سطر الأوامر (CLI) استخدام نفس طريق الإدخال `sorafs-node`، مما يؤدي إلى تنفيذ مواقع تعرض بشكل مباشر أخطاء التحقق من صحة API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

قم بإصدار أمر استئناف JSON (ملخص البيان، معرف المثبت، ملخص الاختبار، عدوى الحب والتحقق الاختياري). Forneca `--manifest-id=<hex>` لضمان أن البيان المخزن يتوافق مع ملخص التحدي، و`--json-out=<path>` عندما يطلب منك حفظ السيرة الذاتية مع المصنوعات الأصلية كدليل سمعي. بما في ذلك `--verdict`، يمكنك استكشاف التدفق الكامل للتحدي → التجريبي → التحقق من عدم الاتصال بالإنترنت قبل الاتصال بـ API HTTP.

حتى يتمكن Torii من استعادة هذه الملفات الصوتية عبر HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

جميع نقاط النهاية عبارة عن خوادم عاملة للتخزين مضمنة، واختبارات دخان مضمنة لـ CLI، وأجهزة استشعار للبوابة متزامنة بشكل دائم.

## 2. Ciclo Pin → جلب1. يتم استخدام حزمة البيان + الحمولة (على سبيل المثال `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. قم بالحسد على البيان باستخدام codeificacao base64:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   يجب أن يكون JSON المطلوب هو `manifest_b64` و`payload_b64`. إجابة ناجحة للإرجاع `manifest_id_hex` وملخص الحمولة.
3. البحث عن الإصلاحات الثابتة:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   قم بفك تشفير النطاق `data_b64` في base64 والتحقق من توافق البايتات الأصلية.

## 3. تمرين التعافي من جديد

1. أصلح الحد الأدنى من البيان كما هو الحال الآن.
2. قم بإعادة تشغيل المعالج Torii (أو ليس بالكامل).
3. قم بمراجعة طلب الجلب. يجب أن تستمر الحمولة في التوفر ويجب أن تتزامن إعادة الخلاصة مع القيمة السابقة للتجديد.
4. افحص `GET /v2/sorafs/storage/state` للتأكد من أن `bytes_used` يعيد البيانات المستمرة بعد إعادة التشغيل.

## 4. اختبار طلب الحصة

1. قم بتخفيض `torii.sorafs.storage.max_capacity_bytes` مؤقتًا لقيمة صغيرة (على سبيل المثال لحجم البيان الفريد).
2. إصلاح البيان؛ مطلوب تحقيق النجاح.
3. قم بإصلاح البيان الثاني للحجم نفسه. يجب أن يطلب Torii من خلال HTTP `400` ورسالة الخطأ `storage capacity exceeded`.
4. قم باستعادة الحد الطبيعي من السعة بعد الانتهاء.

## 5. Sonda de amostragem PoR

1. إصلاح البيان.
2. التماس uma amostra PoR:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```3. تحقق مما إذا كان الرد على `samples` مع الرسالة المطلوبة وما إذا كانت كل إثباتات صحيحة مقابل ما تم تخزينه بشكل واضح.

## 6. قطع غيار السيارات

- يمكن إعادة استخدام اختبارات CI / الدخان من خلال التحقق من الإضافات الموجهة إلى:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  هذا هو `pin_fetch_roundtrip` و`pin_survives_restart` و`pin_quota_rejection` و`por_sampling_returns_verified_proofs`.
- لوحات المعلومات مصاحبة:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` و`torii_sorafs_storage_fetch_inflight`
  - مقاييس النجاح/falha PoR expostos عبر `/v2/sorafs/capacity/state`
  - تجارب التسوية العامة عبر `sorafs_node_deal_publish_total{result=success|failure}`

تضمن متابعة هذه التمارين أن يقوم عامل التخزين بإرسال البيانات، والاستمرار في العمل، وتجديد الحصص التي تم تكوينها، واختبار القدرة على التحديد مسبقًا أو عدم الإعلان عن السعة لمزيد من المساحة.