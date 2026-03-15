---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور cli
العنوان: كتاب الطبخ SoraFS CLI
Sidebar_label: كتاب الطبخ CLI
الوصف: سطح `sorafs_cli` الموحد عبارة عن إرشادات تركز على المهام.
---

:::ملاحظة مستند ماخذ
:::

سطح `sorafs_cli` الموحد (صندوق `sorafs_car` الموجود في الصندوق `cli` يتميز بميزات رائعة) SoraFS المصنوعات اليدوية المزيد من المعلومات عن فضح الكرتا. يستخدم كتاب الطبخ هذا سير العمل بشكل عام؛ السياق التشغيلي هو عبارة عن خطوط أنابيب واضحة وكتيبات تشغيل منسقة متزامنة.

## حمولات الحزمة

يتم استخدام أرشيفات CAR الحتمية وخطط القطع لـ `car pack`. إذا لم يكن هناك مقبض، يمكنك تشغيل الأمر الخاص بـ SF-1 Chunker اختيار الكرتا.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- مقبض القطع الافتراضي: `sorafs.sf1@1.0.0`.
- يمكن لترتيب مدخلات الدليل المعجمي أن يسير ويأخذ مجاميع اختبارية مختلفة من خلال رمز ثابت ومستقر.
- يحتوي ملخص JSON على ملخصات الحمولة النافعة، بما في ذلك البيانات الوصفية المقطوعة، وبيانات التسجيل/المنسق التي تتضمن جذر CID.

## بناء البيانات

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- خيارات `--pin-*` لعرض حقول `sorafs_manifest::ManifestBuilder` في `PinPolicy` في الخريطة.
- `--chunk-plan` انقر فوق "إرسال CLI" إلى SHA3 Chunk Digest مرة أخرى لحساب الحساب؛ يتضمن الدرس والملخص تضمين إعادة استخدام الملخص.
- إخراج JSON Norito الحمولة النافعة تحتوي على مراجعات ومراجعات أثناء الاختلافات.## تظهر الإشارة بدون مفاتيح طويلة الأمد

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- الرموز المضمنة، متغيرات البيئة أو المصادر المستندة إلى الملفات قبول کرتا ہے۔
- البيانات التعريفية للمصدر (`token_source`، `token_hash_hex`، ملخص القطعة) تتضمن نصوصًا وJWT خامًا لمحفوظات نصوص، لكن `--include-token=true` لا تحتوي على نصوص.
- مزيد من المعلومات حول CI: استخدام GitHub Actions OIDC الذي تم استخدامه في `--identity-token-provider=github-actions`.

## إرسال البيانات إلى Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- البراهين الاسمية لـ Norito فك تشفير الكرتا و Torii التي تتطابق مع ملخص البيان.
- خطة قطع SHA3 هضم مرة أخرى لحساب هجمات عدم التطابق والهجمات غير المتطابقة.
- ملخصات الاستجابة بعد تدقيق حالة HTTP والرؤوس وحمولات التسجيل.

## التحقق من محتويات السيارة والبراهين

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- شجرة PoR تقوم بمقارنة البيانات وملخصات الحمولة النافعة وملخص البيان ومقارنة البيانات.
- يمكن لإثباتات النسخ المتماثل للحوكمة إرسال البطاقة في الوقت المطلوب والتقاط المعرفات لها.

## القياس عن بعد لدليل الدفق

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v2/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- هناك دليل متدفق لعناصر NDJSON تنبعث منها كرتا (`--emit-events=false` يتم إعادة تشغيلها بيند كریں).
- أعداد النجاح/الفشل، والمخططات البيانية لوقت الاستجابة، وعينات من حالات الفشل، والتي تلخص JSON إجمالي عدد مرات الوصول إلى سجلات لوحات المعلومات، وتتخلص من النتائج المتراكمة.
- تقرير فشل بوابة جب أو التحقق من PoR المحلي (`--por-root-hex` ذریعے) ترفض البراهين الخروج غير الصفري. يتم ضبط عتبات البروفة على `--max-failures` و`--max-verification-failures`.
- آج بور کو دعم کرتا ہے؛ PDP وPoTR SF-13/SF-14 هما مظروفان يمكن إعادة استخدامهما.
- تم تقديم `--governance-evidence-dir` ملخصًا وبيانات التعريف (الطابع الزمني وإصدار CLI وعنوان URL للبوابة وملخص البيان) والبيان لنسخة واحدة من دليل الدليل لكل حزم الحوكمة وأدلة تدفق الأدلة التي يتم تشغيلها مرة أخرى من أرشيف السجل.

## مراجع إضافية

- `docs/source/sorafs_cli.md` — أعلام تمام کی جامع دستاویزات۔
- `docs/source/sorafs_proof_streaming.md` - مخطط إثبات القياس عن بعد وقالب لوحة المعلومات Grafana ۔
- `docs/source/sorafs/manifest_pipeline.md` - التقطيع، والتكوين الواضح، والتعامل مع CAR