---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : opérations de nœud
titre : دليل تشغيل عمليات العقد
sidebar_label : تشغيل العقد
description: تحقق من نشر `sorafs-node` المضمّن داخل Torii.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/runbooks/sorafs_node_ops.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب مجموعة توثيق Sphinx القديمة.
:::

## نظرة عامة

يرشد هذا الدليل المشغلين خلال التحقق من نشر `sorafs-node` المضمّن داخل Torii. يتطابق
Pour créer un lien vers SF-3 : utiliser pin/fetch, et utilisez le lien ci-dessous.
ورفض الحصص، وأخذ عينات PoR.

## 1. المتطلبات المسبقة

- فعّل عامل التخزين في `torii.sorafs.storage` :

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

- تأكّد من أن عملية Torii تملك صلاحيات القراءة/الكتابة على `data_dir`.
- تحقّق من أن العقدة تعلن السعة المتوقعة عبر `GET /v2/sorafs/capacity/state` بعد
  تسجيل تصريح.
- عند تمكين التنعيم، تعرض لوحات المتابعة عدادات GiB·hour/PoR الخام والمُنعّمة
  لإبراز الاتجاهات الخالية من التذبذب جنبًا إلى جنب مع القيم اللحظية.

### تشغيل تجريبي عبر CLI (اختياري)

Le lien HTTP vers la ligne de commande CLI
Fichier.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Utilisez le code Norito JSON pour télécharger le résumé.
Vous avez besoin de ressources pour CI pour Torii.【crates/sorafs_node/tests/cli.rs#L1】

### تمرين إثبات PoRيمكن للمشغلين الآن إعادة تشغيل آرتيفاكتات PoR الصادرة عن الحوكمة محليًا قبل
Il s'agit de Torii. Utilisez la CLI pour trouver la solution `sorafs-node`, comme indiqué.
Les réseaux sociaux sont basés sur HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

يُصدر الأمر ملخص JSON (digest المانيفست، معرّف المزوّد، digest الإثبات، عدد
العينات، ونتيجة الحكم الاختيارية). وفّر `--manifest-id=<hex>` لضمان تطابق
المانيفست المخزّن مع digest التحدي، و`--json-out=<path>` عندما تريد أرشفة
الملخص مع الآرتيفاكتات الأصلية كدليل تدقيق. إدراج `--verdict` يتيح لك تمرين
حلقة التحدي → الإثبات → الحكم كاملةً دون اتصال قبل استدعاء واجهة HTTP.

Utilisez le code Torii pour utiliser HTTP :

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

يتم تقديم كلا نقطتي النهاية بواسطة عامل التخزين المضمّن، لذا تبقى اختبارات
Utilisez la CLI pour obtenir des informations supplémentaires.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. جولة Épingler → Récupérer

1. أنشئ حزمة مانيفست + حمولة (على سبيل المثال عبر
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Utiliser la base64 :

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   Vous pouvez utiliser JSON pour `manifest_b64` et `payload_b64`. تعيد الاستجابة
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

   Vous utilisez base64 pour `data_b64` et vous êtes à la recherche d'un système de base de données.

## 3. تمرين استعادة ما بعد إعادة التشغيل1. ثبّت مانيفستًا واحدًا على الأقل كما في الأعلى.
2. Utilisez le Torii (ou le modèle Torii).
3. أعد إرسال طلب الجلب. يجب أن تبقى الحمولة قابلة للاسترجاع وأن يتطابق digest
   المُعاد مع القيمة السابقة لإعادة التشغيل.
4. Mettre `GET /v2/sorafs/storage/state` en place pour `bytes_used`
   المانيفستات المحفوظة بعد إعادة التشغيل.

## 4. اختبار رفض الحصة

1. اخفض مؤقتًا `torii.sorafs.storage.max_capacity_bytes` إلى قيمة صغيرة (مثل حجم
   مانيفست واحد).
2. ثبّت مانيفستًا واحدًا؛ يجب أن ينجح الطلب.
3. حاول تثبيت مانيفست ثانٍ بحجم مشابه. يجب أن ترفض Torii الطلب مع HTTP `400`
   ورسالة خطأ تحتوي على `storage capacity exceeded`.
4. استعد حد السعة الطبيعي عند الانتهاء.

## 5. فحص أخذ عينات PoR

1. ثبّت مانيفستًا.
2. اطلب عينة PoR :

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Utilisez le lien `samples` pour la prise en charge des informations personnelles et les informations sur les produits.
   يصحّ مقابل جذر المانيفست المخزّن.

## 6. خطافات الأتمتة

- يمكن لاختبارات CI / الدخان إعادة استخدام الفحوصات المستهدفة المضافة في:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  والتي تغطي `pin_fetch_roundtrip` و`pin_survives_restart` و`pin_quota_rejection`
  و`por_sampling_returns_verified_proofs`.
- يجب أن تتابع لوحات المتابعة:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` et `torii_sorafs_storage_fetch_inflight`
  - عدادات نجاح/فشل PoR المعروضة عبر `/v2/sorafs/capacity/state`
  - محاولات نشر التسوية عبر `sorafs_node_deal_publish_total{result=success|failure}`يضمن اتباع هذه التدريبات أن عامل التخزين المضمّن قادر على إدخال البيانات،
والصمود أمام عمليات إعادة التشغيل، واحترام الحصص المضبوطة، وتوليد إثباتات PoR
حتمية قبل أن تعلن العقدة السعة للشبكة الأوسع.