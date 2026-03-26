---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور cli
العنوان: Recetario de CLI de SoraFS
Sidebar_label: Recetario de CLI
الوصف: يتم إعادة توجيهه إلى السطح الموحد `sorafs_cli`.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/developer/cli.md`. احتفظ بنسخ متزامنة.
:::

يوضح السطح الموحد لـ `sorafs_cli` (المتناسب مع الصندوق `sorafs_car` مع الميزة `cli`) كل خطوة ضرورية لإعداد العناصر SoraFS. الولايات المتحدة الأمريكية هذه الوصفة للملح مباشرة إلى التدفقات المجتمعية; ادمج مع خط البيانات وسجلات التشغيل الخاصة بالمنسق لسياق التشغيل.

## حمولات إمباكويتار

الولايات المتحدة الأمريكية `car pack` لإنتاج أرشيفات محددات السيارات ومستويات القطع. يقوم الأمر تلقائيًا باختيار مطلق النار SF-1 الذي يوفر مقبضًا.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- مقبض القطع المحدد مسبقًا: `sorafs.sf1@1.0.0`.
- يتم إعادة إدخالات الدليل في ترتيب معجمي بحيث يتم الحفاظ على المجاميع الاختبارية بين المنصات.
- تتضمن السيرة الذاتية لـ JSON ملخصات الحمولة والبيانات الوصفية للقطعة وCID الذي تم التعرف عليه من خلال السجل والمنسق.

## بيانات البناء

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```- يتم تعيين الخيارات `--pin-*` مباشرة إلى المجالات `PinPolicy` و`sorafs_manifest::ManifestBuilder`.
- استخدم `--chunk-plan` عندما تريد أن يقوم CLI بإعادة حساب ملخص SHA3 قبل الشحن؛ على النقيض من ذلك، يتم إعادة استخدام الملخص المغطى في السيرة الذاتية.
- تم إخراج JSON من الحمولة Norito للاختلافات البسيطة خلال المراجعات.

## يظهر Firmar بدون مفاتيح طويلة الأمد

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- قبول الرموز المميزة المضمنة، ومتغيرات الإدخال أو التدفقات المستندة إلى الأرشيف.
- إضافة بيانات وصفية (`token_source`، `token_hash_hex`، ملخص القطعة) بدون الاستمرار في JWT في الدفعة الأولى `--include-token=true`.
- تعمل بشكل جيد في CI: يتم الدمج مع OIDC من GitHub Actions الذي تم تكوينه `--identity-token-provider=github-actions`.

## يُظهر Enviar Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority soraカタカナ... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- تحقيق فك التشفير Norito للأسماء المستعارة والتحقق من تطابقه مع ملخص البيان السابق لـ POSTear إلى Torii.
- إعادة حساب ملخص SHA3 من الجزء من الخطة لمنع هجمات التدمير.
- تلتقط خلاصات الاستجابة حالة HTTP والرؤوس وحمولات السجل للاستماعات اللاحقة.

## التحقق من محتوى السيارة والأدلة

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- إعادة بناء جدول البيانات ومقارنة ملخصات الحمولة مع السيرة الذاتية للبيان.
- التقاط الحسابات والمعرفات المطلوبة من خلال إرسال أدلة النسخ إلى الإدارة.## إرسال الأدلة عن بعد

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

- قم بإصدار عناصر NDJSON من خلال كل دليل إرسال (قم بإلغاء تنشيط إعادة التشغيل باستخدام `--emit-events=false`).
- تجميع بيانات النجاح/النجاح ورسوم بيانية لوقت الاستجابة والبيانات المسجلة في السيرة الذاتية JSON حتى تتمكن لوحات المعلومات من رسم النتائج دون قراءة السجلات.
- البيع باستخدام رمز مميز من النحاس عندما تفشل البوابة في الإبلاغ عن إثباتات التحقق من PoR المحلية (عبر `--por-root-hex`). اضبط المظلات باستخدام `--max-failures` و`--max-verification-failures` لتنفيذ التمرين.
- سوبورتا بور هوي؛ يقوم PDP وPoTR بإعادة استخدام نفس الشيء عند استخدام SF-13/SF-14.
- `--governance-evidence-dir` يسرد السيرة الذاتية المقدمة، والبيانات التعريفية (الطابع الزمني، وإصدار CLI، وعنوان URL للبوابة، وملخص البيان) ونسخة من البيان في الدليل الموجز لكي تقوم حزم الإدارة بأرشفة دليل إثبات التدفق دون تكرار التنفيذ.

## المراجع الإضافية

- `docs/source/sorafs_cli.md` — توثيق شامل للأعلام.
- `docs/source/sorafs_proof_streaming.md` — مقياس القياس عن بعد ولوحة القيادة Grafana.
- `docs/source/sorafs/manifest_pipeline.md` - التعميق والتقطيع وتكوين البيان وإدارة السيارة.