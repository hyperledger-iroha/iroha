---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات العقدة
العنوان: Runbook d’exploitation du nœud
Sidebar_label: Runbook d'exploitation du nœud
الوصف: Valider le déploiement embarqué de `sorafs-node` dans Torii.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/runbooks/sorafs_node_ops.md`. تمت مزامنة الإصدارات الثنائية حتى تتقاعد مجموعة Sphinx.
:::

## عرض المجموعة

يرشد دليل التشغيل هذا المشغلين إلى التحقق من النشر `sorafs-node` المحاصر في Torii. قسم Chaque يتوافق مباشرة مع الكائنات الحية SF-3: حلقات الدبوس/الجلب، والتكرار بعد إعادة الزواج، وإلغاء الحصص، والتشذيب PoR.

## 1. المتطلبات الأساسية

- تنشيط عامل التخزين في `torii.sorafs.storage` :

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

- تأكد من أن العملية Torii ستتمكن من الوصول إلى المحاضرة/الكتابة في `data_dir`.
- تأكد من الإعلان عن السعة الحضورية عبر `GET /v2/sorafs/capacity/state` بعد إعلان التسجيل.
- عندما يتم تنشيط عملية النقل، تعرض لوحات المعلومات مرة أخرى أجهزة الكمبيوتر GiB·hour/PoR القاسية والبيانات التي تساعد على قياس الاتجاهات دون اهتزاز بسبب القيم الفورية.

### التنفيذ الأبيض لـ CLI (اختياري)

قبل الكشف عن نقاط النهاية HTTP، يمكنك التحقق من الواجهة الخلفية للتخزين باستخدام واجهة CLI.

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
```أوامر تطبع السيرة الذاتية Norito JSON وترفض الاختلافات في ملف التعريف أو الملخص، مما يوفر أدوات لفحص الدخان CI قبل كابل Torii.[crates/sorafs_node/tests/cli.rs#L1]

### تكرار Preuve PoR

يمكن للمشغلين أن يضطروا إلى إعادة ترتيب العناصر المنبعثة من الإدارة السابقة للتلفزيون إلى Torii. يقوم CLI بإعادة استخدام نفس نظام الإدخال `sorafs-node`، مما يجعل عمليات التنفيذ المحلية تكشف تمامًا أخطاء التحقق من صحة API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

الأمر يتضمن سيرة ذاتية JSON (ملخص البيان، معرف الموفر، ملخص التقديم، اسم القائمة، خيار الحكم). أرسل `--manifest-id=<hex>` لضمان أن البيان المخزون يتوافق مع ملخص التحدي، و`--json-out=<path>` إذا كنت تريد أرشفة السيرة الذاتية مع العناصر الأصلية كإجراء تدقيق. قم بتضمين `--verdict` للسماح بتكرار مجموعة دورة التحدي → الإثبات → الحكم المحلي قبل استدعاء واجهة API HTTP.

مرة واحدة Torii عبر الإنترنت، يمكنك استعادة العناصر المماثلة عبر HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

يتم استخدام نقطتي النهاية من قبل عامل التخزين المغلق، حتى يتم محاذاة اختبارات الدخان CLI ومسبار البوابة.## 2. دبوس بوكل → جلب

1. قم بإنتاج بيان الحزمة + الحمولة (على سبيل المثال عبر `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. قم بإظهار البيان في base64 :

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   يحتوي طلب JSON على `manifest_b64` و`payload_b64`. يتم إعادة الرد `manifest_id_hex` وملخص الحمولة.
3. استرجاع البيانات المحذوفة :

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   قم بفك التشفير باستخدام قاعدة 64 للبطل `data_b64` وتحقق مما إذا كانت تتوافق مع الثمانيات الأصلية.

## 3. تمرين التكرار بعد الزواج

1. قم بإدراجه على الأقل في بيان باسم ci-dessus.
2. أعد تشغيل العملية Torii (أو اكتملت).
3. أرسل طلب الجلب. يجب أن تكون الحمولة قابلة للاسترداد ويجب أن يتوافق الملخص مع القيمة المسبقة للإعادة.
4. افحص `GET /v2/sorafs/storage/state` للتأكد من أن `bytes_used` يعيد البيانات المستمرة بعد إعادة التشغيل.

## 4. اختبار إعادة الحصص

1. قم بالضغط مؤقتًا على `torii.sorafs.storage.max_capacity_bytes` بقيمة ضعيفة (على سبيل المثال حجم البيان نفسه).
2. تقديم بيان ; لا يتطلب الأمر سوى russir.
3. حاول إضافة بيان ثانٍ لتفاصيل مماثلة. Torii قم بإلغاء الطلب باستخدام HTTP `400` ورسالة خطأ تحتوي على `storage capacity exceeded`.
4. قم باستعادة الحد الطبيعي للسعة بعد الاختبار.

## 5. Sondage d’échantillonnage PoR

1. قم بإبلاغ بيان.
2. اطلب échantillon PoR :

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```3. تحقق من أن الرد يحتوي على `samples` مع الرقم المطلوب وكل منها صالح مقابل عنصر بيان المخزون.

## 6. خطافات التشغيل الآلي

- يمكن لاختبارات CI / الدخان إعادة استخدام عناصر التحكم الكهربائية الإضافية من خلال :

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  الذي يغطي `pin_fetch_roundtrip`، `pin_survives_restart`، `pin_quota_rejection` و`por_sampling_returns_verified_proofs`.
- لوحات المعلومات يجب أن تكون SUIVRE :
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` و`torii_sorafs_storage_fetch_inflight`
  - يتم عرض أجهزة قياس النجاح/فحص PoR عبر `/v2/sorafs/capacity/state`
  - les Tentatives de Publishing de التسوية عبر `sorafs_node_deal_publish_total{result=success|failure}`

تضمن هذه التدريبات اللاحقة أن عامل التخزين يمكنه إدخال البيانات، والبقاء على قيد الحياة عند إعادة التشغيل، واحترام الحصص التي تم تكوينها، وإنشاء محددات الأداء قبل عدم الإعلان عن القدرة على بقية الشبكة.