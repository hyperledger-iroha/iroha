---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور cli
العنوان: Recettes CLI SoraFS
Sidebar_label: قراءات سطر الأوامر
الوصف: Parcours orienté tâches de la surface consolidée `sorafs_cli`.
---

:::ملاحظة المصدر الكنسي
:::

السطح الموحد `sorafs_cli` (أربعة أجزاء من الصندوق `sorafs_car` مع الميزة `cli` النشطة) يعرض كل شريط ضروري لإعداد العناصر SoraFS. استخدم كتاب الطبخ هذا لجميع مسارات سير العمل المباشرة؛ قم بربط خط أنابيب البيان ودفاتر التشغيل الخاصة بالمدير من أجل سياق التشغيل.

## Empaqueter les payloads

استخدم `car pack` لإنتاج أرشيفات محددات CAR وخطط القطع. أمر التحديد التلقائي للمقطع SF-1 عندما يكون المقبض متوفرًا.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- مقبض التقطيع الافتراضي: `sorafs.sf1@1.0.0`.
- يتم ترتيب مدخلات الذخيرة حسب ترتيب معجمي بحيث تبقى المجموع الاختباري في الاسطبلات بين اللوحات.
- تشتمل السيرة الذاتية JSON على ملخصات الحمولة والبيانات الموضحة بواسطة القطعة وعنصر CID الذي يتم عرضه بواسطة المسجل والمنسق.

## إنشاء البيانات

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```- تم تعيين الخيارات `--pin-*` مباشرة مقابل الأبطال `PinPolicy` في `sorafs_manifest::ManifestBuilder`.
- قم بتوفير `--chunk-plan` عندما ترغب في إعادة حساب CLI لـ SHA3 من قطعة القطع السابقة؛ لا داعي لإعادة استخدام الملخص المتكامل مع السيرة الذاتية.
- تعكس عملية JSON الحمولة Norito للاختلافات البسيطة أثناء العرض.

## قم بتوقيع البيانات بدون كلمات مرور طويلة

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- قبول الرموز المميزة المضمنة أو متغيرات البيئة أو المصادر المستندة إلى الملفات.
- إضافة بيانات المصدر (`token_source`، `token_hash_hex`، ملخص القطعة) بدون استمرار JWT Brut sauf si `--include-token=true`.
- الوظيفة جيدة في CI: الجمع مع OIDC من GitHub Actions في `--identity-token-provider=github-actions` المحدد.

## عرض البيانات في Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority ih58... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- قم بتفعيل فك التشفير Norito من الأدلة المستعارة والتحقق من المراسلات مع ملخص البيان المسبق POST vers Torii.
- أعد حساب ملخص SHA3 من الجزء من الخطة لتجنب الهجمات غير المتطابقة.
- تلتقط سيرة الاستجابة حالة HTTP والرؤوس والحمولات المسجلة لتدقيق نهائي.

## التحقق من محتوى CAR والأدلة

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```- إعادة بناء نسبة تمثيل الخشب ومقارنة ملخصات الحمولة مع ملخص البيان.
- التقاط البيانات المحاسبية والمعرفات المطلوبة عند الحصول على أدلة التكرار للحوكمة.

## Diffuser la télémétrie des prophes

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Émet des éléments NDJSON pour chaqueproof Streamé (désactivez le replay avec `--emit-events=false`).
- قم بتجميع النجاحات/التحققات الناجحة والمخططات البيانية لزمن الوصول والنتائج النهائية في السيرة الذاتية JSON حتى تتمكن لوحات المعلومات من تتبع النتائج دون فحص السجلات.
- قم بالخروج باستخدام رمز غير فارغ عند إشارة البوابة إلى الشيك أو إعادة التحقق من لغة PoR (عبر `--por-root-hex`) للإثباتات. اضبط المتابعة باستخدام `--max-failures` و`--max-verification-failures` لعمليات التكرار.
- دعم PoR aujourd'hui؛ يعمل PDP وPoTR على إعادة استخدام لفة SF-13/SF-14 في مكانها.
- `--governance-evidence-dir` قم بكتابة السيرة الذاتية، والبيانات (الطابع الزمني، إصدار CLI، عنوان URL للبوابة، ملخص البيان) ونسخة من البيان في المرجع حتى تتمكن حزم الإدارة من أرشفة تدفق الإثبات دون تجديد التنفيذ.

## المراجع الإضافية- `docs/source/sorafs_cli.md` — توثيق شامل للعلامات.
- `docs/source/sorafs_proof_streaming.md` — مخطط البراهين عن بعد ونموذج لوحة المعلومات Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — ممتد بشكل عميق في التقطيع وتكوين البيان وإدارة السيارة.