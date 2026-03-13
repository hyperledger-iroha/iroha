---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور cli
العنوان: وصفات CLI لـ SoraFS
Sidebar_label: وصفات CLI
الوصف: شرح موجه للمهام لسطح `sorafs_cli` الموحّد.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/developer/cli.md`. احرص على جميع النسختين متزامنتين إلى أن يتم إيقاف تشغيل مجموعة Sphinx القديمة.
:::

سطح `sorafs_cli` الموحَّد (الموفر من الصندوق `sorafs_car` مع غرض `cli`) يعرض كل خطوة مطلوبة لإعداد آرتيفاكتات SoraFS. استخدم هذا الكتيب للانتقال إلى ألوان العمل الشائعة؛ واقرنه عملياً لأنابيب المانيفست ودلائل المُنسِّق لإنجاز أعمال السياقة.

## التغليفات

استخدم `car pack` أرشيفات CAR وخطط قطع حتمية. يختار الأمر التنفيذي Chunker SF-1 ما لم يتم توفير المقبض.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- المقسم الافتراضي: `sorafs.sf1@1.0.0`.
- تُستعرض مُدخلات الدلائل بترتيب معجمي حتى تستمر في عرض المجاميع الاختبارية عبر المنصات.
- يشتمل على ملخص JSON الملخصات المحمولة والبيانات الوصفية لكل قطعة وCID وجيرابي المعترف به من السجل والمُنسِّق.

## بناء المانيفستات

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- المتاحة خيارات `--pin-*` و`PinPolicy` ضمن `sorafs_manifest::ManifestBuilder`.
- وفّر `--chunk-plan` عندما تريد من CLI إعادة النظر في نظام SHA3 للـ Chunk قبل؛ وإلا فإن استخدام خلاصة الملخص في الملخص.
- اعتبارات مخرجات JSON حمولة Norito لتسهيل عمل diffs خلال المراجعات.

## توقيع المانيفستات دون مفاتيح طويلة الأمد

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```- يقبل رموزًا ضمنية، ومتغيرات البيئة، أو مصادر قائمة على الملفات.
- بيانات منشأ (`token_source`، `token_hash_hex`، ملخص الـ Chunk) دون حفظ JWT خام ما لم يكن `--include-token=true`.
- يعمل بشكل جيد في CI: تم دمجه مع OIDC في GitHub Actions عبر ضبط `--identity-token-provider=github-actions`.

## إرسال المانيفستات إلى Torii

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

- آيج فك ترميز Norito لأدلة الاسم المستعار ويتحقق من تطابقها مع المانيفست قبل POST إلى Torii.
- إعادة ضبط ملخص SHA3 للـ Chunk من بناء حظر الهجمات عدم التطابق.
- تحديد ملخصات الحالة HTTP والرؤوس وملفات التسجيل للدقيقة لاحقاً.

## التحقق من حماية CAR والأدلة

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- إعادة بناء شجرة PoR ويقارن ملخصات الحمولة بملخص المانيفيست.
- يلتقط العدادات والمعرفات المطلوبة عند إرسال النسخ المتماثلة إلى التصفح.

## بث الأدلة الدعائية

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v2/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- يصدر عناصر NDJSON لكل دليل يتم بثه (يمكن إعادة إعادة التدوير عبر `--emit-events=false`).
- يجمّع عدادات النجاح/الفشل وهيستوغرامات الكمون والإخفاقات المأخوذة من عينات ضمن ملخص JSON بحيث تتمكن من متابعة رسم النتائج دون استخراج تسجيل.
- ينهي بخروج غير صفري عندما تُبلغ البوابة إخفاقات أو عندما يرفض الاعتماد المعتمد محليًا من PoR (عبر `--por-root-hex`) الأدلة. ضبط الاتبات عبر `--max-failures` و`--max-verification-failures` لتجارب التدريب.
- يدعم PoR حاليا؛ أعاد PDP وPoTR استخدام نفسه عند وصول SF-13/SF-14.
- يكتب `--governance-evidence-dir` الملخص المُنسّق والبيانات الوصفية (الطابعة الشرعية، إصدار CLI، عنوان URL للبوابة، ملخص المانيفست) ونسخة من المانيفست في الدليل التجريبي بحيث يمكن لحزم الإتقان أرشفة دليل تدفق الأدلة دون إعادة تشغيل التنفيذ.

## مطلوب إضافي

- `docs/source/sorafs_cli.md` — رواية شاملة للأعلام.
- `docs/source/sorafs_proof_streaming.md` — مخطط تليمتري للأدلة وقالب لوحة Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — تعمّق في التقطيع وتركيب المانيفست وعلاج CAR.