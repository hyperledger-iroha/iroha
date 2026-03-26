---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور cli
العنوان: كتاب الإيصالات CLI SoraFS
Sidebar_label: إيصالات الكتاب CLI
الوصف: طريقة عملية لتعزيز التوحيد `sorafs_cli`.
---

:::note Канонический источник
:::

توحيد التوحيد `sorafs_cli` (يسمح بالصندوق `sorafs_car` مع الميزة المضمنة `cli`) في كل مرة, مطلوب للمنتج الفني SoraFS. استخدم كتاب الطبخ هذا للوصول بسرعة إلى أنواع سير العمل؛ تدرب مع بيان خط الأنابيب ومنسق دفاتر التشغيل لسياق التشغيل.

## تعبئة الحمولات

استخدم `car pack` لتتمكن من تحديد مجموعة أرشيفات وخطط السيارات. يتم اختيار الأمر الآلي لـ Chunker SF-1، إذا لم يتم إغلاق المقبض.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- قطاعة المقبض القياسية: `sorafs.sf1@1.0.0`.
- يتم استخدام الأدلة الإرشادية في الدراسات المعجمية من أجل تحقيق استقرار المجاميع الاختبارية بين المنصات.
- تشتمل السيرة الذاتية لـ JSON على ملخصات الحمولة، والتحويل إلى القطعة، وCID، والجهاز المنسق والمنسق.

## Сborка يظهر

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- الخيار `--pin-*` يتوافق تمامًا مع `PinPolicy` في `sorafs_manifest::ManifestBuilder`.
- قم بتأكيد `--chunk-plan`، إذا كنت ترغب في إرسال CLI إلى SHA3 Digest للقطعة السابقة للتنفيذ؛ ما عليك سوى استخدام الملخص من الملخص.
- JSON-вывод отразает Norito حمولة للفرق المتوافقة عند المراجعة.## قوائم النشر بدون الكلمات الرئيسية

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- استخدم الرموز المميزة المضمنة أو الحفظ المؤقت أو الملفات المؤقتة.
- إضافة مصدر بديل (`token_source`، `token_hash_hex`، قطعة الملخص) بدون تخزين JWT الخام، إذا لم يتم تخزين `--include-token=true`.
- سهل لـ CI: الالتزام بـ GitHub Actions OIDC، التثبيت `--identity-token-provider=github-actions`.

## يظهر Отправка في Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- استخدم بروفات الاسم المستعار لفك ترميز Norito وتحقق من بيان ملخص الرسالة إلى POST في Torii.
- قم بتنزيل قطعة هضم SHA3 من الخطة لمنع الهجمات غير الضرورية.
- استأنف الرد على إصلاح حالة HTTP والرؤوس والحمولات للتدقيق التالي.

## التحقق من السيارة والبراهين

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- قم بإعادة صياغة مصدر PoR واضبط ملخصات الحمولة الصافية مع بيان السيرة الذاتية.
- تحسين المجموعات والمعرفات المطلوبة عند إجراء النسخ المتماثل للأدلة في الإدارة.

## بروفات القياس عن بعد

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- إنشاء عناصر NDJSON لكل إثبات مسبق (باستثناء إعادة التشغيل من خلال `--emit-events=false`).
- قم بتجميع مجموعة من النجاحات/الأوسيبوك والسجلات الكامنة والخيارات المختارة في ملخص JSON حتى تتمكن لوحات المعلومات من إنشاء رسومات بدون تشويش الشعار.
- أكمل العمل باستخدام كود غير مكتمل، عندما تقوم البوابة بإرسال بيانات أو محقق محلي لـ PoR (من خلال `--por-root-hex`) لإلغاء استنساخ البراهين. قم بإنشاء الطرق عبر `--max-failures` و`--max-verification-failures` للتكرار.
- يتم دعمها حاليًا بواسطة PoR؛ يستخدم PDP وPoTR هذا المغلف بعد إصدار SF-13/SF-14.
- `--governance-evidence-dir` يقوم بكتابة السيرة الذاتية، والتحويلات (الطابع الزمني، إصدار CLI، بوابة URL، ملخص البيان) ونسخ البيان في الموقع يمكن للمدير أرشفة حزم الحوكمة لتوثيق تيار الإثبات بدون إغلاق لاحق.

## Дополнительные ссылки

- `docs/source/sorafs_cli.md` - توثيق المستندات المكتملة.
- `docs/source/sorafs_proof_streaming.md` - بروفات مخطط القياس عن بعد ولوحة التحكم Grafana.
- `docs/source/sorafs/manifest_pipeline.md` - تقطيع قطع الغيار وبيانات المعدات ومعدات السيارة.