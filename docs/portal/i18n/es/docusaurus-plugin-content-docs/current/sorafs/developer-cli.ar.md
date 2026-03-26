---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-cli
título: كتيب وصفات CLI لـ SoraFS
sidebar_label: CLI de configuración
descripción: شرح موجّه للمهام لسطح `sorafs_cli` الموحّد.
---

:::nota المصدر المعتمد
Utilice el botón `docs/source/sorafs/developer/cli.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

سطح `sorafs_cli` الموحّد (الموفر من crate `sorafs_car` مع تمكين ميزة `cli`) يعرض كل خطوة مطلوبة لإعداد آرتيفاكتات SoraFS. استخدم هذا الكتيب للانتقال مباشرةً إلى مسارات العمل الشائعة؛ واقرنه بخط أنابيب المانيفست ودلائل تشغيل المُنسِّق للحصول على السياق التشغيلي.

## تغليف الحمولات

استخدم `car pack` لإنتاج أرشيفات CAR وخطط حتمية. يختار الأمر تلقائيًا chunker SF-1 ما لم يتم توفير مقبض.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Troquelador principal: `sorafs.sf1@1.0.0`.
- تُستعرض مُدخلات الدلائل بترتيب معجمي حتى تبقى checksums ثابتة عبر المنصات.
- يتضمن ملخص JSON digests الحمولة وبيانات وصفية لكل chunk و CID الجذري المعترف به من السجل والمُنسِّق.

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

- Las conexiones `--pin-*` están conectadas entre sí `PinPolicy` y `sorafs_manifest::ManifestBuilder`.
- `--chunk-plan` utiliza la CLI para digerir SHA3 y obtener un fragmento del código fuente. وإلا فإنه يعيد استخدام resumen المضمّن في الملخص.
- Utilice el archivo JSON Norito para obtener diferencias entre los archivos.

## توقيع المانيفستات دون مفاتيح طويلة الأمد

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```- يقبل رموزًا مضمنة, ومتغيرات بيئة, أو مصادر قائمة على ملفات.
- يضيف بيانات منشأ (`token_source`, `token_hash_hex`, digest الـ chunk) دون حفظ JWT الخام ما لم يكن `--include-token=true`.
- Establece CI: utiliza OIDC y GitHub Actions en `--identity-token-provider=github-actions`.

## إرسال المانيفستات إلى Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <katakana-i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- يجري فك ترميز Norito لأدلة alias ويتحقق من تطابقها مع digest المانيفست قبل POST إلى Torii.
- يعيد احتساب digest SHA3 للـ chunk من الخطة لمنع هجمات عدم التطابق.
- تلتقط ملخصات الاستجابة حالة HTTP y وحمولات السجل للتدقيق لاحقًا.

## التحقق من محتوى CAR والأدلة

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- يعيد بناء شجرة PoR ويقارن resume الحمولة بملخص المانيفست.
- يلتقط العدادات والمعرفات المطلوبة عند إرسال أدلة النسخ المتماثل إلى الحوكمة.

## بث تليمترية الأدلة

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- يُصدر عناصر NDJSON لكل دليل يتم بثه (يمكن تعطيل الإعادة عبر `--emit-events=false`).
- يجمّع عدادات النجاح/الفشل وهيستوغرامات الكمون والإخفاقات المأخوذة عينات ضمن ملخص JSON بحيث تستطيع لوحات المتابعة رسم النتائج دون تقشير السجلات.
- ينهي بخروج غير صفري عندما تُبلغ البوابة عن إخفاقات أو عندما ترفض عملية التحقق المحلية من PoR (عبر `--por-root-hex`) الأدلة. Utilice `--max-failures` e `--max-verification-failures` para su uso.
- يدعم PoR حاليًا؛ يعيد PDP y PoTR استخدام الغلاف نفسه عند وصول SF-13/SF-14.
- يكتب `--governance-evidence-dir` الملخص المُنسّق والبيانات الوصفية (الطابع الزمني، إصدار CLI، عنوان URL للبوابة, digest المانيفست) ونسخة من المانيفست في الدليل المحدد بحيث يمكن لحزم الحوكمة أرشفة دليل تدفق الأدلة دون إعادة تشغيل التنفيذ.

## مراجع إضافية

- `docs/source/sorafs_cli.md` — توثيق شامل للأعلام.
- `docs/source/sorafs_proof_streaming.md` — Ajuste de la temperatura y del agua Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — تعمّق في fragmentación وتركيب المانيفست ومعالجة CAR.