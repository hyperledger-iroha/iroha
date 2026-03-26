---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور cli
العنوان: Receitas de CLI da SoraFS
Sidebar_label: إيصالات CLI
الوصف: تم تركيز الدليل على المهام السطحية الموحدة لـ `sorafs_cli`.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sorafs/developer/cli.md`. Mantenha ambas as copias sincronzadas.
:::

السطح الموحد `sorafs_cli` (الصندوق المخصص `sorafs_car` مع `cli` المجهز) يعرض كل ما هو ضروري لإعداد العناصر من SoraFS. استخدم كتاب الطبخ هذا لتوجيهه إلى مشتركات سير العمل؛ اجمع بين خط أنابيب البيانات ودفاتر التشغيل التي يقوم بها orquestrador للسياق التشغيلي.

## حمولات إمباكوتار

استخدم `car pack` لإنتاج أرشيفات محددات ومخططات CAR. يتم الأمر تلقائيًا باختيار قطعة SF-1، حتى تتمكن من التعامل معها بشكل تلقائي.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- مقبض قطع اللوح: `sorafs.sf1@1.0.0`.
- إدخالات الدليل يتم اختراقها من خلال ترتيب المعجم حتى يتم وضع المجاميع الاختبارية بين المنصات.
- يتضمن ملخص JSON خلاصات الحمولة، والتوصيفات للقطعة، وCID الذي تم التعرف عليه من خلال السجل والملف الخاص به.

## بيانات البناء

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```- يتم تحديد الخيارات `--pin-*` مباشرة للمجالات `PinPolicy` في `sorafs_manifest::ManifestBuilder`.
- Forneca `--chunk-plan` عندما أطلب منك إعادة حساب CLI أو هضم SHA3 قبل إرساله ؛ في حالة عكس ذلك، لا يتم إعادة استخدام الملخص المضمن.
- يتم إنشاء JSON أو الحمولة Norito لتمييز الأشياء البسيطة أثناء المراجعات.

## يُظهر Assinar sem chaves de longa duracao

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- رموز Aceita مضمنة، متنوعة من البيئة أو الخطوط المستندة إلى المحفوظات.
- إضافة إجراءات أولية (`token_source`، `token_hash_hex`، ملخص القطعة) بدون استمرار أو JWT أولي، على الأقل `--include-token=true`.
- Funciona bem em CI: الجمع بين com OIDC بواسطة GitHub Actions definindo `--identity-token-provider=github-actions`.

## يظهر Enviar للفقرة Torii

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

- قم بفك ترميز Norito للأسماء المستعارة والتحقق من أنها تتوافق مع الملخص وتقوم بالبيان قبل الإرسال عبر POST إلى Torii.
- إعادة حساب هضم SHA3 من خلال جزء من الخطة لتجنب هجمات التباعد.
- تلتقط خلاصات الاستجابة حالة HTTP والرؤوس والحمولات للتسجيل للاستماع الخلفي.

## التحقق من معلومات وإثباتات CAR

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- إعادة بناء تحليل PoR ومقارنة ملخصات الحمولة مع ملخص البيان.
- التقاط العدوى والمعرفات المطلوبة وإرسال إثباتات النسخ للإدارة.

## إرسال البراهين عن بعد للقياس

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- قم بإصدار itens NDJSON لكل ناقل إثبات (مزيل أو إعادة تشغيل com `--emit-events=false`).
- قم بتجميع عدوى النجاح/الفشل، والمخططات البيانية لوقت الاستجابة، والفشل المختلط في استئناف JSON حتى تتمكن لوحات المعلومات من رسم النتائج دون السجلات.
- قم بالتحقق من كوديجو ​​دون صفر عند الإبلاغ عن خطأ البوابة أو عند التحقق من PoR المحلي (عبر `--por-root-hex`) من خلال البراهين. اضبط عمليات التحديد باستخدام `--max-failures` و`--max-verification-failures` لتنفيذ المهام.
- دعم بور اليوم؛ يتم إعادة استخدام PDP وPoTR في نفس المغلف عند تشغيل SF-13/SF-14.
- `--governance-evidence-dir` يتم عرض السيرة الذاتية، والتوصيفات (الطابع الزمني، وعكس سطر الأوامر، وعنوان URL للبوابة، وملخص البيان) ونسخة من البيان بدون دليل مطلوب حتى تتمكن حزم الإدارة من تقديم دليل على تدفق الأدلة دون تكرار التنفيذ.

## المراجع الإضافية

- `docs/source/sorafs_cli.md` - توثيق الأعلام الرائعة.
- `docs/source/sorafs_proof_streaming.md` - اختبار القياس عن بعد ونموذج لوحة المعلومات Grafana.
- `docs/source/sorafs/manifest_pipeline.md` - دمج عميق في التقطيع وتركيب البيان وإدارة السيارة.