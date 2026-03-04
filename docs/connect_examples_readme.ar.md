---
lang: ar
direction: rtl
source: docs/connect_examples_readme.md
status: complete
translator: manual
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-11-02T04:40:28.804038+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/connect_examples_readme.md (Iroha Connect Examples) -->

## أمثلة Iroha Connect (تطبيق / محفظة بـ Rust)

يمكنك تشغيل المثالين المكتوبين بـ Rust من البداية إلى النهاية ضد عقدة Torii.

المتطلبات المسبقة
- عقدة Torii مع تفعيل `connect` على `http://127.0.0.1:8080`.
- حزمة Rust (إصدار stable).
- Python 3.9 أو أحدث مع تثبيت حزمة `iroha-python` (لاستخدام أداة CLI الموضحة أدناه).

الأمثلة
- مثال التطبيق: `crates/iroha_torii_shared/examples/connect_app.rs`
- مثال المحفظة: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- أداة CLI في Python: `python -m iroha_python.examples.connect_flow`

ترتيب التشغيل
1) الطرفية A — التطبيق (تطبع `sid` + tokens، وتفتح WebSocket، وترسل `SignRequestTx`):

    cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   مثال على الخرج:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS connected
    app: sent SignRequestTx
    (waiting for reply)

2) الطرفية B — المحفظة (تتصل باستخدام `token_wallet` وترد برسالة `SignResultOk`):

    cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   مثال على الخرج:

    wallet: connected WS
    wallet: SignRequestTx len=3 at seq 1
    wallet: sent SignResultOk

3) طرفية التطبيق تطبع النتيجة:

    app: got SignResultOk algo=ed25519 sig=deadbeef

  استخدم الـ helper الجديد `connect_norito_decode_envelope_sign_result_alg` (وملفات
  Swift/Kotlin المساعدة في هذا المجلد) للحصول على نص اسم الخوارزمية عند فك ترميز
  الـ payload.

ملاحظات
- تشتق الأمثلة مفاتيح ephemeral تجريبية من `sid` بحيث يتكامل التطبيق والمحفظة بشكل
  تلقائي. لا تستخدم هذا النهج في بيئات الإنتاج.
- يفرض الـ SDK ربط AEAD AAD واستخدام `seq` كـ nonce؛ يجب أن تكون إطارات التحكم بعد
  الموافقة مشفّرة.
- لعملاء Swift، راجع `docs/connect_swift_integration.md` /
  `docs/connect_swift_ios.md` وتحقق باستخدام `make swift-ci` حتى تبقى telemetries
  الخاصة باللوحات (dashboards) متوافقة مع أمثلة Rust وتظل بيانات Buildkite الوصفية
  (`ci/xcframework-smoke:<lane>:device_tag`) صحيحة.
- طريقة استخدام أداة CLI في Python:

    ```bash
    python -m iroha_python.examples.connect_flow \
      --base-url http://127.0.0.1:8080 \
      --sid demo-session \
      --chain-id dev-chain \
      --auth-token admin-token \
      --app-name "Demo App" \
      --frame-output connect-open.hex \
      --frame-json-output connect-open.json \
      --status-json-output connect-status.json
    ```

  تقوم أداة CLI بطباعة معلومات الجلسة المهيكلة (typed)، وتوليد لقطة حالة (snapshot)
  لـ Connect، وإخراج الـ frame المشفَّر بتنسيق Norito من نوع `ConnectControlOpen`. يمكنك
  تمرير `--send-open` لإرسال الـ payload مرة أخرى إلى Torii، واستخدام
  `--frame-output-format binary` لكتابة bytes خام، و`--frame-json-output` للحصول على
  blob بصيغة JSON مناسب لـ base64، و`--status-json-output` عندما تحتاج إلى لقطة حالة
  مهيكلة لأغراض الأتمتة. يمكنك أيضًا تحميل بيانات تعريف التطبيق من ملف JSON عبر
  `--app-metadata-file metadata.json` الذي يحتوي الحقول `name` و`url` و`icon_hash`
  (انظر `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`). أنشئ
  قالبًا جديدًا عبر:
  `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json`.
  ولعمليات القياس (telemetry‑only) يمكنك تخطي إنشاء الجلسة بالكامل باستخدام
  `--status-only`، واختيارياً إخراج JSON عبر
  `--status-json-output status.json`.

</div>

